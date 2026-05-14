"""`trace list` — fast annotated ls of a single directory.

Shows direct children only. For files: complexity rank + passive lifecycle
context. For sub-directories: file count, total CCN, and the most-recent
last_modified across their contents.

File discovery and per-subdir aggregation come from `tracer.repo_files`.
Inside a git repo this means a single `git ls-files` call partitioned by
top-level segment under the listed directory — never walks ignored trees
(`node_modules`, `vendor`, linked worktrees). Outside a repo, falls back
to a non-recursive direct-children listing.
"""

from __future__ import annotations

import json
from pathlib import Path

import click

from tracer import cache, file_facts, git_activity, passive_context, repo_files
from tracer.deps import require_dependencies
from tracer.extraction.dispatch import supported_extensions


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


def _partition_under_base(
    repo_root: Path, base: Path, repo_relative_paths: list[str]
) -> tuple[dict[str, list[str]], list[str]]:
    """Split repo-root-relative paths into (subdirs → paths) and direct
    files at `base`. Keys in the returned dict are subdir names directly
    under `base`; values stay repo-root-relative so they resolve via
    `repo_root / rel` and index `git_map` directly."""
    by_subdir: dict[str, list[str]] = {}
    direct_files: list[str] = []
    rel_base = base.resolve().relative_to(repo_root.resolve())
    rel_base_str = "" if str(rel_base) == "." else str(rel_base) + "/"
    for rel in repo_relative_paths:
        if rel_base_str and not rel.startswith(rel_base_str):
            continue
        under_base = rel[len(rel_base_str):]
        head, sep, _ = under_base.partition("/")
        if sep:
            by_subdir.setdefault(head, []).append(rel)
        else:
            direct_files.append(rel)
    return by_subdir, direct_files


@click.command()
@click.argument("path", type=click.Path(exists=True, file_okay=False))
@click.option("--all", "show_hidden", is_flag=True, help="Include dotfiles")
@click.option("--json", "as_json", is_flag=True)
def command(path: str, show_hidden: bool, as_json: bool) -> None:
    """Annotated ls of one directory: files + sub-directories with passive context."""
    require_dependencies()
    base = Path(path).resolve()
    repo_root = cache.repo_root_for(base)

    source_exts = supported_extensions()
    git_map = git_activity.bulk_cached(repo_root)
    tracked = repo_files.tracked_files(repo_root, base=base)

    dirs_out: list[dict] = []
    files_out: list[dict] = []

    if tracked is not None:
        by_subdir, direct_files = _partition_under_base(repo_root, base, tracked)

        for subdir_name in sorted(by_subdir):
            if subdir_name.startswith(".") and not show_hidden:
                continue
            if subdir_name in repo_files.SKIP_DIRS:
                continue
            summary = repo_files.aggregate_paths(repo_root, by_subdir[subdir_name], source_exts, git_map)
            dirs_out.append({"name": subdir_name, **summary})

        for rel in sorted(direct_files):
            fname = Path(rel).name
            if fname.startswith(".") and not show_hidden:
                continue
            facts = file_facts.get(base / fname, repo_root=repo_root, cache_only=True)
            files_out.append({"name": fname, "facts": facts})
    else:
        children = sorted(base.iterdir(), key=lambda p: (not p.is_dir(), p.name.lower()))
        for child in children:
            if child.name.startswith(".") and not show_hidden:
                continue
            if child.is_symlink():
                continue
            if child.is_dir():
                if child.name in repo_files.SKIP_DIRS:
                    continue
                dirs_out.append({"name": child.name, "file_count": 0, "ccn_total": 0,
                                  "last_modified": None, "has_uncommitted": False})
            elif child.is_file():
                facts = file_facts.get(child, repo_root=repo_root, cache_only=True)
                files_out.append({"name": child.name, "facts": facts})

    if as_json:
        click.echo(
            json.dumps(
                {
                    "path": str(base),
                    "directories": dirs_out,
                    "files": [
                        {
                            "name": f["name"],
                            "rank": f["facts"].rank if f["facts"] else "unknown",
                            "ccn_total": f["facts"].cyclomatic_complexity_total if f["facts"] else 0,
                            "passive_context": passive_context.render_compact(f["facts"]) if f["facts"] else None,
                        }
                        for f in files_out
                    ],
                },
                indent=2,
            )
        )
        return

    click.echo(f"{base}/")
    for d in sorted(dirs_out, key=lambda x: x["name"].lower()):
        click.echo(_format_dir_line(d["name"], d))
    for f in sorted(files_out, key=lambda x: x["name"].lower()):
        click.echo(_format_file_line(f["name"], f["facts"]))
