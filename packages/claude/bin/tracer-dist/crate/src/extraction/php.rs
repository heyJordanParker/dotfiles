//! PHP tree-sitter extraction: `use` statements and class/interface/function defs.

use crate::extraction::{Declaration, Export, ExtractionResult, Import, Reference};
use std::collections::HashMap;
use tree_sitter::{Node, Parser, Query, QueryCursor, StreamingIterator};

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
        declarations: vec![],
        references: vec![],
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

    let declarations = walk_declarations(tree.root_node(), source);
    let references = walk_references(tree.root_node(), source);

    ExtractionResult {
        language: "php".into(),
        imports,
        exports,
        declarations,
        references,
    }
}

/// Every named declaration: class / interface / trait / enum / function /
/// method, anywhere in the tree (including methods inside classes and
/// functions declared inside other functions).
fn walk_declarations(root: Node, source: &[u8]) -> Vec<Declaration> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        let kind: Option<&str> = match n.kind() {
            "class_declaration" => Some("class"),
            "interface_declaration" => Some("interface"),
            "trait_declaration" => Some("class"),
            "enum_declaration" => Some("class"),
            "function_definition" => Some("function"),
            "method_declaration" => Some("function"),
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

/// Every function / method / static-method / object-method call plus the
/// idioms that name a class symbol without calling it: `::class`,
/// `instanceof`, parameter type hints (covers constructor injection),
/// return type hints, and property type declarations.
fn walk_references(root: Node, source: &[u8]) -> Vec<Reference> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        let line = n.start_position().row as i64 + 1;
        match n.kind() {
            "function_call_expression" => {
                if let Some(func) = n.child_by_field_name("function") {
                    if let Some(name) = last_name_segment(func, source) {
                        out.push(Reference { name, line });
                    }
                }
            }
            "member_call_expression" | "scoped_call_expression" => {
                if let Some(method) = n.child_by_field_name("name") {
                    if let Ok(name) = method.utf8_text(source) {
                        out.push(Reference {
                            name: name.to_string(),
                            line: method.start_position().row as i64 + 1,
                        });
                    }
                }
                // `Foo::class` (class_constant_access_expression with a
                // `class` keyword as the constant name) — the class name is
                // the scope target. tree-sitter-php surfaces this same
                // production as `scoped_call_expression` in some grammars,
                // so we also pick the scope identifier up here when the
                // child name is the literal `class`.
                if let Some(scope) = n.child_by_field_name("scope") {
                    if let Some(name_node) = n.child_by_field_name("name") {
                        if name_node.utf8_text(source).ok() == Some("class") {
                            if let Some(name) = last_name_segment(scope, source) {
                                out.push(Reference { name, line });
                            }
                        }
                    }
                }
            }
            "class_constant_access_expression" => {
                // `Foo::CONST` and `Foo::class`. The scope is the class
                // identifier — emit a reference to it either way so a
                // class-name query catches the use site.
                let mut c = n.walk();
                let mut children: Vec<Node> = n.children(&mut c).collect();
                if let Some(first) = children.first_mut() {
                    if matches!(first.kind(), "name" | "qualified_name") {
                        if let Some(name) = last_name_segment(*first, source) {
                            out.push(Reference { name, line });
                        }
                    }
                }
            }
            "object_creation_expression" => {
                // `new Foo(...)` — the class name is the constructor target.
                let mut c = n.walk();
                for child in n.children(&mut c) {
                    if matches!(child.kind(), "name" | "qualified_name") {
                        if let Some(name) = last_name_segment(child, source) {
                            out.push(Reference { name, line });
                        }
                        break;
                    }
                }
            }
            "binary_expression" => {
                // `$x instanceof Foo` — tree-sitter-php models instanceof
                // as a binary_expression with the `instanceof` operator;
                // the right operand is the class name.
                if let Some(op) = n.child_by_field_name("operator") {
                    if op.utf8_text(source).ok() == Some("instanceof") {
                        if let Some(right) = n.child_by_field_name("right") {
                            if let Some(name) = type_name(right, source) {
                                out.push(Reference { name, line });
                            }
                        }
                    }
                }
            }
            // Type hints carry class names: parameter types (covers
            // constructor injection), return types, property types.
            "simple_parameter"
            | "variadic_parameter"
            | "property_promotion_parameter" => {
                if let Some(type_node) = n.child_by_field_name("type") {
                    push_named_type(type_node, source, &mut out);
                }
            }
            "function_definition" | "method_declaration" => {
                if let Some(ret) = n.child_by_field_name("return_type") {
                    push_named_type(ret, source, &mut out);
                }
            }
            "property_declaration" => {
                let mut c = n.walk();
                for child in n.children(&mut c) {
                    if matches!(
                        child.kind(),
                        "named_type" | "primitive_type" | "union_type" | "nullable_type" | "intersection_type" | "optional_type"
                    ) {
                        push_named_type(child, source, &mut out);
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
    out
}

/// Recursively emit references for every `name`/`qualified_name` found
/// inside a type node — handles `named_type`, `nullable_type`, `union_type`,
/// `intersection_type`, and `optional_type`.
fn push_named_type(node: Node, source: &[u8], out: &mut Vec<Reference>) {
    let mut stack = vec![node];
    while let Some(n) = stack.pop() {
        match n.kind() {
            "name" | "qualified_name" => {
                if let Some(name) = last_name_segment(n, source) {
                    out.push(Reference {
                        name,
                        line: n.start_position().row as i64 + 1,
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

/// Read a class-name out of an arbitrary RHS node (used for `instanceof`):
/// drill into the first `name`/`qualified_name` descendant.
fn type_name(node: Node, source: &[u8]) -> Option<String> {
    let mut stack = vec![node];
    while let Some(n) = stack.pop() {
        match n.kind() {
            "name" | "qualified_name" => return last_name_segment(n, source),
            _ => {
                let mut c = n.walk();
                for child in n.children(&mut c) {
                    stack.push(child);
                }
            }
        }
    }
    None
}

fn last_name_segment(node: Node, source: &[u8]) -> Option<String> {
    let txt = node.utf8_text(source).ok()?;
    let normalized = txt.replace("\\\\", "\\");
    let seg = normalized.rsplit('\\').next().unwrap_or(&normalized);
    Some(seg.to_string())
}
