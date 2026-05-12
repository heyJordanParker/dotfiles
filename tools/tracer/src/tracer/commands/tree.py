"""`trace tree` — annotated file tree with complexity ranks."""

from __future__ import annotations

import json
import os
from pathlib import Path

import click

from tracer import file_facts, passive_context
from tracer.deps import require_dependencies
from tracer.enrich import file_complexity
from tracer.repo_context import repo_context


SKIP_DIRS = {".git", "node_modules", ".venv", "venv", "__pycache__", "dist", "build", ".next"}


def _walk(path: Path, max_depth: int) -> list[tuple[Path, dict, str | None]]:
    entries: list[tuple[Path, dict, str | None]] = []
    for root, dirs, files in os.walk(path):
        rel_root = Path(root).relative_to(path)
        depth = 0 if str(rel_root) == "." else len(rel_root.parts)
        if depth >= max_depth:
            dirs[:] = []
            continue
        dirs[:] = sorted([d for d in dirs if d not in SKIP_DIRS and not d.startswith(".")])
        for fname in sorted(files):
            full = Path(root) / fname
            if full.is_symlink():
                continue
            facts = file_facts.get(full)
            ctx = passive_context.render_compact(facts) if facts else None
            entries.append((full, file_complexity(full), ctx))
    return entries


def _format_tree(entries: list[tuple[Path, dict, str | None]], base: Path) -> str:
    lines: list[str] = [f"{base}/"]
    for path, ccn, ctx in entries:
        rel = path.relative_to(base)
        indent = "  " * (len(rel.parts) - 1)
        rank = ccn["rank"]
        marker = {"low": "·", "medium": "•", "high": "●", "critical": "⚠"}.get(rank, "?")
        suffix = f" — {ctx}" if ctx else ""
        lines.append(
            f"{indent}{marker} {rel.name}  [ccn={ccn['ccn_total']} loc={ccn['loc']} {rank}]{suffix}"
        )
    return "\n".join(lines)


@click.command()
@click.argument("path", type=click.Path(exists=True))
@click.option("--depth", default=4, help="Max directory depth to walk")
@click.option("--json", "as_json", is_flag=True)
def command(path: str, depth: int, as_json: bool) -> None:
    """Annotated file tree with per-file complexity ranks."""
    require_dependencies()
    base = Path(path).resolve()
    entries = _walk(base, depth)
    ctx = repo_context(str(base))

    if as_json:
        click.echo(
            json.dumps(
                {
                    "root": str(base),
                    "files": [
                        {"path": str(p.relative_to(base)), **c, "passive_context": pc}
                        for p, c, pc in entries
                    ],
                    "repo_context": ctx,
                },
                indent=2,
            )
        )
    else:
        click.echo(_format_tree(entries, base))
        click.echo("")
        click.echo(f"repo_context: complexity_p95={ctx['complexity_p95']} median={ctx['median_file_ccn']} files={ctx['total_files']}")
