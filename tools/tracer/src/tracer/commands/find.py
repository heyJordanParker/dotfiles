"""`trace find` — file-name pattern search with code intelligence.

Replaces the dominant `find <dir> -type f -name "*.ext"` agent shape (and
its path-constrained variants) with one call that returns matching paths
annotated with complexity rank and the standard lifecycle shoulder.

Respects .gitignore when inside a git repo (uses `git ls-files`), falls
back to a SKIP_DIRS-bounded walk otherwise.
"""

from __future__ import annotations

import fnmatch
import json
import os
import subprocess
from pathlib import Path

import click

from tracer import cache, file_facts, passive_context
from tracer.deps import require_dependencies


_SKIP_DIRS = {
    ".git", "node_modules", ".venv", "venv", "__pycache__", "dist", "build",
    ".next", ".tracer-cache", ".pytest_cache", ".mypy_cache", ".ruff_cache",
    "vendor", "worktrees", ".lando", "playwright-report", "test-results",
}


def _list_files(base: Path, repo_root: Path, include_dirs: bool) -> list[Path]:
    """Enumerate candidate paths under `base`.

    Uses git ls-files when `base` is inside a git repo (respects .gitignore
    transparently). Falls back to os.walk with SKIP_DIRS bounding.
    """
    base_abs = base.resolve()
    try:
        result = subprocess.run(
            ["git", "ls-files", "--cached", "--others", "--exclude-standard"],
            cwd=base_abs,
            capture_output=True,
            text=True,
            check=True,
            timeout=30,
        )
        files = [base_abs / line for line in result.stdout.splitlines() if line]
        if include_dirs:
            dirs: set[Path] = set()
            for f in files:
                for parent in f.parents:
                    if parent == base_abs:
                        break
                    dirs.add(parent)
            return files + sorted(dirs)
        return files
    except (subprocess.CalledProcessError, FileNotFoundError, subprocess.TimeoutExpired):
        return _walk(base_abs, include_dirs)


def _walk(base: Path, include_dirs: bool) -> list[Path]:
    out: list[Path] = []
    for root, dirs, files in os.walk(base):
        dirs[:] = [d for d in dirs if d not in _SKIP_DIRS and not d.startswith(".")]
        for name in files:
            out.append(Path(root) / name)
        if include_dirs:
            for name in dirs:
                out.append(Path(root) / name)
    return out


def _match(
    path: Path,
    pattern: str,
    path_filter: str | None,
    excludes: tuple[str, ...],
) -> bool:
    """Match pattern against basename (glob) and optional path substring.

    `excludes` is a tuple of path globs; any match rejects the entry. Mirrors
    `find -not -path "*/vendor/*"` semantics.
    """
    if not fnmatch.fnmatch(path.name, pattern):
        return False
    if path_filter and not fnmatch.fnmatch(str(path), path_filter):
        return False
    for exclude in excludes:
        if fnmatch.fnmatch(str(path), exclude):
            return False
    return True


@click.command()
@click.argument("pattern")
@click.argument("base", required=False, default=".")
@click.option(
    "--path",
    "path_filter",
    default=None,
    help="Additional glob filter against the full path (e.g. '*/controllers/*')",
)
@click.option(
    "--exclude",
    "excludes",
    multiple=True,
    help="Reject paths matching this glob (repeatable). Mirrors find -not -path '*X*'",
)
@click.option(
    "--type",
    "type_filter",
    type=click.Choice(["f", "d"], case_sensitive=False),
    default="f",
    help="Match files (f) or directories (d). Default: f",
)
@click.option(
    "--limit",
    type=int,
    default=200,
    help="Max results to return. Default: 200",
)
@click.option(
    "--sort",
    type=click.Choice(["complexity", "recent", "path"], case_sensitive=False),
    default="path",
    help="Sort results by complexity, recency, or path",
)
@click.option("--json", "as_json", is_flag=True)
def command(
    pattern: str,
    base: str,
    path_filter: str | None,
    excludes: tuple[str, ...],
    type_filter: str,
    limit: int,
    sort: str,
    as_json: bool,
) -> None:
    """Find files (or directories) by name pattern with code intelligence."""
    require_dependencies()
    base_path = Path(base).resolve()
    if not base_path.exists():
        click.echo(f"Error: {base} does not exist", err=True)
        raise SystemExit(2)

    repo_root = cache.repo_root_for(base_path)
    include_dirs = type_filter.lower() == "d"
    candidates = _list_files(base_path, repo_root, include_dirs)

    if include_dirs:
        candidates = [p for p in candidates if p.is_dir()]
    else:
        candidates = [p for p in candidates if p.is_file()]

    matches = [p for p in candidates if _match(p, pattern, path_filter, excludes)]

    entries: list[dict] = []
    for path in matches:
        try:
            relative = str(path.relative_to(repo_root.resolve()))
        except ValueError:
            relative = str(path)
        if include_dirs:
            entries.append(
                {
                    "path": relative,
                    "kind": "directory",
                    "ccn_total": 0,
                    "ccn_rank": None,
                    "shoulder": None,
                    "last_modified": None,
                }
            )
            continue
        facts = file_facts.get(path, repo_root=repo_root)
        if facts is None:
            entries.append(
                {
                    "path": relative,
                    "kind": "file",
                    "ccn_total": 0,
                    "ccn_rank": "unknown",
                    "shoulder": None,
                    "last_modified": None,
                }
            )
            continue
        entries.append(
            {
                "path": relative,
                "kind": "file",
                "ccn_total": facts.cyclomatic_complexity_total,
                "ccn_rank": facts.rank,
                "shoulder": passive_context.render_compact(facts),
                "last_modified": facts.last_modified,
            }
        )

    if sort == "complexity":
        entries.sort(key=lambda e: (-(e["ccn_total"] or 0), e["path"]))
    elif sort == "recent":
        entries.sort(key=lambda e: (e["last_modified"] or "", e["path"]), reverse=True)
    else:
        entries.sort(key=lambda e: e["path"])

    truncated = len(entries) > limit
    entries = entries[:limit]

    if as_json:
        click.echo(
            json.dumps(
                {
                    "pattern": pattern,
                    "base": str(base_path),
                    "path_filter": path_filter,
                    "excludes": list(excludes),
                    "type": type_filter,
                    "match_count": len(entries),
                    "truncated": truncated,
                    "entries": entries,
                },
                indent=2,
            )
        )
        return

    if not entries:
        click.echo("(no matches)")
        return

    click.echo(f"{len(entries)} matches under {base_path}:")
    for entry in entries:
        if entry["kind"] == "directory":
            click.echo(f"  📁 {entry['path']}/")
            continue
        ccn = entry["ccn_total"]
        rank = entry["ccn_rank"]
        shoulder = entry["shoulder"] or ""
        click.echo(f"  {entry['path']}  [ccn={ccn} {rank}] {shoulder}")
    if truncated:
        click.echo(f"... truncated to {limit} entries (raise with --limit)")
