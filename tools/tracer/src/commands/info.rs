//! `trace info` — complexity structure + architectural overview.
//! The per-function table is AST-derived; `nloc` is the line span.
//! Emits a function complexity profile plus architecture context.

use crate::summary::Facts;
use crate::{cache, file_facts, relations, repo_context, summary, surface};
use anyhow::Result;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn leading_comment(path: &Path) -> Option<String> {
    crate::digest::leading_comment(path, 25)
}

fn file_info(path: &Path) -> Value {
    let repo_root = cache::worktree_root_for(path).unwrap_or_else(|| cache::display_root(path));
    let facts = file_facts::get(path, &repo_root);

    let functions = facts
        .as_ref()
        .map(|facts| facts.functions.as_slice())
        .unwrap_or_default();
    let fn_json: Vec<Value> = functions
        .iter()
        .map(|f| {
            json!({
                "name": f.name,
                "cyclomatic_complexity": f.cyclomatic_complexity,
                "nloc": f.nloc,
                "start_line": f.start_line,
                "end_line": f.start_line + f.nloc - 1,
            })
        })
        .collect();

    let ccn_max = facts
        .as_ref()
        .map(|facts| facts.cyclomatic_complexity_max)
        .unwrap_or(0);
    let function_count = facts
        .as_ref()
        .map(|facts| facts.function_count)
        .unwrap_or(0);

    let leading = leading_comment(path);
    let relative = cache::relative_to_root(path, &repo_root);
    let index = relations::get(&repo_root);
    let graph = index.module_counts(&relative);
    let callers = crate::digest::top_callers(&index, &relative, Some(&repo_root), 10);
    let deps = crate::digest::immediate_dependencies(&index, &relative, 15);

    // The front matter every file command shows, plus what `info` adds: the
    // language, the function count, and the most complex function's score.
    // Outside any git repository there is no git to describe, but the
    // complexity is still `info`'s answer.
    let in_repository = cache::worktree_root_for(path).is_some();
    let mut front_matter = facts
        .as_ref()
        .map(|facts| {
            let mut map = Facts::of(facts, graph.as_ref()).to_map();
            if !in_repository {
                map.remove("git");
            }
            map
        })
        .unwrap_or_else(|| {
            let mut map = serde_json::Map::new();
            map.insert("file".into(), relative.clone().into());
            map
        });
    if let Some(language) = facts.as_ref().and_then(|f| f.language.clone()) {
        front_matter.insert("language".into(), language.into());
    }
    front_matter.insert("functions".into(), function_count.into());
    front_matter.insert("max_function_complexity".into(), ccn_max.into());
    if let Some(doc) = crate::digest::nearest_doc(path, &repo_root) {
        front_matter.insert("nearest_doc".into(), doc.into());
    }
    if let Some(directory) =
        path.parent().and_then(|directory| super::context::directory_facts(directory, true))
    {
        front_matter.insert("directory".into(), Value::Object(directory));
    }

    json!({
        "file": path.to_string_lossy(),
        "facts": front_matter,
        "function_count": function_count,
        "functions": fn_json,
        "leading_comment": leading,
        "top_callers": callers,
        "dependencies": deps,
        "surface": facts.as_ref().map(|facts| surface::rows(facts, None)).unwrap_or_default(),
    })
}

fn dir_info(path: &Path) -> Value {
    let base = cache::absolutize(path);
    let repo_root = cache::worktree_root_for(&base).unwrap_or_else(|| cache::display_root(&base));
    let tracked =
        crate::repo_files::tracked_files(&repo_root, (base != repo_root).then_some(base.as_path()));

    let mut files: Vec<Value> = vec![];
    let mut total_count = 0i64;

    let push_facts =
        |under_base: String, full: &Path, f: &file_facts::FileFacts, files: &mut Vec<Value>| {
            files.push(json!({
                "file": under_base,
                "abs_path": full.to_string_lossy(),
                "lines": f.loc,
                "cyclomatic_complexity": f.cyclomatic_complexity_total,
                "functions": f.function_count,
                "complexity_rank": f.rank,
                "git": Facts::of(f, None).git,
            }));
        };

    match tracked {
        Some(rels) => {
            // Both sides repo-relative, so a symlinked path (macOS `/tmp`)
            // still matches its tracked files.
            let base_relative = cache::relative_to_root(&base, &repo_root);
            for rels in rels.chunks(file_facts::RESOLVE_CHUNK) {
                let fulls: Vec<std::path::PathBuf> =
                    rels.iter().map(|rel| repo_root.join(rel)).collect();
                let facts_map = file_facts::get_batch(&fulls, &repo_root);
                for rel in rels {
                    let full = repo_root.join(rel);
                    let under = match Path::new(rel).strip_prefix(&base_relative) {
                        Ok(p) => p.to_string_lossy().to_string(),
                        Err(_) => continue,
                    };
                    total_count += 1;
                    if let Some(f) = facts_map.get(rel) {
                        push_facts(under, &full, f, &mut files);
                    }
                }
            }
        }
        None => {
            let mut walked = crate::repo_files::walk_files(&base);
            walked.sort();
            for walked in walked.chunks(file_facts::RESOLVE_CHUNK) {
                let normalized: Vec<std::path::PathBuf> = walked
                    .iter()
                    .map(|path| repo_root.join(cache::relative_to_root(path, &repo_root)))
                    .collect();
                let facts_map = file_facts::get_batch(&normalized, &repo_root);
                for full in walked {
                    total_count += 1;
                    let under = full
                        .strip_prefix(&base)
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|_| full.to_string_lossy().to_string());
                    let rel = cache::relative_to_root(full, &repo_root);
                    if let Some(f) = facts_map.get(&rel) {
                        push_facts(under, full, f, &mut files);
                    }
                }
            }
        }
    }

    let ccn_total: i64 = files
        .iter()
        .map(|f| f["cyclomatic_complexity"].as_i64().unwrap_or(0))
        .sum();
    let loc_total: i64 = files.iter().map(|f| f["lines"].as_i64().unwrap_or(0)).sum();

    json!({
        "directory": path.to_string_lossy(),
        "file_count": total_count,
        "cyclomatic_complexity_total": ccn_total,
        "loc_total": loc_total,
        "files": files,
        "nearest_doc": crate::digest::nearest_doc(path, &repo_root),
    })
}

/// One path's document, or — for several — each path's document in order,
/// each printed under its own `== path ==` heading the way `context` prints
/// several paths, sharing the budget evenly.
pub fn run(paths: &[PathBuf], as_json: bool, brief: bool) -> Result<Value> {
    if let [path] = paths {
        crate::pathval::require_exists(path, "PATH");
        return one(path, as_json, brief, crate::output::budget());
    }
    let share = crate::output::budget().map(|budget| budget / paths.len().max(1));
    let mut documents = Vec::with_capacity(paths.len());
    for path in paths {
        if !path.exists() {
            crate::pathval::report(path, "PATH", "does not exist");
            continue;
        }
        if !as_json {
            println!("== {} ==", path.display());
        }
        documents.push(one(path, as_json, brief, share)?);
    }
    Ok(crate::output::document(
        json!({"paths": paths}),
        json!({}),
        json!(documents),
        json!({"paths": documents.len()}),
    ))
}

fn one(path: &Path, as_json: bool, brief: bool, budget: Option<usize>) -> Result<Value> {
    let p = cache::absolutize(path);
    let (mut info, repo_ctx) = rayon::join(
        || {
            if p.is_file() {
                file_info(&p)
            } else {
                dir_info(&p)
            }
        },
        || repo_context::repo_context(&p),
    );
    info["repo_context"] = repo_ctx;

    if !as_json {
        if p.is_file() {
            emit_file_human(&info, !brief, budget);
        } else {
            emit_dir_human(&info);
        }
        let ctx = &info["repo_context"];
        let facts = json!({
            "files": ctx["total_files"].as_i64().unwrap_or(0),
            "median_file_complexity": ctx["median_file_ccn"].as_i64().unwrap_or(0),
            "complexity_p95": ctx["complexity_p95"].as_i64().unwrap_or(0),
        });
        println!();
        println!("repo_context: {}", crate::yamlfmt::flow(&facts, false));
    }
    Ok(enveloped(info, p.is_file()))
}

/// Re-slot the internal flat value into the one document shape. The human
/// renderers read the flat value, so this runs after them rather than
/// forcing every renderer to walk one level deeper.
fn enveloped(info: Value, is_file: bool) -> Value {
    if is_file {
        // The front matter's keys, then what else `info` found, under the
        // repo-relative path every other command keys a file by.
        let mut entry = info["facts"].as_object().cloned().unwrap_or_default();
        let file = entry["file"].as_str().unwrap_or_default().to_string();
        for key in ["surface", "leading_comment", "top_callers", "dependencies"] {
            entry.insert(key.into(), info[key].clone());
        }
        return crate::output::document(
            json!({"file": info["file"]}),
            json!({
                "files": {file: entry},
                "repo": info["repo_context"],
            }),
            json!(info["functions"]),
            json!({"functions": info["function_count"]}),
        );
    }
    crate::output::document(
        json!({"directory": info["directory"]}),
        json!({
            "nearest_doc": info["nearest_doc"],
            "repo": info["repo_context"],
        }),
        info["files"].clone(),
        json!({
            "files": info["file_count"],
            "lines": info["loc_total"],
            "cyclomatic_complexity": info["cyclomatic_complexity_total"],
        }),
    )
}

fn emit_file_human(info: &Value, full: bool, budget: Option<usize>) {
    let front_matter = info["facts"].as_object().map(summary::front_matter).unwrap_or_default();
    print!("{front_matter}");
    let surface: Vec<surface::Row> =
        serde_json::from_value(info["surface"].clone()).unwrap_or_default();
    // The rows take at most half the budget; the digest and function table
    // below take the rest.
    let file = info["file"].as_str().unwrap_or("");
    let rows = surface::render_within(&surface, file, None, budget.map(|b| b / 2));
    print!("{rows}");
    // The table repeats functions the rows already name, so it is detail:
    // it keeps the most complex that fit what is left.
    let mut table_room = budget.map(|b| b.saturating_sub(front_matter.len() + rows.len() + 1_000));
    if let Some(lc) = info["leading_comment"].as_str() {
        println!();
        println!("Purpose (from leading comment):");
        for line in lc.lines() {
            println!("  {line}");
        }
    }
    if let Some(callers) = info["top_callers"].as_array() {
        if !callers.is_empty() {
            println!();
            println!("Top callers (modules depending on this file):");
            for c in callers {
                let label = c["label"].as_str().unwrap_or("");
                let kind = c["kind"].as_str().unwrap_or("");
                let where_ = match (c["source_line"].as_i64(), c["source_file"].as_str()) {
                    (Some(l), Some(f)) => format!(" — {f}:{l}"),
                    (None, Some(f)) => format!(" — {f}"),
                    _ => String::new(),
                };
                let purpose = c["purpose"]
                    .as_str()
                    .map(|s| format!("  {s}"))
                    .unwrap_or_default();
                println!("  {label} ({kind}){where_}{purpose}");
            }
        }
    }
    if let Some(deps) = info["dependencies"].as_array() {
        if !deps.is_empty() {
            println!();
            println!("Immediate dependencies (modules this file imports):");
            for d in deps {
                println!(
                    "  {}  [{}]",
                    d["module"].as_str().unwrap_or(""),
                    d["confidence"].as_str().unwrap_or("")
                );
            }
        }
    }
    println!();
    let mut funcs: Vec<&Value> = info["functions"]
        .as_array()
        .map(|a| a.iter().collect())
        .unwrap_or_default();
    funcs.sort_by(|a, b| {
        b["cyclomatic_complexity"]
            .as_i64()
            .cmp(&a["cyclomatic_complexity"].as_i64())
    });
    let total = funcs.len();
    let shown: Vec<&Value> = if full {
        funcs.clone()
    } else {
        funcs.iter().take(3).cloned().collect()
    };
    println!(
        "Functions ({} of {}):",
        if full { "all" } else { "top 3 by complexity" },
        total
    );
    let mut printed = 0;
    for f in &shown {
        let row = format!(
            "  {:>3}  {:>4} loc  {}  L{}-{}",
            f["cyclomatic_complexity"].as_i64().unwrap_or(0),
            f["nloc"].as_i64().unwrap_or(0),
            f["name"].as_str().unwrap_or(""),
            f["start_line"].as_i64().unwrap_or(0),
            f["end_line"].as_i64().unwrap_or(0),
        );
        if let Some(room) = table_room.as_mut() {
            if row.len() + 1 > *room {
                break;
            }
            *room -= row.len() + 1;
        }
        println!("{row}");
        printed += 1;
    }
    if printed < shown.len() {
        println!("{}", crate::output::shortened_line(shown.len() - printed, shown.len(), "functions"));
    }
    if !full && total > 3 {
        println!("  … {} more (run without --brief to see all)", total - 3);
    }
}

fn emit_dir_human(info: &Value) {
    let mut front_matter = serde_json::Map::new();
    front_matter.insert("directory".into(), info["directory"].clone());
    front_matter.insert("files".into(), info["file_count"].clone());
    front_matter.insert("lines".into(), info["loc_total"].clone());
    front_matter.insert("cyclomatic_complexity".into(), info["cyclomatic_complexity_total"].clone());
    if !info["nearest_doc"].is_null() {
        front_matter.insert("nearest_doc".into(), info["nearest_doc"].clone());
    }
    print!("{}", summary::front_matter(&front_matter));
    println!("Files (top 20 by complexity, with file digest):");
    let mut files: Vec<&Value> = info["files"]
        .as_array()
        .map(|a| a.iter().collect())
        .unwrap_or_default();
    files.sort_by(|a, b| {
        b["cyclomatic_complexity"]
            .as_i64()
            .cmp(&a["cyclomatic_complexity"].as_i64())
    });
    let selected: Vec<&Value> = files.into_iter().take(20).collect();
    let directory = Path::new(info["directory"].as_str().unwrap_or(""));
    let repo_root =
        cache::worktree_root_for(directory).unwrap_or_else(|| cache::display_root(directory));
    let paths: Vec<std::path::PathBuf> = selected
        .iter()
        .filter_map(|file| file["abs_path"].as_str())
        .map(Path::new)
        .map(|path| repo_root.join(cache::relative_to_root(path, &repo_root)))
        .collect();
    let facts = file_facts::get_batch(&paths, &repo_root);
    for f in selected {
        let mut headline = json!({
            "cyclomatic_complexity": f["cyclomatic_complexity"],
            "lines": f["lines"],
            "functions": f["functions"],
            "complexity_rank": f["complexity_rank"],
        });
        if f["git"]["status"] != "unmodified" {
            headline["git"] = f["git"]["status"].clone();
        }
        println!(
            "  {}  {}",
            f["file"].as_str().unwrap_or(""),
            crate::yamlfmt::flow(&headline, false)
        );
        // Per-file digest block: Purpose + Top fns (the 3 hottest
        // functions with their start lines, from the AST backend).
        if let Some(abs) = f["abs_path"].as_str() {
            let p = Path::new(abs);
            if let Some(purpose) = crate::digest::leading_comment(p, 25) {
                if let Some(first) = purpose.lines().next() {
                    let first = first.trim();
                    if !first.is_empty() {
                        println!("        Purpose: {first}");
                    }
                }
            }
            let relative = cache::relative_to_root(p, &repo_root);
            if let Some(mut fns) = facts.get(&relative).map(|facts| facts.functions.clone()) {
                fns.sort_by(|a, b| b.cyclomatic_complexity.cmp(&a.cyclomatic_complexity));
                let top: Vec<String> = fns
                    .iter()
                    .take(3)
                    .map(|fn_| format!("{}() L{}", fn_.name, fn_.start_line))
                    .collect();
                if !top.is_empty() {
                    println!("        Top fns: {}", top.join("  "));
                }
            }
        }
    }
}
