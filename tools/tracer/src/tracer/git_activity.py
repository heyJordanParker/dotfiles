"""Bulk git activity extraction.

Replaces N×2 per-file `git log` subprocesses with 2 total invocations for
the whole repo. For a 750-file repo this drops from ~1500 subprocesses
(~75 seconds) to 2 subprocesses (<1 second).

The two queries:
  1. `git log --name-status -M --pretty=format:...` — walks every commit
     newest→oldest with rename detection. Records last_modified/author from
     the first appearance of each file, first_seen from the last appearance,
     total commit_count, and rename_from when the file's most recent touch
     was a rename.
  2. `git log --since=30days --name-only --oneline` — 30-day commit counts
     per file (count file appearances).

Both are O(commits) not O(files), which on real repos is dramatically less
work than O(files × 2 subprocesses).

A third lightweight call — `git ls-files --others --exclude-standard` —
flags working-tree files that have never been committed. Those carry
commit_count=0 in the resulting map.
"""

from __future__ import annotations

import subprocess
from dataclasses import asdict, dataclass
from datetime import datetime, timedelta, timezone
from pathlib import Path

from tracer import cache as _cache


@dataclass(frozen=True)
class GitActivity:
    last_modified: str | None
    last_author: str | None
    commits_30d: int
    first_seen: str | None
    commit_count: int
    rename_from: str | None
    # Working-tree state vs HEAD. None = clean.
    # "added"     — staged add, no committed history yet
    # "modified"  — uncommitted changes (staged or unstaged)
    # "renamed"   — staged rename
    # "deleted"   — staged deletion (file still on disk in some flows)
    # "untracked" — file on disk, not in index
    working_state: str | None
    # Deployed-branch presence. List of short branch labels where the file
    # exists at HEAD of that branch. Empty list = file exists on the current
    # working branch only (or wasn't found on any tracked deploy branch).
    # Closes the wrong-answer class where an agent treats a file with low
    # commit_count as "never deployed" without checking origin/production.
    present_in: tuple[str, ...] = ()


_EMPTY = GitActivity(
    last_modified=None,
    last_author=None,
    commits_30d=0,
    first_seen=None,
    commit_count=0,
    rename_from=None,
    working_state=None,
    present_in=(),
)


# Deployed branches we check for file presence, in display order. Tracked
# refspec → short label. Refs not present in the repo are silently skipped.
DEPLOY_BRANCHES = (
    ("prod", "origin/production"),
    ("staging", "origin/staging"),
    ("main", "origin/main"),
    ("main", "origin/master"),
)


def bulk(repo_root: Path) -> dict[str, GitActivity]:
    """Return relative-path -> GitActivity for every file in the repo.

    Includes tracked files (with history) and uncommitted/untracked files.
    Files outside the result map look up `_EMPTY`.
    """
    history = _walk_history(repo_root)
    counts_30d = _commits_30d_per_file(repo_root)
    working = _working_tree_state(repo_root)
    presence = _presence_by_path(repo_root)

    out: dict[str, GitActivity] = {}
    for path, info in history.items():
        out[path] = GitActivity(
            last_modified=info["last_modified"],
            last_author=info["last_author"],
            commits_30d=counts_30d.get(path, 0),
            first_seen=info["first_seen"],
            commit_count=info["commit_count"],
            rename_from=info["rename_from"],
            working_state=working.get(path),
            present_in=presence.get(path, ()),
        )
    # Surface working-tree-only entries (staged-but-uncommitted or untracked).
    for path, state in working.items():
        if path in out:
            continue
        out[path] = GitActivity(
            last_modified=None,
            last_author=None,
            commits_30d=0,
            first_seen=None,
            commit_count=0,
            rename_from=None,
            working_state=state,
            present_in=presence.get(path, ()),
        )
    return out


def empty() -> GitActivity:
    return _EMPTY


def bulk_cached(repo_root: Path) -> dict[str, GitActivity]:
    """Bulk git activity with disk caching keyed by HEAD SHA.

    Historical fields (last_modified, first_seen, commit_count, rename_from)
    are cached — they only change when HEAD moves. Working-tree state
    (untracked, staged add, modified) is recomputed fresh each call because
    it changes as the user edits, with no commit needed.
    """
    head = _head_sha(repo_root)
    if head is None:
        return bulk(repo_root)

    cache_key = f"git_activity__{head}"
    cached = _cache.load(_cache.NAMESPACE_FILE, cache_key, repo_root)

    if cached is not None:
        history: dict[str, GitActivity] = {}
        for path, fields in cached.items():
            history[path] = GitActivity(
                last_modified=fields.get("last_modified"),
                last_author=fields.get("last_author"),
                commits_30d=fields.get("commits_30d", 0),
                first_seen=fields.get("first_seen"),
                commit_count=fields.get("commit_count", 0),
                rename_from=fields.get("rename_from"),
                working_state=None,
                present_in=tuple(fields.get("present_in") or ()),
            )
    else:
        history = bulk(repo_root)
        payload = {
            path: {k: v for k, v in asdict(act).items() if k != "working_state"}
            for path, act in history.items()
        }
        _cache.save(_cache.NAMESPACE_FILE, cache_key, payload, repo_root)
        # The freshly-computed bulk already has working_state; re-fetching
        # below would just produce the same answer.
        return history

    working = _working_tree_state(repo_root)
    if not working:
        return history

    out = dict(history)
    for path, state in working.items():
        existing = out.get(path)
        if existing is None:
            out[path] = GitActivity(
                last_modified=None,
                last_author=None,
                commits_30d=0,
                first_seen=None,
                commit_count=0,
                rename_from=None,
                working_state=state,
                present_in=(),
            )
        else:
            out[path] = GitActivity(
                last_modified=existing.last_modified,
                last_author=existing.last_author,
                commits_30d=existing.commits_30d,
                first_seen=existing.first_seen,
                commit_count=existing.commit_count,
                rename_from=existing.rename_from,
                working_state=state,
                present_in=existing.present_in,
            )
    return out


def _presence_by_path(repo_root: Path) -> dict[str, tuple[str, ...]]:
    """For every file tracked on any deploy branch, return the labels of
    branches where it appears at that branch's tip.

    One `git ls-tree -r --name-only <branch>` per configured branch. The
    refs not present in the repo are silently skipped. Each call is bounded
    by repo size (single tree walk per branch), so even a 10k-file repo
    pays ~50ms × N(branches) once per cache key.
    """
    labels_for: dict[str, set[str]] = {}
    for label, ref in DEPLOY_BRANCHES:
        try:
            result = subprocess.run(
                ["git", "ls-tree", "-r", "--name-only", ref],
                cwd=repo_root,
                capture_output=True,
                text=True,
                check=True,
                timeout=30,
            )
        except (subprocess.CalledProcessError, FileNotFoundError, subprocess.TimeoutExpired):
            continue
        for path in result.stdout.splitlines():
            if not path:
                continue
            labels_for.setdefault(path, set()).add(label)
    return {path: tuple(sorted(labels)) for path, labels in labels_for.items()}


def _head_sha(repo_root: Path) -> str | None:
    try:
        result = subprocess.run(
            ["git", "rev-parse", "HEAD"],
            cwd=repo_root,
            capture_output=True,
            text=True,
            check=True,
            timeout=5,
        )
        return result.stdout.strip()
    except (subprocess.CalledProcessError, FileNotFoundError, subprocess.TimeoutExpired):
        return None


def _walk_history(repo_root: Path) -> dict[str, dict]:
    """Single git log pass that produces last/first/count/rename per file.

    `-M` triggers rename detection. Output:
      COMMIT|<date>|<author>
      M\tpath
      A\tpath
      D\tpath
      R100\told\tnew         (rename, percentage similarity)
      C75\tsource\tcopy      (copy, percentage similarity)
    """
    try:
        result = subprocess.run(
            [
                "git", "log",
                "-M",
                "--diff-merges=first-parent",
                "--name-status",
                "--pretty=format:COMMIT|%ad|%an",
                "--date=short",
            ],
            cwd=repo_root,
            capture_output=True,
            text=True,
            check=True,
            timeout=180,
        )
    except (subprocess.CalledProcessError, FileNotFoundError, subprocess.TimeoutExpired):
        return {}

    out: dict[str, dict] = {}
    current_date: str | None = None
    current_author: str | None = None
    for line in result.stdout.splitlines():
        if line.startswith("COMMIT|"):
            parts = line.split("|", 2)
            if len(parts) == 3:
                _, current_date, current_author = parts
            continue
        stripped = line.strip()
        if not stripped or current_date is None:
            continue
        # Parse status\tpath  OR  status\told\tnew (rename/copy).
        tokens = line.split("\t")
        if len(tokens) < 2:
            continue
        status = tokens[0]
        if status.startswith("R") or status.startswith("C"):
            if len(tokens) < 3:
                continue
            old_path, path = tokens[1], tokens[2]
            entry = _ensure(out, path, current_date, current_author)
            entry["commit_count"] += 1
            # Most recent rename wins — we read the log newest→oldest, so the
            # first time we see a rename for `path` is the most recent.
            if entry["rename_from"] is None:
                entry["rename_from"] = old_path
            entry["first_seen"] = current_date
            # Carry history of the old path forward so its commits also count
            # toward the new path. Walked in chronological-reverse, the old
            # name's commits show up after the rename — they belong to the
            # same file lineage.
            out.setdefault(
                f"__alias__::{old_path}",
                {"resolves_to": path},
            )
        else:
            path = tokens[1]
            # If this path has been renamed in a later commit, attribute the
            # touch to the new path (commit_count and first_seen).
            alias = out.get(f"__alias__::{path}")
            target = alias["resolves_to"] if alias else path
            entry = _ensure(out, target, current_date, current_author)
            entry["commit_count"] += 1
            entry["first_seen"] = current_date

    # Drop alias bookkeeping entries before returning.
    return {k: v for k, v in out.items() if not k.startswith("__alias__::")}


def _ensure(
    out: dict[str, dict], path: str, date: str, author: str | None
) -> dict:
    entry = out.get(path)
    if entry is None:
        entry = {
            "last_modified": date,
            "last_author": author or "",
            "first_seen": date,
            "commit_count": 0,
            "rename_from": None,
        }
        out[path] = entry
    return entry


def _commits_30d_per_file(repo_root: Path) -> dict[str, int]:
    """Count commits per file in the last 30 days."""
    since = (datetime.now(timezone.utc) - timedelta(days=30)).strftime("%Y-%m-%d")
    try:
        result = subprocess.run(
            [
                "git", "log",
                f"--since={since}",
                "--diff-merges=first-parent",
                "--name-only",
                "--pretty=format:",
            ],
            cwd=repo_root,
            capture_output=True,
            text=True,
            check=True,
            timeout=60,
        )
    except (subprocess.CalledProcessError, FileNotFoundError, subprocess.TimeoutExpired):
        return {}

    counts: dict[str, int] = {}
    for line in result.stdout.splitlines():
        if not line.strip():
            continue
        counts[line] = counts.get(line, 0) + 1
    return counts


def _working_tree_state(repo_root: Path) -> dict[str, str]:
    """Map relative-path -> working-state label for every file with
    uncommitted activity in the working tree.

    Uses `git status --porcelain=v1` with `-z` to handle paths containing
    spaces and renames (renames produce `R<X>  old\\0new`). State codes
    collapse to a short label the renderer can show without further
    interpretation.
    """
    try:
        result = subprocess.run(
            ["git", "status", "--porcelain=v1", "-z"],
            cwd=repo_root,
            capture_output=True,
            check=True,
            timeout=30,
        )
    except (subprocess.CalledProcessError, FileNotFoundError, subprocess.TimeoutExpired):
        return {}

    out: dict[str, str] = {}
    parts = result.stdout.split(b"\x00")
    i = 0
    while i < len(parts):
        chunk = parts[i].decode("utf-8", errors="replace")
        if len(chunk) < 3:
            i += 1
            continue
        xy = chunk[:2]
        path = chunk[3:]
        # Renames/copies: `R<X>` or `C<X>` then `old\0new`. The split above
        # already separated; the *next* part is the old path. We surface the
        # new path with state "renamed" — the rename_from comes from the log
        # walk for committed renames; staged renames don't have it yet.
        if xy[0] in ("R", "C") or xy[1] in ("R", "C"):
            if i + 1 < len(parts):
                # parts[i+1] is the source path; skip it.
                i += 2
            else:
                i += 1
            out[path] = "renamed"
            continue
        if "?" in xy:
            out[path] = "untracked"
        elif xy[0] == "A" or xy[1] == "A":
            out[path] = "added"
        elif xy[0] == "D" or xy[1] == "D":
            out[path] = "deleted"
        else:
            out[path] = "modified"
        i += 1
    return out
