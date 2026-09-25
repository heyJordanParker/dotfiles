//! Path-argument validation.
//!
//! A missing or wrong-type path is rejected with a non-zero exit (2,
//! matching `read`/`history`) and a clear error before any work runs.
//! The contract is non-zero + a real error, never exit 0 + fabricated
//! output.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

static MISSING: AtomicBool = AtomicBool::new(false);

/// Every file the path arguments name: a file as itself, a directory as the
/// files under it from the repository's own listing, so ignored files,
/// nested repositories and linked worktrees stay out. A path that does not
/// exist is reported and skipped, the way ripgrep reports one and searches
/// the rest; `missing` then turns the run's exit status to 2.
pub fn files_under(paths: &[PathBuf], arg: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for path in paths {
        if !path.exists() {
            report(path, arg, "does not exist");
            continue;
        }
        if path.is_file() {
            out.push(path.clone());
            continue;
        }
        let directory = crate::cache::absolutize(path);
        let mut files = crate::cache::worktree_root_for(&directory)
            .and_then(|root| crate::repo_files::tracked_paths(&root, Some(&directory)))
            .unwrap_or_else(|| crate::repo_files::walk_files(&directory));
        files.sort();
        out.extend(files);
    }
    out
}

/// Whether a path argument named something that does not exist.
pub fn missing() -> bool {
    MISSING.load(Ordering::Relaxed)
}

/// Path must exist (file or directory).
pub fn require_exists(path: &Path, arg: &str) {
    if !path.exists() {
        fail(path, arg, "does not exist");
    }
}

/// Path must exist AND be a directory (used by `list`).
pub fn require_dir(path: &Path, arg: &str) {
    if !path.exists() {
        fail(path, arg, "does not exist");
    }
    if !path.is_dir() {
        fail(path, arg, "is a file");
    }
}

fn fail(path: &Path, arg: &str, why: &str) -> ! {
    report(path, arg, why);
    std::process::exit(2);
}

/// Name a bad path argument on stderr and let the run go on; the run's exit
/// status is 2 when it ends (see `missing`).
pub fn report(path: &Path, arg: &str, why: &str) {
    // "Invalid value for 'ARG': 'path' why" — a single recognizable
    // diagnostic class for every bad path argument.
    eprintln!(
        "Error: Invalid value for '{}': '{}' {}",
        arg,
        path.display(),
        why
    );
    MISSING.store(true, Ordering::Relaxed);
}
