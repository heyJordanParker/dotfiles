//! PHP tree-sitter extraction: `use` statements and class/interface/function defs.

use crate::extraction::{Export, ExtractionResult, Import};
use std::collections::HashMap;
use tree_sitter::{Parser, Query, QueryCursor, StreamingIterator};

const QUERY_SRC: &str = r#"
        ; use statement: `use App\Models\User;`
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
"#;

fn empty() -> ExtractionResult {
    ExtractionResult {
        language: "php".into(),
        imports: vec![],
        exports: vec![],
    }
}

pub fn extract(source: &[u8]) -> ExtractionResult {
    let lang: tree_sitter::Language = tree_sitter_php::LANGUAGE_PHP.into();
    let mut parser = Parser::new();
    if parser.set_language(&lang).is_err() {
        return empty();
    }
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return empty(),
    };
    extract_from_tree(&tree, source)
}

/// Imports/exports from an already-parsed PHP tree. Decoupled from the
/// parse for single-parse. Caller guarantees `tree` came from the PHP
/// grammar.
pub fn extract_from_tree(
    tree: &tree_sitter::Tree,
    source: &[u8],
) -> ExtractionResult {
    let lang: tree_sitter::Language = tree_sitter_php::LANGUAGE_PHP.into();
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
    }
    let mut caps: Vec<Cap> = Vec::new();
    let mut cursor = QueryCursor::new();
    let mut it = cursor.matches(&query, tree.root_node(), source);
    while let Some(m) = it.next() {
        for cap in m.captures {
            let node = cap.node;
            caps.push(Cap {
                name: capture_names[cap.index as usize].to_string(),
                text: node.utf8_text(source).unwrap_or("").to_string(),
                line: node.start_position().row as i64 + 1,
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
                "import.module" => {
                    // Collapse escaped namespace separators then split:
                    // replace "\\\\" (two backslashes) with "\\" (one),
                    // then split on '\\' (one).
                    let normalized = c.text.replace("\\\\", "\\");
                    let segments: Vec<&str> = normalized.split('\\').collect();
                    let (module, symbol) = if segments.len() > 1 {
                        (
                            segments[..segments.len() - 1].join("\\"),
                            Some(segments[segments.len() - 1].to_string()),
                        )
                    } else {
                        (c.text.clone(), None)
                    };
                    imports.push(Import {
                        module,
                        symbol,
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
                "export.interface" => exports.push(Export {
                    name: c.text.clone(),
                    kind: "interface".into(),
                    line: c.line,
                }),
                _ => {}
            }
        }
    }

    ExtractionResult {
        language: "php".into(),
        imports,
        exports,
    }
}
