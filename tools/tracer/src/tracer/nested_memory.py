"""Nested Claude.md loading on file-read context gain.

Mirrors Claude Code's auto-load behavior. When the agent reads a file via
the tracer Read tool, the harness's native Claude.md walk-up does not fire
(tracer's read happens via subprocess, bypassing the Read tool). This
module replicates that walk-up so tracer reads carry the same Claude.md
context the native Read would have surfaced.

Triggered by `trace read <file>` only. Matches Claude Code's trigger shape:
file-read context gain → walk ancestors for instruction files → load any
applicable rules.

Limits enforced:
- @include recursion capped at MAX_INCLUDE_DEPTH (5)
- pass + session dedupe via normalized resolved paths (cycle protection)
- visited-directory tracking inside .claude/rules/ recursion (cycle protection)
- nested traversal bounded to file's dir up to repo_root (no whole-repo scan)
- missing / empty / unreadable / excluded / non-matching files skipped silently
- external @include paths (outside repo_root) blocked for nested loads
- session-level non-evicting dedupe via state file (if TRACER_SESSION_ID set)
- 40,000-char large-memory warning (warning attribute only, no hard cap)
"""

from __future__ import annotations

import fcntl
import os
import re
import tempfile
from dataclasses import dataclass
from fnmatch import fnmatch
from pathlib import Path

MAX_INCLUDE_DEPTH = 5
LARGE_MEMORY_THRESHOLD = 40_000


@dataclass
class LoadedMemory:
    path: str
    relative_path: str
    kind: str  # "claude_md" | "local_md" | "rules_unconditional" | "rules_conditional" | "include"
    content: str
    size: int
    large: bool


def load_for_file(
    file_path: Path,
    repo_root: Path,
    session_dedupe: set[str] | None = None,
    directory_mode: bool = False,
) -> list[LoadedMemory]:
    """Load all nested Claude.md context relevant to file_path.

    Returns instruction files ordered root → target, in the order Claude Code
    would load them: ancestor CLAUDE.md, ancestor .claude/CLAUDE.md, ancestor
    CLAUDE.local.md, then ancestor .claude/rules/ contents (unconditional and
    path-matching conditional). User-global ~/.claude/rules/ conditional
    matches are also surfaced.

    When `directory_mode=True`, file_path is treated as a directory and
    path-conditional rules (those with `paths:` frontmatter) are skipped —
    there's no specific file to match against. Used by Glob/Grep enrichment
    where the target is a directory or pattern, not a concrete file.
    """
    try:
        file_path = file_path.resolve()
        repo_root = repo_root.resolve()
    except OSError:
        return []

    # Allowed paths: file must live inside repo_root
    try:
        file_path.relative_to(repo_root)
    except ValueError:
        return []

    target_dir = file_path.parent if file_path.is_file() else file_path

    # Build nested_directories: from repo_root down to target_dir, inclusive
    chain: list[Path] = []
    current = target_dir
    while True:
        try:
            current.relative_to(repo_root)
        except ValueError:
            break
        chain.append(current)
        if current == repo_root or current.parent == current:
            break
        current = current.parent
    chain.reverse()

    pass_dedupe: set[str] = set()
    if session_dedupe is None:
        session_dedupe = set()

    results: list[LoadedMemory] = []

    for directory in chain:
        # Per-directory instruction files (Claude.md, .claude/CLAUDE.md, CLAUDE.local.md)
        candidates = [
            (directory / "CLAUDE.md", "claude_md"),
            (directory / "Claude.md", "claude_md"),
            (directory / ".claude" / "CLAUDE.md", "claude_md"),
            (directory / ".claude" / "Claude.md", "claude_md"),
            (directory / "CLAUDE.local.md", "local_md"),
            (directory / "Claude.local.md", "local_md"),
        ]
        for candidate, kind in candidates:
            mem = _try_load(candidate, repo_root, kind, pass_dedupe, session_dedupe)
            if mem:
                results.append(mem)
                results.extend(_load_includes(
                    Path(mem.path), mem.content, repo_root,
                    pass_dedupe, session_dedupe, depth=0,
                ))

        # .claude/rules/ recursive scan (project).
        # In directory_mode, only unconditional rules load (no concrete file to
        # match path-conditional `paths:` frontmatter against).
        rules_dir = directory / ".claude" / "rules"
        if rules_dir.is_dir():
            for rule_mem in _scan_rules_dir(rules_dir, repo_root, file_path,
                                            pass_dedupe, session_dedupe,
                                            unconditional_only=directory_mode):
                results.append(rule_mem)
                results.extend(_load_includes(
                    Path(rule_mem.path), rule_mem.content, repo_root,
                    pass_dedupe, session_dedupe, depth=0,
                ))

    # User-global ~/.claude/rules/ conditional matches.
    # In directory_mode, skip — same reasoning as above.
    user_rules = Path.home() / ".claude" / "rules"
    if user_rules.is_dir() and not directory_mode:
        for rule_mem in _scan_rules_dir(user_rules, repo_root, file_path,
                                        pass_dedupe, session_dedupe,
                                        conditional_only=True):
            results.append(rule_mem)
            results.extend(_load_includes(
                Path(rule_mem.path), rule_mem.content, repo_root,
                pass_dedupe, session_dedupe, depth=0,
            ))

    return results


def _try_load(
    path: Path,
    repo_root: Path,
    kind: str,
    pass_dedupe: set[str],
    session_dedupe: set[str],
) -> LoadedMemory | None:
    """Load one file. Returns None on missing/empty/unreadable/already-loaded."""
    try:
        resolved = path.resolve()
    except OSError:
        return None
    if not resolved.is_file():
        return None

    normalized = str(resolved)
    if normalized in pass_dedupe or normalized in session_dedupe:
        return None

    try:
        content = resolved.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return None

    if not content.strip():
        return None

    pass_dedupe.add(normalized)
    session_dedupe.add(normalized)

    try:
        relative = str(resolved.relative_to(repo_root))
    except ValueError:
        # User-global rules live outside repo_root
        try:
            relative = "~/" + str(resolved.relative_to(Path.home()))
        except ValueError:
            relative = normalized

    size = len(content)
    return LoadedMemory(
        path=normalized,
        relative_path=relative,
        kind=kind,
        content=content,
        size=size,
        large=size >= LARGE_MEMORY_THRESHOLD,
    )


def _scan_rules_dir(
    rules_dir: Path,
    repo_root: Path,
    file_path: Path,
    pass_dedupe: set[str],
    session_dedupe: set[str],
    conditional_only: bool = False,
    unconditional_only: bool = False,
) -> list[LoadedMemory]:
    """Recursively scan a .claude/rules/ directory.

    Loads .md files. Unconditional rules always load (unless conditional_only).
    Conditional rules (frontmatter `paths:` glob) only load when file_path
    matches one of the globs (skipped entirely if unconditional_only=True).
    Visited dirs + symlink targets tracked.
    """
    results: list[LoadedMemory] = []
    visited_dirs: set[str] = set()

    def _walk(d: Path) -> None:
        try:
            d_real = str(d.resolve())
        except OSError:
            return
        if d_real in visited_dirs:
            return
        visited_dirs.add(d_real)

        try:
            entries = sorted(d.iterdir())
        except OSError:
            return

        for entry in entries:
            try:
                if entry.is_symlink():
                    target_real = str(entry.resolve())
                    if target_real in visited_dirs:
                        continue
                if entry.is_dir():
                    _walk(entry)
                elif entry.is_file() and entry.suffix == ".md":
                    try:
                        preview = entry.read_text(encoding="utf-8", errors="replace")
                    except OSError:
                        continue
                    paths_globs = _extract_paths_frontmatter(preview)
                    if paths_globs is not None:
                        if unconditional_only:
                            continue
                        kind = "rules_conditional"
                        if not _matches_paths(file_path, paths_globs, repo_root):
                            continue
                    else:
                        if conditional_only:
                            continue
                        kind = "rules_unconditional"
                    mem = _try_load(entry, repo_root, kind, pass_dedupe, session_dedupe)
                    if mem:
                        results.append(mem)
            except OSError:
                continue

    _walk(rules_dir)
    return results


_FRONTMATTER_PATHS_RE = re.compile(r"^paths\s*:\s*(.*)$")


def _extract_paths_frontmatter(content: str) -> list[str] | None:
    """Extract `paths:` glob list from YAML frontmatter. Returns None if no frontmatter.

    Supports:
        paths: "**/*.ts"
        paths:
          - "**/*.ts"
          - "**/*.tsx"
    """
    if not content.startswith("---"):
        return None
    lines = content.splitlines()
    if len(lines) < 2:
        return None
    end = None
    for i in range(1, min(len(lines), 100)):
        if lines[i].rstrip() == "---":
            end = i
            break
    if end is None:
        return None

    paths: list[str] = []
    capturing_list = False
    for ln in lines[1:end]:
        stripped = ln.rstrip()
        if not stripped:
            if capturing_list:
                break
            continue
        m = _FRONTMATTER_PATHS_RE.match(stripped.lstrip())
        if m:
            rest = m.group(1).strip()
            if rest:
                return [rest.strip("\"'")]
            capturing_list = True
            continue
        if capturing_list:
            ls = ln.lstrip()
            if ls.startswith("- "):
                paths.append(ls[2:].strip().strip("\"'"))
            elif ls.startswith("#"):
                continue
            else:
                break

    return paths if paths else None


def _matches_paths(file_path: Path, paths_globs: list[str], repo_root: Path) -> bool:
    """Glob-match file_path against any of paths_globs."""
    try:
        relative = str(file_path.relative_to(repo_root))
    except ValueError:
        relative = str(file_path)
    name = file_path.name
    for glob in paths_globs:
        if fnmatch(relative, glob):
            return True
        # **/ prefix: also try with prefix stripped (fnmatch doesn't support **)
        if glob.startswith("**/") and fnmatch(relative, glob[3:]):
            return True
        if fnmatch(name, glob):
            return True
        # Any-depth match: if glob has no slash, match basename anywhere
        if "/" not in glob and fnmatch(name, glob):
            return True
    return False


_INCLUDE_RE = re.compile(r"@include\s+(\S+)")
_INLINE_CODE_RE = re.compile(r"`[^`]*`")


def _load_includes(
    including_file: Path,
    content: str,
    repo_root: Path,
    pass_dedupe: set[str],
    session_dedupe: set[str],
    depth: int,
) -> list[LoadedMemory]:
    """Parse @include refs from markdown text only (not code blocks)."""
    if depth >= MAX_INCLUDE_DEPTH:
        return []

    results: list[LoadedMemory] = []
    in_fence = False
    for ln in content.splitlines():
        ls = ln.lstrip()
        if ls.startswith("```") or ls.startswith("~~~"):
            in_fence = not in_fence
            continue
        if in_fence:
            continue
        cleaned = _INLINE_CODE_RE.sub("", ln)
        for match in _INCLUDE_RE.finditer(cleaned):
            ref = match.group(1)
            try:
                included = (including_file.parent / ref).resolve()
            except OSError:
                continue
            # Block external includes for nested loads
            try:
                included.relative_to(repo_root)
            except ValueError:
                continue
            mem = _try_load(included, repo_root, "include", pass_dedupe, session_dedupe)
            if mem:
                results.append(mem)
                results.extend(_load_includes(
                    included, mem.content, repo_root,
                    pass_dedupe, session_dedupe, depth + 1,
                ))
    return results


def _session_id() -> str | None:
    """Resolve session id from the env vars Claude Code actually exports.

    Priority: CLAUDE_CODE_SESSION_ID (the real var Claude Code injects into
    Bash subprocesses in internal builds), then CLAUDE_SESSION_ID (forward-
    compat alias), then TRACER_SESSION_ID (manual override for non-Claude-Code
    callers, e.g. tests).
    """
    return (
        os.environ.get("CLAUDE_CODE_SESSION_ID")
        or os.environ.get("CLAUDE_SESSION_ID")
        or os.environ.get("TRACER_SESSION_ID")
    )


def load_session_dedupe() -> set[str]:
    """Load session-level dedupe set from disk."""
    session_id = _session_id()
    if not session_id:
        return set()
    state_file = _session_state_path(session_id)
    if not state_file.is_file():
        return set()
    try:
        return set(line.strip() for line in state_file.read_text().splitlines() if line.strip())
    except OSError:
        return set()


def save_session_dedupe(dedupe: set[str]) -> None:
    """Persist session dedupe set to disk under an exclusive lock.

    Concurrent `trace read` invocations in the same session would race the
    state file (read-modify-write). The lock + merge-before-write guarantees
    every concurrent caller's additions survive: hold flock on the parent
    directory, re-read current state, union with our set, atomic-rename a
    temp file into place. Last writer wins on the file but no caller's
    additions are lost.
    """
    session_id = _session_id()
    if not session_id:
        return
    state_file = _session_state_path(session_id)
    try:
        state_file.parent.mkdir(parents=True, exist_ok=True)
    except OSError:
        return

    lock_file = state_file.parent / ".lock"
    try:
        with open(lock_file, "w") as lock_fh:
            try:
                fcntl.flock(lock_fh, fcntl.LOCK_EX)
            except OSError:
                pass
            try:
                existing: set[str] = set()
                if state_file.is_file():
                    try:
                        existing = set(
                            line.strip()
                            for line in state_file.read_text().splitlines()
                            if line.strip()
                        )
                    except OSError:
                        pass
                merged = existing | dedupe
                fd, tmp_path = tempfile.mkstemp(
                    prefix=".loaded-memories.", dir=state_file.parent
                )
                try:
                    with os.fdopen(fd, "w") as tmp_fh:
                        tmp_fh.write("\n".join(sorted(merged)))
                    os.replace(tmp_path, state_file)
                except OSError:
                    try:
                        os.unlink(tmp_path)
                    except OSError:
                        pass
            finally:
                try:
                    fcntl.flock(lock_fh, fcntl.LOCK_UN)
                except OSError:
                    pass
    except OSError:
        pass


def _session_state_path(session_id: str) -> Path:
    return Path.home() / ".tracer-cache" / "sessions" / session_id / "loaded-memories.txt"


def render(memories: list[LoadedMemory]) -> str:
    """Render loaded memories as a header block for trace read output."""
    if not memories:
        return ""
    blocks: list[str] = []
    for mem in memories:
        marker = " [LARGE]" if mem.large else ""
        blocks.append(f"=== {mem.relative_path} · {mem.kind}{marker} ({mem.size} chars) ===")
        blocks.append(mem.content.rstrip())
        blocks.append("")
    return "\n".join(blocks).rstrip()
