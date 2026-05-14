"""Repo file enumeration and aggregation — single source of truth.

Orientation commands (`list`, `tree`, `info`, `context` primer) all need to
answer two questions about a directory:

  1. Which files belong to this directory under the project's view of the
     repo (tracked + non-ignored — `git ls-files` excludes anything the
     repo's gitignore excludes)?
  2. Summarized: how many, total complexity, last activity, any
     uncommitted state?

This module owns both. Callers never roll their own `Path.rglob` or
`os.walk` — they go through `tracked_files()` and `aggregate_paths()`. The
shared `SKIP_DIRS` set lives here too so build/dependency directories are
pruned consistently in the non-git fallback path (`walk_files`).
"""

from __future__ import annotations

import os
import subprocess
from pathlib import Path

from tracer import file_facts, git_activity


# Names of directories that orientation walks never enter. Build outputs,
# dependency caches, linked-worktree internals, and tracer's own cache.
SKIP_DIRS = frozenset({
    ".git",
    ".next",
    ".tracer-cache",
    ".venv",
    "__pycache__",
    "build",
    "dist",
    "node_modules",
    "venv",
    "vendor",
})

_DIRTY_STATES = frozenset({"untracked", "added", "modified", "renamed"})


def tracked_files(repo_root: Path, base: Path | None = None) -> list[str] | None:
    """Repo-root-relative paths of tracked + non-ignored files.

    Single `git ls-files --cached --others --exclude-standard` invocation.
    When `base` is provided, restricts the result to files under that
    sub-path. Returns None when git is unavailable or `base` is outside
    `repo_root` — callers fall back to `walk_files` in that case.
    """
    args = ["git", "ls-files", "--cached", "--others", "--exclude-standard"]
    if base is not None:
        try:
            rel_base = base.resolve().relative_to(repo_root.resolve())
        except ValueError:
            return None
        if str(rel_base) != ".":
            args.extend(["--", str(rel_base)])
    try:
        result = subprocess.run(
            args,
            cwd=repo_root,
            capture_output=True,
            text=True,
            check=True,
            timeout=30,
        )
    except (subprocess.CalledProcessError, FileNotFoundError, subprocess.TimeoutExpired):
        return None
    return [line for line in result.stdout.splitlines() if line]


def walk_files(base: Path) -> list[Path]:
    """Fallback enumerator for non-git directories. Prunes `SKIP_DIRS` and
    hidden directories at the directory boundary — never descends into
    them, unlike `Path.rglob` which walks them and filters after."""
    out: list[Path] = []
    for current, dirs, files in os.walk(base):
        dirs[:] = [d for d in dirs if d not in SKIP_DIRS and not d.startswith(".")]
        for name in files:
            if name.startswith("."):
                continue
            path = Path(current) / name
            if path.is_symlink() or not path.is_file():
                continue
            out.append(path)
    return out


def _path_signals(
    repo_root: Path,
    rel: str,
    source_exts: set[str],
    git_map: dict[str, git_activity.GitActivity],
) -> tuple[int, str | None, str | None]:
    """Per-file (ccn, last_modified, working_state). Source files pull from
    the warm file_facts cache via cache_only=True; non-source files fall
    through to the bulk-cached git activity map with ccn=0."""
    ext = Path(rel).suffix.lower()
    if ext in source_exts:
        facts = file_facts.get(
            repo_root / rel, repo_root=repo_root, cache_only=True
        )
        if facts is not None:
            return facts.cyclomatic_complexity_total, facts.last_modified, facts.working_state
    activity = git_map.get(rel) or git_activity.empty()
    return 0, activity.last_modified, activity.working_state


def aggregate_paths(
    repo_root: Path,
    relative_paths: list[str],
    source_exts: set[str],
    git_map: dict[str, git_activity.GitActivity],
) -> dict:
    """Summary stats for a pre-discovered file list — file count, total
    cyclomatic complexity, most-recent last_modified, and whether any file
    is in an uncommitted state."""
    file_count = len(relative_paths)
    ccn_total = 0
    last_modified: str | None = None
    has_uncommitted = False

    for rel in relative_paths:
        ccn, mod, state = _path_signals(repo_root, rel, source_exts, git_map)
        ccn_total += ccn
        if mod and (last_modified is None or mod > last_modified):
            last_modified = mod
        if state in _DIRTY_STATES:
            has_uncommitted = True

    return {
        "file_count": file_count,
        "ccn_total": ccn_total,
        "last_modified": last_modified,
        "has_uncommitted": has_uncommitted,
    }


