"""`trace list` — fast annotated ls of a single directory.

Shows direct children only (no recursion). For files: complexity rank +
passive lifecycle context. For sub-directories: file count, total CCN,
and the most-recent last_modified across their contents.

One shot at orientation: replaces glob + multiple stat reads with a single
tool call. Cheaper than `tree` for shallow surveys; more useful than `info`
on a dir because it surfaces directories as first-class entries.
"""

from __future__ import annotations

import json
from pathlib import Path

import click

from tracer import file_facts, passive_context
from tracer.deps import require_dependencies


SKIP_DIRS = {".git", "node_modules", ".venv", "venv", "__pycache__", "dist", "build", ".next", "vendor"}


def _dir_summary(directory: Path) -> dict:
    """Aggregate stats for one sub-directory, one file_facts pass."""
    file_count = 0
    ccn_total = 0
    last_modified: str | None = None
    has_uncommitted = False
    for path in directory.rglob("*"):
        if not path.is_file():
            continue
        if any(part.startswith(".") or part in SKIP_DIRS for part in path.parts):
            continue
        facts = file_facts.get(path)
        if facts is None:
            continue
        file_count += 1
        ccn_total += facts.cyclomatic_complexity_total
        if facts.last_modified and (last_modified is None or facts.last_modified > last_modified):
            last_modified = facts.last_modified
        if facts.working_state in {"untracked", "added", "modified", "renamed"}:
            has_uncommitted = True
    return {
        "file_count": file_count,
        "ccn_total": ccn_total,
        "last_modified": last_modified,
        "has_uncommitted": has_uncommitted,
    }


def _format_dir_line(name: str, summary: dict) -> str:
    bits = [f"{summary['file_count']} files", f"ccn={summary['ccn_total']}"]
    if summary["last_modified"]:
        bits.append(f"last: {summary['last_modified']}")
    if summary["has_uncommitted"]:
        bits.append("uncommitted")
    return f"  📁 {name}/  ({' · '.join(bits)})"


def _format_file_line(name: str, facts) -> str:
    if facts is None:
        return f"  📄 {name}"
    rank_marker = {"low": "·", "medium": "•", "high": "●", "critical": "⚠"}.get(facts.rank, "?")
    return f"  {rank_marker} {name}  [ccn={facts.cyclomatic_complexity_total} {facts.rank}] {passive_context.render_compact(facts)}"


@click.command()
@click.argument("path", type=click.Path(exists=True, file_okay=False))
@click.option("--all", "show_hidden", is_flag=True, help="Include dotfiles")
@click.option("--json", "as_json", is_flag=True)
def command(path: str, show_hidden: bool, as_json: bool) -> None:
    """Annotated ls of one directory: files + sub-directories with passive context."""
    require_dependencies()
    base = Path(path).resolve()
    children = sorted(base.iterdir(), key=lambda p: (not p.is_dir(), p.name.lower()))

    dirs_out: list[dict] = []
    files_out: list[dict] = []

    for child in children:
        if child.name.startswith(".") and not show_hidden:
            continue
        if child.is_symlink():
            continue
        if child.is_dir():
            if child.name in SKIP_DIRS:
                continue
            summary = _dir_summary(child)
            dirs_out.append({"name": child.name, **summary})
        elif child.is_file():
            facts = file_facts.get(child)
            files_out.append(
                {
                    "name": child.name,
                    "rank": facts.rank if facts else "unknown",
                    "ccn_total": facts.cyclomatic_complexity_total if facts else 0,
                    "passive_context": passive_context.render_compact(facts) if facts else None,
                }
            )

    if as_json:
        click.echo(
            json.dumps(
                {"path": str(base), "directories": dirs_out, "files": files_out},
                indent=2,
            )
        )
        return

    click.echo(f"{base}/")
    for d in dirs_out:
        click.echo(_format_dir_line(d["name"], d))
    for f in files_out:
        # Re-fetch facts for the formatter — single stat() via the mtime
        # fast-path, no extraction work.
        facts = file_facts.get(base / f["name"])
        click.echo(_format_file_line(f["name"], facts))
