//! Rust tree-sitter extraction: `use` imports, item declarations, and call
//! / construction / type-hint references.
//!
//! Declarations cover free functions (`function_item`), methods (a
//! `function_item` inside an `impl_item`, whose `container` is the impl'd
//! type), and the type items (`struct_item` / `enum_item` / `trait_item`).
//! References cover free calls (`name(...)`), method calls (`x.name(...)`),
//! associated / path calls (`Type::name(...)`), struct construction
//! (`Type { .. }` / `Type(..)` as a path call), macro invocations, and the
//! calls inside a function-like macro's arguments.
//! Imports are the leaf names brought in by `use` paths and the paths `type`
//! aliases name.
//!
//! A path is written the way the file's own module reads it: `Self` is the
//! enclosing impl's type, a `super` inside an inline `mod` and a path into an
//! inline `mod` stay in this file as `self`, and a `use` alias is the path it
//! stands for inside the block that holds its `use`, so an aliased call is the
//! path call it names.

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
    parse(source).map_or_else(empty, |tree| extract_from_tree(&tree, source))
}

fn parse(source: &[u8]) -> Option<tree_sitter::Tree> {
    let mut parser = Parser::new();
    parser.set_language(&tree_sitter_rust::LANGUAGE.into()).ok()?;
    parser.parse(source, None)
}

/// Imports/exports/declarations/references from an already-parsed Rust tree.
/// Decoupled from the parse so `file_facts` shares one tree with `ccn`.
/// Caller guarantees `tree` came from the Rust grammar.
pub fn extract_from_tree(tree: &tree_sitter::Tree, source: &[u8]) -> ExtractionResult {
    let root = tree.root_node();
    let imports = walk_imports(root, source);
    let declarations = walk_declarations(root, source, &imports);
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
    let references = walk_references(Site::of(root, source), &imports);
    ExtractionResult {
        language: "rust".into(),
        imports,
        exports,
        declarations,
        references,
    }
}

/// Every name a `use` declaration binds. `use a::b::C;` imports `C` from
/// module `a::b`, `use a::b::{self, C as D};` imports module `b` from `a`
/// and `C` bound as `D` (the alias is the import's one local), and
/// `use a::b::*;` imports module `a::b` with no symbol. A path into an
/// inline `mod` of this file, `use outer::inner;`, imports `self` bound by
/// its alias, else by the module name it was written with. A `type Name =
/// path;` outside an `impl` or a trait imports `path` bound as `Name`. A `use`
/// or `type` inside a `mod` body or a block carries that body's lines as its
/// block.
fn walk_imports(root: Node, source: &[u8]) -> Vec<Import> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        let block = n
            .parent()
            .filter(|holder| holder.kind() != "source_file")
            .map(|holder| (holder.start_position().row as i64 + 1, holder.end_position().row as i64 + 1));
        let line = n.start_position().row as i64 + 1;
        match n.kind() {
            "use_declaration" => {
                if let Some(argument) = n.child_by_field_name("argument") {
                    let mut leaves = Vec::new();
                    collect_use(argument, source, &[], &mut leaves);
                    for (path, alias) in leaves {
                        let written = path.iter().rev().find(|segment| *segment != "self").cloned();
                        let mut module = file_level(path, Site::of(n, source));
                        let symbol = match module.pop() {
                            Some(last) if last == "*" => None,
                            Some(last) if last == "self" && !module.is_empty() => module.pop(),
                            last => last,
                        };
                        let local = alias.or_else(|| written.filter(|_| symbol.as_deref() == Some("self")));
                        out.push(Import {
                            module: module.join("::"),
                            symbol,
                            locals: local.into_iter().collect(),
                            line,
                            block,
                        });
                    }
                }
            }
            "type_item" => {
                if let Some((path, name)) = type_alias(n, source) {
                    let mut module = file_level(path, Site::of(n, source));
                    let symbol = module.pop();
                    out.push(Import {
                        module: module.join("::"),
                        symbol,
                        locals: vec![name],
                        line,
                        block,
                    });
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

/// The full path of every leaf in a `use` tree, with its alias. `prefix` is
/// the path the enclosing lists have written so far.
fn collect_use(
    node: Node,
    source: &[u8],
    prefix: &[String],
    out: &mut Vec<(Vec<String>, Option<String>)>,
) {
    let joined = |path: Option<Node>| -> Vec<String> {
        let mut segments = prefix.to_vec();
        segments.extend(path.and_then(|path| path.utf8_text(source).ok()).into_iter().flat_map(segments_of));
        segments
    };
    match node.kind() {
        "scoped_use_list" | "use_list" => {
            let next = joined(node.child_by_field_name("path"));
            let list = node.child_by_field_name("list").unwrap_or(node);
            let mut c = list.walk();
            for child in list.named_children(&mut c) {
                collect_use(child, source, &next, out);
            }
        }
        "use_as_clause" => out.push((
            joined(node.child_by_field_name("path")),
            node.child_by_field_name("alias")
                .and_then(|alias| alias.utf8_text(source).ok())
                .map(str::to_string),
        )),
        "use_wildcard" => {
            let mut path = joined(node.named_child(0));
            path.push("*".to_string());
            out.push((path, None));
        }
        "identifier" | "scoped_identifier" | "self" | "super" | "crate" => out.push((joined(Some(node)), None)),
        _ => {}
    }
}

fn segments_of(path: &str) -> impl Iterator<Item = String> + '_ {
    path.split("::").map(str::trim).filter(|segment| !segment.is_empty()).map(str::to_string)
}

/// The path a `type Name = path;` item aliases, without generic arguments,
/// and `Name`. An associated type of an `impl` or a trait, or an alias of a
/// type no path names, aliases no path.
fn type_alias(n: Node, source: &[u8]) -> Option<(Vec<String>, String)> {
    if n.parent()
        .and_then(|holder| holder.parent())
        .is_some_and(|owner| matches!(owner.kind(), "impl_item" | "trait_item"))
    {
        return None;
    }
    let mut aliased = n.child_by_field_name("type")?;
    if aliased.kind() == "generic_type" {
        aliased = aliased.child_by_field_name("type")?;
    }
    if !matches!(aliased.kind(), "type_identifier" | "scoped_type_identifier") {
        return None;
    }
    let name = n.child_by_field_name("name")?.utf8_text(source).ok()?;
    Some((segments_of(aliased.utf8_text(source).ok()?).collect(), name.to_string()))
}

/// A node with the source its tree was parsed from. A macro's arguments are
/// parsed as a tree of their own, so their nodes also carry the macro
/// invocation that holds them, where the file's tree continues outward.
#[derive(Clone, Copy)]
struct Site<'a> {
    node: Node<'a>,
    source: &'a [u8],
    invocation: Option<&'a Site<'a>>,
}

impl<'a> Site<'a> {
    fn of(node: Node<'a>, source: &'a [u8]) -> Self {
        Site {
            node,
            source,
            invocation: None,
        }
    }

    fn at(&self, node: Node<'a>) -> Self {
        Site { node, ..*self }
    }

    fn field(&self, name: &str) -> Option<Self> {
        self.node.child_by_field_name(name).map(|node| self.at(node))
    }

    fn text(&self) -> Option<&'a str> {
        self.node.utf8_text(self.source).ok()
    }

    /// The node around this one, continuing from a macro's own tree into
    /// the invocation that holds it.
    fn parent(&self) -> Option<Self> {
        match self.node.parent() {
            Some(node) => Some(self.at(node)),
            None => self.invocation.copied(),
        }
    }

    /// The line of the file the node is on; a node of a macro's own tree is
    /// on its invocation's line, which every block holding the node holds.
    fn line(&self) -> i64 {
        self.invocation
            .map_or(self.node.start_position().row as i64 + 1, |invocation| invocation.line())
    }
}

/// The path the name `name` written at `site` stands for, when the one `use`
/// in force there that binds it binds it by an alias. A name several `use`
/// items in force bind, such as `cfg` alternatives, stays as written.
fn aliased(name: &str, site: Site, imports: &[Import]) -> Option<String> {
    let mut binding = imports
        .iter()
        .filter(|import| import.binding() == Some(name) && import.in_force(site.line(), imports));
    let import = binding.next()?;
    if binding.next().is_some() || import.locals.is_empty() {
        return None;
    }
    let symbol = import.symbol.as_deref()?;
    Some(match import.module.as_str() {
        "" => symbol.to_string(),
        module => format!("{module}::{symbol}"),
    })
}

/// `path` as the file's own module writes it. An inline `mod` is part of
/// this file, so the leading segments that stay in it become one `self`:
/// a `self`, each `super` that leaves an inline `mod` around `site`, then
/// each segment that names an inline `mod` declared in the module reached.
fn file_level(mut path: Vec<String>, site: Site) -> Vec<String> {
    let mut modules = Vec::new();
    let mut root = site;
    while let Some(n) = root.parent() {
        if n.node.kind() == "mod_item" {
            modules.extend(n.field("body"));
        }
        root = n;
    }
    modules.push(root);
    let supers = path.iter().take_while(|segment| *segment == "super").count();
    let Some(mut module) = modules.get(supers).copied() else {
        path.drain(..modules.len() - 1);
        return path;
    };
    let mut end = supers + usize::from(path.first().is_some_and(|segment| segment == "self"));
    while let Some(inline) = path.get(end).and_then(|segment| {
        let mut c = module.node.walk();
        let item = module.node.named_children(&mut c).find(|item| {
            item.kind() == "mod_item"
                && module.at(*item).field("name").and_then(|name| name.text()) == Some(segment.as_str())
        })?;
        module.at(item).field("body")
    }) {
        module = inline;
        end += 1;
    }
    if end > 0 {
        path.splice(..end, ["self".to_string()]);
    }
    path
}

/// Every named item: free functions, methods (a `function_item` inside an
/// `impl_item`, carrying the impl'd type as `container`), structs, enums,
/// and traits. A method inside a `trait_item` carries the trait name as its
/// container, and every method its `Self` as the `imports` read it. A `mod`
/// item carries its `#[path]` value. Source-order, deduped on (name, line).
fn walk_declarations(root: Node, source: &[u8], imports: &[Import]) -> Vec<Declaration> {
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
                    let mut attributes = Vec::new();
                    let mut previous = n.prev_sibling();
                    while let Some(sibling) = previous {
                        match sibling.kind() {
                            "attribute_item" => attributes.push(sibling),
                            "line_comment" | "block_comment" => {}
                            _ => break,
                        }
                        previous = sibling.prev_sibling();
                    }
                    attributes.reverse();
                    let header_start = attributes.first().map_or(n.start_byte(), Node::start_byte);
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
                    let method = container.is_some() && matches!(n.kind(), "function_item" | "function_signature_item");
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
                                for attribute in &attributes {
                                    builder.node(*attribute);
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
                            self_type: method.then(|| self_type(Site::of(n, source), imports)).flatten(),
                            module_file: (n.kind() == "mod_item")
                                .then(|| path_attribute(&attributes, source))
                                .flatten(),
                            supertypes: Vec::new(),
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

/// The file a `#[path = "file"]` among an item's `attributes` names.
fn path_attribute(attributes: &[Node], source: &[u8]) -> Option<String> {
    attributes.iter().find_map(|item| {
        let attribute = item.named_child(0)?;
        if attribute.named_child(0)?.utf8_text(source).ok()? != "path" {
            return None;
        }
        let value = attribute.child_by_field_name("value")?.utf8_text(source).ok()?;
        Some(value.trim_matches('"').to_string())
    })
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

/// Every call / construction / macro / type reference, stamped with shape.
/// Free calls `name(..)` are `Free`; method calls `x.name(..)` are `Member`,
/// whose receiver is the type the file states for `x`; associated calls
/// `path::name(..)` are `Static`, whose receiver is the path; struct
/// literals `path::Type { .. }` are `Static` with the type's own path as the
/// receiver.
fn walk_references(root: Site, imports: &[Import]) -> Vec<Reference> {
    let mut out = Vec::new();
    let mut stack = vec![root.node];
    while let Some(node) = stack.pop() {
        let n = root.at(node);
        let line = node.start_position().row as i64 + 1;
        match node.kind() {
            "call_expression" => {
                if let Some((name, l, shape, receiver)) =
                    n.field("function").and_then(|function| call_shape(function, imports))
                {
                    out.push(Reference {
                        name,
                        line: l,
                        shape,
                        receiver,
                    });
                }
            }
            "struct_expression" => {
                if let Some(path) = n.field("name").and_then(|name| written_path(name, imports)) {
                    out.push(Reference {
                        name: path.rsplit("::").next().unwrap_or(&path).to_string(),
                        line,
                        shape: RefShape::Static,
                        receiver: Some(path),
                    });
                }
            }
            // `name!(..)` — a macro invocation references the macro by name.
            "macro_invocation" => {
                if let Some(name) = node
                    .child_by_field_name("macro")
                    .and_then(|m| last_segment(m, root.source))
                {
                    out.push(Reference {
                        name,
                        line,
                        shape: RefShape::Free,
                        receiver: None,
                    });
                }
                out.extend(macro_arguments(n, imports));
            }
            _ => {}
        }
        let mut c = node.walk();
        stack.extend(node.children(&mut c));
    }
    out
}

/// The calls inside a function-like macro's arguments. tree-sitter keeps a
/// macro's arguments as unparsed tokens, so they are parsed again as the
/// items of an array that starts on the macro's own line, a tree that
/// continues outward from the invocation.
fn macro_arguments(invocation: Site, imports: &[Import]) -> Vec<Reference> {
    let mut c = invocation.node.walk();
    let Some(arguments) = invocation
        .node
        .named_children(&mut c)
        .find(|child| child.kind() == "token_tree")
    else {
        return Vec::new();
    };
    let tokens = &invocation.source[arguments.start_byte()..arguments.end_byte()];
    if !matches!(tokens.first(), Some(b'(' | b'[')) || tokens.len() < 2 {
        return Vec::new();
    }
    let mut array = b"[".to_vec();
    array.extend_from_slice(&tokens[1..tokens.len() - 1]);
    array.extend_from_slice(b"];");
    let Some(tree) = parse(&array) else {
        return Vec::new();
    };
    let row = arguments.start_position().row as i64;
    let root = Site {
        node: tree.root_node(),
        source: &array,
        invocation: Some(&invocation),
    };
    walk_references(root, imports)
        .into_iter()
        .map(|reference| Reference {
            line: reference.line + row,
            ..reference
        })
        .collect()
}

/// Name / line / shape / receiver for a `call_expression`'s function node.
/// A bare `identifier` is a `Free` call, or the `Static` call of the path a
/// `use` alias stands for. A `field_expression` `x.name` is a `Member` call
/// whose receiver is `x`'s stated type, or none. A `scoped_identifier`
/// `path::name` is a `Static` call whose receiver is the path. A turbofish
/// `name::<T>` is the call it wraps.
fn call_shape(node: Site, imports: &[Import]) -> Option<(String, i64, RefShape, Option<String>)> {
    let line = |site: Site| site.node.start_position().row as i64 + 1;
    match node.node.kind() {
        "identifier" => {
            let written = node.text()?;
            let aliased = aliased(written, node, imports);
            let path = aliased.as_deref().unwrap_or(written);
            Some(match path.rsplit_once("::") {
                Some((receiver, name)) => (name.to_string(), line(node), RefShape::Static, Some(receiver.to_string())),
                None => (path.to_string(), line(node), RefShape::Free, None),
            })
        }
        "field_expression" => {
            let field = node.field("field")?;
            Some((
                field.text()?.to_string(),
                line(field),
                RefShape::Member,
                node.field("value").and_then(|value| stated_receiver(value, imports)),
            ))
        }
        "scoped_identifier" => {
            let name = node.field("name")?;
            Some((
                name.text()?.to_string(),
                line(name),
                RefShape::Static,
                node.field("path").and_then(|path| written_path(path, imports)),
            ))
        }
        "generic_function" => call_shape(node.field("function")?, imports),
        _ => None,
    }
}

/// The path `node` writes, without generic arguments, as the file's own
/// module reads it: `Self` and a `use` alias stand for the path they name.
fn written_path(node: Site, imports: &[Import]) -> Option<String> {
    let mut depth = 0usize;
    let text: String = node
        .text()?
        .chars()
        .filter(|c| {
            match c {
                '<' => depth += 1,
                '>' => depth = depth.saturating_sub(1),
                _ => return depth == 0 && !c.is_whitespace(),
            }
            false
        })
        .collect();
    let mut path = file_level(segments_of(&text).collect(), node);
    let head = match path.first()?.as_str() {
        "Self" => Some(self_type(node, imports)?),
        first => aliased(first, node, imports),
    };
    if let Some(head) = head {
        path.splice(..1, segments_of(&head));
    }
    Some(path.join("::"))
}

/// The type `self` and `Self` name at `node`: the enclosing impl's type, or
/// the enclosing trait.
fn self_type(node: Site, imports: &[Import]) -> Option<String> {
    let mut scope = node.parent();
    while let Some(n) = scope {
        match n.node.kind() {
            "impl_item" => return written_path(n.field("type")?, imports),
            "trait_item" => return Some(n.field("name")?.text()?.to_string()),
            _ => scope = n.parent(),
        }
    }
    None
}

/// The type a method call's receiver is stated to have: `self`, or a
/// parameter of the enclosing function or closure declared as a plain path
/// that no pattern in that function binds again before the call.
fn stated_receiver(value: Site, imports: &[Import]) -> Option<String> {
    match value.node.kind() {
        "self" => self_type(value, imports),
        "identifier" => {
            let variable = value.text()?;
            let mut scope = value;
            let mut before = value.node.start_byte();
            loop {
                scope = match scope.node.parent() {
                    Some(node) => scope.at(node),
                    None => {
                        let invocation = *scope.invocation?;
                        before = invocation.node.start_byte();
                        invocation
                    }
                };
                if matches!(scope.node.kind(), "function_item" | "closure_expression") {
                    break;
                }
            }
            if rebinds(scope, variable, before) {
                return None;
            }
            let parameters = scope.field("parameters")?;
            let mut c = parameters.node.walk();
            let parameter = parameters.node.named_children(&mut c).find(|parameter| {
                parameter.kind() == "parameter"
                    && parameter
                        .child_by_field_name("pattern")
                        .and_then(|pattern| pattern.utf8_text(scope.source).ok())
                        == Some(variable)
            })?;
            let mut declared = scope.at(parameter).field("type")?;
            while declared.node.kind() == "reference_type" {
                declared = declared.field("type")?;
            }
            (matches!(declared.node.kind(), "type_identifier" | "scoped_type_identifier")
                && !is_type_parameter(declared))
            .then(|| written_path(declared, imports))
            .flatten()
        }
        _ => None,
    }
}

/// Whether a pattern in the body of `scope` binds `variable` and is in force
/// at byte `before`: a `let` after its statement while its block lasts, an
/// `if let` or `while let` after its condition inside its `if` or `while`, a
/// `for` after its iterator inside the loop, a `match` arm after its pattern
/// inside the arm, and a closure's parameters inside the closure.
fn rebinds(scope: Site, variable: &str, before: usize) -> bool {
    let Some(body) = scope.node.child_by_field_name("body") else {
        return false;
    };
    let mut stack = vec![body];
    while let Some(node) = stack.pop() {
        if node.start_byte() >= before {
            continue;
        }
        let binding = match node.kind() {
            "let_declaration" => node
                .child_by_field_name("pattern")
                .zip(node.parent())
                .map(|(pattern, block)| (pattern, node.end_byte(), block.end_byte())),
            "let_condition" => node.child_by_field_name("pattern").zip(
                std::iter::successors(node.parent(), Node::parent)
                    .find(|owner| matches!(owner.kind(), "if_expression" | "while_expression")),
            )
            .map(|(pattern, owner)| (pattern, node.end_byte(), owner.end_byte())),
            "for_expression" => node
                .child_by_field_name("pattern")
                .zip(node.child_by_field_name("value"))
                .map(|(pattern, value)| (pattern, value.end_byte(), node.end_byte())),
            "match_arm" => node
                .child_by_field_name("pattern")
                .map(|pattern| (pattern, pattern.end_byte(), node.end_byte())),
            "closure_parameters" => node.parent().map(|closure| (node, node.end_byte(), closure.end_byte())),
            _ => None,
        };
        if binding.is_some_and(|(pattern, from, until)| {
            (from..until).contains(&before) && binds(pattern, variable, scope.source)
        }) {
            return true;
        }
        let mut c = node.walk();
        stack.extend(node.children(&mut c));
    }
    false
}

/// Whether `pattern` names `variable` anywhere inside it.
fn binds(pattern: Node, variable: &str, source: &[u8]) -> bool {
    let mut stack = vec![pattern];
    while let Some(node) = stack.pop() {
        if matches!(node.kind(), "identifier" | "shorthand_field_identifier")
            && node.utf8_text(source).ok() == Some(variable)
        {
            return true;
        }
        let mut c = node.walk();
        stack.extend(node.children(&mut c));
    }
    false
}

/// Whether `node` names a type parameter of an item that encloses it, which
/// any type may fill.
fn is_type_parameter(node: Site) -> bool {
    let Some(name) = node.text() else {
        return false;
    };
    let mut scope = node.parent();
    while let Some(n) = scope {
        if let Some(parameters) = n.node.child_by_field_name("type_parameters") {
            let mut c = parameters.walk();
            if parameters.named_children(&mut c).any(|parameter| {
                parameter
                    .child_by_field_name("name")
                    .and_then(|declared| declared.utf8_text(n.source).ok())
                    == Some(name)
            }) {
                return true;
            }
        }
        scope = n.parent();
    }
    false
}
