"""Repo-wide context attached to every command's output.

The agent uses `complexity_p95` to calibrate read depth — files exceeding
p95 get full reads, uniformly low complexity gets skims. Computed once via
`scc` over the repo root and cached to disk under `.tracer-cache/file/`
keyed by the current `git rev-parse HEAD`. Working-tree changes are
ignored — the baseline shifts only on commit, which matches how complexity
distributions actually move.
"""

from __future__ import annotations

import json
import statistics
import subprocess
from pathlib import Path

from tracer import cache


_CACHE_KEY_PREFIX = "repo_context_"


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


def _compute(repo_root: Path) -> dict[str, int | float]:
    try:
        result = subprocess.run(
            ["scc", "--format", "json", "--by-file", str(repo_root)],
            capture_output=True,
            text=True,
            check=True,
            timeout=60,
        )
    except (subprocess.CalledProcessError, FileNotFoundError, subprocess.TimeoutExpired):
        return {"total_files": 0, "median_file_ccn": 0, "complexity_p95": 0}

    try:
        data = json.loads(result.stdout)
    except json.JSONDecodeError:
        return {"total_files": 0, "median_file_ccn": 0, "complexity_p95": 0}

    complexities: list[int] = []
    for lang_block in data:
        for file_entry in lang_block.get("Files", []):
            complexities.append(file_entry.get("Complexity", 0))

    if not complexities:
        return {"total_files": 0, "median_file_ccn": 0, "complexity_p95": 0}

    sorted_c = sorted(complexities)
    p95_idx = max(0, int(len(sorted_c) * 0.95) - 1)
    return {
        "total_files": len(complexities),
        "median_file_ccn": int(statistics.median(complexities)),
        "complexity_p95": sorted_c[p95_idx],
    }


def repo_context(path: str = ".") -> dict[str, int | float]:
    """Return repo-wide stats. Disk-cached by HEAD SHA; freshly computed
    when no git or no HEAD."""
    root = cache.repo_root_for(Path(path).resolve())
    head = _git_head(root)
    if head is None:
        return _compute(root)

    key = f"{_CACHE_KEY_PREFIX}{head}"
    cached = cache.load(cache.NAMESPACE_FILE, key, root)
    if cached is not None:
        return cached

    stats = _compute(root)
    cache.save(cache.NAMESPACE_FILE, key, stats, root)
    return stats
