//! `trace tree` — recursive annotated file tree with complexity ranks.
//! Discovery via `repo_files::tracked_files` inside a git repo,
//! `walk_files` outside; both honor SKIP_DIRS. Per-file facts come from
//! bounded `file_facts::get_batch` calls, which always do real extraction.

use crate::summary::Facts;
use crate::{cache, file_facts, repo_context, repo_files};
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
    facts: Option<Facts>,
    max_function_complexity: i64,
}

/// Depth-bounded tree walk under `base`, collecting entries.
fn walk(base: &Path, max_depth: usize) -> Vec<Entry> {
    let repo_root = cache::worktree_root_for(base).unwrap_or_else(|| cache::display_root(base));
    let base_abs = base.canonicalize().unwrap_or_else(|_| base.to_path_buf());
    let tracked = repo_files::tracked_files(&repo_root, (base_abs != repo_root).then_some(base));

    let mut entries: Vec<Entry> = Vec::new();
    // Collect the depth-filtered file set first, then ONE get_batch over
    // it (was O(N²) per-file get() — directory class, same root cause as
    // the list 62s blowup). Real extraction preserved.
    let mut selected: Vec<std::path::PathBuf> = Vec::new();
    match &tracked {
        Some(rels) => {
            for rel in rels.iter() {
                let full = repo_root.join(rel);
                let resolved = full.canonicalize().unwrap_or_else(|_| full.clone());
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
    let index = crate::relations::get(&repo_root);
    for chunk in selected.chunks(file_facts::RESOLVE_CHUNK) {
        let facts_map = file_facts::get_batch(chunk, &repo_root);
        for full in chunk {
            let rel = cache::relative_to_root(full, &repo_root);
            let facts = facts_map.get(&rel);
            entries.push(Entry {
                full: full.clone(),
                facts: facts.map(|f| Facts::of(f, index.module_counts(&rel).as_ref())),
                max_function_complexity: facts.map_or(0, |f| f.cyclomatic_complexity_max),
            });
        }
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

    // Each file's facts are per-file enrichment, so they sit in `context`
    // keyed by the same path the row carries. A `--filter` that projects rows
    // cannot take them along.
    let mut file_facts_by_path = serde_json::Map::new();
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
            if let Some(facts) = &e.facts {
                let mut map = facts.to_map();
                map.insert("max_function_complexity".into(), e.max_function_complexity.into());
                file_facts_by_path.insert(rel.clone(), Value::Object(map));
            }
            json!({"path": rel})
        })
        .collect();
    let value = crate::output::document(
        json!({"root": base.to_string_lossy(), "depth": depth}),
        json!({"repo": ctx.clone(), "files": Value::Object(file_facts_by_path)}),
        json!(files),
        json!({"files": files.len()}),
    );

    if as_json {
        return Ok(value);
    }

    let mut rows: Vec<crate::output::Entry> = Vec::with_capacity(entries.len());
    let mut paths: Vec<&str> = Vec::with_capacity(entries.len());
    let mut open: Vec<&str> = Vec::new();
    for (e, row) in entries.iter().zip(&files) {
        let rel = row["path"].as_str().unwrap_or_default();
        let (directories, name) = match rel.rsplit_once('/') {
            Some((directories, name)) => (directories.split('/').collect(), name),
            None => (Vec::new(), rel),
        };
        let kept = open.iter().zip(&directories).take_while(|(a, b)| a == b).count();
        let mut heading = String::new();
        for (depth, directory) in directories.iter().enumerate().skip(kept) {
            heading.push_str(&format!("{}{directory}/\n", "  ".repeat(depth + 1)));
        }
        let indent = "  ".repeat(directories.len() + 1);
        let marker = rank_marker(e.facts.as_ref().map_or("unknown", |f| &f.complexity_rank));
        let bare = format!("{heading}{indent}{marker} {name}");
        rows.push(match &e.facts {
            Some(facts) => crate::output::Entry {
                rank: facts.imported_by.unwrap_or(0) as i64,
                levels: vec![format!("{bare}  {}", facts.headline()), bare],
            },
            None => crate::output::Entry { rank: 0, levels: vec![bare] },
        });
        paths.push(rel);
        open = directories;
    }
    let facts = json!({
        "files": ctx["total_files"].as_i64().unwrap_or(0),
        "median_file_complexity": ctx["median_file_ccn"].as_i64().unwrap_or(0),
        "complexity_p95": ctx["complexity_p95"].as_i64().unwrap_or(0),
    });
    let footer = format!("\nrepo_context: {}", crate::yamlfmt::flow(&facts, false));
    let root = format!("{}/", base.to_string_lossy());
    let fixed = root.len() + 1 + footer.len() + 1 + crate::output::closing_room(rows.len(), "files");
    let (texts, shortened) = crate::output::fit_listing(&rows, &paths, fixed);
    println!("{root}");
    for text in texts {
        println!("{text}");
    }
    println!("{footer}");
    if shortened > 0 {
        println!("{}", crate::output::shortened_line(shortened, rows.len(), "files"));
    }
    Ok(value)
}
