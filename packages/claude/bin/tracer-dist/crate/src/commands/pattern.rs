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
use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::io::{BufRead, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;

const DIAGNOSTIC_BUDGET_BYTES: usize = 4 * 1024;

fn stderr_reader(
    mut stderr: impl Read + Send + 'static,
) -> thread::JoinHandle<Result<(Vec<u8>, usize)>> {
    thread::spawn(move || {
        let mut kept = Vec::new();
        let mut total = 0;
        let mut chunk = [0; 8192];
        loop {
            let read = stderr.read(&mut chunk)?;
            if read == 0 {
                break;
            }
            total += read;
            let remaining = DIAGNOSTIC_BUDGET_BYTES.saturating_sub(kept.len());
            kept.extend_from_slice(&chunk[..read.min(remaining)]);
        }
        Ok((kept, total))
    })
}

fn backend_error(
    name: &str,
    status: std::process::ExitStatus,
    stderr: Vec<u8>,
    total: usize,
) -> anyhow::Error {
    let mut diagnostic = String::from_utf8_lossy(&stderr).trim().to_string();
    if total > stderr.len() {
        diagnostic.push_str(&format!(
            " [stderr truncated at {} of {} bytes]",
            stderr.len(),
            total
        ));
    }
    if diagnostic.is_empty() {
        diagnostic = "no diagnostic output".to_string();
    }
    anyhow!("{name} failed with {status}: {diagnostic}")
}

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
            while chars
                .peek()
                .is_some_and(|n| n.is_alphanumeric() || *n == '_')
            {
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
fn parse_entry(line: &str, anchor: Option<&str>) -> Result<Match> {
    let entry: Value = serde_json::from_str(line).context("ast-grep wrote malformed JSON")?;
    let start = entry
        .get("range")
        .and_then(|v| v.get("start"))
        .and_then(|v| v.get("line"))
        .and_then(Value::as_i64)
        .context("ast-grep match is missing a start line")?;
    let text = entry
        .get("text")
        .and_then(Value::as_str)
        .context("ast-grep match is missing text")?;
    let file = entry
        .get("file")
        .and_then(Value::as_str)
        .context("ast-grep match is missing a file path")?;
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
    Ok(Match {
        file: file.to_string(),
        line: start + offset + 1,
        snippet,
    })
}

fn run_prefilter(literal: &str, rg_type: &str, paths: &[String]) -> Result<Vec<String>> {
    let mut cmd = Command::new("rg");
    cmd.args(["--files-with-matches", "--fixed-strings", "--null"]);
    if !rg_type.is_empty() {
        cmd.args(["--type", rg_type]);
    }
    cmd.arg("--").arg(literal).args(paths);
    let mut child = match cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn() {
        Ok(child) => child,
        Err(source) if source.kind() == std::io::ErrorKind::ArgumentListTooLong => {
            if paths.len() == 1 {
                return Err(source).context(format!(
                    "one ripgrep candidate argument cannot fit: {:?}",
                    paths[0]
                ));
            }
            let middle = paths.len() / 2;
            let mut matches = run_prefilter(literal, rg_type, &paths[..middle])?;
            matches.extend(run_prefilter(literal, rg_type, &paths[middle..])?);
            return Ok(matches);
        }
        Err(source) => return Err(source).context("failed to start ripgrep prefilter"),
    };
    let stderr = stderr_reader(child.stderr.take().expect("piped ripgrep prefilter stderr"));
    let mut stdout = Vec::new();
    let read_result = child
        .stdout
        .take()
        .expect("piped ripgrep prefilter stdout")
        .read_to_end(&mut stdout)
        .context("failed to read ripgrep prefilter output");
    if read_result.is_err() {
        let _ = child.kill();
    }
    let status = child
        .wait()
        .context("failed to wait for ripgrep prefilter")?;
    let (stderr, total) = stderr
        .join()
        .map_err(|_| anyhow!("ripgrep prefilter stderr reader panicked"))??;
    read_result?;
    if !status.success() && status.code() != Some(1) {
        bail!(
            "{}",
            backend_error("ripgrep prefilter", status, stderr, total)
        );
    }
    Ok(stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| String::from_utf8_lossy(path).into_owned())
        .collect())
}

/// The files under `path` that contain every literal word in the pattern.
/// Parsing a file is orders of magnitude dearer than scanning it, and a
/// pattern's literal words must appear verbatim in any file that can match
/// it, so this narrows the parse set without changing the answer. `None`
/// means "no usable literal" — the whole path is parsed, as before.
fn candidate_files(literals: &[String], path: &str, rg_type: &str) -> Result<Option<Vec<String>>> {
    let Some(first) = literals.first() else {
        return Ok(None);
    };
    let mut files = run_prefilter(first, rg_type, &[path.to_string()])?;
    for word in literals.iter().skip(1) {
        if files.is_empty() {
            break;
        }
        let matches: HashSet<String> = run_prefilter(word, rg_type, &files)?.into_iter().collect();
        files.retain(|file| matches.contains(file));
    }
    Ok(Some(files))
}

/// Run `ast-grep` for a structural pattern and collect the matches, reading
/// its stdout as it is produced.
fn ast_grep(
    pattern: &str,
    lang: &str,
    paths: &[String],
    anchor: Option<&str>,
) -> Result<Vec<Match>> {
    let mut cmd = Command::new("sg");
    cmd.args(["run", "-p", pattern, "-l", lang, "--json=stream"]);
    if paths.is_empty() {
        cmd.arg("--stdin").stdin(Stdio::null());
    } else {
        cmd.arg("--").args(paths);
    }
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to start ast-grep")?;
    let stderr = stderr_reader(child.stderr.take().expect("piped ast-grep stderr"));
    let mut matches = Vec::new();
    let mut parse_result = Ok(());
    for line in std::io::BufReader::new(child.stdout.take().expect("piped ast-grep stdout")).lines()
    {
        match line.context("failed to read ast-grep output") {
            Ok(line) if line.trim().is_empty() => {}
            Ok(line) => match parse_entry(&line, anchor) {
                Ok(found) => matches.push(found),
                Err(error) => {
                    parse_result = Err(error);
                    let _ = child.kill();
                    break;
                }
            },
            Err(error) => {
                parse_result = Err(error);
                let _ = child.kill();
                break;
            }
        }
    }
    let status = child.wait().context("failed to wait for ast-grep")?;
    let (stderr, total) = stderr
        .join()
        .map_err(|_| anyhow!("ast-grep stderr reader panicked"))??;
    parse_result?;
    if status.success() || status.code() == Some(1) {
        Ok(matches)
    } else {
        Err(backend_error("ast-grep", status, stderr, total))
    }
}

fn ast_grep_batches(
    pattern: &str,
    lang: &str,
    paths: &[String],
    anchor: Option<&str>,
) -> Result<Vec<Match>> {
    match ast_grep(pattern, lang, paths, anchor) {
        Ok(matches) => Ok(matches),
        Err(error)
            if error
                .downcast_ref::<std::io::Error>()
                .is_some_and(|source| source.kind() == std::io::ErrorKind::ArgumentListTooLong) =>
        {
            if paths.len() == 1 {
                return Err(error).context(format!(
                    "one structural candidate argument cannot fit: {:?}",
                    paths[0]
                ));
            }
            let middle = paths.len() / 2;
            let mut matches = ast_grep_batches(pattern, lang, &paths[..middle], anchor)?;
            matches.extend(ast_grep_batches(pattern, lang, &paths[middle..], anchor)?);
            Ok(matches)
        }
        Err(error) => Err(error),
    }
}

pub fn run(pattern: &str, lang: &str, path: &str, as_json: bool, sink: &Sink) -> Result<()> {
    let row = match crate::lang::resolve(lang) {
        Some(row) if !row.sg.is_empty() => row,
        Some(row) => {
            eprintln!(
                "Error: {:?} has no code-shape grammar. Use `trace grep` for it.",
                row.name
            );
            std::process::exit(2);
        }
        None => {
            eprintln!(
                "Error: unknown language {lang:?}. Accepted: {}",
                crate::lang::accepted()
            );
            std::process::exit(2);
        }
    };
    let words = literals(pattern);
    let anchor = words.first().cloned();
    let paths = candidate_files(&words, path, row.rg)?.unwrap_or_else(|| vec![path.to_string()]);
    let matches = if paths.is_empty() {
        ast_grep(pattern, row.sg, &[], anchor.as_deref())?
    } else {
        ast_grep_batches(pattern, row.sg, &paths, anchor.as_deref())?
    };
    let abs = cache::absolutize(Path::new(path));
    let search_root = cache::worktree_root_for(&abs).unwrap_or_else(|| cache::display_root(&abs));
    let ((enriched, files, signpost), repo_ctx) = rayon::join(
        || {
            let signpost = enrich::signpost(anchor.as_deref(), &search_root);
            let (enriched, files) = enrich::enrich(&matches, &search_root);
            (enriched, files, signpost)
        },
        || repo_context::repo_context(&abs),
    );

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
