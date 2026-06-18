//! Ruby tree-sitter extraction: `require` imports, method / class / module
//! declarations, and call references.
//!
//! Declarations cover methods (`method` / `singleton_method`, whose
//! enclosing class or module is the `container`), classes (`class`) and
//! modules (`module`, modelled as an interface — a mixin namespace).
//! References cover free calls (`name` with no receiver — `Free`) and
//! receiver calls (`obj.name` — `Member`, the receiver value's class is not
//! named at the site). Ruby constructs via `Foo.new`, a member call on the
//! constant, so construction is not a separate reference shape here.

use crate::extraction::{Declaration, Export, ExtractionResult, Import, Reference, RefShape};
use tree_sitter::{Node, Parser};

fn empty() -> ExtractionResult {
    ExtractionResult {
        language: "ruby".into(),
        imports: vec![],
        exports: vec![],
        declarations: vec![],
        references: vec![],
    }
}

pub fn extract(source: &[u8]) -> ExtractionResult {
    let lang: tree_sitter::Language = tree_sitter_ruby::LANGUAGE.into();
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

/// Imports/exports/declarations/references from an already-parsed Ruby tree.
/// Caller guarantees `tree` came from the Ruby grammar.
pub fn extract_from_tree(tree: &tree_sitter::Tree, source: &[u8]) -> ExtractionResult {
    let root = tree.root_node();
    let imports = walk_imports(root, source);
    let declarations = walk_declarations(root, source);
    let exports = declarations
        .iter()
        .filter(|d| d.container.is_none())
        .map(|d| Export {
            name: d.name.clone(),
            kind: d.kind.clone(),
            line: d.line,
        })
        .collect();
    let references = walk_references(root, source);
    ExtractionResult {
        language: "ruby".into(),
        imports,
        exports,
        declarations,
        references,
    }
}

/// `require 'x'` / `require_relative 'x'` — the only Ruby import statement.
/// Surfaces as a `call` whose method is `require`/`require_relative` and
/// whose first argument is a string. The module is the string; no symbol.
fn walk_imports(root: Node, source: &[u8]) -> Vec<Import> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        if n.kind() == "call" {
            let method_name = n
                .child_by_field_name("method")
                .and_then(|m| m.utf8_text(source).ok());
            if matches!(method_name, Some("require") | Some("require_relative")) {
                if let Some(args) = n.child_by_field_name("arguments") {
                    let mut c = args.walk();
                    for arg in args.children(&mut c) {
                        if arg.kind() == "string" {
                            let raw = arg.utf8_text(source).unwrap_or("");
                            let module = raw.trim_matches(|c| c == '\'' || c == '"').to_string();
                            out.push(Import {
                                module,
                                symbol: None,
                                line: n.start_position().row as i64 + 1,
                            });
                        }
                    }
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

/// Methods (container = enclosing class/module), classes, and modules.
fn walk_declarations(root: Node, source: &[u8]) -> Vec<Declaration> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut stack: Vec<(Node, Option<String>)> = vec![(root, None)];
    while let Some((n, container)) = stack.pop() {
        let kind: Option<&str> = match n.kind() {
            "method" => Some("function"),
            "singleton_method" => Some("function"),
            "class" => Some("class"),
            "module" => Some("interface"),
            _ => None,
        };
        let mut child_container = container.clone();
        if matches!(n.kind(), "class" | "module") {
            child_container = constant_name(n, source).or_else(|| container.clone());
        }
        if let Some(k) = kind {
            let name = match n.kind() {
                "class" | "module" => constant_name(n, source),
                _ => n
                    .child_by_field_name("name")
                    .and_then(|nm| nm.utf8_text(source).ok())
                    .map(|s| s.to_string()),
            };
            if let Some(name) = name {
                let line = n
                    .child_by_field_name("name")
                    .unwrap_or(n)
                    .start_position()
                    .row as i64
                    + 1;
                let decl_container = if matches!(n.kind(), "method" | "singleton_method") {
                    container.clone()
                } else {
                    None
                };
                if seen.insert((name.clone(), line)) {
                    out.push(Declaration {
                        name,
                        kind: k.to_string(),
                        line,
                        container: decl_container,
                    });
                }
            }
        }
        let mut c = n.walk();
        for child in n.children(&mut c) {
            stack.push((child, child_container.clone()));
        }
    }
    out.sort_by_key(|d| d.line);
    out
}

/// The constant name of a `class` / `module` node — its `name` field, last
/// `::` segment (`Foo::Bar` → `Bar`).
fn constant_name(n: Node, source: &[u8]) -> Option<String> {
    let name_node = n.child_by_field_name("name")?;
    let txt = name_node.utf8_text(source).ok()?;
    let seg = txt.rsplit("::").next().unwrap_or(txt).trim();
    if seg.is_empty() {
        None
    } else {
        Some(seg.to_string())
    }
}

/// Calls, stamped with shape and enclosing method. A `call` with no receiver
/// is `Free`; with a receiver it is `Member` (the receiver value's class is
/// not named at the site).
fn walk_references(root: Node, source: &[u8]) -> Vec<Reference> {
    let mut out = Vec::new();
    let mut stack: Vec<(Node, Option<String>)> = vec![(root, None)];
    while let Some((n, enclosing)) = stack.pop() {
        if n.kind() == "call" {
            if let Some(method) = n.child_by_field_name("method") {
                if let Ok(name) = method.utf8_text(source) {
                    // `require`/`require_relative` are imports, not calls.
                    if !matches!(name, "require" | "require_relative") {
                        let shape = if n.child_by_field_name("receiver").is_some() {
                            RefShape::Member
                        } else {
                            RefShape::Free
                        };
                        out.push(Reference {
                            name: name.to_string(),
                            line: method.start_position().row as i64 + 1,
                            shape,
                            receiver: None,
                            enclosing: enclosing.clone(),
                        });
                    }
                }
            }
        }
        let child_enclosing = enclosing_function_name(n, source).or_else(|| enclosing.clone());
        let mut c = n.walk();
        for child in n.children(&mut c) {
            stack.push((child, child_enclosing.clone()));
        }
    }
    out
}

/// A `method` / `singleton_method` introduces its own name as the enclosing
/// method scope.
fn enclosing_function_name(node: Node, source: &[u8]) -> Option<String> {
    if matches!(node.kind(), "method" | "singleton_method") {
        return node
            .child_by_field_name("name")
            .and_then(|nm| nm.utf8_text(source).ok())
            .map(|s| s.to_string());
    }
    None
}
