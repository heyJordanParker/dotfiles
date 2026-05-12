"""`trace structure` — methods, properties, variables, connections.

Symbols come from universal-ctags (broad coverage). Imports come from the
per-file cache (tree-sitter-extracted, accurate). Per-method CCN matches
ctags lines to lizard's function list via the cache.
"""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

import click

from tracer import file_facts
from tracer.deps import require_dependencies


CTAGS_KIND_LABELS = {
    "c": "class",
    "f": "function",
    "m": "method",
    "v": "variable",
    "p": "property",
    "F": "field",
    "i": "import",
    "I": "import",
    "n": "namespace",
    "s": "struct",
    "e": "enum",
    "g": "enum",
    "t": "trait",
    "u": "union",
}


def _ctags_symbols(path: Path) -> list[dict]:
    try:
        result = subprocess.run(
            [
                "ctags",
                "--output-format=json",
                "--fields=+nezKSt",
                "--sort=no",
                "-f", "-",
                str(path),
            ],
            capture_output=True,
            text=True,
            check=False,
            timeout=30,
        )
    except (FileNotFoundError, subprocess.TimeoutExpired) as e:
        click.echo(f"Error: ctags failed: {e}", err=True)
        sys.exit(1)

    symbols: list[dict] = []
    for line in result.stdout.splitlines():
        if not line.strip():
            continue
        try:
            entry = json.loads(line)
        except json.JSONDecodeError:
            continue
        kind_raw = entry.get("kind", "")
        symbols.append(
            {
                "name": entry.get("name"),
                "kind": CTAGS_KIND_LABELS.get(kind_raw, kind_raw),
                "line": entry.get("line"),
                "scope": entry.get("scope"),
                "scope_kind": entry.get("scopeKind"),
                "signature": entry.get("signature"),
            }
        )
    return symbols


@click.command()
@click.argument("path", type=click.Path(exists=True, dir_okay=False))
@click.option("--json", "as_json", is_flag=True)
def command(path: str, as_json: bool) -> None:
    """Methods, properties, variables, and the connections between them."""
    require_dependencies()
    p = Path(path).resolve()
    facts = file_facts.get(p)
    symbols = _ctags_symbols(p)

    # Build line -> ccn map from cached lizard parse (via FileFacts)
    by_line: dict[int, int] = {}
    if facts and facts.function_count > 0:
        # FileFacts stores aggregates; for per-method CCN we re-parse here
        # because the cache stores totals not per-function. Cheap.
        try:
            import lizard
            parsed = lizard.analyze_file(str(p))
            by_line = {f.start_line: f.cyclomatic_complexity for f in parsed.function_list}
        except Exception:
            by_line = {}

    for symbol in symbols:
        if symbol.get("line") in by_line:
            symbol["cyclomatic_complexity"] = by_line[symbol["line"]]

    # Imports come from cached extraction (tree-sitter)
    imports = []
    exports = []
    if facts and facts.extraction:
        imports = [
            {"module": i.module, "symbol": i.symbol, "line": i.line}
            for i in facts.extraction.imports
        ]
        exports = [
            {"name": e.name, "kind": e.kind, "line": e.line}
            for e in facts.extraction.exports
        ]

    by_kind: dict[str, list[dict]] = {}
    for symbol in symbols:
        by_kind.setdefault(symbol["kind"], []).append(symbol)

    if as_json:
        click.echo(
            json.dumps(
                {
                    "file": str(p),
                    "language": facts.language if facts else None,
                    "imports": imports,
                    "exports": exports,
                    "symbols_by_kind": by_kind,
                    "symbol_count": len(symbols),
                },
                indent=2,
            )
        )
        return

    click.echo(f"File: {p}")
    click.echo(f"Language: {facts.language if facts else '(unknown)'}")
    click.echo(f"Symbols: {len(symbols)}  Imports: {len(imports)}  Exports: {len(exports)}")
    click.echo("")
    if imports:
        click.echo("Imports:")
        for i in imports:
            symbol_part = f" -> {i['symbol']}" if i["symbol"] else ""
            click.echo(f"  L{i['line']:<5} {i['module']}{symbol_part}")
        click.echo("")
    if exports:
        click.echo("Exports:")
        for e in exports:
            click.echo(f"  L{e['line']:<5} [{e['kind']}] {e['name']}")
        click.echo("")
    for kind in sorted(by_kind):
        click.echo(f"{kind}s:")
        for symbol in by_kind[kind]:
            ccn_str = f" cyclomatic_complexity={symbol['cyclomatic_complexity']}" if "cyclomatic_complexity" in symbol else ""
            sig = f" {symbol['signature']}" if symbol.get("signature") else ""
            scope = f" [in {symbol['scope']}]" if symbol.get("scope") else ""
            click.echo(f"  L{symbol['line']:<5} {symbol['name']}{sig}{scope}{ccn_str}")
        click.echo("")
