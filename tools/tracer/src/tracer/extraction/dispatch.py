"""Extension -> per-language extractor dispatch.

Per /naming: full words. Each per-language module exposes `extract` taking
source bytes and the file path, returning ExtractionResult.
"""

from __future__ import annotations

from dataclasses import asdict, dataclass, field
from pathlib import Path


@dataclass
class Import:
    """A single import statement.

    `module` is the module path as written in the source (`tracer.deps`,
    `express`, `App\\Models\\User`). `symbol` is the imported name when the
    import names one (None when the whole module is imported). `line` is
    1-indexed for display.
    """

    module: str
    symbol: str | None
    line: int


@dataclass
class Export:
    """A symbol defined at module-level scope.

    `kind` is one of "function", "class", "constant", "interface", "type".
    `line` is 1-indexed.
    """

    name: str
    kind: str
    line: int


@dataclass
class ExtractionResult:
    """What one language extractor returns for one file."""

    language: str
    imports: list[Import] = field(default_factory=list)
    exports: list[Export] = field(default_factory=list)

    def to_dict(self) -> dict:
        return {
            "language": self.language,
            "imports": [asdict(i) for i in self.imports],
            "exports": [asdict(e) for e in self.exports],
        }

    @classmethod
    def from_dict(cls, data: dict) -> "ExtractionResult":
        return cls(
            language=data.get("language", "unknown"),
            imports=[Import(**i) for i in data.get("imports", [])],
            exports=[Export(**e) for e in data.get("exports", [])],
        )


# Extension -> (language id, callable that takes (source, path) -> ExtractionResult)
def _python_extractor():
    from tracer.extraction.python import extract as python_extract
    return python_extract


def _typescript_extractor():
    from tracer.extraction.typescript import extract as typescript_extract
    return typescript_extract


def _php_extractor():
    from tracer.extraction.php import extract as php_extract
    return php_extract


_EXTRACTORS: dict[str, tuple[str, callable]] = {
    ".py": ("python", _python_extractor),
    ".ts": ("typescript", _typescript_extractor),
    ".tsx": ("typescript", _typescript_extractor),
    ".js": ("typescript", _typescript_extractor),
    ".jsx": ("typescript", _typescript_extractor),
    ".php": ("php", _php_extractor),
}


def supported_extensions() -> set[str]:
    return set(_EXTRACTORS)


def extract(source: bytes, path: str) -> ExtractionResult | None:
    """Run the language extractor matching `path`'s extension.

    Returns None for unsupported extensions; commands handle this by
    skipping the file from the architecture layer (it still gets per-file
    complexity via lizard).
    """
    extension = Path(path).suffix.lower()
    entry = _EXTRACTORS.get(extension)
    if entry is None:
        return None
    _language_name, factory = entry
    extractor = factory()
    return extractor(source, path)
