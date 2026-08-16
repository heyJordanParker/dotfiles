//! `trace structure` — methods, properties, variables, connections.
//!
//! Symbols come from universal-ctags (broad coverage). Imports/exports come
//! from the per-file cache (tree-sitter, accurate). Per-method CCN is joined
//! by matching ctags symbol lines to the AST per-function list keyed by
//! start_line, giving per-symbol complexity where a function starts there.

use crate::commands::signatures;
use crate::{cache, ccn, file_facts};
use anyhow::Result;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use std::process::Command;

/// Human label for a raw ctags kind code.
fn kind_label(raw: &str) -> String {
    match raw {
        "c" => "class",
        "f" => "function",
        "m" => "method",
        "v" => "variable",
        "p" => "property",
        "F" => "field",
        "i" | "I" => "import",
        "n" => "namespace",
        "s" => "struct",
        "e" | "g" => "enum",
        "t" => "trait",
        "u" => "union",
        other => other,
    }
    .to_string()
}

#[derive(Clone)]
struct Symbol {
    name: String,
    kind: String,
    line: Option<i64>,
    scope: Option<String>,
    scope_kind: Option<String>,
    signature: Option<String>,
    cyclomatic_complexity: Option<i64>,
    /// Tree-sitter-extracted per-line signature info (visibility, return
    /// type, attributes/decorators, parameters, etc). Merged field-by-field
    /// into the JSON output. Built once per file in `run`, looked up here
    /// by `line`.
    extra: Option<Value>,
}

/// Symbols for a file via `universal-ctags`, in ctags `--sort=no` order.
fn ctags_symbols(path: &Path) -> Result<Vec<Symbol>> {
    let result = Command::new("ctags")
        .args([
            "--output-format=json",
            "--fields=+nezKSt",
            "--sort=no",
            "-f",
            "-",
        ])
        .arg(path)
        .output();

    let out = match result {
        Ok(o) => o,
        Err(e) => {
            anyhow::bail!("ctags failed: {e}");
        }
    };
    // A non-zero exit is checked, not just a failed spawn. On linux-x86_64 a
    // missing `ctags` does not surface as a spawn error, so an absent
    // universal-ctags produced empty stdout, zero symbols, and a silent
    // fall-through to the tree-sitter backfill below — a thinner answer with
    // nothing saying the tool never ran. linux-arm64 and mac-arm64 bail on the
    // same input. Reading the status makes every platform report it.
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let detail = stderr.trim();
        if detail.is_empty() {
            anyhow::bail!("ctags failed: {}", out.status);
        }
        anyhow::bail!("ctags failed: {} ({detail})", out.status);
    }

    let mut symbols = Vec::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        if line.trim().is_empty() {
            continue;
        }
        let entry: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let kind_raw = entry.get("kind").and_then(|x| x.as_str()).unwrap_or("");
        symbols.push(Symbol {
            name: entry
                .get("name")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
            kind: kind_label(kind_raw),
            line: entry.get("line").and_then(|x| x.as_i64()),
            scope: entry
                .get("scope")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string()),
            scope_kind: entry
                .get("scopeKind")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string()),
            signature: entry
                .get("signature")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string()),
            cyclomatic_complexity: None,
            extra: None,
        });
    }
    Ok(symbols)
}

fn symbol_to_json(s: &Symbol) -> Value {
    let mut m = Map::new();
    m.insert("name".into(), json!(s.name));
    m.insert("kind".into(), json!(s.kind));
    m.insert(
        "line".into(),
        s.line.map(|l| json!(l)).unwrap_or(Value::Null),
    );
    m.insert(
        "scope".into(),
        s.scope.as_ref().map(|x| json!(x)).unwrap_or(Value::Null),
    );
    m.insert(
        "scope_kind".into(),
        s.scope_kind
            .as_ref()
            .map(|x| json!(x))
            .unwrap_or(Value::Null),
    );
    m.insert(
        "signature".into(),
        s.signature
            .as_ref()
            .map(|x| json!(x))
            .unwrap_or(Value::Null),
    );
    if let Some(c) = s.cyclomatic_complexity {
        m.insert("cyclomatic_complexity".into(), json!(c));
    }
    if let Some(extra) = &s.extra {
        if let Some(obj) = extra.as_object() {
            for (k, v) in obj {
                // Additive merge: signature fields are new keys (visibility,
                // return_type, attributes, parameters, etc). Never overwrite
                // an existing key.
                m.entry(k.clone()).or_insert_with(|| v.clone());
            }
        }
    }
    Value::Object(m)
}

pub fn run(path: &Path, as_json: bool) -> Result<Value> {
    crate::pathval::require_file(path, "PATH");
    let p = cache::absolutize(path);
    let repo_root = cache::worktree_root_for(&p).unwrap_or_else(|| cache::display_root(&p));
    let facts = file_facts::get(&p, &repo_root, None);
    let mut symbols = ctags_symbols(&p)?;

    // universal-ctags doesn't know `.tsx`/`.jsx` and returns zero entries
    // on those files even when the file holds populated declarations.
    // Backfill from the cached tree-sitter declaration index so structure
    // matches what the architecture graph already sees for the file.
    if symbols.is_empty() {
        if let Some(f) = &facts {
            if let Some(ex) = &f.extraction {
                for d in &ex.declarations {
                    symbols.push(Symbol {
                        name: d.name.clone(),
                        kind: d.kind.clone(),
                        line: Some(d.line),
                        scope: None,
                        scope_kind: None,
                        signature: None,
                        cyclomatic_complexity: None,
                        extra: None,
                    });
                }
            }
        }
    }

    // Per-method CCN: match ctags symbol lines to the AST per-function
    // list, keyed by the function's start_line. The source is read once and
    // reused for the signature extraction pass below.
    let function_count = facts.as_ref().map(|f| f.function_count).unwrap_or(0);
    let mut by_line: BTreeMap<i64, i64> = BTreeMap::new();
    let source = std::fs::read(&p).unwrap_or_default();
    if function_count > 0 {
        if let Some(functions) = ccn::analyze(&source, &p.to_string_lossy()) {
            for f in functions {
                by_line.insert(f.start_line, f.cyclomatic_complexity);
            }
        }
    }
    // Per-symbol signatures (visibility, return types, attributes, params,
    // property hooks, class extends/implements). Matched to existing
    // ctags-found symbols by (line, name); any signature not matched is
    // backfilled as a new symbol so ctags-coverage gaps (PHP class nodes,
    // PHP 8.4 hooked properties, TSX) still surface in the output.
    let sigs = signatures::extract(&source, &p);
    let mut matched: HashSet<usize> = HashSet::new();
    for s in &mut symbols {
        let line = match s.line {
            Some(l) => l,
            None => continue,
        };
        if let Some(c) = by_line.get(&line) {
            s.cyclomatic_complexity = Some(*c);
        }
        for (i, sig) in sigs.iter().enumerate() {
            if sig.line == line && sig.name == s.name {
                s.extra = Some(sig.extra.clone());
                matched.insert(i);
                break;
            }
        }
    }
    // Backfill: any signature not matched to a ctags symbol becomes a fresh
    // symbol entry. This is what surfaces PHP class declarations and PHP
    // 8.4 hooked properties — both invisible to ctags today.
    for (i, sig) in sigs.iter().enumerate() {
        if matched.contains(&i) {
            continue;
        }
        let ccn = by_line.get(&sig.line).copied();
        symbols.push(Symbol {
            name: sig.name.clone(),
            kind: sig.kind.clone(),
            line: Some(sig.line),
            scope: None,
            scope_kind: None,
            signature: None,
            cyclomatic_complexity: ccn,
            extra: Some(sig.extra.clone()),
        });
    }
    // Keep deterministic source order across all symbols after the backfill.
    symbols.sort_by(|a, b| a.line.unwrap_or(0).cmp(&b.line.unwrap_or(0)));

    // Imports/exports from cached tree-sitter extraction.
    let mut imports: Vec<Value> = vec![];
    let mut exports: Vec<Value> = vec![];
    if let Some(f) = &facts {
        if let Some(ex) = &f.extraction {
            imports = ex
                .imports
                .iter()
                .map(|i| {
                    json!({
                        "module": i.module,
                        "symbol": i.symbol,
                        "line": i.line,
                    })
                })
                .collect();
            exports = ex
                .exports
                .iter()
                .map(|e| {
                    json!({"name": e.name, "kind": e.kind, "line": e.line})
                })
                .collect();
        }
    }

    // Group by kind, preserving the first-seen order so output follows
    // ctags `--sort=no` order.
    let mut by_kind: BTreeMap<String, Vec<Symbol>> = BTreeMap::new();
    let mut kind_order: Vec<String> = Vec::new();
    for s in &symbols {
        if !by_kind.contains_key(&s.kind) {
            kind_order.push(s.kind.clone());
        }
        by_kind.entry(s.kind.clone()).or_default().push(s.clone());
    }

    let language = facts.as_ref().and_then(|f| f.language.clone());

    let mut kinds_json = Map::new();
    // Emit kinds in first-seen order.
    for k in &kind_order {
        kinds_json.insert(
            k.clone(),
            Value::Array(by_kind[k].iter().map(symbol_to_json).collect()),
        );
    }
    let out = json!({
        "file": p.to_string_lossy(),
        "language": language.clone(),
        "imports": imports.clone(),
        "exports": exports.clone(),
        "symbols_by_kind": Value::Object(kinds_json),
        "symbol_count": symbols.len(),
    });

    if as_json {
        return Ok(out);
    }

    println!("File: {}", p.to_string_lossy());
    println!(
        "Language: {}",
        language.as_deref().unwrap_or("(unknown)")
    );
    println!(
        "Symbols: {}  Imports: {}  Exports: {}",
        symbols.len(),
        imports.len(),
        exports.len()
    );
    println!();
    if !imports.is_empty() {
        println!("Imports:");
        for i in &imports {
            let symbol_part = match i.get("symbol").and_then(|x| x.as_str()) {
                Some(s) if !s.is_empty() => format!(" -> {s}"),
                _ => String::new(),
            };
            println!(
                "  L{:<5} {}{}",
                i.get("line").and_then(|x| x.as_i64()).unwrap_or(0),
                i.get("module").and_then(|x| x.as_str()).unwrap_or(""),
                symbol_part
            );
        }
        println!();
    }
    if !exports.is_empty() {
        println!("Exports:");
        for e in &exports {
            println!(
                "  L{:<5} [{}] {}",
                e.get("line").and_then(|x| x.as_i64()).unwrap_or(0),
                e.get("kind").and_then(|x| x.as_str()).unwrap_or(""),
                e.get("name").and_then(|x| x.as_str()).unwrap_or("")
            );
        }
        println!();
    }
    // Text output iterates kinds in sorted order.
    for kind in by_kind.keys() {
        println!("{kind}s:");
        for s in &by_kind[kind] {
            let ccn_str = s
                .cyclomatic_complexity
                .map(|c| format!(" cyclomatic_complexity={c}"))
                .unwrap_or_default();
            let sig = s
                .signature
                .as_ref()
                .filter(|x| !x.is_empty())
                .map(|x| format!(" {x}"))
                .unwrap_or_default();
            let scope = s
                .scope
                .as_ref()
                .filter(|x| !x.is_empty())
                .map(|x| format!(" [in {x}]"))
                .unwrap_or_default();
            println!(
                "  L{:<5} {}{}{}{}",
                s.line.unwrap_or(0),
                s.name,
                sig,
                scope,
                ccn_str
            );
        }
        println!();
    }
    Ok(out)
}
