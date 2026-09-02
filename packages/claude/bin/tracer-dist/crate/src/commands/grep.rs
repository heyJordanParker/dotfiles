//! `trace grep` — text search with rich per-match context.
//! Wraps `rg --json`; each match is enriched with per-file complexity,
//! nearest doc, and git activity, plus a repo-wide complexity_p95 for
//! read-depth calibration.
//!
//! ripgrep's output is consumed as it is written rather than through
//! `Command::output`, which buffers every byte of the search before the first
//! match is looked at. The document is serialized from borrowed structures
//! through `output::Sink`, so the result never exists as a `serde_json::Value`
//! tree on the way out.

use crate::commands::enrich::{self, Match};
use crate::output::Sink;
use crate::{cache, repo_context};
use anyhow::Result;
use serde_json::{json, Value};
use std::io::BufRead;
use std::path::Path;
use std::process::{Command, Stdio};

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

/// One `rg --json` event line into a `Match`, or None when the event is not
/// a match.
fn parse_event(line: &str) -> Option<Match> {
    let event: Value = serde_json::from_str(line).ok()?;
    if event.get("type").and_then(|x| x.as_str()) != Some("match") {
        return None;
    }
    let data = &event["data"];
    let text = data["lines"]["text"].as_str().unwrap_or("");
    let match_start = data["submatches"][0]["start"].as_u64().unwrap_or(0) as usize;
    Some(Match {
        file: data["path"]["text"].as_str().unwrap_or("").to_string(),
        line: data["line_number"].as_i64().unwrap_or(0),
        snippet: window_snippet(text.trim_end_matches('\n'), match_start),
    })
}

/// Run `ripgrep` for a text pattern and collect the matches, reading its
/// stdout as it is produced.
fn ripgrep(pattern: &str, path: &str, lang: Option<&str>) -> Vec<Match> {
    let mut cmd = Command::new("rg");
    cmd.arg("--json");
    if let Some(l) = lang {
        cmd.args(["--type", l]);
    }
    // stderr is discarded, as the previous `Command::output` call discarded
    // it: ripgrep's own warnings (unreadable paths, invalid UTF-8) are not
    // part of this command's result, and inheriting them writes them into
    // the caller's terminal alongside the answer.
    cmd.arg(pattern)
        .arg(path)
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let stdout = match child.stdout.take() {
        Some(s) => s,
        None => return vec![],
    };
    let matches: Vec<Match> = std::io::BufReader::new(stdout)
        .lines()
        .map_while(Result::ok)
        .filter_map(|l| parse_event(&l))
        .collect();
    let _ = child.wait();
    matches
}

/// Search a commit rather than the working tree. ripgrep reads files on
/// disk, so the tool for a past state is `git grep`, which reads the tree
/// object directly — no checkout, no temp files.
fn git_grep(pattern: &str, at: &str, path: &str, exts: &[&str]) -> Vec<Match> {
    let mut cmd = Command::new("git");
    cmd.args(["grep", "-n", "--no-color", "-e", pattern, at, "--"]);
    if exts.is_empty() {
        cmd.arg(path);
    } else {
        for ext in exts {
            cmd.arg(format!("{}/*.{ext}", path.trim_end_matches('/')));
        }
    }
    let out = match cmd.stderr(Stdio::null()).output() {
        Ok(o) => o,
        Err(_) => return vec![],
    };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| {
            // `<ref>:<path>:<line>:<text>`
            let rest = line.strip_prefix(at)?.strip_prefix(':')?;
            let (file, rest) = rest.split_once(':')?;
            let (number, text) = rest.split_once(':')?;
            Some(Match {
                file: file.to_string(),
                line: number.parse().ok()?,
                snippet: window_snippet(text, text.find(pattern).unwrap_or(0)),
            })
        })
        .collect()
}

pub fn run(
    pattern: &str,
    lang: Option<&str>,
    path: &str,
    at: Option<&str>,
    as_json: bool,
    sink: &Sink,
) -> Result<()> {
    // A language name is resolved before the search runs: an unknown one
    // must fail by name, never as an empty result the agent reads as
    // "this does not exist".
    let row = match lang {
        None => None,
        Some(l) => match crate::lang::resolve(l) {
            Some(row) => Some(row),
            None => {
                eprintln!("Error: unknown language {l:?}. Accepted: {}", crate::lang::accepted());
                std::process::exit(2);
            }
        },
    };
    let matches = match at {
        Some(r) => git_grep(pattern, r, path, row.map(|l| l.exts).unwrap_or(&[])),
        None => ripgrep(pattern, path, row.map(|l| l.rg)),
    };
    let abs = cache::absolutize(Path::new(path));
    let search_root = cache::worktree_root_for(&abs).unwrap_or_else(|| cache::display_root(&abs));
    let (enriched, files) = enrich::enrich(&matches, &search_root);
    let repo_ctx = repo_context::repo_context(&abs);

    let nested = if enriched.is_empty() && abs.is_dir() {
        crate::repo_files::nested_repo_rels(&abs)
    } else {
        Vec::new()
    };

    let signpost = enrich::signpost(enrich::searched_name(pattern), &search_root);

    if !as_json {
        enrich::render_human(&enriched, &files, &repo_ctx, signpost.as_deref());
        for r in &nested {
            println!("nested repository (its own search scope): {r}");
        }
    }

    sink.emit(&enrich::SearchDocument {
        query: json!({"pattern": pattern, "lang": lang, "path": path, "at": at}),
        context: enrich::SearchContext {
            files: &files,
            repo: &repo_ctx,
            signpost,
        },
        results: &enriched,
        nested_repos: &nested,
    })
}
