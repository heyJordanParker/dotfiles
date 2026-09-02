//! `trace pattern` — code-shape search via ast-grep, with rich context.
//! Same per-match enrichment as `grep` (per-file complexity, nearest doc,
//! git activity).
//!
//! ast-grep runs under `--json=stream`, one JSON object per line, so its
//! output is consumed as it is written. The default `--json` prints one
//! pretty array, which had to be buffered whole and parsed into a `Value`
//! before a single match could be read.

use crate::commands::enrich::{self, Match};
use crate::output::Sink;
use crate::{cache, repo_context};
use anyhow::Result;
use serde_json::{json, Value};
use std::io::BufRead;
use std::path::Path;
use std::process::{Command, Stdio};

/// The literal words in a pattern: everything that is not a metavariable
/// (`$X`, `$$$ARGS`) or punctuation. `dispatch($$$A)` yields `dispatch`;
/// `$X->save($$$A)` yields `save`. They are what the match can be located by
/// and what a file must contain to be worth parsing.
fn literals(pattern: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut word = String::new();
    let mut chars = pattern.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '$' {
            // Skip the whole metavariable, including `$$$`.
            while chars.peek().is_some_and(|n| *n == '$') {
                chars.next();
            }
            while chars.peek().is_some_and(|n| n.is_alphanumeric() || *n == '_') {
                chars.next();
            }
            word.clear();
            continue;
        }
        if c.is_alphanumeric() || c == '_' {
            word.push(c);
            continue;
        }
        if word.len() > 1 {
            out.push(std::mem::take(&mut word));
        } else {
            word.clear();
        }
    }
    if word.len() > 1 {
        out.push(word);
    }
    out
}

/// One `sg --json=stream` line into a `Match`.
///
/// A match spanning several lines is reported at the line holding the
/// pattern's own word, not at the line the expression starts on: a call
/// chained across five lines used to be reported five lines above the call,
/// which sends the reader to the wrong place. The snippet is that same line,
/// so a multi-line match does not carry the whole expression as its snippet.
fn parse_entry(line: &str, anchor: Option<&str>) -> Option<Match> {
    let entry: Value = serde_json::from_str(line).ok()?;
    let start = entry["range"]["start"]["line"].as_i64().unwrap_or(0);
    let text = entry["text"].as_str().unwrap_or("");
    let mut offset = 0;
    let mut snippet = text.to_string();
    if text.contains('\n') {
        let lines: Vec<&str> = text.lines().collect();
        let index = anchor
            .and_then(|a| lines.iter().position(|l| l.contains(a)))
            .unwrap_or(0);
        offset = index as i64;
        snippet = lines[index].trim_end().to_string();
    }
    Some(Match {
        file: entry["file"].as_str().unwrap_or("").to_string(),
        line: start + offset + 1,
        snippet,
    })
}

/// The files under `path` that contain every literal word in the pattern.
/// Parsing a file is orders of magnitude dearer than scanning it, and a
/// pattern's literal words must appear verbatim in any file that can match
/// it, so this narrows the parse set without changing the answer. `None`
/// means "no usable literal" — the whole path is parsed, as before.
fn candidate_files(literals: &[String], path: &str, rg_type: &str) -> Option<Vec<String>> {
    let first = literals.first()?;
    let mut cmd = Command::new("rg");
    cmd.args(["--files-with-matches", "--fixed-strings"]);
    if !rg_type.is_empty() {
        cmd.args(["--type", rg_type]);
    }
    cmd.arg(first).arg(path);
    let out = cmd.stderr(Stdio::null()).output().ok()?;
    let mut files: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.to_string())
        .collect();
    // Every remaining literal must appear in the file too.
    for word in literals.iter().skip(1) {
        if files.is_empty() {
            break;
        }
        let mut cmd = Command::new("rg");
        cmd.args(["--files-with-matches", "--fixed-strings", word]);
        cmd.args(&files);
        let out = cmd.stderr(Stdio::null()).output().ok()?;
        let keep: std::collections::HashSet<String> = String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(|l| l.to_string())
            .collect();
        files.retain(|f| keep.contains(f));
    }
    Some(files)
}

/// Run `ast-grep` for a structural pattern and collect the matches, reading
/// its stdout as it is produced.
fn ast_grep(pattern: &str, lang: &str, paths: &[String], anchor: Option<&str>) -> Vec<Match> {
    // stderr is discarded, as the previous `Command::output` call discarded
    // it: ast-grep's pattern warnings are its own diagnostics, not part of
    // this command's result, and inheriting them writes them into the
    // caller's terminal alongside the answer.
    let mut cmd = Command::new("sg");
    cmd.args(["run", "-p", pattern, "-l", lang, "--json=stream"]);
    cmd.args(paths);
    let mut child = match cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
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
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| parse_entry(&l, anchor))
        .collect();
    let _ = child.wait();
    matches
}

pub fn run(
    pattern: &str,
    lang: &str,
    path: &str,
    as_json: bool,
    sink: &Sink,
) -> Result<()> {
    let row = match crate::lang::resolve(lang) {
        Some(row) if !row.sg.is_empty() => row,
        Some(row) => {
            eprintln!("Error: {:?} has no code-shape grammar. Use `trace grep` for it.", row.name);
            std::process::exit(2);
        }
        None => {
            eprintln!("Error: unknown language {lang:?}. Accepted: {}", crate::lang::accepted());
            std::process::exit(2);
        }
    };
    let words = literals(pattern);
    let anchor = words.first().cloned();
    let paths = candidate_files(&words, path, row.rg).unwrap_or_else(|| vec![path.to_string()]);
    // No candidate file holds the pattern's words, so nothing can match and
    // there is nothing to parse.
    let matches = if paths.is_empty() {
        Vec::new()
    } else {
        ast_grep(pattern, row.sg, &paths, anchor.as_deref())
    };
    let abs = cache::absolutize(Path::new(path));
    let search_root = cache::worktree_root_for(&abs).unwrap_or_else(|| cache::display_root(&abs));
    let (enriched, files) = enrich::enrich(&matches, &search_root);
    let repo_ctx = repo_context::repo_context(&abs);

    let signpost = enrich::signpost(anchor.as_deref(), &search_root);

    if !as_json {
        enrich::render_human(&enriched, &files, &repo_ctx, signpost.as_deref());
    }

    sink.emit(&enrich::SearchDocument {
        query: json!({"pattern": pattern, "lang": lang, "path": path}),
        context: enrich::SearchContext {
            files: &files,
            repo: &repo_ctx,
            signpost,
        },
        results: &enriched,
        nested_repos: &[],
    })
}
