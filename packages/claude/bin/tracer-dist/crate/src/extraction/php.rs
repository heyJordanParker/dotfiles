//! PHP tree-sitter extraction: `use` statements and class/interface/function defs.

use crate::extraction::{Declaration, Export, ExtractionResult, Import, RefShape, Reference};
use std::collections::HashMap;
use tree_sitter::{Node, Parser, Query, QueryCursor, StreamingIterator};

const QUERY_SRC: &str = r#"
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
pub fn extract_from_tree(tree: &tree_sitter::Tree, source: &[u8]) -> ExtractionResult {
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

    let imports = use_imports(tree.root_node(), source);
    let mut exports: Vec<Export> = Vec::new();

    for name in &order {
        for c in &groups[name] {
            match c.name.as_str() {
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

fn use_imports(root: Node, source: &[u8]) -> Vec<Import> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node.kind() == "namespace_use_declaration" {
            let text = node.utf8_text(source).unwrap_or("").trim();
            let text = text
                .strip_prefix("use ")
                .unwrap_or(text)
                .trim_end_matches(';')
                .trim();
            if !(text.starts_with("function ") || text.starts_with("const ")) {
                let line = node.start_position().row as i64 + 1;
                if let Some((prefix, names)) = text.split_once('{') {
                    let prefix = prefix.trim().trim_end_matches('\\');
                    for name in names.trim_end_matches('}').split(',') {
                        push_use_import(
                            &mut out,
                            &format!("{prefix}\\{}", imported_name(name)),
                            line,
                        );
                    }
                } else {
                    for name in text.split(',') {
                        push_use_import(&mut out, imported_name(name), line);
                    }
                }
            }
            continue;
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            stack.push(child);
        }
    }
    out.sort_by_key(|import| import.line);
    out
}

fn imported_name(text: &str) -> &str {
    text.trim().split_whitespace().next().unwrap_or("")
}

fn push_use_import(out: &mut Vec<Import>, name: &str, line: i64) {
    let name = name.replace("\\\\", "\\");
    let segments: Vec<&str> = name.split('\\').collect();
    let (module, symbol) = match segments.split_last() {
        Some((symbol, module)) if !module.is_empty() => {
            (module.join("\\"), Some((*symbol).to_string()))
        }
        _ => (name, None),
    };
    out.push(Import {
        module,
        symbol,
        locals: Vec::new(),
        line,
    });
}

/// Every named declaration: class / interface / trait / enum / function /
/// method, anywhere in the tree (including methods inside classes and
/// functions declared inside other functions). A `method_declaration`
/// carries its enclosing class/interface/trait/enum name as `container`;
/// a `function_definition` and the type declarations themselves have none.
fn walk_declarations(root: Node, source: &[u8]) -> Vec<Declaration> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut stack: Vec<(Node, Option<String>)> = vec![(root, None)];
    while let Some((n, container)) = stack.pop() {
        let kind: Option<&str> = match n.kind() {
            "class_declaration" => Some("class"),
            "interface_declaration" => Some("interface"),
            "trait_declaration" => Some("class"),
            "enum_declaration" => Some("enum"),
            "function_definition" => Some("function"),
            "method_declaration" => Some("function"),
            "property_element" | "property_promotion_parameter" | "enum_case" => Some("property"),
            "const_element" => Some("constant"),
            _ => None,
        };
        let mut child_container = container.clone();
        if let Some(k) = kind {
            let name_node = n
                .child_by_field_name("name")
                .or_else(|| php_variable_name(n))
                .or_else(|| php_constant_name(n));
            if let Some(name_node) = name_node {
                if let Ok(name) = name_node.utf8_text(source) {
                    let line = name_node.start_position().row as i64 + 1;
                    let decl_container = if matches!(
                        n.kind(),
                        "method_declaration"
                            | "property_element"
                            | "property_promotion_parameter"
                            | "enum_case"
                            | "const_element"
                    ) {
                        container.clone()
                    } else {
                        None
                    };
                    // A class/interface/trait/enum scopes its members.
                    if matches!(
                        n.kind(),
                        "class_declaration"
                            | "interface_declaration"
                            | "trait_declaration"
                            | "enum_declaration"
                    ) {
                        child_container = Some(name.to_string());
                    }
                    if seen.insert((name.to_string(), line)) {
                        out.push(Declaration {
                            name: name.to_string(),
                            kind: k.to_string(),
                            header_line: php_header_line(n, source),
                            line,
                            end_line: php_end_line(n),
                            container: decl_container,
                            parent: None,
                            header: php_header(n, source),
                            annotations: php_annotations(n, source),
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
    let parents: std::collections::HashMap<String, u32> = out
        .iter()
        .enumerate()
        .filter(|(_, declaration)| {
            matches!(declaration.kind.as_str(), "class" | "enum" | "interface")
        })
        .map(|(index, declaration)| (declaration.name.clone(), index as u32))
        .collect();
    for declaration in &mut out {
        declaration.parent = declaration
            .container
            .as_ref()
            .and_then(|container| parents.get(container).copied());
    }
    out
}

fn php_header_line(node: Node, source: &[u8]) -> i64 {
    node.prev_named_sibling()
        .filter(|sibling| sibling.kind() == "comment" && php_access_tag(*sibling, source))
        .map(|comment| comment.start_position().row as i64 + 1)
        .unwrap_or_else(|| node.start_position().row as i64 + 1)
}

fn php_end_line(mut node: Node) -> i64 {
    if node.kind() == "property_element" || node.kind() == "const_element" {
        while node.kind() != "property_declaration" && node.kind() != "const_declaration" {
            let Some(parent) = node.parent() else {
                break;
            };
            node = parent;
        }
    }
    node.end_position().row as i64 + 1
}

fn php_header(node: Node, source: &[u8]) -> String {
    if node.kind() == "enum_case" {
        let start = node.start_byte().saturating_sub(4);
        let mut builder = crate::extraction::header::Builder::new(source);
        builder.slice(start, node.end_byte());
        return builder.finish();
    }
    if node.kind() == "property_element" || node.kind() == "const_element" {
        let mut declaration = node;
        while declaration.kind() != "property_declaration"
            && declaration.kind() != "const_declaration"
        {
            let Some(parent) = declaration.parent() else {
                break;
            };
            declaration = parent;
        }
        let mut cursor = declaration.walk();
        let first = declaration
            .named_children(&mut cursor)
            .find(|child| child.kind() == node.kind())
            .unwrap_or(node);
        let mut builder = crate::extraction::header::Builder::new(source);
        builder
            .slice(declaration.start_byte(), first.start_byte())
            .node(node);
        let mut bodies = Vec::new();
        collect_hook_bodies(declaration, &mut bodies);
        bodies.retain(|body| body.start_byte() >= node.end_byte());
        if !bodies.is_empty() {
            bodies.sort_by_key(Node::start_byte);
            let mut start = node.end_byte();
            for body in bodies {
                builder.slice(start, body.start_byte());
                if body.kind() == "compound_statement" {
                    builder.block(body);
                } else {
                    builder.expression(body);
                }
                start = body.end_byte();
            }
            builder.slice(start, declaration.end_byte());
            let header = builder.finish();
            if let Some((before_hooks, hooks)) = header.split_once('{') {
                return format!(
                    "{} {{ {}",
                    before_hooks.trim_end(),
                    hooks.split_whitespace().collect::<Vec<_>>().join(" ")
                );
            }
            return header;
        } else if !source
            .get(node.end_byte().saturating_sub(1)..node.end_byte())
            .is_some_and(|tail| tail == b";")
            && source
                .get(node.end_byte()..declaration.end_byte())
                .is_some_and(|tail| tail.contains(&b';'))
        {
            builder.slice(declaration.end_byte() - 1, declaration.end_byte());
        }
        return builder.finish();
    }
    let body = node.children(&mut node.walk()).find(|child| {
        matches!(
            child.kind(),
            "declaration_list" | "enum_declaration_list" | "compound_statement"
        )
    });
    let mut builder = crate::extraction::header::Builder::new(source);
    if let Some(comment) = node
        .prev_named_sibling()
        .filter(|sibling| sibling.kind() == "comment" && php_access_tag(*sibling, source))
    {
        builder.node(comment);
    }
    if let Some(body) = body {
        builder
            .slice(node.start_byte(), body.start_byte())
            .block(body);
    } else {
        builder.node(node);
    }
    builder.finish()
}

fn collect_hook_bodies<'a>(node: Node<'a>, out: &mut Vec<Node<'a>>) {
    if node.kind() == "property_hook" {
        if let Some(body) = node.child_by_field_name("body") {
            out.push(body);
        }
        return;
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_hook_bodies(child, out);
    }
}

fn php_variable_name(node: Node) -> Option<Node> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "variable_name" {
            return Some(child);
        }
        if child.kind() == "property_element" {
            let mut element_cursor = child.walk();
            let variable = child
                .children(&mut element_cursor)
                .find(|element| element.kind() == "variable_name");
            if let Some(variable) = variable {
                return Some(variable);
            }
        }
    }
    None
}

fn php_constant_name(node: Node) -> Option<Node> {
    if node.kind() != "const_element" {
        return None;
    }
    let mut cursor = node.walk();
    let name = node
        .named_children(&mut cursor)
        .find(|child| child.kind() == "name");
    name
}

fn php_annotations(mut node: Node, source: &[u8]) -> Vec<String> {
    if matches!(node.kind(), "property_element" | "const_element") {
        if let Some(parent) = node.parent() {
            node = parent;
        }
    }
    let mut annotations = Vec::new();
    if let Some(comment) = node
        .prev_named_sibling()
        .filter(|sibling| sibling.kind() == "comment")
    {
        if let Ok(text) = comment.utf8_text(source) {
            for line in text.lines().map(str::trim) {
                let source = line
                    .trim_start_matches('/')
                    .trim_start_matches('*')
                    .trim()
                    .trim_end_matches("*/")
                    .trim();
                if let Some(tag) = ["@internal", "@deprecated", "@api"]
                    .into_iter()
                    .find(|tag| {
                        source
                            .strip_prefix(*tag)
                            .is_some_and(|rest| rest.chars().next().is_none_or(char::is_whitespace))
                    })
                {
                    annotations.push(tag.to_string());
                }
            }
        }
    }
    let mut cursor = node.walk();
    for list in node
        .children(&mut cursor)
        .filter(|child| child.kind() == "attribute_list")
    {
        let mut stack = vec![list];
        while let Some(current) = stack.pop() {
            if current.kind() == "attribute" {
                let mut name_cursor = current.walk();
                let name = current
                    .children(&mut name_cursor)
                    .find(|child| matches!(child.kind(), "name" | "qualified_name"))
                    .and_then(|child| child.utf8_text(source).ok())
                    .map(|name| name.rsplit('\\').next().unwrap_or(name).to_string())
                    .unwrap_or_default();
                annotations.push(name);
                continue;
            }
            let mut cursor = current.walk();
            for child in current.children(&mut cursor) {
                stack.push(child);
            }
        }
    }
    annotations
}

fn php_access_tag(comment: Node, source: &[u8]) -> bool {
    comment.utf8_text(source).ok().is_some_and(|text| {
        ["@internal", "@deprecated", "@api"]
            .into_iter()
            .any(|tag| text.contains(tag))
    })
}

/// Every function / method / static-method / object-method call plus the
/// idioms that name a class symbol without calling it: `::class`,
/// `instanceof`, parameter type hints (covers constructor injection),
/// return type hints, and property type declarations.
fn walk_references(root: Node, source: &[u8]) -> Vec<Reference> {
    let mut out = Vec::new();
    // Each stack entry carries the name of the nearest enclosing function /
    // method / anonymous-function — the calling symbol the use site belongs
    // to. A function-shaped node sets the enclosing for its subtree (its own
    // parameters and body), so a parameter type hint resolves to the
    // declaring function. `None` is module top level.
    let mut stack: Vec<(Node, Option<String>)> = vec![(root, None)];
    while let Some((n, enclosing)) = stack.pop() {
        let line = n.start_position().row as i64 + 1;
        // The enclosing scope for THIS node's children. A function-shaped
        // node names its own scope; its return type and body are emitted
        // under that name, not the parent's.
        let child_enclosing = if matches!(
            n.kind(),
            "function_definition"
                | "method_declaration"
                | "anonymous_function"
                | "anonymous_function_creation_expression"
                | "arrow_function"
        ) {
            n.child_by_field_name("name")
                .and_then(|nm| nm.utf8_text(source).ok())
                .map(|s| s.to_string())
                .or_else(|| enclosing.clone())
        } else {
            enclosing.clone()
        };
        match n.kind() {
            "function_call_expression" => {
                if let Some(func) = n.child_by_field_name("function") {
                    if let Some(name) = last_name_segment(func, source) {
                        out.push(Reference {
                            name,
                            line,
                            shape: RefShape::Free,
                            receiver: None,
                            enclosing: enclosing.clone(),
                        });
                    }
                }
            }
            // `$x->foo()` and `$x?->foo()` — a method call on an object value.
            // The receiver is a variable/expression, never a class name, so
            // it carries no class to disambiguate against.
            "member_call_expression" | "nullsafe_member_call_expression" => {
                if let Some(method) = n.child_by_field_name("name") {
                    if let Ok(name) = method.utf8_text(source) {
                        out.push(Reference {
                            name: name.to_string(),
                            line: method.start_position().row as i64 + 1,
                            shape: RefShape::Member,
                            receiver: None,
                            enclosing: enclosing.clone(),
                        });
                    }
                }
            }
            // `Foo::bar()` — a static call. The scope names the class, so
            // the receiver is that class name.
            "scoped_call_expression" => {
                let scope_class = n
                    .child_by_field_name("scope")
                    .and_then(|s| last_name_segment(s, source));
                if let Some(method) = n.child_by_field_name("name") {
                    if let Ok(name) = method.utf8_text(source) {
                        out.push(Reference {
                            name: name.to_string(),
                            line: method.start_position().row as i64 + 1,
                            shape: RefShape::Static,
                            receiver: scope_class.clone(),
                            enclosing: enclosing.clone(),
                        });
                    }
                }
                // `Foo::class` surfaced as a scoped_call_expression in some
                // grammars — the class name is the scope.
                if let Some(name_node) = n.child_by_field_name("name") {
                    if name_node.utf8_text(source).ok() == Some("class") {
                        if let Some(name) = scope_class {
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
            "class_constant_access_expression" => {
                // `Foo::CONST` and `Foo::class`. The scope is the class
                // identifier — a Static use that names the class.
                let mut c = n.walk();
                let mut children: Vec<Node> = n.children(&mut c).collect();
                if let Some(first) = children.first_mut() {
                    if matches!(first.kind(), "name" | "qualified_name") {
                        if let Some(name) = last_name_segment(*first, source) {
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
            "object_creation_expression" => {
                // `new Foo(...)` — the class name is the constructor target,
                // a Static use that names the class.
                let mut c = n.walk();
                for child in n.children(&mut c) {
                    if matches!(child.kind(), "name" | "qualified_name") {
                        if let Some(name) = last_name_segment(child, source) {
                            out.push(Reference {
                                name: name.clone(),
                                line,
                                shape: RefShape::Static,
                                receiver: Some(name),
                                enclosing: enclosing.clone(),
                            });
                        }
                        break;
                    }
                }
            }
            "binary_expression" => {
                // `$x instanceof Foo` — the right operand names the class.
                if let Some(op) = n.child_by_field_name("operator") {
                    if op.utf8_text(source).ok() == Some("instanceof") {
                        if let Some(right) = n.child_by_field_name("right") {
                            if let Some(name) = type_name(right, source) {
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
            }
            // Type hints carry class names: parameter types (covers
            // constructor injection), return types, property types. A
            // parameter sits inside the declaring function, so its type use
            // belongs to that function (`child_enclosing` of the function is
            // already in scope here as `enclosing`).
            "simple_parameter" | "variadic_parameter" | "property_promotion_parameter" => {
                if let Some(type_node) = n.child_by_field_name("type") {
                    push_named_type(type_node, source, enclosing.as_deref(), &mut out);
                }
            }
            "function_definition"
            | "method_declaration"
            | "anonymous_function"
            | "anonymous_function_creation_expression"
            | "arrow_function" => {
                if let Some(ret) = n.child_by_field_name("return_type") {
                    // The return type belongs to the function being declared.
                    push_named_type(ret, source, child_enclosing.as_deref(), &mut out);
                }
            }
            "property_declaration" => {
                let mut c = n.walk();
                for child in n.children(&mut c) {
                    if matches!(
                        child.kind(),
                        "named_type"
                            | "primitive_type"
                            | "union_type"
                            | "nullable_type"
                            | "intersection_type"
                            | "optional_type"
                    ) {
                        push_named_type(child, source, enclosing.as_deref(), &mut out);
                    }
                }
            }
            // `#[Foo(...)]` names the attribute class without importing it
            // when both sit in one namespace, so the class stays reachable
            // through the site rather than through a `use` line. A method's
            // attribute_list sits inside the method node, so `enclosing` is
            // already that method.
            "attribute" => {
                let mut c = n.walk();
                for child in n.children(&mut c) {
                    if matches!(child.kind(), "name" | "qualified_name") {
                        push_named_type(child, source, enclosing.as_deref(), &mut out);
                        break;
                    }
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

/// Recursively emit references for every `name`/`qualified_name` found
/// inside a type node — handles `named_type`, `nullable_type`, `union_type`,
/// `intersection_type`, and `optional_type`. A type hint names a class, so
/// each is a Static use whose receiver is the class itself.
fn push_named_type(node: Node, source: &[u8], enclosing: Option<&str>, out: &mut Vec<Reference>) {
    let mut stack = vec![node];
    while let Some(n) = stack.pop() {
        match n.kind() {
            "name" | "qualified_name" => {
                if let Some(name) = last_name_segment(n, source) {
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
