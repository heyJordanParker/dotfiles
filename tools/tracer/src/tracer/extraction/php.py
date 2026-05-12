"""Tree-sitter extraction for PHP.

Extracts `use` statements (the PHP import mechanism) and module-level
definitions — functions, classes, interfaces, traits, enums.
"""

from __future__ import annotations

from functools import lru_cache

from tracer.extraction.dispatch import Export, ExtractionResult, Import


@lru_cache(maxsize=1)
def _parser():
    import tree_sitter_php
    from tree_sitter import Language, Parser

    return Parser(Language(tree_sitter_php.language_php()))


@lru_cache(maxsize=1)
def _query():
    import tree_sitter_php
    from tree_sitter import Language, Query

    language = Language(tree_sitter_php.language_php())
    return Query(
        language,
        """
        ; use statement: `use App\\Models\\User;`
        (namespace_use_declaration
          (namespace_use_clause (qualified_name) @import.module))
        ; namespaced names are also captured as `name` in some grammars
        (namespace_use_declaration
          (namespace_use_clause (name) @import.module))

        ; class / interface / trait / enum / function declarations
        (class_declaration name: (name) @export.class)
        (interface_declaration name: (name) @export.interface)
        (trait_declaration name: (name) @export.class)
        (enum_declaration name: (name) @export.class)
        (function_definition name: (name) @export.function)
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

    for capture_name, nodes in captures.items():
        for node in nodes:
            text = node.text.decode("utf-8", errors="replace")
            line = node.start_point[0] + 1

            if capture_name == "import.module":
                # PHP `use App\Models\User` — the imported symbol is the
                # last segment; the module is the leading namespace.
                segments = text.replace("\\\\", "\\").split("\\")
                if len(segments) > 1:
                    module = "\\".join(segments[:-1])
                    symbol = segments[-1]
                else:
                    module = text
                    symbol = None
                imports.append(Import(module=module, symbol=symbol, line=line))

            elif capture_name == "export.function":
                exports.append(Export(name=text, kind="function", line=line))

            elif capture_name == "export.class":
                exports.append(Export(name=text, kind="class", line=line))

            elif capture_name == "export.interface":
                exports.append(Export(name=text, kind="interface", line=line))

    return ExtractionResult(language="php", imports=imports, exports=exports)
