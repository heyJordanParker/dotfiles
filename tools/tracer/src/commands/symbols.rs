//! `trace symbols <file>` — module-level symbols of a file from the
//! architecture graph.

use crate::commands::enrich;
use crate::{architecture, cache};
use anyhow::Result;
use serde_json::{json, Value};
use std::path::Path;

pub fn run(file: &Path, as_json: bool) -> Result<Value> {
    crate::pathval::require_file(file, "FILE");
    let target = cache::absolutize(file);
    let repo_root = cache::worktree_root_for(&target).unwrap_or_else(|| cache::display_root(&target));
    let relative = cache::relative_to_root(&target, &repo_root);

    let shoulder = enrich::file_shoulders(&[relative.clone()], &repo_root)
        .remove(&relative);

    let graph = architecture::get(&repo_root);
    let mut file_symbols: Vec<&architecture::Node> = graph
        .node_order
        .iter()
        .filter_map(|id| graph.nodes.get(id))
        .filter(|n| n.source_file.as_deref() == Some(relative.as_str()) && n.kind != "module")
        .collect();

    let symbols: Vec<_> = file_symbols
        .iter()
        .map(|s| {
            json!({
                "node_id": s.id,
                "label": s.label,
                "kind": s.kind,
                "source_line": s.source_line,
            })
        })
        .collect();
    let out = json!({
        "file": relative,
        "shoulder": shoulder,
        "symbols": symbols,
        "symbol_count": file_symbols.len(),
    });

    if !as_json {
        println!("Symbols in {relative} ({}):", file_symbols.len());
        if let Some(s) = &shoulder {
            println!("  {s}");
        }
        if file_symbols.is_empty() {
            println!("  (no module-level symbols found in architecture graph)");
            println!("  (file may not have been extracted — check supported extensions via `trace doctor`)");
        } else {
            file_symbols.sort_by_key(|n| n.source_line.unwrap_or(0));
            for s in &file_symbols {
                println!(
                    "  L{:<5} [{}] {}",
                    s.source_line.unwrap_or(0),
                    s.kind,
                    s.label,
                );
            }
        }
    }
    Ok(out)
}
