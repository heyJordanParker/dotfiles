//! `trace defines <symbol>` — where a symbol is defined.
//!
//! The symbol index names the files that declare it; those files carry the
//! kind, line and container. Nothing else is read, so the cost is the answer.

use crate::commands::enrich;
use crate::{cache, file_facts, relations, surface};
use anyhow::Result;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub fn run(symbol: &str, as_json: bool) -> Result<Value> {
    let here = Path::new(".");
    let repo_root = cache::worktree_root_for(here).unwrap_or_else(|| cache::display_root(here));
    let matches = relations::declarations(symbol, &repo_root);

    if matches.is_empty() {
        eprintln!("{}", crate::output::not_declared(symbol, &repo_root));
        std::process::exit(2);
    }

    let mut source_files: Vec<String> = matches.iter().map(|m| m.file.clone()).collect();
    source_files.sort();
    source_files.dedup();
    let paths: Vec<PathBuf> = source_files.iter().map(|file| repo_root.join(file)).collect();
    let facts = file_facts::get_batch(&paths, &repo_root);
    let file_facts_by_path = enrich::facts_of(&facts, &repo_root);
    let rows: std::collections::HashMap<&str, Vec<surface::Row>> = facts
        .iter()
        .map(|(file, facts)| (file.as_str(), surface::rows(facts, None)))
        .collect();
    let declarations: Vec<Option<&surface::Row>> = matches
        .iter()
        .map(|m| {
            rows.get(m.file.as_str())
                .and_then(|rows| rows.iter().find(|row| row.line == m.declaration.line && row.name == m.declaration.name))
        })
        .collect();

    let definitions: Vec<_> = matches
        .iter()
        .zip(&declarations)
        .map(|(m, declaration)| {
            json!({
                "node_id": relations::symbol_id(&m.file, &m.declaration.name),
                "label": m.declaration.name,
                "kind": m.declaration.kind,
                "source_file": m.file,
                "source_line": m.declaration.line,
                "declaration": declaration,
            })
        })
        .collect();
    let out = crate::output::document(
        json!({"symbol": symbol}),
        json!({"files": enrich::facts_context(&file_facts_by_path)}),
        json!(definitions),
        json!({"definitions": matches.len()}),
    );

    if !as_json {
        let mut files: Vec<(String, Vec<String>, usize)> = Vec::new();
        for (m, declaration) in matches.iter().zip(&declarations) {
            let rendered = declaration
                .map(|declaration| surface::inline(declaration, &m.file))
                .unwrap_or_else(|| format!("[{}] {}", m.declaration.kind, m.declaration.name));
            if files.last().is_none_or(|(last, _, _)| *last != m.file) {
                files.push((m.file.clone(), Vec::new(), 0));
            }
            let (_, rows, count) = files.last_mut().unwrap();
            rows.push(format!("      L{:<5}{rendered}", m.declaration.line));
            *count += 1;
        }
        let section = enrich::Section {
            heading: format!("Definitions of '{symbol}' ({}):", matches.len()),
            files,
        };
        enrich::render_sections(&[section], &file_facts_by_path, "definition", "definitions");
    }
    Ok(out)
}
