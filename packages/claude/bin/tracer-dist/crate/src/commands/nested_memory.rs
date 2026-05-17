//! Nested Claude.md loading on file-read — command-local helper.
//! Consumed only by `trace read` (for the `nested_memories` payload field
//! and the rendered header block); not a foundation module, so it lives
//! next to its single caller. Replicates Claude Code's Claude.md walk-up
//! that a subprocess `read` would otherwise bypass.
//!
//! Limits matched: @include depth cap (5), pass + session dedupe by resolved
//! path, visited-dir tracking in rules recursion, nested traversal bounded
//! to file dir..repo_root, external @include blocked, 40k-char large warning.

use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

const MAX_INCLUDE_DEPTH: i32 = 5;
const LARGE_MEMORY_THRESHOLD: usize = 40_000;

#[derive(Clone)]
pub struct LoadedMemory {
    pub relative_path: String,
    pub kind: String,
    pub content: String,
    pub size: usize,
    pub large: bool,
    pub path: String,
}

fn resolve(p: &Path) -> Option<PathBuf> {
    p.canonicalize().ok()
}

/// Instruction files for a path, ordered root -> target. `directory_mode`
/// skips path-conditional rules.
pub fn load_for_file(
    file_path: &Path,
    repo_root: &Path,
    session_dedupe: &mut BTreeSet<String>,
    directory_mode: bool,
) -> Vec<LoadedMemory> {
    let file_path = match resolve(file_path) {
        Some(p) => p,
        None => return vec![],
    };
    let repo_root = match resolve(repo_root) {
        Some(p) => p,
        None => return vec![],
    };
    if file_path.strip_prefix(&repo_root).is_err() {
        return vec![];
    }
    let target_dir = if file_path.is_file() {
        file_path.parent().unwrap_or(&file_path).to_path_buf()
    } else {
        file_path.clone()
    };

    // Chain: repo_root down to target_dir inclusive.
    let mut chain: Vec<PathBuf> = Vec::new();
    let mut current = target_dir.clone();
    loop {
        if current.strip_prefix(&repo_root).is_err() {
            break;
        }
        chain.push(current.clone());
        if current == repo_root || current.parent() == Some(current.as_path()) {
            break;
        }
        match current.parent() {
            Some(p) => current = p.to_path_buf(),
            None => break,
        }
    }
    chain.reverse();

    let mut pass_dedupe: BTreeSet<String> = BTreeSet::new();
    let mut results: Vec<LoadedMemory> = Vec::new();

    for directory in &chain {
        let candidates: [(PathBuf, &str); 6] = [
            (directory.join("CLAUDE.md"), "claude_md"),
            (directory.join("Claude.md"), "claude_md"),
            (directory.join(".claude").join("CLAUDE.md"), "claude_md"),
            (directory.join(".claude").join("Claude.md"), "claude_md"),
            (directory.join("CLAUDE.local.md"), "local_md"),
            (directory.join("Claude.local.md"), "local_md"),
        ];
        for (candidate, kind) in &candidates {
            if let Some(mem) = try_load(
                candidate,
                &repo_root,
                kind,
                &mut pass_dedupe,
                session_dedupe,
            ) {
                let inc = load_includes(
                    Path::new(&mem.path),
                    &mem.content,
                    &repo_root,
                    &mut pass_dedupe,
                    session_dedupe,
                    0,
                );
                results.push(mem);
                results.extend(inc);
            }
        }

        let rules_dir = directory.join(".claude").join("rules");
        if rules_dir.is_dir() {
            for rule_mem in scan_rules_dir(
                &rules_dir,
                &repo_root,
                &file_path,
                &mut pass_dedupe,
                session_dedupe,
                false,
                directory_mode,
            ) {
                let inc = load_includes(
                    Path::new(&rule_mem.path),
                    &rule_mem.content,
                    &repo_root,
                    &mut pass_dedupe,
                    session_dedupe,
                    0,
                );
                results.push(rule_mem);
                results.extend(inc);
            }
        }
    }

    if !directory_mode {
        if let Some(home) = home_dir() {
            let user_rules = home.join(".claude").join("rules");
            if user_rules.is_dir() {
                for rule_mem in scan_rules_dir(
                    &user_rules,
                    &repo_root,
                    &file_path,
                    &mut pass_dedupe,
                    session_dedupe,
                    true,
                    false,
                ) {
                    let inc = load_includes(
                        Path::new(&rule_mem.path),
                        &rule_mem.content,
                        &repo_root,
                        &mut pass_dedupe,
                        session_dedupe,
                        0,
                    );
                    results.push(rule_mem);
                    results.extend(inc);
                }
            }
        }
    }

    results
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn try_load(
    path: &Path,
    repo_root: &Path,
    kind: &str,
    pass_dedupe: &mut BTreeSet<String>,
    session_dedupe: &mut BTreeSet<String>,
) -> Option<LoadedMemory> {
    // Dedupe key + displayed path come from the file's real on-disk
    // identity (canonicalize). On a case-insensitive filesystem this
    // collapses `CLAUDE.md` / `Claude.md` probes of the same physical file
    // to one entry (no double-print); on a case-sensitive filesystem two
    // genuinely distinct files keep distinct keys and both still load.
    let resolved = resolve(path)?;
    if !resolved.is_file() {
        return None;
    }
    let normalized = resolved.to_string_lossy().to_string();
    if pass_dedupe.contains(&normalized) || session_dedupe.contains(&normalized) {
        return None;
    }
    let content = fs::read_to_string(&resolved).ok()?;
    if content.trim().is_empty() {
        return None;
    }
    pass_dedupe.insert(normalized.clone());
    session_dedupe.insert(normalized.clone());

    let relative = match resolved.strip_prefix(repo_root) {
        Ok(r) => r.to_string_lossy().to_string(),
        Err(_) => match home_dir().and_then(|h| {
            resolved
                .strip_prefix(&h)
                .ok()
                .map(|r| format!("~/{}", r.to_string_lossy()))
        }) {
            Some(r) => r,
            None => normalized.clone(),
        },
    };
    let size = content.chars().count();
    Some(LoadedMemory {
        relative_path: relative,
        kind: kind.to_string(),
        large: size >= LARGE_MEMORY_THRESHOLD,
        size,
        content,
        path: normalized,
    })
}

#[allow(clippy::too_many_arguments)]
fn scan_rules_dir(
    rules_dir: &Path,
    repo_root: &Path,
    file_path: &Path,
    pass_dedupe: &mut BTreeSet<String>,
    session_dedupe: &mut BTreeSet<String>,
    conditional_only: bool,
    unconditional_only: bool,
) -> Vec<LoadedMemory> {
    let mut results: Vec<LoadedMemory> = Vec::new();
    let mut visited_dirs: BTreeSet<String> = BTreeSet::new();
    walk_rules(
        rules_dir,
        repo_root,
        file_path,
        pass_dedupe,
        session_dedupe,
        conditional_only,
        unconditional_only,
        &mut visited_dirs,
        &mut results,
    );
    results
}

#[allow(clippy::too_many_arguments)]
fn walk_rules(
    d: &Path,
    repo_root: &Path,
    file_path: &Path,
    pass_dedupe: &mut BTreeSet<String>,
    session_dedupe: &mut BTreeSet<String>,
    conditional_only: bool,
    unconditional_only: bool,
    visited_dirs: &mut BTreeSet<String>,
    results: &mut Vec<LoadedMemory>,
) {
    let d_real = match resolve(d) {
        Some(p) => p.to_string_lossy().to_string(),
        None => return,
    };
    if visited_dirs.contains(&d_real) {
        return;
    }
    visited_dirs.insert(d_real);

    let mut entries: Vec<PathBuf> = match fs::read_dir(d) {
        Ok(rd) => rd.flatten().map(|e| e.path()).collect(),
        Err(_) => return,
    };
    entries.sort();

    for entry in entries {
        let is_symlink = entry
            .symlink_metadata()
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false);
        if is_symlink {
            if let Some(t) = resolve(&entry) {
                if visited_dirs.contains(&t.to_string_lossy().to_string()) {
                    continue;
                }
            }
        }
        if entry.is_dir() {
            walk_rules(
                &entry,
                repo_root,
                file_path,
                pass_dedupe,
                session_dedupe,
                conditional_only,
                unconditional_only,
                visited_dirs,
                results,
            );
        } else if entry.is_file()
            && entry.extension().and_then(|e| e.to_str()) == Some("md")
        {
            let preview = match fs::read_to_string(&entry) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let paths_globs = extract_paths_frontmatter(&preview);
            let kind = match &paths_globs {
                Some(globs) => {
                    if unconditional_only {
                        continue;
                    }
                    if !matches_paths(file_path, globs, repo_root) {
                        continue;
                    }
                    "rules_conditional"
                }
                None => {
                    if conditional_only {
                        continue;
                    }
                    "rules_unconditional"
                }
            };
            if let Some(mem) = try_load(
                &entry,
                repo_root,
                kind,
                pass_dedupe,
                session_dedupe,
            ) {
                results.push(mem);
            }
        }
    }
}

/// The `paths:` globs from a file's YAML frontmatter. None when absent.
fn extract_paths_frontmatter(content: &str) -> Option<Vec<String>> {
    if !content.starts_with("---") {
        return None;
    }
    let lines: Vec<&str> = content.split('\n').collect();
    if lines.len() < 2 {
        return None;
    }
    let mut end: Option<usize> = None;
    for (i, l) in lines.iter().enumerate().take(lines.len().min(100)).skip(1) {
        if l.trim_end() == "---" {
            end = Some(i);
            break;
        }
    }
    let end = end?;

    let mut paths: Vec<String> = Vec::new();
    let mut capturing_list = false;
    for ln in &lines[1..end] {
        let stripped = ln.trim_end();
        if stripped.is_empty() {
            if capturing_list {
                break;
            }
            continue;
        }
        let lstripped = stripped.trim_start();
        if let Some(rest) = parse_paths_key(lstripped) {
            let rest = rest.trim();
            if !rest.is_empty() {
                return Some(vec![rest.trim_matches(|c| c == '"' || c == '\'').to_string()]);
            }
            capturing_list = true;
            continue;
        }
        if capturing_list {
            let ls = ln.trim_start();
            if let Some(item) = ls.strip_prefix("- ") {
                paths.push(item.trim().trim_matches(|c| c == '"' || c == '\'').to_string());
            } else if ls.starts_with('#') {
                continue;
            } else {
                break;
            }
        }
    }
    if paths.is_empty() {
        None
    } else {
        Some(paths)
    }
}

/// `^paths\s*:\s*(.*)$` applied to the lstripped line.
fn parse_paths_key(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("paths")?;
    let rest = rest.trim_start();
    let rest = rest.strip_prefix(':')?;
    Some(rest.trim_start())
}

/// True when `file_path` matches any glob, using fnmatch semantics with
/// the `**/` special-case.
fn matches_paths(file_path: &Path, globs: &[String], repo_root: &Path) -> bool {
    let relative = match file_path.strip_prefix(repo_root) {
        Ok(r) => r.to_string_lossy().to_string(),
        Err(_) => file_path.to_string_lossy().to_string(),
    };
    let name = file_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    for glob in globs {
        if fnmatch(&relative, glob) {
            return true;
        }
        if let Some(rest) = glob.strip_prefix("**/") {
            if fnmatch(&relative, rest) {
                return true;
            }
        }
        if fnmatch(&name, glob) {
            return true;
        }
        if !glob.contains('/') && fnmatch(&name, glob) {
            return true;
        }
    }
    false
}

/// `fnmatch`: case-normalized shell-style match. Supports `*`, `?`,
/// `[seq]`, `[!seq]`. `*` treats `/` like any other char (no path-segment
/// special-casing).
fn fnmatch(name: &str, pattern: &str) -> bool {
    fn matches(n: &[char], p: &[char]) -> bool {
        if p.is_empty() {
            return n.is_empty();
        }
        match p[0] {
            '*' => {
                if matches(n, &p[1..]) {
                    return true;
                }
                if !n.is_empty() {
                    return matches(&n[1..], p);
                }
                false
            }
            '?' => !n.is_empty() && matches(&n[1..], &p[1..]),
            '[' => {
                if n.is_empty() {
                    return false;
                }
                if let Some(close) = p.iter().position(|&c| c == ']').filter(|&i| i > 1) {
                    let mut set = &p[1..close];
                    let negate = set.first() == Some(&'!');
                    if negate {
                        set = &set[1..];
                    }
                    let mut hit = false;
                    let mut i = 0;
                    while i < set.len() {
                        if i + 2 < set.len() && set[i + 1] == '-' {
                            if n[0] >= set[i] && n[0] <= set[i + 2] {
                                hit = true;
                            }
                            i += 3;
                        } else {
                            if n[0] == set[i] {
                                hit = true;
                            }
                            i += 1;
                        }
                    }
                    if hit != negate {
                        return matches(&n[1..], &p[close + 1..]);
                    }
                    false
                } else {
                    n[0] == '[' && matches(&n[1..], &p[1..])
                }
            }
            c => !n.is_empty() && n[0] == c && matches(&n[1..], &p[1..]),
        }
    }
    let n: Vec<char> = name.to_lowercase().chars().collect();
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    matches(&n, &p)
}

fn load_includes(
    including_file: &Path,
    content: &str,
    repo_root: &Path,
    pass_dedupe: &mut BTreeSet<String>,
    session_dedupe: &mut BTreeSet<String>,
    depth: i32,
) -> Vec<LoadedMemory> {
    if depth >= MAX_INCLUDE_DEPTH {
        return vec![];
    }
    let mut results: Vec<LoadedMemory> = Vec::new();
    let mut in_fence = false;
    let parent = including_file.parent().unwrap_or(Path::new("."));
    for ln in content.split('\n') {
        let ls = ln.trim_start();
        if ls.starts_with("```") || ls.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let cleaned = strip_inline_code(ln);
        for refpath in find_includes(&cleaned) {
            let included = match resolve(&parent.join(&refpath)) {
                Some(p) => p,
                None => continue,
            };
            if included.strip_prefix(repo_root).is_err() {
                continue;
            }
            if let Some(mem) = try_load(
                &included,
                repo_root,
                "include",
                pass_dedupe,
                session_dedupe,
            ) {
                let inc = load_includes(
                    &included,
                    &mem.content,
                    repo_root,
                    pass_dedupe,
                    session_dedupe,
                    depth + 1,
                );
                results.push(mem);
                results.extend(inc);
            }
        }
    }
    results
}

/// Remove `` `...` `` inline-code spans from a line.
fn strip_inline_code(line: &str) -> String {
    let mut out = String::new();
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '`' {
            // consume until the next backtick; an unbalanced trailing
            // backtick is left in place.
            let mut span = String::from('`');
            let mut closed = false;
            for d in chars.by_ref() {
                span.push(d);
                if d == '`' {
                    closed = true;
                    break;
                }
            }
            if !closed {
                out.push_str(&span);
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// `@include\s+(\S+)` finditer.
fn find_includes(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes: Vec<char> = line.chars().collect();
    let needle: Vec<char> = "@include".chars().collect();
    let mut i = 0;
    while i + needle.len() <= bytes.len() {
        if bytes[i..i + needle.len()] == needle[..] {
            let mut j = i + needle.len();
            // require at least one whitespace
            if j < bytes.len() && bytes[j].is_whitespace() {
                while j < bytes.len() && bytes[j].is_whitespace() {
                    j += 1;
                }
                let mut token = String::new();
                while j < bytes.len() && !bytes[j].is_whitespace() {
                    token.push(bytes[j]);
                    j += 1;
                }
                if !token.is_empty() {
                    out.push(token);
                }
                i = j;
                continue;
            }
        }
        i += 1;
    }
    out
}

// ---------- session dedupe ----------

fn session_id() -> Option<String> {
    std::env::var("CLAUDE_CODE_SESSION_ID")
        .ok()
        .or_else(|| std::env::var("CLAUDE_SESSION_ID").ok())
        .or_else(|| std::env::var("TRACER_SESSION_ID").ok())
}

fn session_state_path(session_id: &str) -> Option<PathBuf> {
    home_dir().map(|h| {
        h.join(".tracer-cache")
            .join("sessions")
            .join(session_id)
            .join("loaded-memories.txt")
    })
}

pub fn load_session_dedupe() -> BTreeSet<String> {
    let sid = match session_id() {
        Some(s) => s,
        None => return BTreeSet::new(),
    };
    let path = match session_state_path(&sid) {
        Some(p) => p,
        None => return BTreeSet::new(),
    };
    if !path.is_file() {
        return BTreeSet::new();
    }
    fs::read_to_string(&path)
        .map(|t| {
            t.split('\n')
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

pub fn save_session_dedupe(dedupe: &BTreeSet<String>) {
    let sid = match session_id() {
        Some(s) => s,
        None => return,
    };
    let path = match session_state_path(&sid) {
        Some(p) => p,
        None => return,
    };
    let parent = match path.parent() {
        Some(p) => p,
        None => return,
    };
    if fs::create_dir_all(parent).is_err() {
        return;
    }

    // Hold an exclusive advisory lock on `<parent>/.lock` across the entire
    // re-read + merge + atomic-replace, then release it. Concurrent
    // same-session `trace read` invocations race this read-modify-write,
    // and without the lock a writer's dedupe additions can be lost between
    // the re-read and the rename. Lock acquisition failure is non-fatal —
    // lock errors are swallowed and the read proceeds, never failing.
    let lock_path = parent.join(".lock");
    let lock_fh = match fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&lock_path)
    {
        Ok(fh) => fh,
        Err(_) => return,
    };
    let _ = rustix::fs::flock(&lock_fh, rustix::fs::FlockOperation::LockExclusive);

    let mut merged: BTreeSet<String> = if path.is_file() {
        fs::read_to_string(&path)
            .map(|t| {
                t.split('\n')
                    .map(|l| l.trim().to_string())
                    .filter(|l| !l.is_empty())
                    .collect()
            })
            .unwrap_or_default()
    } else {
        BTreeSet::new()
    };
    for d in dedupe {
        merged.insert(d.clone());
    }
    let body = merged.into_iter().collect::<Vec<_>>().join("\n");
    if let Ok(mut tmp) = tempfile::Builder::new()
        .prefix(".loaded-memories.")
        .tempfile_in(parent)
    {
        if tmp.write_all(body.as_bytes()).is_ok() {
            let _ = tmp.persist(&path);
        }
    }

    let _ = rustix::fs::flock(&lock_fh, rustix::fs::FlockOperation::Unlock);
}

/// Render the nested-memory header block for `trace read` output.
pub fn render(memories: &[LoadedMemory]) -> String {
    if memories.is_empty() {
        return String::new();
    }
    let mut blocks: Vec<String> = Vec::new();
    for mem in memories {
        let marker = if mem.large { " [LARGE]" } else { "" };
        blocks.push(format!(
            "=== {} · {}{} ({} chars) ===",
            mem.relative_path, mem.kind, marker, mem.size
        ));
        blocks.push(mem.content.trim_end().to_string());
        blocks.push(String::new());
    }
    blocks.join("\n").trim_end().to_string()
}
