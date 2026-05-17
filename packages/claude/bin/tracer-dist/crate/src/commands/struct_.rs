//! `trace struct` — AST-structural search via ast-grep, with rich context.
//! Same per-match enrichment as `grep` (per-file complexity, nearest doc,
//! git activity). The module is named `struct_` to avoid colliding with
//! Rust's `struct` keyword.

use crate::commands::enrich::{self, Match};
use crate::{cache, repo_context};
use anyhow::Result;
use serde_json::{json, Value};
use std::path::Path;
use std::process::Command;

/// Run `ast-grep` for a structural pattern and collect the matches.
fn ast_grep(pattern: &str, lang: &str, path: &str) -> Vec<Match> {
    let out = match Command::new("sg")
        .args(["run", "-p", pattern, "-l", lang, "--json", path])
        .output()
    {
        Ok(o) => o,
        Err(_) => return vec![],
    };
    let stdout = String::from_utf8_lossy(&out.stdout);
    if stdout.trim().is_empty() {
        return vec![];
    }
    let data: Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(_) => return vec![],
    };
    let arr = match data.as_array() {
        Some(a) => a,
        None => return vec![],
    };
    arr.iter()
        .map(|entry| Match {
            file: entry["file"].as_str().unwrap_or("").to_string(),
            line: entry["range"]["start"]["line"].as_i64().unwrap_or(0) + 1,
            snippet: entry["text"].as_str().unwrap_or("").to_string(),
        })
        .collect()
}

pub fn run(pattern: &str, lang: &str, path: &str, as_json: bool) -> Result<Value> {
    let matches = ast_grep(pattern, lang, path);
    let search_root = cache::repo_root_for(&cache::absolutize(Path::new(path)));
    let (enriched, files_matched) = enrich::enrich(&matches, &search_root);
    let repo_ctx = repo_context::repo_context(&cache::absolutize(Path::new(path)));

    let out = json!({
        "pattern": pattern,
        "lang": lang,
        "matches": enriched,
        "match_count": enriched.len(),
        "files_matched": files_matched,
        "repo_context": repo_ctx,
    });

    if !as_json {
        enrich::render_human(&enriched, files_matched, &repo_ctx);
    }
    Ok(out)
}
