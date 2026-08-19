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

/// A snippet stays a snippet on minified or generated lines. The matched
/// line is the unit for ordinary source, but a 27KB single-line bundle is
/// not "context around the match" — so past `MAX_SNIPPET_CHARS` the snippet
/// becomes a character window positioned by the submatch byte offset
/// `rg --json` already reports, ellipsized on the cut side(s).
const MAX_SNIPPET_CHARS: usize = 240;
const WINDOW_BEFORE_CHARS: usize = 80;

fn window_snippet(line: &str, match_byte_start: usize) -> String {
    let total_chars = line.chars().count();
    if total_chars <= MAX_SNIPPET_CHARS {
        return line.to_string();
    }
    let prefix_chars = line
        .get(..match_byte_start.min(line.len()))
        .map(|p| p.chars().count())
        .unwrap_or(0);
    let begin = prefix_chars.saturating_sub(WINDOW_BEFORE_CHARS);
    let window: String = line.chars().skip(begin).take(MAX_SNIPPET_CHARS).collect();
    let mut out = String::new();
    if begin > 0 {
        out.push('\u{2026}');
    }
    out.push_str(&window);
    if begin + MAX_SNIPPET_CHARS < total_chars {
        out.push('\u{2026}');
    }
    out
}

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
        let match_start = data["submatches"][0]["start"].as_u64().unwrap_or(0) as usize;
        matches.push(Match {
            file: data["path"]["text"].as_str().unwrap_or("").to_string(),
            line: data["line_number"].as_i64().unwrap_or(0),
            snippet: window_snippet(text.trim_end_matches('\n'), match_start),
        });
    }
    matches
}

pub fn run(pattern: &str, lang: Option<&str>, path: &str, as_json: bool) -> Result<Value> {
    let matches = ripgrep(pattern, path, lang);
    let abs = cache::absolutize(Path::new(path));
    let search_root = cache::worktree_root_for(&abs).unwrap_or_else(|| cache::display_root(&abs));
    let (enriched, files_matched) = enrich::enrich(&matches, &search_root);
    let repo_ctx = repo_context::repo_context(&cache::absolutize(Path::new(path)));

    let mut out = json!({
        "query": pattern,
        "lang_filter": lang,
        "matches": enriched,
        "match_count": enriched.len(),
        "files_matched": files_matched,
        "repo_context": repo_ctx,
    });
    // An empty result over a base that contains nested checkouts is a scope
    // fact, not an absence fact — name them so the next call is scoped inside.
    let nested = if enriched.is_empty() && abs.is_dir() {
        crate::repo_files::nested_repo_rels(&abs)
    } else {
        Vec::new()
    };
    if !nested.is_empty() {
        out["nested_repos"] = json!(nested);
    }

    if !as_json {
        enrich::render_human(&enriched, files_matched, &repo_ctx);
        for r in &nested {
            println!("nested repository (its own search scope): {r}");
        }
    }
    Ok(out)
}
