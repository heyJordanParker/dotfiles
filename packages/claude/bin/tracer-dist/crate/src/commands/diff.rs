//! `trace diff` — files (or symbols) changed between HEAD and a base ref.
//! Per-file mode ranks the changed set most-load-bearing first (direct
//! dependents, then ccn); the per-symbol mode (`--symbols`) diffs
//! module-level exports against the base blob via the tree-sitter
//! extractor. CCN is AST-derived.

use crate::{cache, file_facts, passive_context, relations};
use anyhow::Result;
use rayon::prelude::*;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// git diff --name-status codes → display labels. R100/C075 collapse to the
/// letter only (handled by the caller taking `code[0]`).
pub fn status_label(kind: char) -> String {
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

/// What `diff` is comparing. With no `--base` the answer is everything that
/// differs from HEAD right now — staged, unstaged, and untracked files —
/// because that is the question an agent asks before it commits, and plain
/// `git diff` answers only a third of it (`git diff` unstaged, `--cached`
/// staged, `HEAD` both but no new files). With `--base <ref>` it is the
/// committed difference from that ref, which is the review question.
pub enum Scope {
    Worktree,
    Base { name: String, merge_base: String },
}

impl Scope {
    /// The revision argument `git diff` takes for this scope.
    fn revision(&self) -> String {
        match self {
            Scope::Worktree => "HEAD".to_string(),
            Scope::Base { merge_base, .. } => format!("{merge_base}..HEAD"),
        }
    }

    fn label(&self) -> &str {
        match self {
            Scope::Worktree => "worktree",
            Scope::Base { name, .. } => name,
        }
    }
}

/// Untracked files, each reported as `added` — their whole content is a
/// change, and a diff that hides new files hides the newest work in the tree.
fn untracked(repo_root: &Path, pathspec: Option<&str>) -> Vec<Change> {
    let mut cmd = Command::new("git");
    cmd.args(["ls-files", "--others", "--exclude-standard"]);
    if let Some(p) = pathspec {
        cmd.args(["--", p]);
    }
    let out = cmd.current_dir(repo_root).output();
    match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| Change {
                status: "added".to_string(),
                path: l.to_string(),
                rename_from: None,
            })
            .collect(),
        _ => vec![],
    }
}

/// Changed files in `scope` via `git diff --name-status`.
fn name_status(repo_root: &Path, scope: &Scope, pathspec: Option<&str>) -> Vec<Change> {
    let mut cmd = Command::new("git");
    cmd.args(["diff", "--name-status", "-M", &scope.revision()]);
    if let Some(p) = pathspec {
        cmd.args(["--", p]);
    }
    let out = cmd.current_dir(repo_root).output();
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

/// Direct module-level dependents of `relative_path`, straight off the import
/// inversion. It counts edges that resolved to the owning module *and* edges
/// that resolved to a symbol living in that module's file (`from X import Y`
/// — the dominant Python form), because `relations::resolve_imports` already
/// points a from-import at the symbol's own file when one declares it.
/// Without that, the load-bearing ranking sees zero dependents for every
/// from-imported file and the ordering is meaningless.
fn direct_dependent_count(index: &relations::Relations, relative_path: &str) -> i64 {
    index.importers_of(relative_path).len() as i64
}

/// Per-file budget for the rendered hunks. A generated lock file rewritten
/// whole is tens of thousands of lines that bury every other file in the
/// answer; the cut is at a line boundary and says so, naming the command that
/// returns that file's diff alone.
const DIFF_LINES_BUDGET: usize = 400;

/// The changed lines for one file: unified hunks, headers stripped, so what
/// remains is `@@` markers plus `-`/`+`/context lines.
///
/// An untracked file has no blob to diff against, so its whole content is the
/// change and every line is rendered as an addition — the same shape the
/// reader already understands.
fn changed_lines(repo_root: &Path, scope: &Scope, change: &Change) -> Option<String> {
    let raw = if change.status == "added" && matches!(scope, Scope::Worktree) {
        let content = std::fs::read(repo_root.join(&change.path)).ok()?;
        let text = String::from_utf8_lossy(&content);
        let mut out = format!("@@ +1,{} @@\n", text.lines().count());
        for line in text.lines() {
            out.push('+');
            out.push_str(line);
            out.push('\n');
        }
        out
    } else {
        let out = Command::new("git")
            .args([
                "diff",
                "--unified=3",
                "--no-color",
                &scope.revision(),
                "--",
                &change.path,
            ])
            .current_dir(repo_root)
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .skip_while(|l| !l.starts_with("@@"))
            .map(|l| format!("{l}\n"))
            .collect()
    };
    if raw.trim().is_empty() {
        return None;
    }
    let total = raw.lines().count();
    if total <= DIFF_LINES_BUDGET {
        return Some(raw);
    }
    let mut kept: String = raw
        .lines()
        .take(DIFF_LINES_BUDGET)
        .map(|l| format!("{l}\n"))
        .collect();
    kept.push_str(&format!(
        "[trimmed at {DIFF_LINES_BUDGET} of {total} diff lines \u{00b7} whole file: trace diff {}]\n",
        change.path
    ));
    Some(kept)
}

fn file_row(
    index: &relations::Relations,
    change: &Change,
    facts: Option<&file_facts::FileFacts>,
    lines: Option<String>,
) -> Value {
    let mut direct = direct_dependent_count(index, &change.path);
    if let Some(rf) = &change.rename_from {
        direct = direct.max(direct_dependent_count(index, rf));
    }

    json!({
        "path": change.path,
        "status": change.status,
        "rename_from": change.rename_from,
        "lines": lines,
        "language": facts.and_then(|f| f.language.clone()),
        "cyclomatic_complexity_total":
            facts.map(|f| f.cyclomatic_complexity_total).unwrap_or(0),
        "rank": facts.map(|f| f.rank.clone()).unwrap_or_else(|| "absent".into()),
        "loc": facts.map(|f| f.loc).unwrap_or(0),
        "direct_dependents": direct,
        "present_in": facts.map(|f| f.present_in.clone()).unwrap_or_default(),
        "passive_context": facts.map(|f| passive_context::render(f, None)),
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
    scope: &Scope,
    changes: &[Change],
    as_json: bool,
) -> Result<Value> {
    let index = relations::get(repo_root);
    // Facts through the bulk resolver and one `git diff` per file in
    // parallel, a chunk at a time: per-file `get` rebuilt the git and scc
    // maps once per changed file, and the diffs are process-bound.
    let mut rows: Vec<Value> = Vec::with_capacity(changes.len());
    for chunk in changes.chunks(file_facts::RESOLVE_CHUNK) {
        let paths: Vec<PathBuf> = chunk.iter().map(|c| repo_root.join(&c.path)).collect();
        let facts = file_facts::get_batch(&paths, repo_root);
        let lines: Vec<Option<String>> = chunk
            .par_iter()
            .map(|c| changed_lines(repo_root, scope, c))
            .collect();
        for (change, lines) in chunk.iter().zip(lines) {
            rows.push(file_row(&index, change, facts.get(&change.path), lines));
        }
    }
    // Stable sort by load-bearing key, descending.
    rows.sort_by(|a, b| load_bearing_key(b).cmp(&load_bearing_key(a)));

    // The shoulder is per-file enrichment: it leaves the row for `context`,
    // keyed by the path the row names.
    let mut shoulders = serde_json::Map::new();
    for row in rows.iter_mut() {
        let path = row["path"].as_str().unwrap_or_default().to_string();
        let shoulder = row
            .as_object_mut()
            .and_then(|o| o.remove("passive_context"))
            .unwrap_or(Value::Null);
        shoulders.insert(path, json!({"shoulder": shoulder}));
    }

    let payload = crate::output::document(
        json!({"base": scope.label(), "granularity": "file"}),
        json!({
            "merge_base": match scope {
                Scope::Base { merge_base, .. } => Value::String(merge_base.clone()),
                Scope::Worktree => Value::Null,
            },
            "files": Value::Object(shoulders),
        }),
        json!(rows),
        json!({"files": rows.len()}),
    );

    if as_json {
        return Ok(payload);
    }

    let files = rows.clone();
    match scope {
        Scope::Worktree => println!(
            "Diff base=worktree (staged + unstaged + untracked vs HEAD)  files={}",
            files.len()
        ),
        Scope::Base { name, merge_base } => {
            let mb_short: String = merge_base.chars().take(12).collect();
            println!(
                "Diff base={name}  merge_base={mb_short}  files={}",
                files.len()
            );
        }
    }
    if files.is_empty() {
        println!("(nothing differs)");
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
        if let Some(shoulder) = payload["context"]["files"][row["path"].as_str().unwrap_or("")]
            ["shoulder"]
            .as_str()
        {
            println!("      {shoulder}");
        }
        if let Some(lines) = row["lines"].as_str() {
            for line in lines.lines() {
                println!("      {line}");
            }
        }
    }
    Ok(payload)
}

// --- Symbol mode --------------------------------------------------------

type Exports = Vec<(String, String, i64)>;

/// Head-side exports for every changed path, resolved through the bulk
/// resolver so the git map, the scc map, and the mtime index are read once
/// for the whole diff instead of once per file. Chunked so a diff spanning
/// thousands of files never holds every file's extraction at once.
fn head_exports(repo_root: &Path, changes: &[Change]) -> HashMap<String, Exports> {
    let mut out: HashMap<String, Exports> = HashMap::with_capacity(changes.len());
    for chunk in changes.chunks(file_facts::RESOLVE_CHUNK) {
        let paths: Vec<PathBuf> = chunk.iter().map(|c| repo_root.join(&c.path)).collect();
        let facts = file_facts::get_batch(&paths, repo_root);
        for change in chunk {
            let Some(f) = facts.get(&change.path) else {
                continue;
            };
            let Some(e) = &f.extraction else { continue };
            out.insert(
                change.path.clone(),
                e.exports
                    .iter()
                    .map(|x| (x.name.clone(), x.kind.clone(), x.line))
                    .collect(),
            );
        }
    }
    out
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
    index: &relations::Relations,
    relative_path: &str,
    name: &str,
    kind: &str,
    line: i64,
    state: &str,
) -> Value {
    // A symbol's blast radius is the other files that name it, straight off
    // the symbol inversion — never its file's importer count, which would
    // give every symbol in a widely-imported file the same rank.
    //
    // Read, not resolved. This is a ranking key over every symbol a diff
    // touches, and resolving each one's use sites made `diff --symbols` take
    // 75s on laravel-framework; the inversion answers it without opening a
    // file. The count is by name, so it is an upper bound on the resolved
    // callers `trace callers <name>` reports.
    let direct = if index.defined_in(name).any(|f| f == relative_path) {
        index.used_in(name).filter(|f| *f != relative_path).count() as i64
    } else {
        0
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
    index: &relations::Relations,
    change: &Change,
    head: &Exports,
    base: &Exports,
) -> Vec<Value> {
    let pre_path = change.rename_from.clone().unwrap_or_else(|| change.path.clone());

    // Index exports by (name, kind) with insertion-ordered, last-value-wins
    // semantics: a duplicate (name, kind) collapses to ONE entry at the
    // FIRST occurrence's position holding the LAST value. This exact
    // behavior is load-bearing — without it, tied entries (same
    // dependents + state-weight) would order differently after the stable
    // load-bearing sort whenever a file has duplicate (name, kind) exports.
    let head_by = InsertionOrderedMap::from_exports(head);
    let base_by = InsertionOrderedMap::from_exports(base);

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
        rows.push(symbol_row(index, &change.path, name, kind, *line, state));
    }
    for ((name, kind), line) in base_by.items() {
        if head_by.contains(&(name.clone(), kind.clone())) {
            continue;
        }
        rows.push(symbol_row(index, &pre_path, name, kind, *line, "removed"));
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
    let index = relations::get(repo_root);
    let head = head_exports(repo_root, changes);
    // One `git show` per changed path, run in parallel: the base side is
    // process-bound, and serializing it made a 1,000-file diff wait on a
    // thousand round trips. Chunked, so only one chunk's base blobs are
    // resident however large the diff.
    let empty: Exports = Vec::new();
    let mut rows: Vec<Value> = Vec::new();
    for chunk in changes.chunks(file_facts::RESOLVE_CHUNK) {
        let base_side: Vec<Exports> = chunk
            .par_iter()
            .map(|c| {
                let pre = c.rename_from.as_deref().unwrap_or(&c.path);
                base_exports(repo_root, merge_base, pre)
            })
            .collect();
        for (change, base) in chunk.iter().zip(&base_side) {
            rows.extend(symbol_rows_for_change(
                &index,
                change,
                head.get(&change.path).unwrap_or(&empty),
                base,
            ));
        }
    }
    rows.sort_by(|a, b| symbol_load_bearing_key(b).cmp(&symbol_load_bearing_key(a)));

    let payload = crate::output::document(
        json!({"base": base, "granularity": "symbol"}),
        json!({"merge_base": merge_base}),
        json!(rows),
        json!({"symbols": rows.len()}),
    );

    if as_json {
        return Ok(payload);
    }

    let symbols = rows.clone();
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

pub fn run(
    path: Option<&str>,
    base: Option<&str>,
    symbol_mode: bool,
    as_json: bool,
) -> Result<Value> {
    let here = Path::new(".");
    let repo_root = cache::worktree_root_for(here).unwrap_or_else(|| cache::display_root(here));

    // No `--base` means the working tree: everything that differs from HEAD
    // right now, new files included.
    let scope = match base {
        None => Scope::Worktree,
        Some(name) => {
            verify_base_ref(&repo_root, name);
            Scope::Base {
                name: name.to_string(),
                merge_base: merge_base(&repo_root, name),
            }
        }
    };

    let mut changes = name_status(&repo_root, &scope, path);
    if matches!(scope, Scope::Worktree) {
        changes.extend(untracked(&repo_root, path));
    }

    if symbol_mode {
        let mb = match &scope {
            Scope::Base { merge_base, .. } => merge_base.clone(),
            // Symbol mode diffs each file's exports against a committed blob;
            // for the working tree that blob is HEAD's.
            Scope::Worktree => "HEAD".to_string(),
        };
        emit_symbol_mode(&repo_root, scope.label(), &mb, &changes, as_json)
    } else {
        emit_file_mode(&repo_root, &scope, &changes, as_json)
    }
}
