"""Tree-sitter extraction for TypeScript / TSX / JavaScript / JSX.

Extracts ES module imports (`import x from 'm'`, `import {y} from 'm'`,
`import * as x from 'm'`) and CommonJS requires (`require('m')`). Captures
top-level exports — function declarations, class declarations, exported
const declarations, and re-exports.
"""

from __future__ import annotations

from functools import lru_cache

from tracer.extraction.dispatch import Export, ExtractionResult, Import


@lru_cache(maxsize=2)
def _parser_for(language: str):
    from tree_sitter import Language, Parser

    if language == "tsx":
        import tree_sitter_typescript

        return Parser(Language(tree_sitter_typescript.language_tsx()))
    import tree_sitter_typescript

    return Parser(Language(tree_sitter_typescript.language_typescript()))


@lru_cache(maxsize=2)
def _query_for(language: str):
    from tree_sitter import Language, Query

    import tree_sitter_typescript

    if language == "tsx":
        ts_language = Language(tree_sitter_typescript.language_tsx())
    else:
        ts_language = Language(tree_sitter_typescript.language_typescript())

    return Query(
        ts_language,
        """
        ; import statements — module path is always a string literal
        (import_statement source: (string (string_fragment) @import.module))

        ; named imports inside an import statement — captured separately,
        ; paired by byte position with the nearest preceding module
        (import_specifier name: (identifier) @import.symbol)

        ; require('m')
        (call_expression
          function: (identifier) @_fn
          arguments: (arguments (string (string_fragment) @import.module))
          (#eq? @_fn "require"))

        ; exported function declarations
        (export_statement
          declaration: (function_declaration name: (identifier) @export.function))

        ; exported class declarations
        (export_statement
          declaration: (class_declaration name: (type_identifier) @export.class))

        ; exported const / let / var declarations
        (export_statement
          declaration: (lexical_declaration
                         (variable_declarator name: (identifier) @export.constant)))
        (export_statement
          declaration: (variable_declaration
                         (variable_declarator name: (identifier) @export.constant)))

        ; exported interfaces and types
        (export_statement
          declaration: (interface_declaration name: (type_identifier) @export.interface))
        (export_statement
          declaration: (type_alias_declaration name: (type_identifier) @export.type))
        """,
    )


def extract(source: bytes, path: str) -> ExtractionResult:
    from tree_sitter import QueryCursor

    is_tsx = path.lower().endswith(".tsx") or path.lower().endswith(".jsx")
    language_label = "tsx" if is_tsx else "ts"
    parser = _parser_for(language_label)
    query = _query_for(language_label)
    tree = parser.parse(source)
    captures = QueryCursor(query).captures(tree.root_node)

    # Pair named symbol imports with their nearest preceding module by byte
    module_anchors: list[tuple[int, str]] = []
    imports: list[Import] = []
    exports: list[Export] = []

    for capture_name, nodes in captures.items():
        for node in nodes:
            text = node.text.decode("utf-8", errors="replace")
            line = node.start_point[0] + 1

            if capture_name == "import.module":
                imports.append(Import(module=text, symbol=None, line=line))
                module_anchors.append((node.start_byte, text))

            elif capture_name == "import.symbol":
                module = _nearest_module(module_anchors, node.start_byte)
                imports.append(Import(module=module, symbol=text, line=line))

            elif capture_name == "export.function":
                exports.append(Export(name=text, kind="function", line=line))

            elif capture_name == "export.class":
                exports.append(Export(name=text, kind="class", line=line))

            elif capture_name == "export.constant":
                exports.append(Export(name=text, kind="constant", line=line))

            elif capture_name == "export.interface":
                exports.append(Export(name=text, kind="interface", line=line))

            elif capture_name == "export.type":
                exports.append(Export(name=text, kind="type", line=line))

    return ExtractionResult(
        language="typescript", imports=imports, exports=exports
    )


def _nearest_module(anchors: list[tuple[int, str]], symbol_byte: int) -> str:
    candidates = [(byte, mod) for byte, mod in anchors if byte < symbol_byte]
    if not candidates:
        return "unknown"
    return max(candidates)[1]
