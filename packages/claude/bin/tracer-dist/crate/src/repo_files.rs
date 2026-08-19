//! Repo file enumeration. The single source of truth for "which files does
//! the repo contain" — every file-listing command routes through here so
//! they agree on the deletion policy (a path in git's index but absent from
//! disk is excluded).
//! `tracked_files` (git ls-files, repo-root-relative) and `tracked_paths`
//! (the same set as absolute paths) plus `walk_files` (SKIP_DIRS-bounded
//! walk) for the non-git fallback.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn skip_dirs() -> HashSet<&'static str> {
    [
        ".git",
        ".next",
        ".tracer-cache",
        ".venv",
        "__pycache__",
        "build",
        "dist",
        "node_modules",
        "venv",
        "vendor",
    ]
    .into_iter()
    .collect()
}

/// git-tracked files under `base`, repo-root-relative. None when git is
/// unavailable or `base` is outside `repo_root`.
///
/// Deletion policy — the single source of truth every file-listing command
/// shares: a path in git's index but absent from disk (`rm`'d from the
/// working tree but never `git rm`'d) is excluded. `git ls-files` reports
/// the stale index entry; the on-disk existence check drops it so the
/// listing reflects the working tree, never git's index.
pub fn tracked_files(repo_root: &Path, base: Option<&Path>) -> Option<Vec<String>> {
    let mut args: Vec<String> = vec![
        "ls-files".into(),
        "--cached".into(),
        "--others".into(),
        "--exclude-standard".into(),
    ];
    if let Some(b) = base {
        let rel = b
            .canonicalize()
            .ok()?
            .strip_prefix(repo_root.canonicalize().ok()?)
            .ok()?
            .to_path_buf();
        if rel.to_string_lossy() != "." && !rel.as_os_str().is_empty() {
            args.push("--".into());
            args.push(rel.to_string_lossy().to_string());
        }
    }
    let out = Command::new("git")
        .args(&args)
        .current_dir(repo_root)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter(|l| !l.is_empty())
            .filter(|l| repo_root.join(l).exists())
            .map(|l| l.to_string())
            .collect(),
    )
}

/// The same set as `tracked_files`, returned as absolute paths joined onto
/// `repo_root` — the shape `find` / `glob` / the architecture graph need.
/// Routes through `tracked_files` so the deletion policy lives in exactly
/// one place. None when git is unavailable or `base` is outside `repo_root`.
pub fn tracked_paths(repo_root: &Path, base: Option<&Path>) -> Option<Vec<PathBuf>> {
    tracked_files(repo_root, base)
        .map(|rels| rels.into_iter().map(|r| repo_root.join(r)).collect())
}

/// Nested git checkouts strictly under `base` — directories carrying their
/// own `.git` entry (a directory for a checkout, a file for a linked
/// worktree). Each is its own scope: `tracked_files` never crosses into one
/// and `walk_files` prunes them, so a search that came up empty names them
/// instead of silently skipping a vendored repository. Only the outermost
/// nested root is reported; repos inside a reported repo belong to its scope.
pub fn nested_repos(base: &Path) -> Vec<PathBuf> {
    let skip = skip_dirs();
    let mut found: Vec<PathBuf> = Vec::new();
    let walker = walkdir::WalkDir::new(base).into_iter().filter_entry(|e| {
        if !e.file_type().is_dir() {
            return false;
        }
        if e.depth() == 0 {
            return true;
        }
        let name = e.file_name().to_string_lossy();
        !skip.contains(name.as_ref()) && !name.starts_with('.')
    });
    for entry in walker.flatten() {
        if entry.depth() > 0 && entry.path().join(".git").exists() {
            found.push(entry.path().to_path_buf());
        }
    }
    found.sort();
    let mut outermost: Vec<PathBuf> = Vec::new();
    for p in found {
        if !outermost.iter().any(|kept| p.starts_with(kept)) {
            outermost.push(p);
        }
    }
    outermost
}

/// `nested_repos` as base-relative strings — the shape the search commands
/// report on an empty result.
pub fn nested_repo_rels(base: &Path) -> Vec<String> {
    nested_repos(base)
        .iter()
        .map(|p| {
            p.strip_prefix(base)
                .unwrap_or(p)
                .to_string_lossy()
                .to_string()
        })
        .collect()
}

/// Filesystem walk under `base`, pruning SKIP_DIRS and hidden dirs/files.
pub fn walk_files(base: &Path) -> Vec<PathBuf> {
    let skip = skip_dirs();
    let mut out = Vec::new();
    let walker = walkdir::WalkDir::new(base).into_iter().filter_entry(|e| {
        let name = e.file_name().to_string_lossy();
        if e.file_type().is_dir() {
            // A nested repository is its own scope, never part of the parent's
            // file set — `.git` is a directory for a normal checkout and a file
            // for a linked worktree, so `exists` covers both.
            if e.depth() > 0 && e.path().join(".git").exists() {
                return false;
            }
            !skip.contains(name.as_ref()) && !name.starts_with('.')
        } else {
            true
        }
    });
    for entry in walker.flatten() {
        if entry.file_type().is_file() {
            let name = entry.file_name().to_string_lossy();
            if name.starts_with('.') {
                continue;
            }
            let p = entry.path();
            if p.is_symlink() {
                continue;
            }
            out.push(p.to_path_buf());
        }
    }
    out
}
