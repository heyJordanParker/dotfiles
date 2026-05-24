//! `trace status` — working-tree dirty set with code intelligence.
//! Every uncommitted-state file annotated with ccn rank, downstream
//! caller count, deploy-branch presence, and the lifecycle shoulder,
//! ordered by blast radius. CCN is AST-derived.

use crate::{architecture, cache, file_facts, git_activity, passive_context};
use anyhow::Result;
use serde_json::{json, Value};
use std::path::Path;

/// Human-output grouping order; also the tertiary sort key.
const STATE_ORDER: &[&str] = &["added", "renamed", "modified", "deleted", "untracked"];

fn state_rank(state: &str) -> usize {
    STATE_ORDER.iter().position(|s| *s == state).unwrap_or(STATE_ORDER.len())
}

/// Caller/dependency counts for a file from the graph. None when the file
/// has no graph node.
fn graph_counts(relative: &str, graph: Option<&architecture::Graph>) -> Option<Value> {
    let graph = graph?;
    let module_id = graph.file_to_module_id.get(relative)?;
    Some(json!({
        "callers": architecture::dependents_of(graph, module_id).len(),
        "depended_on_by_modules": architecture::dependencies_of(graph, module_id).len(),
    }))
}

fn entries_for_state(
    repo_root: &Path,
    states: &[(String, String)],
    graph: Option<&architecture::Graph>,
) -> Vec<Value> {
    let mut entries = Vec::new();
    for (relative, state) in states {
        let abs_path = repo_root.join(relative);
        let facts = if abs_path.exists() {
            file_facts::get(&abs_path, repo_root, None)
        } else {
            None
        };
        let gc = if facts.is_some() {
            graph_counts(relative, graph)
        } else {
            None
        };
        let shoulder = facts
            .as_ref()
            .map(|f| passive_context::render(f, gc.as_ref()));
        entries.push(json!({
            "path": relative,
            "state": state,
            "shoulder": shoulder,
            "callers": gc.as_ref().and_then(|g| g["callers"].as_i64()).unwrap_or(0),
            "depended_on_by_modules":
                gc.as_ref().and_then(|g| g["depended_on_by_modules"].as_i64()).unwrap_or(0),
            "ccn_total":
                facts.as_ref().map(|f| f.cyclomatic_complexity_total).unwrap_or(0),
            "ccn_rank":
                facts.as_ref().map(|f| f.rank.clone()).unwrap_or_else(|| "unknown".into()),
            "present_in":
                facts.as_ref().map(|f| f.present_in.clone()).unwrap_or_default(),
            "last_subject": facts.as_ref().and_then(|f| f.last_subject.clone()),
            "top_author": facts.as_ref().and_then(|f| f.top_author.clone()),
        }));
    }
    entries
}

/// Blast-radius sort key: (-callers, -ccn_total, state_rank, path).
fn sort_key(entry: &Value) -> (i64, i64, usize, String) {
    (
        -entry["callers"].as_i64().unwrap_or(0),
        -entry["ccn_total"].as_i64().unwrap_or(0),
        state_rank(entry["state"].as_str().unwrap_or("")),
        entry["path"].as_str().unwrap_or("").to_string(),
    )
}

pub fn run(as_json: bool, state_filter: Option<&str>) -> Result<Value> {
    let here = Path::new(".");
    let repo_root = cache::worktree_root_for(here).unwrap_or_else(|| cache::display_root(here));
    let state_map = git_activity::working_tree_state(&repo_root);
    let mut states: Vec<(String, String)> = state_map
        .into_iter()
        .filter(|(_, s)| match state_filter {
            Some(f) => s == f,
            None => true,
        })
        .collect();
    // Deterministic pre-sort by path so the stable final sort resolves
    // ties by path.
    states.sort();

    // Build (not just load) the graph: `status` runs precisely when the
    // tree is dirty, and any dirty file changes the architecture
    // fingerprint, so a load-only path would miss the cache every time
    // and collapse every blast-radius count to zero.
    let graph = architecture::get(&repo_root);
    let mut entries = entries_for_state(&repo_root, &states, Some(&graph));
    entries.sort_by(|a, b| sort_key(a).cmp(&sort_key(b)));

    let value = json!({
        "repo_root": repo_root.to_string_lossy(),
        "count": entries.len(),
        "entries": entries.clone(),
    });

    if as_json {
        return Ok(value);
    }

    if entries.is_empty() {
        println!("(working tree clean)");
        return Ok(value);
    }

    println!("{} files with uncommitted state:", entries.len());
    println!();
    let mut current_state: Option<String> = None;
    for entry in &entries {
        let state = entry["state"].as_str().unwrap_or("").to_string();
        if Some(&state) != current_state.as_ref() {
            println!("## {state}");
            current_state = Some(state);
        }
        let callers = entry["callers"].as_i64().unwrap_or(0);
        let ccn = entry["ccn_total"].as_i64().unwrap_or(0);
        let rank = entry["ccn_rank"].as_str().unwrap_or("");
        println!(
            "  {}  (callers={callers}, ccn={ccn} {rank})",
            entry["path"].as_str().unwrap_or("")
        );
        if let Some(shoulder) = entry["shoulder"].as_str() {
            if !shoulder.is_empty() {
                println!("    {shoulder}");
            }
        }
    }
    Ok(value)
}
