//! Java tree-sitter extraction: imports, class / interface / enum / method
//! declarations, and call / construction / type references.
//!
//! Declarations cover classes, interfaces, enums, and their methods /
//! constructors (the enclosing type is the `container`). References cover
//! method invocations (`obj.name(..)` and bare `name(..)` — both `Member`,
//! since Java has no free functions and the receiver type is not named at
//! the site), object creation (`new Type(..)` — `Static`) and parameter /
//! return type hints (`Static`, the type names the class).

use crate::extraction::{Declaration, Export, ExtractionResult, Import, Reference, RefShape};
use tree_sitter::{Node, Parser};

fn empty() -> ExtractionResult {
    ExtractionResult {
        language: "java".into(),
        imports: vec![],
        exports: vec![],
        declarations: vec![],
        references: vec![],
    }
}

pub fn extract(source: &[u8]) -> ExtractionResult {
    let lang: tree_sitter::Language = tree_sitter_java::LANGUAGE.into();
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

/// Imports/exports/declarations/references from an already-parsed Java tree.
/// Caller guarantees `tree` came from the Java grammar.
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
        language: "java".into(),
        imports,
        exports,
        declarations,
        references,
    }
}

/// `import a.b.C;` → module `a.b`, symbol `C`. `import a.b.*;` → module
/// `a.b`, no symbol.
fn walk_imports(root: Node, source: &[u8]) -> Vec<Import> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        if n.kind() == "import_declaration" {
            let line = n.start_position().row as i64 + 1;
            // The declaration holds a scoped_identifier (and optionally a `*`
            // for wildcard imports).
            let mut c = n.walk();
            let mut is_wildcard = false;
            let mut path: Option<String> = None;
            for child in n.children(&mut c) {
                match child.kind() {
                    "scoped_identifier" | "identifier" => {
                        path = child.utf8_text(source).ok().map(|s| s.to_string());
                    }
                    "asterisk" | "*" => is_wildcard = true,
                    _ => {}
                }
            }
            if let Some(p) = path {
                if is_wildcard {
                    out.push(Import {
                        module: p,
                        symbol: None,
                        line,
                    });
                } else {
                    let (module, symbol) = match p.rsplit_once('.') {
                        Some((m, s)) => (m.to_string(), Some(s.to_string())),
                        None => (p.clone(), None),
                    };
                    out.push(Import {
                        module,
                        symbol,
                        line,
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

/// Types and their methods / constructors. A method's container is the
/// enclosing class / interface / enum; a constructor takes the type name as
/// its declared name.
fn walk_declarations(root: Node, source: &[u8]) -> Vec<Declaration> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut stack: Vec<(Node, Option<String>)> = vec![(root, None)];
    while let Some((n, container)) = stack.pop() {
        let kind: Option<&str> = match n.kind() {
            "class_declaration" => Some("class"),
            "interface_declaration" => Some("interface"),
            "enum_declaration" => Some("enum"),
            "method_declaration" => Some("function"),
            "constructor_declaration" => Some("function"),
            _ => None,
        };
        let mut child_container = container.clone();
        if matches!(
            n.kind(),
            "class_declaration" | "interface_declaration" | "enum_declaration"
        ) {
            child_container = n
                .child_by_field_name("name")
                .and_then(|nm| nm.utf8_text(source).ok())
                .map(|s| s.to_string())
                .or_else(|| container.clone());
        }
        if let Some(k) = kind {
            if let Some(name_node) = n.child_by_field_name("name") {
                if let Ok(name) = name_node.utf8_text(source) {
                    let line = name_node.start_position().row as i64 + 1;
                    let decl_container = if matches!(
                        n.kind(),
                        "method_declaration" | "constructor_declaration"
                    ) {
                        container.clone()
                    } else {
                        None
                    };
                    if seen.insert((name.to_string(), line)) {
                        out.push(Declaration {
                            name: name.to_string(),
                            kind: k.to_string(),
                            line,
                            container: decl_container,
                        });
                    }
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

/// Method invocations, object creation, and parameter / return type hints,
/// stamped with shape and enclosing method.
fn walk_references(root: Node, source: &[u8]) -> Vec<Reference> {
    let mut out = Vec::new();
    let mut stack: Vec<(Node, Option<String>)> = vec![(root, None)];
    while let Some((n, enclosing)) = stack.pop() {
        let line = n.start_position().row as i64 + 1;
        // The enclosing scope for this node's children — a method /
        // constructor names its own scope, so its parameter type hints
        // resolve to the declaring method.
        let child_enclosing = enclosing_function_name(n, source).or_else(|| enclosing.clone());
        match n.kind() {
            // `obj.name(..)` or bare `name(..)` — Java has no free functions,
            // so every invocation is a method call. The receiver type is not
            // named at the site, so it is `Member`.
            "method_invocation" => {
                if let Some(name_node) = n.child_by_field_name("name") {
                    if let Ok(name) = name_node.utf8_text(source) {
                        out.push(Reference {
                            name: name.to_string(),
                            line: name_node.start_position().row as i64 + 1,
                            shape: RefShape::Member,
                            receiver: None,
                            enclosing: enclosing.clone(),
                        });
                    }
                }
            }
            // `new Type(..)` — construction. The type names the class.
            "object_creation_expression" => {
                if let Some(ty) = n.child_by_field_name("type") {
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
            // A parameter's type hint names a class — a Static use belonging
            // to the declaring method (its scope is in `child_enclosing`).
            "formal_parameter" | "spread_parameter" => {
                if let Some(ty) = n.child_by_field_name("type") {
                    push_type(ty, source, child_enclosing.as_deref(), &mut out);
                }
            }
            "method_declaration" => {
                if let Some(ty) = n.child_by_field_name("type") {
                    push_type(ty, source, child_enclosing.as_deref(), &mut out);
                }
            }
            _ => {}
        }
        let mut c = n.walk();
        for child in n.children(&mut c) {
            stack.push((child, child_enclosing.clone()));
        }
    }
    out
}

/// Emit a Static reference for each named class inside a type node — handles
/// a bare `type_identifier`, a `generic_type` (`List<Foo>` references both
/// `List` and `Foo`), and a `scoped_type_identifier` (`a.b.C` → `C`).
fn push_type(node: Node, source: &[u8], enclosing: Option<&str>, out: &mut Vec<Reference>) {
    let mut stack = vec![node];
    while let Some(n) = stack.pop() {
        match n.kind() {
            "type_identifier" => {
                if let Ok(name) = n.utf8_text(source) {
                    out.push(Reference {
                        name: name.to_string(),
                        line: n.start_position().row as i64 + 1,
                        shape: RefShape::Static,
                        receiver: Some(name.to_string()),
                        enclosing: enclosing.map(|s| s.to_string()),
                    });
                }
            }
            "scoped_type_identifier" => {
                if let Some(name) = last_type_segment(n, source) {
                    out.push(Reference {
                        name: name.clone(),
                        line: n.start_position().row as i64 + 1,
                        shape: RefShape::Static,
                        receiver: Some(name),
                        enclosing: enclosing.map(|s| s.to_string()),
                    });
                }
            }
            _ => {
                let mut c = n.walk();
                for child in n.children(&mut c) {
                    stack.push(child);
                }
            }
        }
    }
}

/// Last `.` segment of a (possibly scoped / generic) type — `a.b.C<T>` → `C`.
fn last_type_segment(node: Node, source: &[u8]) -> Option<String> {
    let txt = node.utf8_text(source).ok()?;
    let base = txt.split('<').next().unwrap_or(txt).trim();
    let seg = base.rsplit('.').next().unwrap_or(base).trim();
    if seg.is_empty() {
        None
    } else {
        Some(seg.to_string())
    }
}

/// A `method_declaration` / `constructor_declaration` introduces its own
/// name as the enclosing method scope.
fn enclosing_function_name(node: Node, source: &[u8]) -> Option<String> {
    if matches!(
        node.kind(),
        "method_declaration" | "constructor_declaration"
    ) {
        return node
            .child_by_field_name("name")
            .and_then(|nm| nm.utf8_text(source).ok())
            .map(|s| s.to_string());
    }
    None
}
