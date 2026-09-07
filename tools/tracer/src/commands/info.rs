//! `trace info` — complexity structure + architectural overview.
//! The per-function table is AST-derived; `nloc` is the line span.
//! Emits a function complexity profile plus architecture context.

use crate::{cache, file_facts, passive_context, relations, repo_context};
use anyhow::Result;
use serde_json::{json, Value};
use std::path::Path;

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

    let ccn_total = facts
        .as_ref()
        .map(|facts| facts.cyclomatic_complexity_total)
        .unwrap_or(0);
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
    let callers = crate::digest::top_callers(&index, &relative, Some(&repo_root), 10);
    let deps = crate::digest::immediate_dependencies(&index, &relative, 15);

    let loc = facts
        .as_ref()
        .map(|f| f.loc)
        .unwrap_or_else(|| functions.first().map(|f| f.nloc).unwrap_or(0));

    json!({
        "file": path.to_string_lossy(),
        "language": facts.as_ref().and_then(|f| f.language.clone()),
        "loc": loc,
        "function_count": function_count,
        "cyclomatic_complexity_total": ccn_total,
        "cyclomatic_complexity_max": ccn_max,
        "rank": file_facts::rank(ccn_total),
        "functions": fn_json,
        "nearest_doc": crate::digest::nearest_doc(path),
        "passive_context": facts.as_ref().map(|f| passive_context::render(f, None)),
        "leading_comment": leading,
        "top_callers": callers,
        "dependencies": deps,
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
                "loc": f.loc,
                "cyclomatic_complexity_total": f.cyclomatic_complexity_total,
                "function_count": f.function_count,
                "rank": f.rank,
                "passive_context": passive_context::render_compact(f),
            }));
        };

    match tracked {
        Some(rels) => {
            for rels in rels.chunks(file_facts::RESOLVE_CHUNK) {
                let fulls: Vec<std::path::PathBuf> =
                    rels.iter().map(|rel| repo_root.join(rel)).collect();
                let facts_map = file_facts::get_batch(&fulls, &repo_root);
                for rel in rels {
                    let full = repo_root.join(rel);
                    let under = match full
                        .canonicalize()
                        .ok()
                        .and_then(|c| c.strip_prefix(&base).ok().map(|p| p.to_path_buf()))
                    {
                        Some(p) => p.to_string_lossy().to_string(),
                        None => continue,
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
        .map(|f| f["cyclomatic_complexity_total"].as_i64().unwrap_or(0))
        .sum();
    let loc_total: i64 = files.iter().map(|f| f["loc"].as_i64().unwrap_or(0)).sum();

    json!({
        "directory": path.to_string_lossy(),
        "file_count": total_count,
        "cyclomatic_complexity_total": ccn_total,
        "loc_total": loc_total,
        "files": files,
        "nearest_doc": crate::digest::nearest_doc(path),
    })
}

pub fn run(path: &Path, as_json: bool, brief: bool) -> Result<Value> {
    crate::pathval::require_exists(path, "PATH");
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
            emit_file_human(&info, !brief);
        } else {
            emit_dir_human(&info);
        }
        let ctx = &info["repo_context"];
        println!();
        println!(
            "repo_context: complexity_p95={} median={} files={}",
            ctx["complexity_p95"].as_i64().unwrap_or(0),
            ctx["median_file_ccn"].as_i64().unwrap_or(0),
            ctx["total_files"].as_i64().unwrap_or(0),
        );
    }
    Ok(enveloped(info, p.is_file()))
}

/// Re-slot the internal flat value into the one document shape. The human
/// renderers read the flat value, so this runs after them rather than
/// forcing every renderer to walk one level deeper.
fn enveloped(info: Value, is_file: bool) -> Value {
    if is_file {
        let file = info["file"].as_str().unwrap_or_default().to_string();
        return crate::output::document(
            json!({"file": info["file"]}),
            json!({
                "files": {
                    file: {
                        "language": info["language"],
                        "nearest_doc": info["nearest_doc"],
                        "shoulder": info["passive_context"],
                        "leading_comment": info["leading_comment"],
                        "top_callers": info["top_callers"],
                        "dependencies": info["dependencies"],
                    }
                },
                "repo": info["repo_context"],
            }),
            json!(info["functions"]),
            json!({
                "functions": info["function_count"],
                "loc": info["loc"],
                "ccn_total": info["cyclomatic_complexity_total"],
                "ccn_max_function": info["cyclomatic_complexity_max"],
                "rank": info["rank"],
            }),
        );
    }
    // Each file's shoulder is enrichment, so it leaves the row and sits in
    // `context` under the same relative path the row names.
    let mut shoulders = serde_json::Map::new();
    let mut rows: Vec<Value> = Vec::new();
    for row in info["files"].as_array().into_iter().flatten() {
        let mut row = row.clone();
        let shoulder = row
            .as_object_mut()
            .and_then(|o| o.remove("passive_context"))
            .unwrap_or(Value::Null);
        shoulders.insert(
            row["file"].as_str().unwrap_or_default().to_string(),
            json!({"shoulder": shoulder}),
        );
        rows.push(row);
    }
    crate::output::document(
        json!({"directory": info["directory"]}),
        json!({
            "nearest_doc": info["nearest_doc"],
            "repo": info["repo_context"],
            "files": Value::Object(shoulders),
        }),
        json!(rows),
        json!({
            "files": info["file_count"],
            "ccn_total": info["cyclomatic_complexity_total"],
            "loc": info["loc_total"],
        }),
    )
}

fn emit_file_human(info: &Value, full: bool) {
    println!("File: {}", info["file"].as_str().unwrap_or(""));
    if let Some(pc) = info["passive_context"].as_str() {
        println!("{pc}");
    }
    println!("Language: {}", info["language"].as_str().unwrap_or("None"));
    println!(
        "LOC: {}  Functions: {}  CCN total: {}  CCN max: {}  Rank: {}",
        info["loc"].as_i64().unwrap_or(0),
        info["function_count"].as_i64().unwrap_or(0),
        info["cyclomatic_complexity_total"].as_i64().unwrap_or(0),
        info["cyclomatic_complexity_max"].as_i64().unwrap_or(0),
        info["rank"].as_str().unwrap_or(""),
    );
    println!(
        "Nearest doc: {}",
        info["nearest_doc"].as_str().unwrap_or("(none)")
    );
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
                let summary = c["summary"]
                    .as_str()
                    .map(|s| format!("  {s}"))
                    .unwrap_or_default();
                println!("  {label} ({kind}){where_}{summary}");
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
    for f in shown {
        println!(
            "  {:>3}  {:>4} loc  {}  L{}-{}",
            f["cyclomatic_complexity"].as_i64().unwrap_or(0),
            f["nloc"].as_i64().unwrap_or(0),
            f["name"].as_str().unwrap_or(""),
            f["start_line"].as_i64().unwrap_or(0),
            f["end_line"].as_i64().unwrap_or(0),
        );
    }
    if !full && total > 3 {
        println!("  … {} more (use --full to see all)", total - 3);
    }
}

fn emit_dir_human(info: &Value) {
    println!("Directory: {}", info["directory"].as_str().unwrap_or(""));
    println!(
        "Files: {}  LOC: {}  CCN total: {}",
        info["file_count"].as_i64().unwrap_or(0),
        info["loc_total"].as_i64().unwrap_or(0),
        info["cyclomatic_complexity_total"].as_i64().unwrap_or(0),
    );
    println!(
        "Nearest doc: {}",
        info["nearest_doc"].as_str().unwrap_or("(none)")
    );
    println!();
    println!("Files (top 20 by complexity, with file digest):");
    let mut files: Vec<&Value> = info["files"]
        .as_array()
        .map(|a| a.iter().collect())
        .unwrap_or_default();
    files.sort_by(|a, b| {
        b["cyclomatic_complexity_total"]
            .as_i64()
            .cmp(&a["cyclomatic_complexity_total"].as_i64())
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
        println!(
            "  {:>5}  {:>5} loc  {:>3} fn  [{:<8}]  {}  ({})",
            f["cyclomatic_complexity_total"].as_i64().unwrap_or(0),
            f["loc"].as_i64().unwrap_or(0),
            f["function_count"].as_i64().unwrap_or(0),
            f["rank"].as_str().unwrap_or(""),
            f["file"].as_str().unwrap_or(""),
            f["passive_context"].as_str().unwrap_or(""),
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
