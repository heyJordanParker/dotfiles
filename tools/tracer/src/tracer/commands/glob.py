"""`trace glob` — file-path pattern search.

Mirrors Claude Code's Glob tool: a pattern matched against full paths under a
base directory, with `**` for recursive descent. Returns the complete matched
file list, deterministically sorted by path, suitable as a strict replacement
for native Glob.

Default output is paths only. `--details` adds per-line ccn + rank + lifecycle
shoulder — the shape the deleted `trace context --glob` mode used to emit.

Gitignore is respected: the universe of candidate files comes from
`git ls-files --cached --others --exclude-standard`; pattern matching runs
against that universe. Outside a git repo, a SKIP_DIRS-bounded walk is the
fallback.
"""

from __future__ import annotations

import json
import os
import subprocess
from pathlib import Path

import click

from tracer import cache, file_facts, passive_context
from tracer.deps import require_dependencies


# Directories the fallback walker (non-git base) refuses to descend into.
# Matches the SKIP_DIRS used elsewhere in tracer for consistency.
_SKIP_DIRS = {
    ".git", "node_modules", ".venv", "venv", "__pycache__", "dist", "build",
    ".next", ".tracer-cache", ".pytest_cache", ".mypy_cache", ".ruff_cache",
    "vendor", "worktrees",
}


def _tracked_universe(base: Path) -> set[Path] | None:
    """Set of absolute paths git considers tracked-or-not-ignored under `base`.

    Returns None when `base` is not inside a git repo — caller falls back to
    the SKIP_DIRS walker.
    """
    try:
        result = subprocess.run(
            ["git", "ls-files", "--cached", "--others", "--exclude-standard"],
            cwd=base,
            capture_output=True,
            text=True,
            check=True,
            timeout=30,
        )
    except (subprocess.CalledProcessError, FileNotFoundError, subprocess.TimeoutExpired):
        return None
    return {(base / line).resolve() for line in result.stdout.splitlines() if line}


def _walk_universe(base: Path) -> set[Path]:
    """Fallback file universe when `base` is not in a git repo."""
    out: set[Path] = set()
    for current_root, dirs, files in os.walk(base):
        dirs[:] = [d for d in dirs if d not in _SKIP_DIRS and not d.startswith(".")]
        for name in files:
            out.add((Path(current_root) / name).resolve())
    return out


def _resolve_glob(pattern: str, base: Path) -> list[Path]:
    """Resolve `pattern` against `base` and intersect with the tracked universe.

    Returns absolute paths to regular files, deterministically sorted.
    """
    try:
        raw_matches = list(base.glob(pattern))
    except (OSError, ValueError):
        return []

    universe = _tracked_universe(base)
    ignore_policy = "gitignore"
    if universe is None:
        universe = _walk_universe(base)
        ignore_policy = "skip_dirs"

    matches = [
        path.resolve() for path in raw_matches
        if path.is_file() and path.resolve() in universe
    ]
    matches.sort()
    # Stash the policy on the function call site via attribute — caller
    # reads it back to populate the JSON envelope.
    _resolve_glob.last_ignore_policy = ignore_policy  # type: ignore[attr-defined]
    return matches


def _render_detail_line(path: Path, base: Path, repo_root: Path) -> dict:
    """Build the per-match detail payload (also reused in --json --details)."""
    try:
        rel = str(path.relative_to(base))
    except ValueError:
        rel = str(path)
    facts = file_facts.get(path, repo_root=repo_root)
    if facts is None:
        return {
            "path": rel,
            "ccn_total": 0,
            "rank": "unknown",
            "shoulder": None,
        }
    return {
        "path": rel,
        "ccn_total": facts.cyclomatic_complexity_total,
        "rank": facts.rank,
        "shoulder": passive_context.render_compact(facts),
    }


@click.command()
@click.argument("pattern")
@click.argument("base", required=False, default=".")
@click.option(
    "--details",
    is_flag=True,
    help="Per-line ccn + rank + lifecycle shoulder (vs bare paths by default).",
)
@click.option("--json", "as_json", is_flag=True, help="Machine-parseable JSON output.")
def command(pattern: str, base: str, details: bool, as_json: bool) -> None:
    """File-path pattern search with Claude Glob parity.

    Returns the complete file list matching `pattern` under `base`,
    deterministically sorted by path, respecting .gitignore inside a git repo.
    """
    require_dependencies()
    base_path = Path(base).resolve()
    if not base_path.exists():
        click.echo(f"Error: {base} does not exist", err=True)
        raise SystemExit(2)
    if not base_path.is_dir():
        click.echo(f"Error: {base} is not a directory", err=True)
        raise SystemExit(2)

    matches = _resolve_glob(pattern, base_path)
    ignore_policy = getattr(_resolve_glob, "last_ignore_policy", "gitignore")

    if as_json:
        repo_root = cache.repo_root_for(base_path)
        if details:
            entries = [_render_detail_line(p, base_path, repo_root) for p in matches]
            payload = {
                "pattern": pattern,
                "base": str(base_path),
                "ignore_policy": ignore_policy,
                "match_count": len(entries),
                "matches": entries,
            }
        else:
            rel_paths = []
            for p in matches:
                try:
                    rel_paths.append(str(p.relative_to(base_path)))
                except ValueError:
                    rel_paths.append(str(p))
            payload = {
                "pattern": pattern,
                "base": str(base_path),
                "ignore_policy": ignore_policy,
                "match_count": len(rel_paths),
                "matches": rel_paths,
            }
        click.echo(json.dumps(payload, indent=2))
        return

    if not matches:
        click.echo("(no matches)")
        return

    repo_root = cache.repo_root_for(base_path)
    if details:
        for path in matches:
            entry = _render_detail_line(path, base_path, repo_root)
            shoulder = entry["shoulder"] or ""
            click.echo(
                f"{entry['path']}  [ccn={entry['ccn_total']} {entry['rank']}]  {shoulder}"
            )
    else:
        for path in matches:
            try:
                click.echo(str(path.relative_to(base_path)))
            except ValueError:
                click.echo(str(path))
