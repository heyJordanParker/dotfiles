//! Worktree-anchored disk cache for the two repo-state namespaces.
//!
//! `.tracer-cache/file/{key}.json` and `.tracer-cache/architecture/{key}.bin`
//! at the worktree root — the main-repo root for a normal checkout, or the
//! linked-worktree's own root for a `git worktree add` checkout. A tracer
//! cache exists ONLY at a worktree root. Outside any worktree, reads still
//! return live results but nothing persists — the `cache::save` /
//! `cache::save_entry` chokepoints hard-gate on `worktree_root_for` and
//! no-op when it returns `None`.
//!
//! The namespaces never read each other. The third namespace, `sessions/`,
//! lives under the same `.tracer-cache/` at the worktree root and is owned
//! by `commands::session_log`. File-namespace entries are written as a
//! single line of JSON (no indent, the `jsonfmt` byte format) via `save`.
//!
//! `architecture/` holds the unified architecture graph — code
//! symbols/modules with their cross-file `imports` and `references` edges,
//! plus doc-file nodes (CLAUDE.md / Claude.md / .claude/rules/*.md) with
//! their `@include` edges. It is a bincode-encoded `Graph` written as a
//! single `.bin` entry via `save_entry` and decoded straight into the
//! struct on load — no JSON parse, no intermediate `Value` tree. Exactly
//! one entry per repo state: a write evicts every superseded fingerprint's
//! `.bin` sibling, so the namespace never grows without bound across HEAD
//! moves or doc changes. Fingerprinted over per-file content hashes AND the
//! doc-file mtime aggregate (plus git HEAD), so an edit to either input
//! invalidates the unified graph.
//!
//! File cache key:
//!   sha256("v{SCHEMA_VERSION}|ccn:{backend}\0" + contents + "\0" + relpath)
//! Architecture fingerprint:
//!   sha256(
//!     "v{SCHEMA_VERSION}|architecture\0"
//!     + (for relpath in sorted(hashes): relpath + "\0" + hash + "\n")
//!     + "\0docs\0" + git_head + "\0" + doc_mtime_aggregate
//!   )

use anyhow::Result;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const CACHE_DIR_NAME: &str = ".tracer-cache";
pub const NAMESPACE_FILE: &str = "file";
pub const NAMESPACE_ARCHITECTURE: &str = "architecture";

/// Bump whenever extraction, `FileFacts` shape, or the unified
/// architecture-graph shape changes — old entries become unreachable
/// automatically across all namespaces.
pub const SCHEMA_VERSION: u32 = 15;

/// Active CCN backend. There is exactly one backend — the tree-sitter
/// AST decision-node walker — so cache identity is unconditionally
/// `ccn:ast`. This keeps cache keys stable for `TRACER_CCN_BACKEND=ast`,
/// so warm caches are interchangeable and key-for-key comparable.
pub fn active_ccn_backend() -> &'static str {
    "ast"
}

/// Strict worktree-root resolver. Returns the worktree root containing
/// `path` — for the main repo, the repo root; for a linked git worktree,
/// the linked worktree's own root (git's `rev-parse --show-toplevel`
/// already returns the linked worktree's root when invoked from inside
/// it, so worktree-aware semantics fall out of the same one git call).
/// Returns `None` when `path` is not inside any worktree (no git repo, or
/// git unavailable) — the no-op trigger every cache-write path observes
/// so nothing ever persists outside a worktree root.
pub fn worktree_root_for(path: &Path) -> Option<PathBuf> {
    let cwd = cwd_of(path);
    let out = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(&cwd)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s.is_empty() {
        return None;
    }
    Some(PathBuf::from(s))
}

/// Non-persisting display root for read paths that need *some* base for
/// relative-path rendering when `path` lies outside any worktree. Never
/// pass this to `cache::save` or `cache::namespace_dir` — `cache::save`'s
/// hard-gate on `worktree_root_for` already no-ops if you do, but the
/// design contract is: read paths use this; write paths gate on the
/// worktree resolver themselves.
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

pub fn cache_root(repo_root: &Path) -> Result<PathBuf> {
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
    if !repo_root.join(".git").exists() {
        anyhow::bail!(
            "cache_root: not a worktree root: {}",
            repo_root.display()
        );
    }
    let dir = repo_root.join(CACHE_DIR_NAME);
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn namespace_dir(namespace: &str, repo_root: &Path) -> Result<PathBuf> {
    let dir = cache_root(repo_root)?.join(namespace);
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
    hasher.update(
        format!("v{}|ccn:{}\0", SCHEMA_VERSION, active_ccn_backend()).as_bytes(),
    );
    hasher.update(data);
    hasher.update(b"\0");
    hasher.update(relative_to_root(path, repo_root).as_bytes());
    hex::encode(hasher.finalize())
}

pub fn file_hash(path: &Path, repo_root: &Path) -> Result<String> {
    let data = fs::read(path)?;
    Ok(file_hash_from_bytes(&data, path, repo_root))
}

/// Unified architecture-graph fingerprint. `hashes` is relpath -> file hash
/// for the code side (BTreeMap guarantees sorted iteration); `git_head`
/// and `doc_mtime_aggregate` are the docs side. A change to either side
/// rotates the key, so the single cache entry under `architecture/`
/// invalidates whenever a code file or a doc file moves.
pub fn architecture_fingerprint(
    hashes: &BTreeMap<String, String>,
    git_head: &str,
    doc_mtime_aggregate: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(format!("v{SCHEMA_VERSION}|architecture\0").as_bytes());
    for (rel, h) in hashes {
        hasher.update(rel.as_bytes());
        hasher.update(b"\0");
        hasher.update(h.as_bytes());
        hasher.update(b"\n");
    }
    hasher.update(b"\0docs\0");
    hasher.update(git_head.as_bytes());
    hasher.update(b"\0");
    hasher.update(doc_mtime_aggregate.as_bytes());
    hex::encode(hasher.finalize())
}

/// On-disk extension for the architecture namespace's bincode entry. Every
/// other namespace stays JSON; the architecture graph is the one binary
/// entry, decoded straight into its struct with no JSON parse.
pub const ARCHITECTURE_ENTRY_EXTENSION: &str = "bin";

/// Load a cache entry as a serde_json::Value. None when missing or corrupt.
pub fn load(namespace: &str, key: &str, repo_root: &Path) -> Option<serde_json::Value> {
    let dir = namespace_dir(namespace, repo_root).ok()?;
    let entry = dir.join(format!("{key}.json"));
    if !entry.exists() {
        return None;
    }
    let bytes = fs::read(&entry).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Load a raw `.bin` entry's bytes. None when missing. Used by the
/// architecture namespace, whose entry is a bincode-encoded graph rather
/// than JSON.
pub fn load_bytes(namespace: &str, key: &str, repo_root: &Path) -> Option<Vec<u8>> {
    let dir = namespace_dir(namespace, repo_root).ok()?;
    let entry = dir.join(format!("{key}.{ARCHITECTURE_ENTRY_EXTENSION}"));
    if !entry.exists() {
        return None;
    }
    fs::read(&entry).ok()
}

/// The single `.bin` entry in a namespace, returned as `(key, bytes)` where
/// `key` is the filename stem (the fingerprint). Eviction keeps exactly one
/// architecture entry per repo, so the read path reads the sole entry and
/// validates it against the current code side without recomputing the full
/// fingerprint up front — and without re-walking the doc tree. Returns
/// `None` when the namespace has no `.bin` entry. When more than one exists
/// (a torn write that eviction has not yet collapsed), the
/// lexicographically-first is returned deterministically.
pub fn load_sole_entry(namespace: &str, repo_root: &Path) -> Option<(String, Vec<u8>)> {
    let dir = namespace_dir(namespace, repo_root).ok()?;
    let mut entries: Vec<PathBuf> = fs::read_dir(&dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension().and_then(|x| x.to_str()) == Some(ARCHITECTURE_ENTRY_EXTENSION)
        })
        .collect();
    entries.sort();
    let entry = entries.into_iter().next()?;
    let key = entry.file_stem()?.to_string_lossy().to_string();
    let bytes = fs::read(&entry).ok()?;
    Some((key, bytes))
}

/// Atomic save: write a temp file in the same dir, fsync, rename into place.
/// Serialized as a single line (no indent) via the `jsonfmt` byte format.
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
pub fn save(
    namespace: &str,
    key: &str,
    value: &serde_json::Value,
    repo_root: &Path,
) -> Result<()> {
    debug_assert!(
        repo_root.join(".git").exists(),
        "cache::save called with non-worktree repo_root: {}",
        repo_root.display()
    );
    // Hard-gate: a `.git` entry at `repo_root` is the cheap, exact
    // worktree-root predicate (see `cache_root`). When the gate fails the
    // save is a silent no-op so standalone use outside a git repo keeps
    // working without persisting state.
    if !repo_root.join(".git").exists() {
        return Ok(());
    }
    let dir = namespace_dir(namespace, repo_root)?;
    let entry = dir.join(format!("{key}.json"));
    // Unique temp per call: process id plus a monotonic sequence number,
    // so concurrent rayon writers in get_batch never collide on one temp
    // path (a collision is both a concurrency hazard and a write
    // serialization point).
    static SEQ: std::sync::atomic::AtomicU64 =
        std::sync::atomic::AtomicU64::new(0);
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
    Ok(())
}

/// Delete every `{prefix}*` entry in `namespace` except `keep`.
///
/// The git-activity and deploy-presence maps are keyed by a repo state that
/// moves — HEAD, the 30-day cutoff date, the deploy-branch tips — so without
/// this sweep each move leaves its predecessor behind forever: 64 superseded
/// `git_activity__*` entries totalling 52 MB had accumulated in this repo.
/// Runs after the rename, so a crash mid-write never removes the only good
/// entry, and matches on the filename prefix so the per-file content-hash
/// entries sharing the namespace are never touched.
pub fn evict_prefixed(namespace: &str, prefix: &str, keep: &str, repo_root: &Path) {
    let dir = match namespace_dir(namespace, repo_root) {
        Ok(d) => d,
        Err(_) => return,
    };
    let keep_name = format!("{keep}.json");
    if let Ok(rd) = fs::read_dir(&dir) {
        for e in rd.flatten() {
            let p = e.path();
            let name = match p.file_name().and_then(|n| n.to_str()) {
                Some(n) => n,
                None => continue,
            };
            if name != keep_name && name.starts_with(prefix) && is_cache_entry(&p) {
                let _ = fs::remove_file(&p);
            }
        }
    }
}

/// Atomic save of a raw `.bin` entry, then evict every stale sibling so the
/// namespace holds exactly the current fingerprint's entry. Same
/// worktree-root gate and atomic temp-then-rename as `save`. The architecture
/// namespace is keyed by a whole-repo fingerprint, so each HEAD move or doc
/// change would otherwise leave the prior fingerprint's entry behind to
/// accumulate without bound; deleting the siblings after the rename keeps one
/// entry per repo. Eviction runs after the rename so a crash mid-write never
/// removes the only good entry.
pub fn save_entry(
    namespace: &str,
    key: &str,
    bytes: &[u8],
    repo_root: &Path,
) -> Result<()> {
    debug_assert!(
        repo_root.join(".git").exists(),
        "cache::save_entry called with non-worktree repo_root: {}",
        repo_root.display()
    );
    if !repo_root.join(".git").exists() {
        return Ok(());
    }
    let dir = namespace_dir(namespace, repo_root)?;
    let entry = dir.join(format!("{key}.{ARCHITECTURE_ENTRY_EXTENSION}"));
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = dir.join(format!("{key}.{}.{n}.tmp", std::process::id()));
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
    }
    fs::rename(&tmp, &entry)?;
    // Evict stale siblings: every other cache entry is a superseded
    // fingerprint — in any format, so a schema bump that changed the on-disk
    // format (e.g. `.json` → `.bin`) leaves no orphaned prior-format entry
    // behind. Keep only the one just written; never touch temp/lock files.
    if let Ok(rd) = fs::read_dir(&dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p == entry {
                continue;
            }
            if is_cache_entry(&p) {
                let _ = fs::remove_file(&p);
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub namespace: String,
    pub entry_count: usize,
    pub total_bytes: u64,
}

pub fn stats(repo_root: &Path) -> Result<Vec<CacheStats>> {
    let root = cache_root(repo_root)?;
    let mut out = Vec::new();
    for ns in [NAMESPACE_FILE, NAMESPACE_ARCHITECTURE] {
        let dir = root.join(ns);
        if !dir.is_dir() {
            out.push(CacheStats {
                namespace: ns.to_string(),
                entry_count: 0,
                total_bytes: 0,
            });
            continue;
        }
        let mut count = 0;
        let mut size = 0u64;
        if let Ok(rd) = fs::read_dir(&dir) {
            for e in rd.flatten() {
                let p = e.path();
                if is_cache_entry(&p) {
                    count += 1;
                    size += e.metadata().map(|m| m.len()).unwrap_or(0);
                }
            }
        }
        out.push(CacheStats {
            namespace: ns.to_string(),
            entry_count: count,
            total_bytes: size,
        });
    }
    Ok(out)
}

/// Clear a namespace (`None` clears both). Returns the removed entry count.
pub fn clear(namespace: Option<&str>, repo_root: &Path) -> Result<usize> {
    let root = cache_root(repo_root)?;
    let mut removed = 0;
    let namespaces: Vec<&str> = match namespace {
        Some(ns) => vec![ns],
        None => vec![NAMESPACE_FILE, NAMESPACE_ARCHITECTURE],
    };
    for ns in namespaces {
        let dir = root.join(ns);
        if !dir.is_dir() {
            continue;
        }
        if let Ok(rd) = fs::read_dir(&dir) {
            for e in rd.flatten() {
                let p = e.path();
                if is_cache_entry(&p) {
                    fs::remove_file(&p)?;
                    removed += 1;
                }
            }
        }
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

/// A persisted cache entry is a `.json` (file namespace + mtime index) or a
/// `.bin` (the architecture graph). Temp files and lock files are excluded so
/// `stats` / `clear` count only durable entries.
fn is_cache_entry(p: &Path) -> bool {
    matches!(
        p.extension().and_then(|x| x.to_str()),
        Some("json") | Some(ARCHITECTURE_ENTRY_EXTENSION)
    )
}
