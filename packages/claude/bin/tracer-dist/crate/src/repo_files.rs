//! Repo file enumeration.
//! `tracked_files` (git ls-files) and `walk_files` (SKIP_DIRS-bounded walk).

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

/// git-tracked files under `base`. None when git is unavailable or `base`
/// is outside `repo_root`.
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
            .map(|l| l.to_string())
            .collect(),
    )
}

/// Filesystem walk under `base`, pruning SKIP_DIRS and hidden dirs/files.
pub fn walk_files(base: &Path) -> Vec<PathBuf> {
    let skip = skip_dirs();
    let mut out = Vec::new();
    let walker = walkdir::WalkDir::new(base).into_iter().filter_entry(|e| {
        let name = e.file_name().to_string_lossy();
        if e.file_type().is_dir() {
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
