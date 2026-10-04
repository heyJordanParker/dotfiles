//! `trace callers <symbol>` — who calls this, resolved now.
//!
//! A symbol query's rows are USE SITES: the symbol index names the files that
//! mention the name, those files are loaded, and their references resolve
//! against the declarations. A module query's rows are the files that import
//! it, straight off the import inversion.
//!
//! Cost is the answer: `callers Model` on laravel-framework reaches 82 files
//! out of 3,198, and reads only those.

use crate::commands::enrich;
use crate::{cache, file_facts, relations, surface};
use anyhow::Result;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

fn confidence_rank(confidence: &str) -> u8 {
    match confidence {
        relations::CONFIDENCE_EXTRACTED => 0,
        relations::CONFIDENCE_INFERRED => 1,
        _ => 2,
    }
}

#[derive(Serialize)]
struct Caller {
    node_id: String,
    label: String,
    kind: String,
    source_file: String,
    source_line: i64,
    relation: &'static str,
    confidence: &'static str,
    declaration: Option<surface::Row>,
}

struct Symbol {
    node_id: String,
    label: String,
    kind: String,
    source_file: String,
    source_line: i64,
    callers: Vec<Caller>,
}

impl Symbol {
    fn new(node_id: String, label: &str, kind: &str, source_file: &str, source_line: i64, mut callers: Vec<Caller>) -> Self {
        callers.sort_by(|a, b| {
            confidence_rank(a.confidence)
                .cmp(&confidence_rank(b.confidence))
                .then_with(|| a.source_file.cmp(&b.source_file))
                .then_with(|| a.source_line.cmp(&b.source_line))
        });
        Self {
            node_id,
            label: label.to_string(),
            kind: kind.to_string(),
            source_file: source_file.to_string(),
            source_line,
            callers,
        }
    }

    fn ambiguous(&self) -> usize {
        self.callers
            .iter()
            .filter(|caller| caller.confidence == relations::CONFIDENCE_AMBIGUOUS)
            .count()
    }

    fn to_value(&self) -> Value {
        let ambiguous = self.ambiguous();
        json!({
            "node_id": self.node_id,
            "symbol": self.label,
            "kind": self.kind,
            "source_file": self.source_file,
            "source_line": self.source_line,
            "caller_count": self.callers.len(),
            "resolved_count": self.callers.len() - ambiguous,
            "ambiguous_count": ambiguous,
            "callers": self.callers,
        })
    }
}

enum Edge<'a> {
    Reference(&'a relations::UseSite),
    Import(&'a relations::Importer),
}

impl Edge<'_> {
    fn order(&self) -> (u8, &str, i64) {
        match self {
            Edge::Reference(site) => (confidence_rank(site.confidence), &site.file, site.line),
            Edge::Import(importer) => (confidence_rank(importer.confidence), &importer.file, 1),
        }
    }

    fn to_caller(&self, rows: &HashMap<&str, Vec<surface::Row>>, index: &relations::Relations) -> Caller {
        match self {
            Edge::Reference(site) => match &site.caller {
                Some(caller) => Caller {
                    node_id: relations::symbol_id(&site.file, &caller.name),
                    label: caller.name.clone(),
                    kind: caller.kind.clone(),
                    source_file: site.file.clone(),
                    source_line: site.line,
                    relation: "references",
                    confidence: site.confidence,
                    declaration: rows
                        .get(site.file.as_str())
                        .and_then(|rows| rows.iter().find(|row| row.line == caller.line && row.name == caller.name))
                        .cloned(),
                },
                None => {
                    let language = index.language(&site.file);
                    Caller {
                        node_id: relations::module_id(&site.file, language),
                        label: relations::file_to_module(&site.file, language),
                        kind: "module".to_string(),
                        source_file: site.file.clone(),
                        source_line: site.line,
                        relation: "references",
                        confidence: site.confidence,
                        declaration: None,
                    }
                }
            },
            Edge::Import(importer) => {
                let language = index.language(&importer.file);
                Caller {
                    node_id: relations::module_id(&importer.file, language),
                    label: relations::file_to_module(&importer.file, language),
                    kind: "module".to_string(),
                    source_file: importer.file.to_string(),
                    source_line: 1,
                    relation: "imports",
                    confidence: importer.confidence,
                    declaration: None,
                }
            }
        }
    }
}

pub fn run(symbol: &str, limit: usize, as_json: bool) -> Result<Value> {
    let here = Path::new(".");
    let repo_root = cache::worktree_root_for(here).unwrap_or_else(|| cache::display_root(here));
    let candidates = relations::candidates(&[symbol], &repo_root);
    let declarations: Vec<&relations::Candidate> =
        candidates.iter().flat_map(relations::Candidates::declarations).collect();
    let modules = if declarations.is_empty() {
        relations::modules_named(symbol, &repo_root)
    } else {
        Vec::new()
    };

    if declarations.is_empty() && modules.is_empty() {
        eprintln!("Symbol '{symbol}' not declared anywhere in this repository.");
        std::process::exit(2);
    }
    if !declarations.is_empty()
        && declarations
            .iter()
            .all(|declaration| declaration.declaration.kind == "property")
    {
        eprintln!("a property has readers, not callers; tracer does not extract reads");
        std::process::exit(2);
    }

    let sites = if declarations.is_empty() {
        Vec::new()
    } else {
        crate::timing::phase("use_sites", || relations::use_sites(&candidates, None, &repo_root)).concat()
    };
    let index = relations::get(&repo_root);
    let declared: HashMap<(&str, &str, i64), usize> = declarations
        .iter()
        .enumerate()
        .map(|(at, declaration)| {
            let key = (declaration.file.as_str(), declaration.declaration.name.as_str(), declaration.declaration.line);
            (key, at)
        })
        .collect();
    let mut edges: Vec<(usize, Edge)> = sites
        .iter()
        .filter_map(|site| {
            let key = (site.target_file.as_str(), site.target.name.as_str(), site.target.line);
            declared.get(&key).map(|&at| (at, Edge::Reference(site)))
        })
        .collect();
    let referenced: HashSet<usize> = edges.iter().map(|(at, _)| *at).collect();
    let subjects: Vec<&str> = declarations
        .iter()
        .map(|declaration| declaration.file.as_str())
        .chain(modules.iter().map(String::as_str))
        .collect();
    for (at, file) in subjects.iter().enumerate().filter(|(at, _)| !referenced.contains(at)) {
        edges.extend(index.importers_of(file).iter().map(|importer| (at, Edge::Import(importer))));
    }
    edges.sort_by(|(_, a), (_, b)| a.order().cmp(&b.order()));
    let total = edges.len();
    let truncated = total > limit;
    edges.truncate(limit);

    let mut row_files: Vec<String> = edges.iter().map(|(_, edge)| edge.order().1.to_string()).collect();
    row_files.sort();
    row_files.dedup();
    let fact_paths: Vec<PathBuf> = row_files.iter().map(|file| repo_root.join(file)).collect();
    let facts = file_facts::get_batch(&fact_paths, &repo_root);
    let file_facts_by_path = enrich::facts_of(&facts, &repo_root);
    let rows: HashMap<&str, Vec<surface::Row>> = edges
        .iter()
        .filter_map(|(_, edge)| match edge {
            Edge::Reference(site) if site.caller.is_some() => Some(site.file.as_str()),
            _ => None,
        })
        .collect::<HashSet<&str>>()
        .into_iter()
        .filter_map(|file| facts.get(file).map(|facts| (file, surface::rows(facts, None))))
        .collect();

    let mut callers: Vec<Vec<Caller>> = subjects.iter().map(|_| Vec::new()).collect();
    for (at, edge) in &edges {
        callers[*at].push(edge.to_caller(&rows, &index));
    }
    let mut callers = callers.into_iter();
    let mut symbols: Vec<Symbol> = declarations
        .iter()
        .zip(&mut callers)
        .map(|(declaration, callers)| {
            Symbol::new(
                relations::symbol_id(&declaration.file, &declaration.declaration.name),
                &declaration.declaration.name,
                &declaration.declaration.kind,
                &declaration.file,
                declaration.declaration.line,
                callers,
            )
        })
        .collect();
    symbols.extend(modules.iter().zip(callers).map(|(file, callers)| {
        let language = index.language(file);
        Symbol::new(
            relations::module_id(file, language),
            &relations::file_to_module(file, language),
            "module",
            file,
            1,
            callers,
        )
    }));

    if !as_json {
        let sections: Vec<enrich::Section> = symbols.iter().map(section).collect();
        enrich::render_sections(&sections, &file_facts_by_path, "call site", "call sites");
        if truncated {
            println!("\n... {} more (see all: --limit {total})", total - edges.len());
        }
    }
    Ok(crate::output::document(
        json!({"symbol": symbol, "limit": limit}),
        json!({"files": enrich::facts_context(&file_facts_by_path)}),
        Value::Array(symbols.iter().map(Symbol::to_value).collect()),
        json!({
            "symbols": symbols.len(),
            "callers": edges.len(),
            "total": total,
            "truncated": truncated,
        }),
    ))
}

fn section(symbol: &Symbol) -> enrich::Section {
    let mut heading = format!("\n{} [{}] @ {}:{}", symbol.label, symbol.kind, symbol.source_file, symbol.source_line);
    if symbol.callers.is_empty() {
        heading.push_str("\n  (no callers found)");
    } else {
        let ambiguous = symbol.ambiguous();
        heading.push_str(&format!(
            "\n  callers ({}): {} resolved, {} ambiguous",
            symbol.callers.len(),
            symbol.callers.len() - ambiguous,
            ambiguous,
        ));
    }
    let mut files: Vec<(String, Vec<(String, Vec<String>)>, usize)> = Vec::new();
    for caller in &symbol.callers {
        let file = if caller.source_file.is_empty() { "(external)" } else { caller.source_file.as_str() };
        let rendered = caller
            .declaration
            .as_ref()
            .map_or_else(|| caller.label.clone(), |row| surface::inline(row, file));
        let text = if caller.confidence == relations::CONFIDENCE_EXTRACTED {
            format!("      {rendered}")
        } else {
            format!("      [{}] {rendered}", caller.confidence)
        };
        let line = format!("L{}", caller.source_line);
        if files.last().is_none_or(|(last, _, _)| last != file) {
            files.push((file.to_string(), Vec::new(), 0));
        }
        let (_, rows, count) = files.last_mut().unwrap();
        *count += 1;
        match rows.last_mut() {
            Some((last, lines)) if *last == text => lines.push(line),
            _ => rows.push((text, vec![line])),
        }
    }
    enrich::Section {
        heading,
        files: files
            .into_iter()
            .map(|(file, rows, count)| {
                let rows = rows.into_iter().map(|(text, lines)| format!("{text} @ {}", lines.join(", "))).collect();
                (file, rows, count)
            })
            .collect(),
    }
}
