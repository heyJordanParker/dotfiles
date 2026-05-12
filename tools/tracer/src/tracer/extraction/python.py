"""Tree-sitter extraction for Python.

Extracts module-level imports (both `import x` and `from x import y` forms)
and module-level definitions (functions, classes, top-level constants).
Nested definitions and method definitions are intentionally excluded —
the architecture graph models module-public surface, not internal structure.
"""

from __future__ import annotations

from functools import lru_cache

from tracer.extraction.dispatch import Export, ExtractionResult, Import


@lru_cache(maxsize=1)
def _parser():
    import tree_sitter_python
    from tree_sitter import Language, Parser

    return Parser(Language(tree_sitter_python.language()))


@lru_cache(maxsize=1)
def _query():
    import tree_sitter_python
    from tree_sitter import Language, Query

    language = Language(tree_sitter_python.language())
    return Query(
        language,
        """
        ; import_statement covers `import a`, `import a.b`, `import a as c`
        (import_statement
          name: (dotted_name) @import.module)
        (import_statement
          name: (aliased_import
                  name: (dotted_name) @import.module))

        ; import_from_statement covers `from x import y`, `from x import y as z`
        (import_from_statement
          module_name: (dotted_name) @import_from.module
          name: (dotted_name) @import_from.symbol)
        (import_from_statement
          module_name: (dotted_name) @import_from.module
          name: (aliased_import
                  name: (dotted_name) @import_from.symbol))

        ; module-level definitions only — nested defs are filtered below
        (module
          (function_definition name: (identifier) @export.function))
        (module
          (class_definition name: (identifier) @export.class))
        (module
          (decorated_definition
            definition: (function_definition name: (identifier) @export.function)))
        (module
          (decorated_definition
            definition: (class_definition name: (identifier) @export.class)))
        """,
    )


def extract(source: bytes, path: str) -> ExtractionResult:
    from tree_sitter import QueryCursor

    parser = _parser()
    query = _query()
    tree = parser.parse(source)
    captures = QueryCursor(query).captures(tree.root_node)

    imports: list[Import] = []
    exports: list[Export] = []

    pending_module: dict[int, str] = {}

    for capture_name, nodes in captures.items():
        for node in nodes:
            text = node.text.decode("utf-8", errors="replace")
            line = node.start_point[0] + 1

            if capture_name == "import.module":
                imports.append(Import(module=text, symbol=None, line=line))

            elif capture_name == "import_from.module":
                pending_module[node.start_byte] = text

            elif capture_name == "import_from.symbol":
                module = _nearest_pending_module(pending_module, node.start_byte)
                imports.append(Import(module=module, symbol=text, line=line))

            elif capture_name == "export.function":
                exports.append(Export(name=text, kind="function", line=line))

            elif capture_name == "export.class":
                exports.append(Export(name=text, kind="class", line=line))

    return ExtractionResult(language="python", imports=imports, exports=exports)


def _nearest_pending_module(pending: dict[int, str], symbol_byte: int) -> str:
    """For `from X import Y, Z`, both Y and Z attach to the same X.

    Tree-sitter captures don't pair them automatically — we look up the
    most recent module capture before the symbol's byte position.
    """
    candidates = [byte for byte in pending if byte < symbol_byte]
    if not candidates:
        return "unknown"
    return pending[max(candidates)]
