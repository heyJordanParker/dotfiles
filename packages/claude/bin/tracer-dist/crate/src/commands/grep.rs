//! `trace grep` — text search with rich per-match context.
//! Wraps `rg --json` and takes ripgrep's own flags (`-i`, `-l`, `-c`, `-C`,
//! `-A`, `-B`, `-n`, `-U`, `-t`, `-g`); each match is enriched with per-file facts and grouped
//! under the declarations that enclose it.
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
use std::collections::{BTreeMap, HashMap};
use std::io::{BufRead, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;

/// ripgrep's own flags, as `trace grep` takes them.
pub struct Options {
    pub ignore_case: bool,
    /// `-l` and `-c`: each matching file once, with its facts and match count.
    pub files_only: bool,
    /// `-B` and `-A`, both `-C` when unset: lines shown before and after each match.
    pub before: usize,
    pub after: usize,
    pub multiline: bool,
    pub types: Vec<String>,
    pub globs: Vec<String>,
}

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

type Lines = HashMap<String, BTreeMap<i64, String>>;
type Groups = Vec<(PathBuf, Vec<Match>)>;

struct Event {
    file: String,
    line: i64,
    text: String,
    match_start: Option<usize>,
}

fn parse_event(line: &str) -> Result<Option<Event>> {
    let event: Value = serde_json::from_str(line).context("ripgrep wrote malformed JSON")?;
    let is_match = match event.get("type").and_then(Value::as_str) {
        Some("match") => true,
        Some("context") => false,
        _ => return Ok(None),
    };
    let data = event.get("data").context("ripgrep match is missing data")?;
    let Some(text) = data
        .get("lines")
        .and_then(|lines| lines.get("text"))
        .and_then(Value::as_str)
    else {
        return if is_match { Err(anyhow!("ripgrep match is missing line text")) } else { Ok(None) };
    };
    let match_start = if is_match {
        Some(
            data.get("submatches")
                .and_then(Value::as_array)
                .and_then(|matches| matches.first())
                .and_then(|matched| matched.get("start"))
                .and_then(Value::as_u64)
                .context("ripgrep match is missing a submatch start")? as usize,
        )
    } else {
        None
    };
    let file = data
        .get("path")
        .and_then(|path| path.get("text"))
        .and_then(Value::as_str)
        .context("ripgrep match is missing a file path")?;
    let line = data
        .get("line_number")
        .and_then(Value::as_i64)
        .context("ripgrep match is missing a line number")?;
    Ok(Some(Event {
        file: file.to_string(),
        line,
        text: text.trim_end_matches('\n').to_string(),
        match_start,
    }))
}

fn snippet(text: &str, match_start: usize) -> String {
    text.split('\n')
        .enumerate()
        .map(|(index, text)| window_snippet(text, if index == 0 { match_start } else { 0 }))
        .collect::<Vec<_>>()
        .join("\n")
}

fn keep_lines(lines: &mut Lines, file: &str, first: i64, text: &str) {
    let kept = lines.entry(file.to_string()).or_default();
    for (line, text) in (first..).zip(text.split('\n')) {
        kept.insert(line, text.to_string());
    }
}

fn ripgrep(pattern: &str, paths: &[String], options: &Options, types: &[String]) -> Result<(Vec<Match>, Lines)> {
    let mut cmd = Command::new("rg");
    cmd.args(["--json", "--threads", &rayon::current_num_threads().to_string()]);
    if options.ignore_case {
        cmd.arg("--ignore-case");
    }
    if options.multiline {
        cmd.arg("--multiline");
    }
    let with_context = options.before + options.after > 0;
    if with_context {
        cmd.args(["--before-context", &options.before.to_string()]);
        cmd.args(["--after-context", &options.after.to_string()]);
    }
    for rg_type in types {
        cmd.args(["--type", rg_type]);
    }
    for glob in &options.globs {
        cmd.args(["--glob", glob]);
    }
    cmd.arg("--")
        .arg(pattern)
        .args(paths)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().context("failed to start ripgrep")?;
    let stderr = stderr_reader(child.stderr.take().expect("piped ripgrep stderr"));
    let mut matches = Vec::new();
    let mut lines = Lines::new();
    let mut parse_result = Ok(());
    for line in std::io::BufReader::new(child.stdout.take().expect("piped ripgrep stdout")).lines()
    {
        match line
            .context("failed to read ripgrep output")
            .and_then(|line| parse_event(&line))
        {
            Ok(Some(event)) => {
                if with_context {
                    keep_lines(&mut lines, &event.file, event.line, &event.text);
                }
                if let Some(start) = event.match_start {
                    matches.push(Match {
                        snippet: snippet(&event.text, start),
                        file: event.file,
                        line: event.line,
                        ..Match::default()
                    });
                }
            }
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
        Ok((matches, lines))
    } else {
        Err(backend_error("ripgrep", status, stderr, total))
    }
}

/// Search a commit rather than the working tree. ripgrep reads files on
/// disk, so the tool for a past state is `git grep`, which reads the tree
/// object directly — no checkout, no temp files. Its files then pass through
/// the type and glob matchers ripgrep itself builds, so `-t` and `-g` select
/// the same files on a commit as on the working tree.
fn git_grep(
    pattern: &str,
    at: &str,
    repo_root: &Path,
    scope: &[(&str, String)],
    options: &Options,
    types: &ignore::types::Types,
) -> Result<Vec<(String, Match)>> {
    if options.multiline {
        bail!("-U needs ripgrep, and --at searches with git grep, which has no multiline mode");
    }
    let mut globs = ignore::overrides::OverrideBuilder::new(".");
    for glob in &options.globs {
        globs.add(glob)?;
    }
    let globs = globs.build()?;
    let selected = |file: &str| {
        let file = Path::new(file);
        !types.matched(file, false).is_ignore()
            && !globs.matched(file, false).is_ignore()
            && !file
                .ancestors()
                .skip(1)
                .take_while(|parent| !parent.as_os_str().is_empty())
                .any(|parent| globs.matched(parent, true).is_ignore())
    };
    let mut args = vec!["grep", "-z", "--text", "-n", "--no-color", "--perl-regexp"];
    if options.ignore_case {
        args.push("--ignore-case");
    }
    args.extend(["-e", pattern, at, "--"]);
    args.extend(scope.iter().map(|(_, within)| within.as_str()));
    let mut matches = crate::git_activity::git_command(repo_root, args, |command| {
        let mut child = command
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
            matches.push((
                file.clone(),
                Match {
                    file: shown(&file, scope),
                    line,
                    snippet: window_snippet(&text, text.find(pattern).unwrap_or(0)),
                    ..Match::default()
                },
            ));
        }
        Ok::<_, anyhow::Error>(matches)
    })?;
    let cwd = std::env::current_dir().ok();
    matches.retain(|(within, _)| {
        let found = repo_root.join(within);
        match cwd.as_deref().and_then(|cwd| found.strip_prefix(cwd).ok()) {
            Some(here) => selected(&here.to_string_lossy()),
            None => selected(within),
        }
    });
    Ok(matches)
}

fn shown(file: &str, scope: &[(&str, String)]) -> String {
    scope
        .iter()
        .filter_map(|(written, within)| {
            let rest = match within.as_str() {
                "." => file,
                _ if file == within => "",
                _ => file.strip_prefix(within.as_str())?.strip_prefix('/')?,
            };
            Some((within.len(), Path::new(written).join(rest)))
        })
        .max_by_key(|(depth, _)| *depth)
        .map(|(_, path)| {
            path.to_string_lossy()
                .trim_start_matches("./")
                .trim_end_matches('/')
                .to_string()
        })
        .unwrap_or_else(|| file.to_string())
}

fn lines_at(repo_root: &Path, revision: &str, matches: &[(String, Match)]) -> Result<Lines> {
    let mut lines = Lines::new();
    for (within, found) in matches {
        if lines.contains_key(&found.file) {
            continue;
        }
        let bytes = crate::git_activity::blob(repo_root, revision, within)
            .with_context(|| format!("failed to read {} at {revision}", found.file))?;
        keep_lines(&mut lines, &found.file, 1, String::from_utf8_lossy(&bytes).trim_end_matches('\n'));
    }
    Ok(lines)
}

fn root_of(path: &str) -> PathBuf {
    let abs = cache::absolutize(Path::new(path));
    cache::worktree_root_for(&abs).unwrap_or_else(|| cache::display_root(&abs))
}

fn at_revision(
    pattern: &str,
    revision: &str,
    paths: &[String],
    options: &Options,
    types: &ignore::types::Types,
) -> Result<(Groups, Lines)> {
    let mut scopes: Vec<(PathBuf, Vec<(&str, String)>)> = Vec::new();
    for path in paths {
        let root = root_of(path);
        let within = match cache::relative_to_root(&cache::absolutize(Path::new(path)), &root) {
            within if within.is_empty() => ".".to_string(),
            within => within,
        };
        match scopes.iter_mut().find(|(known, _)| *known == root) {
            Some((_, scope)) => scope.push((path.as_str(), within)),
            None => scopes.push((root, vec![(path.as_str(), within)])),
        }
    }
    let mut groups = Vec::new();
    let mut lines = Lines::new();
    for (root, scope) in scopes {
        let found = git_grep(pattern, revision, &root, &scope, options, types)?;
        if found.is_empty() {
            continue;
        }
        if options.before + options.after > 0 {
            lines.extend(lines_at(&root, revision, &found)?);
        }
        groups.push((root, found.into_iter().map(|(_, found)| found).collect()));
    }
    Ok((groups, lines))
}

fn by_root(matches: Vec<Match>, paths: &[String]) -> Groups {
    let roots: Vec<PathBuf> = paths.iter().map(|path| root_of(path)).collect();
    let mut groups = Groups::new();
    for found in matches {
        let root = paths
            .iter()
            .zip(&roots)
            .filter(|(path, _)| found.file.starts_with(path.as_str()))
            .max_by_key(|(path, _)| path.len())
            .map_or(&roots[0], |(_, root)| root);
        match groups.iter_mut().find(|(known, _)| known == root) {
            Some((_, group)) => group.push(found),
            None => groups.push((root.clone(), vec![found])),
        }
    }
    groups
}

fn attach_context(matches: &mut [Match], lines: &Lines, before: usize, after: usize) {
    for found in matches {
        let Some(kept) = lines.get(&found.file) else {
            continue;
        };
        let window = |from: i64, to: i64| kept.range(from..to).map(|(_, text)| window_snippet(text, 0)).collect();
        let next = found.line + found.snippet.split('\n').count() as i64;
        found.before = window(found.line - before as i64, found.line);
        found.after = window(next, next + after as i64);
    }
}

pub fn run(
    pattern: &str,
    paths: &[String],
    at: Option<&str>,
    options: &Options,
    as_json: bool,
    sink: &Sink,
) -> Result<()> {
    // A type is resolved before the search runs: an unknown one must fail by
    // name, never as an empty result the agent reads as "this does not
    // exist".
    let (types, matcher) = match crate::lang::ripgrep_types(&options.types) {
        Ok(resolved) => resolved,
        Err(message) => {
            eprintln!("Error: {message}");
            std::process::exit(2);
        }
    };
    let (mut groups, lines) = match at {
        Some(revision) => at_revision(pattern, revision, paths, options, &matcher)?,
        None => {
            let (matches, lines) = ripgrep(pattern, paths, options, &types)?;
            (by_root(matches, paths), lines)
        }
    };
    for (_, group) in &mut groups {
        attach_context(group, &lines, options.before, options.after);
    }
    let abs = cache::absolutize(Path::new(&paths[0]));
    let first_root = root_of(&paths[0]);
    let ((enriched, files, surfaces, signpost), repo_ctx) = rayon::join(
        || {
            let signpost = enrich::signpost(enrich::searched_name(pattern), &first_root);
            let mut enriched = Vec::new();
            let mut files = BTreeMap::new();
            let mut surfaces = BTreeMap::new();
            for (root, group) in &groups {
                let relations = crate::relations::get(root);
                let (group_enriched, group_files, group_surfaces) = enrich::enrich(group, root, Some(&relations), at);
                enriched.extend(group_enriched);
                files.extend(group_files);
                surfaces.extend(group_surfaces);
            }
            (enriched, files, surfaces, signpost)
        },
        || if as_json { repo_context::repo_context(&abs) } else { Value::Null },
    );

    let nested: Vec<String> = if enriched.is_empty() {
        paths
            .iter()
            .map(|path| cache::absolutize(Path::new(path)))
            .filter(|path| path.is_dir())
            .flat_map(|path| crate::repo_files::nested_repo_rels(&path))
            .collect()
    } else {
        Vec::new()
    };

    if !as_json {
        enrich::render_human(&enriched, &files, &surfaces, signpost.as_deref(), options.files_only);
        for r in &nested {
            println!("nested repository (its own search scope): {r}");
        }
    }
    drop(surfaces);

    sink.emit(&enrich::SearchDocument {
        query: json!({
            "pattern": pattern,
            "paths": paths,
            "at": at,
            "type": options.types,
            "glob": options.globs,
            "ignore_case": options.ignore_case,
            "multiline": options.multiline,
            "before_context": options.before,
            "after_context": options.after,
        }),
        context: enrich::SearchContext {
            files: &files,
            repo: &repo_ctx,
            signpost,
        },
        results: &enriched,
        nested_repos: &nested,
    })
}
