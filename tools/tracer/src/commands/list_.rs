//! `trace list` — one-level annotated directory listing.
//! Files get a complexity rank + compact lifecycle shoulder;
//! sub-directories get file count, total CCN, and most-recent
//! last_modified across their contents. Discovery is a single
//! `git ls-files` partitioned by top-level segment under `base`
//! (falls back to a direct-children listing outside a git repo).
//!
//! Per-subdir aggregation is command-level glue: it consumes the
//! `file_facts` + `git_activity` APIs, it does not re-derive them.

use crate::{cache, file_facts, git_activity, passive_context, repo_files};
use anyhow::Result;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;

const DIRTY_STATES: [&str; 4] = ["untracked", "added", "modified", "renamed"];

fn skip_dir(name: &str) -> bool {
    repo_files::skip_dirs().contains(name)
}

fn rank_marker(rank: &str) -> &'static str {
    match rank {
        "low" => "·",
        "medium" => "•",
        "high" => "●",
        "critical" => "⚠",
        _ => "?",
    }
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

struct DirSummary {
    file_count: usize,
    ccn_total: i64,
    last_modified: Option<String>,
    has_uncommitted: bool,
}

/// Aggregate per-entry signals: source files pull real per-file facts
/// (warm cache); non-source files fall to the bulk git activity map with
/// ccn 0.
fn aggregate(
    rels: &[String],
    git_map: &std::collections::HashMap<String, git_activity::GitActivity>,
    facts_map: &std::collections::HashMap<String, file_facts::FileFacts>,
) -> DirSummary {
    let mut ccn_total = 0i64;
    let mut last_modified: Option<String> = None;
    let mut has_uncommitted = false;

    for rel in rels {
        let (ccn, modified, state) = if is_source_ext(rel) {
            match facts_map.get(rel) {
                Some(f) => (
                    f.cyclomatic_complexity_total,
                    f.last_modified.clone(),
                    f.working_state.clone(),
                ),
                None => {
                    let a = git_map
                        .get(rel)
                        .cloned()
                        .unwrap_or_else(git_activity::GitActivity::empty);
                    (0, a.last_modified, a.working_state)
                }
            }
        } else {
            let a = git_map
                .get(rel)
                .cloned()
                .unwrap_or_else(git_activity::GitActivity::empty);
            (0, a.last_modified, a.working_state)
        };
        ccn_total += ccn;
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
) -> (BTreeMap<String, Vec<String>>, Vec<String>) {
    let mut by_subdir: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut direct: Vec<String> = Vec::new();

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
        match under.split_once('/') {
            Some((head, _)) => {
                by_subdir.entry(head.to_string()).or_default().push(rel.clone());
            }
            None => direct.push(rel.clone()),
        }
    }
    (by_subdir, direct)
}

pub fn run(path: &Path, show_hidden: bool, as_json: bool) -> Result<Value> {
    crate::pathval::require_dir(path, "PATH");
    // Canonicalize the displayed path (drops trailing `/.`, resolves
    // symlinks).
    let base = path
        .canonicalize()
        .unwrap_or_else(|_| cache::absolutize(path));
    let repo_root = cache::repo_root_for(&base);
    let git_map = git_activity::bulk_cached(&repo_root);
    let tracked = repo_files::tracked_files(&repo_root, Some(&base));

    // (name, summary) for dirs; (name, Option<FileFacts>) for files.
    let mut dirs_out: Vec<(String, DirSummary)> = Vec::new();
    let mut files_out: Vec<(String, Option<file_facts::FileFacts>)> = Vec::new();

    match tracked {
        Some(rels) => {
            let (by_subdir, direct) = partition_under_base(&repo_root, &base, &rels);
            // SINGLE batch over every source file under base (subdir
            // aggregates + direct files). Replaces the O(N²) per-file
            // get() loop that caused the 62s blowup. No lite-facts:
            // get_batch does real extraction, just with bulk inputs
            // hoisted once and the mtime index written once.
            let all_source: Vec<std::path::PathBuf> = rels
                .iter()
                .filter(|r| is_source_ext(r))
                .map(|r| repo_root.join(r))
                .collect();
            let facts_map = file_facts::get_batch(&all_source, &repo_root);

            for (subdir_name, paths) in &by_subdir {
                if subdir_name.starts_with('.') && !show_hidden {
                    continue;
                }
                if skip_dir(subdir_name) {
                    continue;
                }
                dirs_out.push((
                    subdir_name.clone(),
                    aggregate(paths, &git_map, &facts_map),
                ));
            }
            let mut direct_sorted = direct.clone();
            direct_sorted.sort();
            for rel in &direct_sorted {
                let fname = Path::new(rel)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| rel.clone());
                if fname.starts_with('.') && !show_hidden {
                    continue;
                }
                // Hit the prebuilt map; fall back to single get() only for
                // a non-source direct file (rare, cheap, not in the map).
                let facts = facts_map.get(rel).cloned().or_else(|| {
                    file_facts::get(&base.join(&fname), &repo_root, None)
                });
                files_out.push((fname, facts));
            }
        }
        None => {
            let mut children: Vec<std::path::PathBuf> = match std::fs::read_dir(&base) {
                Ok(rd) => rd.flatten().map(|e| e.path()).collect(),
                Err(_) => Vec::new(),
            };
            // Sort key: directories first, then case-insensitive name.
            children.sort_by(|a, b| {
                let a_dir = a.is_dir();
                let b_dir = b.is_dir();
                (!a_dir, a.file_name().map(|n| n.to_string_lossy().to_lowercase()))
                    .cmp(&(!b_dir, b.file_name().map(|n| n.to_string_lossy().to_lowercase())))
            });
            for child in children {
                let name = match child.file_name() {
                    Some(n) => n.to_string_lossy().to_string(),
                    None => continue,
                };
                if name.starts_with('.') && !show_hidden {
                    continue;
                }
                if std::fs::symlink_metadata(&child)
                    .map(|m| m.file_type().is_symlink())
                    .unwrap_or(false)
                {
                    continue;
                }
                if child.is_dir() {
                    if skip_dir(&name) {
                        continue;
                    }
                    dirs_out.push((
                        name,
                        DirSummary {
                            file_count: 0,
                            ccn_total: 0,
                            last_modified: None,
                            has_uncommitted: false,
                        },
                    ));
                } else if child.is_file() {
                    let facts = file_facts::get(&child, &repo_root, None);
                    files_out.push((name, facts));
                }
            }
        }
    }

    let dirs_json: Vec<Value> = dirs_out
        .iter()
        .map(|(name, s)| {
            json!({
                "name": name,
                "file_count": s.file_count,
                "ccn_total": s.ccn_total,
                "last_modified": s.last_modified,
                "has_uncommitted": s.has_uncommitted,
            })
        })
        .collect();
    let files_json: Vec<Value> = files_out
        .iter()
        .map(|(name, facts)| match facts {
            Some(f) => json!({
                "name": name,
                "rank": f.rank,
                "ccn_total": f.cyclomatic_complexity_total,
                "passive_context": passive_context::render_compact(f),
            }),
            None => json!({
                "name": name,
                "rank": "unknown",
                "ccn_total": 0,
                "passive_context": Value::Null,
            }),
        })
        .collect();
    let value = json!({
        "path": base.to_string_lossy(),
        "directories": dirs_json,
        "files": files_json,
    });

    if as_json {
        return Ok(value);
    }

    println!("{}/", base.to_string_lossy());
    let mut dirs_sorted: Vec<&(String, DirSummary)> = dirs_out.iter().collect();
    dirs_sorted.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    for (name, s) in dirs_sorted {
        let mut bits = vec![
            format!("{} files", s.file_count),
            format!("ccn={}", s.ccn_total),
        ];
        if let Some(lm) = &s.last_modified {
            bits.push(format!("last: {lm}"));
        }
        if s.has_uncommitted {
            bits.push("uncommitted".to_string());
        }
        println!("  📁 {name}/  ({})", bits.join(" · "));
    }
    let mut files_sorted: Vec<&(String, Option<file_facts::FileFacts>)> =
        files_out.iter().collect();
    files_sorted.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    for (name, facts) in files_sorted {
        match facts {
            None => println!("  📄 {name}"),
            Some(f) => println!(
                "  {} {name}  [ccn={} {}] {}",
                rank_marker(&f.rank),
                f.cyclomatic_complexity_total,
                f.rank,
                passive_context::render_compact(f),
            ),
        }
    }
    Ok(value)
}
