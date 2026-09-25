//! Rust tree-sitter extraction: `use` imports, item declarations, and call
//! / construction / type-hint references.
//!
//! Declarations cover free functions (`function_item`), methods (a
//! `function_item` inside an `impl_item`, whose `container` is the impl'd
//! type), and the type items (`struct_item` / `enum_item` / `trait_item`).
//! References cover free calls (`name(...)`), method calls (`x.name(...)`),
//! associated / path calls (`Type::name(...)`), struct construction
//! (`Type { .. }` / `Type(..)` as a path call) and macro invocations.
//! Imports are the leaf names brought in by `use` paths.

use crate::extraction::{Declaration, Export, ExtractionResult, Import, RefShape, Reference};
use tree_sitter::{Node, Parser};

fn empty() -> ExtractionResult {
    ExtractionResult {
        language: "rust".into(),
        imports: vec![],
        exports: vec![],
        declarations: vec![],
        references: vec![],
    }
}

pub fn extract(source: &[u8]) -> ExtractionResult {
    let lang: tree_sitter::Language = tree_sitter_rust::LANGUAGE.into();
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

/// Imports/exports/declarations/references from an already-parsed Rust tree.
/// Decoupled from the parse so `file_facts` shares one tree with `ccn`.
/// Caller guarantees `tree` came from the Rust grammar.
pub fn extract_from_tree(tree: &tree_sitter::Tree, source: &[u8]) -> ExtractionResult {
    let root = tree.root_node();
    let imports = walk_imports(root, source);
    let declarations = walk_declarations(root, source);
    // Exports are the public, module-level items — the narrower set the
    // structure/module-API views want. A `pub` item is exported.
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
        language: "rust".into(),
        imports,
        exports,
        declarations,
        references,
    }
}

/// Every leaf name a `use` declaration brings into scope. `use a::b::C;`
/// imports `C` from module `a::b`; `use a::b::{C, D};` imports both. The
/// last path segment is the symbol; the prefix is the module.
fn walk_imports(root: Node, source: &[u8]) -> Vec<Import> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        if n.kind() == "use_declaration" {
            if let Some(arg) = n.child_by_field_name("argument") {
                collect_use(
                    arg,
                    source,
                    &[],
                    n.start_position().row as i64 + 1,
                    &mut out,
                );
            }
        }
        let mut c = n.walk();
        for child in n.children(&mut c) {
            stack.push(child);
        }
    }
    out
}

/// Resolve a `use` tree into leaf imports. `prefix` is the path accumulated
/// so far (module segments). `scoped_identifier` is `path::name`,
/// `scoped_use_list` is `path::{a, b}`, `use_as_clause` renames, and a bare
/// `identifier` is the leaf.
fn collect_use(node: Node, source: &[u8], prefix: &[String], line: i64, out: &mut Vec<Import>) {
    match node.kind() {
        "scoped_identifier" => {
            let path = node.child_by_field_name("path");
            let name = node.child_by_field_name("name");
            let mut next = prefix.to_vec();
            if let Some(p) = path {
                if let Ok(t) = p.utf8_text(source) {
                    next = t.split("::").map(|s| s.to_string()).collect();
                }
            }
            if let Some(nm) = name {
                if let Ok(t) = nm.utf8_text(source) {
                    out.push(Import {
                        module: next.join("::"),
                        symbol: Some(t.to_string()),
                        locals: Vec::new(),
                        line,
                    });
                }
            }
        }
        "scoped_use_list" => {
            let path = node.child_by_field_name("path");
            let mut next = prefix.to_vec();
            if let Some(p) = path {
                if let Ok(t) = p.utf8_text(source) {
                    next = t.split("::").map(|s| s.to_string()).collect();
                }
            }
            if let Some(list) = node.child_by_field_name("list") {
                let mut c = list.walk();
                for child in list.children(&mut c) {
                    if matches!(
                        child.kind(),
                        "scoped_identifier" | "scoped_use_list" | "use_as_clause" | "identifier"
                    ) {
                        collect_use(child, source, &next, line, out);
                    }
                }
            }
        }
        "use_as_clause" => {
            if let Some(path) = node.child_by_field_name("path") {
                collect_use(path, source, prefix, line, out);
            }
        }
        "use_list" => {
            let mut c = node.walk();
            for child in node.children(&mut c) {
                if matches!(
                    child.kind(),
                    "scoped_identifier" | "scoped_use_list" | "use_as_clause" | "identifier"
                ) {
                    collect_use(child, source, prefix, line, out);
                }
            }
        }
        "identifier" => {
            if let Ok(t) = node.utf8_text(source) {
                out.push(Import {
                    module: prefix.join("::"),
                    symbol: Some(t.to_string()),
                    locals: Vec::new(),
                    line,
                });
            }
        }
        _ => {}
    }
}

/// Every named item: free functions, methods (a `function_item` inside an
/// `impl_item`, carrying the impl'd type as `container`), structs, enums,
/// and traits. A method inside a `trait_item` carries the trait name as its
/// container. Source-order, deduped on (name, line).
fn walk_declarations(root: Node, source: &[u8]) -> Vec<Declaration> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut stack: Vec<(Node, Option<String>)> = vec![(root, None)];
    while let Some((n, container)) = stack.pop() {
        let kind: Option<&str> = match n.kind() {
            "function_item" => Some("function"),
            "function_signature_item" => Some("function"),
            "struct_item" => Some("class"),
            "enum_item" => Some("enum"),
            "trait_item" => Some("interface"),
            "union_item" => Some("class"),
            "impl_item" => Some("impl"),
            "mod_item" => Some("module"),
            "type_item" => Some("type"),
            "const_item" | "static_item" => Some("constant"),
            "field_declaration" | "enum_variant" => Some("property"),
            _ => None,
        };
        let mut child_container = container.clone();
        // An `impl` / `trait` body scopes the functions declared inside it
        // to its type — that type becomes the container of those methods.
        match n.kind() {
            "impl_item" => {
                child_container = impl_type_name(n, source).or_else(|| container.clone());
            }
            "trait_item" => {
                child_container = n
                    .child_by_field_name("name")
                    .and_then(|nm| nm.utf8_text(source).ok())
                    .map(|s| s.to_string())
                    .or_else(|| container.clone());
            }
            _ => {}
        }
        if let Some(k) = kind {
            if let Some(name_node) = n.child_by_field_name("name").or_else(|| {
                if n.kind() == "impl_item" {
                    n.child_by_field_name("type")
                } else {
                    None
                }
            }) {
                if let Ok(name) = name_node.utf8_text(source) {
                    let line = name_node.start_position().row as i64 + 1;
                    let mut header_start = n.start_byte();
                    let mut previous = n.prev_sibling();
                    while let Some(attribute) = previous {
                        if attribute.kind() != "attribute_item" {
                            break;
                        }
                        header_start = attribute.start_byte();
                        previous = attribute.prev_sibling();
                    }
                    // A function is a method when its container is set (it
                    // sits inside an impl/trait body); a type item is never
                    // contained.
                    let decl_container = if matches!(
                        n.kind(),
                        "function_item"
                            | "function_signature_item"
                            | "field_declaration"
                            | "enum_variant"
                    ) {
                        container.clone()
                    } else {
                        None
                    };
                    if seen.insert((name.to_string(), line)) {
                        out.push(Declaration {
                            name: name.to_string(),
                            kind: k.to_string(),
                            header_line: source[..header_start]
                                .iter()
                                .filter(|byte| **byte == b'\n')
                                .count() as i64
                                + 1,
                            line,
                            end_line: n.end_position().row as i64 + 1,
                            container: decl_container,
                            parent: None,
                            header: {
                                let mut builder = crate::extraction::header::Builder::new(source);
                                if header_start != n.start_byte() {
                                    builder.slice(header_start, n.start_byte());
                                }
                                let mut cursor = n.walk();
                                // A struct-like variant's fields are rows of
                                // their own; a tuple variant's types are its
                                // whole shape.
                                let variant_fields = n
                                    .child_by_field_name("body")
                                    .filter(|body| body.kind() == "field_declaration_list");
                                if let Some(fields) = variant_fields.filter(|_| n.kind() == "enum_variant") {
                                    builder.slice(n.start_byte(), fields.start_byte()).block(fields);
                                } else if n.kind() != "enum_variant" {
                                    if let Some(body) =
                                        n.child_by_field_name("body").or_else(|| {
                                            n.named_children(&mut cursor).find(|child| {
                                                matches!(
                                                    child.kind(),
                                                    "declaration_list"
                                                        | "field_declaration_list"
                                                        | "enum_variant_list"
                                                        | "block"
                                                )
                                            })
                                        })
                                    {
                                        builder
                                            .slice(n.start_byte(), body.start_byte())
                                            .block(body);
                                    } else {
                                        builder.node(n);
                                    }
                                } else {
                                    builder.node(n);
                                }
                                builder.finish()
                            },
                            annotations: Vec::new(),
                        });
                    }
                }
            }
        }
        // Pushed in reverse so the walk meets declarations in source order:
        // two on one line (a variant's fields) keep that order after the
        // stable sort by line.
        let mut c = n.walk();
        let children: Vec<Node> = n.children(&mut c).collect();
        for child in children.into_iter().rev() {
            stack.push((child, child_container.clone()));
        }
    }
    out.sort_by_key(|d| d.line);
    for index in 0..out.len() {
        if let Some((parent, _)) = out[..index]
            .iter()
            .enumerate()
            .rev()
            .find(|(_, candidate)| {
                // A struct-like variant holds its fields, often on its own line.
                let variant = candidate.kind == "property" && candidate.header.ends_with("{ … }");
                (matches!(
                    candidate.kind.as_str(),
                    "impl" | "class" | "enum" | "interface"
                ) && candidate.line < out[index].line
                    || variant && candidate.line <= out[index].line)
                    && candidate.end_line >= out[index].line
            })
        {
            out[index].parent = Some(parent as u32);
        }
    }
    out
}

/// The implemented type name of an `impl_item` — `impl Foo { .. }` or
/// `impl Trait for Foo { .. }` both have type field `Foo`. The last path
/// segment is the bare type.
fn impl_type_name(n: Node, source: &[u8]) -> Option<String> {
    let ty = n.child_by_field_name("type")?;
    last_segment(ty, source)
}

/// The bare last segment of a type / path node (`a::b::C` → `C`, `C` → `C`).
fn last_segment(node: Node, source: &[u8]) -> Option<String> {
    let txt = node.utf8_text(source).ok()?;
    // Strip generic args, then take the last `::` segment.
    let base = txt.split('<').next().unwrap_or(txt).trim();
    let seg = base.rsplit("::").next().unwrap_or(base).trim();
    if seg.is_empty() {
        None
    } else {
        Some(seg.to_string())
    }
}

/// Every call / construction / macro / type reference, stamped with shape
/// and enclosing function. Free calls `name(..)` are `Free`; method calls
/// `x.name(..)` are `Member`; associated calls `Type::name(..)` and struct
/// literals `Type { .. }` are `Static` (the type is the receiver).
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
            // `Type { field: .. }` — struct construction. The type names the
            // class, a Static use.
            "struct_expression" => {
                if let Some(ty) = n.child_by_field_name("name") {
                    if let Some(name) = last_segment(ty, source) {
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
            // `name!(..)` — a macro invocation references the macro by name.
            "macro_invocation" => {
                if let Some(m) = n.child_by_field_name("macro") {
                    if let Some(name) = last_segment(m, source) {
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

/// Name / line / shape / receiver for a `call_expression`'s function node.
/// A bare `identifier` is a `Free` call. A `field_expression` `x.name` is a
/// `Member` call (receiver is a value, type unknown). A `scoped_identifier`
/// `Type::name` is a `Static` call whose receiver is the named type.
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
        "field_expression" => {
            let field = node.child_by_field_name("field")?;
            let txt = field.utf8_text(source).ok()?;
            Some((
                txt.to_string(),
                field.start_position().row as i64 + 1,
                RefShape::Member,
                None,
            ))
        }
        "scoped_identifier" => {
            let name = node.child_by_field_name("name")?;
            let txt = name.utf8_text(source).ok()?;
            let receiver = node
                .child_by_field_name("path")
                .and_then(|p| last_segment(p, source));
            Some((
                txt.to_string(),
                name.start_position().row as i64 + 1,
                RefShape::Static,
                receiver,
            ))
        }
        _ => None,
    }
}

/// The declared function name a node introduces as a new enclosing scope —
/// a `function_item` (named directly). Other nodes introduce no new function
/// scope.
fn enclosing_function_name(node: Node, source: &[u8]) -> Option<String> {
    if matches!(node.kind(), "function_item" | "function_signature_item") {
        return node
            .child_by_field_name("name")
            .and_then(|nm| nm.utf8_text(source).ok())
            .map(|s| s.to_string());
    }
    None
}
