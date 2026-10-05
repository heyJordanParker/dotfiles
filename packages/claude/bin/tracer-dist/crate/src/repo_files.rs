//! Repo file enumeration. The single source of truth for "which files does
//! the repo contain" — every file-listing command routes through here so
//! they agree on the deletion policy (a path in git's index but absent from
//! disk is excluded).
//! `tracked_files` (git ls-files, repo-root-relative) and `tracked_paths`
//! (the same set as absolute paths) plus `walk_files` (SKIP_DIRS-bounded
//! walk) for the non-git fallback.

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::fs;
use std::ops::Deref;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

const LISTING: &str = "tracked_files_v1";

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Stamp {
    pub mtime: i64,
    pub size: u64,
    pub ctime: i64,
    pub inode: u64,
}

#[derive(Default)]
pub(crate) struct TrackedFiles {
    pub paths: Vec<String>,
    pub stamps: Vec<Stamp>,
    pub tracked: Vec<bool>,
    pub symlinks: Vec<bool>,
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct StoredTrackedFiles {
    paths: Vec<String>,
    tracked: Vec<bool>,
    directories: Vec<(String, Stamp)>,
    ignored: Vec<(String, Stamp)>,
    index: Option<Stamp>,
}

impl Deref for TrackedFiles {
    type Target = Vec<String>;

    fn deref(&self) -> &Self::Target {
        &self.paths
    }
}

impl FromIterator<(String, Stamp, bool, bool)> for TrackedFiles {
    fn from_iter<I: IntoIterator<Item = (String, Stamp, bool, bool)>>(rows: I) -> Self {
        let mut files = TrackedFiles::default();
        for (path, stamp, tracked, symlink) in rows {
            files.paths.push(path);
            files.stamps.push(stamp);
            files.tracked.push(tracked);
            files.symlinks.push(symlink);
        }
        files
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
            .iter()
            .map(|index| paths.stamps[*index].clone())
            .collect(),
        tracked: selected.iter().map(|index| paths.tracked[*index]).collect(),
        symlinks: selected
            .iter()
            .map(|index| paths.symlinks[*index])
            .collect(),
    }))
}

pub(crate) fn tracked_set(repo_root: &Path) -> Option<Arc<HashSet<String>>> {
    tracked_set_state(repo_root).map(|set| Arc::clone(&set.tracked))
}

pub(crate) struct TrackedSet {
    pub tracked: Arc<HashSet<String>>,
    pub untracked: Arc<HashSet<String>>,
    pub discovered: bool,
}

pub(crate) fn tracked_set_state(repo_root: &Path) -> Option<Arc<TrackedSet>> {
    static MEMO: crate::memo::Memo<Option<Arc<TrackedSet>>> = OnceLock::new();
    crate::memo::get_or_build(&MEMO, repo_root, || {
        if let Some(cached) = fresh_listing(repo_root) {
            let (tracked, untracked) = split_tracked(&cached.paths, &cached.tracked);
            return Some(Arc::new(TrackedSet {
                tracked,
                untracked,
                discovered: false,
            }));
        }
        tracked_files(repo_root, None).map(|files| {
            let (tracked, untracked) = split_tracked(&files.paths, &files.tracked);
            Arc::new(TrackedSet {
                tracked,
                untracked,
                discovered: true,
            })
        })
    })
    .as_ref()
    .as_ref()
    .map(Arc::clone)
}

/// The names among one directory's `entries`, a sub-directory suffixed `/`,
/// that git does not ignore: a file the repository lists, or a directory
/// holding one. `directory` is the prefix its repository-relative paths
/// share, empty for the root. None when the repository cannot be listed.
pub(crate) fn unignored(repo_root: &Path, directory: &str, entries: &[String]) -> Option<Vec<String>> {
    let listing = tracked_set_state(repo_root)?;
    let held: HashSet<&str> = listing
        .tracked
        .iter()
        .chain(listing.untracked.iter())
        .filter_map(|path| path.strip_prefix(directory))
        .filter_map(|below| below.split('/').next())
        .collect();
    Some(
        entries
            .iter()
            .filter(|entry| held.contains(entry.trim_end_matches('/')))
            .cloned()
            .collect(),
    )
}

fn split_tracked(
    paths: &[String],
    statuses: &[bool],
) -> (Arc<HashSet<String>>, Arc<HashSet<String>>) {
    let mut tracked = HashSet::new();
    let mut untracked = HashSet::new();
    for (path, status) in paths.iter().zip(statuses) {
        if *status {
            tracked.insert(path.clone());
        } else {
            untracked.insert(path.clone());
        }
    }
    (Arc::new(tracked), Arc::new(untracked))
}

fn fresh_listing(repo_root: &Path) -> Option<StoredTrackedFiles> {
    crate::cache::load_bytes(crate::cache::NAMESPACE_FILE, LISTING, repo_root)
        .and_then(|bytes| serde_json::from_slice::<StoredTrackedFiles>(&bytes).ok())
        .filter(|stored| listing_is_fresh(stored, repo_root))
}

pub(crate) fn stamped_files_uncached(repo_root: &Path) -> Option<TrackedFiles> {
    let index = ListingIndex {
        repo_root,
        discovered: RefCell::new(None),
        failed: Cell::new(false),
    };
    let stored = crate::cache::index(&index, repo_root).filter(|_| !index.failed.get())?;
    Some(
        index
            .discovered
            .take()
            .unwrap_or_else(|| stamp_paths(repo_root, &stored.paths, &stored.tracked)),
    )
}

struct ListingIndex<'a> {
    repo_root: &'a Path,
    discovered: RefCell<Option<TrackedFiles>>,
    failed: Cell<bool>,
}

impl crate::cache::Index for ListingIndex<'_> {
    type Stored = StoredTrackedFiles;
    type Change = ();

    fn key(&self) -> String {
        LISTING.to_string()
    }

    fn read(&self, bytes: &[u8]) -> Option<StoredTrackedFiles> {
        serde_json::from_slice(bytes).ok()
    }

    fn change(&self, stored: Option<&StoredTrackedFiles>) -> Option<()> {
        (!stored.is_some_and(|stored| listing_is_fresh(stored, self.repo_root))).then_some(())
    }

    fn apply(&self, _stored: Option<StoredTrackedFiles>, _change: ()) -> StoredTrackedFiles {
        let Some(listed) = discover_files(self.repo_root) else {
            self.failed.set(true);
            return StoredTrackedFiles::default();
        };
        let (directories, ignored) = watched_paths(self.repo_root, &listed.paths);
        let stored = StoredTrackedFiles {
            paths: listed.paths.clone(),
            tracked: listed.tracked.clone(),
            directories,
            ignored,
            index: crate::cache::git_dir(self.repo_root).and_then(|git_dir| stamp(&git_dir.join("index"))),
        };
        self.discovered.replace(Some(listed));
        stored
    }

    fn complete(&self, _stored: &StoredTrackedFiles) -> bool {
        !self.failed.get()
    }
}

fn listing_is_fresh(stored: &StoredTrackedFiles, repo_root: &Path) -> bool {
    stored.paths.len() == stored.tracked.len()
        && stamps_are_fresh(repo_root, &stored.directories, false)
        && stamps_are_fresh(repo_root, &stored.ignored, true)
        && crate::cache::git_dir(repo_root).and_then(|git_dir| stamp(&git_dir.join("index")))
            == stored.index
}

fn stamps_are_fresh(repo_root: &Path, paths: &[(String, Stamp)], ignored: bool) -> bool {
    in_workers(paths, |part| {
        part.iter().all(|(path, saved)| {
            let observed = if ignored {
                ignore_stamp(repo_root, path)
            } else {
                stamp(&repo_root.join(path))
            };
            observed.as_ref() == Some(saved)
        })
    })
    .into_iter()
    .all(|fresh| fresh)
}

fn in_workers<T: Sync, R: Send>(items: &[T], work: impl Fn(&[T]) -> R + Sync) -> Vec<R> {
    let size = items.len().div_ceil(rayon::current_num_threads().max(1)).max(1);
    std::thread::scope(|scope| {
        let workers: Vec<_> = items.chunks(size).map(|part| scope.spawn(|| work(part))).collect();
        workers.into_iter().map(|worker| worker.join().expect("stamping worker panicked")).collect()
    })
}

fn ignore_stamp(repo_root: &Path, path: &str) -> Option<Stamp> {
    if path == ".git/info/exclude" {
        return crate::cache::git_dir(repo_root)
            .and_then(|git_dir| stamp(&git_dir.join("info/exclude")));
    }
    stamp(&repo_root.join(path))
}

fn watched_paths(
    repo_root: &Path,
    paths: &[String],
) -> (Vec<(String, Stamp)>, Vec<(String, Stamp)>) {
    let mut directories = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut ignored = Vec::new();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_nanos()).ok())
        .unwrap_or(i64::MAX);
    let settled = |directory: &Path| {
        stamp(directory).map(|mut saved| {
            if saved.mtime >= now.saturating_sub(2_000_000_000) {
                saved.mtime = 0;
            }
            saved
        })
    };
    let walker = ignore::WalkBuilder::new(repo_root)
        .hidden(false)
        .git_ignore(true)
        .git_global(false)
        .git_exclude(true)
        .build();
    for entry in walker.flatten() {
        let path = entry.path();
        if path == repo_root.join(".git") || path.starts_with(repo_root.join(".tracer-cache")) {
            continue;
        }
        let Ok(relative) = path.strip_prefix(repo_root) else {
            continue;
        };
        let relative = relative.to_string_lossy().to_string();
        if entry.file_type().is_some_and(|kind| kind.is_dir()) {
            if let Some(saved) = settled(path) {
                seen.insert(relative.clone());
                directories.push((relative, saved));
            }
        } else if matches!(
            path.file_name().and_then(|name| name.to_str()),
            Some(".gitignore" | ".ignore" | ".sccignore")
        ) {
            if let Some(saved) = stamp(path) {
                ignored.push((relative, saved));
            }
        }
    }
    if let Some(git_dir) = crate::cache::git_dir(repo_root) {
        let exclude = git_dir.join("info/exclude");
        if let Some(saved) = stamp(&exclude) {
            ignored.push((format!(".git/{}", "info/exclude"), saved));
        }
    }
    for path in paths {
        let mut parent = Path::new(path).parent();
        while let Some(directory) = parent {
            let relative = directory.to_string_lossy().to_string();
            if !seen.contains(&relative) {
                if let Some(saved) = settled(&repo_root.join(directory)) {
                    seen.insert(relative.clone());
                    directories.push((relative, saved));
                }
            }
            parent = directory.parent();
        }
    }
    (directories, ignored)
}

fn stamped_path(path: &Path) -> Option<(Stamp, bool)> {
    let metadata = fs::symlink_metadata(path).ok()?;
    Some((
        Stamp {
            mtime: metadata
                .mtime()
                .saturating_mul(1_000_000_000)
                .saturating_add(metadata.mtime_nsec()),
            size: metadata.len(),
            ctime: metadata
                .ctime()
                .saturating_mul(1_000_000_000)
                .saturating_add(metadata.ctime_nsec()),
            inode: metadata.ino(),
        },
        metadata.file_type().is_symlink(),
    ))
}

fn stamp(path: &Path) -> Option<Stamp> {
    stamped_path(path).map(|(stamp, _)| stamp)
}

fn discover_files(repo_root: &Path) -> Option<TrackedFiles> {
    // The index listing and the porcelain status are two git subprocesses
    // that read the same index and never wait on each other.
    let (out, working) = rayon::join(
        || crate::git_activity::git_output(repo_root, ["ls-files", "--cached", "-z"]),
        || crate::git_activity::working_paths(repo_root),
    );
    let out = out.ok()?;
    if !out.status.success() {
        return None;
    }
    let tracked: HashSet<String> = out
        .stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| String::from_utf8_lossy(path).into_owned())
        .collect();
    let mut paths = tracked.clone();
    paths.extend(working?);
    let mut paths: Vec<String> = paths
        .into_iter()
        .filter(|path| !is_tracer_cache(path))
        .collect();
    paths.sort();
    let flags: Vec<bool> = paths.iter().map(|path| tracked.contains(path)).collect();
    Some(stamp_paths(repo_root, &paths, &flags))
}

fn stamp_paths(repo_root: &Path, paths: &[String], tracked: &[bool]) -> TrackedFiles {
    let rows: Vec<(&String, &bool)> = paths.iter().zip(tracked).collect();
    in_workers(&rows, |part| {
        part.iter()
            .filter_map(|&(path, &tracked)| {
                let (stamp, symlink) = stamped_path(&repo_root.join(path))?;
                Some((path.clone(), stamp, tracked, symlink))
            })
            .collect::<Vec<_>>()
    })
    .into_iter()
    .flatten()
    .collect()
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
            e.depth() == 0 || (!skip.contains(name.as_ref()) && !name.starts_with('.'))
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
