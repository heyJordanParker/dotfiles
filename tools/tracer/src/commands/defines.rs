//! `trace defines <symbol>` — where a symbol is defined.
//!
//! The symbol index names the files that declare it; those files carry the
//! kind, line and container. Nothing else is read, so the cost is the answer.

use crate::commands::enrich;
use crate::{cache, relations};
use anyhow::Result;
use serde_json::{json, Value};
use std::path::Path;

pub fn run(symbol: &str, as_json: bool) -> Result<Value> {
    let here = Path::new(".");
    let repo_root = cache::worktree_root_for(here).unwrap_or_else(|| cache::display_root(here));
    let matches = relations::declarations(symbol, &repo_root);

    if matches.is_empty() {
        eprintln!("Symbol '{symbol}' not declared anywhere in this repository.");
        std::process::exit(2);
    }

    let source_files: Vec<String> = matches.iter().map(|m| m.file.clone()).collect();
    let shoulders = enrich::file_shoulders(&source_files, &repo_root);

    let definitions: Vec<_> = matches
        .iter()
        .map(|m| {
            json!({
                "node_id": relations::symbol_id(&m.file, &m.declaration.name),
                "label": m.declaration.name,
                "kind": m.declaration.kind,
                "source_file": m.file,
                "source_line": m.declaration.line,
            })
        })
        .collect();
    let out = crate::output::document(
        json!({"symbol": symbol}),
        json!({"files": enrich::shoulder_context(&shoulders)}),
        json!(definitions),
        json!({"definitions": matches.len()}),
    );

    if !as_json {
        println!("Definitions of '{symbol}' ({}):", matches.len());
        for m in &matches {
            println!(
                "  [{}] {} @ {}:{}",
                m.declaration.kind, m.declaration.name, m.file, m.declaration.line,
            );
            if let Some(s) = shoulders.get(&m.file) {
                println!("      {s}");
            }
        }
    }
    Ok(out)
}
