//! `trace find` — filename-pattern search with code-intelligence enrichment.
//! Replaces `find <dir> -type f -name "*.ext"` with one enriched call:
//! matching paths annotated with complexity rank + the lifecycle shoulder.
//! Respects .gitignore inside a git repo (git ls-files), SKIP_DIRS walk
//! otherwise.

use super::glob_match::fnmatch;
use crate::{cache, file_facts, passive_context};
use anyhow::Result;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

const SKIP_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    ".venv",
    "venv",
    "__pycache__",
    "dist",
    "build",
    ".next",
    ".tracer-cache",
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
    "vendor",
    "worktrees",
    ".lando",
    "playwright-report",
    "test-results",
];

/// SKIP_DIRS- and hidden-dir-bounded filesystem walk (the non-git fallback).
fn walk(base: &Path, include_dirs: bool) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let walker = walkdir::WalkDir::new(base)
        .into_iter()
        .filter_entry(|e| {
            if e.file_type().is_dir() && e.path() != base {
                let name = e.file_name().to_string_lossy();
                !SKIP_DIRS.contains(&name.as_ref()) && !name.starts_with('.')
            } else {
                true
            }
        });
    for entry in walker.flatten() {
        if entry.path() == base {
            continue;
        }
        if entry.file_type().is_file() {
            out.push(entry.path().to_path_buf());
        } else if include_dirs && entry.file_type().is_dir() {
            out.push(entry.path().to_path_buf());
        }
    }
    out
}

/// Candidate files: the shared `git ls-files` enumeration scoped to `base`
/// (deleted-in-index paths already excluded), falling back to the SKIP_DIRS
/// walk. `include_dirs` synthesizes the parent directories of the matched
/// files, stopping at `base`.
fn list_files(repo_root: &Path, base: &Path, include_dirs: bool) -> Vec<PathBuf> {
    match crate::repo_files::tracked_paths(repo_root, Some(base)) {
        Some(files) => {
            if include_dirs {
                let mut dirs: BTreeSet<PathBuf> = BTreeSet::new();
                for f in &files {
                    let mut cur = f.parent();
                    while let Some(p) = cur {
                        if p == base {
                            break;
                        }
                        dirs.insert(p.to_path_buf());
                        cur = p.parent();
                    }
                }
                let mut result = files;
                result.extend(dirs);
                result
            } else {
                files
            }
        }
        None => walk(base, include_dirs),
    }
}

/// True when a path's basename matches `pattern`, passes `path_filter`,
/// and is not excluded.
fn matches(
    path: &Path,
    pattern: &str,
    path_filter: Option<&str>,
    excludes: &[String],
) -> bool {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    if !fnmatch(&name, pattern) {
        return false;
    }
    let full = path.to_string_lossy();
    if let Some(pf) = path_filter {
        if !fnmatch(&full, pf) {
            return false;
        }
    }
    for ex in excludes {
        if fnmatch(&full, ex) {
            return false;
        }
    }
    true
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    pattern: &str,
    base: &str,
    path_filter: Option<String>,
    excludes: Vec<String>,
    type_filter: String,
    limit: usize,
    sort: String,
    as_json: bool,
) -> Result<Value> {
    let abs = cache::absolutize(Path::new(base));
    if !abs.exists() {
        eprintln!("Error: {base} does not exist");
        std::process::exit(2);
    }
    // Canonicalized base path, printed verbatim in output.
    let base_abs = abs.canonicalize().unwrap_or(abs);
    let base_path = base_abs.clone();
    let repo_root = cache::worktree_root_for(&base_abs).unwrap_or_else(|| cache::display_root(&base_abs));
    let include_dirs = type_filter.to_lowercase() == "d";
    let candidates = list_files(&repo_root, &base_abs, include_dirs);

    let candidates: Vec<PathBuf> = candidates
        .into_iter()
        .filter(|p| if include_dirs { p.is_dir() } else { p.is_file() })
        .collect();

    let matched: Vec<PathBuf> = candidates
        .into_iter()
        .filter(|p| matches(p, pattern, path_filter.as_deref(), &excludes))
        .collect();

    let root_resolved = repo_root
        .canonicalize()
        .unwrap_or_else(|_| repo_root.clone());

    // (path, kind, ccn_total, ccn_rank, shoulder, last_modified)
    struct E {
        path: String,
        kind: &'static str,
        ccn_total: i64,
        ccn_rank: Option<String>,
        shoulder: Option<String>,
        last_modified: Option<String>,
    }
    // SINGLE batch over all file matches (was per-match file_facts::get —
    // each a cold read+hash+extract+write; same defect class as the old
    // `list` per-file loop). get_batch hoists bulk maps once with the
    // in-memory mtime fast-path. No lite-facts: real extraction preserved,
    // just batched.
    let file_matches: Vec<std::path::PathBuf> = if include_dirs {
        Vec::new()
    } else {
        matched.iter().cloned().collect()
    };
    let facts_map = if file_matches.is_empty() {
        std::collections::HashMap::new()
    } else {
        file_facts::get_batch(&file_matches, &repo_root)
    };

    let mut entries: Vec<E> = Vec::new();
    for path in &matched {
        // Relative path against the UNRESOLVED repo root — symlinked dirs
        // keep their tracked name and are not collapsed onto their target.
        let relative = path
            .strip_prefix(&root_resolved)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| path.to_string_lossy().to_string());
        if include_dirs {
            entries.push(E {
                path: relative,
                kind: "directory",
                ccn_total: 0,
                ccn_rank: None,
                shoulder: None,
                last_modified: None,
            });
            continue;
        }
        let fkey = cache::relative_to_root(path, &repo_root);
        match facts_map.get(&fkey) {
            None => entries.push(E {
                path: relative,
                kind: "file",
                ccn_total: 0,
                ccn_rank: Some("unknown".to_string()),
                shoulder: None,
                last_modified: None,
            }),
            Some(f) => entries.push(E {
                path: relative,
                kind: "file",
                ccn_total: f.cyclomatic_complexity_total,
                ccn_rank: Some(f.rank.clone()),
                shoulder: Some(passive_context::render_compact(f)),
                last_modified: f.last_modified.clone(),
            }),
        }
    }

    match sort.as_str() {
        "complexity" => {
            entries.sort_by(|a, b| {
                (-(a.ccn_total), &a.path).cmp(&(-(b.ccn_total), &b.path))
            });
        }
        "recent" => {
            entries.sort_by(|a, b| {
                let ka = (
                    a.last_modified.clone().unwrap_or_default(),
                    a.path.clone(),
                );
                let kb = (
                    b.last_modified.clone().unwrap_or_default(),
                    b.path.clone(),
                );
                kb.cmp(&ka)
            });
        }
        _ => entries.sort_by(|a, b| a.path.cmp(&b.path)),
    }

    let truncated = entries.len() > limit;
    entries.truncate(limit);

    let json_entries: Vec<_> = entries
        .iter()
        .map(|e| {
            json!({
                "path": e.path,
                "kind": e.kind,
                "ccn_total": e.ccn_total,
                "ccn_rank": e.ccn_rank,
                "shoulder": e.shoulder,
                "last_modified": e.last_modified,
            })
        })
        .collect();
    let value = json!({
        "pattern": pattern,
        "base": base_path.to_string_lossy(),
        "path_filter": path_filter,
        "excludes": excludes,
        "type": type_filter,
        "match_count": json_entries.len(),
        "truncated": truncated,
        "entries": json_entries,
    });

    if as_json {
        return Ok(value);
    }

    if entries.is_empty() {
        println!("(no matches)");
        return Ok(value);
    }

    println!(
        "{} matches under {}:",
        entries.len(),
        base_path.to_string_lossy()
    );
    for e in &entries {
        if e.kind == "directory" {
            println!("  📁 {}/", e.path);
            continue;
        }
        println!(
            "  {}  [ccn={} {}] {}",
            e.path,
            e.ccn_total,
            e.ccn_rank.as_deref().unwrap_or(""),
            e.shoulder.as_deref().unwrap_or(""),
        );
    }
    if truncated {
        println!("... truncated to {limit} entries (raise with --limit)");
    }
    Ok(value)
}
