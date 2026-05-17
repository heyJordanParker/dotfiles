//! `trace history` — three git-archaeology modes in one command.
//!
//!   trace history <file>                whole-file mode
//!   trace history <file> <symbol>       function-line history (git log -L)
//!   trace history --contains <pattern>  pickaxe (git log -S)
//!
//! Whole-file mode reuses the cached bulk git pipeline (`git_activity`) for
//! the settled-history fields. The function and pickaxe modes shell out to
//! git's native function-range and pickaxe history — command-local
//! git/ctags calls outside the cached pipeline.

use crate::{cache, git_activity};
use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::path::Path;
use std::process::Command;

const RECENT_COMMITS: i64 = 10;
const FUNCTION_COMMITS: i64 = 20;
const PICKAXE_COMMITS: i64 = 30;

// ---------- mode 1: whole-file ----------

fn recent_commits(repo_root: &Path, relative: &str, n: i64) -> Vec<Value> {
    let out = Command::new("git")
        .args([
            "log",
            &format!("-{n}"),
            "--pretty=format:%h|%an|%ad|%s",
            "--date=short",
            "--",
            relative,
        ])
        .current_dir(repo_root)
        .output();
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
    let out = Command::new("git")
        .args(["blame", "--line-porcelain", &file.to_string_lossy()])
        .current_dir(repo_root)
        .output();
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
    let out = Command::new("git")
        .args([
            "log",
            "--follow",
            "--name-status",
            "--diff-filter=R",
            "--pretty=format:",
            "--",
            relative,
        ])
        .current_dir(repo_root)
        .output();
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
            .map(|(p, n)| json!({"path": p, "commits": n})).collect::<Vec<_>>(),
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

fn function_history(
    repo_root: &Path,
    relative: &str,
    symbol: &str,
    n: i64,
) -> Result<Vec<Value>> {
    let out = Command::new("git")
        .args([
            "log",
            &format!("-L:{symbol}:{relative}"),
            &format!("-{n}"),
            "--pretty=format:%x00COMMIT%x00%H%x00%an%x00%ad%x00%s",
            "--date=short",
            "--no-color",
        ])
        .current_dir(repo_root)
        .output()?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        bail!("git log -L failed for symbol '{symbol}' in {relative}: {stderr}");
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut commits: Vec<Value> = Vec::new();
    let mut current: Option<(String, String, String, String)> = None;
    let mut hunk_lines: Vec<String> = Vec::new();

    let flush =
        |commits: &mut Vec<Value>,
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
    println!("File: {}", p["file"].as_str().unwrap_or(""));
    println!("Symbol: {}", p["symbol"].as_str().unwrap_or(""));
    let commits = p["commits"].as_array().cloned().unwrap_or_default();
    println!("Commits touching symbol: {}", commits.len());
    println!();
    for c in &commits {
        println!(
            "{} {} {}: {}",
            c["sha"].as_str().unwrap_or(""),
            c["date"].as_str().unwrap_or(""),
            c["author"].as_str().unwrap_or(""),
            c["subject"].as_str().unwrap_or("")
        );
        let hunk = c["hunk"].as_str().unwrap_or("");
        if !hunk.is_empty() {
            for line in hunk.split('\n') {
                println!("  {line}");
            }
        }
        println!();
    }
}

// ---------- mode 3: pickaxe ----------

struct PickaxeCommit {
    sha: String,
    short_sha: String,
    author: String,
    date: String,
    subject: String,
    files: Vec<String>,
}

fn pickaxe_commits(pattern: &str, repo_root: &Path, n: i64) -> Result<Vec<PickaxeCommit>> {
    let out = Command::new("git")
        .args([
            "log",
            &format!("-{n}"),
            &format!("-S{pattern}"),
            "--name-only",
            "--pretty=format:%x00COMMIT%x00%H%x00%an%x00%ad%x00%s",
            "--date=short",
        ])
        .current_dir(repo_root)
        .output()?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        bail!("git log -S failed: {stderr}");
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut commits: Vec<PickaxeCommit> = Vec::new();
    let mut current: Option<PickaxeCommit> = None;
    for line in stdout.split('\n') {
        if let Some(rest) = line.strip_prefix("\u{0}COMMIT\u{0}") {
            if let Some(c) = current.take() {
                commits.push(c);
            }
            let parts: Vec<&str> = rest.split('\u{0}').collect();
            let sha = parts.first().unwrap_or(&"").to_string();
            current = Some(PickaxeCommit {
                short_sha: sha.chars().take(12).collect(),
                sha,
                author: parts.get(1).unwrap_or(&"").to_string(),
                date: parts.get(2).unwrap_or(&"").to_string(),
                subject: parts.get(3).unwrap_or(&"").to_string(),
                files: Vec::new(),
            });
            continue;
        }
        let stripped = line.trim();
        if !stripped.is_empty() {
            if let Some(c) = current.as_mut() {
                c.files.push(stripped.to_string());
            }
        }
    }
    if let Some(c) = current.take() {
        commits.push(c);
    }
    Ok(commits)
}

fn commit_line_for_pattern(
    commit_sha: &str,
    path: &str,
    pattern: &str,
    repo_root: &Path,
) -> Option<i64> {
    let out = Command::new("git")
        .args(["show", &format!("{commit_sha}:{path}")])
        .current_dir(repo_root)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    for (idx, line) in text.split('\n').enumerate() {
        if line.contains(pattern) {
            return Some(idx as i64 + 1);
        }
    }
    None
}

/// universal-ctags on the file's blob at `commit_sha` to find the enclosing
/// symbol for `line`. None when ctags can't resolve. Command-local ctags is
/// allowed by the brief (the foundation does not cover blob-scoped ctags).
fn enclosing_symbol(
    commit_sha: &str,
    path: &str,
    line: i64,
    repo_root: &Path,
) -> Option<String> {
    let show = Command::new("git")
        .args(["show", &format!("{commit_sha}:{path}")])
        .current_dir(repo_root)
        .output()
        .ok()?;
    if !show.status.success() || show.stdout.is_empty() {
        return None;
    }
    let suffix = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{e}"))
        .unwrap_or_else(|| ".txt".into());
    let mut tmp = tempfile::Builder::new()
        .suffix(&suffix)
        .tempfile()
        .ok()?;
    use std::io::Write;
    tmp.write_all(&show.stdout).ok()?;
    let tmp_path = tmp.path().to_path_buf();

    let ctags = Command::new("ctags")
        .args([
            "--output-format=json",
            "--fields=+ne",
            "--sort=no",
            "-f",
            "-",
            &tmp_path.to_string_lossy(),
        ])
        .output()
        .ok()?;
    if !ctags.status.success() {
        return None;
    }
    let mut enclosing: Option<(i64, String)> = None;
    for entry_line in String::from_utf8_lossy(&ctags.stdout).split('\n') {
        if entry_line.trim().is_empty() {
            continue;
        }
        let entry: Value = match serde_json::from_str(entry_line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let start = entry.get("line").and_then(|x| x.as_i64());
        let name = entry.get("name").and_then(|x| x.as_str());
        let (start, name) = match (start, name) {
            (Some(s), Some(n)) if !n.is_empty() => (s, n.to_string()),
            _ => continue,
        };
        let end = entry.get("end").and_then(|x| x.as_i64()).unwrap_or(start);
        if start <= line && line <= end {
            if enclosing.as_ref().map(|(s, _)| start > *s).unwrap_or(true) {
                enclosing = Some((start, name));
            }
        }
    }
    enclosing.map(|(_, n)| n)
}

fn pickaxe_payload(pattern: &str, repo_root: &Path) -> Result<Value> {
    let commits = pickaxe_commits(pattern, repo_root, PICKAXE_COMMITS)?;
    let mut annotated: Vec<Value> = Vec::new();
    for commit in &commits {
        let mut entries: Vec<Value> = Vec::new();
        for path in &commit.files {
            let line = commit_line_for_pattern(&commit.sha, path, pattern, repo_root);
            let symbol = match line {
                Some(l) => enclosing_symbol(&commit.sha, path, l, repo_root),
                None => None,
            };
            entries.push(json!({
                "path": path,
                "line": line,
                "enclosing_symbol": symbol,
            }));
        }
        annotated.push(json!({
            "sha": commit.short_sha,
            "date": commit.date,
            "author": commit.author,
            "subject": commit.subject,
            "matches": entries,
        }));
    }
    Ok(json!({
        "mode": "contains",
        "pattern": pattern,
        "commit_count": annotated.len(),
        "commits": annotated,
    }))
}

fn render_pickaxe(p: &Value) {
    println!("Pattern: {}", p["pattern"].as_str().unwrap_or(""));
    println!(
        "Commits introducing or removing the pattern: {}",
        p["commit_count"].as_i64().unwrap_or(0)
    );
    println!();
    for commit in p["commits"].as_array().cloned().unwrap_or_default() {
        println!(
            "{} {} {}: {}",
            commit["sha"].as_str().unwrap_or(""),
            commit["date"].as_str().unwrap_or(""),
            commit["author"].as_str().unwrap_or(""),
            commit["subject"].as_str().unwrap_or("")
        );
        for m in commit["matches"].as_array().cloned().unwrap_or_default() {
            let line_part = match m["line"].as_i64() {
                Some(l) => format!("L{l}"),
                None => "L?".to_string(),
            };
            let symbol_part = match m["enclosing_symbol"].as_str() {
                Some(s) => format!(" [in {s}]"),
                None => String::new(),
            };
            println!(
                "  {:<7} {}{}",
                line_part,
                m["path"].as_str().unwrap_or(""),
                symbol_part
            );
        }
        println!();
    }
}

// ---------- entry point ----------

pub fn run(
    file: Option<&Path>,
    symbol: Option<&str>,
    contains: Option<&str>,
    as_json: bool,
) -> Result<Value> {
    if let Some(pattern) = contains {
        if file.is_some() || symbol.is_some() {
            bail!("--contains is mutually exclusive with <file>/<symbol> arguments.");
        }
        let repo_root = cache::repo_root_for(Path::new("."));
        let payload = pickaxe_payload(pattern, &repo_root)?;
        if !as_json {
            render_pickaxe(&payload);
        }
        return Ok(payload);
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
    let repo_root = cache::repo_root_for(&file_path);

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
        return Ok(payload);
    }

    let payload = whole_file_payload(&file_path, &repo_root);
    if !as_json {
        render_whole_file(&payload);
    }
    Ok(payload)
}
