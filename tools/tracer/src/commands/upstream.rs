//! `trace upstream` — what a symbol or path depends on.
//!
//! Symbol mode: transitive dependencies of one symbol (BFS forward edges).
//! Path mode: top-N highest-coupling symbols across the path's graph.

use crate::{architecture, cache};
use anyhow::Result;
use serde_json::{json, Map, Value};
use std::path::Path;

pub fn run(
    symbol: Option<&str>,
    path: Option<&Path>,
    depth: i64,
    limit: i64,
    as_json: bool,
) -> Result<Value> {
    if let Some(p) = path {
        crate::pathval::require_exists(p, "--path");
        return path_mode(p, depth, limit, as_json);
    }
    let symbol = match symbol {
        Some(s) if !s.is_empty() => s,
        _ => {
            eprintln!("Error: pass a SYMBOL or --path <path>.");
            std::process::exit(2);
        }
    };
    symbol_mode(symbol, depth, as_json)
}

fn symbol_mode(symbol: &str, depth: i64, as_json: bool) -> Result<Value> {
    let here = Path::new(".");
    let repo_root = cache::worktree_root_for(here).unwrap_or_else(|| cache::display_root(here));
    let graph = architecture::get(&repo_root);
    let matches = architecture::find_symbols(&graph, symbol);
    if matches.is_empty() {
        eprintln!("Symbol '{symbol}' not found in architecture graph.");
        std::process::exit(2);
    }

    let mut output = Map::new();
    for m in &matches {
        let chain = architecture::transitive_dependencies(&graph, &m.id, depth);
        let deps: Vec<Value> = chain
            .iter()
            .map(|(node, d)| {
                json!({
                    "node_id": node.id,
                    "label": node.label,
                    "kind": node.kind,
                    "source_file": node.source_file,
                    "depth": d,
                })
            })
            .collect();
        output.insert(
            m.id.clone(),
            json!({
                "symbol": m.label,
                "kind": m.kind,
                "source_file": m.source_file,
                "source_line": m.source_line,
                "dependencies": deps,
            }),
        );
    }

    if !as_json {
        for m in &matches {
            let chain = architecture::transitive_dependencies(&graph, &m.id, depth);
            println!(
                "\n{} [{}] @ {}:{}",
                m.label,
                m.kind,
                m.source_file.as_deref().unwrap_or("None"),
                m.source_line
                    .map(|l| l.to_string())
                    .unwrap_or_else(|| "None".into()),
            );
            println!("  depends on (depth ≤ {depth}):");
            if chain.is_empty() {
                println!("    (no architecture-graph dependencies found)");
                continue;
            }
            for (node, d) in &chain {
                let location = match node.source_file.as_deref() {
                    Some(f) if !f.is_empty() => format!(
                        "{f}:{}",
                        node.source_line
                            .map(|l| l.to_string())
                            .unwrap_or_else(|| "None".into())
                    ),
                    _ => "(external)".to_string(),
                };
                println!("    [d={d}] {} [{}] @ {}", node.label, node.kind, location);
            }
        }
    }
    Ok(Value::Object(output))
}

fn path_mode(path: &Path, depth: i64, limit: i64, as_json: bool) -> Result<Value> {
    // Same logic as downstream::path_mode — the graph must be built
    // against the real git repo root so single-file `--path` arguments
    // don't amputate every cross-file edge.
    let repo_root = cache::worktree_root_for(path).unwrap_or_else(|| cache::display_root(path));
    let graph = architecture::get(&repo_root);

    // Counter(edge.source) — first-seen order, count descending (stable).
    // Path-mode coupling ranks by IMPORT graph only; reference edges are a
    // separate dimension exposed via `references_to` rather than absorbed
    // into module-level fan-out.
    let ranked = ranked_by_edge_count(
        graph
            .edges
            .iter()
            .filter(|e| e.relation == architecture::RELATION_IMPORTS)
            .map(|e| e.source.as_str()),
    );
    let ranked: Vec<(String, usize)> = ranked
        .into_iter()
        .filter(|(id, _)| !id.starts_with("module::external::"))
        .collect();
    let outgoing: std::collections::HashMap<&str, usize> =
        ranked.iter().map(|(id, c)| (id.as_str(), *c)).collect();

    let top_n = (limit * 3).max(0) as usize;
    let top_ids: Vec<String> = ranked
        .iter()
        .take(top_n)
        .map(|(id, _)| id.clone())
        .collect();

    let mut transitive_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for node_id in &top_ids {
        transitive_counts.insert(
            node_id.clone(),
            architecture::transitive_dependencies(&graph, node_id, depth).len(),
        );
    }

    // Rank by (transitive, outgoing) descending. A stable sort on the
    // negated key tuple keeps the original relative order for equal keys
    // (ties are not reversed — only the comparison is).
    let mut re_ranked = top_ids.clone();
    re_ranked.sort_by(|a, b| {
        let ta = *transitive_counts.get(a).unwrap_or(&0);
        let tb = *transitive_counts.get(b).unwrap_or(&0);
        let oa = *outgoing.get(a.as_str()).unwrap_or(&0);
        let ob = *outgoing.get(b.as_str()).unwrap_or(&0);
        (tb, ob).cmp(&(ta, oa))
    });
    re_ranked.truncate(limit.max(0) as usize);

    let mut rows: Vec<Value> = vec![];
    for node_id in &re_ranked {
        let node = match graph.nodes.get(node_id) {
            Some(n) => n,
            None => continue,
        };
        rows.push(json!({
            "rank": rows.len() + 1,
            "node_id": node.id,
            "label": node.label,
            "kind": node.kind,
            "source_file": node.source_file,
            "source_line": node.source_line,
            "direct_dependencies": *outgoing.get(node_id.as_str()).unwrap_or(&0),
            "transitive_dependencies": *transitive_counts.get(node_id).unwrap_or(&0),
        }));
    }

    let out = json!({
        "path": path.to_string_lossy(),
        "limit": limit,
        "depth": depth,
        "mode": "upstream",
        "results": rows,
    });

    if !as_json {
        if rows.is_empty() {
            println!("(no nodes in the architecture graph — cache may be empty; run `trace cache build`)");
        } else {
            println!(
                "Top {} highest-coupling nodes in {} (transitive depth ≤ {depth}):",
                rows.len(),
                path.to_string_lossy(),
            );
            println!(
                "  {:<3} {:>6}  {:>10}  {:<10}  symbol @ source",
                "#", "direct", "transitive", "kind"
            );
            for row in &rows {
                let location = match row["source_file"].as_str() {
                    Some(f) if !f.is_empty() => format!(
                        "{f}:{}",
                        row["source_line"]
                            .as_i64()
                            .map(|l| l.to_string())
                            .unwrap_or_else(|| "None".into())
                    ),
                    _ => "(no source)".to_string(),
                };
                println!(
                    "  {:<3} {:>6}  {:>10}  {:<10}  {} @ {}",
                    row["rank"].as_i64().unwrap_or(0),
                    row["direct_dependencies"].as_i64().unwrap_or(0),
                    row["transitive_dependencies"].as_i64().unwrap_or(0),
                    row["kind"].as_str().unwrap_or(""),
                    row["label"].as_str().unwrap_or(""),
                    location,
                );
            }
        }
    }
    Ok(out)
}

/// Replicates `collections.Counter(...).most_common()`: count occurrences,
/// then order by count descending with ties in first-seen order.
fn ranked_by_edge_count<'a>(
    items: impl Iterator<Item = &'a str>,
) -> Vec<(String, usize)> {
    let mut counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    let mut first_seen: Vec<String> = vec![];
    for it in items {
        let entry = counts.entry(it.to_string()).or_insert_with(|| {
            first_seen.push(it.to_string());
            0
        });
        *entry += 1;
    }
    let mut ranked: Vec<(String, usize)> = first_seen
        .into_iter()
        .map(|k| {
            let c = counts[&k];
            (k, c)
        })
        .collect();
    // Stable sort by count descending; ties keep first-seen order.
    ranked.sort_by(|a, b| b.1.cmp(&a.1));
    ranked
}
