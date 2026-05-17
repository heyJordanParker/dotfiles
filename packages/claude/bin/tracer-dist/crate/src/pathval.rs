//! Path-argument validation.
//!
//! A missing or wrong-type path is rejected with a non-zero exit (2,
//! matching `read`/`history`) and a clear error before any work runs.
//! The contract is non-zero + a real error, never exit 0 + fabricated
//! output.

use std::path::Path;

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

/// Path must exist AND be a file (used by `structure`, `symbols`, `blame`).
pub fn require_file(path: &Path, arg: &str) {
    if !path.exists() {
        fail(path, arg, "does not exist");
    }
    if !path.is_file() {
        fail(path, arg, "is a directory");
    }
}

fn fail(path: &Path, arg: &str, why: &str) -> ! {
    // "Invalid value for 'ARG': 'path' why" — a single recognizable
    // diagnostic class for every bad path argument.
    eprintln!(
        "Error: Invalid value for '{}': '{}' {}",
        arg,
        path.display(),
        why
    );
    std::process::exit(2);
}
