"""`trace grep` — text search with rich per-match context.

Wraps ripgrep --json. For each match enriches with file_complexity (lizard),
nearest_doc (walk-up search), and git activity (log + 30d count). Adds
`repo_context.complexity_p95` for read-depth calibration.
"""

from __future__ import annotations

import json
import subprocess
from pathlib import Path

import click

from tracer.deps import require_dependencies
from tracer.enrich import file_complexity, git_context, nearest_doc
from tracer.repo_context import repo_context


def _ripgrep(pattern: str, path: str, lang: str | None) -> list[dict]:
    cmd = ["rg", "--json"]
    if lang:
        cmd.extend(["--type", lang])
    cmd.extend([pattern, path])
    result = subprocess.run(cmd, capture_output=True, text=True, check=False, timeout=60)
    matches: list[dict] = []
    for line in result.stdout.splitlines():
        try:
            event = json.loads(line)
        except json.JSONDecodeError:
            continue
        if event.get("type") != "match":
            continue
        data = event["data"]
        matches.append(
            {
                "file": data["path"]["text"],
                "line": data["line_number"],
                "snippet": data["lines"]["text"].rstrip("\n"),
            }
        )
    return matches


@click.command()
@click.argument("pattern")
@click.option("-l", "--lang", default=None, help="ripgrep --type filter (e.g. py, ts)")
@click.option("--path", default=".", help="Path to search (default: cwd)")
@click.option("--json", "as_json", is_flag=True)
def command(pattern: str, lang: str | None, path: str, as_json: bool) -> None:
    """Text search with per-match architectural enrichment."""
    require_dependencies()
    matches = _ripgrep(pattern, path, lang)

    # Cache file-level enrichment (one parse per file even with many matches)
    file_cache: dict[str, dict] = {}
    enriched: list[dict] = []
    for m in matches:
        fpath = m["file"]
        if fpath not in file_cache:
            file_cache[fpath] = {
                "file_complexity": file_complexity(fpath),
                "nearest_doc": nearest_doc(fpath),
                "git": git_context(fpath),
            }
        enriched.append({**m, **file_cache[fpath]})

    output = {
        "query": pattern,
        "lang_filter": lang,
        "matches": enriched,
        "match_count": len(enriched),
        "files_matched": len(file_cache),
        "repo_context": repo_context(path),
    }

    if as_json:
        click.echo(json.dumps(output, indent=2))
        return

    if not enriched:
        click.echo("(no matches)")
        return

    current_file: str | None = None
    for m in enriched:
        if m["file"] != current_file:
            current_file = m["file"]
            ccn = m["file_complexity"]
            doc = m["nearest_doc"] or "(no doc)"
            git = m["git"]
            click.echo("")
            click.echo(f"{current_file}  [ccn={ccn['ccn_total']} {ccn['rank']}, last={git['last_modified']}, doc={doc}]")
        click.echo(f"  L{m['line']:<5} {m['snippet']}")

    ctx = output["repo_context"]
    click.echo("")
    click.echo(f"matches={output['match_count']} files={output['files_matched']} repo_p95={ctx['complexity_p95']}")
