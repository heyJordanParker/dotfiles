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
use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};
use std::io::{BufRead, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;

/// A snippet stays a snippet on minified or generated lines. The matched
/// line is the unit for ordinary source, but a 27KB single-line bundle is
/// not "context around the match" — so past `MAX_SNIPPET_CHARS` the snippet
/// becomes a character window positioned by the submatch byte offset
/// `rg --json` already reports, ellipsized on the cut side(s).
const MAX_SNIPPET_CHARS: usize = 240;
const WINDOW_BEFORE_CHARS: usize = 80;
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
fn parse_event(line: &str) -> Result<Option<Match>> {
    let event: Value = serde_json::from_str(line).context("ripgrep wrote malformed JSON")?;
    if event.get("type").and_then(|x| x.as_str()) != Some("match") {
        return Ok(None);
    }
    let data = event.get("data").context("ripgrep match is missing data")?;
    let text = data
        .get("lines")
        .and_then(|lines| lines.get("text"))
        .and_then(Value::as_str)
        .context("ripgrep match is missing line text")?;
    let match_start = data
        .get("submatches")
        .and_then(Value::as_array)
        .and_then(|matches| matches.first())
        .and_then(|matched| matched.get("start"))
        .and_then(Value::as_u64)
        .context("ripgrep match is missing a submatch start")? as usize;
    let file = data
        .get("path")
        .and_then(|path| path.get("text"))
        .and_then(Value::as_str)
        .context("ripgrep match is missing a file path")?;
    let line = data
        .get("line_number")
        .and_then(Value::as_i64)
        .context("ripgrep match is missing a line number")?;
    Ok(Some(Match {
        file: file.to_string(),
        line,
        snippet: window_snippet(text.trim_end_matches('\n'), match_start),
    }))
}

/// Run `ripgrep` for a text pattern and collect the matches, reading its
/// stdout as it is produced.
fn ripgrep(pattern: &str, path: &str, lang: Option<&str>) -> Result<Vec<Match>> {
    let mut cmd = Command::new("rg");
    cmd.arg("--json");
    if let Some(l) = lang {
        cmd.args(["--type", l]);
    }
    cmd.arg("--")
        .arg(pattern)
        .arg(path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().context("failed to start ripgrep")?;
    let stderr = stderr_reader(child.stderr.take().expect("piped ripgrep stderr"));
    let mut matches = Vec::new();
    let mut parse_result = Ok(());
    for line in std::io::BufReader::new(child.stdout.take().expect("piped ripgrep stdout")).lines()
    {
        match line
            .context("failed to read ripgrep output")
            .and_then(|line| parse_event(&line))
        {
            Ok(Some(found)) => matches.push(found),
            Ok(None) => {}
            Err(error) => {
                parse_result = Err(error);
                let _ = child.kill();
                break;
            }
        }
    }
    let status = child.wait().context("failed to wait for ripgrep")?;
    let (stderr, total) = stderr
        .join()
        .map_err(|_| anyhow!("ripgrep stderr reader panicked"))??;
    parse_result?;
    if status.success() || status.code() == Some(1) {
        Ok(matches)
    } else {
        Err(backend_error("ripgrep", status, stderr, total))
    }
}

/// Search a commit rather than the working tree. ripgrep reads files on
/// disk, so the tool for a past state is `git grep`, which reads the tree
/// object directly — no checkout, no temp files.
fn git_grep(pattern: &str, at: &str, path: &str, exts: &[&str]) -> Result<Vec<Match>> {
    let mut cmd = Command::new("git");
    cmd.args([
        "grep",
        "-z",
        "--text",
        "-n",
        "--no-color",
        "-e",
        pattern,
        at,
        "--",
    ]);
    if exts.is_empty() {
        cmd.arg(path);
    } else {
        for ext in exts {
            cmd.arg(format!("{}/*.{ext}", path.trim_end_matches('/')));
        }
    }
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to start git grep")?;
    let stderr = stderr_reader(child.stderr.take().expect("piped git grep stderr"));
    let mut stdout = Vec::new();
    let read_result = child
        .stdout
        .take()
        .expect("piped git grep stdout")
        .read_to_end(&mut stdout)
        .context("failed to read git grep output");
    if read_result.is_err() {
        let _ = child.kill();
    }
    let status = child.wait().context("failed to wait for git grep")?;
    let (stderr, total) = stderr
        .join()
        .map_err(|_| anyhow!("git grep stderr reader panicked"))??;
    read_result?;
    if !status.success() && status.code() != Some(1) {
        bail!("{}", backend_error("git grep", status, stderr, total));
    }
    let prefix = format!("{at}:").into_bytes();
    let mut matches = Vec::new();
    let mut remaining = stdout.as_slice();
    while !remaining.is_empty() {
        let path_end = remaining
            .iter()
            .position(|byte| *byte == 0)
            .context("git grep wrote a record without a path delimiter")?;
        let framed_path = &remaining[..path_end];
        let file = framed_path
            .strip_prefix(prefix.as_slice())
            .context("git grep wrote a malformed revision/path field")?;
        remaining = &remaining[path_end + 1..];

        let line_end = remaining
            .iter()
            .position(|byte| *byte == 0)
            .context("git grep wrote a record without a line delimiter")?;
        let line = std::str::from_utf8(&remaining[..line_end])
            .context("git grep wrote a non-text line number")?
            .parse()
            .context("git grep wrote an invalid line number")?;
        remaining = &remaining[line_end + 1..];

        let text_end = remaining
            .iter()
            .position(|byte| *byte == b'\n')
            .context("git grep wrote a record without a text delimiter")?;
        let text = String::from_utf8_lossy(&remaining[..text_end]);
        remaining = &remaining[text_end + 1..];
        let file = String::from_utf8_lossy(file).into_owned();
        matches.push(Match {
            file,
            line,
            snippet: window_snippet(&text, text.find(pattern).unwrap_or(0)),
        });
    }
    Ok(matches)
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
                eprintln!(
                    "Error: unknown language {l:?}. Accepted: {}",
                    crate::lang::accepted()
                );
                std::process::exit(2);
            }
        },
    };
    let matches = match at {
        Some(r) => git_grep(pattern, r, path, row.map(|l| l.exts).unwrap_or(&[])),
        None => ripgrep(pattern, path, row.map(|l| l.rg)),
    }?;
    let abs = cache::absolutize(Path::new(path));
    let search_root = cache::worktree_root_for(&abs).unwrap_or_else(|| cache::display_root(&abs));
    let ((enriched, files, signpost), repo_ctx) = rayon::join(
        || {
            let signpost = enrich::signpost(enrich::searched_name(pattern), &search_root);
            let (enriched, files) = enrich::enrich(&matches, &search_root);
            (enriched, files, signpost)
        },
        || repo_context::repo_context(&abs),
    );

    let nested = if enriched.is_empty() && abs.is_dir() {
        crate::repo_files::nested_repo_rels(&abs)
    } else {
        Vec::new()
    };

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
