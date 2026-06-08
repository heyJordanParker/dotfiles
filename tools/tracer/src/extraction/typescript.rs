//! TypeScript/TSX/JS/JSX tree-sitter extraction: imports and exports.
//!
//! Note: tree-sitter 0.25's `QueryCursor::matches` does NOT auto-apply
//! `#eq?` text predicates, so the `(#eq? @_fn "require")` predicate is
//! enforced manually below.

use crate::extraction::{Declaration, Export, ExtractionResult, Import, Reference, RefShape};
use std::collections::HashMap;
use tree_sitter::{Node, Parser, Query, QueryCursor, StreamingIterator};

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
        declarations: vec![],
        references: vec![],
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

    let declarations = walk_declarations(tree.root_node(), source);
    let references = walk_references(tree.root_node(), source);

    ExtractionResult {
        language: "typescript".into(),
        imports,
        exports,
        declarations,
        references,
    }
}

/// Every named declaration in the tree — top-level, nested, and class
/// members. Covers function/class/interface/type/enum/method definitions
/// plus `const`/`let`/`var` declarators (with single-identifier names). A
/// `method_definition` carries the enclosing class name as its `container`;
/// every other declaration's container is `None`.
fn walk_declarations(root: Node, source: &[u8]) -> Vec<Declaration> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    // Stack carries (node, enclosing-class-name) so a method picks up the
    // class it is declared inside without a second ancestor walk.
    let mut stack: Vec<(Node, Option<String>)> = vec![(root, None)];
    while let Some((n, container)) = stack.pop() {
        let kind: Option<&str> = match n.kind() {
            "function_declaration" => Some("function"),
            "class_declaration" => Some("class"),
            "interface_declaration" => Some("interface"),
            "type_alias_declaration" => Some("type"),
            "enum_declaration" => Some("enum"),
            "method_definition" => Some("function"),
            "method_signature" => Some("function"),
            "abstract_method_signature" => Some("function"),
            "variable_declarator" => Some("constant"),
            _ => None,
        };
        // The class name this node's children are declared inside. A class
        // node sets the container for everything below it.
        let mut child_container = container.clone();
        if let Some(k) = kind {
            if let Some(name_node) = n.child_by_field_name("name") {
                let name_kind = name_node.kind();
                if name_kind == "identifier"
                    || name_kind == "property_identifier"
                    || name_kind == "type_identifier"
                {
                    if let Ok(name) = name_node.utf8_text(source) {
                        let line = name_node.start_position().row as i64 + 1;
                        // A method's container is the enclosing class; every
                        // other declaration is a top-level / free symbol.
                        let decl_container =
                            if k == "function" && n.kind() == "method_definition" {
                                container.clone()
                            } else {
                                None
                            };
                        if k == "class" {
                            child_container = Some(name.to_string());
                        }
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
        }
        let mut c = n.walk();
        for child in n.children(&mut c) {
            stack.push((child, child_container.clone()));
        }
    }
    out.sort_by_key(|d| d.line);
    out
}

/// Every `call_expression` and `new_expression` in the tree, stamped with
/// its call shape: a bare `foo()` is `Free`, an `o.foo()` is `Member` (the
/// object text becomes the receiver), and `new Foo()` is `Static` (the
/// class name is the receiver).
fn walk_references(root: Node, source: &[u8]) -> Vec<Reference> {
    let mut out = Vec::new();
    // Each stack entry carries the name of the nearest enclosing declared
    // function symbol — the calling symbol the use site belongs to. The
    // declaration index resolves three function-bearing shapes: a
    // `function_declaration` / `method_definition` (named directly) and a
    // `variable_declarator` whose value is an arrow/function (named by the
    // declarator). Setting the enclosing for exactly those keeps the edge
    // source pointing at a node the declaration index actually created.
    // `None` is module top level.
    let mut stack: Vec<(Node, Option<String>)> = vec![(root, None)];
    while let Some((n, enclosing)) = stack.pop() {
        if n.kind() == "new_expression" {
            let ctor = n
                .child_by_field_name("constructor")
                .or_else(|| n.child_by_field_name("function"));
            if let Some(ctor) = ctor {
                if let Some((name, line)) = callee_name(ctor, source) {
                    out.push(Reference {
                        name: name.clone(),
                        line,
                        shape: RefShape::Static,
                        receiver: Some(name),
                        enclosing: enclosing.clone(),
                    });
                }
            }
        } else if n.kind() == "call_expression" {
            if let Some(func) = n.child_by_field_name("function") {
                if let Some((name, line, shape, receiver)) = call_shape(func, source) {
                    out.push(Reference {
                        name,
                        line,
                        shape,
                        receiver,
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

/// The declared function-symbol name a node introduces as a new enclosing
/// scope, or `None` if the node is not a function-bearing declaration. A
/// `function_declaration` / `method_definition` names its own scope; a
/// `variable_declarator` whose value is an arrow / function expression names
/// the scope by the declarator (the arrow-const form the declaration index
/// records). Other nodes introduce no new function scope.
fn enclosing_function_name(node: Node, source: &[u8]) -> Option<String> {
    match node.kind() {
        "function_declaration" | "method_definition" => node
            .child_by_field_name("name")
            .and_then(|nm| nm.utf8_text(source).ok())
            .map(|s| s.to_string()),
        "variable_declarator" => {
            let value = node.child_by_field_name("value")?;
            if matches!(
                value.kind(),
                "arrow_function" | "function_expression" | "function"
            ) {
                node.child_by_field_name("name")
                    .filter(|nm| nm.kind() == "identifier")
                    .and_then(|nm| nm.utf8_text(source).ok())
                    .map(|s| s.to_string())
            } else {
                None
            }
        }
        _ => None,
    }
}

/// The callee name + line for a `new_expression` constructor node.
fn callee_name(node: Node, source: &[u8]) -> Option<(String, i64)> {
    match node.kind() {
        "identifier" | "type_identifier" => {
            let txt = node.utf8_text(source).ok()?;
            Some((txt.to_string(), node.start_position().row as i64 + 1))
        }
        "member_expression" => {
            let prop = node.child_by_field_name("property")?;
            let txt = prop.utf8_text(source).ok()?;
            Some((txt.to_string(), prop.start_position().row as i64 + 1))
        }
        _ => None,
    }
}

/// Name, line, shape, and receiver for a `call_expression`'s function node.
/// A bare identifier is a `Free` call; a `member_expression` is a `Member`
/// call whose receiver is the object text (`this`, a variable, or a longer
/// expression) — the receiver names a value, never a type, so it does not
/// disambiguate the method's class.
fn call_shape(node: Node, source: &[u8]) -> Option<(String, i64, RefShape, Option<String>)> {
    match node.kind() {
        "identifier" | "type_identifier" => {
            let txt = node.utf8_text(source).ok()?;
            Some((
                txt.to_string(),
                node.start_position().row as i64 + 1,
                RefShape::Free,
                None,
            ))
        }
        "member_expression" => {
            let prop = node.child_by_field_name("property")?;
            let txt = prop.utf8_text(source).ok()?;
            let receiver = node
                .child_by_field_name("object")
                .and_then(|o| o.utf8_text(source).ok())
                .map(|s| s.to_string());
            Some((
                txt.to_string(),
                prop.start_position().row as i64 + 1,
                RefShape::Member,
                receiver,
            ))
        }
        _ => None,
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
