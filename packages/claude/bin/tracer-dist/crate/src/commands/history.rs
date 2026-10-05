//! `trace history` — three git-archaeology modes in one command.
//!
//!   trace history <file>                whole-file mode
//!   trace history <file> <symbol>       function-line history (git log -L)
//!   trace history --contains <pattern>  pickaxe (git log -S)
//!
//! Whole-file mode reuses the cached bulk git pipeline (`git_activity`) for
//! the settled-history fields. The function mode shells out to git's native
//! function-range history. The pickaxe reads the commit index: each distinct
//! blob the indexed commits changed is read once and the pattern counted in
//! it, and a commit matches where a file's old and new counts differ, the rule
//! `git log -S` applies. A match names the declaration `surface::innermost`
//! picks from the file's extraction at that commit.

use crate::git_activity::{Commit, CommitIndex, Commits, CommitsChange, FileChange};
use crate::{cache, git_activity};
use anyhow::{bail, Result};
use rayon::prelude::*;
use regex::bytes::{Regex, RegexBuilder};
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::Path;

const RECENT_COMMITS: i64 = 10;
const FUNCTION_COMMITS: i64 = 20;
const NEWEST_SHOWN: usize = 29;
const BINARY_PROBE: usize = 8000;
const ANNOTATED_FILES: usize = 200;

// ---------- mode 1: whole-file ----------

fn recent_commits(repo_root: &Path, relative: &str, n: i64) -> Vec<Value> {
    let out = crate::git_activity::git_output(
        repo_root,
        [
            "log",
            &format!("-{n}"),
            "--pretty=format:%h|%an|%ad|%s",
            "--date=short",
            "--",
            relative,
        ],
    );
    let mut commits = Vec::new();
    if let Ok(o) = out {
        for line in String::from_utf8_lossy(&o.stdout).split('\n') {
            let parts: Vec<&str> = line.splitn(4, '|').collect();
            if parts.len() < 4 {
                continue;
            }
            commits.push(json!({
                "sha": parts[0],
                "author": parts[1],
                "date": parts[2],
                "subject": parts[3],
            }));
        }
    }
    commits
}

fn blame_top_authors(repo_root: &Path, file: &Path, top: usize) -> Vec<Value> {
    let out = crate::git_activity::git_output(
        repo_root,
        ["blame", "--line-porcelain", &file.to_string_lossy()],
    );
    let mut order: Vec<String> = Vec::new();
    let mut counts: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    if let Ok(o) = out {
        for line in String::from_utf8_lossy(&o.stdout).split('\n') {
            if let Some(author) = line.strip_prefix("author ") {
                if !counts.contains_key(author) {
                    order.push(author.to_string());
                }
                *counts.entry(author.to_string()).or_insert(0) += 1;
            }
        }
    }
    // Counter.most_common: count desc, ties keep first-insertion order.
    let mut v: Vec<(usize, &String, i64)> = order
        .iter()
        .enumerate()
        .map(|(i, a)| (i, a, counts[a]))
        .collect();
    v.sort_by(|a, b| b.2.cmp(&a.2).then(a.0.cmp(&b.0)));
    v.into_iter()
        .take(top)
        .map(|(_, a, n)| json!({"author": a, "lines": n}))
        .collect()
}

/// Full transitive rename lineage, newest -> oldest, via
/// `git log --follow --name-status --diff-filter=R`.
fn rename_chain(repo_root: &Path, relative: &str) -> Vec<String> {
    let out = crate::git_activity::git_output(
        repo_root,
        [
            "log",
            "--follow",
            "--name-status",
            "--diff-filter=R",
            "--pretty=format:",
            "--",
            relative,
        ],
    );
    let mut chain: Vec<String> = Vec::new();
    let mut current = relative.to_string();
    if let Ok(o) = out {
        for line in String::from_utf8_lossy(&o.stdout).split('\n') {
            if line.trim().is_empty() || !line.starts_with('R') {
                continue;
            }
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() < 3 {
                continue;
            }
            let (old, new) = (parts[1], parts[2]);
            if new == current {
                chain.push(old.to_string());
                current = old.to_string();
            }
        }
    }
    chain
}

fn whole_file_payload(file: &Path, repo_root: &Path) -> Value {
    let relative = cache::relative_to_root(file, repo_root);
    let activity_map = git_activity::bulk_cached(repo_root);
    let activity = activity_map
        .get(&relative)
        .cloned()
        .unwrap_or_else(git_activity::GitActivity::empty);

    json!({
        "mode": "file",
        "file": relative,
        "commit_count": activity.commit_count,
        "commits_30d": activity.commits_30d,
        "first_seen": activity.first_seen,
        "last_modified": activity.last_modified,
        "last_author": activity.last_author,
        "last_subject": activity.last_subject,
        "top_author": activity.top_author,
        "working_state": activity.working_state,
        "present_in": activity.present_in,
        "recent_commits": recent_commits(repo_root, &relative, RECENT_COMMITS),
        "top_blame_authors": blame_top_authors(repo_root, file, 5),
        "rename_chain": rename_chain(repo_root, &relative),
        "co_changed": activity.co_changed.iter()
            .map(|(p, n)| json!({"path": p.as_ref(), "commits": n})).collect::<Vec<_>>(),
    })
}

fn render_whole_file(p: &Value) {
    println!("File: {}", p["file"].as_str().unwrap_or(""));
    let state = p["working_state"].as_str();
    let state_part = state
        .map(|s| format!(", working_state={s}"))
        .unwrap_or_default();
    println!(
        "Commits: {} total, {} in last 30 days{}",
        p["commit_count"].as_i64().unwrap_or(0),
        p["commits_30d"].as_i64().unwrap_or(0),
        state_part
    );
    let first = p["first_seen"].as_str();
    let last = p["last_modified"].as_str();
    if first.is_some() || last.is_some() {
        println!(
            "First seen: {}  Last modified: {} ({})",
            first.unwrap_or(""),
            last.unwrap_or(""),
            p["last_author"].as_str().unwrap_or("")
        );
    }
    if let Some(s) = p["last_subject"].as_str() {
        println!("Last subject: {s}");
    }
    if let Some(s) = p["top_author"].as_str() {
        println!("Top author (by commits): {s}");
    }
    let present: Vec<&str> = p["present_in"]
        .as_array()
        .map(|a| a.iter().filter_map(|x| x.as_str()).collect())
        .unwrap_or_default();
    if !present.is_empty() {
        println!("Present on: {}", present.join(", "));
    }
    println!();

    let chain: Vec<&str> = p["rename_chain"]
        .as_array()
        .map(|a| a.iter().filter_map(|x| x.as_str()).collect())
        .unwrap_or_default();
    if !chain.is_empty() {
        println!("Rename chain (newest -> oldest):");
        println!("  {}", p["file"].as_str().unwrap_or(""));
        for old in &chain {
            println!("  <- {old}");
        }
        println!();
    }

    if let Some(commits) = p["recent_commits"].as_array() {
        if !commits.is_empty() {
            println!("Recent commits:");
            for c in commits {
                println!(
                    "  {} {} {}: {}",
                    c["sha"].as_str().unwrap_or(""),
                    c["date"].as_str().unwrap_or(""),
                    c["author"].as_str().unwrap_or(""),
                    c["subject"].as_str().unwrap_or("")
                );
            }
            println!();
        }
    }

    if let Some(authors) = p["top_blame_authors"].as_array() {
        if !authors.is_empty() {
            println!("Top blame authors (lines in current file):");
            for e in authors {
                println!(
                    "  {:>5}  {}",
                    e["lines"].as_i64().unwrap_or(0),
                    e["author"].as_str().unwrap_or("")
                );
            }
            println!();
        }
    }

    if let Some(co) = p["co_changed"].as_array() {
        if !co.is_empty() {
            println!("Files that change together:");
            for e in co {
                println!(
                    "  {:>4}  {}",
                    e["commits"].as_i64().unwrap_or(0),
                    e["path"].as_str().unwrap_or("")
                );
            }
        }
    }
}

// ---------- mode 2: function-level ----------

fn function_history(repo_root: &Path, relative: &str, symbol: &str, n: i64) -> Result<Vec<Value>> {
    let out = crate::git_activity::git_output(
        repo_root,
        [
            "log",
            &format!("-L:{symbol}:{relative}"),
            &format!("-{n}"),
            "--pretty=format:%x00COMMIT%x00%H%x00%an%x00%ad%x00%s",
            "--date=short",
            "--no-color",
        ],
    )?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        bail!("git log -L failed for symbol '{symbol}' in {relative}: {stderr}");
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut commits: Vec<Value> = Vec::new();
    let mut current: Option<(String, String, String, String)> = None;
    let mut hunk_lines: Vec<String> = Vec::new();

    let flush = |commits: &mut Vec<Value>,
                 current: &Option<(String, String, String, String)>,
                 hunk_lines: &[String]| {
        if let Some((sha, author, date, subject)) = current {
            commits.push(json!({
                "sha": sha,
                "author": author,
                "date": date,
                "subject": subject,
                "hunk": hunk_lines.join("\n").trim_end().to_string(),
            }));
        }
    };

    for line in stdout.split('\n') {
        if let Some(rest) = line.strip_prefix("\u{0}COMMIT\u{0}") {
            flush(&mut commits, &current, &hunk_lines);
            let parts: Vec<&str> = rest.split('\u{0}').collect();
            let sha: String = parts.first().unwrap_or(&"").chars().take(12).collect();
            current = Some((
                sha,
                parts.get(1).unwrap_or(&"").to_string(),
                parts.get(2).unwrap_or(&"").to_string(),
                parts.get(3).unwrap_or(&"").to_string(),
            ));
            hunk_lines = Vec::new();
            continue;
        }
        if current.is_some() {
            hunk_lines.push(line.to_string());
        }
    }
    flush(&mut commits, &current, &hunk_lines);
    Ok(commits)
}

fn render_function(p: &Value) {
    let commits = p["commits"].as_array().cloned().unwrap_or_default();
    let head = format!(
        "File: {}\nSymbol: {}\nCommits touching symbol: {}\n\n",
        p["file"].as_str().unwrap_or(""),
        p["symbol"].as_str().unwrap_or(""),
        commits.len()
    );
    print!("{head}");
    let mut entries: Vec<crate::output::Entry> = Vec::with_capacity(commits.len());
    for (index, c) in commits.iter().enumerate() {
        let header = format!(
            "{} {} {}: {}",
            c["sha"].as_str().unwrap_or(""),
            c["date"].as_str().unwrap_or(""),
            c["author"].as_str().unwrap_or(""),
            c["subject"].as_str().unwrap_or("")
        );
        let mut whole = header.clone();
        for line in c["hunk"].as_str().unwrap_or("").split('\n').filter(|line| !line.is_empty()) {
            whole.push_str(&format!("\n  {line}"));
        }
        entries.push(crate::output::Entry { rank: -(index as i64), levels: vec![format!("{whole}\n"), header] });
    }
    let (chosen, shortened) = crate::output::fit(&entries, head.len() + crate::output::closing_room(entries.len(), "commits"));
    for (_, text) in chosen {
        println!("{text}");
    }
    if shortened > 0 {
        println!("{}", crate::output::shortened_line(shortened, entries.len(), "commits"));
    }
}

// ---------- mode 3: pickaxe ----------

struct Found<'a> {
    id: &'a str,
    commit: &'a Commit,
    files: Vec<&'a FileChange>,
}

/// Each blob's count of the pattern, and the blobs found to be binary, which
/// git's own test calls a blob with a zero byte in its first 8,000 bytes.
struct Counts {
    of: HashMap<String, usize>,
    binary: Vec<String>,
}

impl Counts {
    fn of(&self, blob: &Option<String>) -> Option<usize> {
        match blob {
            None => Some(0),
            Some(id) => self.of.get(id).copied(),
        }
    }
}

fn matcher(pattern: &str, regex: bool) -> Option<Regex> {
    let source = if regex { pattern.to_string() } else { regex::escape(pattern) };
    RegexBuilder::new(&source).multi_line(true).build().ok()
}

fn counts(searched: &[(&str, &Commit)], known_binary: &BTreeSet<String>, matcher: &Regex, repo_root: &Path) -> Counts {
    let blobs: Vec<String> = searched
        .iter()
        .flat_map(|(_, commit)| &commit.changes)
        .flat_map(|change| change.old.iter().chain(&change.new))
        .filter(|blob| !known_binary.contains(*blob))
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let share = blobs.len().div_ceil(rayon::current_num_threads().max(1)).max(1);
    let read: Vec<(String, Option<usize>)> = blobs
        .par_chunks(share)
        .flat_map_iter(|chunk| {
            let mut read = Vec::with_capacity(chunk.len());
            git_activity::blobs(repo_root, chunk, |index, bytes| {
                if let Some(bytes) = bytes {
                    let binary = bytes[..bytes.len().min(BINARY_PROBE)].contains(&0);
                    read.push((chunk[index].clone(), (!binary).then(|| matcher.find_iter(bytes).count())));
                }
            });
            read
        })
        .collect();
    let mut counts = Counts {
        of: HashMap::with_capacity(read.len()),
        binary: Vec::new(),
    };
    for (blob, count) in read {
        match count {
            Some(count) => {
                counts.of.insert(blob, count);
            }
            None => counts.binary.push(blob),
        }
    }
    counts
}

fn pickaxe<'a>(pattern: &str, regex: bool, commits: &'a Commits, repo_root: &Path) -> Result<(Vec<Found<'a>>, Option<Counts>)> {
    let searched: Vec<(&str, &Commit)> = commits
        .order
        .iter()
        .filter_map(|id| Some((id.as_str(), commits.commits.get(id)?)))
        .filter(|(_, commit)| !commit.merge)
        .collect();
    let Some(matcher) = matcher(pattern, regex) else {
        return Ok((confirmed(pattern, &searched, repo_root)?, None));
    };
    let counts = crate::timing::phase("count blobs", || counts(&searched, &commits.binary, &matcher, repo_root));
    if !counts.binary.is_empty() {
        cache::update(&CommitIndex { repo_root }, repo_root, CommitsChange::Binary(counts.binary.clone()));
    }
    let changed = |change: &FileChange| match (counts.of(&change.old), counts.of(&change.new)) {
        (Some(old), Some(new)) if regex => (old > 0 || new > 0) && change.old != change.new,
        (Some(old), Some(new)) => old != new,
        _ => false,
    };
    let found: Vec<Found> = searched
        .iter()
        .filter_map(|(id, commit)| {
            let files: Vec<&FileChange> = commit.changes.iter().filter(|change| changed(change)).collect();
            (!files.is_empty()).then_some(Found { id, commit, files })
        })
        .collect();
    if !regex {
        return Ok((found, Some(counts)));
    }
    let candidates: Vec<(&str, &Commit)> = found.iter().map(|found| (found.id, found.commit)).collect();
    Ok((confirmed(pattern, &candidates, repo_root)?, Some(counts)))
}

/// The candidates `git log -G` confirms, each with the files it names: a
/// regular expression's meaning is git's, so git makes the final call.
fn confirmed<'a>(pattern: &str, candidates: &[(&'a str, &'a Commit)], repo_root: &Path) -> Result<Vec<Found<'a>>> {
    if candidates.is_empty() {
        return Ok(Vec::new());
    }
    let request = candidates.iter().map(|(id, _)| *id).collect::<Vec<_>>().join("\n").into_bytes();
    let needle = format!("-G{pattern}");
    let (stdout, finished) = git_activity::piped(
        repo_root,
        &["log", "--no-walk=unsorted", "--stdin", &needle, "-M", "--name-only", "-z", "--format=COMMIT|%H"],
        request,
        |_| {},
        |mut stdout| {
            let mut read = Vec::new();
            std::io::Read::read_to_end(&mut stdout, &mut read)?;
            Ok(read)
        },
    )?;
    if !finished {
        bail!("git log {needle} failed: the pattern is not a regular expression git reads");
    }
    let mut named: HashMap<String, Vec<String>> = HashMap::new();
    let mut current: Option<String> = None;
    for field in stdout.split(|byte| *byte == 0) {
        let field = String::from_utf8_lossy(field);
        let field = field.trim_matches('\n');
        if let Some(id) = field.strip_prefix("COMMIT|") {
            current = Some(id.to_string());
            named.entry(id.to_string()).or_default();
        } else if let (Some(id), false) = (&current, field.is_empty()) {
            named.entry(id.clone()).or_default().push(field.to_string());
        }
    }
    Ok(candidates
        .iter()
        .filter_map(|(id, commit)| {
            let paths: HashSet<&str> = named.get(*id)?.iter().map(String::as_str).collect();
            let files: Vec<&FileChange> = commit.changes.iter().filter(|change| paths.contains(change.path.as_str())).collect();
            (!files.is_empty()).then_some(Found { id, commit, files })
        })
        .collect())
}

/// Which shown commits get each file's line and declaration: whole commits,
/// the oldest first and then newest to oldest, while their files fit
/// `ANNOTATED_FILES`. A commit past it still names its files.
fn annotated(shown: &[&Found]) -> Vec<bool> {
    let mut annotated = vec![false; shown.len()];
    let mut left = ANNOTATED_FILES;
    let oldest = shown.len().saturating_sub(1);
    for index in std::iter::once(oldest).chain(0..oldest) {
        if let Some(found) = shown.get(index).filter(|found| found.files.len() <= left) {
            left -= found.files.len();
            annotated[index] = true;
        }
    }
    annotated
}

/// The lines a commit's change added and removed in one file, each numbered
/// on its own side.
#[derive(Default)]
struct Hunks {
    added: Vec<(i64, Vec<u8>)>,
    removed: Vec<(i64, Vec<u8>)>,
}

/// Each commit's changed lines in the files it matched, keyed by the commit
/// and the file's path after the change, read from git's own diff in one
/// `git log -p -U0`, so a line names what `git show` names.
fn hunks(commits: &[&Found], repo_root: &Path) -> HashMap<(String, String), Hunks> {
    let mut hunks: HashMap<(String, String), Hunks> = HashMap::new();
    if commits.is_empty() {
        return hunks;
    }
    let request = commits.iter().map(|found| found.id).collect::<Vec<_>>().join("\n").into_bytes();
    let paths: BTreeSet<&str> = commits
        .iter()
        .flat_map(|found| &found.files)
        .flat_map(|change| [change.path.as_str(), change.old_path()])
        .collect();
    let mut args = vec![
        "-c",
        "core.quotePath=false",
        "log",
        "--no-walk=unsorted",
        "--stdin",
        "-p",
        "-U0",
        "-M",
        "--diff-merges=first-parent",
        "--no-color",
        "--no-ext-diff",
        "--no-textconv",
        "--format=COMMIT|%H",
        "--",
    ];
    args.extend(paths);
    let Ok((patch, _)) = git_activity::piped(
        repo_root,
        &args,
        request,
        |command| {
            command.env("GIT_LITERAL_PATHSPECS", "1");
        },
        |mut stdout| {
            let mut read = Vec::new();
            std::io::Read::read_to_end(&mut stdout, &mut read)?;
            Ok(read)
        },
    ) else {
        return hunks;
    };
    let text = |bytes: &[u8]| String::from_utf8_lossy(bytes).into_owned();
    let (mut commit, mut path, mut old_path) = (String::new(), None::<String>, None::<String>);
    let (mut old, mut new, mut in_hunk) = (0_i64, 0_i64, false);
    for line in patch.split(|byte| *byte == b'\n') {
        if let Some(id) = line.strip_prefix(b"COMMIT|") {
            (commit, path, in_hunk) = (text(id), None, false);
        } else if line.starts_with(b"diff --git ") {
            (path, old_path, in_hunk) = (None, None, false);
        } else if let Some(range) = line.strip_prefix(b"@@ ") {
            let starts: Vec<i64> = text(range)
                .split_whitespace()
                .take(2)
                .filter_map(|side| side[1..].split(',').next()?.parse().ok())
                .collect();
            if let [from, to] = starts[..] {
                (old, new, in_hunk) = (from, to, true);
            }
        } else if !in_hunk {
            if let Some(name) = line.strip_prefix(b"--- a/") {
                old_path = Some(text(name));
            } else if let Some(name) = line.strip_prefix(b"+++ b/") {
                path = Some(text(name));
            } else if line == b"+++ /dev/null" {
                path = old_path.clone();
            }
        } else if let Some(file) = &path {
            let entry = hunks.entry((commit.clone(), file.clone())).or_default();
            match line.first() {
                Some(b'-') => {
                    entry.removed.push((old, line[1..].to_vec()));
                    old += 1;
                }
                Some(b'+') => {
                    entry.added.push((new, line[1..].to_vec()));
                    new += 1;
                }
                _ => {}
            }
        }
    }
    hunks
}

/// One direction of a file's change: what it did to the pattern, the file as
/// that side names it, and the first changed line with its declaration.
fn row(change: &str, path: &str, lines: &[i64], bytes: Option<&[u8]>, repo_root: &Path) -> Value {
    let line = lines.first().copied();
    let symbol = bytes.zip(line).and_then(|(bytes, line)| {
        let extraction = crate::file_facts::extraction_of(bytes, path, repo_root)?;
        let declarations = &extraction.declarations;
        crate::surface::innermost(declarations.iter().map(|d| (d.header_line, d.end_line)), line)
            .map(|index| declarations[index].name.clone())
    });
    json!({
        "change": change,
        "path": path,
        "line": line,
        "lines": (!lines.is_empty()).then_some(lines.len()),
        "enclosing_symbol": symbol,
    })
}

/// A file's rows: the lines its change added, then the lines it removed. A
/// file past `ANNOTATED_FILES`, or a pattern only git reads, takes its
/// direction from the counts alone.
fn file_rows(
    change: &FileChange,
    hunks: Option<&Hunks>,
    matcher: Option<&Regex>,
    counts: Option<&Counts>,
    contents: &HashMap<String, Vec<u8>>,
    repo_root: &Path,
) -> Vec<Value> {
    if let (Some(hunks), Some(matcher)) = (hunks, matcher) {
        let matching = |lines: &[(i64, Vec<u8>)]| -> Vec<i64> {
            lines.iter().filter(|(_, text)| matcher.is_match(text)).map(|(line, _)| *line).collect()
        };
        let read = |blob: &Option<String>| blob.as_ref().and_then(|blob| contents.get(blob)).map(Vec::as_slice);
        let (added, removed) = (matching(&hunks.added), matching(&hunks.removed));
        let (added, removed) = rayon::join(
            || (!added.is_empty()).then(|| row("added", &change.path, &added, read(&change.new), repo_root)),
            || (!removed.is_empty()).then(|| row("removed", change.old_path(), &removed, read(&change.old), repo_root)),
        );
        let rows: Vec<Value> = added.into_iter().chain(removed).collect();
        if !rows.is_empty() {
            return rows;
        }
    }
    let (old, new) = counts.map_or((None, None), |counts| (counts.of(&change.old), counts.of(&change.new)));
    match (old, new) {
        (Some(old), Some(new)) if old > new => vec![row("removed", change.old_path(), &[], None, repo_root)],
        (Some(old), Some(new)) if new > old => vec![row("added", &change.path, &[], None, repo_root)],
        _ => vec![row("changed", &change.path, &[], None, repo_root)],
    }
}

/// Each shown commit with its files' rows: the lines the change added and
/// removed, each first line with its enclosing declaration.
fn annotate(shown: &[&Found], matcher: Option<&Regex>, counts: Option<&Counts>, repo_root: &Path) -> Vec<Value> {
    let annotated = annotated(shown);
    let names: Vec<String> = shown
        .iter()
        .zip(&annotated)
        .filter(|(_, annotated)| **annotated)
        .flat_map(|(found, _)| &found.files)
        .flat_map(|change| change.old.iter().chain(&change.new))
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut contents: HashMap<String, Vec<u8>> = HashMap::with_capacity(names.len());
    git_activity::blobs(repo_root, &names, |index, bytes| {
        if let Some(bytes) = bytes {
            contents.insert(names[index].clone(), bytes.to_vec());
        }
    });
    let read: Vec<&Found> = shown.iter().zip(&annotated).filter(|(_, annotated)| **annotated).map(|(found, _)| *found).collect();
    let hunks = hunks(&read, repo_root);
    let files: Vec<(usize, &str, &FileChange)> = shown
        .iter()
        .enumerate()
        .flat_map(|(index, found)| found.files.iter().map(move |change| (index, found.id, *change)))
        .collect();
    let rows: Vec<(usize, Vec<Value>)> = files
        .par_iter()
        .map(|(index, id, change)| {
            let hunks = hunks.get(&(id.to_string(), change.path.clone()));
            (*index, file_rows(change, hunks, matcher, counts, &contents, repo_root))
        })
        .collect();
    let mut matches: Vec<Vec<Value>> = vec![Vec::new(); shown.len()];
    for (index, file) in rows {
        matches[index].extend(file);
    }
    shown
        .iter()
        .zip(matches)
        .map(|(found, matches)| {
            json!({
                "sha": found.id.chars().take(12).collect::<String>(),
                "date": found.commit.date,
                "author": found.commit.author,
                "subject": found.commit.subject,
                "matches": matches,
            })
        })
        .collect()
}

fn pickaxe_payload(pattern: &str, regex: bool, all: bool, repo_root: &Path) -> Result<Value> {
    let commits = git_activity::commits(repo_root);
    let (found, counts) = pickaxe(pattern, regex, &commits, repo_root)?;
    let shown: Vec<&Found> = if all || found.len() <= NEWEST_SHOWN + 1 {
        found.iter().collect()
    } else {
        found[..NEWEST_SHOWN].iter().chain(found.last()).collect()
    };
    let matcher = matcher(pattern, regex);
    Ok(json!({
        "mode": "contains",
        "pattern": pattern,
        "commit_count": found.len(),
        "between": found.len() - shown.len(),
        "searched": commits.order.len(),
        "floor": commits.truncated(),
        "commits": crate::timing::phase("annotate", || annotate(&shown, matcher.as_ref(), counts.as_ref(), repo_root)),
    }))
}

fn render_pickaxe(p: &Value) {
    let total = p["commit_count"].as_u64().unwrap_or(0);
    let between = p["between"].as_u64().unwrap_or(0);
    let mut head = format!(
        "Pattern: {}\nCommits introducing or removing the pattern: {total}",
        p["pattern"].as_str().unwrap_or("")
    );
    if between > 0 {
        head.push_str(&format!(", the newest {NEWEST_SHOWN} and the oldest shown"));
    }
    if p["floor"].as_bool().unwrap_or(false) {
        head.push_str(&format!(", within the newest {} commits", p["searched"].as_u64().unwrap_or(0)));
    }
    head.push_str("\n\n");
    let commits = p["commits"].as_array().cloned().unwrap_or_default();
    let entries: Vec<crate::output::Entry> = commits
        .iter()
        .enumerate()
        .map(|(index, commit)| {
            let header = format!(
                "{} {} {}: {}",
                commit["sha"].as_str().unwrap_or(""),
                commit["date"].as_str().unwrap_or(""),
                commit["author"].as_str().unwrap_or(""),
                commit["subject"].as_str().unwrap_or("")
            );
            let mut whole = header.clone();
            for m in commit["matches"].as_array().cloned().unwrap_or_default() {
                let sign = match m["change"].as_str() {
                    Some("added") => "+",
                    Some("removed") => "-",
                    _ => "~",
                };
                let count = match m["lines"].as_u64() {
                    Some(lines) if lines > 1 => format!("{sign}{lines}"),
                    _ => sign.to_string(),
                };
                let line_part = m["line"].as_i64().map(|l| format!("L{l}")).unwrap_or_default();
                let symbol_part = match m["enclosing_symbol"].as_str() {
                    Some(s) => format!(" [in {s}]"),
                    None => String::new(),
                };
                whole.push_str(&format!("\n  {count:<4} {line_part:<7} {}{symbol_part}", m["path"].as_str().unwrap_or("")));
            }
            crate::output::Entry {
                rank: if index + 1 == commits.len() { 1 } else { -(index as i64) },
                levels: vec![format!("{whole}\n"), header],
            }
        })
        .collect();
    let gap = (between > 0).then(|| format!("… {between} commits between: {} --all\n", crate::output::this_command()));
    let fixed = head.len() + gap.as_ref().map_or(0, |gap| gap.len() + 1) + crate::output::closing_room(entries.len(), "commits");
    let (chosen, shortened) = crate::output::fit(&entries, fixed);
    print!("{head}");
    let oldest = chosen.len().saturating_sub(1);
    for (index, (_, text)) in chosen.iter().enumerate() {
        if let (Some(gap), true) = (&gap, index == oldest) {
            println!("{gap}");
        }
        println!("{text}");
    }
    if shortened > 0 {
        println!("{}", crate::output::shortened_line(shortened, entries.len(), "commits"));
    }
}

// ---------- one commit ----------

/// One commit in full: its message body, author, parents, the files it
/// touched, and the lines it changed.
///
/// `%s` is a subject, and a subject is the least of what a commit says. The
/// body is where the reason lives — what was rejected, which invariant the
/// change protects — which is why `git show -s --format=full` is the one raw
/// git command the understand Process still prescribes. This answers it.
fn commit_payload(reference: &str, repo_root: &Path) -> Result<Value> {
    let meta = crate::git_activity::git_output(
        repo_root,
        [
            "show",
            "-s",
            "--pretty=format:%H%x00%h%x00%an%x00%ae%x00%ad%x00%P%x00%s%x00%b",
            "--date=short",
            reference,
        ],
    )?;
    if !meta.status.success() {
        let stderr = String::from_utf8_lossy(&meta.stderr).trim().to_string();
        bail!("commit not found: {reference} ({stderr})");
    }
    let text = String::from_utf8_lossy(&meta.stdout);
    let f: Vec<&str> = text.split('\u{0}').collect();
    let field = |i: usize| f.get(i).unwrap_or(&"").to_string();

    let files = crate::git_activity::git_output(
        repo_root,
        ["show", "--name-status", "--pretty=format:", "-M", reference],
    )?;
    let changed: Vec<Value> = String::from_utf8_lossy(&files.stdout)
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| {
            let tokens: Vec<&str> = l.split('\t').collect();
            let kind = tokens.first()?.chars().next()?;
            let path = tokens.last()?.to_string();
            Some(json!({"status": status_label(kind), "path": path}))
        })
        .collect();

    let patch = crate::git_activity::git_output(
        repo_root,
        [
            "show",
            "--unified=3",
            "--no-color",
            "-M",
            "--pretty=format:",
            reference,
        ],
    )?;
    let lines = String::from_utf8_lossy(&patch.stdout).trim().to_string();

    Ok(json!({
        "sha": field(0),
        "short_sha": field(1),
        "author": field(2),
        "author_email": field(3),
        "date": field(4),
        "parents": field(5).split_whitespace().map(String::from).collect::<Vec<_>>(),
        "subject": field(6),
        "body": field(7).trim(),
        "files": changed,
        "lines": lines,
    }))
}

/// `git diff --name-status` codes → the words `diff` already uses, so one
/// vocabulary describes a change wherever it is reported.
fn status_label(kind: char) -> String {
    crate::commands::diff::status_label(kind)
}

/// Each file of the patch under the path its `diff --git` line names last.
fn patch_by_path(lines: &str) -> HashMap<String, String> {
    let mut sections: HashMap<String, String> = HashMap::new();
    let mut current: Option<String> = None;
    for line in lines.split('\n') {
        if let Some(header) = line.strip_prefix("diff --git ") {
            let path = header.rfind(" b/").map_or(header, |at| &header[at + 3..]).to_string();
            sections.insert(path.clone(), line.to_string());
            current = Some(path);
        } else if let Some(section) = current.as_ref().and_then(|path| sections.get_mut(path)) {
            section.push('\n');
            section.push_str(line);
        }
    }
    sections
}

fn render_commit(p: &Value) {
    let mut head = format!(
        "{} {}  {}  {}\n",
        p["short_sha"].as_str().unwrap_or(""),
        p["date"].as_str().unwrap_or(""),
        p["author"].as_str().unwrap_or(""),
        p["subject"].as_str().unwrap_or(""),
    );
    let parents: Vec<&str> = p["parents"]
        .as_array()
        .map(|a| a.iter().filter_map(|x| x.as_str()).collect())
        .unwrap_or_default();
    if !parents.is_empty() {
        head.push_str(&format!("parents: {}\n", parents.join(" ")));
    }
    let body = p["body"].as_str().unwrap_or("");
    if !body.is_empty() {
        head.push_str(&format!("\n{body}\n"));
    }
    head.push('\n');
    print!("{head}");
    let sections = patch_by_path(p["lines"].as_str().unwrap_or(""));
    let files = p["files"].as_array().cloned().unwrap_or_default();
    let paths: Vec<&str> = files.iter().map(|file| file["path"].as_str().unwrap_or("")).collect();
    let entries: Vec<crate::output::Entry> = files
        .iter()
        .zip(&paths)
        .enumerate()
        .map(|(index, (file, path))| {
            let line = format!("  {:<12} {path}", file["status"].as_str().unwrap_or(""));
            let levels = match sections.get(*path) {
                Some(section) => vec![format!("{line}\n{section}\n"), line],
                None => vec![line],
            };
            crate::output::Entry { rank: -(index as i64), levels }
        })
        .collect();
    let fixed = head.len() + crate::output::closing_room(entries.len(), "files");
    let (texts, shortened) = crate::output::fit_listing(&entries, &paths, fixed);
    for text in texts {
        println!("{text}");
    }
    if shortened > 0 {
        println!("{}", crate::output::shortened_line(shortened, entries.len(), "files"));
    }
}

// ---------- entry point ----------

pub fn run(
    file: Option<&Path>,
    symbol: Option<&str>,
    contains: Option<&str>,
    regex: bool,
    all: bool,
    commit: Option<&str>,
    as_json: bool,
) -> Result<Value> {
    if let Some(reference) = commit {
        if file.is_some() || symbol.is_some() || contains.is_some() {
            bail!("--commit is mutually exclusive with <file>/<symbol>/--contains.");
        }
        let here = Path::new(".");
        let repo_root = cache::worktree_root_for(here).unwrap_or_else(|| cache::display_root(here));
        let payload = commit_payload(reference, &repo_root)?;
        if !as_json {
            render_commit(&payload);
        }
        return Ok(crate::output::document(
            json!({"mode": "commit", "commit": reference}),
            json!({
                "sha": payload["sha"],
                "author": payload["author"],
                "author_email": payload["author_email"],
                "date": payload["date"],
                "parents": payload["parents"],
                "subject": payload["subject"],
                "body": payload["body"],
                "lines": payload["lines"],
            }),
            payload["files"].clone(),
            json!({"files": payload["files"].as_array().map(|a| a.len()).unwrap_or(0)}),
        ));
    }
    if let Some(pattern) = contains {
        if file.is_some() || symbol.is_some() {
            bail!("--contains is mutually exclusive with <file>/<symbol> arguments.");
        }
        let here = Path::new(".");
        let repo_root = cache::worktree_root_for(here).unwrap_or_else(|| cache::display_root(here));
        let payload = pickaxe_payload(pattern, regex, all, &repo_root)?;
        if !as_json {
            render_pickaxe(&payload);
        }
        return Ok(crate::output::document(
            json!({"mode": "contains", "pattern": pattern, "regex": regex, "all": all}),
            json!({
                "repo_root": repo_root.to_string_lossy(),
                "searched_commits": payload["searched"],
                "floor": payload["floor"],
            }),
            payload["commits"].clone(),
            json!({
                "commits": payload["commit_count"],
                "shown": payload["commits"].as_array().map(|a| a.len()).unwrap_or(0),
                "between": payload["between"],
            }),
        ));
    }

    let file = match file {
        Some(f) => f,
        None => bail!("provide <file>, <file> <symbol>, or --contains <pattern>."),
    };
    if !file.is_file() {
        bail!("file not found: {}", file.display());
    }
    let file_path = file
        .canonicalize()
        .unwrap_or_else(|_| cache::absolutize(file));
    let repo_root =
        cache::worktree_root_for(&file_path).unwrap_or_else(|| cache::display_root(&file_path));

    if let Some(sym) = symbol {
        let relative = cache::relative_to_root(&file_path, &repo_root);
        let payload = json!({
            "mode": "function",
            "file": relative,
            "symbol": sym,
            "commits": function_history(&repo_root, &relative, sym, FUNCTION_COMMITS)?,
        });
        if !as_json {
            render_function(&payload);
        }
        return Ok(crate::output::document(
            json!({"mode": "function", "file": relative, "symbol": sym}),
            json!({"repo_root": repo_root.to_string_lossy()}),
            payload["commits"].clone(),
            json!({"commits": payload["commits"].as_array().map(|a| a.len()).unwrap_or(0)}),
        ));
    }

    let payload = whole_file_payload(&file_path, &repo_root);
    if !as_json {
        render_whole_file(&payload);
    }
    let mut lifecycle = payload.clone();
    if let Some(map) = lifecycle.as_object_mut() {
        map.remove("mode");
        map.remove("file");
        map.remove("recent_commits");
        map.remove("commit_count");
    }
    Ok(crate::output::document(
        json!({"mode": "file", "file": payload["file"]}),
        lifecycle,
        payload["recent_commits"].clone(),
        json!({"commits": payload["commit_count"], "commits_30d": payload["commits_30d"]}),
    ))
}
