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
use crate::commands::signatures::{self, Signature};
use crate::{cache, relations};
use anyhow::Result;
use rayon::prelude::*;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Resolved callers before ambiguous ones, so the rows an agent can trust
/// come first.
fn confidence_rank(confidence: &str) -> u8 {
    match confidence {
        relations::CONFIDENCE_EXTRACTED => 0,
        relations::CONFIDENCE_INFERRED => 1,
        _ => 2,
    }
}

/// Every caller file's signature list, parsed once per file and in parallel.
/// Each file is a tree-sitter parse, and `Collection` on laravel-framework
/// reaches over a thousand caller rows, so parsing them one after another
/// was the whole cost of the command.
fn signature_lists(
    files: &[String],
    repo_root: &Path,
) -> HashMap<String, Vec<Signature>> {
    let mut wanted: Vec<&String> = files.iter().filter(|f| !f.is_empty()).collect();
    wanted.sort();
    wanted.dedup();
    wanted
        .par_iter()
        .map(|file| {
            let abs: PathBuf = repo_root.join(file);
            let sigs = match std::fs::read(&abs) {
                Ok(source) => signatures::extract(&source, &abs),
                Err(_) => Vec::new(),
            };
            ((*file).clone(), sigs)
        })
        .collect()
}

/// The signature `extra` JSON for the calling symbol at `source_file:line`
/// named `name`, or `Value::Null` when the file's language has no signature
/// extractor or no signature matches.
fn signature_for(
    source_file: &str,
    source_line: i64,
    label: &str,
    cache: &HashMap<String, Vec<Signature>>,
) -> Value {
    cache
        .get(source_file)
        .into_iter()
        .flatten()
        .find(|s| s.line == source_line && s.name == label)
        .map(|s| s.extra.clone())
        .unwrap_or(Value::Null)
}

/// One caller row. The signature is the CALLING symbol's surface, so it is
/// looked up at that symbol's declaration coordinates, not the use site: a
/// function called on line 9 may be declared on line 3.
#[allow(clippy::too_many_arguments)]
fn caller_row(
    node_id: String,
    label: &str,
    kind: &str,
    declared_in: &str,
    declared_line: i64,
    location_file: &str,
    location_line: i64,
    relation: &str,
    confidence: &str,
    sig_cache: &HashMap<String, Vec<Signature>>,
) -> Value {
    let signature = signature_for(declared_in, declared_line, label, sig_cache);
    json!({
        "node_id": node_id,
        "label": label,
        "kind": kind,
        "source_file": location_file,
        "source_line": location_line,
        "relation": relation,
        "confidence": confidence,
        "signature": signature,
    })
}

/// The rows for one module: the files that import it.
fn importer_rows(
    file: &str,
    repo_root: &Path,
    languages: &HashMap<String, Option<String>>,
    sig_cache: &HashMap<String, Vec<Signature>>,
) -> Vec<Value> {
    let index = relations::get(repo_root);
    index
        .importers_of(file)
        .iter()
        .map(|importer| {
            let language = languages.get(&*importer.file).cloned().flatten();
            let module = relations::file_to_module(&importer.file, language.as_deref());
            // A module has no calling-function source, so its row carries the
            // importing module's own coordinates and a null signature.
            caller_row(
                relations::module_id(&importer.file, language.as_deref()),
                &module,
                "module",
                "",
                0,
                &importer.file,
                1,
                "imports",
                &importer.confidence,
                sig_cache,
            )
        })
        .collect()
}

pub fn run(symbol: &str, limit: usize, as_json: bool) -> Result<Value> {
    let here = Path::new(".");
    let repo_root = cache::worktree_root_for(here).unwrap_or_else(|| cache::display_root(here));
    let declarations = relations::declarations(symbol, &repo_root);
    // A name the symbol index does not declare may still name a module, the
    // way `trace callers app` asks about `src/app.py`.
    let modules = if declarations.is_empty() {
        relations::modules_named(symbol, &repo_root)
    } else {
        Vec::new()
    };

    if declarations.is_empty() && modules.is_empty() {
        eprintln!("Symbol '{symbol}' not declared anywhere in this repository.");
        std::process::exit(2);
    }

    let mut sites = if declarations.is_empty() {
        Vec::new()
    } else {
        relations::use_sites(symbol, &repo_root)
    };

    // Rank before the cut, so the rows that survive it are the ones worth
    // keeping: resolved before ambiguous, then by file and line. A member
    // call that matches many same-named methods fans out to one row per
    // candidate — `get` in laravel-framework is declared 112 times, and its
    // 2,055 call sites produced 230,000 rows and 276 MB of stdout. The cut is
    // the same contract `find` carries: `counts.total` and `counts.truncated`
    // say what was held back, and `--limit` returns it.
    sites.sort_by(|a, b| {
        confidence_rank(a.confidence)
            .cmp(&confidence_rank(b.confidence))
            .then_with(|| a.file.cmp(&b.file))
            .then_with(|| a.line.cmp(&b.line))
    });
    let total_sites = sites.len();
    let truncated = total_sites > limit;
    sites.truncate(limit);

    let languages = relations::languages(&repo_root);

    // The use-site file is the file the agent would open to read the call, so
    // its lifecycle state is what each caller row carries.
    let mut row_files: Vec<String> = sites.iter().map(|s| s.file.clone()).collect();
    for file in &modules {
        row_files.extend(
            relations::get(&repo_root)
                .importers_of(file)
                .iter()
                .map(|i| i.file.to_string()),
        );
    }
    for declaration in &declarations {
        row_files.extend(
            relations::get(&repo_root)
                .importers_of(&declaration.file)
                .iter()
                .map(|i| i.file.to_string()),
        );
    }
    let shoulders = enrich::file_shoulders(&row_files, &repo_root);
    // Every row's signature comes from its calling symbol's own file, and the
    // row files are exactly that set.
    let sig_cache = signature_lists(&row_files, &repo_root);

    let mut symbols: Vec<Value> = Vec::new();
    let mut headings: Vec<(String, String, String, i64)> = Vec::new();

    for declaration in &declarations {
        let mut callers: Vec<Value> = sites
            .iter()
            .filter(|s| {
                s.target_file == declaration.file
                    && s.target.name == declaration.declaration.name
            })
            .map(|s| {
                // A use site inside a declared function resolves its row to
                // that calling symbol; one at module top level keeps the
                // file's own module identity.
                let (node_id, label, kind, declared_in, declared_line) = match &s.caller {
                    Some(caller) => (
                        relations::symbol_id(&s.file, &caller.name),
                        caller.name.clone(),
                        caller.kind.clone(),
                        s.file.clone(),
                        caller.line,
                    ),
                    None => {
                        let language = languages.get(&s.file).cloned().flatten();
                        (
                            relations::module_id(&s.file, language.as_deref()),
                            relations::file_to_module(&s.file, language.as_deref()),
                            "module".to_string(),
                            String::new(),
                            0,
                        )
                    }
                };
                caller_row(
                    node_id,
                    &label,
                    &kind,
                    &declared_in,
                    declared_line,
                    &s.file,
                    s.line,
                    "references",
                    s.confidence,
                    &sig_cache,
                )
            })
            .collect();

        // Fallback: a symbol with zero use sites falls back to the importers
        // of its own file. A class used everywhere through `use App\Models\
        // User;` has no call site the reference walker can catch, and
        // returning zero callers for it would be strictly worse.
        if callers.is_empty() {
            callers = importer_rows(&declaration.file, &repo_root, &languages, &sig_cache);
        }

        push_symbol(
            &mut symbols,
            &mut headings,
            relations::symbol_id(&declaration.file, &declaration.declaration.name),
            &declaration.declaration.name,
            &declaration.declaration.kind,
            &declaration.file,
            declaration.declaration.line,
            callers,
        );
    }

    for file in &modules {
        let language = languages.get(file).cloned().flatten();
        let callers = importer_rows(file, &repo_root, &languages, &sig_cache);
        push_symbol(
            &mut symbols,
            &mut headings,
            relations::module_id(file, language.as_deref()),
            &relations::file_to_module(file, language.as_deref()),
            "module",
            file,
            1,
            callers,
        );
    }

    if !as_json {
        for ((label, kind, source_file, source_line), entry) in
            headings.iter().zip(symbols.iter())
        {
            println!("\n{label} [{kind}] @ {source_file}:{source_line}");
            let callers = entry["callers"].as_array().unwrap();
            if callers.is_empty() {
                println!("  (no callers found)");
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
                if let Some(s) = source_file.and_then(|f| shoulders.get(f)) {
                    println!("        {s}");
                }
            }
        }
    }
    if !as_json && truncated {
        println!(
            "\n... {} more (see all: --limit {total_sites})",
            total_sites - sites.len()
        );
    }
    let total: i64 = symbols
        .iter()
        .map(|s| s["caller_count"].as_i64().unwrap_or(0))
        .sum();
    Ok(crate::output::document(
        json!({"symbol": symbol, "limit": limit}),
        json!({"files": enrich::shoulder_context(&shoulders)}),
        json!(symbols),
        json!({
            "symbols": symbols.len(),
            "callers": total,
            "total": total_sites,
            "truncated": truncated,
        }),
    ))
}

/// Sort one symbol's caller rows and append the result entry.
#[allow(clippy::too_many_arguments)]
fn push_symbol(
    symbols: &mut Vec<Value>,
    headings: &mut Vec<(String, String, String, i64)>,
    node_id: String,
    label: &str,
    kind: &str,
    source_file: &str,
    source_line: i64,
    mut callers: Vec<Value>,
) {
    // Confidence-first ordering: resolved callers ahead of ambiguous ones;
    // ties broken by file then line so output is deterministic.
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

    let caller_count = callers.len() as i64;
    let ambiguous_count = callers
        .iter()
        .filter(|c| c["confidence"].as_str() == Some(relations::CONFIDENCE_AMBIGUOUS))
        .count() as i64;

    headings.push((
        label.to_string(),
        kind.to_string(),
        source_file.to_string(),
        source_line,
    ));
    symbols.push(json!({
        "node_id": node_id,
        "symbol": label,
        "kind": kind,
        "source_file": source_file,
        "source_line": source_line,
        "caller_count": caller_count,
        "resolved_count": caller_count - ambiguous_count,
        "ambiguous_count": ambiguous_count,
        "callers": callers,
    }));
}
