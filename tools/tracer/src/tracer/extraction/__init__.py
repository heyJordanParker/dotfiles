"""Per-language extraction via tree-sitter.

Each language module exposes one function `extract(source: bytes, path: str)
-> ExtractionResult` that returns symbols (functions, classes, module-level
constants) and imports (module references with optional symbol). The result
is consumed by `file_facts` and combined across all files into the
architecture graph.

Languages live in their own module so the dispatch table stays trivial:
file extension -> module function. Adding a language is a 5-line change.
"""

from tracer.extraction.dispatch import (
    ExtractionResult,
    Export,
    Import,
    extract,
    supported_extensions,
)

__all__ = [
    "ExtractionResult",
    "Export",
    "Import",
    "extract",
    "supported_extensions",
]
