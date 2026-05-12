"""Per-file enrichment for `grep` / `struct` matches.

Reads from the per-file cache via `file_facts.get`. Never reads from the
architecture cache — joins between the two layers happen at rendering time
in the commands themselves, not here.
"""

from __future__ import annotations

from pathlib import Path

from tracer import file_facts

DOC_FILENAMES = {
    "Claude.md",
    "CLAUDE.md",
    "Readme.md",
    "README.md",
    "ARCHITECTURE.md",
    "architecture.md",
}


def file_complexity(path: str | Path) -> dict[str, int | str]:
    """Per-file complexity from the cache (extracts and caches on miss)."""
    facts = file_facts.get(path)
    if facts is None:
        return {"ccn_total": 0, "ccn_max_function": 0, "loc": 0, "rank": "unknown"}
    return {
        "ccn_total": facts.cyclomatic_complexity_total,
        "ccn_max_function": facts.cyclomatic_complexity_max,
        "loc": facts.loc,
        "rank": facts.rank,
    }


def nearest_doc(path: str | Path) -> str | None:
    """Walk up from the file to find the nearest project documentation file."""
    p = Path(path).resolve()
    current = p.parent if p.is_file() else p
    root = Path("/")
    while current != root:
        for name in DOC_FILENAMES:
            candidate = current / name
            if candidate.exists():
                return str(candidate)
        current = current.parent
    return None


def git_context(path: str | Path) -> dict[str, str | int | None]:
    """Last commit, author, and 30-day commit count from the cache."""
    facts = file_facts.get(path)
    if facts is None:
        return {"last_modified": None, "last_author": None, "commits_30d": 0}
    return {
        "last_modified": facts.last_modified,
        "last_author": facts.last_author,
        "commits_30d": facts.commits_30d,
    }
