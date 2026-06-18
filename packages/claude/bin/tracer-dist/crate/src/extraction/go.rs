//! Go tree-sitter extraction: imports, top-level / method declarations, and
//! call / construction references.
//!
//! Declarations cover functions (`function_declaration`), methods
//! (`method_declaration`, whose receiver type is the `container`), and named
//! types (`type_spec` with a struct / interface / other underlying type).
//! References cover free calls (`Name(..)` — `Free`), selector calls
//! (`pkg.Name(..)` / `x.Name(..)` — `Free`, since Go's dominant cross-file
//! edge is the package-qualified function call and the site names no type)
//! and composite literals (`Type{..}` — `Static`).

use crate::extraction::{Declaration, Export, ExtractionResult, Import, Reference, RefShape};
use tree_sitter::{Node, Parser};

fn empty() -> ExtractionResult {
    ExtractionResult {
        language: "go".into(),
        imports: vec![],
        exports: vec![],
        declarations: vec![],
        references: vec![],
    }
}

pub fn extract(source: &[u8]) -> ExtractionResult {
    let lang: tree_sitter::Language = tree_sitter_go::LANGUAGE.into();
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

/// Imports/exports/declarations/references from an already-parsed Go tree.
/// Caller guarantees `tree` came from the Go grammar.
pub fn extract_from_tree(tree: &tree_sitter::Tree, source: &[u8]) -> ExtractionResult {
    let root = tree.root_node();
    let imports = walk_imports(root, source);
    let declarations = walk_declarations(root, source);
    // Go exports anything starting with an uppercase letter; the structure
    // view wants the module-level (non-method) named items.
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
        language: "go".into(),
        imports,
        exports,
        declarations,
        references,
    }
}

/// Every `import_spec` path. The module is the full quoted path; the symbol
/// is its last path segment (the package name the code uses unqualified).
fn walk_imports(root: Node, source: &[u8]) -> Vec<Import> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        if n.kind() == "import_spec" {
            if let Some(path) = n.child_by_field_name("path") {
                if let Ok(raw) = path.utf8_text(source) {
                    let module = raw.trim_matches('"').to_string();
                    let symbol = module.rsplit('/').next().map(|s| s.to_string());
                    out.push(Import {
                        module,
                        symbol,
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

/// Functions, methods (receiver type as `container`), and named types.
fn walk_declarations(root: Node, source: &[u8]) -> Vec<Declaration> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        match n.kind() {
            "function_declaration" => {
                push_named(&n, source, "function", None, &mut seen, &mut out);
            }
            "method_declaration" => {
                let container = method_receiver_type(n, source);
                push_named(&n, source, "function", container, &mut seen, &mut out);
            }
            "type_spec" => {
                if let Some(name_node) = n.child_by_field_name("name") {
                    if let Ok(name) = name_node.utf8_text(source) {
                        let kind = match n.child_by_field_name("type").map(|t| t.kind()) {
                            Some("interface_type") => "interface",
                            Some("struct_type") => "class",
                            _ => "class",
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

fn push_named(
    n: &Node,
    source: &[u8],
    kind: &str,
    container: Option<String>,
    seen: &mut std::collections::HashSet<(String, i64)>,
    out: &mut Vec<Declaration>,
) {
    if let Some(name_node) = n.child_by_field_name("name") {
        if let Ok(name) = name_node.utf8_text(source) {
            let line = name_node.start_position().row as i64 + 1;
            if seen.insert((name.to_string(), line)) {
                out.push(Declaration {
                    name: name.to_string(),
                    kind: kind.to_string(),
                    line,
                    container,
                });
            }
        }
    }
}

/// The receiver type of a `method_declaration`: `func (r *Foo) Bar()` →
/// `Foo`. The receiver is a `parameter_list`; drill to the type identifier,
/// stripping a leading pointer.
fn method_receiver_type(n: Node, source: &[u8]) -> Option<String> {
    let recv = n.child_by_field_name("receiver")?;
    let mut stack = vec![recv];
    while let Some(node) = stack.pop() {
        match node.kind() {
            "type_identifier" => {
                return node.utf8_text(source).ok().map(|s| s.to_string());
            }
            _ => {
                let mut c = node.walk();
                for child in node.children(&mut c) {
                    stack.push(child);
                }
            }
        }
    }
    None
}

/// Calls and composite literals, stamped with shape and enclosing function.
fn walk_references(root: Node, source: &[u8]) -> Vec<Reference> {
    let mut out = Vec::new();
    let mut stack: Vec<(Node, Option<String>)> = vec![(root, None)];
    while let Some((n, enclosing)) = stack.pop() {
        let line = n.start_position().row as i64 + 1;
        match n.kind() {
            "call_expression" => {
                if let Some(func) = n.child_by_field_name("function") {
                    if let Some((name, l, shape, receiver)) = call_shape(func, source) {
                        out.push(Reference {
                            name,
                            line: l,
                            shape,
                            receiver,
                            enclosing: enclosing.clone(),
                        });
                    }
                }
            }
            // `Type{..}` — composite-literal construction. The named type is
            // a Static use. `&Type{..}` and bare `Type{..}` both carry a
            // `type` field.
            "composite_literal" => {
                if let Some(ty) = n.child_by_field_name("type") {
                    if matches!(ty.kind(), "type_identifier" | "qualified_type") {
                        if let Some(name) = last_type_segment(ty, source) {
                            out.push(Reference {
                                name: name.clone(),
                                line,
                                shape: RefShape::Static,
                                receiver: Some(name),
                                enclosing: enclosing.clone(),
                            });
                        }
                    }
                }
            }
            _ => {}
        }
        let child_enclosing = enclosing_function_name(n, source).or_else(|| enclosing.clone());
        let mut c = n.walk();
        for child in n.children(&mut c) {
            stack.push((child, child_enclosing.clone()));
        }
    }
    out
}

/// A bare `identifier` callee is `Free`. A `selector_expression`
/// `operand.field` is `Member` — Go does not name the operand's type at the
/// site (it could be a package or a value), so it resolves to methods of
/// that name, matching the member-call model.
fn call_shape(node: Node, source: &[u8]) -> Option<(String, i64, RefShape, Option<String>)> {
    match node.kind() {
        "identifier" => {
            let txt = node.utf8_text(source).ok()?;
            Some((
                txt.to_string(),
                node.start_position().row as i64 + 1,
                RefShape::Free,
                None,
            ))
        }
        // `operand.field(..)` — in Go this is overwhelmingly a
        // package-qualified function call (`pkg.Func()`), the dominant
        // cross-file edge. It can also be a value method call (`v.Method()`),
        // but Go names no type at the site, so the two are indistinguishable
        // syntactically. Classifying it `Free` resolves the high-value
        // package-function case to the free function of that name; a
        // value-method call to a method whose receiver type is not named at
        // the site does not resolve, matching the free-call model. The bare
        // last segment (`field`) is the called name; the operand is a
        // package or value, never a type, so the receiver is None.
        "selector_expression" => {
            let field = node.child_by_field_name("field")?;
            let txt = field.utf8_text(source).ok()?;
            Some((
                txt.to_string(),
                field.start_position().row as i64 + 1,
                RefShape::Free,
                None,
            ))
        }
        _ => None,
    }
}

/// Last segment of a (possibly qualified) type node — `pkg.Foo` → `Foo`,
/// `Foo` → `Foo`.
fn last_type_segment(node: Node, source: &[u8]) -> Option<String> {
    let txt = node.utf8_text(source).ok()?;
    let seg = txt.rsplit('.').next().unwrap_or(txt).trim();
    if seg.is_empty() {
        None
    } else {
        Some(seg.to_string())
    }
}

/// A `function_declaration` / `method_declaration` introduces its own name
/// as the enclosing function scope.
fn enclosing_function_name(node: Node, source: &[u8]) -> Option<String> {
    if matches!(node.kind(), "function_declaration" | "method_declaration") {
        return node
            .child_by_field_name("name")
            .and_then(|nm| nm.utf8_text(source).ok())
            .map(|s| s.to_string());
    }
    None
}
