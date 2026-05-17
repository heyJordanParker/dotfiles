//! TypeScript/TSX/JS/JSX tree-sitter extraction: imports and exports.
//!
//! Note: tree-sitter 0.25's `QueryCursor::matches` does NOT auto-apply
//! `#eq?` text predicates, so the `(#eq? @_fn "require")` predicate is
//! enforced manually below.

use crate::extraction::{Export, ExtractionResult, Import};
use std::collections::HashMap;
use tree_sitter::{Parser, Query, QueryCursor, StreamingIterator};

const QUERY_SRC: &str = r#"
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
"#;

fn empty() -> ExtractionResult {
    ExtractionResult {
        language: "typescript".into(),
        imports: vec![],
        exports: vec![],
    }
}

pub fn extract(source: &[u8], path: &str, _is_tsx_arg: bool) -> ExtractionResult {
    // The parser/query is keyed by the `.tsx`/`.jsx` suffix on the path.
    let lower = path.to_lowercase();
    let is_tsx = lower.ends_with(".tsx") || lower.ends_with(".jsx");
    let lang: tree_sitter::Language = if is_tsx {
        tree_sitter_typescript::LANGUAGE_TSX.into()
    } else {
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
    };
    let mut parser = Parser::new();
    if parser.set_language(&lang).is_err() {
        return empty();
    }
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return empty(),
    };
    extract_from_tree(&tree, source, is_tsx)
}

/// Imports/exports from an already-parsed TS/TSX tree. Decoupled from the
/// parse for the single-parse optimization. `is_tsx` MUST match the grammar
/// the caller used to produce `tree` (so the query grammar matches the tree).
pub fn extract_from_tree(
    tree: &tree_sitter::Tree,
    source: &[u8],
    is_tsx: bool,
) -> ExtractionResult {
    let lang: tree_sitter::Language = if is_tsx {
        tree_sitter_typescript::LANGUAGE_TSX.into()
    } else {
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
    };
    let query = match Query::new(&lang, QUERY_SRC) {
        Ok(q) => q,
        Err(_) => return empty(),
    };
    let capture_names = query.capture_names();

    #[derive(Clone)]
    struct Cap {
        name: String,
        text: String,
        line: i64,
        /// For an `import.symbol`: the module of its enclosing
        /// `import_statement`, resolved structurally (an `import { a } from
        /// 'm'` has the module string AFTER the specifiers, so a
        /// byte-distance heuristic resolves it to the wrong module).
        owner_module: Option<String>,
    }
    let mut caps: Vec<Cap> = Vec::new();
    let mut cursor = QueryCursor::new();
    let mut it = cursor.matches(&query, tree.root_node(), source);
    while let Some(m) = it.next() {
        // Manually enforce `(#eq? @_fn "require")`: if this match has an
        // `@_fn` capture, drop the whole match unless its text is "require".
        let mut fn_text: Option<String> = None;
        let mut has_fn = false;
        for cap in m.captures {
            if capture_names[cap.index as usize] == "_fn" {
                has_fn = true;
                fn_text = cap.node.utf8_text(source).ok().map(|s| s.to_string());
            }
        }
        if has_fn && fn_text.as_deref() != Some("require") {
            continue;
        }
        for cap in m.captures {
            let name = capture_names[cap.index as usize];
            if name == "_fn" {
                continue;
            }
            let node = cap.node;
            let owner_module = if name == "import.symbol" {
                enclosing_import_module(node, source)
            } else {
                None
            };
            caps.push(Cap {
                name: name.to_string(),
                text: node.utf8_text(source).unwrap_or("").to_string(),
                line: node.start_position().row as i64 + 1,
                owner_module,
            });
        }
    }

    let mut order: Vec<String> = Vec::new();
    let mut groups: HashMap<String, Vec<Cap>> = HashMap::new();
    for c in caps {
        if !groups.contains_key(&c.name) {
            order.push(c.name.clone());
        }
        groups.entry(c.name.clone()).or_default().push(c);
    }

    let mut imports: Vec<Import> = Vec::new();
    let mut exports: Vec<Export> = Vec::new();

    for name in &order {
        for c in &groups[name] {
            match c.name.as_str() {
                "import.module" => imports.push(Import {
                    module: c.text.clone(),
                    symbol: None,
                    line: c.line,
                }),
                "import.symbol" => {
                    let module = c
                        .owner_module
                        .clone()
                        .unwrap_or_else(|| "unknown".to_string());
                    imports.push(Import {
                        module,
                        symbol: Some(c.text.clone()),
                        line: c.line,
                    });
                }
                "export.function" => exports.push(Export {
                    name: c.text.clone(),
                    kind: "function".into(),
                    line: c.line,
                }),
                "export.class" => exports.push(Export {
                    name: c.text.clone(),
                    kind: "class".into(),
                    line: c.line,
                }),
                "export.constant" => exports.push(Export {
                    name: c.text.clone(),
                    kind: "constant".into(),
                    line: c.line,
                }),
                "export.interface" => exports.push(Export {
                    name: c.text.clone(),
                    kind: "interface".into(),
                    line: c.line,
                }),
                "export.type" => exports.push(Export {
                    name: c.text.clone(),
                    kind: "type".into(),
                    line: c.line,
                }),
                _ => {}
            }
        }
    }

    ExtractionResult {
        language: "typescript".into(),
        imports,
        exports,
    }
}

/// The module string of the `import_statement` that structurally encloses
/// an `import_specifier`. Walks ancestors to the `import_statement`, then
/// reads its `source` string fragment. This is order-independent: in
/// `import { a } from 'm'` the module string follows the specifiers, so a
/// byte-position heuristic would mis-resolve it.
fn enclosing_import_module(node: tree_sitter::Node, source: &[u8]) -> Option<String> {
    let mut cur = node.parent();
    while let Some(n) = cur {
        if n.kind() == "import_statement" {
            let src = n.child_by_field_name("source")?;
            // `source` is a `string` node; its inner `string_fragment`
            // child holds the bare module path without quotes.
            let mut c = src.walk();
            for child in src.children(&mut c) {
                if child.kind() == "string_fragment" {
                    return child
                        .utf8_text(source)
                        .ok()
                        .map(|s| s.to_string());
                }
            }
            return None;
        }
        cur = n.parent();
    }
    None
}
