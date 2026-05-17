//! `trace grep` — text search with rich per-match context.
//! Wraps `rg --json`; each match is enriched with per-file complexity,
//! nearest doc, and git activity, plus a repo-wide complexity_p95 for
//! read-depth calibration.

use crate::commands::enrich::{self, Match};
use crate::{cache, repo_context};
use anyhow::Result;
use serde_json::{json, Value};
use std::path::Path;
use std::process::Command;

/// Run `ripgrep` for a text pattern and collect the matches.
fn ripgrep(pattern: &str, path: &str, lang: Option<&str>) -> Vec<Match> {
    let mut cmd = Command::new("rg");
    cmd.arg("--json");
    if let Some(l) = lang {
        cmd.args(["--type", l]);
    }
    cmd.arg(pattern).arg(path);
    let out = match cmd.output() {
        Ok(o) => o,
        Err(_) => return vec![],
    };
    let mut matches = Vec::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let event: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if event.get("type").and_then(|x| x.as_str()) != Some("match") {
            continue;
        }
        let data = &event["data"];
        let text = data["lines"]["text"].as_str().unwrap_or("");
        matches.push(Match {
            file: data["path"]["text"].as_str().unwrap_or("").to_string(),
            line: data["line_number"].as_i64().unwrap_or(0),
            snippet: text.trim_end_matches('\n').to_string(),
        });
    }
    matches
}

pub fn run(pattern: &str, lang: Option<&str>, path: &str, as_json: bool) -> Result<Value> {
    let matches = ripgrep(pattern, path, lang);
    let search_root = cache::repo_root_for(&cache::absolutize(Path::new(path)));
    let (enriched, files_matched) = enrich::enrich(&matches, &search_root);
    let repo_ctx = repo_context::repo_context(&cache::absolutize(Path::new(path)));

    let out = json!({
        "query": pattern,
        "lang_filter": lang,
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
