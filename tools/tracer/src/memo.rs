//! One per-repository memo, shared by every whole-repo map.
//!
//! Five maps are built at most once per process and keyed by repository root:
//! the relations index, the git-activity map, the porcelain status, the scc
//! payload, and the mtime index. Each had its own copy of the same twelve
//! lines, and each copy had to get the same two things right — hand out a
//! shared handle rather than a clone, and hold the lock across the build so
//! concurrent callers serialize onto one run instead of racing the same git
//! subprocesses. Getting that wrong is silent: the map is still correct, it
//! is just built twice and copied per caller.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

/// The memo a caller declares as its `static`.
pub type Memo<T> = OnceLock<Mutex<HashMap<PathBuf, Arc<T>>>>;

/// The memoized value for `repo_root`, building it under the lock on a miss.
///
/// The lock is deliberately held across `build`: these builds shell out to
/// git, and two threads racing the same `git status` contend on `index.lock`
/// rather than doing half the work each.
pub fn get_or_build<T>(memo: &Memo<T>, repo_root: &Path, build: impl FnOnce() -> T) -> Arc<T> {
    let cell = memo.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = cell.lock().unwrap();
    if let Some(hit) = guard.get(repo_root) {
        return Arc::clone(hit);
    }
    let built = Arc::new(build());
    guard.insert(repo_root.to_path_buf(), Arc::clone(&built));
    built
}

/// Replace the memoized value for `repo_root`, under the same lock.
///
/// The mtime index is the one map that is written during a run as well as
/// read; storing the new value under this lock means a later read in the same
/// process can never serve the superseded index.
pub fn replace<T>(memo: &Memo<T>, repo_root: &Path, value: T) -> Arc<T> {
    let cell = memo.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = cell.lock().unwrap();
    let stored = Arc::new(value);
    guard.insert(repo_root.to_path_buf(), Arc::clone(&stored));
    stored
}
