//! `trace structure` — the cached declaration record as source rows, for
//! every file the path arguments name (a directory names the files under it).

use crate::summary::Facts;
use crate::{cache, file_facts, relations, surface};
use anyhow::Result;
use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};

/// One file's structure: its facts, its rows, and its imports and exports.
struct FileStructure {
    relative: String,
    facts: Map<String, Value>,
    rows: Vec<surface::Row>,
    imports: Vec<Value>,
    exports: Vec<Value>,
}

fn file_structure(path: &Path) -> FileStructure {
    let path = cache::absolutize(path);
    let root = cache::worktree_root_for(&path);
    let repo_root = root.clone().unwrap_or_else(|| cache::display_root(&path));
    let relative = cache::relative_to_root(&path, &repo_root);
    let facts = file_facts::get(&path, &repo_root);
    let (language, imports, exports, rows) = facts
        .as_ref()
        .and_then(|facts| facts.extraction.as_ref().map(|extraction| {
            (
                extraction.language.clone(),
                extraction.imports.iter().map(|import| json!({"module": import.module, "symbol": import.symbol, "line": import.line})).collect::<Vec<_>>(),
                extraction.exports.iter().map(|export| json!({"name": export.name, "kind": export.kind, "line": export.line})).collect::<Vec<_>>(),
                surface::rows(facts, None),
            )
        }))
        .unwrap_or_else(|| (String::new(), Vec::new(), Vec::new(), Vec::new()));
    let mut file_facts = facts
        .as_ref()
        .map(|facts| Facts::of(facts, relations::get(&repo_root).module_counts(&relative).as_ref()).to_map())
        .unwrap_or_default();
    // Outside any git repository there is no git state to describe.
    if root.is_none() {
        file_facts.remove("git");
    }
    if !language.is_empty() {
        file_facts.insert("language".into(), language.into());
    }
    FileStructure {
        relative,
        facts: file_facts,
        rows,
        imports,
        exports,
    }
}

pub fn run(paths: &[PathBuf], as_json: bool) -> Result<Value> {
    let structures: Vec<FileStructure> = crate::pathval::files_under(paths, "PATH")
        .iter()
        .map(|path| file_structure(path))
        .collect();

    // One shape for one file and for many: every import, export and row
    // carries the file it came from, so several files merge into the same
    // three lists a single file fills.
    let mut context = Map::new();
    let mut imports = Vec::new();
    let mut exports = Vec::new();
    let mut symbols_by_kind = Map::new();
    let with_file = |value: &Value, file: &str| {
        let mut value = value.clone();
        value["file"] = json!(file);
        value
    };
    for structure in &structures {
        let file = structure.relative.as_str();
        imports.extend(structure.imports.iter().map(|import| with_file(import, file)));
        exports.extend(structure.exports.iter().map(|export| with_file(export, file)));
        for row in &structure.rows {
            let mut value = serde_json::to_value(row).expect("surface row is serializable");
            value["node_id"] = json!(relations::symbol_id(file, &row.name));
            value["file"] = json!(file);
            symbols_by_kind
                .entry(row.kind.clone())
                .or_insert_with(|| Value::Array(Vec::new()))
                .as_array_mut()
                .expect("symbols are arrays")
                .push(value);
        }
        context.insert(structure.relative.clone(), Value::Object(structure.facts.clone()));
    }
    let document = crate::output::document(
        json!({"paths": paths}),
        json!({"files": context}),
        json!({"imports": imports, "exports": exports, "symbols_by_kind": symbols_by_kind}),
        json!({
            "files": structures.len(),
            "symbols": structures.iter().map(|s| s.rows.len()).sum::<usize>(),
            "imports": structures.iter().map(|s| s.imports.len()).sum::<usize>(),
            "exports": structures.iter().map(|s| s.exports.len()).sum::<usize>(),
        }),
    );
    if as_json {
        return Ok(document);
    }

    // One front-matter document per file, each still naming every
    // declaration; together they share the budget, and the files the fewest
    // others import give up their detail first.
    let closing = crate::output::closing_room(structures.len(), "files");
    let share = crate::output::budget()
        .map(|budget| budget.saturating_sub(closing) / structures.len().max(1));
    let entries: Vec<crate::output::Entry> = structures
        .iter()
        .map(|structure| {
            let mut front_matter = structure.facts.clone();
            front_matter.entry("file").or_insert_with(|| structure.relative.clone().into());
            front_matter.insert(
                "declarations".into(),
                json!({"symbols": structure.rows.len(), "imports": structure.imports.len(), "exports": structure.exports.len()}),
            );
            let front_matter = crate::summary::front_matter(&front_matter);
            let rows = &structure.rows;
            let within = share.map(|share| share.saturating_sub(front_matter.len()));
            crate::output::Entry {
                rank: structure.facts.get("imported_by").and_then(Value::as_i64).unwrap_or(0),
                levels: vec![
                    format!("{front_matter}{}", surface::render_within(rows, &structure.relative, None, None)),
                    format!("{front_matter}{}", surface::render_within(rows, &structure.relative, None, within)),
                    front_matter,
                    format!("---\nfile: {}\n---\n", structure.relative),
                ],
            }
        })
        .collect();
    let (texts, shortened) = crate::output::fit(&entries, closing);
    for text in texts {
        print!("{text}");
    }
    if shortened > 0 {
        println!("{}", crate::output::shortened_line(shortened, entries.len(), "files"));
    }
    Ok(document)
}
