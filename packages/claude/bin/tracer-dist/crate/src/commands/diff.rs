//! `trace diff` — files (or symbols) changed between HEAD and a base ref.
//! Per-file mode ranks the changed set most-load-bearing first (direct
//! dependents, then ccn); the per-symbol mode (`--symbols`) diffs
//! module-level exports against the base blob via the tree-sitter
//! extractor. CCN is AST-derived.

use super::session_log;
use crate::summary::Facts;
use crate::{cache, file_facts, relations, surface};
use anyhow::Result;
use rayon::prelude::*;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

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
    let ok = crate::git_activity::git_output(
        repo_root,
        [
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{base}^{{commit}}"),
        ],
    )
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
    let out = crate::git_activity::git_output(repo_root, ["merge-base", base, "HEAD"]);
    match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
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
    Worktree { head: String },
    Base { name: String, merge_base: String },
}

impl Scope {
    /// The revision argument `git diff` takes for this scope.
    fn revision(&self) -> String {
        match self {
            Scope::Worktree { head } => head.clone(),
            Scope::Base { merge_base, .. } => format!("{merge_base}..HEAD"),
        }
    }

    fn label(&self) -> &str {
        match self {
            Scope::Worktree { .. } => "worktree",
            Scope::Base { name, .. } => name,
        }
    }
}

/// Untracked files, each reported as `added` — their whole content is a
/// change, and a diff that hides new files hides the newest work in the tree.
fn untracked(repo_root: &Path, pathspecs: &[String]) -> Vec<Change> {
    let mut args = vec![
        "ls-files".to_string(),
        "--others".to_string(),
        "--exclude-standard".to_string(),
        "--".to_string(),
    ];
    args.extend(pathspecs.iter().cloned());
    let out = crate::git_activity::git_output(repo_root, args);
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
fn name_status(repo_root: &Path, scope: &Scope, pathspecs: &[String]) -> Vec<Change> {
    let mut args = vec![
        "diff".to_string(),
        "--name-status".to_string(),
        "-M".to_string(),
        scope.revision(),
        "--".to_string(),
    ];
    args.extend(pathspecs.iter().cloned());
    let out = crate::git_activity::git_output(repo_root, args);
    let stdout = match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        Ok(o) => {
            eprintln!(
                "Error: git diff failed: {}",
                String::from_utf8_lossy(&o.stderr).trim()
            );
            std::process::exit(2);
        }
        Err(error) => {
            eprintln!("Error: git diff failed: {error}");
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

/// The changed lines for one file: unified hunks, headers stripped, so what
/// remains is `@@` markers plus `-`/`+`/context lines.
///
/// An untracked file has no blob to diff against, so its whole content is the
/// change and every line is rendered as an addition — the same shape the
/// reader already understands.
fn changed_lines(repo_root: &Path, scope: &Scope, change: &Change) -> Option<String> {
    let raw = if change.status == "added" && matches!(scope, Scope::Worktree { .. }) {
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
        let out = crate::git_activity::git_output(
            repo_root,
            [
                "diff",
                "--unified=3",
                "--no-color",
                &scope.revision(),
                "--",
                &change.path,
            ],
        )
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
    (!raw.trim().is_empty()).then_some(raw)
}

fn hunk_range(token: &str) -> Option<(i64, i64)> {
    let token = token.trim_start_matches(['-', '+']);
    let (start, count) = token.split_once(',').unwrap_or((token, "1"));
    let start = start.parse::<i64>().ok()?;
    let count = count.parse::<i64>().ok()?;
    Some((start, start + count - 1))
}

fn changed_ranges(
    repo_root: &Path,
    scope: &Scope,
    changes: &[Change],
) -> HashMap<String, (Vec<(i64, i64)>, Vec<(i64, i64)>)> {
    let mut ranges = HashMap::new();
    let mut paths = Vec::new();
    for change in changes {
        if change.status == "added" && matches!(scope, Scope::Worktree { .. }) {
            ranges.insert(change.path.clone(), (Vec::new(), vec![(1, i64::MAX)]));
        } else {
            ranges.insert(change.path.clone(), (Vec::new(), Vec::new()));
            paths.push(change.path.clone());
        }
    }
    if paths.is_empty() {
        return ranges;
    }
    let mut args = vec![
        "diff".to_string(),
        "--unified=0".to_string(),
        "-z".to_string(),
        "--no-color".to_string(),
        scope.revision(),
        "--".to_string(),
    ];
    args.extend(paths);
    let Ok(output) = crate::git_activity::git_output(repo_root, args) else {
        return ranges;
    };
    if !output.status.success() {
        return ranges;
    }
    for patch in String::from_utf8_lossy(&output.stdout)
        .split("diff --git ")
        .skip(1)
    {
        let path = patch.lines().find_map(|line| {
            line.strip_prefix("+++ b/")
                .or_else(|| line.strip_prefix("--- a/"))
        });
        let Some((base, current)) = path.and_then(|path| ranges.get_mut(path)) else {
            continue;
        };
        for line in patch.lines() {
            let Some(header) = line.strip_prefix("@@ ") else {
                continue;
            };
            let Some((ranges, _)) = header.split_once(" @@") else {
                continue;
            };
            let mut ranges = ranges.split_whitespace();
            if let Some(range) = ranges.next().and_then(hunk_range) {
                base.push(range);
            }
            if let Some(range) = ranges.next().and_then(hunk_range) {
                current.push(range);
            }
        }
    }
    ranges
}

fn declaration_rows_for_change(
    repo_root: &Path,
    scope: &Scope,
    change: &Change,
    facts: Option<&file_facts::FileFacts>,
    ranges: &(Vec<(i64, i64)>, Vec<(i64, i64)>),
) -> (
    Vec<Value>,
    Vec<surface::Row>,
    Vec<surface::Row>,
    Vec<surface::Row>,
) {
    let current = facts
        .map(|facts| surface::rows(facts, None))
        .unwrap_or_default();
    let base = match scope {
        Scope::Worktree { head } => head,
        Scope::Base { merge_base, .. } => merge_base,
    };
    let base_path = change.rename_from.as_deref().unwrap_or(&change.path);
    let base = if change.status == "added" {
        Vec::new()
    } else {
        surface::rows_at(repo_root, base, base_path)
    };
    let mut current_by_key: HashMap<(Option<String>, String), Vec<usize>> = HashMap::new();
    for (index, row) in current.iter().enumerate() {
        current_by_key
            .entry((row.container.clone(), row.name.clone()))
            .or_default()
            .push(index);
    }
    let mut offsets: HashMap<(Option<String>, String), usize> = HashMap::new();
    let mut paired = HashSet::new();
    let mut changed = Vec::new();
    let mut removed = Vec::new();
    for before in base {
        let key = (before.container.clone(), before.name.clone());
        let offset = offsets.entry(key.clone()).or_default();
        let after = current_by_key
            .get(&key)
            .and_then(|rows| rows.get(*offset))
            .copied();
        *offset += 1;
        let Some(after) = after else {
            removed.push(before);
            continue;
        };
        paired.insert(after);
        if before.header != current[after].header {
            changed.push(json!({"before": before, "after": current[after]}));
        }
    }
    let added = current
        .iter()
        .enumerate()
        .filter(|(index, _)| !paired.contains(index))
        .map(|(_, row)| row.clone())
        .collect();
    let mut touches = Vec::new();
    for (start, end) in &ranges.1 {
        let rows = if end >= start {
            facts
                .map(|facts| surface::rows(facts, Some((*start, *end))))
                .unwrap_or_default()
        } else {
            surface::enclosing(&current, *start)
                .0
                .or_else(|| surface::enclosing(&current, *start).1)
                .cloned()
                .into_iter()
                .collect()
        };
        for row in rows {
            if !touches.iter().any(|seen: &surface::Row| {
                seen.header_line == row.header_line
                    && seen.end_line == row.end_line
                    && seen.kind == row.kind
                    && seen.name == row.name
            }) {
                touches.push(row);
            }
        }
    }
    (changed, removed, added, touches)
}

/// The directories holding `paths`, each with its graph counts and, when the
/// session has seen the directory before, what changed since that first look.
pub(crate) fn directory_context(index: &relations::Relations, repo_root: &Path, paths: &[&str]) -> Value {
    let mut directories = serde_json::Map::new();
    for path in paths {
        let key = Path::new(path)
            .parent()
            .map(|parent| format!("{}/", parent.to_string_lossy().trim_end_matches('/')))
            .filter(|key| key != "/")
            .unwrap_or_else(|| "./".to_string());
        if directories.contains_key(&key) {
            continue;
        }
        let Some(metrics) = index.directory_metrics_for(&key) else {
            continue;
        };
        let mut entry = serde_json::Map::new();
        entry.insert("files".into(), metrics.files.into());
        entry.insert("imported_by".into(), metrics.imported_by.into());
        entry.insert("imports".into(), metrics.imports.into());
        if let Some(since) = session_log::at_session_start(repo_root, &key, &metrics) {
            entry.insert("at_session_start".into(), Value::Object(since));
        }
        directories.insert(key, Value::Object(entry));
    }
    Value::Object(directories)
}

pub(crate) fn render_directory_context(directories: &Value) -> String {
    let Some(entries) = directories.as_object().filter(|entries| !entries.is_empty()) else {
        return String::new();
    };
    let room = crate::output::budget().map(|budget| budget / 2);
    let mut out = String::from("Directories of changed files:\n");
    let mut shown = 0;
    for (path, entry) in entries {
        let line = format!("  {}: {}\n", crate::yamlfmt::scalar(path, false), crate::yamlfmt::flow(entry, false));
        if room.is_some_and(|room| out.len() + line.len() > room) {
            break;
        }
        out.push_str(&line);
        shown += 1;
    }
    if shown < entries.len() {
        out.push_str(&format!("  … {} more directories\n", entries.len() - shown));
    }
    out.push('\n');
    out
}

fn file_row(
    index: &relations::Relations,
    change: &Change,
    lines: Option<String>,
    changed: Vec<Value>,
    removed: Vec<surface::Row>,
    added: Vec<surface::Row>,
    touches: Vec<surface::Row>,
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
        "direct_dependents": direct,
        "changed": changed,
        "removed": removed,
        "added": added,
        "touches": touches,
    })
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
    let ranges = changed_ranges(repo_root, scope, changes);
    let mut rows: Vec<Value> = Vec::with_capacity(changes.len());
    let mut files = serde_json::Map::new();
    let mut headlines: HashMap<String, String> = HashMap::new();
    for chunk in changes.chunks(file_facts::RESOLVE_CHUNK) {
        let paths: Vec<PathBuf> = chunk.iter().map(|c| repo_root.join(&c.path)).collect();
        let facts = file_facts::get_batch(&paths, repo_root);
        let lines: Vec<Option<String>> = chunk
            .par_iter()
            .map(|c| changed_lines(repo_root, scope, c))
            .collect();
        let declarations: Vec<(
            Vec<Value>,
            Vec<surface::Row>,
            Vec<surface::Row>,
            Vec<surface::Row>,
        )> = chunk
            .par_iter()
            .map(|change| {
                let facts = facts.get(&change.path);
                let ranges = ranges
                    .get(&change.path)
                    .expect("every changed path has ranges");
                declaration_rows_for_change(repo_root, scope, change, facts, ranges)
            })
            .collect();
        for ((change, lines), (changed, removed, added, touches)) in
            chunk.iter().zip(lines).zip(declarations)
        {
            if let Some(facts) = facts.get(&change.path) {
                let facts = Facts::of(facts, index.module_counts(&change.path).as_ref());
                headlines.insert(change.path.clone(), facts.headline());
                files.insert(change.path.clone(), Value::Object(facts.to_map()));
            }
            rows.push(file_row(&index, change, lines, changed, removed, added, touches));
        }
    }
    // Stable sort by blast radius, then complexity, descending.
    let complexity = |row: &Value| {
        files
            .get(row["path"].as_str().unwrap_or(""))
            .and_then(|facts| facts["cyclomatic_complexity"].as_i64())
            .unwrap_or(0)
    };
    rows.sort_by(|a, b| {
        (b["direct_dependents"].as_i64(), complexity(b))
            .cmp(&(a["direct_dependents"].as_i64(), complexity(a)))
    });
    let paths: Vec<&str> = rows
        .iter()
        .flat_map(|row| [row["path"].as_str(), row["rename_from"].as_str()])
        .flatten()
        .collect();
    let directories = directory_context(&index, repo_root, &paths);

    let payload = crate::output::document(
        json!({"base": scope.label(), "granularity": "file"}),
        json!({
            "merge_base": match scope {
                Scope::Base { merge_base, .. } => Value::String(merge_base.clone()),
                Scope::Worktree { .. } => Value::Null,
            },
            "files": Value::Object(files.clone()),
            "directories": directories,
        }),
        json!(rows),
        json!({"files": rows.len(), "directories": directories.as_object().map_or(0, serde_json::Map::len)}),
    );

    if as_json {
        return Ok(payload);
    }

    let files = rows;
    let title = match scope {
        Scope::Worktree { .. } => format!(
            "Diff base=worktree (staged + unstaged + untracked vs HEAD)  files={}",
            files.len()
        ),
        Scope::Base { name, merge_base } => {
            let mb_short: String = merge_base.chars().take(12).collect();
            format!("Diff base={name}  merge_base={mb_short}  files={}", files.len())
        }
    };
    if files.is_empty() {
        println!("{title}\n(nothing differs)");
        return Ok(payload);
    }
    let head = format!("{title}\n\n{}", render_directory_context(&directories));
    print!("{head}");
    // Each file whole, then without its hunks, then its line alone; the
    // budget cuts the files the fewest others import first.
    let inline = |declaration: &Value, path: &str| {
        serde_json::from_value(declaration.clone())
            .ok()
            .map(|declaration| surface::inline(&declaration, path))
            .unwrap_or_default()
    };
    let mut entries: Vec<crate::output::Entry> = Vec::with_capacity(files.len());
    for row in &files {
        let path = row["path"].as_str().unwrap_or("");
        let mut header = match headlines.get(path) {
            Some(headline) => format!("  {} {path}  {headline}", row["status"].as_str().unwrap_or("")),
            None => format!("  {} {path}", row["status"].as_str().unwrap_or("")),
        };
        if let Some(rf) = row["rename_from"].as_str() {
            header.push_str(&format!("\n      renamed from: {rf}"));
        }
        let mut declarations = String::new();
        for change in row["changed"].as_array().into_iter().flatten() {
            declarations.push_str(&format!(
                "\n      changed: L{} {} → L{} {}",
                change["before"]["line"].as_i64().unwrap_or(0),
                inline(&change["before"], path),
                change["after"]["line"].as_i64().unwrap_or(0),
                inline(&change["after"], path),
            ));
        }
        for (label, rows) in [
            ("removed", &row["removed"]),
            ("added", &row["added"]),
            ("touches", &row["touches"]),
        ] {
            for declaration in rows.as_array().into_iter().flatten() {
                declarations.push_str(&format!(
                    "\n      {label}: L{} {}",
                    declaration["line"].as_i64().unwrap_or(0),
                    inline(declaration, path),
                ));
            }
        }
        let mut hunks = String::new();
        for line in row["lines"].as_str().unwrap_or("").lines() {
            hunks.push_str(&format!("\n      {line}"));
        }
        entries.push(crate::output::Entry {
            rank: row["direct_dependents"].as_i64().unwrap_or(0),
            levels: vec![
                format!("{header}{hunks}{declarations}"),
                format!("{header}{declarations}"),
                header,
            ],
        });
    }
    let changed: Vec<&str> = files.iter().map(|row| row["path"].as_str().unwrap_or("")).collect();
    let fixed = head.len() + crate::output::closing_room(entries.len(), "files");
    let (texts, shortened) = crate::output::fit_listing(&entries, &changed, fixed);
    for text in texts {
        println!("{text}");
    }
    if shortened > 0 {
        println!("{}", crate::output::shortened_line(shortened, entries.len(), "files"));
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
    let Some(source) = crate::git_activity::blob(repo_root, merge_base, relative_path) else {
        return vec![];
    };
    match crate::file_facts::extraction_of(&source, relative_path, repo_root) {
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
    let pre_path = change
        .rename_from
        .clone()
        .unwrap_or_else(|| change.path.clone());

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
    paths: &[String],
    base: Option<&str>,
    symbol_mode: bool,
    as_json: bool,
) -> Result<Value> {
    let root_of = |path: &Path| cache::worktree_root_for(path).unwrap_or_else(|| cache::display_root(path));
    let anchors: Vec<PathBuf> = paths.iter().map(|path| cache::absolutize(Path::new(path))).collect();
    let repo_root = root_of(anchors.first().map_or(Path::new("."), PathBuf::as_path));
    if anchors.iter().any(|anchor| root_of(anchor) != repo_root) {
        eprintln!("Error: the paths belong to more than one repository. Run one diff per repository.");
        std::process::exit(2);
    }
    let paths: Vec<String> = anchors
        .iter()
        .map(|anchor| match cache::relative_to_root(anchor, &repo_root) {
            within if within.is_empty() => ".".to_string(),
            within => within,
        })
        .collect();
    let paths = paths.as_slice();

    // No `--base` means the working tree: everything that differs from HEAD
    // right now, new files included.
    let scope = match base {
        None => Scope::Worktree {
            head: crate::git_activity::head_sha(&repo_root)
                .or_else(|| crate::git_activity::git_str(&repo_root, &["hash-object", "-t", "tree", "/dev/null"]))
                .unwrap_or_else(|| "HEAD".to_string()),
        },
        Some(name) => {
            verify_base_ref(&repo_root, name);
            Scope::Base {
                name: name.to_string(),
                merge_base: merge_base(&repo_root, name),
            }
        }
    };

    let mut changes = name_status(&repo_root, &scope, paths);
    if matches!(scope, Scope::Worktree { .. }) {
        changes.extend(untracked(&repo_root, paths));
    }

    if symbol_mode {
        let mb = match &scope {
            Scope::Base { merge_base, .. } => merge_base.clone(),
            // Symbol mode diffs each file's exports against a committed blob;
            // for the working tree that blob is HEAD's.
            Scope::Worktree { head } => head.clone(),
        };
        emit_symbol_mode(&repo_root, scope.label(), &mb, &changes, as_json)
    } else {
        emit_file_mode(&repo_root, &scope, &changes, as_json)
    }
}
