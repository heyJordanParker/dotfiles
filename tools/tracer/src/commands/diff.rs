//! `trace diff` — files (or symbols) changed between HEAD and a base ref.
//! Per-file mode ranks the changed set most-load-bearing first (direct
//! dependents, then ccn); the per-symbol mode (`--symbols`) diffs
//! module-level exports against the base blob via the tree-sitter
//! extractor. CCN is AST-derived.

use crate::{architecture, cache, file_facts, passive_context};
use anyhow::Result;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

pub const DEFAULT_BASE: &str = "origin/development";

/// git diff --name-status codes → display labels. R100/C075 collapse to the
/// letter only (handled by the caller taking `code[0]`).
fn status_label(kind: char) -> String {
    match kind {
        'A' => "added".into(),
        'M' => "modified".into(),
        'D' => "deleted".into(),
        'R' => "renamed".into(),
        'C' => "copied".into(),
        'T' => "type-changed".into(),
        other => other.to_lowercase().to_string(),
    }
}

#[derive(Clone)]
struct Change {
    status: String,
    path: String,
    rename_from: Option<String>,
}

/// Verify the base ref resolves; hard-fail + exit 2 if it doesn't.
fn verify_base_ref(repo_root: &Path, base: &str) {
    let ok = Command::new("git")
        .args([
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{base}^{{commit}}"),
        ])
        .current_dir(repo_root)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !ok {
        eprintln!(
            "Error: base ref '{base}' not found in this repository. \
             Pass --base <ref> with a ref that exists \
             (e.g. main, origin/main, a SHA)."
        );
        std::process::exit(2);
    }
}

/// Merge base of HEAD and `base`; exit 2 when histories are disjoint.
fn merge_base(repo_root: &Path, base: &str) -> String {
    let out = Command::new("git")
        .args(["merge-base", base, "HEAD"])
        .current_dir(repo_root)
        .output();
    match out {
        Ok(o) if o.status.success() => {
            String::from_utf8_lossy(&o.stdout).trim().to_string()
        }
        _ => {
            eprintln!("Error: no common ancestor between HEAD and '{base}'.");
            std::process::exit(2);
        }
    }
}

/// Changed files vs the merge base via `git diff --name-status`.
fn name_status(repo_root: &Path, merge_base: &str) -> Vec<Change> {
    let out = Command::new("git")
        .args([
            "diff",
            "--name-status",
            "-M",
            &format!("{merge_base}..HEAD"),
        ])
        .current_dir(repo_root)
        .output();
    let stdout = match out {
        Ok(o) if o.status.success() => {
            String::from_utf8_lossy(&o.stdout).to_string()
        }
        Ok(o) => {
            eprintln!(
                "Error: git diff failed: {}",
                String::from_utf8_lossy(&o.stderr).trim()
            );
            std::process::exit(2);
        }
        Err(e) => {
            eprintln!("Error: git diff failed: {e}");
            std::process::exit(2);
        }
    };

    let mut changes = Vec::new();
    for line in stdout.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let tokens: Vec<&str> = line.split('\t').collect();
        let raw_status = tokens[0];
        let kind = raw_status.chars().next().unwrap_or('?');
        let label = status_label(kind);
        if (kind == 'R' || kind == 'C') && tokens.len() >= 3 {
            changes.push(Change {
                status: label,
                path: tokens[2].to_string(),
                rename_from: Some(tokens[1].to_string()),
            });
        } else if tokens.len() >= 2 {
            changes.push(Change {
                status: label,
                path: tokens[1].to_string(),
                rename_from: None,
            });
        }
    }
    changes
}

/// Direct module-level dependents of `relative_path`. Counts edges that
/// resolved straight to the owning module *and* edges that resolved to a
/// symbol living in that module's file (`from X import Y` — the dominant
/// Python form), via the owner-expansion in `architecture::dependents_of`.
/// Without that widening the load-bearing ranking sees zero dependents for
/// every from-imported file and the ordering is meaningless.
fn direct_dependent_count(graph: &architecture::Graph, relative_path: &str) -> i64 {
    let module_id = match graph.file_to_module_id.get(relative_path) {
        Some(m) => m,
        None => return 0,
    };
    architecture::dependents_of(graph, module_id).len() as i64
}

fn file_row(
    repo_root: &Path,
    graph: &architecture::Graph,
    change: &Change,
) -> Value {
    let abs_path = repo_root.join(&change.path);
    let facts = if abs_path.is_file() {
        file_facts::get(&abs_path, repo_root, None)
    } else {
        None
    };

    let mut direct = direct_dependent_count(graph, &change.path);
    if let Some(rf) = &change.rename_from {
        direct = direct.max(direct_dependent_count(graph, rf));
    }

    json!({
        "path": change.path,
        "status": change.status,
        "rename_from": change.rename_from,
        "language": facts.as_ref().and_then(|f| f.language.clone()),
        "cyclomatic_complexity_total":
            facts.as_ref().map(|f| f.cyclomatic_complexity_total).unwrap_or(0),
        "rank": facts.as_ref().map(|f| f.rank.clone()).unwrap_or_else(|| "absent".into()),
        "loc": facts.as_ref().map(|f| f.loc).unwrap_or(0),
        "direct_dependents": direct,
        "present_in": facts.as_ref().map(|f| f.present_in.clone()).unwrap_or_default(),
        "passive_context": facts.as_ref().map(|f| passive_context::render(f, None)),
    })
}

fn load_bearing_key(row: &Value) -> (i64, i64) {
    (
        row["direct_dependents"].as_i64().unwrap_or(0),
        row["cyclomatic_complexity_total"].as_i64().unwrap_or(0),
    )
}

fn emit_file_mode(
    repo_root: &Path,
    base: &str,
    merge_base: &str,
    changes: &[Change],
    as_json: bool,
) -> Result<Value> {
    let graph = architecture::get(repo_root);
    let mut rows: Vec<Value> =
        changes.iter().map(|c| file_row(repo_root, &graph, c)).collect();
    // Stable sort by load-bearing key, descending.
    rows.sort_by(|a, b| load_bearing_key(b).cmp(&load_bearing_key(a)));

    let payload = json!({
        "base": base,
        "merge_base": merge_base,
        "granularity": "file",
        "file_count": rows.len(),
        "files": rows,
    });

    if as_json {
        return Ok(payload);
    }

    let files = payload["files"].as_array().cloned().unwrap_or_default();
    let mb_short: String = merge_base.chars().take(12).collect();
    println!(
        "Diff base={base}  merge_base={mb_short}  files={}",
        files.len()
    );
    if files.is_empty() {
        println!("(no files differ between HEAD and base)");
        return Ok(payload);
    }
    println!();
    println!(
        "  {:<3} {:<10} {:>6}  {:>5}  {:<8}  path",
        "#", "status", "direct", "ccn", "rank"
    );
    for (index, row) in files.iter().enumerate() {
        println!(
            "  {:<3} {:<10} {:>6}  {:>5}  {:<8}  {}",
            index + 1,
            row["status"].as_str().unwrap_or(""),
            row["direct_dependents"].as_i64().unwrap_or(0),
            row["cyclomatic_complexity_total"].as_i64().unwrap_or(0),
            row["rank"].as_str().unwrap_or(""),
            row["path"].as_str().unwrap_or(""),
        );
        if let Some(rf) = row["rename_from"].as_str() {
            println!("      renamed from: {rf}");
        }
        if let Some(pc) = row["passive_context"].as_str() {
            println!("      {pc}");
        }
    }
    Ok(payload)
}

// --- Symbol mode --------------------------------------------------------

fn head_exports(path: &Path, repo_root: &Path) -> Vec<(String, String, i64)> {
    if !path.is_file() {
        return vec![];
    }
    match file_facts::get(path, repo_root, None).and_then(|f| f.extraction) {
        Some(e) => e
            .exports
            .iter()
            .map(|x| (x.name.clone(), x.kind.clone(), x.line))
            .collect(),
        None => vec![],
    }
}

fn base_exports(
    repo_root: &Path,
    merge_base: &str,
    relative_path: &str,
) -> Vec<(String, String, i64)> {
    let out = Command::new("git")
        .args(["show", &format!("{merge_base}:{relative_path}")])
        .current_dir(repo_root)
        .output();
    let source = match out {
        Ok(o) if o.status.success() => o.stdout,
        _ => return vec![],
    };
    match crate::extraction::extract(&source, relative_path) {
        Some(e) => e
            .exports
            .iter()
            .map(|x| (x.name.clone(), x.kind.clone(), x.line))
            .collect(),
        None => vec![],
    }
}

fn symbol_row(
    graph: &architecture::Graph,
    relative_path: &str,
    name: &str,
    kind: &str,
    line: i64,
    state: &str,
) -> Value {
    let node_id = format!("{relative_path}::{name}");
    let direct = match graph.nodes.get(&node_id) {
        Some(n) => architecture::dependents_of(graph, &n.id).len() as i64,
        None => 0,
    };
    json!({
        "state": state,
        "name": name,
        "kind": kind,
        "source_file": relative_path,
        "line": line,
        "direct_dependents": direct,
    })
}

fn symbol_rows_for_change(
    repo_root: &Path,
    graph: &architecture::Graph,
    merge_base: &str,
    change: &Change,
) -> Vec<Value> {
    let head_path = repo_root.join(&change.path);
    let head = head_exports(&head_path, repo_root);
    let pre_path = change.rename_from.clone().unwrap_or_else(|| change.path.clone());
    let base = base_exports(repo_root, merge_base, &pre_path);

    // Index exports by (name, kind) with insertion-ordered, last-value-wins
    // semantics: a duplicate (name, kind) collapses to ONE entry at the
    // FIRST occurrence's position holding the LAST value. This exact
    // behavior is load-bearing — without it, tied entries (same
    // dependents + state-weight) would order differently after the stable
    // load-bearing sort whenever a file has duplicate (name, kind) exports.
    let head_by = InsertionOrderedMap::from_exports(&head);
    let base_by = InsertionOrderedMap::from_exports(&base);

    let mut rows = Vec::new();
    for ((name, kind), line) in head_by.items() {
        let state = match base_by.get(&(name.clone(), kind.clone())) {
            None => "added",
            Some(bl) if bl != line => "changed",
            Some(_) => "unchanged",
        };
        if state == "unchanged" {
            continue;
        }
        rows.push(symbol_row(graph, &change.path, name, kind, *line, state));
    }
    for ((name, kind), line) in base_by.items() {
        if head_by.contains(&(name.clone(), kind.clone())) {
            continue;
        }
        rows.push(symbol_row(graph, &pre_path, name, kind, *line, "removed"));
    }
    rows
}

/// (name, kind) → line index with first-insertion ordering: keys keep
/// their first-insertion position, re-inserting a key overwrites its value
/// without moving it, and iteration is first-insertion order.
struct InsertionOrderedMap {
    order: Vec<(String, String)>,
    map: HashMap<(String, String), i64>,
}

impl InsertionOrderedMap {
    fn from_exports(exports: &[(String, String, i64)]) -> Self {
        let mut d = InsertionOrderedMap {
            order: Vec::new(),
            map: HashMap::new(),
        };
        for (n, k, l) in exports {
            let key = (n.clone(), k.clone());
            if !d.map.contains_key(&key) {
                d.order.push(key.clone());
            }
            d.map.insert(key, *l);
        }
        d
    }
    fn items(&self) -> impl Iterator<Item = (&(String, String), &i64)> {
        self.order.iter().map(move |k| (k, &self.map[k]))
    }
    fn get(&self, key: &(String, String)) -> Option<&i64> {
        self.map.get(key)
    }
    fn contains(&self, key: &(String, String)) -> bool {
        self.map.contains_key(key)
    }
}

fn symbol_load_bearing_key(row: &Value) -> (i64, i64) {
    let weight = match row["state"].as_str().unwrap_or("") {
        "removed" => 2,
        "added" => 1,
        _ => 0,
    };
    (row["direct_dependents"].as_i64().unwrap_or(0), weight)
}

fn emit_symbol_mode(
    repo_root: &Path,
    base: &str,
    merge_base: &str,
    changes: &[Change],
    as_json: bool,
) -> Result<Value> {
    let graph = architecture::get(repo_root);
    let mut rows: Vec<Value> = Vec::new();
    for change in changes {
        rows.extend(symbol_rows_for_change(repo_root, &graph, merge_base, change));
    }
    rows.sort_by(|a, b| symbol_load_bearing_key(b).cmp(&symbol_load_bearing_key(a)));

    let payload = json!({
        "base": base,
        "merge_base": merge_base,
        "granularity": "symbol",
        "symbol_count": rows.len(),
        "symbols": rows,
    });

    if as_json {
        return Ok(payload);
    }

    let symbols = payload["symbols"].as_array().cloned().unwrap_or_default();
    let mb_short: String = merge_base.chars().take(12).collect();
    println!(
        "Diff base={base}  merge_base={mb_short}  symbols={}",
        symbols.len()
    );
    if symbols.is_empty() {
        println!("(no symbol-level changes detected in supported languages)");
        return Ok(payload);
    }
    println!();
    println!(
        "  {:<8} {:>6}  {:<10}  symbol @ source",
        "state", "direct", "kind"
    );
    for row in &symbols {
        let location = match row["line"].as_i64() {
            Some(l) => format!("{}:{}", row["source_file"].as_str().unwrap_or(""), l),
            None => row["source_file"].as_str().unwrap_or("").to_string(),
        };
        println!(
            "  {:<8} {:>6}  {:<10}  {} @ {}",
            row["state"].as_str().unwrap_or(""),
            row["direct_dependents"].as_i64().unwrap_or(0),
            row["kind"].as_str().unwrap_or(""),
            row["name"].as_str().unwrap_or(""),
            location,
        );
    }
    Ok(payload)
}

pub fn run(base: &str, symbol_mode: bool, as_json: bool) -> Result<Value> {
    let repo_root = cache::repo_root_for(Path::new("."));
    verify_base_ref(&repo_root, base);
    let mb = merge_base(&repo_root, base);
    let changes = name_status(&repo_root, &mb);

    if symbol_mode {
        emit_symbol_mode(&repo_root, base, &mb, &changes, as_json)
    } else {
        emit_file_mode(&repo_root, base, &mb, &changes, as_json)
    }
}
