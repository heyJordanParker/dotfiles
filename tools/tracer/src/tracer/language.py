"""File extension → language detection.

Maps file extensions to:
- ast-grep language identifiers (for `trace struct`)
- multilspy language identifiers (for `trace callers / defines / symbols`)
- lizard parser identifiers (used implicitly via lizard.analyze_file)
"""

from __future__ import annotations

from pathlib import Path

# Map extensions to (ast-grep id, multilspy id) pairs.
# multilspy id of None means LSP is not configured for that language.
EXTENSION_MAP: dict[str, tuple[str, str | None]] = {
    ".py": ("python", "python"),
    ".ts": ("typescript", "typescript"),
    ".tsx": ("tsx", "typescript"),
    ".js": ("javascript", "typescript"),
    ".jsx": ("jsx", "typescript"),
    ".rb": ("ruby", "ruby"),
    ".go": ("go", "go"),
    ".rs": ("rust", "rust"),
    ".java": ("java", "java"),
    ".kt": ("kotlin", None),
    ".cs": ("csharp", "csharp"),
    ".php": ("php", "php"),
    ".c": ("c", None),
    ".h": ("c", None),
    ".cpp": ("cpp", None),
    ".hpp": ("cpp", None),
    ".swift": ("swift", None),
    ".dart": ("dart", "dart"),
    ".scala": ("scala", None),
    ".sh": ("bash", None),
    ".bash": ("bash", None),
    ".html": ("html", None),
    ".css": ("css", None),
    ".json": ("json", None),
    ".yaml": ("yaml", None),
    ".yml": ("yaml", None),
}


def ast_grep_language(path: str | Path) -> str | None:
    ext = Path(path).suffix.lower()
    pair = EXTENSION_MAP.get(ext)
    return pair[0] if pair else None


def lsp_language(path: str | Path) -> str | None:
    ext = Path(path).suffix.lower()
    pair = EXTENSION_MAP.get(ext)
    return pair[1] if pair else None
