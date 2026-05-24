//! `trace cache build|stats|clear` — manage the .tracer-cache/ disk cache.
//! Two-namespace cache: `build` warms per-file facts + architecture graph,
//! `stats` reports size and entry count per namespace (human + --json,
//! cross-consistent), `clear`
//! scoped by --namespace or wiping the whole tree with --all.

use crate::{architecture, cache};
use anyhow::Result;
use serde_json::{json, Value};
use std::path::Path;
use std::time::Instant;

fn print_stats_rows(repo_root: &Path) -> Result<()> {
    for row in cache::stats(repo_root)? {
        let size_kb = row.total_bytes as f64 / 1024.0;
        println!(
            "  {:<14}  {:>5} entries  {:>8.1} KB",
            row.namespace, row.entry_count, size_kb
        );
    }
    Ok(())
}

/// `cache build` — discover source files, populate per-file facts, build the
/// architecture graph, then print timing + graph size + per-namespace stats.
/// Idempotent: a warm cache only does the cheap revalidation path.
pub fn build(path: &Path) -> Result<()> {
    crate::pathval::require_exists(path, "PATH");
    // The PATH argument IS the repo-root override (scoped build), not a
    // hint to walk up from. `cache build app` builds only `app/`, not the
    // whole repo.
    let repo_root = path
        .canonicalize()
        .unwrap_or_else(|_| cache::absolutize(path));
    let start = Instant::now();
    let graph = architecture::get(&repo_root);
    let elapsed = start.elapsed().as_secs_f64();
    println!("Built in {elapsed:.2}s");
    println!(
        "Architecture graph: {} nodes, {} edges",
        graph.nodes.len(),
        graph.edges.len()
    );
    println!();
    print_stats_rows(&repo_root)
}

/// `cache clear` — delete entries in one namespace, both, or the whole tree.
pub fn clear(path: &Path, namespace: Option<&str>, clear_all: bool) -> Result<()> {
    let repo_root = cache::worktree_root_for(path).unwrap_or_else(|| cache::display_root(path));
    if clear_all {
        let removed = cache::clear_all(&repo_root)?;
        println!("Removed {removed} cache entries (entire .tracer-cache/).");
        return Ok(());
    }
    let removed = cache::clear(namespace, &repo_root)?;
    let target = namespace.unwrap_or("all namespaces");
    println!("Removed {removed} cache entries from {target}.");
    Ok(())
}

/// `cache stats` — size and entry count per namespace. `--json` emits an
/// object keyed by namespace, cross-consistent with the human columns.
pub fn stats(path: &Path, as_json: bool) -> Result<Value> {
    let repo_root = cache::worktree_root_for(path).unwrap_or_else(|| cache::display_root(path));
    let rows = cache::stats(&repo_root)?;
    let mut obj = serde_json::Map::new();
    for row in &rows {
        obj.insert(
            row.namespace.clone(),
            json!({"entries": row.entry_count, "bytes": row.total_bytes}),
        );
    }
    if !as_json {
        for row in &rows {
            let size_kb = row.total_bytes as f64 / 1024.0;
            println!(
                "  {:<14}  {:>5} entries  {:>8.1} KB",
                row.namespace, row.entry_count, size_kb
            );
        }
    }
    Ok(Value::Object(obj))
}
