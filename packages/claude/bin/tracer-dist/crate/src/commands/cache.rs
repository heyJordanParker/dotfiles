//! `trace cache build|stats|clear` — manage the .tracer-cache/ disk cache.
//! `build` warms per-file facts and the relations index, `stats` reports
//! size and entry count (human + --json, cross-consistent), `clear` empties
//! the `file/` namespace or wipes the whole tree with --all.

use crate::{cache, relations};
use anyhow::Result;
use serde_json::{json, Value};
use std::path::Path;
use std::time::Instant;

fn print_stats_row(row: &cache::CacheStats) {
    println!(
        "  {:<14}  {:>5} entries  {:>8.1} KB",
        cache::NAMESPACE_FILE,
        row.entry_count,
        row.total_bytes as f64 / 1024.0
    );
}

/// `cache build` — discover source files, populate per-file facts, build the
/// relations index, then print timing + index size + per-namespace stats.
/// Idempotent: a warm cache only does the cheap revalidation path.
pub fn build(path: &Path) -> Result<()> {
    crate::pathval::require_exists(path, "PATH");
    // One repository, one index. `.tracer-cache/` exists only at a worktree
    // root, so PATH says which repository to build, never which part of it.
    // Treating PATH as the root indexed a subdirectory in isolation, wrote
    // none of it (the save gate refuses a non-worktree root), and then
    // failed reading the stats back.
    let Some(repo_root) = cache::worktree_root_for(path) else {
        println!("Not inside a git repository — nothing to build.");
        return Ok(());
    };
    let start = Instant::now();
    let index = relations::get(&repo_root);
    let elapsed = start.elapsed().as_secs_f64();
    println!("Built in {elapsed:.2}s");
    println!(
        "Relations: {} files, {} names, {} import edges",
        index.files().count(),
        index.name_count(),
        index.import_edges().count(),
    );
    println!();
    print_stats_row(&cache::stats(&repo_root)?);
    Ok(())
}

/// `cache clear` — empty the `file/` namespace, or wipe the whole tree
/// with `--all`.
pub fn clear(path: &Path, clear_all: bool) -> Result<()> {
    let repo_root = cache::worktree_root_for(path).unwrap_or_else(|| cache::display_root(path));
    if clear_all {
        let removed = cache::clear_all(&repo_root)?;
        println!("Removed {removed} cache entries (entire .tracer-cache/).");
        return Ok(());
    }
    let removed = cache::clear(&repo_root)?;
    println!(
        "Removed {removed} cache entries from {}.",
        cache::NAMESPACE_FILE
    );
    Ok(())
}

/// `cache stats` — size and entry count. `--json` emits an object keyed by
/// namespace, cross-consistent with the human columns.
pub fn stats(path: &Path, as_json: bool) -> Result<Value> {
    let repo_root = cache::worktree_root_for(path).unwrap_or_else(|| cache::display_root(path));
    let row = cache::stats(&repo_root)?;
    if !as_json {
        print_stats_row(&row);
    }
    Ok(crate::output::document(
        json!({"path": repo_root.to_string_lossy()}),
        json!({}),
        json!({
            cache::NAMESPACE_FILE: {
                "entries": row.entry_count,
                "bytes": row.total_bytes,
            }
        }),
        json!({"entries": row.entry_count, "bytes": row.total_bytes}),
    ))
}
