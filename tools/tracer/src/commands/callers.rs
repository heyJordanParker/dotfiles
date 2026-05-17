//! `trace callers <symbol>` — direct callers / importers of a symbol via the
//! architecture graph.

use crate::{architecture, cache};
use anyhow::Result;
use serde_json::{json, Map, Value};
use std::path::Path;

pub fn run(symbol: &str, as_json: bool) -> Result<Value> {
    let repo_root = cache::repo_root_for(Path::new("."));
    let graph = architecture::get(&repo_root);
    let matches = architecture::find_symbols(&graph, symbol);

    if matches.is_empty() {
        eprintln!("Symbol '{symbol}' not found in architecture graph.");
        std::process::exit(2);
    }

    let mut output = Map::new();
    for m in &matches {
        let edges = architecture::dependents_of(&graph, &m.id);
        let mut callers: Vec<Value> = vec![];
        for edge in edges {
            let source_node = match graph.nodes.get(&edge.source) {
                Some(n) => n,
                None => continue,
            };
            callers.push(json!({
                "node_id": source_node.id,
                "label": source_node.label,
                "kind": source_node.kind,
                "source_file": source_node.source_file,
                "source_line": source_node.source_line,
                "relation": edge.relation,
                "confidence": edge.confidence,
            }));
        }
        output.insert(
            m.id.clone(),
            json!({
                "symbol": m.label,
                "kind": m.kind,
                "source_file": m.source_file,
                "source_line": m.source_line,
                "callers": callers,
            }),
        );
    }

    if !as_json {
        for m in &matches {
            println!(
                "\n{} [{}] @ {}:{}",
                m.label,
                m.kind,
                m.source_file.as_deref().unwrap_or("None"),
                m.source_line
                    .map(|l| l.to_string())
                    .unwrap_or_else(|| "None".into()),
            );
            let callers = output[&m.id]["callers"].as_array().unwrap();
            if callers.is_empty() {
                println!("  (no architecture-graph callers found)");
                continue;
            }
            println!("  callers ({}):", callers.len());
            for caller in callers {
                let source_file = caller["source_file"].as_str();
                let location = match source_file {
                    Some(f) if !f.is_empty() => format!(
                        "{f}:{}",
                        caller["source_line"]
                            .as_i64()
                            .map(|l| l.to_string())
                            .unwrap_or_else(|| "None".into())
                    ),
                    _ => "(external)".to_string(),
                };
                println!(
                    "    [{}] {} [{}] @ {}",
                    caller["confidence"].as_str().unwrap_or(""),
                    caller["label"].as_str().unwrap_or(""),
                    caller["kind"].as_str().unwrap_or(""),
                    location,
                );
            }
        }
    }
    Ok(Value::Object(output))
}
