"""File digest helpers — substitute for separate read/structure/callers calls.

The goal: one `trace info <file>` call returns enough about the file that
an agent can decide whether to invest in a full read. Components:

- `leading_comment(path)` — first contiguous comment block from the top of
  the file, with the comment punctuation stripped. Captures the "what is
  this file for" docstring most projects put at the top.
- `top_callers(graph, file)` — list of `{source_file, source_line}` for
  modules that import or reference this file's module, cap N.
- `immediate_dependencies(graph, file)` — module ids this file imports,
  cap N.
"""

from __future__ import annotations

from pathlib import Path
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from tracer.architecture import Graph


_PY_COMMENT_PREFIXES = ("# ", "#")
_C_LIKE_LINE_COMMENT = ("// ", "//", "*", "* ")
_TRIPLE_QUOTES = ('"""', "'''")
_BLOCK_OPEN = ("/*", "/**")
_BLOCK_CLOSE = "*/"


def leading_comment(path: Path, max_lines: int = 25) -> str | None:
    """Extract the leading comment block as plain text. Returns None when
    no recognizable docblock is at the top of the file.

    Supported shapes:
      - PHP / TS / JS / C-like:  /\\*\\* ... \\*/, /\\* ... \\*/, // runs
      - Python / Ruby / shell:   triple-double, triple-single, hash runs
    """
    try:
        with open(path, encoding="utf-8", errors="replace") as fh:
            lines = [next(fh) for _ in range(max_lines)]
    except (StopIteration, OSError):
        try:
            with open(path, encoding="utf-8", errors="replace") as fh:
                lines = fh.read().splitlines(keepends=True)[:max_lines]
        except OSError:
            return None
    if not lines:
        return None

    # Skip blank lines, shebang, and language-specific opening tokens
    i = 0
    while i < len(lines) and (not lines[i].strip() or lines[i].startswith("#!")):
        i += 1
    # PHP: skip the opening `<?php` line
    if i < len(lines) and lines[i].strip().startswith("<?"):
        i += 1
    while i < len(lines) and not lines[i].strip():
        i += 1
    if i >= len(lines):
        return None

    head = lines[i].strip()

    # Block comment: /* ... */  /** ... */
    if any(head.startswith(token) for token in _BLOCK_OPEN):
        body: list[str] = []
        for line in lines[i:]:
            stripped = line.strip()
            # Strip opening tokens, leading stars, trailing close.
            cleaned = stripped
            for token in ("/**", "/*"):
                if cleaned.startswith(token):
                    cleaned = cleaned[len(token):].lstrip()
            if cleaned.endswith(_BLOCK_CLOSE):
                cleaned = cleaned[: -len(_BLOCK_CLOSE)].rstrip()
                if cleaned.startswith("*"):
                    cleaned = cleaned[1:].lstrip()
                if cleaned:
                    body.append(cleaned)
                break
            if cleaned.startswith("*"):
                cleaned = cleaned[1:].lstrip()
            body.append(cleaned)
        return "\n".join(line for line in body if line) or None

    # Triple-quoted docstring
    for quote in _TRIPLE_QUOTES:
        if head.startswith(quote):
            body = []
            first_after = head[len(quote):]
            if quote in first_after:  # Single-line """foo"""
                return first_after[: first_after.index(quote)].strip() or None
            if first_after:
                body.append(first_after)
            for line in lines[i + 1:]:
                stripped = line.rstrip()
                if quote in stripped:
                    body.append(stripped[: stripped.index(quote)].rstrip())
                    break
                body.append(stripped)
            return "\n".join(b for b in body if b is not None) or None

    # Run of // or # line comments
    line_tokens = _PY_COMMENT_PREFIXES + _C_LIKE_LINE_COMMENT
    if any(head.startswith(token) for token in line_tokens):
        body = []
        for line in lines[i:]:
            stripped = line.rstrip()
            if not stripped:
                break
            matched = False
            for token in line_tokens:
                if stripped.lstrip().startswith(token):
                    body.append(stripped.lstrip()[len(token):].strip())
                    matched = True
                    break
            if not matched:
                break
        return "\n".join(b for b in body if b) or None

    return None


def top_callers(graph: "Graph", relative_file: str, repo_root: "Path | None" = None, limit: int = 10) -> list[dict]:
    """Return the `limit` modules that depend on this file's module, each
    with a one-line summary from its leading docblock when `repo_root` is
    provided. The summary lets the agent see what kind of code is calling
    this file without a separate read."""
    from pathlib import Path
    from tracer import architecture
    module_id = graph.file_to_module_id.get(relative_file)
    if not module_id:
        return []
    out: list[dict] = []
    seen: set[str] = set()
    for edge in architecture.dependents_of(graph, module_id):
        if edge.source in seen:
            continue
        seen.add(edge.source)
        node = graph.nodes.get(edge.source)
        if node is None:
            continue
        meaningful_line = node.source_line if node.kind != "module" else None
        summary: str | None = None
        if repo_root is not None and node.source_file:
            caller_path = Path(repo_root) / node.source_file
            if caller_path.is_file():
                comment = leading_comment(caller_path, max_lines=15)
                if comment:
                    summary = comment.splitlines()[0].strip()
        out.append(
            {
                "source_file": node.source_file,
                "source_line": meaningful_line,
                "label": node.label,
                "kind": node.kind,
                "summary": summary,
            }
        )
        if len(out) >= limit:
            break
    return out


def immediate_dependencies(graph: "Graph", relative_file: str, limit: int = 15) -> list[dict]:
    """Modules that this file directly depends on (imports / references).
    Returns a list of `{module, confidence}` capped at `limit`."""
    from tracer import architecture
    module_id = graph.file_to_module_id.get(relative_file)
    if not module_id:
        return []
    out: list[dict] = []
    seen: set[str] = set()
    for edge in architecture.dependencies_of(graph, module_id):
        if edge.target in seen:
            continue
        seen.add(edge.target)
        node = graph.nodes.get(edge.target)
        label = node.label if node else edge.target
        out.append({"module": label, "confidence": edge.confidence})
        if len(out) >= limit:
            break
    return out
