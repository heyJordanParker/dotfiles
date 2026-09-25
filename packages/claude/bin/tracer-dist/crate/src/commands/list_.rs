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
//! their own search scopes. `--recent` orders directories and files
//! newest-first by stat mtime. `--limit N` (opt-in, no default) keeps the
//! first N entries of both kinds after sorting, the way `ls -t | head` does,
//! and enrichment runs only on the rendered rows; `entries=N` always carries
//! the pre-limit total.
//!
//! Per-subdir aggregation is command-level glue over the `file_facts` +
//! `git_activity` APIs; it does not re-derive them.

use crate::{cache, file_facts, git_activity, repo_files, summary};
use anyhow::Result;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};
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
        .map(|e| crate::extraction::supported_extensions().contains(&e.to_lowercase().as_str()))
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
    /// The code group, when the extractor knows the file's language.
    code: Option<Map<String, Value>>,
    /// The git group, when git holds anything for the file.
    git: Option<Map<String, Value>>,
}

impl FileRow {
    /// The file's line in a listing: its code facts, then its git headline.
    fn headline(&self) -> Option<String> {
        let mut map = Map::new();
        if let Some(code) = &self.code {
            for key in ["lines", "cyclomatic_complexity", "complexity_rank"] {
                if let Some(value) = code.get(key) {
                    map.insert(key.into(), value.clone());
                }
            }
        }
        if let Some(git) = &self.git {
            for (key, value) in git {
                let key = if key == "status" { "git" } else { key.as_str() };
                if matches!(key, "git" | "commits_last_30_days" | "last_commit") {
                    map.insert(key.into(), value.clone());
                }
            }
        }
        (!map.is_empty()).then(|| crate::yamlfmt::flow(&Value::Object(map), false))
    }
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

/// Aggregate git signals over a tracked subtree. Complexity is projected into
/// the summary as each bounded facts chunk resolves.
fn aggregate(
    rels: &[String],
    git_map: &std::collections::HashMap<String, git_activity::GitActivity>,
) -> DirSummary {
    let mut last_modified: Option<String> = None;
    let mut has_uncommitted = false;

    for rel in rels {
        let activity = git_map.get(rel);
        let modified = activity.and_then(|a| a.last_modified.clone());
        let state = activity.and_then(|a| a.working_state.clone());
        if let Some(m) = &modified {
            if last_modified.as_ref().map(|lm| m > lm).unwrap_or(true) {
                last_modified = Some(m.clone());
            }
        }
        if state
            .as_deref()
            .map(|s| DIRTY_STATES.contains(&s))
            .unwrap_or(false)
        {
            has_uncommitted = true;
        }
    }

    DirSummary {
        file_count: rels.len(),
        ccn_total: 0,
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
    let rel_base_str = if rel_base.as_os_str().is_empty() || rel_base.to_string_lossy() == "." {
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
            by_subdir
                .entry(head.to_string())
                .or_default()
                .push(rel.clone());
        }
    }
    by_subdir
}

/// The git group from a bulk-map entry, in the facts' own keys. None when git
/// holds nothing for the file.
fn git_group(a: Option<&git_activity::GitActivity>) -> Option<Map<String, Value>> {
    a.filter(|a| a.commit_count != 0 || a.working_state.is_some())
        .map(summary::activity_git)
}

/// The code group from facts, in the facts' own keys. None when no language
/// resolved.
fn code_group(f: &file_facts::FileFacts) -> Option<Map<String, Value>> {
    let language = f.language.as_ref()?;
    let value = json!({
        "language": language,
        "lines": f.loc,
        "cyclomatic_complexity": f.cyclomatic_complexity_total,
        "max_function_complexity": f.cyclomatic_complexity_max,
        "functions": f.function_count,
        "complexity_rank": f.rank,
    });
    value.as_object().cloned()
}

pub fn run(paths: &[PathBuf], show_hidden: bool, recent: bool, limit: Option<usize>, as_json: bool) -> Result<Value> {
    if let [path] = paths {
        return one(path, show_hidden, recent, limit, as_json);
    }
    let share = crate::output::budget().map(|budget| budget / paths.len().max(1));
    let mut documents = Vec::with_capacity(paths.len());
    for path in paths {
        if !path.is_dir() {
            let why = if path.exists() { "is not a directory" } else { "does not exist" };
            crate::pathval::report(path, "PATH", why);
            continue;
        }
        if !as_json {
            println!("== {} ==", path.display());
        }
        documents.push(crate::output::within(share, || one(path, show_hidden, recent, limit, as_json))?);
    }
    Ok(crate::output::document(
        json!({"paths": paths}),
        json!({}),
        json!(documents),
        json!({"paths": documents.len()}),
    ))
}

fn one(path: &Path, show_hidden: bool, recent: bool, limit: Option<usize>, as_json: bool) -> Result<Value> {
    crate::pathval::require_dir(path, "PATH");
    // Canonicalize the displayed path (drops trailing `/.`, resolves
    // symlinks).
    let base = path
        .canonicalize()
        .unwrap_or_else(|_| cache::absolutize(path));

    // Row source: one readdir over base. Everything on disk is a row;
    // `.git` and skip-dirs are pruned; a nested checkout is a scope, not a
    // row set.
    let mut dir_stats: Vec<(String, i64)> = Vec::new(); // (name, mtime_ns)
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
            let mtime = md
                .as_ref()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_nanos() as i64)
                .unwrap_or(0);
            if is_dir {
                if name == ".git" || skip_dir(&name) {
                    continue;
                }
                if child.join(".git").exists() {
                    nested_repos.push(name);
                    continue;
                }
                dir_stats.push((name, mtime));
            } else if let Some(md) = md {
                file_stats.push((name, md.len() as i64, mtime));
            }
        }
    }
    nested_repos.sort();

    // Order, then bound, the way `ls -t | head` does: over directories and
    // files alike. Enrichment below touches only the rendered rows.
    if recent {
        dir_stats.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        file_stats.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(&b.0)));
    } else {
        dir_stats.sort_by_cached_key(|e| e.0.to_lowercase());
        file_stats.sort_by_cached_key(|e| e.0.to_lowercase());
    }
    let entries_total = dir_stats.len() + file_stats.len();
    let (dirs_kept, files_kept) = match limit {
        None => (dir_stats.len(), file_stats.len()),
        Some(n) if recent => {
            let mut stamps: Vec<(i64, bool)> = dir_stats
                .iter()
                .map(|dir| (dir.1, true))
                .chain(file_stats.iter().map(|file| (file.2, false)))
                .collect();
            stamps.sort_by(|a, b| b.cmp(a));
            let dirs = stamps.iter().take(n).filter(|(_, is_dir)| *is_dir).count();
            (dirs, n.min(entries_total) - dirs)
        }
        Some(n) => (dir_stats.len().min(n), n - dir_stats.len().min(n)),
    };
    let dir_names: Vec<String> = dir_stats.iter().take(dirs_kept).map(|e| e.0.clone()).collect();
    let rendered: Vec<(String, i64, i64)> = file_stats.iter().take(files_kept).cloned().collect();

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
                    .filter(|e| show_hidden || !e.file_name().to_string_lossy().starts_with('.'))
                    .count()
            })
            .unwrap_or(0)
    };

    match &worktree {
        Some(root) => {
            let scc = crate::repo_context::metrics(root);
            let tracked = repo_files::tracked_files(root, Some(&base)).unwrap_or_default();
            let git_paths: Vec<String> = tracked.iter().cloned().collect();
            let git_map = git_activity::for_paths(root, &git_paths);
            let by_subdir = partition_under_base(root, &base, &tracked);

            let rel_of = |name: &str| -> String { cache::relative_to_root(&base.join(name), root) };

            let mut dir_index_by_rel: HashMap<String, usize> = HashMap::new();
            for name in &dir_names {
                let tracked = by_subdir.get(name);
                let index = dirs_out.len();
                if let Some(paths) = tracked {
                    for rel in paths {
                        dir_index_by_rel.insert(rel.clone(), index);
                    }
                }
                dirs_out.push(DirRow {
                    name: name.clone(),
                    child_count: child_count_of(name),
                    // An ignored dir has no tracked paths and stays None —
                    // its row carries the disk's child count alone.
                    tracked: tracked.map(|paths| aggregate(paths, &git_map)),
                });
            }
            let mut direct_index_by_rel: HashMap<String, usize> = HashMap::new();
            for (name, size, mtime) in &rendered {
                let rel = rel_of(name);
                let activity = git_map.get(&rel);
                direct_index_by_rel.insert(rel.clone(), files_out.len());
                files_out.push(FileRow {
                    name: name.clone(),
                    size_bytes: *size,
                    mtime_ns: *mtime,
                    rel: Some(rel.clone()),
                    git: git_group(activity),
                    code: None,
                });
            }

            // Resolve at most one shared chunk at a time, then immediately
            // project its facts into directory summaries or direct rows.
            let mut batch_rels: BTreeSet<String> = tracked
                .iter()
                .filter(|rel| is_source_ext(rel))
                .cloned()
                .collect();
            for (name, _, _) in &rendered {
                let rel = rel_of(name);
                if is_source_ext(name) || scc.per_file.get(&rel).is_some() {
                    batch_rels.insert(rel);
                }
            }
            let batch_rels: Vec<String> = batch_rels.into_iter().collect();
            for rels in batch_rels.chunks(file_facts::RESOLVE_CHUNK) {
                let paths: Vec<PathBuf> = rels.iter().map(|rel| root.join(rel)).collect();
                let facts = file_facts::get_batch(&paths, root);
                for (rel, fact) in facts {
                    if let Some(index) = dir_index_by_rel.get(&rel) {
                        if let Some(summary) = dirs_out[*index].tracked.as_mut() {
                            summary.ccn_total += fact.cyclomatic_complexity_total;
                        }
                    }
                    if let Some(index) = direct_index_by_rel.get(&rel) {
                        files_out[*index].code = code_group(&fact);
                    }
                }
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
                    code: None,
                    git: None,
                });
            }
        }
    }

    let dirs_json: Vec<Value> = dirs_out
        .iter()
        .map(|d| {
            let mut v = json!({
                "name": d.name,
                "child_count": d.child_count,
            });
            if let Some(s) = &d.tracked {
                v["tracked_files"] = json!(s.file_count);
                v["cyclomatic_complexity"] = json!(s.ccn_total);
                v["last_commit"] = json!(s.last_modified.as_deref().and_then(summary::age));
                v["uncommitted"] = json!(s.has_uncommitted);
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
                "code": f.code,
                "git": f.git,
            })
        })
        .collect();
    let value = crate::output::document(
        json!({"path": base.to_string_lossy(), "limit": limit}),
        json!({"nested_repos": nested_repos}),
        json!({"directories": dirs_json, "files": files_json}),
        json!({
            "entries": entries_total,
            "limited": limit.map(|n| entries_total > n).unwrap_or(false),
        }),
    );

    if as_json {
        return Ok(value);
    }

    let mut rows: Vec<crate::output::Entry> = Vec::with_capacity(dirs_out.len() + files_out.len());
    let mut names: Vec<String> = Vec::with_capacity(rows.capacity());
    for (d, json) in dirs_out.iter().zip(&dirs_json) {
        let mut facts = json.as_object().cloned().unwrap_or_default();
        facts.remove("name");
        if d.tracked.as_ref().is_some_and(|s| !s.has_uncommitted) {
            facts.remove("uncommitted");
        }
        if facts.get("last_commit").is_some_and(Value::is_null) {
            facts.remove("last_commit");
        }
        let bare = format!("  \u{1F4C1} {}/", d.name);
        rows.push(crate::output::Entry {
            rank: -(rows.len() as i64),
            levels: vec![format!("{bare}  {}", crate::yamlfmt::flow(&Value::Object(facts), false)), bare],
        });
        names.push(format!("{}/", d.name));
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
        if let Some(headline) = f.headline() {
            line.push_str(&format!("  {headline}"));
        }
        rows.push(crate::output::Entry {
            rank: -(rows.len() as i64),
            levels: vec![line.trim_end().to_string(), format!("  {}", f.name)],
        });
        names.push(f.name.clone());
    }

    let mut footer = match limit {
        Some(n) if entries_total > n => format!("entries={entries_total} (showing {n})"),
        _ => format!("entries={entries_total}"),
    };
    for r in &nested_repos {
        footer.push_str(&format!("\nnested repository (its own search scope): {r}"));
    }
    let root = format!("{}/", base.to_string_lossy());
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    let fixed = root.len() + 1 + footer.len() + 1 + crate::output::closing_room(rows.len(), "entries");
    let (texts, shortened) = crate::output::fit_listing(&rows, &names, fixed);
    println!("{root}");
    for text in texts {
        println!("{text}");
    }
    println!("{footer}");
    if shortened > 0 {
        println!("{}", crate::output::shortened_line(shortened, rows.len(), "entries"));
    }
    Ok(value)
}
