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
        let mut callers: Vec<Value> = vec![];
        if m.kind == "module" {
            // Module-granular query: keep the existing importer-module
            // answer — these are the modules that import the module.
            for edge in architecture::dependents_of(&graph, &m.id) {
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
        } else {
            // Symbol-granular query: rows are USE SITES. Each carries its
            // own file:line drawn from the reference edge, so two calls in
            // one file appear as two distinct rows.
            for edge in architecture::references_to(&graph, &m.id) {
                let source_node = match graph.nodes.get(&edge.source) {
                    Some(n) => n,
                    None => continue,
                };
                callers.push(json!({
                    "node_id": source_node.id,
                    "label": source_node.label,
                    "kind": source_node.kind,
                    "source_file": edge.source_file,
                    "source_line": edge.source_line,
                    "relation": edge.relation,
                    "confidence": edge.confidence,
                }));
            }
            // Fallback: when the symbol has zero reference rows, fall back
            // to the module-importer answer. The pre-symbol-index behaviour
            // listed importing modules for any symbol query; preserving
            // that capability matters most for class symbols whose use
            // sites the reference walker can't catch (or that the
            // codebase actually only uses by importing). Without this, a
            // class used everywhere via `use App\Models\User;` returns
            // zero callers — strictly worse than the prior behaviour.
            if callers.is_empty() {
                if let Some(sf) = &m.source_file {
                    if let Some(owning_module) = graph.file_to_module_id.get(sf) {
                        for edge in architecture::dependents_of(&graph, owning_module) {
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
                    }
                }
            }
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
