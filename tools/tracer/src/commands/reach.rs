//! `trace usages` and `trace dependencies` — one walk of the import
//! inversion, read in either direction.
//!
//! Only the importer direction is stored; the forward direction is that map
//! inverted in memory, which is why both commands answer from one structure
//! and, here, from one body of code. The direction decides four things and
//! nothing else: which way `relations::reachable` walks, what the results key
//! is called, whether a reached row is named by its module or by the symbol
//! the edge named, and the human wording.

use crate::commands::enrich;
use crate::{cache, file_facts, relations, surface};
use anyhow::Result;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Which way to read the import inversion, and every word that changes with
/// it. One value per command, so a new direction cannot half-exist.
#[derive(Clone, Copy)]
pub enum Direction {
    /// `usages`: who reaches this.
    Dependents,
    /// `dependencies`: what this reaches.
    Dependencies,
}

impl Direction {
    fn reach(self) -> relations::Reach {
        match self {
            Direction::Dependents => relations::Reach::Importers,
            Direction::Dependencies => relations::Reach::Imports,
        }
    }

    /// The key the rows sit under, and the noun the counts use.
    fn rows_key(self) -> &'static str {
        match self {
            Direction::Dependents => "dependents",
            Direction::Dependencies => "dependencies",
        }
    }

    fn symbol_heading(self) -> &'static str {
        match self {
            Direction::Dependents => "depended on by",
            Direction::Dependencies => "depends on",
        }
    }

    fn path_heading(self) -> &'static str {
        match self {
            Direction::Dependents => "most-depended-on",
            Direction::Dependencies => "highest-coupling",
        }
    }

    /// The count keys the path-mode rows carry.
    fn direct_key(self) -> &'static str {
        match self {
            Direction::Dependents => "direct_dependents",
            Direction::Dependencies => "direct_dependencies",
        }
    }

    fn transitive_key(self) -> &'static str {
        match self {
            Direction::Dependents => "transitive_dependents",
            Direction::Dependencies => "transitive_dependencies",
        }
    }
}

pub fn run(
    direction: Direction,
    symbol: Option<&str>,
    path: Option<&Path>,
    depth: i64,
    limit: i64,
    as_json: bool,
) -> Result<Value> {
    if let Some(p) = path {
        return path_mode(direction, p, depth, limit, as_json);
    }
    let symbol = match symbol {
        Some(s) if !s.is_empty() => s,
        _ => {
            eprintln!("Error: pass a SYMBOL or --path <path>.");
            std::process::exit(2);
        }
    };
    symbol_mode(direction, symbol, depth, as_json)
}

/// One reached file as a row.
///
/// The edge's symbol lives in the TARGET file. Read forward that target is the
/// dependency, so `from util import helper` names `helper`; read backward the
/// row is about the importer, which the symbol says nothing about, so it is
/// named by its module. The asymmetry is the edge's, not the renderer's.
type Rows<'a> = HashMap<&'a str, Vec<surface::Row>>;

fn loaded(files: &[String], repo_root: &Path) -> HashMap<String, file_facts::FileFacts> {
    let paths: Vec<PathBuf> = files.iter().map(|file| repo_root.join(file)).collect();
    file_facts::get_batch(&paths, repo_root)
}

fn rows_of(facts: &HashMap<String, file_facts::FileFacts>, named: bool) -> Rows<'_> {
    if !named {
        return Rows::new();
    }
    facts
        .iter()
        .map(|(file, facts)| (file.as_str(), surface::rows(facts, None)))
        .collect()
}

fn declaration(rows: &Rows, file: &str, symbol: Option<&String>) -> Option<surface::Row> {
    let symbol = symbol?;
    rows.get(file)?.iter().find(|row| row.name == *symbol).cloned()
}

fn reached_row(
    direction: Direction,
    reached: &relations::Reached,
    index: &relations::Relations,
    declared: Option<&surface::Row>,
) -> Value {
    let language = index.language(&reached.file);
    let named = match direction {
        Direction::Dependencies => reached.symbol.as_ref(),
        Direction::Dependents => None,
    };
    let (node_id, label, kind) = match named {
        Some(symbol) => (
            relations::symbol_id(&reached.file, symbol),
            symbol.clone(),
            "symbol",
        ),
        None => (
            relations::module_id(&reached.file, language),
            relations::file_to_module(&reached.file, language),
            "module",
        ),
    };
    json!({
        "node_id": node_id,
        "label": label,
        "kind": kind,
        "source_file": reached.file,
        "depth": reached.depth,
        "declaration": declared,
    })
}

fn symbol_mode(direction: Direction, symbol: &str, depth: i64, as_json: bool) -> Result<Value> {
    let here = Path::new(".");
    let repo_root = cache::worktree_root_for(here).unwrap_or_else(|| cache::display_root(here));
    let declarations = relations::declarations(symbol, &repo_root);
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
    let index = relations::get(&repo_root);

    // (node_id, label, kind, the file to walk from, source line)
    let mut subjects: Vec<(String, String, String, String, i64)> = Vec::new();
    for declaration in &declarations {
        subjects.push((
            relations::symbol_id(&declaration.file, &declaration.declaration.name),
            declaration.declaration.name.clone(),
            declaration.declaration.kind.clone(),
            declaration.file.clone(),
            declaration.declaration.line,
        ));
    }
    for file in &modules {
        let language = index.language(file);
        subjects.push((
            relations::module_id(file, language),
            relations::file_to_module(file, language),
            "module".to_string(),
            file.clone(),
            1,
        ));
    }

    let chains: Vec<Vec<relations::Reached>> = subjects
        .iter()
        .map(|(_, _, _, file, _)| relations::reachable(file, depth, direction.reach(), &repo_root))
        .collect();

    // Both directions carry the same file state, so they answer with
    // identical context.
    let mut reached_files: Vec<String> = chains
        .iter()
        .flat_map(|chain| chain.iter().map(|r| r.file.clone()))
        .collect();
    reached_files.sort();
    reached_files.dedup();
    let facts = loaded(&reached_files, &repo_root);
    let file_facts_by_path = enrich::facts_of(&facts, &repo_root);
    let file_rows = rows_of(&facts, matches!(direction, Direction::Dependencies));
    let declared: Vec<Vec<Option<surface::Row>>> = chains
        .iter()
        .map(|chain| {
            chain
                .iter()
                .map(|r| match direction {
                    Direction::Dependencies => declaration(&file_rows, &r.file, r.symbol.as_ref()),
                    Direction::Dependents => None,
                })
                .collect()
        })
        .collect();

    let rows_key = direction.rows_key();
    let mut symbols: Vec<Value> = Vec::with_capacity(subjects.len());
    for ((node_id, label, kind, source_file, source_line), (chain, declared)) in
        subjects.iter().zip(chains.iter().zip(&declared))
    {
        let rows: Vec<Value> = chain
            .iter()
            .zip(declared)
            .map(|(r, declared)| reached_row(direction, r, &index, declared.as_ref()))
            .collect();
        symbols.push(json!({
            "node_id": node_id,
            "symbol": label,
            "kind": kind,
            "source_file": source_file,
            "source_line": source_line,
            rows_key: rows,
        }));
    }

    if !as_json {
        let mut sections: Vec<enrich::Section> = Vec::with_capacity(subjects.len());
        for ((_, label, kind, source_file, source_line), ((chain, symbol), declared)) in
            subjects.iter().zip(chains.iter().zip(&symbols).zip(&declared))
        {
            let mut heading = format!(
                "\n{label} [{kind}] @ {source_file}:{source_line}\n  {} (depth ≤ {depth}):",
                direction.symbol_heading()
            );
            if chain.is_empty() {
                heading.push_str(&format!("\n    (no {rows_key} found)"));
            }
            let mut files: Vec<(String, Vec<String>, usize)> = Vec::new();
            for ((r, row), declared) in chain.iter().zip(symbol[rows_key].as_array().into_iter().flatten()).zip(declared) {
                let rendered = declared
                    .as_ref()
                    .map(|declaration| surface::inline(declaration, &r.file))
                    .unwrap_or_else(|| format!("{} [{}]", row["label"].as_str().unwrap_or(""), row["kind"].as_str().unwrap_or("")));
                if files.last().is_none_or(|(last, _, _)| *last != r.file) {
                    files.push((r.file.clone(), Vec::new(), 0));
                }
                let (_, rows, count) = files.last_mut().unwrap();
                rows.push(format!("      [d={}] {rendered}", r.depth));
                *count += 1;
            }
            sections.push(enrich::Section { heading, files });
        }
        let (one, many) = match direction {
            Direction::Dependents => ("dependent", "dependents"),
            Direction::Dependencies => ("dependency", "dependencies"),
        };
        enrich::render_sections(&sections, &file_facts_by_path, one, many);
    }
    let total: i64 = symbols
        .iter()
        .map(|s| s[rows_key].as_array().map(|a| a.len()).unwrap_or(0) as i64)
        .sum();
    Ok(crate::output::document(
        json!({"symbol": symbol, "depth": depth, "mode": "symbol"}),
        json!({"files": enrich::facts_context(&file_facts_by_path)}),
        json!(symbols),
        json!({"symbols": symbols.len(), rows_key: total}),
    ))
}

fn path_mode(
    direction: Direction,
    path: &Path,
    depth: i64,
    limit: i64,
    as_json: bool,
) -> Result<Value> {
    // The index is always read against the real git repo root: a single-file
    // `--path` argument used as the root would index one file and amputate
    // every cross-file edge.
    let repo_root = cache::worktree_root_for(path)
        .or_else(|| cache::worktree_root_for(Path::new(".")))
        .unwrap_or_else(|| cache::display_root(path));
    let scope = repo_root
        .canonicalize()
        .ok()
        .zip(path.canonicalize().ok())
        .and_then(|(root, path)| {
            path.strip_prefix(root)
                .ok()
                .map(|path| path.to_string_lossy().replace('\\', "/"))
        });
    let ranked = relations::ranked_by_reach(
        depth,
        limit.max(0) as usize,
        direction.reach(),
        scope.as_deref(),
        &repo_root,
    );
    let index = relations::get(&repo_root);

    let ranked_files: Vec<String> = ranked.iter().map(|r| r.file.clone()).collect();
    let facts = loaded(&ranked_files, &repo_root);
    let file_facts_by_path = enrich::facts_of(&facts, &repo_root);
    let file_rows = rows_of(&facts, matches!(direction, Direction::Dependents));
    let declared: Vec<Option<surface::Row>> = ranked
        .iter()
        .map(|r| match direction {
            Direction::Dependents => declaration(&file_rows, &r.file, r.symbol.as_ref()),
            Direction::Dependencies => None,
        })
        .collect();

    let (direct_key, transitive_key) = (direction.direct_key(), direction.transitive_key());
    // Ranked by dependents the file is every counted edge's TARGET, so the
    // symbol its importers named is what is actually depended on. Ranked by
    // dependencies it is the SOURCE — the importer — which is named by its
    // own module and never by what it imported.
    let rows: Vec<Value> = ranked
        .iter()
        .zip(&declared)
        .enumerate()
        .map(|(i, (r, declared))| {
            let language = index.language(&r.file);
            let named = match direction {
                Direction::Dependents => r.symbol.as_ref(),
                Direction::Dependencies => None,
            };
            let (node_id, label, kind) = match named {
                Some(symbol) => (
                    relations::symbol_id(&r.file, symbol),
                    symbol.clone(),
                    "symbol",
                ),
                None => (
                    relations::module_id(&r.file, language),
                    relations::file_to_module(&r.file, language),
                    "module",
                ),
            };
            json!({
                "rank": i + 1,
                "node_id": node_id,
                "label": label,
                "kind": kind,
                "source_file": r.file,
                "source_line": 1,
                direct_key: r.direct,
                transitive_key: r.transitive,
                "declaration": declared,
            })
        })
        .collect();

    let out = crate::output::document(
        json!({
            "path": path.to_string_lossy(),
            "limit": limit,
            "depth": depth,
            "mode": "path",
        }),
        json!({"files": enrich::facts_context(&file_facts_by_path)}),
        json!(rows),
        json!({"nodes": rows.len()}),
    );

    if !as_json {
        if rows.is_empty() {
            println!(
                "(no files with {} found in this scope)",
                direction.rows_key()
            );
        } else {
            println!(
                "Top {} {} nodes in {} (transitive depth ≤ {depth}):",
                rows.len(),
                direction.path_heading(),
                path.to_string_lossy(),
            );
            println!(
                "  {:<3} {:>6}  {:>10}  {:<10}  symbol @ source",
                "#", "direct", "transitive", "kind"
            );
            for (row, declared) in rows.iter().zip(&declared) {
                println!(
                    "  {:<3} {:>6}  {:>10}  {:<10}  {} @ {}:1",
                    row["rank"].as_i64().unwrap_or(0),
                    row[direct_key].as_i64().unwrap_or(0),
                    row[transitive_key].as_i64().unwrap_or(0),
                    row["kind"].as_str().unwrap_or(""),
                    row["label"].as_str().unwrap_or(""),
                    row["source_file"].as_str().unwrap_or(""),
                );
                if let Some(declaration) = declared {
                    println!("        {}", surface::inline(declaration, row["source_file"].as_str().unwrap_or("")));
                }
                if let Some(facts) =
                    row["source_file"].as_str().and_then(|f| file_facts_by_path.get(f))
                {
                    println!("        {}", facts.headline());
                }
            }
        }
    }
    Ok(out)
}
