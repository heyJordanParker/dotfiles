//! C tree-sitter extraction: `#include` imports, function / struct / enum
//! declarations, and call references.
//!
//! C has no methods or classes — every function is free and every call is a
//! `Free` reference (no container, no member/static shapes). Declarations
//! cover function definitions (the name nested in the declarator), structs /
//! unions (`class`) and enums (`enum`). Imports are `#include` paths.

use crate::extraction::{Declaration, Export, ExtractionResult, Import, Reference, RefShape};
use tree_sitter::{Node, Parser};

fn empty() -> ExtractionResult {
    ExtractionResult {
        language: "c".into(),
        imports: vec![],
        exports: vec![],
        declarations: vec![],
        references: vec![],
    }
}

pub fn extract(source: &[u8]) -> ExtractionResult {
    let lang: tree_sitter::Language = tree_sitter_c::LANGUAGE.into();
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

/// Imports/exports/declarations/references from an already-parsed C tree.
/// Caller guarantees `tree` came from the C grammar.
pub fn extract_from_tree(tree: &tree_sitter::Tree, source: &[u8]) -> ExtractionResult {
    let root = tree.root_node();
    let imports = walk_imports(root, source);
    let declarations = walk_declarations(root, source);
    // Every C top-level declaration is "exported" (visible to other TUs
    // unless `static`); the structure view wants them all.
    let exports = declarations
        .iter()
        .map(|d| Export {
            name: d.name.clone(),
            kind: d.kind.clone(),
            line: d.line,
        })
        .collect();
    let references = walk_references(root, source);
    ExtractionResult {
        language: "c".into(),
        imports,
        exports,
        declarations,
        references,
    }
}

/// `#include <stdio.h>` / `#include "local.h"` — the path is the module; no
/// symbol.
fn walk_imports(root: Node, source: &[u8]) -> Vec<Import> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        if n.kind() == "preproc_include" {
            if let Some(path) = n.child_by_field_name("path") {
                if let Ok(raw) = path.utf8_text(source) {
                    let module = raw
                        .trim_matches(|c| c == '<' || c == '>' || c == '"')
                        .to_string();
                    out.push(Import {
                        module,
                        symbol: None,
                        line: n.start_position().row as i64 + 1,
                    });
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

/// Function definitions, structs / unions, and enums. C has no nesting of
/// named functions, so every declaration has no container.
fn walk_declarations(root: Node, source: &[u8]) -> Vec<Declaration> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        match n.kind() {
            "function_definition" => {
                if let Some((name, line)) = function_name(n, source) {
                    if seen.insert((name.clone(), line)) {
                        out.push(Declaration {
                            name,
                            kind: "function".to_string(),
                            line,
                            container: None,
                        });
                    }
                }
            }
            "struct_specifier" | "union_specifier" | "enum_specifier" => {
                if let Some(name_node) = n.child_by_field_name("name") {
                    if let Ok(name) = name_node.utf8_text(source) {
                        let kind = if n.kind() == "enum_specifier" {
                            "enum"
                        } else {
                            "class"
                        };
                        let line = name_node.start_position().row as i64 + 1;
                        if seen.insert((name.to_string(), line)) {
                            out.push(Declaration {
                                name: name.to_string(),
                                kind: kind.to_string(),
                                line,
                                container: None,
                            });
                        }
                    }
                }
            }
            _ => {}
        }
        let mut c = n.walk();
        for child in n.children(&mut c) {
            stack.push(child);
        }
    }
    out.sort_by_key(|d| d.line);
    out
}

/// The function name of a `function_definition`: drill through the
/// declarator chain (pointer / array declarators wrap a `function_declarator`
/// whose declarator is the `identifier`).
fn function_name(n: Node, source: &[u8]) -> Option<(String, i64)> {
    let mut node = n.child_by_field_name("declarator")?;
    loop {
        match node.kind() {
            "identifier" => {
                let txt = node.utf8_text(source).ok()?;
                return Some((txt.to_string(), node.start_position().row as i64 + 1));
            }
            "function_declarator"
            | "pointer_declarator"
            | "array_declarator"
            | "parenthesized_declarator" => {
                node = node.child_by_field_name("declarator")?;
            }
            _ => return None,
        }
    }
}

/// Every `call_expression` — a bare `name(..)` is a `Free` call (C has no
/// member or static calls). A call through a function pointer field
/// (`obj->fn(..)`) resolves the field name as `Member`.
fn walk_references(root: Node, source: &[u8]) -> Vec<Reference> {
    let mut out = Vec::new();
    let mut stack: Vec<(Node, Option<String>)> = vec![(root, None)];
    while let Some((n, enclosing)) = stack.pop() {
        if n.kind() == "call_expression" {
            if let Some(func) = n.child_by_field_name("function") {
                if let Some((name, line, shape)) = call_shape(func, source) {
                    out.push(Reference {
                        name,
                        line,
                        shape,
                        receiver: None,
                        enclosing: enclosing.clone(),
                    });
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

/// A bare `identifier` callee is a `Free` call. A `field_expression`
/// `obj.fn` / `obj->fn` (function-pointer field) is a `Member` call.
fn call_shape(node: Node, source: &[u8]) -> Option<(String, i64, RefShape)> {
    match node.kind() {
        "identifier" => {
            let txt = node.utf8_text(source).ok()?;
            Some((
                txt.to_string(),
                node.start_position().row as i64 + 1,
                RefShape::Free,
            ))
        }
        "field_expression" => {
            let field = node.child_by_field_name("field")?;
            let txt = field.utf8_text(source).ok()?;
            Some((
                txt.to_string(),
                field.start_position().row as i64 + 1,
                RefShape::Member,
            ))
        }
        _ => None,
    }
}

/// A `function_definition` introduces its function name as the enclosing
/// scope for its body.
fn enclosing_function_name(node: Node, source: &[u8]) -> Option<String> {
    if node.kind() == "function_definition" {
        return function_name(node, source).map(|(n, _)| n);
    }
    None
}
