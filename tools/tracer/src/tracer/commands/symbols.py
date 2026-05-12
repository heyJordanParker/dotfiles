"""`trace symbols <file>` — module-level symbols of a file from the architecture graph."""

from __future__ import annotations

import json
import sys
from pathlib import Path

import click

from tracer import architecture, cache
from tracer.deps import require_dependencies


@click.command()
@click.argument("file", type=click.Path(exists=True, dir_okay=False))
@click.option("--json", "as_json", is_flag=True)
def command(file: str, as_json: bool) -> None:
    """Module-level symbols of `file`, sourced from the architecture graph."""
    require_dependencies()
    target = Path(file).resolve()
    repo_root = cache.repo_root_for(target)
    try:
        relative = str(target.relative_to(repo_root.resolve()))
    except ValueError:
        relative = str(target)

    graph = architecture.get(repo_root)
    file_symbols = [
        node for node in graph.nodes.values()
        if node.source_file == relative and node.kind != "module"
    ]

    if as_json:
        click.echo(
            json.dumps(
                {
                    "file": relative,
                    "symbols": [
                        {
                            "node_id": s.id,
                            "label": s.label,
                            "kind": s.kind,
                            "source_line": s.source_line,
                        }
                        for s in file_symbols
                    ],
                    "symbol_count": len(file_symbols),
                },
                indent=2,
            )
        )
        return

    click.echo(f"Symbols in {relative} ({len(file_symbols)}):")
    if not file_symbols:
        click.echo("  (no module-level symbols found in architecture graph)")
        click.echo("  (file may not have been extracted — check supported extensions via `trace doctor`)")
        return
    for s in sorted(file_symbols, key=lambda n: n.source_line or 0):
        click.echo(f"  L{s.source_line:<5} [{s.kind}] {s.label}")
