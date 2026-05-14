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
from collections import Counter
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
    # Subject of the most recent commit that touched this file. Renders
    # alongside the shoulder so the agent doesn't need a follow-up
    # `git log -1 --format=%s` after reading.
    last_subject: str | None = None
    # Author with the most commits to this file (over full history). May
    # differ from last_author (most recent committer). Surfaces ownership
    # without a separate `trace blame` call.
    top_author: str | None = None
    # Top files that change together with this one, ranked by commit
    # co-occurrence count. Capped at 5 entries to bound cache size.
    co_changed: tuple[tuple[str, int], ...] = ()


_EMPTY = GitActivity(
    last_modified=None,
    last_author=None,
    commits_30d=0,
    first_seen=None,
    commit_count=0,
    rename_from=None,
    working_state=None,
    present_in=(),
    last_subject=None,
    top_author=None,
    co_changed=(),
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
            last_subject=info.get("last_subject"),
            top_author=info.get("top_author"),
            co_changed=tuple(info.get("co_changed") or ()),
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
            last_subject=None,
            top_author=None,
            co_changed=(),
        )
    return out


def empty() -> GitActivity:
    return _EMPTY


# Per-process memo: 1500 `_lite_facts` callers shouldn't each spawn a
# `git rev-parse HEAD` subprocess + JSON parse + reconstruct ~N
# `GitActivity` dataclasses. Caller IDs the repo by absolute path; the map
# is recomputed once per invocation (short-lived process is the invariant).
_BULK_CACHE: dict[str, dict[str, "GitActivity"]] = {}


def bulk_cached(repo_root: Path) -> dict[str, GitActivity]:
    """Bulk git activity with disk caching keyed by HEAD SHA + process-level
    memo so repeat callers in the same tracer invocation pay only once.

    Historical fields (last_modified, first_seen, commit_count, rename_from)
    are cached — they only change when HEAD moves. Working-tree state
    (untracked, staged add, modified) is recomputed fresh each call because
    it changes as the user edits, with no commit needed.
    """
    memo_key = str(repo_root.resolve())
    memoed = _BULK_CACHE.get(memo_key)
    if memoed is not None:
        return memoed

    head = _head_sha(repo_root)
    if head is None:
        result = bulk(repo_root)
        _BULK_CACHE[memo_key] = result
        return result

    cache_key = f"git_activity__{head}"
    cached = _cache.load(_cache.NAMESPACE_FILE, cache_key, repo_root)

    if cached is not None:
        history: dict[str, GitActivity] = {}
        for path, fields in cached.items():
            co = fields.get("co_changed") or ()
            history[path] = GitActivity(
                last_modified=fields.get("last_modified"),
                last_author=fields.get("last_author"),
                commits_30d=fields.get("commits_30d", 0),
                first_seen=fields.get("first_seen"),
                commit_count=fields.get("commit_count", 0),
                rename_from=fields.get("rename_from"),
                working_state=None,
                present_in=tuple(fields.get("present_in") or ()),
                last_subject=fields.get("last_subject"),
                top_author=fields.get("top_author"),
                co_changed=tuple((p, c) for p, c in co),
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
        _BULK_CACHE[memo_key] = history
        return history

    working = _working_tree_state(repo_root)
    if not working:
        _BULK_CACHE[memo_key] = history
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
                last_subject=None,
                top_author=None,
                co_changed=(),
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
                last_subject=existing.last_subject,
                top_author=existing.top_author,
                co_changed=existing.co_changed,
            )
    _BULK_CACHE[memo_key] = out
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
      COMMIT|<date>|<author>|<subject>
      M\tpath
      A\tpath
      D\tpath
      R100\told\tnew         (rename, percentage similarity)
      C75\tsource\tcopy      (copy, percentage similarity)

    Also derives, in the same pass:
      - last_subject: subject of the newest commit touching the file
      - top_author: author with the most commits to the file
      - co_changed: top-5 files that change together with this one
    """
    try:
        result = subprocess.run(
            [
                "git", "log",
                "-M",
                "--diff-merges=first-parent",
                "--name-status",
                "--pretty=format:COMMIT|%ad|%an|%s",
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
    authors_by_path: dict[str, Counter[str]] = {}
    co_by_path: dict[str, Counter[str]] = {}
    current_date: str | None = None
    current_author: str | None = None
    current_subject: str | None = None
    current_paths: set[str] = set()

    def _flush_co() -> None:
        if len(current_paths) <= 1:
            return
        paths = list(current_paths)
        for a in paths:
            counter = co_by_path.setdefault(a, Counter())
            for b in paths:
                if a != b:
                    counter[b] += 1

    for line in result.stdout.splitlines():
        if line.startswith("COMMIT|"):
            _flush_co()
            current_paths = set()
            parts = line.split("|", 3)
            if len(parts) == 4:
                _, current_date, current_author, current_subject = parts
            elif len(parts) == 3:
                _, current_date, current_author = parts
                current_subject = None
            continue
        stripped = line.strip()
        if not stripped or current_date is None:
            continue
        tokens = line.split("\t")
        if len(tokens) < 2:
            continue
        status = tokens[0]
        if status.startswith("R") or status.startswith("C"):
            if len(tokens) < 3:
                continue
            old_path, path = tokens[1], tokens[2]
            entry = _ensure(out, path, current_date, current_author, current_subject)
            entry["commit_count"] += 1
            if entry["rename_from"] is None:
                entry["rename_from"] = old_path
            entry["first_seen"] = current_date
            if current_author:
                authors_by_path.setdefault(path, Counter())[current_author] += 1
            current_paths.add(path)
            out.setdefault(
                f"__alias__::{old_path}",
                {"resolves_to": path},
            )
        else:
            path = tokens[1]
            alias = out.get(f"__alias__::{path}")
            target = alias["resolves_to"] if alias else path
            entry = _ensure(out, target, current_date, current_author, current_subject)
            entry["commit_count"] += 1
            entry["first_seen"] = current_date
            if current_author:
                authors_by_path.setdefault(target, Counter())[current_author] += 1
            current_paths.add(target)

    _flush_co()

    for path, entry in out.items():
        if path.startswith("__alias__::"):
            continue
        author_counter = authors_by_path.get(path)
        entry["top_author"] = (
            author_counter.most_common(1)[0][0] if author_counter else None
        )
        co_counter = co_by_path.get(path)
        entry["co_changed"] = (
            [list(pair) for pair in co_counter.most_common(5)]
            if co_counter
            else []
        )

    return {k: v for k, v in out.items() if not k.startswith("__alias__::")}


def _ensure(
    out: dict[str, dict],
    path: str,
    date: str,
    author: str | None,
    subject: str | None = None,
) -> dict:
    entry = out.get(path)
    if entry is None:
        entry = {
            "last_modified": date,
            "last_author": author or "",
            "last_subject": subject,
            "first_seen": date,
            "commit_count": 0,
            "rename_from": None,
            "top_author": None,
            "co_changed": [],
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
