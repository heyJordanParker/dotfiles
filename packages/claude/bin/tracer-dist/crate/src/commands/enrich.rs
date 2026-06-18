//! Per-match file enrichment shared by `grep` and `struct`.
//! file_complexity, git_context, and the `nearest_doc` walk (in
//! `crate::digest`). One parse per file even with many matches, via the
//! per-file cache.
//!
//! Also the shared `file_shoulders` join used by the architecture commands
//! (`callers`, `downstream`, `defines`, `symbols`) to attach the canonical
//! passive-context shoulder to each result's `source_file` — a render-time
//! join of `file/` facts onto `architecture/` results, batched and deduped
//! by path so a file appearing in many rows is resolved once.

use crate::{cache, digest, file_facts, passive_context};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct Match {
    pub file: String,
    pub line: i64,
    pub snippet: String,
}

/// Per-file complexity scalars from cached facts.
fn file_complexity(facts: Option<&file_facts::FileFacts>) -> Value {
    match facts {
        None => json!({
            "ccn_total": 0,
            "ccn_max_function": 0,
            "loc": 0,
            "rank": "unknown",
        }),
        Some(f) => json!({
            "ccn_total": f.cyclomatic_complexity_total,
            "ccn_max_function": f.cyclomatic_complexity_max,
            "loc": f.loc,
            "rank": f.rank,
        }),
    }
}

/// Git context for a file: last commit, author, 30-day commit count.
fn git_context(facts: Option<&file_facts::FileFacts>) -> Value {
    match facts {
        None => json!({
            "last_modified": Value::Null,
            "last_author": Value::Null,
            "commits_30d": 0,
        }),
        Some(f) => json!({
            "last_modified": f.last_modified,
            "last_author": f.last_author,
            "commits_30d": f.commits_30d,
        }),
    }
}

/// Enrich matches with per-file complexity, nearest doc, and git context.
/// One `file_facts::get` per unique file. `repo_root` is resolved once by
/// the caller for the search path — every match lives under it, so per-file
/// root resolution is correct without paying a `git rev-parse` per match.
///
/// Matches are sorted by `(file, line, snippet)` before enrichment so the
/// emitted order is byte-identical across repeated identical invocations.
/// The underlying search backends (`rg --json`, `sg run --json`) walk files
/// in parallel and emit per-file blocks in nondeterministic order; without
/// this sort the same query returns the same matches in a different order
/// each run, which breaks output diffing and caching for any consumer.
pub fn enrich(matches: &[Match], repo_root: &Path) -> (Vec<Value>, usize) {
    let mut matches: Vec<&Match> = matches.iter().collect();
    matches.sort_by(|a, b| {
        (&a.file, a.line, &a.snippet).cmp(&(&b.file, b.line, &b.snippet))
    });
    let mut file_cache: HashMap<String, Value> = HashMap::new();
    let mut enriched: Vec<Value> = Vec::new();
    for m in matches {
        if !file_cache.contains_key(&m.file) {
            let abs = cache::absolutize(Path::new(&m.file));
            let facts = file_facts::get(&abs, repo_root, None);
            file_cache.insert(
                m.file.clone(),
                json!({
                    "file_complexity": file_complexity(facts.as_ref()),
                    "nearest_doc": digest::nearest_doc(&abs),
                    "git": git_context(facts.as_ref()),
                    "shoulder": facts.as_ref().map(|f| passive_context::render(f, None)),
                }),
            );
        }
        let fc = &file_cache[&m.file];
        enriched.push(json!({
            "file": m.file,
            "line": m.line,
            "snippet": m.snippet,
            "file_complexity": fc["file_complexity"],
            "nearest_doc": fc["nearest_doc"],
            "git": fc["git"],
            "shoulder": fc["shoulder"],
        }));
    }
    let files_matched = file_cache.len();
    (enriched, files_matched)
}

/// Shared human renderer for grep/struct: grouped-by-file with a per-file
/// shoulder, then a one-line summary.
pub fn render_human(enriched: &[Value], files_matched: usize, repo_ctx: &Value) {
    if enriched.is_empty() {
        println!("(no matches)");
        return;
    }
    let mut current_file: Option<String> = None;
    for m in enriched {
        let file = m["file"].as_str().unwrap_or("").to_string();
        if Some(&file) != current_file.as_ref() {
            let doc = m["nearest_doc"].as_str().unwrap_or("(no doc)");
            println!();
            match m["shoulder"].as_str() {
                Some(s) => println!("{file}  {s}  [doc={doc}]"),
                None => println!("{file}  [doc={doc}]"),
            }
            current_file = Some(file);
        }
        println!(
            "  L{:<5} {}",
            m["line"].as_i64().unwrap_or(0),
            m["snippet"].as_str().unwrap_or("")
        );
    }
    println!();
    println!(
        "matches={} files={} repo_p95={}",
        enriched.len(),
        files_matched,
        repo_ctx["complexity_p95"].as_i64().unwrap_or(0),
    );
}

/// Canonical passive-context shoulder per `source_file`, batched and deduped.
/// Maps each unique repo-relative source file in `rel_files` to its shoulder
/// string. Files with no resolvable facts (external nodes, deleted files) are
/// absent from the map, so a caller looks up by path and renders nothing when
/// the entry is missing. The architecture commands carry a `source_file` per
/// result row; this lets each result carry the same file-state shoulder the
/// per-file commands emit, without recomputing facts per row.
pub fn file_shoulders(
    rel_files: &[String],
    repo_root: &Path,
) -> HashMap<String, String> {
    let mut unique: Vec<String> = rel_files.to_vec();
    unique.sort();
    unique.dedup();
    let abs: Vec<PathBuf> = unique.iter().map(|r| repo_root.join(r)).collect();
    let facts_map = file_facts::get_batch(&abs, repo_root);
    let mut out = HashMap::with_capacity(unique.len());
    for rel in unique {
        if let Some(f) = facts_map.get(&rel) {
            out.insert(rel, passive_context::render(f, None));
        }
    }
    out
}
