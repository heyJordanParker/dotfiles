//! `trace structure` — the cached declaration record as source rows, for
//! every file the path arguments name (a directory names the files under it).

use super::session_log::ShownRecord;
use crate::summary::Facts;
use crate::{cache, file_facts, relations, surface};
use anyhow::Result;
use serde_json::{json, Map, Value};
use std::io::Write as _;
use std::path::{Path, PathBuf};

/// One file's structure: its facts, its rows, and its imports and exports.
struct FileStructure {
    path: PathBuf,
    relative: String,
    shown_facts: Option<Facts>,
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
    let shown_facts = facts
        .as_ref()
        .map(|facts| Facts::of(facts, relations::get(&repo_root).module_counts(&relative).as_ref()));
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
    let mut file_facts = shown_facts.as_ref().map(Facts::to_map).unwrap_or_default();
    // Outside any git repository there is no git state to describe.
    if root.is_none() {
        file_facts.remove("git");
    }
    if !language.is_empty() {
        file_facts.insert("language".into(), language.into());
    }
    FileStructure {
        path,
        relative,
        shown_facts,
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

    // One front-matter document per file, each naming every declaration;
    // together they share the budget, and the files the fewest others import
    // give up their detail first: the block becomes the one-line headline
    // before any declaration goes, because the declarations are what this
    // command is for.
    let closing = crate::output::closing_room(structures.len(), "files");
    let share = crate::output::budget()
        .map(|budget| budget.saturating_sub(closing) / structures.len().max(1));
    let front_matters: Vec<Map<String, Value>> = structures
        .iter()
        .map(|structure| {
            let mut front_matter = structure.facts.clone();
            front_matter.entry("file").or_insert_with(|| structure.relative.clone().into());
            front_matter.insert(
                "declarations".into(),
                json!({"symbols": structure.rows.len(), "imports": structure.imports.len(), "exports": structure.exports.len()}),
            );
            front_matter
        })
        .collect();
    let blocks: Vec<String> = front_matters.iter().map(crate::summary::front_matter).collect();
    let wholes: Vec<String> = structures
        .iter()
        .map(|structure| surface::render_within(&structure.rows, &structure.relative, None, None))
        .collect();
    let entries: Vec<crate::output::Entry> = structures
        .iter()
        .zip(&blocks)
        .zip(&wholes)
        .map(|((structure, block), whole)| {
            let headline = structure.shown_facts.as_ref().map(|facts| facts.headline()).unwrap_or_default();
            let line = format!("{}  {headline}\n", structure.relative);
            let within = share.map(|share| share.saturating_sub(line.len()));
            let cut = surface::render_within(&structure.rows, &structure.relative, None, within);
            crate::output::Entry {
                rank: structure.facts.get("imported_by").and_then(Value::as_i64).unwrap_or(0),
                levels: vec![
                    format!("{block}{whole}"),
                    format!("{line}{whole}"),
                    format!("{line}{cut}"),
                    line,
                    format!("{}\n", structure.relative),
                ],
            }
        })
        .collect();
    // Every file shares one level, the most detailed one they all fit at, so
    // a directory never prints one file whole beside bare paths; `fit` then
    // lifts the most imported files one level above it.
    let shared = crate::output::budget().map_or(0, |budget| {
        (0..4)
            .find(|&level| {
                closing + entries.iter().map(|entry| crate::output::width(&entry.levels[level]) + 1).sum::<usize>()
                    <= budget
            })
            .unwrap_or(4)
    });
    let start = shared.saturating_sub(1);
    let offered: Vec<crate::output::Entry> = entries
        .iter()
        .map(|entry| crate::output::Entry {
            rank: entry.rank,
            levels: entry.levels[start..].to_vec(),
        })
        .collect();
    let (chosen, _) = crate::output::fit(&offered, closing);
    let shortened = chosen.iter().filter(|(level, _)| start + level > 0).count();
    for (index, (level, text)) in chosen.into_iter().enumerate() {
        let structure = &structures[index];
        match start + level {
            0 => {
                let surface = &wholes[index];
                let mut record = ShownRecord::default();
                let front_matter = crate::summary::front_matter_once(
                    None,
                    &structure.path,
                    structure.shown_facts.as_ref(),
                    &front_matters[index],
                    &mut record,
                    true,
                );
                print!("{front_matter}{surface}");
                std::io::stdout().flush()?;
                record.save();
            }
            _ => print!("{text}"),
        }
    }
    if shortened > 0 {
        println!("{}", crate::output::shortened_line(shortened, entries.len(), "files"));
    }
    Ok(document)
}
