//! Worktree-anchored disk cache for the repo-state namespace.
//!
//! `.tracer-cache/file/schema{SCHEMA_VERSION}/{key}.json` at the worktree
//! root — the main-repo root for a normal checkout, or the linked-worktree's
//! own root for a `git worktree add` checkout. A tracer cache exists ONLY at
//! a worktree root. Outside any worktree, reads still return live results but
//! nothing persists — the `cache::save` chokepoint hard-gates on
//! `worktree_root_for` and no-ops when it returns `None`.
//!
//! Each schema's directory holds two kinds of entry, both JSON written as a
//! single line (no indent, the `jsonfmt` byte format) via `save`:
//! content-addressed per-file entries, which are immutable, and repo-wide
//! entries, each swept by `evict_prefixed` so a superseded key leaves nothing
//! behind. Another schema's directory stays until no build has written in it
//! for `stale_schema_age`. The second namespace, `sessions/`, lives under the
//! same `.tracer-cache/` at the worktree root and is owned by
//! `commands::session_log`.
//!
//! File cache key:
//!   sha256("v{SCHEMA_VERSION}|ccn:{backend}\0" + contents + "\0" + relpath)

use anyhow::Result;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const CACHE_DIR_NAME: &str = ".tracer-cache";
pub const NAMESPACE_FILE: &str = "file";

/// This schema's cross-process maintenance lock: `file/schema<N>/.maintain.lock`,
/// held with an exclusive `flock` across every repo-wide index update. A
/// process that finds its index fresh never takes it; one that finds it stale
/// takes it, re-reads the index the holder just wrote, and does only what is
/// left. Without it eight concurrent calls after one saved file each rebuilt
/// the index and overwrote each other's write. A build at another schema
/// writes none of these entries, so it holds its own lock and never waits here.
/// The process that takes the lock sweeps the stale schemas.
///
/// Reentrant per process: the relations update calls `get_batch`, which
/// stores the mtime index, which takes this lock again. `flock` on a second
/// descriptor in the same process would block forever, so the depth counter
/// hands a nested caller the lock the process already holds.
pub struct Maintenance;

#[allow(non_upper_case_globals)]
static maintenance: std::sync::Mutex<(usize, Option<fs::File>)> = std::sync::Mutex::new((0, None));

pub fn maintain(repo_root: &Path) -> Option<Maintenance> {
    let mut held = maintenance.lock().unwrap();
    if held.0 == 0 {
        let dir = schema_directory(NAMESPACE_FILE, repo_root).ok()?;
        let lock = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(dir.join(".maintain.lock"))
            .ok()?;
        crate::timing::phase("lock maintain", || {
            rustix::fs::flock(&lock, rustix::fs::FlockOperation::LockExclusive)
        })
        .ok()?;
        evict_stale_schemas(&dir);
        held.1 = Some(lock);
    }
    held.0 += 1;
    Some(Maintenance)
}

impl Drop for Maintenance {
    fn drop(&mut self) {
        let mut held = maintenance.lock().unwrap();
        held.0 -= 1;
        if held.0 == 0 {
            // Closing the descriptor releases the flock.
            held.1 = None;
        }
    }
}

/// Bump whenever extraction, the `FileFacts` shape, or a repo-wide index
/// shape changes — a new schema writes into its own directory, so old
/// entries become unreachable automatically.
pub const SCHEMA_VERSION: u32 = 49;

/// A build in use writes its schema's git-activity key at least once a day,
/// because that key carries the date, so a week without a write means no build
/// at that schema ran through a weekend and the days around it.
#[allow(non_upper_case_globals)]
const stale_schema_age: Duration = Duration::from_secs(7 * 24 * 60 * 60);

/// Active CCN backend. There is exactly one backend — the tree-sitter
/// AST decision-node walker — so cache identity is unconditionally
/// `ccn:ast`. This keeps cache keys stable for `TRACER_CCN_BACKEND=ast`,
/// so warm caches are interchangeable and key-for-key comparable.
pub fn active_ccn_backend() -> &'static str {
    "ast"
}

pub fn worktree_root_for(path: &Path) -> Option<PathBuf> {
    let mut current = cwd_of(path);
    loop {
        if current.file_name().is_some_and(|name| name == ".git") {
            return None;
        }
        let git = current.join(".git");
        if git.is_dir()
            || fs::read_to_string(&git)
                .ok()
                .is_some_and(|contents| contents.starts_with("gitdir:"))
        {
            return Some(current);
        }
        current = current.parent()?.to_path_buf();
    }
}

pub fn git_dir(root: &Path) -> Option<PathBuf> {
    let git = root.join(".git");
    if git.is_dir() {
        return Some(git);
    }
    let contents = fs::read_to_string(&git).ok()?;
    let target = contents.strip_prefix("gitdir:")?.trim();
    let target = PathBuf::from(target);
    Some(if target.is_absolute() {
        target
    } else {
        root.join(target)
    })
}

/// Non-persisting display root for read paths that need *some* base for
/// relative-path rendering when `path` lies outside any worktree. Never
/// pass this to `cache::save` — its hard-gate on `worktree_root_for`
/// already no-ops if you do, but the design contract is: read paths use
/// this; write paths gate on the worktree resolver themselves.
pub(crate) fn display_root(path: &Path) -> PathBuf {
    cwd_of(path)
}

fn cwd_of(path: &Path) -> PathBuf {
    let abs = path.canonicalize().unwrap_or_else(|_| absolutize(path));
    if abs.is_dir() {
        abs
    } else {
        abs.parent().map(|p| p.to_path_buf()).unwrap_or(abs)
    }
}

pub fn absolutize(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|c| c.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    }
}

fn cache_root(repo_root: &Path) -> Result<PathBuf> {
    // Worktree gate: a `.tracer-cache/` directory exists only at a worktree
    // root. The cheap-and-correct check is `<repo_root>/.git` — every
    // worktree (main or linked) has a `.git` entry at its own root (a
    // directory for the main repo, a file for a linked worktree), and
    // nothing else does. Using a filesystem stat instead of a fresh git
    // subprocess keeps the per-file warm-read path free of an extra
    // `git rev-parse` per call. Reads via `load` cleanly miss; writes via
    // `save` are already gated separately. The gate stops scattered
    // `.tracer-cache/` dirs from materializing under cwd when tracer is
    // invoked outside any git repo.
    if git_dir(repo_root).is_none() {
        anyhow::bail!("cache_root: not a worktree root: {}", repo_root.display());
    }
    let dir = repo_root.join(CACHE_DIR_NAME);
    fs::create_dir_all(&dir)?;
    let ignore = dir.join(".gitignore");
    if !ignore.exists() {
        fs::write(ignore, "*\n")?;
    }
    Ok(dir)
}

fn schema_directory(namespace: &str, repo_root: &Path) -> Result<PathBuf> {
    let dir = cache_root(repo_root)?
        .join(namespace)
        .join(format!("schema{SCHEMA_VERSION}"));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Relative path from `repo_root` (both canonicalized), falling back to the
/// absolute path string when `path` is not under `repo_root`.
pub fn relative_to_root(path: &Path, repo_root: &Path) -> String {
    let abs = path.canonicalize().unwrap_or_else(|_| absolutize(path));
    let root = repo_root
        .canonicalize()
        .unwrap_or_else(|_| absolutize(repo_root));
    match abs.strip_prefix(&root) {
        Ok(rel) => rel.to_string_lossy().to_string(),
        Err(_) => abs.to_string_lossy().to_string(),
    }
}

/// sha256("v{SCHEMA}|ccn:{backend}\0" + data + "\0" + relpath).
pub fn file_hash_from_bytes(data: &[u8], path: &Path, repo_root: &Path) -> String {
    let mut hasher = Sha256::new();
    hasher.update(format!("v{}|ccn:{}\0", SCHEMA_VERSION, active_ccn_backend()).as_bytes());
    hasher.update(data);
    hasher.update(b"\0");
    hasher.update(relative_to_root(path, repo_root).as_bytes());
    hex::encode(hasher.finalize())
}

pub fn file_hash(path: &Path, repo_root: &Path) -> Result<String> {
    let data = fs::read(path)?;
    Ok(file_hash_from_bytes(&data, path, repo_root))
}

/// The entry's raw bytes, for an entry whose reader deserializes straight
/// into its own type. A `serde_json::Value` is a tree of boxed nodes, so
/// parsing a repo-wide index into one first costs more memory than the index
/// itself: the relations index reads this instead.
pub fn load_bytes(namespace: &str, key: &str, repo_root: &Path) -> Option<Vec<u8>> {
    let dir = schema_directory(namespace, repo_root).ok()?;
    let entry = dir.join(format!("{key}.json"));
    if !entry.exists() {
        return None;
    }
    if crate::timing::enabled() {
        let name = format!("decode {key}");
        return crate::timing::phase(&name, || fs::read(&entry).ok());
    }
    fs::read(&entry).ok()
}

/// Atomic save: write a temp file in the same dir, fsync, rename into place.
/// Serialized as a single line (no indent) via the `jsonfmt` byte format.
/// Returns whether the key was new, which is the one moment a caller sweeps
/// its superseded keys: a stable key exists after its first write, so the
/// whole-directory sweep stops running on every write.
///
/// Hard-gated on `worktree_root_for(repo_root)` returning `Some` — the
/// single chokepoint that enforces "tracer caches live only at a worktree
/// root, never anywhere else." When `repo_root` is not itself a worktree
/// root (callers that slipped a `display_root` through, or paths outside
/// any worktree), the save is a silent no-op so standalone tracer use
/// outside a git repo keeps working without persisting state.
///
/// Debug builds assert that `repo_root` has a `.git` ancestor — that's
/// the same invariant `worktree_root_for` enforces, but the assert fires
/// the moment a caller passes a non-worktree path so the wrong call site
/// is named in tests rather than silently no-op'd.
pub fn save<T: serde::Serialize + ?Sized>(
    namespace: &str,
    key: &str,
    value: &T,
    repo_root: &Path,
) -> Result<bool> {
    debug_assert!(
        git_dir(repo_root).is_some(),
        "cache::save called with non-worktree repo_root: {}",
        repo_root.display()
    );
    // Hard-gate: a `.git` entry at `repo_root` is the cheap, exact
    // worktree-root predicate (see `cache_root`). When the gate fails the
    // save is a silent no-op so standalone use outside a git repo keeps
    // working without persisting state.
    if git_dir(repo_root).is_none() {
        return Ok(false);
    }
    let dir = schema_directory(namespace, repo_root)?;
    let entry = dir.join(format!("{key}.json"));
    let new = !entry.exists();
    // Unique temp per call: process id plus a monotonic sequence number,
    // so concurrent rayon writers in get_batch never collide on one temp
    // path (a collision is both a concurrency hazard and a write
    // serialization point).
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = dir.join(format!("{key}.{}.{n}.tmp", std::process::id()));
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(crate::jsonfmt::to_compact(value).as_bytes())?;
        // No fsync: atomic rename alone gives the crash-consistency we
        // need — a lost cache entry just re-extracts. Skipping the
        // per-entry fsync eliminates the 1000+-fsync cold-`list` stall.
    }
    fs::rename(&tmp, &entry)?;
    Ok(new)
}

/// Remove one entry. The mtime index calls this for the per-file key it just
/// superseded, so the namespace holds one entry per file instead of one per
/// version ever written: dotfiles had reached 17,736 entries for 795 files.
pub fn remove(namespace: &str, key: &str, repo_root: &Path) {
    if let Ok(dir) = schema_directory(namespace, repo_root) {
        let _ = fs::remove_file(dir.join(format!("{key}.json")));
    }
}

/// Delete every `{prefix}*` entry in this schema's directory except `keep`.
///
/// The git-activity and deploy-presence maps are keyed by a repo state that
/// moves — HEAD, the 30-day cutoff date, the deploy-branch tips — so without
/// this sweep each move leaves its predecessor behind forever: 64 superseded
/// `git_activity__*` entries totalling 52 MB had accumulated in this repo.
/// Runs after the rename, so a crash mid-write never removes the only good
/// entry. A content entry's name is its hex hash, which no prefix matches.
pub fn evict_prefixed(namespace: &str, prefix: &str, keep: &str, repo_root: &Path) {
    let Ok(dir) = schema_directory(namespace, repo_root) else {
        return;
    };
    let Ok(entries) = fs::read_dir(&dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_suffix(".json"))
            .is_some_and(|key| key != keep && key.starts_with(prefix))
        {
            let _ = fs::remove_file(&path);
        }
    }
}

/// Delete every other schema's directory beside `current` once nothing was
/// written in it for `stale_schema_age`, and the loose entries a build from
/// before schema directories left beside them once none of them was. A
/// directory's own modification time is its newest write, because `save`
/// renames every entry into it. Another schema's directory belongs to another
/// installed build that may still run, so removing it on sight made the two
/// builds rebuild each other's indexes on alternate calls.
fn evict_stale_schemas(current: &Path) {
    let Some(entries) = current.parent().and_then(|namespace| fs::read_dir(namespace).ok()) else {
        return;
    };
    let stale = |written: SystemTime| written.elapsed().is_ok_and(|age| age > stale_schema_age);
    let mut loose = (UNIX_EPOCH, Vec::new());
    for entry in entries.flatten() {
        let path = entry.path();
        let written = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .unwrap_or(UNIX_EPOCH);
        if !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            loose.0 = loose.0.max(written);
            loose.1.push(path);
        } else if path != current && stale(written) {
            let _ = fs::remove_dir_all(path);
        }
    }
    if stale(loose.0) {
        for path in loose.1 {
            let _ = fs::remove_file(path);
        }
    }
}

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub entry_count: usize,
    pub total_bytes: u64,
}

/// Entry count and total size of the `file/` namespace, every schema's
/// directory included. Absent means zero, not an error: a repo that has never
/// been read has no namespace directory.
pub fn stats(repo_root: &Path) -> Result<CacheStats> {
    let dir = cache_root(repo_root)?.join(NAMESPACE_FILE);
    let mut out = CacheStats {
        entry_count: 0,
        total_bytes: 0,
    };
    for e in walkdir::WalkDir::new(&dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| is_cache_entry(e.path()))
    {
        out.entry_count += 1;
        out.total_bytes += e.metadata().map(|m| m.len()).unwrap_or(0);
    }
    Ok(out)
}

/// Delete every entry in the `file/` namespace, every schema's directory
/// included, returning the removed count. The directories and their locks
/// stay, so `clear_all` remains the only way to remove the cache tree.
pub fn clear(repo_root: &Path) -> Result<usize> {
    let dir = cache_root(repo_root)?.join(NAMESPACE_FILE);
    let mut removed = 0;
    for e in walkdir::WalkDir::new(&dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| is_cache_entry(e.path()))
    {
        fs::remove_file(e.path())?;
        removed += 1;
    }
    Ok(removed)
}

/// Remove the entire cache tree, returning the count of cache entries.
pub fn clear_all(repo_root: &Path) -> Result<usize> {
    let root = repo_root.join(CACHE_DIR_NAME);
    if !root.is_dir() {
        return Ok(0);
    }
    let count = walkdir::WalkDir::new(&root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| is_cache_entry(e.path()))
        .count();
    fs::remove_dir_all(&root)?;
    Ok(count)
}

/// Every persisted cache entry is a `.json`. Temp files and lock files are
/// excluded so `stats` / `clear` count only durable entries.
fn is_cache_entry(p: &Path) -> bool {
    p.extension().and_then(|x| x.to_str()) == Some("json")
}
