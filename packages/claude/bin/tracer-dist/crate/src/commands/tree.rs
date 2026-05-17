//! `trace tree` — recursive annotated file tree with complexity ranks.
//! Discovery via `repo_files::tracked_files` inside a git repo,
//! `walk_files` outside; both honor SKIP_DIRS. Per-file facts come from
//! `file_facts::get`, which always does real extraction (no lite-facts).

use crate::{cache, file_facts, passive_context, repo_context, repo_files};
use anyhow::Result;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn rank_marker(rank: &str) -> &'static str {
    match rank {
        "low" => "·",
        "medium" => "•",
        "high" => "●",
        "critical" => "⚠",
        _ => "?",
    }
}

struct Entry {
    full: PathBuf,
    ccn_total: i64,
    ccn_max_function: i64,
    loc: i64,
    rank: String,
    passive_context: Option<String>,
}

fn entry_from(full: &Path, facts: Option<&file_facts::FileFacts>) -> Entry {
    match facts {
        None => Entry {
            full: full.to_path_buf(),
            ccn_total: 0,
            ccn_max_function: 0,
            loc: 0,
            rank: "unknown".to_string(),
            passive_context: None,
        },
        Some(f) => Entry {
            full: full.to_path_buf(),
            ccn_total: f.cyclomatic_complexity_total,
            ccn_max_function: f.cyclomatic_complexity_max,
            loc: f.loc,
            rank: f.rank.clone(),
            passive_context: Some(passive_context::render_compact(f)),
        },
    }
}

/// Depth-bounded tree walk under `base`, collecting entries.
fn walk(base: &Path, max_depth: usize) -> Vec<Entry> {
    let repo_root = cache::repo_root_for(base);
    let base_abs = base.canonicalize().unwrap_or_else(|_| base.to_path_buf());
    let tracked = repo_files::tracked_files(&repo_root, Some(base));

    let mut entries: Vec<Entry> = Vec::new();
    // Collect the depth-filtered file set first, then ONE get_batch over
    // it (was O(N²) per-file get() — directory class, same root cause as
    // the list 62s blowup). Real extraction preserved.
    let mut selected: Vec<std::path::PathBuf> = Vec::new();
    match &tracked {
        Some(rels) => {
            let mut rels = rels.clone();
            rels.sort();
            for rel in &rels {
                let full = repo_root.join(rel);
                let resolved =
                    full.canonicalize().unwrap_or_else(|_| full.clone());
                let under = match resolved.strip_prefix(&base_abs) {
                    Ok(u) => u.to_string_lossy().to_string(),
                    Err(_) => continue,
                };
                let depth = under.matches('/').count() + 1;
                if depth > max_depth {
                    continue;
                }
                selected.push(full);
            }
        }
        None => {
            let mut walked = repo_files::walk_files(&base_abs);
            walked.sort();
            for full in walked {
                let depth = match full.strip_prefix(&base_abs) {
                    Ok(r) => r.components().count(),
                    Err(_) => continue,
                };
                if depth > max_depth {
                    continue;
                }
                selected.push(full);
            }
        }
    }
    // Depth filter uses a non-strict resolve: canonicalize when the target
    // exists (symlink / `..` normalization, e.g. a symlink into a deeper
    // dir), else the lexical join (keeps indexed-but-deleted files).
    // Applied above when building `selected`.
    let facts_map = file_facts::get_batch(&selected, &repo_root);
    for full in &selected {
        let rel = cache::relative_to_root(full, &repo_root);
        entries.push(entry_from(full, facts_map.get(&rel)));
    }
    entries
}

pub fn run(path: &Path, depth: usize, as_json: bool) -> Result<Value> {
    crate::pathval::require_exists(path, "PATH");
    // The displayed base is the resolved path.
    let base = path
        .canonicalize()
        .unwrap_or_else(|_| cache::absolutize(path));
    let base_abs = base.clone();
    let entries = walk(&base, depth);
    let ctx = repo_context::repo_context(&base);

    let files: Vec<Value> = entries
        .iter()
        .map(|e| {
            // Relative path against the UNRESOLVED repo_root/rel —
            // symlinks keep their tracked name.
            let rel = e
                .full
                .strip_prefix(&base_abs)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| e.full.to_string_lossy().to_string());
            json!({
                "path": rel,
                "ccn_total": e.ccn_total,
                "ccn_max_function": e.ccn_max_function,
                "loc": e.loc,
                "rank": e.rank,
                "passive_context": e.passive_context,
            })
        })
        .collect();
    let value = json!({
        "root": base.to_string_lossy(),
        "files": files,
        "repo_context": ctx.clone(),
    });

    if as_json {
        return Ok(value);
    }

    println!("{}/", base.to_string_lossy());
    for e in &entries {
        // Relative path against the UNRESOLVED repo_root/rel.
        let rel = e
            .full
            .strip_prefix(&base_abs)
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|_| e.full.clone());
        let parts = rel.components().count();
        let indent = "  ".repeat(parts.saturating_sub(1));
        let marker = rank_marker(&e.rank);
        let name = rel
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let suffix = e
            .passive_context
            .as_ref()
            .map(|c| format!(" — {c}"))
            .unwrap_or_default();
        println!(
            "{indent}{marker} {name}  [ccn={} loc={} {}]{suffix}",
            e.ccn_total, e.loc, e.rank
        );
    }
    println!();
    println!(
        "repo_context: complexity_p95={} median={} files={}",
        ctx["complexity_p95"].as_i64().unwrap_or(0),
        ctx["median_file_ccn"].as_i64().unwrap_or(0),
        ctx["total_files"].as_i64().unwrap_or(0),
    );
    Ok(value)
}
