//! `trace callers <symbol>` — direct callers / use sites of a symbol via the
//! architecture graph.
//!
//! For a symbol query each caller row is a use site resolved to its calling
//! function (the reference edge's function-granular source), carrying that
//! caller's signature (reused from the `structure` extractor) so the asking
//! agent gets the calling symbol's surface in the same call. Rows are ordered
//! confidence-first — resolved (EXTRACTED / INFERRED) callers ahead of
//! AMBIGUOUS ones — and each matched symbol carries a count summary
//! (`caller_count`, `resolved_count`, `ambiguous_count`) so the agent sees at
//! a glance how much of the answer is confirmed.

use crate::commands::enrich;
use crate::commands::signatures::{self, Signature};
use crate::{architecture, cache};
use anyhow::Result;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Confidence sort rank: resolved callers first, ambiguous last. Within a
/// rank the caller of a deterministic file:line order keeps output stable.
fn confidence_rank(confidence: &str) -> u8 {
    match confidence {
        architecture::CONFIDENCE_EXTRACTED => 0,
        architecture::CONFIDENCE_INFERRED => 1,
        architecture::CONFIDENCE_AMBIGUOUS => 2,
        _ => 3,
    }
}

/// The signature `extra` JSON for the calling symbol at `source_file:line`
/// named `name`, or `Value::Null` when the file's language has no signature
/// extractor or no signature matches. `cache` memoizes the per-file
/// signature list so a file with N caller rows is read and parsed once.
fn signature_for(
    source_file: Option<&str>,
    source_line: Option<i64>,
    label: &str,
    repo_root: &Path,
    cache: &mut HashMap<String, Vec<Signature>>,
) -> Value {
    let (file, line) = match (source_file, source_line) {
        (Some(f), Some(l)) if !f.is_empty() => (f, l),
        _ => return Value::Null,
    };
    let sigs = cache.entry(file.to_string()).or_insert_with(|| {
        let abs: PathBuf = repo_root.join(file);
        match std::fs::read(&abs) {
            Ok(source) => signatures::extract(&source, &abs),
            Err(_) => Vec::new(),
        }
    });
    sigs.iter()
        .find(|s| s.line == line && s.name == label)
        .map(|s| s.extra.clone())
        .unwrap_or(Value::Null)
}

/// One caller row: the source node's identity plus the edge's confidence,
/// use-site location, and (for a function-granular source) the calling
/// symbol's signature.
fn caller_row(
    source_node: &architecture::Node,
    location_file: Option<&str>,
    location_line: Option<i64>,
    relation: &str,
    confidence: &str,
    repo_root: &Path,
    sig_cache: &mut HashMap<String, Vec<Signature>>,
) -> Value {
    // The signature is the CALLING SYMBOL's surface, so it is looked up at
    // that symbol's declaration coordinates (`source_node`), not the
    // use-site location — a function called on line 9 may be declared on
    // line 3, and the signature lives at the declaration.
    let signature = signature_for(
        source_node.source_file.as_deref(),
        source_node.source_line,
        &source_node.label,
        repo_root,
        sig_cache,
    );
    json!({
        "node_id": source_node.id,
        "label": source_node.label,
        "kind": source_node.kind,
        "source_file": location_file,
        "source_line": location_line,
        "relation": relation,
        "confidence": confidence,
        "signature": signature,
    })
}

pub fn run(symbol: &str, as_json: bool) -> Result<Value> {
    let here = Path::new(".");
    let repo_root = cache::worktree_root_for(here).unwrap_or_else(|| cache::display_root(here));
    let graph = architecture::get(&repo_root);
    let matches = architecture::find_symbols(&graph, symbol);

    if matches.is_empty() {
        eprintln!("Symbol '{symbol}' not found in architecture graph.");
        std::process::exit(2);
    }

    // One signature list per source file, shared across every caller row of
    // every matched symbol — a file is read and parsed at most once.
    let mut sig_cache: HashMap<String, Vec<Signature>> = HashMap::new();

    // Canonical file-state shoulder per caller use-site file, batched once
    // across every matched symbol's rows. The use-site file (the reference
    // edge's source_file) is the file the agent would open to read the call,
    // so its lifecycle/complexity state is what each caller row carries.
    let row_files: Vec<String> = matches
        .iter()
        .flat_map(|m| {
            let symbol_rows = if m.kind == "module" {
                architecture::dependents_of(&graph, &m.id)
            } else {
                architecture::references_to(&graph, &m.id)
            };
            symbol_rows
                .into_iter()
                .filter_map(|e| {
                    e.source_file.clone().or_else(|| {
                        graph
                            .nodes
                            .get(&e.source)
                            .and_then(|n| n.source_file.clone())
                    })
                })
                .collect::<Vec<_>>()
        })
        .collect();
    let shoulders = enrich::file_shoulders(&row_files, &repo_root);

    let mut output = Map::new();
    for m in &matches {
        let mut callers: Vec<Value> = vec![];
        if m.kind == "module" {
            // Module-granular query: the importer modules of the module. A
            // module has no calling-function source, so its rows carry the
            // importer module's own coordinates and a null signature.
            for edge in architecture::dependents_of(&graph, &m.id) {
                let source_node = match graph.nodes.get(&edge.source) {
                    Some(n) => n,
                    None => continue,
                };
                callers.push(caller_row(
                    source_node,
                    source_node.source_file.as_deref(),
                    source_node.source_line,
                    &edge.relation,
                    &edge.confidence,
                    &repo_root,
                    &mut sig_cache,
                ));
            }
        } else {
            // Symbol-granular query: rows are USE SITES. Each carries the
            // calling function's file:line (the reference edge's
            // function-granular source) and that caller's signature.
            for edge in architecture::references_to(&graph, &m.id) {
                let source_node = match graph.nodes.get(&edge.source) {
                    Some(n) => n,
                    None => continue,
                };
                callers.push(caller_row(
                    source_node,
                    edge.source_file.as_deref(),
                    edge.source_line,
                    &edge.relation,
                    &edge.confidence,
                    &repo_root,
                    &mut sig_cache,
                ));
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
                            callers.push(caller_row(
                                source_node,
                                source_node.source_file.as_deref(),
                                source_node.source_line,
                                &edge.relation,
                                &edge.confidence,
                                &repo_root,
                                &mut sig_cache,
                            ));
                        }
                    }
                }
            }
        }

        // Confidence-first ordering: resolved callers ahead of ambiguous
        // ones; ties broken by file then line so output is deterministic.
        callers.sort_by(|a, b| {
            let ra = confidence_rank(a["confidence"].as_str().unwrap_or(""));
            let rb = confidence_rank(b["confidence"].as_str().unwrap_or(""));
            ra.cmp(&rb)
                .then_with(|| {
                    a["source_file"]
                        .as_str()
                        .unwrap_or("")
                        .cmp(b["source_file"].as_str().unwrap_or(""))
                })
                .then_with(|| {
                    a["source_line"]
                        .as_i64()
                        .unwrap_or(0)
                        .cmp(&b["source_line"].as_i64().unwrap_or(0))
                })
        });

        // Attach the canonical file-state shoulder to each caller row from
        // its use-site file. A row whose use-site file has no resolvable
        // facts (external node) carries a null shoulder.
        for caller in &mut callers {
            let shoulder = caller["source_file"]
                .as_str()
                .and_then(|f| shoulders.get(f))
                .cloned();
            caller["shoulder"] = json!(shoulder);
        }

        let caller_count = callers.len() as i64;
        let ambiguous_count = callers
            .iter()
            .filter(|c| {
                c["confidence"].as_str() == Some(architecture::CONFIDENCE_AMBIGUOUS)
            })
            .count() as i64;
        let resolved_count = caller_count - ambiguous_count;

        output.insert(
            m.id.clone(),
            json!({
                "symbol": m.label,
                "kind": m.kind,
                "source_file": m.source_file,
                "source_line": m.source_line,
                "caller_count": caller_count,
                "resolved_count": resolved_count,
                "ambiguous_count": ambiguous_count,
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
            let entry = &output[&m.id];
            let callers = entry["callers"].as_array().unwrap();
            if callers.is_empty() {
                println!("  (no architecture-graph callers found)");
                continue;
            }
            println!(
                "  callers ({}): {} resolved, {} ambiguous",
                entry["caller_count"].as_i64().unwrap_or(0),
                entry["resolved_count"].as_i64().unwrap_or(0),
                entry["ambiguous_count"].as_i64().unwrap_or(0),
            );
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
                if let Some(s) = caller["shoulder"].as_str() {
                    println!("        {s}");
                }
            }
        }
    }
    Ok(Value::Object(output))
}
