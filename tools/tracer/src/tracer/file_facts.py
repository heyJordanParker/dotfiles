"""Per-file facts layer.

`get(path)` returns the cached FileFacts for a file or extracts and caches
fresh if the cache is stale. Layers below this never call lizard or
tree-sitter directly — they go through `get` so cache reads are honored.

This module owns the `file/` cache namespace. It does not read or write the
`architecture/` namespace. The architecture layer reads FileFacts from here
to build its graph; nothing goes the other way.
"""

from __future__ import annotations

import os
from concurrent.futures import ThreadPoolExecutor, as_completed
from dataclasses import asdict, dataclass, field
from pathlib import Path

import lizard

from tracer import cache, git_activity
from tracer.extraction import ExtractionResult, extract, supported_extensions


# Worker count for parallel cold-cache extraction. Tree-sitter and lizard
# release the GIL during C-level parsing so threads scale on multi-core.
_PARALLEL_WORKERS = min(8, (os.cpu_count() or 4))


@dataclass
class FileFacts:
    """Everything we know about a single file at the per-file layer.

    No cross-file relationships. No graph membership. Pure file-local data.
    Joins with the architecture layer happen at query rendering time only.
    """

    path: str
    language: str | None
    loc: int
    function_count: int
    cyclomatic_complexity_total: int
    cyclomatic_complexity_max: int
    rank: str
    extraction: ExtractionResult | None
    last_modified: str | None
    last_author: str | None
    commits_30d: int
    # Lifecycle signals — let the agent reason about how settled the code is.
    first_seen: str | None = None
    commit_count: int = 0
    rename_from: str | None = None
    working_state: str | None = None
    # Deploy-branch presence. Tuple of labels: "prod", "staging", "main".
    # Empty = file does not exist on any tracked deploy branch.
    present_in: tuple[str, ...] = ()
    # Subject of the newest commit touching this file. Surfaced in the
    # shoulder so the agent doesn't need a follow-up `git log -1 --format=%s`.
    last_subject: str | None = None
    # Author with the most commits to this file over full history. May
    # differ from last_author (most recent committer).
    top_author: str | None = None
    # Top files that historically change together with this one, ranked by
    # commit co-occurrence. Capped at 5.
    co_changed: tuple[tuple[str, int], ...] = ()
    # mtime + size for the fast-path. If the file's current mtime + size
    # match the cached entry, skip the SHA computation entirely on warm
    # validation. mtime + size collisions on a real edit are vanishingly
    # rare; the SHA still gates the actual cache entry name (correctness).
    mtime_ns: int = 0
    size_bytes: int = 0

    def to_dict(self) -> dict:
        return {
            **{k: v for k, v in asdict(self).items() if k != "extraction"},
            "extraction": self.extraction.to_dict() if self.extraction else None,
        }

    @classmethod
    def from_dict(cls, data: dict) -> "FileFacts":
        extraction_data = data.get("extraction")
        extraction = (
            ExtractionResult.from_dict(extraction_data) if extraction_data else None
        )
        return cls(
            path=data["path"],
            language=data.get("language"),
            loc=data.get("loc", 0),
            function_count=data.get("function_count", 0),
            cyclomatic_complexity_total=data.get("cyclomatic_complexity_total", 0),
            cyclomatic_complexity_max=data.get("cyclomatic_complexity_max", 0),
            rank=data.get("rank", "unknown"),
            extraction=extraction,
            last_modified=data.get("last_modified"),
            last_author=data.get("last_author"),
            commits_30d=data.get("commits_30d", 0),
            first_seen=data.get("first_seen"),
            commit_count=data.get("commit_count", 0),
            rename_from=data.get("rename_from"),
            working_state=data.get("working_state"),
            present_in=tuple(data.get("present_in") or ()),
            last_subject=data.get("last_subject"),
            top_author=data.get("top_author"),
            co_changed=tuple(
                (p, c) for p, c in (data.get("co_changed") or ())
            ),
            mtime_ns=data.get("mtime_ns", 0),
            size_bytes=data.get("size_bytes", 0),
        )


def _rank(complexity: int) -> str:
    if complexity < 10:
        return "low"
    if complexity < 30:
        return "medium"
    if complexity < 80:
        return "high"
    return "critical"


def _extract_facts(
    path: Path,
    repo_root: Path,
    git: git_activity.GitActivity = git_activity.empty(),
) -> FileFacts:
    """Fresh extraction — called on cache miss.

    `git` is precomputed in bulk by the caller for cold-cache builds; for
    single-file paths it falls back to the empty default (the bulk path is
    still O(commits) but a single-file caller pays no extra cost — the
    field just shows None until next bulk refresh).
    """
    relative = str(path.resolve().relative_to(repo_root.resolve()))

    try:
        parsed = lizard.analyze_file(str(path))
        ccn_total = sum(f.cyclomatic_complexity for f in parsed.function_list)
        ccn_max = max(
            (f.cyclomatic_complexity for f in parsed.function_list), default=0
        )
        loc = parsed.nloc
        function_count = len(parsed.function_list)
    except Exception:
        ccn_total = 0
        ccn_max = 0
        loc = 0
        function_count = 0

    extraction: ExtractionResult | None = None
    if path.suffix.lower() in supported_extensions():
        try:
            source = path.read_bytes()
            extraction = extract(source, str(path))
        except Exception:
            extraction = None

    try:
        stat = path.stat()
        mtime_ns = stat.st_mtime_ns
        size_bytes = stat.st_size
    except OSError:
        mtime_ns = 0
        size_bytes = 0

    return FileFacts(
        path=relative,
        language=extraction.language if extraction else None,
        loc=loc,
        function_count=function_count,
        cyclomatic_complexity_total=ccn_total,
        cyclomatic_complexity_max=ccn_max,
        rank=_rank(ccn_total),
        extraction=extraction,
        last_modified=git.last_modified,
        last_author=git.last_author,
        commits_30d=git.commits_30d,
        first_seen=git.first_seen,
        commit_count=git.commit_count,
        rename_from=git.rename_from,
        working_state=git.working_state,
        present_in=git.present_in,
        last_subject=git.last_subject,
        top_author=git.top_author,
        co_changed=git.co_changed,
        mtime_ns=mtime_ns,
        size_bytes=size_bytes,
    )


def get(
    path: str | Path,
    repo_root: Path | None = None,
    git: git_activity.GitActivity | None = None,
) -> FileFacts | None:
    """Return cached FileFacts for `path`, extracting and caching on miss.

    Cache resolution order (fastest first):
      1. mtime + size match against the cached fast-path index → return
         cached without reading file bytes.
      2. Full SHA hash → cache entry exists → return.
      3. Fresh extraction.

    Step 1 turns warm-cache validation from "read file bytes + SHA" into
    "stat()" — microseconds vs milliseconds per file.
    """
    p = Path(path).resolve()
    if not p.is_file():
        return None
    root = repo_root or cache.repo_root_for(p)

    # Fast path: stat-only validation against the mtime index
    try:
        stat = p.stat()
    except OSError:
        return None

    fast = _mtime_index_lookup(root, p, stat, root)
    if fast is not None:
        return _with_working_state(fast, p, root)

    try:
        key = cache.file_hash(p, root)
    except (OSError, IsADirectoryError):
        return None

    cached = cache.load(cache.NAMESPACE_FILE, key, root)
    if cached is not None:
        try:
            facts = FileFacts.from_dict(cached)
            _mtime_index_record(root, p, stat, root, key)
            return _with_working_state(facts, p, root)
        except (KeyError, TypeError):
            pass

    if git is None:
        try:
            relative = str(p.relative_to(root.resolve()))
        except ValueError:
            relative = str(p)
        git = git_activity.bulk_cached(root).get(relative, git_activity.empty())

    facts = _extract_facts(p, root, git)
    cache.save(cache.NAMESPACE_FILE, key, facts.to_dict(), root)
    _mtime_index_record(root, p, stat, root, key)
    return facts


def _with_working_state(facts: FileFacts, path: Path, repo_root: Path) -> FileFacts:
    """Overlay live working-tree state onto a cached FileFacts.

    Working-tree state (untracked, staged add, modified) drifts between cache
    writes — a file can become staged then committed while its content hash
    is unchanged. We treat it as live data, not cached: one cheap git status
    call per repo root, joined onto the cached FileFacts at render time.
    """
    state_map = _working_state_for(repo_root)
    if not state_map:
        return facts
    try:
        relative = str(path.relative_to(repo_root.resolve()))
    except ValueError:
        return facts
    state = state_map.get(relative)
    if state == facts.working_state:
        return facts
    facts.working_state = state
    return facts


_WORKING_STATE_CACHE: dict[str, dict[str, str]] = {}


def _working_state_for(repo_root: Path) -> dict[str, str]:
    """Per-process memo of `git status` so repeated reads in one invocation
    pay the cost once. A `trace` invocation is short-lived, so process-level
    is fine — no staleness window worth handling."""
    key = str(repo_root.resolve())
    if key not in _WORKING_STATE_CACHE:
        _WORKING_STATE_CACHE[key] = git_activity._working_tree_state(repo_root)
    return _WORKING_STATE_CACHE[key]


_MTIME_INDEX_KEY = "mtime_index_v1"


def _mtime_index_lookup(
    repo_root: Path, path: Path, stat: "os.stat_result", root_for_relative: Path
) -> "FileFacts | None":
    """Read the per-repo mtime index; return cached FileFacts if mtime matches."""
    index = cache.load(cache.NAMESPACE_FILE, _MTIME_INDEX_KEY, repo_root)
    if not index:
        return None
    try:
        relative = str(path.relative_to(root_for_relative.resolve()))
    except ValueError:
        return None
    entry = index.get(relative)
    if entry is None:
        return None
    if entry.get("mtime_ns") != stat.st_mtime_ns or entry.get("size") != stat.st_size:
        return None
    cached = cache.load(cache.NAMESPACE_FILE, entry["key"], repo_root)
    if cached is None:
        return None
    try:
        return FileFacts.from_dict(cached)
    except (KeyError, TypeError):
        return None


def _mtime_index_record(
    repo_root: Path, path: Path, stat: "os.stat_result", root_for_relative: Path, key: str
) -> None:
    """Append/update the mtime index entry for one file."""
    try:
        relative = str(path.relative_to(root_for_relative.resolve()))
    except ValueError:
        return
    index = cache.load(cache.NAMESPACE_FILE, _MTIME_INDEX_KEY, repo_root) or {}
    index[relative] = {
        "mtime_ns": stat.st_mtime_ns,
        "size": stat.st_size,
        "key": key,
    }
    cache.save(cache.NAMESPACE_FILE, _MTIME_INDEX_KEY, index, repo_root)


def get_many(paths: list[str | Path], repo_root: Path | None = None) -> list[FileFacts]:
    """Bulk get — returns FileFacts for every readable file in `paths`.

    Computes git activity for the whole repo in 2 subprocesses (instead of
    2 per file). Then dispatches per-file extraction across a thread pool
    — both cache hits (JSON read) and cache misses (lizard + tree-sitter
    parse) are I/O- or C-extension-bound, so threads scale.
    """
    root = repo_root or cache.repo_root_for(".")
    root_resolved = root.resolve()
    git_map = git_activity.bulk(root)
    empty = git_activity.empty()

    def _one(p: str | Path) -> FileFacts | None:
        path_obj = Path(p).resolve()
        try:
            relative = str(path_obj.relative_to(root_resolved))
        except ValueError:
            relative = str(path_obj)
        return get(p, root, git=git_map.get(relative, empty))

    with ThreadPoolExecutor(max_workers=_PARALLEL_WORKERS) as pool:
        futures = [pool.submit(_one, p) for p in paths]
        results = [
            facts
            for facts in (future.result() for future in as_completed(futures))
            if facts is not None
        ]
    return results


def file_hashes_for(paths: list[Path], repo_root: Path) -> dict[str, str]:
    """Map relative_path -> file_hash for every readable file in `paths`.

    Used by the architecture layer to compute its cache fingerprint.

    Fast path: read the per-repo mtime index. For files whose (mtime, size)
    match the index, reuse the cached SHA — no file bytes read.
    Slow path: only files that don't match the index get a fresh SHA.

    On a fully warm cache this turns 750 file-byte reads into 750 stats.
    """
    root_resolved = repo_root.resolve()
    index = cache.load(cache.NAMESPACE_FILE, _MTIME_INDEX_KEY, repo_root) or {}

    fast: dict[str, str] = {}
    misses: list[tuple[Path, str]] = []

    for p in paths:
        if not p.is_file():
            continue
        try:
            relative = str(p.resolve().relative_to(root_resolved))
        except (ValueError, OSError):
            continue
        try:
            stat = p.stat()
        except OSError:
            continue
        entry = index.get(relative)
        if entry is not None and entry.get("mtime_ns") == stat.st_mtime_ns and entry.get("size") == stat.st_size:
            fast[relative] = entry["key"]
        else:
            misses.append((p, relative))

    if misses:
        def _hash(item: tuple[Path, str]) -> tuple[str, str] | None:
            p, relative = item
            try:
                return (relative, cache.file_hash(p, repo_root))
            except (OSError, IsADirectoryError):
                return None

        with ThreadPoolExecutor(max_workers=_PARALLEL_WORKERS) as pool:
            for entry in pool.map(_hash, misses):
                if entry is not None:
                    fast[entry[0]] = entry[1]

    return fast
