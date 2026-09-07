//! Repo file enumeration. The single source of truth for "which files does
//! the repo contain" — every file-listing command routes through here so
//! they agree on the deletion policy (a path in git's index but absent from
//! disk is excluded).
//! `tracked_files` (git ls-files, repo-root-relative) and `tracked_paths`
//! (the same set as absolute paths) plus `walk_files` (SKIP_DIRS-bounded
//! walk) for the non-git fallback.

use std::collections::HashSet;
use std::fs;
use std::ops::Deref;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, OnceLock};
use std::thread;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Stamp {
    pub mode: u32,
    pub size: u64,
    pub mtime: i64,
    pub mtime_nsec: i64,
    pub ctime: i64,
    pub ctime_nsec: i64,
    pub inode: u64,
}

#[derive(Default)]
pub(crate) struct TrackedFiles {
    pub paths: Vec<String>,
    pub stamps: Vec<Stamp>,
}

impl Deref for TrackedFiles {
    type Target = Vec<String>;

    fn deref(&self) -> &Self::Target {
        &self.paths
    }
}

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
pub fn tracked_files(repo_root: &Path, base: Option<&Path>) -> Option<Arc<TrackedFiles>> {
    static MEMO: crate::memo::Memo<Option<Arc<TrackedFiles>>> = OnceLock::new();
    let base_rel = base.and_then(|b| {
        b.canonicalize()
            .ok()?
            .strip_prefix(repo_root.canonicalize().ok()?)
            .ok()
            .map(Path::to_path_buf)
    });
    let paths = crate::memo::get_or_build(&MEMO, repo_root, || {
        stamped_files_uncached(repo_root).map(Arc::new)
    });
    let paths = paths.as_ref().as_ref()?;
    if base.is_none() {
        return Some(Arc::clone(paths));
    }
    let selected: Vec<usize> = paths
        .iter()
        .enumerate()
        .filter_map(|(index, path)| {
            base_rel
                .as_ref()
                .map_or(true, |base| {
                    base.as_os_str().is_empty() || Path::new(path).starts_with(base)
                })
                .then_some(index)
        })
        .collect();
    Some(Arc::new(TrackedFiles {
        paths: selected.iter().map(|index| paths[*index].clone()).collect(),
        stamps: selected
            .into_iter()
            .map(|index| paths.stamps[index].clone())
            .collect(),
    }))
}

pub(crate) fn stamped_files_uncached(repo_root: &Path) -> Option<TrackedFiles> {
    let args = ["ls-files", "--cached", "-z"];
    let out = Command::new("git")
        .args(args)
        .current_dir(repo_root)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let mut paths: HashSet<String> = out
        .stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| String::from_utf8_lossy(path).into_owned())
        .collect();
    paths.extend(crate::git_activity::working_paths(repo_root)?);
    let mut paths: Vec<String> = paths
        .into_iter()
        .filter(|path| !is_tracer_cache(path))
        .collect();
    paths.sort();
    let worker_count = thread::available_parallelism().ok()?.get().min(paths.len());
    if worker_count == 0 {
        return Some(TrackedFiles::default());
    }
    let stamped = thread::scope(|scope| {
        let mut remaining = paths.into_iter();
        let mut workers = Vec::with_capacity(worker_count);
        for worker_index in 0..worker_count {
            let partition_size = remaining.len().div_ceil(worker_count - worker_index);
            let partition = remaining.by_ref().take(partition_size).collect::<Vec<_>>();
            let worker = thread::Builder::new()
                .spawn_scoped(scope, move || {
                    partition
                        .into_iter()
                        .filter_map(|path| {
                            let metadata = fs::symlink_metadata(repo_root.join(&path)).ok()?;
                            Some((
                                path,
                                Stamp {
                                    mode: metadata.mode(),
                                    size: metadata.len(),
                                    mtime: metadata.mtime(),
                                    mtime_nsec: metadata.mtime_nsec(),
                                    ctime: metadata.ctime(),
                                    ctime_nsec: metadata.ctime_nsec(),
                                    inode: metadata.ino(),
                                },
                            ))
                        })
                        .collect::<Vec<_>>()
                })
                .ok()?;
            workers.push(worker);
        }
        let mut stamped = Vec::new();
        for worker in workers {
            match worker.join() {
                Ok(partition) => stamped.extend(partition),
                Err(panic) => std::panic::resume_unwind(panic),
            }
        }
        Some(stamped)
    })?;
    let (paths, stamps) = stamped.into_iter().unzip();
    Some(TrackedFiles { paths, stamps })
}

/// `.tracer-cache/` is tracer's own state, and `--others` reports it in any
/// repo that has not gitignored it — so a search for `*.json` returned the
/// cache entries the search itself had just written. The filesystem walk
/// already prunes it through `skip_dirs`; this is the same rule on the git
/// enumeration, so both universes agree.
fn is_tracer_cache(relative: &str) -> bool {
    relative
        .split('/')
        .any(|segment| segment == crate::cache::CACHE_DIR_NAME)
}

/// The same set as `tracked_files`, returned as absolute paths joined onto
/// `repo_root` — the shape `find` and the relations index need.
/// Routes through `tracked_files` so the deletion policy lives in exactly
/// one place. None when git is unavailable or `base` is outside `repo_root`.
pub fn tracked_paths(repo_root: &Path, base: Option<&Path>) -> Option<Vec<PathBuf>> {
    tracked_files(repo_root, base).map(|rels| rels.iter().map(|r| repo_root.join(r)).collect())
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
    let mut walker = walkdir::WalkDir::new(base).into_iter().filter_entry(|e| {
        if !e.file_type().is_dir() {
            return false;
        }
        if e.depth() == 0 {
            return true;
        }
        let name = e.file_name().to_string_lossy();
        !skip.contains(name.as_ref()) && !name.starts_with('.')
    });
    while let Some(result) = walker.next() {
        if let Ok(entry) = result {
            if entry.depth() > 0 && entry.path().join(".git").exists() {
                found.push(entry.path().to_path_buf());
                walker.skip_current_dir();
            }
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

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn nested_repo_scan_continues_after_a_walk_error() {
        let root = std::env::temp_dir().join(format!(
            "trace-nested-error-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let blocked = root.join("a-blocked");
        let visible = root.join("z-visible");
        std::fs::create_dir_all(&blocked).unwrap();
        std::fs::write(blocked.join("child"), "blocked").unwrap();
        std::fs::create_dir_all(visible.join(".git")).unwrap();
        std::fs::set_permissions(&blocked, std::fs::Permissions::from_mode(0o000)).unwrap();

        let found = nested_repos(&root);

        std::fs::set_permissions(&blocked, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::remove_dir_all(&root).unwrap();
        assert_eq!(
            found,
            vec![visible],
            "a walk error stopped later sibling discovery"
        );
    }
}
