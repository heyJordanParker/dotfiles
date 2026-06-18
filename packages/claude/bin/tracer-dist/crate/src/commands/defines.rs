//! `trace defines <symbol>` — where a symbol is defined, via the architecture
//! graph.

use crate::commands::enrich;
use crate::{architecture, cache};
use anyhow::Result;
use serde_json::{json, Value};
use std::path::Path;

pub fn run(symbol: &str, as_json: bool) -> Result<Value> {
    let here = Path::new(".");
    let repo_root = cache::worktree_root_for(here).unwrap_or_else(|| cache::display_root(here));
    let graph = architecture::get(&repo_root);
    let matches = architecture::find_symbols(&graph, symbol);

    if matches.is_empty() {
        eprintln!("Symbol '{symbol}' not found in architecture graph.");
        std::process::exit(2);
    }

    let source_files: Vec<String> =
        matches.iter().filter_map(|m| m.source_file.clone()).collect();
    let shoulders = enrich::file_shoulders(&source_files, &repo_root);

    let definitions: Vec<_> = matches
        .iter()
        .map(|m| {
            json!({
                "node_id": m.id,
                "label": m.label,
                "kind": m.kind,
                "source_file": m.source_file,
                "source_line": m.source_line,
                "shoulder": m.source_file.as_deref().and_then(|f| shoulders.get(f)),
            })
        })
        .collect();
    let out = json!({
        "symbol": symbol,
        "definitions": definitions,
        "definition_count": matches.len(),
    });

    if !as_json {
        println!("Definitions of '{symbol}' ({}):", matches.len());
        for m in &matches {
            println!(
                "  [{}] {} @ {}:{}",
                m.kind,
                m.label,
                m.source_file.as_deref().unwrap_or("None"),
                m.source_line
                    .map(|l| l.to_string())
                    .unwrap_or_else(|| "None".into()),
            );
            if let Some(s) = m.source_file.as_deref().and_then(|f| shoulders.get(f)) {
                println!("      {s}");
            }
        }
    }
    Ok(out)
}
