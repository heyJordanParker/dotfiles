//! `trace list` — one-level directory listing on a filesystem row source.
//!
//! Rows come from one `readdir` + one `stat` per entry, so the listing states
//! what is on disk — gitignored and untracked files included. Context joins
//! on as column groups, one group per source, and a group renders only when
//! its source resolves:
//!   identity (name, kind)       readdir      always
//!   stat (bytes, mtime)         stat         always
//!   code (loc, ccn, rank)       extraction   when the extractor or the repo
//!                                            metrics know the file's type
//!   git (state, age, 30d, owner) git history when the file resolves in a repo
//! No relevance logic: the source answering is the only switch.
//!
//! `.git` and nested checkouts are pruned; nested checkouts are named as
//! their own search scopes. `--recent` orders files newest-first by stat
//! mtime. `--limit N` (opt-in, no default) caps the file rows after sorting,
//! and enrichment runs only on the rendered rows; `entries=N` always carries
//! the pre-limit file total.
//!
//! Per-subdir aggregation is command-level glue over the `file_facts` +
//! `git_activity` APIs; it does not re-derive them.

use crate::{cache, file_facts, git_activity, passive_context, repo_files};
use anyhow::Result;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const DIRTY_STATES: [&str; 4] = ["untracked", "added", "modified", "renamed"];

fn skip_dir(name: &str) -> bool {
    repo_files::skip_dirs().contains(name)
}

/// True when `rel`'s lowercased extension is one of the supported source
/// extensions.
fn is_source_ext(rel: &str) -> bool {
    Path::new(rel)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            crate::extraction::supported_extensions().contains(&e.to_lowercase().as_str())
        })
        .unwrap_or(false)
}

/// `YYYY-MM-DD HH:MM` (UTC) from an epoch-nanosecond mtime. Civil-date
/// arithmetic (Howard Hinnant's `civil_from_days`), no chrono dependency.
fn fmt_mtime(mtime_ns: i64) -> String {
    if mtime_ns <= 0 {
        return String::new();
    }
    let secs = mtime_ns / 1_000_000_000;
    let days = secs.div_euclid(86400);
    let sod = secs.rem_euclid(86400);
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}",
        sod / 3600,
        (sod % 3600) / 60
    )
}

/// Human byte size: `64B`, `9.8KB`, `2.1MB`, `1.3GB`.
fn fmt_size(bytes: i64) -> String {
    let b = bytes as f64;
    if bytes < 1000 {
        format!("{bytes}B")
    } else if b < 1e6 {
        format!("{:.1}KB", b / 1e3)
    } else if b < 1e9 {
        format!("{:.1}MB", b / 1e6)
    } else {
        format!("{:.1}GB", b / 1e9)
    }
}

struct FileRow {
    name: String,
    size_bytes: i64,
    mtime_ns: i64,
    /// Repo-relative key when the listing sits inside a worktree.
    rel: Option<String>,
    facts: Option<file_facts::FileFacts>,
    git: Option<String>,
    git_json: Value,
}

struct DirRow {
    name: String,
    child_count: usize,
    /// Tracked-subtree aggregate; None outside a repo or for an ignored dir.
    tracked: Option<DirSummary>,
}

struct DirSummary {
    file_count: usize,
    ccn_total: i64,
    last_modified: Option<String>,
    has_uncommitted: bool,
}

/// Aggregate per-entry signals over a tracked subtree: complexity from the
/// per-file facts (only source files are batched, so the rest contribute 0),
/// git state from the bulk activity map that owns it.
fn aggregate(
    rels: &[String],
    git_map: &std::collections::HashMap<String, git_activity::GitActivity>,
    facts_map: &std::collections::HashMap<String, file_facts::FileFacts>,
) -> DirSummary {
    let mut ccn_total = 0i64;
    let mut last_modified: Option<String> = None;
    let mut has_uncommitted = false;

    for rel in rels {
        ccn_total += facts_map
            .get(rel)
            .map(|f| f.cyclomatic_complexity_total)
            .unwrap_or(0);
        let activity = git_map.get(rel);
        let modified = activity.and_then(|a| a.last_modified.clone());
        let state = activity.and_then(|a| a.working_state.clone());
        if let Some(m) = &modified {
            if last_modified.as_ref().map(|lm| m > lm).unwrap_or(true) {
                last_modified = Some(m.clone());
            }
        }
        if state.as_deref().map(|s| DIRTY_STATES.contains(&s)).unwrap_or(false) {
            has_uncommitted = true;
        }
    }

    DirSummary {
        file_count: rels.len(),
        ccn_total,
        last_modified,
        has_uncommitted,
    }
}

/// Split repo-root-relative paths into (subdir name → paths) and the
/// direct files at `base`.
fn partition_under_base(
    repo_root: &Path,
    base: &Path,
    rels: &[String],
) -> BTreeMap<String, Vec<String>> {
    let mut by_subdir: BTreeMap<String, Vec<String>> = BTreeMap::new();

    let root = repo_root
        .canonicalize()
        .unwrap_or_else(|_| repo_root.to_path_buf());
    let base_abs = base.canonicalize().unwrap_or_else(|_| base.to_path_buf());
    let rel_base = base_abs.strip_prefix(&root).unwrap_or(Path::new(""));
    let rel_base_str = if rel_base.as_os_str().is_empty()
        || rel_base.to_string_lossy() == "."
    {
        String::new()
    } else {
        format!("{}/", rel_base.to_string_lossy())
    };

    for rel in rels {
        if !rel_base_str.is_empty() && !rel.starts_with(&rel_base_str) {
            continue;
        }
        let under = &rel[rel_base_str.len()..];
        if let Some((head, _)) = under.split_once('/') {
            by_subdir.entry(head.to_string()).or_default().push(rel.clone());
        }
    }
    by_subdir
}

/// The git-history JSON group from a bulk-map entry. Null when git holds
/// nothing for the file.
fn git_json_group(a: Option<&git_activity::GitActivity>) -> Value {
    match a {
        Some(a) if a.commit_count != 0 || a.working_state.is_some() => json!({
            "state": a.working_state,
            "last_modified": a.last_modified,
            "first_seen": a.first_seen,
            "commit_count": a.commit_count,
            "commits_30d": a.commits_30d,
            "owner": a.top_author,
        }),
        _ => Value::Null,
    }
}

/// The code JSON group from facts. Null when no language resolved.
fn code_json_group(facts: Option<&file_facts::FileFacts>) -> Value {
    match facts {
        Some(f) if f.language.is_some() => json!({
            "language": f.language,
            "loc": f.loc,
            "ccn_total": f.cyclomatic_complexity_total,
            "ccn_max_function": f.cyclomatic_complexity_max,
            "function_count": f.function_count,
            "rank": f.rank,
        }),
        _ => Value::Null,
    }
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    path: &Path,
    show_hidden: bool,
    recent: bool,
    limit: Option<usize>,
    as_json: bool,
) -> Result<Value> {
    crate::pathval::require_dir(path, "PATH");
    // Canonicalize the displayed path (drops trailing `/.`, resolves
    // symlinks).
    let base = path
        .canonicalize()
        .unwrap_or_else(|_| cache::absolutize(path));

    // Row source: one readdir over base. Everything on disk is a row;
    // `.git` and skip-dirs are pruned; a nested checkout is a scope, not a
    // row set.
    let mut dir_names: Vec<String> = Vec::new();
    let mut file_stats: Vec<(String, i64, i64)> = Vec::new(); // (name, size, mtime_ns)
    let mut nested_repos: Vec<String> = Vec::new();
    if let Ok(rd) = fs::read_dir(&base) {
        for entry in rd.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') && !show_hidden {
                continue;
            }
            let child: PathBuf = entry.path();
            // stat follows symlinks so a linked file carries its target's
            // size and mtime; a broken link falls back to the link itself.
            let md = fs::metadata(&child)
                .or_else(|_| fs::symlink_metadata(&child))
                .ok();
            let is_dir = md.as_ref().map(|m| m.is_dir()).unwrap_or(false);
            if is_dir {
                if name == ".git" || skip_dir(&name) {
                    continue;
                }
                if child.join(".git").exists() {
                    nested_repos.push(name);
                    continue;
                }
                dir_names.push(name);
            } else if let Some(md) = md {
                let mtime = md
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_nanos() as i64)
                    .unwrap_or(0);
                file_stats.push((name, md.len() as i64, mtime));
            }
        }
    }
    nested_repos.sort();

    // Order, then bound. Enrichment below touches only the rendered rows.
    if recent {
        file_stats.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(&b.0)));
    } else {
        file_stats.sort_by_cached_key(|e| e.0.to_lowercase());
    }
    let entries_total = file_stats.len();
    let rendered: Vec<(String, i64, i64)> = match limit {
        Some(n) => file_stats.iter().take(n).cloned().collect(),
        None => file_stats.clone(),
    };

    // Context joins — only inside a worktree do git and code sources exist.
    let worktree = cache::worktree_root_for(&base);
    let mut dirs_out: Vec<DirRow> = Vec::new();
    let mut files_out: Vec<FileRow> = Vec::new();

    // Direct-children count per subdir: one readdir each, present for
    // artifact dirs the git universe cannot see. Counts what listing that
    // dir would show — the hidden filter applies here exactly as above.
    let child_count_of = |name: &str| -> usize {
        fs::read_dir(base.join(name))
            .map(|rd| {
                rd.flatten()
                    .filter(|e| {
                        show_hidden
                            || !e.file_name().to_string_lossy().starts_with('.')
                    })
                    .count()
            })
            .unwrap_or(0)
    };

    match &worktree {
        Some(root) => {
            let git_map = git_activity::bulk_cached(root);
            let scc_map = crate::repo_context::per_file_metrics(root);
            let tracked =
                repo_files::tracked_files(root, Some(&base)).unwrap_or_default();
            let by_subdir = partition_under_base(root, &base, &tracked);

            let rel_of = |name: &str| -> String {
                cache::relative_to_root(&base.join(name), root)
            };

            // ONE batch: the tracked source subtree (dir aggregates) plus the
            // rendered direct files whose type a source resolves — extraction
            // supported, or the repo metrics indexed it. Ephemeral artifacts
            // resolve in neither and stay stat-only, so nothing is extracted
            // or cached for them.
            let mut batch: Vec<PathBuf> = tracked
                .iter()
                .filter(|r| is_source_ext(r))
                .map(|r| root.join(r))
                .collect();
            for (name, _, _) in &rendered {
                let rel = rel_of(name);
                if is_source_ext(name) || scc_map.contains_key(&rel) {
                    batch.push(base.join(name));
                }
            }
            let facts_map = file_facts::get_batch(&batch, root);

            for name in &dir_names {
                dirs_out.push(DirRow {
                    name: name.clone(),
                    child_count: child_count_of(name),
                    // An ignored dir has no tracked paths and stays None —
                    // its row carries the disk's child count alone.
                    tracked: by_subdir
                        .get(name)
                        .map(|paths| aggregate(paths, &git_map, &facts_map)),
                });
            }
            for (name, size, mtime) in &rendered {
                let rel = rel_of(name);
                let facts = facts_map.get(&rel).cloned();
                let activity = git_map.get(&rel);
                files_out.push(FileRow {
                    name: name.clone(),
                    size_bytes: *size,
                    mtime_ns: *mtime,
                    rel: Some(rel.clone()),
                    git: activity.and_then(passive_context::git_group),
                    git_json: git_json_group(activity),
                    facts,
                });
            }
        }
        None => {
            for name in &dir_names {
                dirs_out.push(DirRow {
                    name: name.clone(),
                    child_count: child_count_of(name),
                    tracked: None,
                });
            }
            for (name, size, mtime) in &rendered {
                files_out.push(FileRow {
                    name: name.clone(),
                    size_bytes: *size,
                    mtime_ns: *mtime,
                    rel: None,
                    facts: None,
                    git: None,
                    git_json: Value::Null,
                });
            }
        }
    }
    dirs_out.sort_by_cached_key(|d| d.name.to_lowercase());

    let dirs_json: Vec<Value> = dirs_out
        .iter()
        .map(|d| {
            let mut v = json!({
                "name": d.name,
                "child_count": d.child_count,
            });
            if let Some(s) = &d.tracked {
                v["file_count"] = json!(s.file_count);
                v["ccn_total"] = json!(s.ccn_total);
                v["last_modified"] = json!(s.last_modified);
                v["has_uncommitted"] = json!(s.has_uncommitted);
            }
            v
        })
        .collect();
    let files_json: Vec<Value> = files_out
        .iter()
        .map(|f| {
            json!({
                "name": f.name,
                "rel": f.rel,
                "stat": {
                    "size_bytes": f.size_bytes,
                    "mtime_ns": f.mtime_ns,
                    "mtime": fmt_mtime(f.mtime_ns),
                },
                "code": code_json_group(f.facts.as_ref()),
                "git": f.git_json,
            })
        })
        .collect();
    let value = json!({
        "path": base.to_string_lossy(),
        "directories": dirs_json,
        "files": files_json,
        "entries": entries_total,
        "limited": limit.map(|n| entries_total > n).unwrap_or(false),
        "nested_repos": nested_repos,
    });

    if as_json {
        return Ok(value);
    }

    println!("{}/", base.to_string_lossy());
    for d in &dirs_out {
        let mut bits = vec![format!("{} entries", d.child_count)];
        if let Some(s) = &d.tracked {
            bits.push(format!("{} tracked files", s.file_count));
            bits.push(format!("ccn={}", s.ccn_total));
            if let Some(lm) = &s.last_modified {
                bits.push(format!("last: {lm}"));
            }
            if s.has_uncommitted {
                bits.push("uncommitted".to_string());
            }
        }
        println!("  \u{1F4C1} {}/  ({})", d.name, bits.join(" \u{00b7} "));
    }

    let name_width = files_out
        .iter()
        .map(|f| f.name.chars().count())
        .max()
        .unwrap_or(0)
        .min(48);
    for f in &files_out {
        let mut line = format!(
            "  {:<name_width$}  {:>8}  {}",
            f.name,
            fmt_size(f.size_bytes),
            fmt_mtime(f.mtime_ns),
        );
        if let Some(facts) = &f.facts {
            if facts.language.is_some() {
                line.push_str(&format!(
                    "  loc {} \u{00b7} ccn {} {}",
                    facts.loc, facts.cyclomatic_complexity_total, facts.rank
                ));
            }
        }
        if let Some(g) = &f.git {
            line.push_str(&format!("  [{g}]"));
        }
        println!("{}", line.trim_end());
    }

    match limit {
        Some(n) if entries_total > n => {
            println!("entries={entries_total} (showing {n})");
        }
        _ => println!("entries={entries_total}"),
    }
    for r in &nested_repos {
        println!("nested repository (its own search scope): {r}");
    }
    Ok(value)
}
