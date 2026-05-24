//! `trace docs` — project-docs surface: a `--graph` flag off the noun plus
//! two sub-verbs (`load`, `status`).
//!
//! - default (no flag, no sub-verb): the deduped project-docs set for a
//!   path. Walks the full ancestor chain, partitions against the per-session
//!   log, records only the newly-surfaced slice, and returns
//!   `{ path, directory_scoped, docs[], doc_count, already_loaded[]? }`.
//!   `docs[]` carries the freshly surfaced docs (with content); the optional
//!   `already_loaded[]` carries the skipped slice (without content, with
//!   per-entry source attribution) and is omitted when empty.
//!
//!   `--source` / `--triggering-tool` / `--triggering-command` flags let
//!   hook callers stamp the log event with the calling
//!   surface and the tool/command that triggered the load. The flags
//!   default to `trace_docs` / `None` / `None` so direct CLI calls keep
//!   working without them.
//!
//! - `--graph`: the whole-repo docs graph projected out of the unified
//!   `architecture/` cache entry (doc-file nodes + `@include` edges), plus
//!   the "available but not loaded" set computed against the session
//!   log when one is active.
//! - `load`: thin alias forwarding to the default implementation with the
//!   `--source` default flipped to `trace_docs_load`. Same shape, same
//!   behavior; preserved as an explicit CLI verb for hook callers.
//! - `status`: the agent-facing "what do I have right now?" query. With no
//!   path argument returns the full session manifest (every loaded doc with
//!   source attribution). With a path argument returns that path's ancestor
//!   chain partitioned into `loaded` (with source) and `not_loaded`.

use super::{nested_memory, session_log};
use crate::{architecture, cache, docs_graph};
use anyhow::Result;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Path-mode (the default) and load-mode share this implementation. The
/// chain is walked with no dedupe, then partitioned against the live
/// log: new entries land in `docs[]` (with content);
/// already-loaded entries land in `already_loaded[]` (without content, with
/// per-entry source). Only the new slice gets recorded to the
/// log.
pub fn run(
    target_raw: &Path,
    directory_mode: bool,
    source: &str,
    triggering_tool: Option<&str>,
    triggering_command: Option<&str>,
    as_json: bool,
) -> Result<Value> {
    // Triggering env vars feed `session_log::record_emission`, which reads
    // them at append time. Setting them here keeps the log
    // API unchanged and lets the same flag shape work for every caller.
    let _guard = TriggeringEnv::set(triggering_tool, triggering_command);

    let (target, repo_root, scope_dir) = resolve_target(target_raw, directory_mode);

    // Pre-load snapshot of log state: canonical paths the
    // log has already surfaced, and a per-path source map
    // drawn from the events log so already_loaded entries can name who
    // originally loaded them.
    let pre_loaded: BTreeSet<String> = session_log::loaded_paths();
    let prior_sources: BTreeMap<String, String> = prior_source_map();

    // Walk the ancestor chain with NO dedupe — yields every doc reachable
    // for this path. Partition by the pre-load set: anything already in
    // the log is `already_loaded`, anything else is newly
    // surfaced.
    let mut empty_dedupe: BTreeSet<String> = BTreeSet::new();
    let full =
        nested_memory::load_for_file(&target, &repo_root, &mut empty_dedupe, scope_dir);

    let (new_docs, skipped): (Vec<_>, Vec<_>) =
        full.into_iter().partition(|m| !pre_loaded.contains(&m.path));

    session_log::record_emission(&new_docs, source);

    let already_loaded: Vec<Value> = skipped
        .iter()
        .map(|m| {
            json!({
                "path": m.relative_path,
                "kind": m.kind,
                "size": m.size,
                "large": m.large,
                "source": prior_sources
                    .get(&m.path)
                    .cloned()
                    .unwrap_or_else(|| "unknown".to_string()),
            })
        })
        .collect();

    let display = cache::relative_to_root(&target, &repo_root);

    let mut out = json!({
        "path": display,
        "directory_scoped": scope_dir,
        "source": source,
        "triggering_tool": triggering_tool,
        "triggering_command": triggering_command,
        "docs": new_docs.iter().map(|m| json!({
            "path": m.relative_path,
            "kind": m.kind,
            "size": m.size,
            "large": m.large,
            "content": m.content,
        })).collect::<Vec<_>>(),
        "doc_count": new_docs.len(),
    });
    if !already_loaded.is_empty() {
        out.as_object_mut()
            .expect("response is an object")
            .insert("already_loaded".to_string(), Value::Array(already_loaded.clone()));
    }

    if as_json {
        return Ok(out);
    }
    print_human(&new_docs, &already_loaded, &display, scope_dir);
    Ok(out)
}

/// Status-mode: the agent-facing "what do I have right now?" query.
///
/// With `target_raw` Some: returns the ancestor chain for that path
/// partitioned into `loaded` (with per-entry source) and `not_loaded`. The
/// chain is the same one path-mode would walk; the partition is computed
/// against the live session log so the agent can immediately
/// tell whether a path's rules are in context.
///
/// With `target_raw` None: returns the full session manifest — every doc
/// the log has surfaced so far, with source attribution.
///
/// Pure read. Never records, never mutates the log.
pub fn run_status(target_raw: Option<&Path>, as_json: bool) -> Result<Value> {
    let loaded_entries = session_log::loaded_entries();
    let session_active = session_log::session_active();

    match target_raw {
        Some(path) => run_status_path(path, &loaded_entries, session_active, as_json),
        None => run_status_session(&loaded_entries, session_active, as_json),
    }
}

fn run_status_session(
    loaded_entries: &[session_log::LoadedEntry],
    session_active: bool,
    as_json: bool,
) -> Result<Value> {
    let loaded_json: Vec<Value> = loaded_entries
        .iter()
        .map(|e| {
            json!({
                "path": e.visible_as,
                "source": e.source,
                "kind": e.kind,
                "size": e.size,
                "content_hash": e.content_hash,
            })
        })
        .collect();

    let by_source = group_by_source(loaded_entries);
    let out = json!({
        "scope": "session",
        "session_active": session_active,
        "loaded": loaded_json,
        "loaded_count": loaded_entries.len(),
        "by_source": by_source,
    });

    if as_json {
        return Ok(out);
    }
    print_status_session_human(loaded_entries, session_active, &by_source);
    Ok(out)
}

fn run_status_path(
    target_raw: &Path,
    loaded_entries: &[session_log::LoadedEntry],
    session_active: bool,
    as_json: bool,
) -> Result<Value> {
    let (target, repo_root, scope_dir) = resolve_target(target_raw, false);

    // Walk-up with NO dedupe — every doc reachable for this path. We don't
    // record anything; status is a pure read.
    let mut empty_dedupe: BTreeSet<String> = BTreeSet::new();
    let chain = nested_memory::load_for_file(&target, &repo_root, &mut empty_dedupe, scope_dir);

    // Source map for partition attribution. Same shape as path-mode uses for
    // already_loaded so consumers see consistent attribution.
    let source_map = source_map(loaded_entries);
    let loaded_set: BTreeSet<String> = loaded_entries.iter().map(|e| e.path.clone()).collect();

    let (loaded_chain, not_loaded_chain): (Vec<_>, Vec<_>) = chain
        .iter()
        .partition(|m| loaded_set.contains(&m.path));

    let loaded_json: Vec<Value> = loaded_chain
        .iter()
        .map(|m| {
            json!({
                "path": m.relative_path,
                "kind": m.kind,
                "size": m.size,
                "source": source_map.get(&m.path).cloned().unwrap_or_else(|| "unknown".to_string()),
            })
        })
        .collect();
    let not_loaded_json: Vec<Value> = not_loaded_chain
        .iter()
        .map(|m| {
            json!({
                "path": m.relative_path,
                "kind": m.kind,
                "size": m.size,
            })
        })
        .collect();

    let display = cache::relative_to_root(&target, &repo_root);
    let out = json!({
        "scope": "path",
        "path": display,
        "session_active": session_active,
        "loaded": loaded_json,
        "not_loaded": not_loaded_json,
        "loaded_count": loaded_chain.len(),
        "not_loaded_count": not_loaded_chain.len(),
        "chain_size": chain.len(),
    });

    if as_json {
        return Ok(out);
    }
    print_status_path_human(&display, &loaded_chain, &not_loaded_chain, &source_map);
    Ok(out)
}

/// Graph-mode: whole-repo docs graph + the available-but-not-loaded slice.
/// Invoked via the `--graph` flag on `trace docs`. Reads the doc subset
/// out of the unified architecture-graph cache entry — same data, same
/// invalidation contract as before, just sharing the cache file with the
/// symbol/module graph.
pub fn run_graph(path: Option<&Path>, as_json: bool) -> Result<Value> {
    let here = Path::new(".");
    let resolve_root = |p: &Path| -> PathBuf {
        cache::worktree_root_for(p).unwrap_or_else(|| cache::display_root(p))
    };
    let repo_root = match path {
        Some(p) => resolve_root(p),
        None => resolve_root(here),
    };
    let arch = architecture::get(&repo_root);
    let docs = docs_graph::DocsGraph {
        head: arch.docs_head.clone(),
        mtime_aggregate: arch.docs_mtime_aggregate.clone(),
        built_at_ms: arch.docs_built_at_ms,
        nodes: arch.doc_nodes.clone(),
        edges: arch.doc_edges.clone(),
    };
    let graph_json = docs.to_json();

    // Diff against the session log to surface "available but
    // not loaded" — the agent-facing report. The log keys by
    // canonical absolute path, so re-canonicalize each node path under the
    // repo root to compare.
    let log_paths: BTreeSet<String> = session_log::loaded_paths();
    let mut not_loaded: Vec<String> = Vec::new();
    for n in &docs.nodes {
        let abs = repo_root.join(&n.path);
        let canonical = abs
            .canonicalize()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| abs.to_string_lossy().to_string());
        if !log_paths.contains(&canonical) {
            not_loaded.push(n.path.clone());
        }
    }

    let out = json!({
        "graph": graph_json,
        "available_not_loaded": not_loaded,
        "node_count": docs.nodes.len(),
        "edge_count": docs.edges.len(),
    });

    if as_json {
        return Ok(out);
    }
    print_graph_human(&docs, &not_loaded);
    Ok(out)
}

// ---------- shared helpers ----------

fn resolve_target(target_raw: &Path, directory_mode: bool) -> (PathBuf, PathBuf, bool) {
    let target = target_raw
        .canonicalize()
        .unwrap_or_else(|_| cache::absolutize(target_raw));
    if !target.exists() {
        eprintln!("Error: path not found: {}", target_raw.display());
        std::process::exit(2);
    }
    let repo_root = cache::worktree_root_for(&target).unwrap_or_else(|| cache::display_root(&target));
    let scope_dir = directory_mode || target.is_dir();
    (target, repo_root, scope_dir)
}

/// Build a canonical-path -> source map from the log events.
/// The most recent event for each path wins, so a path that was
/// surfaced by `agent_read` and later re-touched by `trace_docs` reports
/// the latest source. One pass over the events log, no per-entry replay.
fn prior_source_map() -> BTreeMap<String, String> {
    let mut out: BTreeMap<String, String> = BTreeMap::new();
    for ev in session_log::events() {
        let path = match ev.get("path").and_then(|p| p.as_str()) {
            Some(p) => p.to_string(),
            None => continue,
        };
        if let Some(source) = ev.get("source").and_then(|v| v.as_str()) {
            out.insert(path, source.to_string());
        }
    }
    out
}

fn print_human(
    new_docs: &[nested_memory::LoadedMemory],
    already_loaded: &[Value],
    display: &str,
    scope_dir: bool,
) {
    let mut header = format!("# docs · {display}");
    if scope_dir {
        header += " (directory-scoped)";
    }
    header += &format!(" · docs {} · already_loaded {}", new_docs.len(), already_loaded.len());
    println!("{header}");
    if new_docs.is_empty() && already_loaded.is_empty() {
        println!("  (no project docs for this path)");
        return;
    }
    if !new_docs.is_empty() {
        let block = nested_memory::render(new_docs);
        if !block.is_empty() {
            println!("{block}");
        }
    }
    if !already_loaded.is_empty() {
        println!();
        println!("## already in context ({})", already_loaded.len());
        for entry in already_loaded {
            let path = entry["path"].as_str().unwrap_or("?");
            let source = entry["source"].as_str().unwrap_or("?");
            let size = entry["size"].as_i64().unwrap_or(0);
            println!("  · {path}  (source: {source}, {size} chars)");
        }
    }
}

/// path -> latest source map, built from the loaded entries. Matches the
/// attribution `prior_source_map` produces, so `status` and path-mode report
/// the same source for any given path.
fn source_map(entries: &[session_log::LoadedEntry]) -> BTreeMap<String, String> {
    entries
        .iter()
        .map(|e| (e.path.clone(), e.source.clone()))
        .collect()
}

/// source -> count breakdown of the session manifest. Stable sorted by
/// source string so the human-readable form is deterministic.
fn group_by_source(entries: &[session_log::LoadedEntry]) -> BTreeMap<String, usize> {
    let mut out: BTreeMap<String, usize> = BTreeMap::new();
    for e in entries {
        *out.entry(e.source.clone()).or_insert(0) += 1;
    }
    out
}

fn print_status_session_human(
    entries: &[session_log::LoadedEntry],
    session_active: bool,
    by_source: &BTreeMap<String, usize>,
) {
    if !session_active {
        println!("# docs status · no active session (log is empty)");
        return;
    }
    println!("# docs status · session manifest · {} loaded", entries.len());
    if entries.is_empty() {
        println!("  (no docs loaded in this session yet)");
        return;
    }
    if !by_source.is_empty() {
        let items: Vec<String> = by_source.iter().map(|(s, n)| format!("{s}: {n}")).collect();
        println!("  by source: {}", items.join(", "));
    }
    for e in entries {
        println!(
            "  · {}  (source: {}, kind: {}, {} chars)",
            e.visible_as, e.source, e.kind, e.size
        );
    }
}

fn print_status_path_human(
    display: &str,
    loaded: &[&nested_memory::LoadedMemory],
    not_loaded: &[&nested_memory::LoadedMemory],
    source_map: &BTreeMap<String, String>,
) {
    println!(
        "# docs status · {display} · loaded {} · not_loaded {}",
        loaded.len(),
        not_loaded.len()
    );
    if !loaded.is_empty() {
        println!("## in context");
        for m in loaded {
            let source = source_map
                .get(&m.path)
                .cloned()
                .unwrap_or_else(|| "unknown".to_string());
            println!(
                "  · {}  (source: {source}, kind: {}, {} chars)",
                m.relative_path, m.kind, m.size
            );
        }
    }
    if !not_loaded.is_empty() {
        println!("## not loaded");
        for m in not_loaded {
            println!(
                "  · {}  (kind: {}, {} chars)",
                m.relative_path, m.kind, m.size
            );
        }
    }
}

fn print_graph_human(graph: &docs_graph::DocsGraph, not_loaded: &[String]) {
    println!(
        "# docs graph: {} nodes, {} edges (head={}, available-not-loaded={})",
        graph.nodes.len(),
        graph.edges.len(),
        graph.head,
        not_loaded.len()
    );
    for n in &graph.nodes {
        let marker = if not_loaded.contains(&n.path) {
            " [not loaded]"
        } else {
            ""
        };
        println!("  · {} ({}, {} chars){}", n.path, n.kind, n.size, marker);
    }
    if !graph.edges.is_empty() {
        println!();
        println!("## edges");
        for e in &graph.edges {
            println!("  {} --{}--> {}", e.source, e.relation, e.target);
        }
    }
}

/// RAII guard for the triggering-tool / triggering-command env vars the
/// session log reads at append time. Restores prior values
/// on drop so concurrent test invocations don't leak env state.
struct TriggeringEnv {
    prior_tool: Option<std::ffi::OsString>,
    prior_command: Option<std::ffi::OsString>,
}

impl TriggeringEnv {
    fn set(tool: Option<&str>, command: Option<&str>) -> Self {
        let prior_tool = std::env::var_os("TRACER_TRIGGERING_TOOL");
        let prior_command = std::env::var_os("TRACER_TRIGGERING_COMMAND");
        if let Some(t) = tool {
            std::env::set_var("TRACER_TRIGGERING_TOOL", t);
        }
        if let Some(c) = command {
            std::env::set_var("TRACER_TRIGGERING_COMMAND", c);
        }
        Self {
            prior_tool,
            prior_command,
        }
    }
}

impl Drop for TriggeringEnv {
    fn drop(&mut self) {
        match self.prior_tool.take() {
            Some(v) => std::env::set_var("TRACER_TRIGGERING_TOOL", v),
            None => std::env::remove_var("TRACER_TRIGGERING_TOOL"),
        }
        match self.prior_command.take() {
            Some(v) => std::env::set_var("TRACER_TRIGGERING_COMMAND", v),
            None => std::env::remove_var("TRACER_TRIGGERING_COMMAND"),
        }
    }
}
