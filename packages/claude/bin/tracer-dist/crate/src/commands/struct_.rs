//! `trace struct` — AST-structural search via ast-grep, with rich context.
//! Same per-match enrichment as `grep` (per-file complexity, nearest doc,
//! git activity). The module is named `struct_` to avoid colliding with
//! Rust's `struct` keyword.
//!
//! ast-grep runs under `--json=stream`, one JSON object per line, so its
//! output is consumed as it is written. The default `--json` prints one
//! pretty array, which had to be buffered whole and parsed into a `Value`
//! before a single match could be read.

use crate::commands::enrich::{self, EnrichedMatch, Match};
use crate::output::Sink;
use crate::{cache, repo_context};
use anyhow::Result;
use serde::ser::{SerializeMap, Serializer};
use serde::Serialize;
use serde_json::Value;
use std::io::BufRead;
use std::path::Path;
use std::process::{Command, Stdio};

/// One `sg --json=stream` line into a `Match`.
fn parse_entry(line: &str) -> Option<Match> {
    let entry: Value = serde_json::from_str(line).ok()?;
    Some(Match {
        file: entry["file"].as_str().unwrap_or("").to_string(),
        line: entry["range"]["start"]["line"].as_i64().unwrap_or(0) + 1,
        snippet: entry["text"].as_str().unwrap_or("").to_string(),
    })
}

/// Run `ast-grep` for a structural pattern and collect the matches, reading
/// its stdout as it is produced.
fn ast_grep(pattern: &str, lang: &str, path: &str) -> Vec<Match> {
    // stderr is discarded, as the previous `Command::output` call discarded
    // it: ast-grep's pattern warnings are its own diagnostics, not part of
    // this command's result, and inheriting them writes them into the
    // caller's terminal alongside the answer.
    let mut child = match Command::new("sg")
        .args(["run", "-p", pattern, "-l", lang, "--json=stream", path])
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
        .filter_map(|l| parse_entry(&l))
        .collect();
    let _ = child.wait();
    matches
}

/// The emitted document, serialized from borrowed structures. The key order
/// is the order the result has always carried.
struct Document<'a> {
    pattern: &'a str,
    lang: &'a str,
    matches: &'a [EnrichedMatch<'a>],
    files_matched: usize,
    repo_context: &'a Value,
}

impl Serialize for Document<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(6))?;
        map.serialize_entry("pattern", self.pattern)?;
        map.serialize_entry("lang", self.lang)?;
        map.serialize_entry("matches", self.matches)?;
        map.serialize_entry("match_count", &self.matches.len())?;
        map.serialize_entry("files_matched", &self.files_matched)?;
        map.serialize_entry("repo_context", self.repo_context)?;
        map.end()
    }
}

pub fn run(
    pattern: &str,
    lang: &str,
    path: &str,
    as_json: bool,
    sink: &Sink,
) -> Result<()> {
    let matches = ast_grep(pattern, lang, path);
    let abs = cache::absolutize(Path::new(path));
    let search_root = cache::worktree_root_for(&abs).unwrap_or_else(|| cache::display_root(&abs));
    let (enriched, files_matched) = enrich::enrich(&matches, &search_root);
    let repo_ctx = repo_context::repo_context(&abs);

    if !as_json {
        enrich::render_human(&enriched, files_matched, &repo_ctx);
    }

    sink.emit(&Document {
        pattern,
        lang,
        matches: &enriched,
        files_matched,
        repo_context: &repo_ctx,
    })
}
