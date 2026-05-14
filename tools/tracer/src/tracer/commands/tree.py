"""`trace tree` — annotated file tree with complexity ranks."""

from __future__ import annotations

import json
from pathlib import Path

import click

from tracer import cache, file_facts, passive_context, repo_files
from tracer.deps import require_dependencies
from tracer.repo_context import repo_context


def _empty_ccn() -> dict:
    return {"ccn_total": 0, "ccn_max_function": 0, "loc": 0, "rank": "unknown"}


def _render_entry(full: Path, repo_root: Path) -> tuple[Path, dict, str | None]:
    facts = file_facts.get(full, repo_root=repo_root, cache_only=True)
    if facts is None:
        return (full, _empty_ccn(), None)
    return (
        full,
        {
            "ccn_total": facts.cyclomatic_complexity_total,
            "ccn_max_function": facts.cyclomatic_complexity_max,
            "loc": facts.loc,
            "rank": facts.rank,
        },
        passive_context.render_compact(facts),
    )


def _walk(path: Path, max_depth: int) -> list[tuple[Path, dict, str | None]]:
    """Annotated walk. File discovery via `repo_files.tracked_files` inside
    a git repo, `repo_files.walk_files` outside. Both honor the shared
    `SKIP_DIRS` set and never descend into ignored trees. file_facts
    queried with cache_only=True — tree never blocks on per-file
    extraction."""
    base = path.resolve()
    repo_root = cache.repo_root_for(base)
    tracked = repo_files.tracked_files(repo_root, base=base)

    entries: list[tuple[Path, dict, str | None]] = []
    if tracked is not None:
        for rel in sorted(tracked):
            full = repo_root / rel
            try:
                under_base = str(full.resolve().relative_to(base))
            except ValueError:
                continue
            if under_base.count("/") + 1 > max_depth:
                continue
            entries.append(_render_entry(full, repo_root))
        return entries

    for full in sorted(repo_files.walk_files(base)):
        try:
            depth = len(full.relative_to(base).parts)
        except ValueError:
            continue
        if depth > max_depth:
            continue
        entries.append(_render_entry(full, repo_root))
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
