//! Per-symbol signature extraction for `trace structure`.
//!
//! Reparses a single file with tree-sitter to surface every piece of API
//! information an agent needs to reconstruct a file's public surface without
//! reading the body: visibility, return types, parameter types and defaults,
//! property types/defaults, attributes/decorators, class extends/implements,
//! generic/type parameters, and PHP 8.4 property hooks.
//!
//! Returns a vector of `Signature` records (one per declared class /
//! interface / trait / enum / function / method / property), each carrying
//! its `name`, `kind`, `line`, and an `extra` JSON object with the rich
//! signature fields. The `structure` command:
//!   - merges `extra` field-by-field into ctags-found symbols by line
//!   - synthesizes a fresh symbol from any signature whose line ctags missed
//!     (PHP 8.4 hooked properties and class declarations are the two ctags
//!     gaps this covers)
//!
//! Fields added are always additive — no existing field is touched.
//!
//! This module owns its parse. The cached extractor (`extraction/`) stays
//! unchanged: signatures are a `structure`-only concern and do not enter the
//! architecture graph or the per-file cache.

use serde_json::{json, Map, Value};
use std::path::Path;
use tree_sitter::{Node, Parser};

/// A per-symbol signature record. `name`, `kind`, `line` mirror the
/// `Symbol` shape `structure` already produces; `extra` carries the
/// signature-specific fields (visibility, return_type, parameters,
/// attributes, etc).
#[derive(Debug, Clone)]
pub struct Signature {
    pub name: String,
    pub kind: String,
    pub line: i64,
    pub extra: Value,
}

/// Extract per-symbol signatures for `path` from `source`. Empty vec for
/// unsupported languages or parse failure.
pub fn extract(source: &[u8], path: &Path) -> Vec<Signature> {
    let ext = match path.extension().and_then(|e| e.to_str()) {
        Some(e) => e.to_lowercase(),
        None => return Vec::new(),
    };
    match ext.as_str() {
        "php" => extract_php(source),
        "ts" | "js" => extract_ts(source, false),
        "tsx" | "jsx" => extract_ts(source, true),
        "py" => extract_py(source),
        _ => Vec::new(),
    }
}

fn parse(source: &[u8], lang: tree_sitter::Language) -> Option<tree_sitter::Tree> {
    let mut parser = Parser::new();
    parser.set_language(&lang).ok()?;
    parser.parse(source, None)
}

fn text(node: Node, source: &[u8]) -> String {
    node.utf8_text(source).unwrap_or("").to_string()
}

fn line(node: Node) -> i64 {
    node.start_position().row as i64 + 1
}

// ---------- PHP ----------

fn extract_php(source: &[u8]) -> Vec<Signature> {
    let lang: tree_sitter::Language = tree_sitter_php::LANGUAGE_PHP.into();
    let tree = match parse(source, lang) {
        Some(t) => t,
        None => return Vec::new(),
    };
    let mut out: Vec<Signature> = Vec::new();
    let mut stack = vec![tree.root_node()];
    while let Some(n) = stack.pop() {
        let kind: Option<&str> = match n.kind() {
            "class_declaration" => Some("class"),
            "interface_declaration" => Some("interface"),
            "trait_declaration" => Some("trait"),
            "enum_declaration" => Some("enum"),
            "function_definition" => Some("function"),
            "method_declaration" => Some("function"),
            _ => None,
        };
        if let Some(k) = kind {
            if let Some(name_node) = n.child_by_field_name("name") {
                let name = text(name_node, source);
                let l = line(name_node);
                let extra = match k {
                    "function" => php_function_signature(n, source),
                    _ => php_class_signature(n, source),
                };
                out.push(Signature {
                    name,
                    kind: k.to_string(),
                    line: l,
                    extra,
                });
            }
        }
        if n.kind() == "property_declaration" {
            for (nm, l, extra) in php_properties(n, source) {
                out.push(Signature {
                    name: nm,
                    kind: "property".to_string(),
                    line: l,
                    extra,
                });
            }
        }
        let mut c = n.walk();
        for child in n.children(&mut c) {
            stack.push(child);
        }
    }
    out
}

fn php_class_signature(n: Node, source: &[u8]) -> Value {
    let mut m = Map::new();
    let (attrs, modifiers) = php_attrs_and_modifiers(n, source);
    if !attrs.is_empty() {
        m.insert("attributes".into(), Value::Array(attrs));
    }
    if !modifiers.is_empty() {
        m.insert(
            "modifiers".into(),
            Value::Array(modifiers.into_iter().map(|s| json!(s)).collect()),
        );
    }
    // base_clause and class_interface_clause are children of the class node.
    let mut cur = n.walk();
    let mut extends: Option<String> = None;
    let mut implements: Vec<String> = Vec::new();
    for child in n.children(&mut cur) {
        match child.kind() {
            "base_clause" => {
                let mut cc = child.walk();
                for sub in child.children(&mut cc) {
                    if matches!(sub.kind(), "name" | "qualified_name") {
                        extends = Some(text(sub, source));
                        break;
                    }
                }
            }
            "class_interface_clause" => {
                let mut cc = child.walk();
                for sub in child.children(&mut cc) {
                    if matches!(sub.kind(), "name" | "qualified_name") {
                        implements.push(text(sub, source));
                    }
                }
            }
            _ => {}
        }
    }
    if let Some(e) = extends {
        m.insert("extends".into(), json!(e));
    }
    if !implements.is_empty() {
        m.insert(
            "implements".into(),
            Value::Array(implements.into_iter().map(|s| json!(s)).collect()),
        );
    }
    Value::Object(m)
}

fn php_function_signature(n: Node, source: &[u8]) -> Value {
    let mut m = Map::new();
    let (attrs, modifiers) = php_attrs_and_modifiers(n, source);
    if !attrs.is_empty() {
        m.insert("attributes".into(), Value::Array(attrs));
    }
    for mod_text in modifiers.iter() {
        if matches!(mod_text.as_str(), "public" | "protected" | "private") {
            m.insert("visibility".into(), json!(mod_text));
            break;
        }
    }
    if !modifiers.is_empty() {
        m.insert(
            "modifiers".into(),
            Value::Array(modifiers.into_iter().map(|s| json!(s)).collect()),
        );
    }
    // tree-sitter-php exposes the parameter list as a `formal_parameters`
    // node child of method_declaration / function_definition. The field
    // name is "parameters" on function_definition; on method_declaration
    // it's typically a positional child of kind `formal_parameters`.
    let params_node = n
        .child_by_field_name("parameters")
        .or_else(|| find_child(n, "formal_parameters"));
    if let Some(params) = params_node {
        m.insert("parameters".into(), Value::Array(php_parameters(params, source)));
    }
    let ret_node = n.child_by_field_name("return_type").or_else(|| {
        // method_declaration places the return type as a direct child after
        // formal_parameters; pick the first type-shaped child after parens.
        find_return_type(n)
    });
    if let Some(ret) = ret_node {
        m.insert("return_type".into(), json!(text(ret, source)));
    }
    Value::Object(m)
}

/// First child of `n` with the given `kind`.
fn find_child<'a>(n: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut c = n.walk();
    for ch in n.children(&mut c) {
        if ch.kind() == kind {
            return Some(ch);
        }
    }
    None
}

/// PHP method_declaration encodes the return type as a type-shaped child
/// appearing after the `formal_parameters` and the `:` colon. Pick the
/// first such node, which is what tree-sitter-php produces.
fn find_return_type<'a>(n: Node<'a>) -> Option<Node<'a>> {
    let mut seen_params = false;
    let mut c = n.walk();
    for child in n.children(&mut c) {
        if child.kind() == "formal_parameters" {
            seen_params = true;
            continue;
        }
        if !seen_params {
            continue;
        }
        if matches!(
            child.kind(),
            "primitive_type"
                | "named_type"
                | "nullable_type"
                | "union_type"
                | "intersection_type"
                | "optional_type"
        ) {
            return Some(child);
        }
        if child.kind() == "compound_statement" || child.kind() == ";" {
            return None;
        }
    }
    None
}

/// One entry per `property_element` inside a `property_declaration`.
/// Returns `(name, name_line, extra)` triples where `name` is the bare
/// identifier (no leading `$`).
fn php_properties(n: Node, source: &[u8]) -> Vec<(String, i64, Value)> {
    let mut visibility: Option<String> = None;
    let mut modifiers: Vec<String> = Vec::new();
    let mut type_text: Option<String> = None;
    let mut attrs: Vec<Value> = Vec::new();
    let mut hook_list: Option<Node> = None;
    let mut cur = n.walk();
    for child in n.children(&mut cur) {
        match child.kind() {
            "attribute_list" => {
                for a in extract_attribute_list(child, source) {
                    attrs.push(a);
                }
            }
            "visibility_modifier" => {
                let v = text(child, source);
                visibility = Some(v.clone());
                modifiers.push(v);
            }
            "static_modifier"
            | "readonly_modifier"
            | "abstract_modifier"
            | "final_modifier" => modifiers.push(text(child, source)),
            "primitive_type"
            | "named_type"
            | "nullable_type"
            | "union_type"
            | "intersection_type"
            | "optional_type" => {
                type_text = Some(text(child, source));
            }
            "property_hook_list" => {
                hook_list = Some(child);
            }
            _ => {}
        }
    }
    let hooks: Vec<Value> = hook_list
        .map(|h| php_property_hooks(h, source))
        .unwrap_or_default();

    let mut out: Vec<(String, i64, Value)> = Vec::new();
    let mut cur = n.walk();
    for child in n.children(&mut cur) {
        if child.kind() != "property_element" {
            continue;
        }
        let mut name_node: Option<Node> = None;
        let mut default_text: Option<String> = None;
        let mut ec = child.walk();
        for sub in child.children(&mut ec) {
            match sub.kind() {
                "variable_name" => {
                    name_node = Some(sub);
                }
                "property_initializer" => {
                    let mut ic = sub.walk();
                    for init in sub.children(&mut ic) {
                        if init.kind() != "=" {
                            default_text = Some(text(init, source));
                        }
                    }
                }
                _ => {}
            }
        }
        let nn = match name_node {
            Some(x) => x,
            None => continue,
        };
        // The variable_name has a leading `$` token plus a `name` child.
        let bare = nn
            .child_by_field_name("name")
            .map(|nm| text(nm, source))
            .unwrap_or_else(|| {
                text(nn, source).trim_start_matches('$').to_string()
            });
        let l = line(nn);
        let mut m = Map::new();
        if !attrs.is_empty() {
            m.insert("attributes".into(), Value::Array(attrs.clone()));
        }
        if let Some(v) = &visibility {
            m.insert("visibility".into(), json!(v));
        }
        if !modifiers.is_empty() {
            m.insert(
                "modifiers".into(),
                Value::Array(modifiers.iter().map(|s| json!(s)).collect()),
            );
        }
        if let Some(t) = &type_text {
            m.insert("type".into(), json!(t));
        }
        if let Some(d) = default_text {
            m.insert("default".into(), json!(d));
        }
        if !hooks.is_empty() {
            m.insert("hooks".into(), Value::Array(hooks.clone()));
        }
        out.push((bare, l, Value::Object(m)));
    }
    out
}

fn php_property_hooks(list: Node, source: &[u8]) -> Vec<Value> {
    let mut out = Vec::new();
    let mut c = list.walk();
    for child in list.children(&mut c) {
        if child.kind() == "property_hook" {
            let mut hook = Map::new();
            let mut hc = child.walk();
            for sub in child.children(&mut hc) {
                if sub.kind() == "name" {
                    hook.insert("accessor".into(), json!(text(sub, source)));
                }
            }
            hook.insert("source".into(), json!(text(child, source)));
            out.push(Value::Object(hook));
        }
    }
    out
}

fn php_parameters(params: Node, source: &[u8]) -> Vec<Value> {
    let mut out = Vec::new();
    let mut c = params.walk();
    for child in params.children(&mut c) {
        match child.kind() {
            "simple_parameter"
            | "variadic_parameter"
            | "property_promotion_parameter" => {
                out.push(php_parameter(child, source));
            }
            _ => {}
        }
    }
    out
}

fn php_parameter(p: Node, source: &[u8]) -> Value {
    let mut m = Map::new();
    let mut type_text: Option<String> = None;
    let mut name_text: Option<String> = None;
    let mut default_text: Option<String> = None;
    let mut visibility: Option<String> = None;
    let mut modifiers: Vec<String> = Vec::new();
    let mut attrs: Vec<Value> = Vec::new();
    let mut variadic = false;
    let mut reference = false;

    let mut c = p.walk();
    for child in p.children(&mut c) {
        match child.kind() {
            "attribute_list" => {
                for a in extract_attribute_list(child, source) {
                    attrs.push(a);
                }
            }
            "visibility_modifier" => {
                let v = text(child, source);
                visibility = Some(v.clone());
                modifiers.push(v);
            }
            "readonly_modifier" => modifiers.push(text(child, source)),
            "primitive_type"
            | "named_type"
            | "nullable_type"
            | "union_type"
            | "intersection_type"
            | "optional_type" => {
                type_text = Some(text(child, source));
            }
            "variable_name" => {
                // tree-sitter-php: variable_name has a `$` token and a
                // `name` child. Strip the `$` so output matches what
                // callers expect ("slug", not "$slug").
                let bare = child
                    .child_by_field_name("name")
                    .map(|nm| text(nm, source))
                    .unwrap_or_else(|| {
                        text(child, source).trim_start_matches('$').to_string()
                    });
                name_text = Some(bare);
            }
            "..." => variadic = true,
            "&" => reference = true,
            _ => {}
        }
    }
    // Default value. tree-sitter-php places `= expr` as two positional
    // children after the variable_name (no field name on most grammar
    // versions). Grab the expression following the `=` token.
    if default_text.is_none() {
        if let Some(d) = p.child_by_field_name("default_value") {
            default_text = Some(text(d, source));
        } else {
            let mut c = p.walk();
            let mut seen_eq = false;
            for ch in p.children(&mut c) {
                if ch.kind() == "=" {
                    seen_eq = true;
                    continue;
                }
                if seen_eq {
                    default_text = Some(text(ch, source));
                    break;
                }
            }
        }
    }
    if !attrs.is_empty() {
        m.insert("attributes".into(), Value::Array(attrs));
    }
    if let Some(v) = visibility {
        m.insert("visibility".into(), json!(v));
    }
    if !modifiers.is_empty() {
        m.insert(
            "modifiers".into(),
            Value::Array(modifiers.into_iter().map(|s| json!(s)).collect()),
        );
    }
    if let Some(n) = name_text {
        m.insert("name".into(), json!(n));
    }
    if let Some(t) = type_text {
        m.insert("type".into(), json!(t));
    }
    if let Some(d) = default_text {
        m.insert("default".into(), json!(d));
    }
    if variadic {
        m.insert("variadic".into(), json!(true));
    }
    if reference {
        m.insert("by_reference".into(), json!(true));
    }
    Value::Object(m)
}

/// Walk children of a class/method/function/property declaration and
/// collect (attributes, modifiers). tree-sitter-php places these as
/// direct children of the declaration node, in source order.
fn php_attrs_and_modifiers(n: Node, source: &[u8]) -> (Vec<Value>, Vec<String>) {
    let mut attrs: Vec<Value> = Vec::new();
    let mut modifiers: Vec<String> = Vec::new();
    let mut c = n.walk();
    for child in n.children(&mut c) {
        match child.kind() {
            "attribute_list" => {
                for a in extract_attribute_list(child, source) {
                    attrs.push(a);
                }
            }
            "abstract_modifier"
            | "final_modifier"
            | "readonly_modifier"
            | "visibility_modifier"
            | "static_modifier" => {
                modifiers.push(text(child, source));
            }
            _ => {}
        }
    }
    (attrs, modifiers)
}

fn extract_attribute_list(list: Node, source: &[u8]) -> Vec<Value> {
    // tree-sitter-php shape: attribute_list > attribute_group+ > attribute+.
    // Walk down through the groups and collect every `attribute` node.
    let mut out = Vec::new();
    let mut stack = vec![list];
    while let Some(n) = stack.pop() {
        if n.kind() == "attribute" {
            let mut name: Option<String> = None;
            let mut cc = n.walk();
            for sub in n.children(&mut cc) {
                if matches!(sub.kind(), "name" | "qualified_name") {
                    name = Some(text(sub, source));
                    break;
                }
            }
            let mut m = Map::new();
            if let Some(nm) = name {
                m.insert("name".into(), json!(nm));
            }
            m.insert("source".into(), json!(text(n, source)));
            out.push(Value::Object(m));
            continue;
        }
        let mut c = n.walk();
        for child in n.children(&mut c) {
            stack.push(child);
        }
    }
    out.reverse();
    out
}


// ---------- TypeScript / JavaScript ----------

fn extract_ts(source: &[u8], is_tsx: bool) -> Vec<Signature> {
    let lang: tree_sitter::Language = if is_tsx {
        tree_sitter_typescript::LANGUAGE_TSX.into()
    } else {
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
    };
    let tree = match parse(source, lang) {
        Some(t) => t,
        None => return Vec::new(),
    };
    let mut out: Vec<Signature> = Vec::new();
    let mut stack = vec![tree.root_node()];
    while let Some(n) = stack.pop() {
        let entry: Option<(&str, Value)> = match n.kind() {
            "class_declaration" | "abstract_class_declaration" => {
                Some(("class", ts_class_signature(n, source)))
            }
            "interface_declaration" => Some(("interface", ts_interface_signature(n, source))),
            "function_declaration"
            | "generator_function_declaration"
            | "function_signature" => Some(("function", ts_function_signature(n, source))),
            "method_definition"
            | "method_signature"
            | "abstract_method_signature" => Some(("function", ts_method_signature(n, source))),
            "public_field_definition" | "property_signature" => {
                Some(("property", ts_field_signature(n, source)))
            }
            _ => None,
        };
        if let Some((kind, extra)) = entry {
            if let Some(name_node) = n.child_by_field_name("name") {
                out.push(Signature {
                    name: text(name_node, source),
                    kind: kind.to_string(),
                    line: line(name_node),
                    extra,
                });
            }
        }
        let mut c = n.walk();
        for child in n.children(&mut c) {
            stack.push(child);
        }
    }
    out
}

fn ts_decorators_before(n: Node, source: &[u8]) -> Vec<Value> {
    // Decorators on a class declaration appear inside the wrapping
    // `export_statement`, not as siblings of the class node. Walk siblings
    // of whichever node is positioned in the surrounding scope.
    let target = match n.parent() {
        Some(p) if p.kind() == "export_statement" => p
            .children(&mut p.walk())
            .find(|c| c.id() == n.id())
            .unwrap_or(n),
        _ => n,
    };
    // Sibling walk: collect every leading `decorator` node up to the target.
    let parent = target.parent();
    let mut decorators: Vec<Node> = Vec::new();
    if let Some(par) = parent {
        let mut c = par.walk();
        for child in par.children(&mut c) {
            if child.id() == target.id() {
                break;
            }
            if child.kind() == "decorator" {
                decorators.push(child);
            }
        }
    } else {
        let mut cur = target.prev_sibling();
        while let Some(s) = cur {
            if s.kind() == "decorator" {
                decorators.push(s);
                cur = s.prev_sibling();
            } else {
                break;
            }
        }
        decorators.reverse();
    }
    decorators
        .into_iter()
        .map(|d| {
            let mut m = Map::new();
            m.insert("source".into(), json!(text(d, source)));
            Value::Object(m)
        })
        .collect()
}

fn ts_class_signature(n: Node, source: &[u8]) -> Value {
    let mut m = Map::new();
    let decorators = ts_decorators_before(n, source);
    if !decorators.is_empty() {
        m.insert("decorators".into(), Value::Array(decorators));
    }
    if n.kind() == "abstract_class_declaration" {
        m.insert("abstract".into(), json!(true));
    }
    if let Some(tp) = n.child_by_field_name("type_parameters") {
        m.insert("type_parameters".into(), json!(text(tp, source)));
    }
    // Heritage: class_heritage child carries extends_clause + implements_clause.
    let mut cur = n.walk();
    for child in n.children(&mut cur) {
        if child.kind() == "class_heritage" {
            let mut cc = child.walk();
            for sub in child.children(&mut cc) {
                match sub.kind() {
                    "extends_clause" => {
                        let mut ec = sub.walk();
                        for e in sub.children(&mut ec) {
                            if matches!(
                                e.kind(),
                                "identifier"
                                    | "type_identifier"
                                    | "member_expression"
                                    | "generic_type"
                            ) {
                                m.insert("extends".into(), json!(text(e, source)));
                                break;
                            }
                        }
                    }
                    "implements_clause" => {
                        let mut imps = Vec::new();
                        let mut ic = sub.walk();
                        for i in sub.children(&mut ic) {
                            if matches!(
                                i.kind(),
                                "type_identifier" | "generic_type" | "nested_type_identifier"
                            ) {
                                imps.push(json!(text(i, source)));
                            }
                        }
                        if !imps.is_empty() {
                            m.insert("implements".into(), Value::Array(imps));
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    Value::Object(m)
}

fn ts_interface_signature(n: Node, source: &[u8]) -> Value {
    let mut m = Map::new();
    if let Some(tp) = n.child_by_field_name("type_parameters") {
        m.insert("type_parameters".into(), json!(text(tp, source)));
    }
    let mut cur = n.walk();
    let mut extends = Vec::new();
    for child in n.children(&mut cur) {
        if child.kind() == "extends_type_clause" {
            let mut cc = child.walk();
            for sub in child.children(&mut cc) {
                if matches!(
                    sub.kind(),
                    "type_identifier" | "generic_type" | "nested_type_identifier"
                ) {
                    extends.push(json!(text(sub, source)));
                }
            }
        }
    }
    if !extends.is_empty() {
        m.insert("extends".into(), Value::Array(extends));
    }
    Value::Object(m)
}

fn ts_function_signature(n: Node, source: &[u8]) -> Value {
    let mut m = Map::new();
    if let Some(tp) = n.child_by_field_name("type_parameters") {
        m.insert("type_parameters".into(), json!(text(tp, source)));
    }
    if let Some(params) = n.child_by_field_name("parameters") {
        m.insert("parameters".into(), Value::Array(ts_parameters(params, source)));
    }
    if let Some(ret) = n.child_by_field_name("return_type") {
        m.insert(
            "return_type".into(),
            json!(text(ret, source).trim_start_matches(':').trim().to_string()),
        );
    }
    let mut c = n.walk();
    for child in n.children(&mut c) {
        if child.kind() == "async" {
            m.insert("async".into(), json!(true));
        }
    }
    Value::Object(m)
}

fn ts_method_signature(n: Node, source: &[u8]) -> Value {
    let mut base = ts_function_signature(n, source);
    let m = base.as_object_mut().unwrap();
    let decorators = ts_decorators_before(n, source);
    if !decorators.is_empty() {
        m.insert("decorators".into(), Value::Array(decorators));
    }
    let mut c = n.walk();
    let mut modifiers: Vec<String> = Vec::new();
    for child in n.children(&mut c) {
        match child.kind() {
            "accessibility_modifier" => {
                let v = text(child, source);
                m.insert("visibility".into(), json!(v.clone()));
                modifiers.push(v);
            }
            "static" | "readonly" | "abstract" | "override" | "async" => {
                modifiers.push(text(child, source));
            }
            _ => {}
        }
    }
    if !modifiers.is_empty() {
        m.insert(
            "modifiers".into(),
            Value::Array(modifiers.into_iter().map(|s| json!(s)).collect()),
        );
    }
    base
}

fn ts_field_signature(n: Node, source: &[u8]) -> Value {
    let mut m = Map::new();
    let decorators = ts_decorators_before(n, source);
    if !decorators.is_empty() {
        m.insert("decorators".into(), Value::Array(decorators));
    }
    let mut c = n.walk();
    let mut modifiers: Vec<String> = Vec::new();
    for child in n.children(&mut c) {
        match child.kind() {
            "accessibility_modifier" => {
                let v = text(child, source);
                m.insert("visibility".into(), json!(v.clone()));
                modifiers.push(v);
            }
            "static" | "readonly" | "abstract" | "override" | "declare" => {
                modifiers.push(text(child, source));
            }
            _ => {}
        }
    }
    if !modifiers.is_empty() {
        m.insert(
            "modifiers".into(),
            Value::Array(modifiers.into_iter().map(|s| json!(s)).collect()),
        );
    }
    if let Some(t) = n.child_by_field_name("type") {
        m.insert(
            "type".into(),
            json!(text(t, source).trim_start_matches(':').trim().to_string()),
        );
    }
    if let Some(v) = n.child_by_field_name("value") {
        m.insert("default".into(), json!(text(v, source)));
    }
    Value::Object(m)
}

fn ts_parameters(params: Node, source: &[u8]) -> Vec<Value> {
    let mut out = Vec::new();
    let mut c = params.walk();
    for child in params.children(&mut c) {
        match child.kind() {
            "required_parameter" | "optional_parameter" => {
                out.push(ts_parameter(child, source));
            }
            _ => {}
        }
    }
    out
}

fn ts_parameter(p: Node, source: &[u8]) -> Value {
    let mut m = Map::new();
    let mut decorators: Vec<Value> = Vec::new();
    let mut modifiers: Vec<String> = Vec::new();
    let mut c = p.walk();
    for child in p.children(&mut c) {
        match child.kind() {
            "decorator" => {
                let mut dm = Map::new();
                dm.insert("source".into(), json!(text(child, source)));
                decorators.push(Value::Object(dm));
            }
            "accessibility_modifier" => {
                let v = text(child, source);
                m.insert("visibility".into(), json!(v.clone()));
                modifiers.push(v);
            }
            "readonly" | "override" => modifiers.push(text(child, source)),
            _ => {}
        }
    }
    if !decorators.is_empty() {
        m.insert("decorators".into(), Value::Array(decorators));
    }
    if !modifiers.is_empty() {
        m.insert(
            "modifiers".into(),
            Value::Array(modifiers.into_iter().map(|s| json!(s)).collect()),
        );
    }
    if let Some(pat) = p.child_by_field_name("pattern") {
        m.insert("name".into(), json!(text(pat, source)));
    }
    if let Some(t) = p.child_by_field_name("type") {
        m.insert(
            "type".into(),
            json!(text(t, source).trim_start_matches(':').trim().to_string()),
        );
    }
    if let Some(v) = p.child_by_field_name("value") {
        m.insert("default".into(), json!(text(v, source)));
    }
    if p.kind() == "optional_parameter" {
        m.insert("optional".into(), json!(true));
    }
    Value::Object(m)
}

// ---------- Python ----------

fn extract_py(source: &[u8]) -> Vec<Signature> {
    let lang: tree_sitter::Language = tree_sitter_python::LANGUAGE.into();
    let tree = match parse(source, lang) {
        Some(t) => t,
        None => return Vec::new(),
    };
    let mut out: Vec<Signature> = Vec::new();
    let mut stack = vec![tree.root_node()];
    while let Some(n) = stack.pop() {
        let entry: Option<(&str, Value)> = match n.kind() {
            "function_definition" => Some(("function", py_function_signature(n, source))),
            "class_definition" => Some(("class", py_class_signature(n, source))),
            _ => None,
        };
        if let Some((kind, extra)) = entry {
            if let Some(name_node) = n.child_by_field_name("name") {
                out.push(Signature {
                    name: text(name_node, source),
                    kind: kind.to_string(),
                    line: line(name_node),
                    extra,
                });
            }
        }
        let mut c = n.walk();
        for child in n.children(&mut c) {
            stack.push(child);
        }
    }
    out
}

fn py_decorators_before(n: Node, source: &[u8]) -> Vec<Value> {
    // In tree-sitter-python a decorated def is wrapped in a
    // `decorated_definition` whose children are the leading decorators
    // followed by the `definition`. Walk the parent.
    let mut out = Vec::new();
    if let Some(p) = n.parent() {
        if p.kind() == "decorated_definition" {
            let mut c = p.walk();
            for child in p.children(&mut c) {
                if child.kind() == "decorator" {
                    let mut m = Map::new();
                    m.insert("source".into(), json!(text(child, source)));
                    out.push(Value::Object(m));
                }
            }
        }
    }
    out
}

fn py_function_signature(n: Node, source: &[u8]) -> Value {
    let mut m = Map::new();
    let decorators = py_decorators_before(n, source);
    if !decorators.is_empty() {
        m.insert("decorators".into(), Value::Array(decorators));
    }
    if let Some(tp) = n.child_by_field_name("type_parameters") {
        m.insert("type_parameters".into(), json!(text(tp, source)));
    }
    if let Some(params) = n.child_by_field_name("parameters") {
        m.insert("parameters".into(), Value::Array(py_parameters(params, source)));
    }
    if let Some(ret) = n.child_by_field_name("return_type") {
        m.insert("return_type".into(), json!(text(ret, source)));
    }
    let mut c = n.walk();
    for child in n.children(&mut c) {
        if child.kind() == "async" {
            m.insert("async".into(), json!(true));
        }
    }
    Value::Object(m)
}

fn py_class_signature(n: Node, source: &[u8]) -> Value {
    let mut m = Map::new();
    let decorators = py_decorators_before(n, source);
    if !decorators.is_empty() {
        m.insert("decorators".into(), Value::Array(decorators));
    }
    if let Some(tp) = n.child_by_field_name("type_parameters") {
        m.insert("type_parameters".into(), json!(text(tp, source)));
    }
    if let Some(supers) = n.child_by_field_name("superclasses") {
        let mut bases: Vec<Value> = Vec::new();
        let mut c = supers.walk();
        for child in supers.children(&mut c) {
            match child.kind() {
                "(" | ")" | "," => continue,
                _ => bases.push(json!(text(child, source))),
            }
        }
        if !bases.is_empty() {
            m.insert("bases".into(), Value::Array(bases));
        }
    }
    Value::Object(m)
}

fn py_parameters(params: Node, source: &[u8]) -> Vec<Value> {
    let mut out = Vec::new();
    let mut c = params.walk();
    for child in params.children(&mut c) {
        match child.kind() {
            "identifier" => out.push(json!({"name": text(child, source)})),
            "typed_parameter" => {
                let mut m = Map::new();
                let mut cc = child.walk();
                for sub in child.children(&mut cc) {
                    match sub.kind() {
                        "identifier" => {
                            m.insert("name".into(), json!(text(sub, source)));
                        }
                        "type" => {
                            m.insert("type".into(), json!(text(sub, source)));
                        }
                        _ => {}
                    }
                }
                out.push(Value::Object(m));
            }
            "default_parameter" => {
                let mut m = Map::new();
                if let Some(name) = child.child_by_field_name("name") {
                    m.insert("name".into(), json!(text(name, source)));
                }
                if let Some(v) = child.child_by_field_name("value") {
                    m.insert("default".into(), json!(text(v, source)));
                }
                out.push(Value::Object(m));
            }
            "typed_default_parameter" => {
                let mut m = Map::new();
                if let Some(name) = child.child_by_field_name("name") {
                    m.insert("name".into(), json!(text(name, source)));
                }
                if let Some(t) = child.child_by_field_name("type") {
                    m.insert("type".into(), json!(text(t, source)));
                }
                if let Some(v) = child.child_by_field_name("value") {
                    m.insert("default".into(), json!(text(v, source)));
                }
                out.push(Value::Object(m));
            }
            "list_splat_pattern" => {
                let mut m = Map::new();
                m.insert("name".into(), json!(text(child, source)));
                m.insert("variadic".into(), json!(true));
                out.push(Value::Object(m));
            }
            "dictionary_splat_pattern" => {
                let mut m = Map::new();
                m.insert("name".into(), json!(text(child, source)));
                m.insert("keyword_variadic".into(), json!(true));
                out.push(Value::Object(m));
            }
            _ => {}
        }
    }
    out
}
