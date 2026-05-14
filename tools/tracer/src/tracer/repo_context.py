"""Repo-wide context attached to every command's output.

The agent uses `complexity_p95` to calibrate read depth — files exceeding
p95 get full reads, uniformly low complexity gets skims. Computed once via
`scc` over the repo root and cached to disk under `.tracer-cache/file/`
keyed by the current `git rev-parse HEAD`. Working-tree changes are
ignored — the baseline shifts only on commit, which matches how complexity
distributions actually move.

The same `scc` invocation populates a per-file map (`ccn`, `loc`,
`language`) consumed by `file_facts._lite_facts` so orientation commands
can hand back rich ccn data without paying per-file lizard / tree-sitter
extraction. Caching them together avoids running `scc` twice.
"""

from __future__ import annotations

import json
import statistics
import subprocess
from pathlib import Path

from tracer import cache


# Bumped from `repo_context_` to invalidate older entries that only stored
# the summary stats and missed the per_file map.
_CACHE_KEY_PREFIX = "repo_context_v2_"


def _git_head(repo_root: Path) -> str | None:
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


def _empty_payload() -> dict:
    return {
        "summary": {"total_files": 0, "median_file_ccn": 0, "complexity_p95": 0},
        "per_file": {},
    }


def _compute(repo_root: Path) -> dict:
    """Single scc invocation → summary stats + per-file (ccn, loc, language)."""
    try:
        result = subprocess.run(
            ["scc", "--format", "json", "--by-file", str(repo_root)],
            capture_output=True,
            text=True,
            check=True,
            timeout=60,
        )
    except (subprocess.CalledProcessError, FileNotFoundError, subprocess.TimeoutExpired):
        return _empty_payload()

    try:
        data = json.loads(result.stdout)
    except json.JSONDecodeError:
        return _empty_payload()

    root_resolved = repo_root.resolve()
    complexities: list[int] = []
    per_file: dict[str, dict] = {}
    for lang_block in data:
        language = lang_block.get("Name")
        for file_entry in lang_block.get("Files", []):
            ccn = file_entry.get("Complexity", 0)
            loc = file_entry.get("Code", 0)
            # scc's `Location` is absolute when the input path is absolute
            # and relative when relative. `Filename` is always the basename
            # (collapses files with the same name across the tree, so it's
            # unusable as a key). Normalize Location to a repo-root-relative
            # string so callers can index by `path.relative_to(repo_root)`.
            location = file_entry.get("Location") or ""
            if not location:
                continue
            complexities.append(ccn)
            try:
                rel = str(Path(location).relative_to(root_resolved))
            except ValueError:
                rel = location  # already relative; use as-is
            per_file[rel] = {"ccn": ccn, "loc": loc, "language": language}

    if not complexities:
        return _empty_payload()

    sorted_c = sorted(complexities)
    p95_idx = max(0, int(len(sorted_c) * 0.95) - 1)
    return {
        "summary": {
            "total_files": len(complexities),
            "median_file_ccn": int(statistics.median(complexities)),
            "complexity_p95": sorted_c[p95_idx],
        },
        "per_file": per_file,
    }


# Per-process memo so the per-file scc map is loaded once per invocation
# even when 1500 files all call into `per_file_metrics`. Same pattern as
# `file_facts._MTIME_INDEX_CACHE` — short-lived process, mirror writes via
# `_load_or_compute` which also persists to disk.
_PAYLOAD_CACHE: dict[str, dict] = {}


def _load_or_compute(repo_root: Path) -> dict:
    memo_key = str(repo_root.resolve())
    memoed = _PAYLOAD_CACHE.get(memo_key)
    if memoed is not None:
        return memoed

    head = _git_head(repo_root)
    if head is None:
        payload = _compute(repo_root)
        _PAYLOAD_CACHE[memo_key] = payload
        return payload

    key = f"{_CACHE_KEY_PREFIX}{head}"
    cached = cache.load(cache.NAMESPACE_FILE, key, repo_root)
    if cached is not None and "summary" in cached and "per_file" in cached:
        _PAYLOAD_CACHE[memo_key] = cached
        return cached
    payload = _compute(repo_root)
    cache.save(cache.NAMESPACE_FILE, key, payload, repo_root)
    _PAYLOAD_CACHE[memo_key] = payload
    return payload


def repo_context(path: str = ".") -> dict[str, int | float]:
    """Return repo-wide summary stats. Same shape as before: total_files,
    median_file_ccn, complexity_p95."""
    root = cache.repo_root_for(Path(path).resolve())
    return _load_or_compute(root)["summary"]


def per_file_metrics(repo_root: Path) -> dict[str, dict]:
    """Per-file `{ccn, loc, language}` map keyed by repo-root-relative path.
    Same scc invocation as `repo_context()` — both share one disk cache
    entry per HEAD SHA."""
    return _load_or_compute(repo_root)["per_file"]
