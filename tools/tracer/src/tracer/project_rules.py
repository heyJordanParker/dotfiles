"""Discover and summarize project rules from Claude.md ancestors.

For any file, walk up the directory tree finding `Claude.md` (and `CLAUDE.md`)
files. Extract their Requirements and Boundaries sections — these are where
this repo encodes the rules that constrain modifications. Emit a tight inline
summary that fits in the passive shoulder.

Why this matters: agents that try to follow project rules currently have to
make a separate read for the nearest doc, then parse it. Pushing the rule
text directly into `trace read` output closes that round-trip.
"""

from __future__ import annotations

import re
from pathlib import Path


_HEADING_RE = re.compile(r"^(#{1,6})\s+(.+?)\s*$")
_RULE_HEADINGS = {"requirements", "boundaries"}


def for_file(path: Path, repo_root: Path, max_chars: int = 1200) -> list[dict]:
    """Walk up from `path` to `repo_root` collecting Requirements + Boundaries
    from every Claude.md / CLAUDE.md found along the way. Returns a list of
    `{"source": "<relative path>", "section": "...", "lines": [...]}` entries,
    sorted nearest-first and capped at `max_chars` total payload.
    """
    found: list[dict] = []
    used = 0
    target = path.resolve()
    root = repo_root.resolve()

    # Walk from file's directory up to (and including) repo_root.
    if target.is_file():
        current = target.parent
    else:
        current = target
    visited: set[Path] = set()
    while True:
        if current in visited:
            break
        visited.add(current)
        for name in ("Claude.md", "CLAUDE.md"):
            doc = current / name
            if not doc.is_file():
                continue
            for entry in _extract_rule_sections(doc):
                entry["source"] = str(doc.relative_to(root)) if doc.is_relative_to(root) else str(doc)
                payload = len("\n".join(entry["lines"]))
                if used + payload > max_chars:
                    # Truncate the last entry's lines to fit.
                    remaining = max_chars - used
                    if remaining > 80:  # Only worth including if meaningful.
                        truncated_lines: list[str] = []
                        accumulated = 0
                        for line in entry["lines"]:
                            line_size = len(line) + 1
                            if accumulated + line_size > remaining:
                                break
                            truncated_lines.append(line)
                            accumulated += line_size
                        if truncated_lines:
                            truncated_lines.append("… (truncated)")
                            entry["lines"] = truncated_lines
                            found.append(entry)
                    return found
                found.append(entry)
                used += payload
        if current == root or current.parent == current:
            break
        current = current.parent
    return found


def _extract_rule_sections(doc: Path) -> list[dict]:
    """Parse a Claude.md and pull every Requirements / Boundaries subsection.

    Returns one entry per section found. Each entry has `section` (the
    heading text) and `lines` (the bullet content under it, until the next
    same-or-higher-level heading).
    """
    try:
        text = doc.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return []

    lines = text.splitlines()
    out: list[dict] = []
    i = 0
    while i < len(lines):
        match = _HEADING_RE.match(lines[i])
        if not match:
            i += 1
            continue
        level = len(match.group(1))
        title = match.group(2).strip().lower()
        if title not in _RULE_HEADINGS:
            i += 1
            continue
        # Collect content until the next heading at the same or higher level.
        body_lines: list[str] = []
        j = i + 1
        while j < len(lines):
            next_match = _HEADING_RE.match(lines[j])
            if next_match and len(next_match.group(1)) <= level:
                break
            stripped = lines[j].rstrip()
            if stripped:
                body_lines.append(stripped)
            j += 1
        if body_lines:
            out.append({"section": match.group(2).strip(), "lines": body_lines})
        i = j
    return out


def render_compact(entries: list[dict]) -> str:
    """One-line-per-bullet rendering for the trace read header."""
    if not entries:
        return ""
    out: list[str] = []
    for entry in entries:
        out.append(f"# {entry['source']} · {entry['section']}")
        for line in entry["lines"]:
            out.append(line)
        out.append("")
    return "\n".join(out).rstrip()
