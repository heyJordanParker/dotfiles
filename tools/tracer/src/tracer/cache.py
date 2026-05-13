"""Two-layer disk cache for tracer.

Two namespaces under `.tracer-cache/` at the repo root:

- `file/{hash}.json` — per-file facts (complexity, loc, language, raw imports
  list, raw exports list, git activity). One entry per file. Cache key is
  `sha256(file_contents) + relative_path_from_repo_root`. Invalidates when
  the file's contents change.

- `architecture/{hash}.json` — the cross-file architecture graph. Nodes are
  symbols and modules — never files. Edges are cross-file relationships
  (`imports`, `calls`, `references`) tagged with `confidence` (EXTRACTED |
  INFERRED | AMBIGUOUS). One entry per repo state. Cache key is the
  fingerprint of all currently-cached file hashes.

The two namespaces never read each other. Per-file commands read `file/`.
Architecture commands (`callers`, `defines`, `symbols`, `deps`, `dependents`)
read `architecture/`. Joins between the layers happen in the rendering
layer at query time only — never persisted, never cached together.

Atomic writes via `tempfile.mkstemp` + `os.replace` so concurrent invocations
or crashes mid-write don't corrupt cache entries (graphify's `cache.py`
pattern; see `cache.py:108-145` of `~/Developer/references/graphify`).
"""

from __future__ import annotations

import hashlib
import json
import os
import shutil
import subprocess
import tempfile
from dataclasses import dataclass
from pathlib import Path

CACHE_DIR_NAME = ".tracer-cache"

# Cache namespaces — full words per project naming convention.
NAMESPACE_FILE = "file"
NAMESPACE_ARCHITECTURE = "architecture"

# Schema version baked into every cache key. Bump when the extraction logic,
# FileFacts shape, or architecture graph schema changes — old entries become
# unreachable, invalidating the cache without needing a manual `cache clear`.
SCHEMA_VERSION = 6


def repo_root_for(path: str | Path = ".") -> Path:
    """Resolve the git repository root containing `path`.

    Falls back to `path` itself (or its parent if it's a file) when not
    inside a git repository.
    """
    p = Path(path).resolve()
    cwd = p if p.is_dir() else p.parent
    try:
        result = subprocess.run(
            ["git", "rev-parse", "--show-toplevel"],
            cwd=cwd,
            capture_output=True,
            text=True,
            check=True,
            timeout=5,
        )
        return Path(result.stdout.strip())
    except (subprocess.CalledProcessError, FileNotFoundError, subprocess.TimeoutExpired):
        return cwd


def cache_root(repo_root: Path | None = None) -> Path:
    """Return `.tracer-cache/` at the repo root. Creates it if missing."""
    root = repo_root or repo_root_for(".")
    directory = root / CACHE_DIR_NAME
    directory.mkdir(parents=True, exist_ok=True)
    return directory


def namespace_dir(namespace: str, repo_root: Path | None = None) -> Path:
    """Return `.tracer-cache/{namespace}/`. Creates it if missing."""
    directory = cache_root(repo_root) / namespace
    directory.mkdir(parents=True, exist_ok=True)
    return directory


def file_hash(path: Path, repo_root: Path | None = None) -> str:
    """SHA-256 of file contents + relative path + schema version.

    Including the relative path keeps cache entries portable across machines
    and prevents two files with identical contents but different paths from
    sharing one cache entry. The schema version invalidates the cache when
    extraction or FileFacts shape changes — bump SCHEMA_VERSION at the
    module top to roll over the cache.
    """
    p = Path(path)
    if not p.is_file():
        raise IsADirectoryError(f"file_hash requires a file, got: {p}")
    root = repo_root or repo_root_for(p)
    digest = hashlib.sha256()
    digest.update(f"v{SCHEMA_VERSION}\x00".encode())
    digest.update(p.read_bytes())
    digest.update(b"\x00")
    try:
        relative = p.resolve().relative_to(root.resolve())
        digest.update(str(relative).encode())
    except ValueError:
        digest.update(str(p.resolve()).encode())
    return digest.hexdigest()


def architecture_fingerprint(file_hashes: dict[str, str]) -> str:
    """Stable fingerprint of the per-file cache state.

    Used as the architecture cache key. Any per-file change produces a new
    fingerprint, causing the architecture cache to rebuild from current
    per-file facts. Sorted by relative path so insertion order doesn't
    affect the result.
    """
    digest = hashlib.sha256()
    for relative_path in sorted(file_hashes):
        digest.update(relative_path.encode())
        digest.update(b"\x00")
        digest.update(file_hashes[relative_path].encode())
        digest.update(b"\n")
    return digest.hexdigest()


def load(namespace: str, key: str, repo_root: Path | None = None) -> dict | None:
    """Load a cache entry. Returns None when missing or corrupt."""
    entry = namespace_dir(namespace, repo_root) / f"{key}.json"
    if not entry.exists():
        return None
    try:
        return json.loads(entry.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError):
        return None


def save(namespace: str, key: str, value: dict, repo_root: Path | None = None) -> None:
    """Atomically save a cache entry.

    Writes to a temp file in the same directory then `os.replace`s into
    place — safe under concurrent writers and crashes.
    """
    target_dir = namespace_dir(namespace, repo_root)
    entry = target_dir / f"{key}.json"
    fd, tmp_path = tempfile.mkstemp(dir=target_dir, prefix=f"{key}.", suffix=".tmp")
    try:
        os.write(fd, json.dumps(value, default=str).encode())
        os.close(fd)
        os.replace(tmp_path, entry)
    except Exception:
        try:
            os.close(fd)
        except OSError:
            pass
        try:
            os.unlink(tmp_path)
        except OSError:
            pass
        raise


@dataclass
class CacheStats:
    namespace: str
    entry_count: int
    total_bytes: int


def stats(repo_root: Path | None = None) -> list[CacheStats]:
    """Return per-namespace counts and on-disk sizes."""
    root = cache_root(repo_root)
    out: list[CacheStats] = []
    for namespace in (NAMESPACE_FILE, NAMESPACE_ARCHITECTURE):
        directory = root / namespace
        if not directory.is_dir():
            out.append(CacheStats(namespace, 0, 0))
            continue
        entries = list(directory.glob("*.json"))
        size = sum(entry.stat().st_size for entry in entries)
        out.append(CacheStats(namespace, len(entries), size))
    return out


def clear(namespace: str | None = None, repo_root: Path | None = None) -> int:
    """Delete cache entries. Returns number of entries removed.

    `namespace=None` clears both namespaces.
    """
    root = cache_root(repo_root)
    removed = 0
    namespaces = (
        (namespace,) if namespace else (NAMESPACE_FILE, NAMESPACE_ARCHITECTURE)
    )
    for ns in namespaces:
        directory = root / ns
        if not directory.is_dir():
            continue
        for entry in directory.glob("*.json"):
            entry.unlink()
            removed += 1
    return removed


def clear_all(repo_root: Path | None = None) -> int:
    """Nuke the entire `.tracer-cache/` directory."""
    root = cache_root(repo_root)
    if not root.is_dir():
        return 0
    file_count = sum(1 for _ in root.rglob("*.json"))
    shutil.rmtree(root)
    return file_count
