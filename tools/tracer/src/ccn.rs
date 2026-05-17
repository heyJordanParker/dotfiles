//! Tree-sitter AST per-function cyclomatic-complexity backend.
//!
//! Covers python/ts/php with fixed decision-node sets, plus bash, lua, go,
//! rust, ruby, java, c so every supported source file gets a real
//! per-function complexity profile.
//!
//! Per-language decision-node sets are derived from each grammar's
//! `node-types.json` vocabulary and follow the McCabe convention:
//! CCN starts at 1; +1 per branch point (if/elif/else-if, for/while/loop,
//! each case/when arm, each catch/rescue/except, ternary/conditional, and
//! short-circuit boolean operators). `else`/`default` are not counted —
//! the branch is attributed to the owning `if`/`switch`.
//!
//! Function names are parent-qualified (`outer.inner`) for nested
//! functions and methods.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;
use tree_sitter::{Language, Node, Parser};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionFact {
    pub name: String,
    pub start_line: i64,
    pub nloc: i64,
    pub cyclomatic_complexity: i64,
}

/// How a language names a function node and counts short-circuit operators.
struct LangSpec {
    function_kinds: HashSet<&'static str>,
    decision_kinds: HashSet<&'static str>,
    /// For grammars where short-circuit appears as a `boolean_operator`
    /// parent, each `and`/`or` token child is counted.
    boolean_operator_kind: Option<&'static str>,
    /// Short-circuit operator tokens (`&&`, `||`, `??`, `and`, `or`, …).
    /// Counted wherever they appear as a direct child — the parent node
    /// kind varies by grammar (`binary_expression` in most, `list` in
    /// bash), so the count is not gated on the parent kind.
    short_circuit: HashSet<&'static str>,
}

fn set(items: &[&'static str]) -> HashSet<&'static str> {
    items.iter().copied().collect()
}

fn spec_for(lang: &str) -> Option<LangSpec> {
    Some(match lang {
        // --- python ---------------------------------------------------------
        "python" => LangSpec {
            function_kinds: set(&["function_definition", "lambda"]),
            decision_kinds: set(&[
                "if_statement",
                "elif_clause",
                "for_statement",
                "while_statement",
                "except_clause",
                "case_clause",
                "conditional_expression",
                "if_clause",
                "for_in_clause",
            ]),
            boolean_operator_kind: Some("boolean_operator"),
            short_circuit: HashSet::new(),
        },
        // --- typescript -----------------------------------------------------
        "typescript" => LangSpec {
            function_kinds: set(&[
                "function_declaration",
                "function_expression",
                "arrow_function",
                "method_definition",
                "generator_function",
                "generator_function_declaration",
            ]),
            decision_kinds: set(&[
                "if_statement",
                "for_statement",
                "for_in_statement",
                "while_statement",
                "do_statement",
                "switch_case",
                "catch_clause",
                "ternary_expression",
            ]),
            boolean_operator_kind: None,
            short_circuit: set(&["&&", "||", "??"]),
        },
        // --- php ------------------------------------------------------------
        "php" => LangSpec {
            function_kinds: set(&[
                "function_definition",
                "method_declaration",
                "anonymous_function",
                "anonymous_function_creation_expression",
                "arrow_function",
            ]),
            decision_kinds: set(&[
                "if_statement",
                "else_if_clause",
                "for_statement",
                "foreach_statement",
                "while_statement",
                "do_statement",
                "case_statement",
                "catch_clause",
                "conditional_expression",
            ]),
            boolean_operator_kind: None,
            short_circuit: set(&["&&", "||", "??", "and", "or", "xor"]),
        },
        // --- bash ----------------------------------------------------------
        "bash" => LangSpec {
            function_kinds: set(&["function_definition"]),
            decision_kinds: set(&[
                "if_statement",
                "elif_clause",
                "for_statement",
                "c_style_for_statement",
                "while_statement",
                "case_item",
                "ternary_expression",
            ]),
            boolean_operator_kind: None,
            short_circuit: set(&["&&", "||"]),
        },
        // --- lua -----------------------------------------------------------
        "lua" => LangSpec {
            function_kinds: set(&[
                "function_declaration",
                "function_definition",
            ]),
            decision_kinds: set(&[
                "if_statement",
                "elseif_statement",
                "for_statement",
                "while_statement",
            ]),
            boolean_operator_kind: None,
            short_circuit: set(&["and", "or"]),
        },
        // --- go ------------------------------------------------------------
        "go" => LangSpec {
            function_kinds: set(&[
                "function_declaration",
                "method_declaration",
                "func_literal",
            ]),
            decision_kinds: set(&[
                "if_statement",
                "for_statement",
                "expression_case",
                "type_case",
                "communication_case",
                "select_statement",
            ]),
            boolean_operator_kind: None,
            short_circuit: set(&["&&", "||"]),
        },
        // --- rust ----------------------------------------------------------
        "rust" => LangSpec {
            function_kinds: set(&[
                "function_item",
                "closure_expression",
            ]),
            decision_kinds: set(&[
                "if_expression",
                "for_expression",
                "while_expression",
                "loop_expression",
                "match_arm",
                "let_condition",
            ]),
            boolean_operator_kind: None,
            short_circuit: set(&["&&", "||"]),
        },
        // --- ruby ----------------------------------------------------------
        "ruby" => LangSpec {
            function_kinds: set(&[
                "method",
                "singleton_method",
                "lambda",
                "do_block",
                "block",
            ]),
            decision_kinds: set(&[
                "if",
                "elsif",
                "unless",
                "while",
                "until",
                "for",
                "case",
                "case_match",
                "when",
                "in_clause",
                "rescue",
                "conditional",
                "if_modifier",
                "unless_modifier",
                "while_modifier",
                "until_modifier",
                "rescue_modifier",
            ]),
            boolean_operator_kind: None,
            short_circuit: set(&["&&", "||", "and", "or"]),
        },
        // --- java ----------------------------------------------------------
        "java" => LangSpec {
            function_kinds: set(&[
                "method_declaration",
                "constructor_declaration",
                "lambda_expression",
            ]),
            decision_kinds: set(&[
                "if_statement",
                "for_statement",
                "enhanced_for_statement",
                "while_statement",
                "do_statement",
                "catch_clause",
                "switch_label",
                "ternary_expression",
            ]),
            boolean_operator_kind: None,
            short_circuit: set(&["&&", "||"]),
        },
        // --- c -------------------------------------------------------------
        "c" => LangSpec {
            function_kinds: set(&["function_definition"]),
            decision_kinds: set(&[
                "if_statement",
                "for_statement",
                "while_statement",
                "do_statement",
                "case_statement",
                "conditional_expression",
                "preproc_if",
                "preproc_ifdef",
                "preproc_elif",
            ]),
            boolean_operator_kind: None,
            short_circuit: set(&["&&", "||"]),
        },
        _ => return None,
    })
}

fn language_for(ext: &str) -> Option<(&'static str, Language)> {
    Some(match ext {
        "py" => ("python", tree_sitter_python::LANGUAGE.into()),
        "ts" | "js" => (
            "typescript",
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        ),
        "tsx" | "jsx" => ("typescript", tree_sitter_typescript::LANGUAGE_TSX.into()),
        "php" => ("php", tree_sitter_php::LANGUAGE_PHP.into()),
        "sh" | "bash" | "zsh" => ("bash", tree_sitter_bash::LANGUAGE.into()),
        "lua" => ("lua", tree_sitter_lua::LANGUAGE.into()),
        "go" => ("go", tree_sitter_go::LANGUAGE.into()),
        "rs" => ("rust", tree_sitter_rust::LANGUAGE.into()),
        "rb" => ("ruby", tree_sitter_ruby::LANGUAGE.into()),
        "java" => ("java", tree_sitter_java::LANGUAGE.into()),
        "c" | "h" => ("c", tree_sitter_c::LANGUAGE.into()),
        _ => return None,
    })
}

/// Resolve a path to a CCN language id + tree-sitter `Language`, including
/// the extensionless-shell heuristic. Public so `file_facts` can parse ONCE
/// and feed the tree to both this module and `extraction` (the single-parse
/// optimization — eliminates the CCN/extraction double-parse without any
/// algorithm change).
pub fn lang_for_path(path: &str) -> Option<(&'static str, Language)> {
    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
        .or_else(|| {
            Path::new(path)
                .file_name()
                .and_then(|f| f.to_str())
                .and_then(|f| {
                    let f = f.trim_start_matches('.');
                    if f.ends_with("rc") || f == "profile" {
                        Some("sh".to_string())
                    } else {
                        None
                    }
                })
        })?;
    language_for(&ext)
}

/// Per-function CCN facts for a source file across all ten languages.
/// None for genuinely unsupported extensions (caller falls back to scc).
pub fn analyze(source: &[u8], path: &str) -> Option<Vec<FunctionFact>> {
    let (lang_name, language) = lang_for_path(path)?;
    let mut parser = Parser::new();
    if parser.set_language(&language).is_err() {
        return Some(vec![]);
    }
    let tree = parser.parse(source, None)?;
    Some(facts_from_tree(&tree, source, lang_name))
}

/// CCN facts from an already-parsed tree. Decoupled from the parse so the
/// tree can be shared with `extraction` (the single-parse optimization).
/// `lang_name` must be the id `lang_for_path` returned for this file (same
/// grammar that produced `tree`).
pub fn facts_from_tree(
    tree: &tree_sitter::Tree,
    source: &[u8],
    lang_name: &str,
) -> Vec<FunctionFact> {
    let spec = match spec_for(lang_name) {
        Some(s) => s,
        None => return vec![],
    };
    let mut facts = Vec::new();
    for fnode in iter_functions(tree.root_node(), &spec.function_kinds) {
        let ccn = 1 + count_decision_nodes(fnode, &spec);
        let start = fnode.start_position().row as i64 + 1;
        let end = fnode.end_position().row as i64 + 1;
        facts.push(FunctionFact {
            name: qualified_name(fnode, source, lang_name, &spec.function_kinds),
            start_line: start,
            nloc: end - start + 1,
            cyclomatic_complexity: ccn,
        });
    }
    facts
}

/// LIFO-stack walk over all function nodes, including nested ones.
fn iter_functions<'a>(
    root: Node<'a>,
    function_kinds: &HashSet<&'static str>,
) -> Vec<Node<'a>> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if function_kinds.contains(node.kind()) {
            out.push(node);
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            stack.push(child);
        }
    }
    out
}

/// Count decision nodes in one function body: branch statements, boolean
/// operators, and short-circuit operators. Stops at nested fn boundaries.
fn count_decision_nodes(fn_node: Node, spec: &LangSpec) -> i64 {
    let mut count: i64 = 0;
    let mut cursor = fn_node.walk();
    let mut stack: Vec<Node> = fn_node.children(&mut cursor).collect();
    while let Some(node) = stack.pop() {
        if node.id() != fn_node.id()
            && spec.function_kinds.contains(node.kind())
        {
            continue;
        }
        // A decision point is a named grammar construct. Some grammars
        // (notably tree-sitter-ruby) emit the keyword token (`if`, `for`,
        // …) with the SAME `kind()` string as the enclosing statement
        // node; the token is anonymous, the statement node is named.
        // Gating on `is_named()` counts the statement once and ignores the
        // duplicate keyword token. For every other language the decision
        // kinds are already named rule nodes, so this is a no-op.
        if node.is_named() && spec.decision_kinds.contains(node.kind()) {
            count += 1;
        } else if let Some(bok) = spec.boolean_operator_kind {
            if node.kind() == bok {
                let mut c = node.walk();
                count += node
                    .children(&mut c)
                    .filter(|ch| ch.kind() == "and" || ch.kind() == "or")
                    .count() as i64;
            }
        } else if !spec.short_circuit.is_empty() {
            // Short-circuit operators are anonymous tokens whose parent
            // node kind differs by grammar: `binary_expression` in
            // TS/PHP/Rust/…, but a `list` node in bash. Scan every node's
            // direct children — each `&&`/`||`/`??` token is a child of
            // exactly one node, so each is counted exactly once.
            let mut c = node.walk();
            for child in node.children(&mut c) {
                if spec.short_circuit.contains(child.kind()) {
                    count += 1;
                }
            }
        }
        let mut c = node.walk();
        for child in node.children(&mut c) {
            stack.push(child);
        }
    }
    count
}

/// The bare name of one function node, per language.
fn bare_name(node: Node, source: &[u8], lang: &str) -> String {
    match lang {
        "python" => {
            if node.kind() == "lambda" {
                return "<lambda>".into();
            }
        }
        "ruby" => {
            if matches!(node.kind(), "do_block" | "block" | "lambda") {
                return "<block>".into();
            }
        }
        "rust" => {
            if node.kind() == "closure_expression" {
                return "<closure>".into();
            }
        }
        "go" => {
            if node.kind() == "func_literal" {
                return "<func>".into();
            }
        }
        "typescript" => {
            if let Some(n) = node.child_by_field_name("name") {
                if let Ok(t) = n.utf8_text(source) {
                    return t.to_string();
                }
            }
            if let Some(parent) = node.parent() {
                if matches!(
                    parent.kind(),
                    "variable_declarator" | "pair" | "assignment_expression"
                ) {
                    if let Some(named) = parent
                        .child_by_field_name("name")
                        .or_else(|| parent.child_by_field_name("key"))
                    {
                        if let Ok(t) = named.utf8_text(source) {
                            return t.to_string();
                        }
                    }
                }
            }
            return "<anonymous>".into();
        }
        "java" => {
            if node.kind() == "lambda_expression" {
                return "<lambda>".into();
            }
        }
        _ => {}
    }
    node.child_by_field_name("name")
        .and_then(|n| n.utf8_text(source).ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "<anonymous>".into())
}

/// Parent-qualified name: `outermost.….inner`, joining enclosing function
/// names with `.`.
/// Class/trait/interface/enum container node kinds per language. A function
/// directly enclosed by one of these is a *method* and is qualified
/// `ClassName::method` (e.g. PHP `JobServiceProvider::register`). Nested
/// *functions* stay `parent.inner` (e.g. `_walk_history._flush_co`).
fn class_kinds(lang: &str) -> &'static [&'static str] {
    match lang {
        "php" => &[
            "class_declaration",
            "trait_declaration",
            "interface_declaration",
            "enum_declaration",
        ],
        "python" => &["class_definition"],
        "typescript" => &[
            "class_declaration",
            "abstract_class_declaration",
            "class",
            "interface_declaration",
        ],
        "java" => &["class_declaration", "interface_declaration", "enum_declaration"],
        "ruby" => &["class", "module", "singleton_class"],
        _ => &[],
    }
}

/// Name of a class-like container node (its `name` field).
fn container_name(node: Node, source: &[u8]) -> Option<String> {
    node.child_by_field_name("name")
        .and_then(|n| n.utf8_text(source).ok())
        .map(|s| s.to_string())
}

fn qualified_name(
    node: Node,
    source: &[u8],
    lang: &str,
    function_kinds: &HashSet<&'static str>,
) -> String {
    let classes = class_kinds(lang);
    // Build the scope chain from the node up: each ancestor that is a
    // function contributes a `.`-joined segment; a class container
    // contributes a `::`-joined segment. Emitting order is outermost→inner.
    // Represent as (segment, joined_by_double_colon) so we can compose the
    // mixed `Class::method` / `outer.inner` shape.
    let mut segments: Vec<(String, bool)> = vec![(bare_name(node, source, lang), false)];
    let mut cur = node.parent();
    while let Some(p) = cur {
        if classes.contains(&p.kind()) {
            if let Some(cn) = container_name(p, source) {
                segments.push((cn, true));
            }
        } else if function_kinds.contains(p.kind()) {
            segments.push((bare_name(p, source, lang), false));
        }
        cur = p.parent();
    }
    segments.reverse();
    // Compose: join each segment to the previous with `::` when the CURRENT
    // segment was contributed by a class container, else `.`.
    let mut out = String::new();
    for (i, (seg, is_class)) in segments.iter().enumerate() {
        if i == 0 {
            out.push_str(seg);
        } else if *is_class {
            out.push_str("::");
            out.push_str(seg);
        } else {
            // The separator before a member is `::` if the immediately
            // preceding segment was a class, else `.`.
            let prev_is_class = segments[i - 1].1;
            out.push_str(if prev_is_class { "::" } else { "." });
            out.push_str(seg);
        }
    }
    out
}
