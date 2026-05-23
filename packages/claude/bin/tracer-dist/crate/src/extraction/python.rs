//! Python tree-sitter extraction: module-level imports and definitions.

use crate::extraction::{Declaration, Export, ExtractionResult, Import, Reference};
use std::collections::HashMap;
use tree_sitter::{Node, Parser, Query, QueryCursor, StreamingIterator};

const QUERY_SRC: &str = r#"
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
"#;

fn empty() -> ExtractionResult {
    ExtractionResult {
        language: "python".into(),
        imports: vec![],
        exports: vec![],
        declarations: vec![],
        references: vec![],
    }
}

pub fn extract(source: &[u8]) -> ExtractionResult {
    let lang: tree_sitter::Language = tree_sitter_python::LANGUAGE.into();
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

/// Imports/exports from an already-parsed Python tree. Decoupled from the
/// parse so `file_facts` shares one tree with `ccn` (single-parse). Caller
/// guarantees `tree` came from the Python grammar.
pub fn extract_from_tree(
    tree: &tree_sitter::Tree,
    source: &[u8],
) -> ExtractionResult {
    let lang: tree_sitter::Language = tree_sitter_python::LANGUAGE.into();
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
        start_byte: usize,
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
                start_byte: node.start_byte(),
            });
        }
    }

    // Group captures by name in first-seen order, then iterate the nodes
    // within each group.
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

    // Symbol resolution depends only on byte position, not on the order
    // captures are emitted. Collect every pending module first so a
    // `from X import a, b` resolves its symbols correctly regardless of
    // capture-emission order.
    let mut pending_module: Vec<(usize, String)> = Vec::new();
    if let Some(g) = groups.get("import_from.module") {
        for c in g {
            pending_module.push((c.start_byte, c.text.clone()));
        }
    }

    for name in &order {
        for c in &groups[name] {
            match c.name.as_str() {
                "import.module" => imports.push(Import {
                    module: c.text.clone(),
                    symbol: None,
                    line: c.line,
                }),
                "import_from.module" => {}
                "import_from.symbol" => {
                    let module = nearest_pending(&pending_module, c.start_byte);
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
                _ => {}
            }
        }
    }

    let declarations = walk_declarations(tree.root_node(), source);
    let references = walk_references(tree.root_node(), source);

    ExtractionResult {
        language: "python".into(),
        imports,
        exports,
        declarations,
        references,
    }
}

/// Every `function_definition` and `class_definition` in the tree —
/// top-level, methods on classes, and definitions nested inside other
/// functions. Source-order, deduped (one entry per (name, line) pair).
fn walk_declarations(root: Node, source: &[u8]) -> Vec<Declaration> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        let kind = match n.kind() {
            "function_definition" => Some("function"),
            "class_definition" => Some("class"),
            _ => None,
        };
        if let Some(k) = kind {
            if let Some(name_node) = n.child_by_field_name("name") {
                if let Ok(name) = name_node.utf8_text(source) {
                    let line = name_node.start_position().row as i64 + 1;
                    if seen.insert((name.to_string(), line)) {
                        out.push(Declaration {
                            name: name.to_string(),
                            kind: k.to_string(),
                            line,
                        });
                    }
                }
            }
        }
        let mut c = n.walk();
        for child in n.children(&mut c) {
            stack.push(child);
        }
    }
    out.sort_by_key(|d| d.line);
    out
}

/// Every `call` expression in the tree. Resolves the callee identifier:
///   - bare `foo()`            → name = "foo"
///   - attribute `o.foo()`     → name = "foo"  (last segment)
///   - qualified `a.b.foo()`   → name = "foo"
/// Subscripts and complex callees are skipped — only the last identifier
/// matters for reference resolution against the declaration index.
fn walk_references(root: Node, source: &[u8]) -> Vec<Reference> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        if n.kind() == "call" {
            if let Some(func) = n.child_by_field_name("function") {
                if let Some((name, line)) = callee_name(func, source) {
                    out.push(Reference { name, line });
                }
            }
        }
        let mut c = n.walk();
        for child in n.children(&mut c) {
            stack.push(child);
        }
    }
    out
}

fn callee_name(node: Node, source: &[u8]) -> Option<(String, i64)> {
    match node.kind() {
        "identifier" => {
            let txt = node.utf8_text(source).ok()?;
            Some((txt.to_string(), node.start_position().row as i64 + 1))
        }
        "attribute" => {
            let attr = node.child_by_field_name("attribute")?;
            let txt = attr.utf8_text(source).ok()?;
            Some((txt.to_string(), attr.start_position().row as i64 + 1))
        }
        _ => None,
    }
}

/// The pending `from`-module whose byte offset most closely precedes a
/// symbol — the module that symbol was imported from.
fn nearest_pending(pending: &[(usize, String)], symbol_byte: usize) -> String {
    pending
        .iter()
        .filter(|(b, _)| *b < symbol_byte)
        .max_by_key(|(b, _)| *b)
        .map(|(_, m)| m.clone())
        .unwrap_or_else(|| "unknown".to_string())
}
