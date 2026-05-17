//! Two-namespace disk cache.
//!
//! `.tracer-cache/{file,architecture}/{key}.json` at the repo root. The two
//! namespaces never read each other. Cache entries are written as a single
//! line of JSON (no indent, the `jsonfmt` byte format).
//!
//! File cache key:
//!   sha256("v{SCHEMA_VERSION}|ccn:{backend}\0" + contents + "\0" + relpath)
//! Architecture fingerprint:
//!   sha256(for relpath in sorted(hashes): relpath + "\0" + hash + "\n")

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

/// Bump whenever extraction or `FileFacts` shape changes — old entries
/// become unreachable automatically across all namespaces.
pub const SCHEMA_VERSION: u32 = 7;

/// Active CCN backend. There is exactly one backend — the tree-sitter
/// AST decision-node walker — so cache identity is unconditionally
/// `ccn:ast`. This keeps cache keys stable for `TRACER_CCN_BACKEND=ast`,
/// so warm caches are interchangeable and key-for-key comparable.
pub fn active_ccn_backend() -> &'static str {
    "ast"
}

/// `git rev-parse --show-toplevel` from `path`'s directory, falling back to
/// the directory itself (or the file's parent) when not in a git repo.
pub fn repo_root_for(path: &Path) -> PathBuf {
    let abs = path.canonicalize().unwrap_or_else(|_| absolutize(path));
    let cwd = if abs.is_dir() {
        abs.clone()
    } else {
        abs.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| abs.clone())
    };
    if let Ok(out) = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(&cwd)
        .output()
    {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() {
                return PathBuf::from(s);
            }
        }
    }
    cwd
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

/// Architecture-graph fingerprint. `hashes` is relpath -> file hash; the
/// BTreeMap guarantees the sorted iteration the fingerprint depends on.
pub fn architecture_fingerprint(hashes: &BTreeMap<String, String>) -> String {
    let mut hasher = Sha256::new();
    for (rel, h) in hashes {
        hasher.update(rel.as_bytes());
        hasher.update(b"\0");
        hasher.update(h.as_bytes());
        hasher.update(b"\n");
    }
    hex::encode(hasher.finalize())
}

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

/// Atomic save: write a temp file in the same dir, fsync, rename into place.
/// Serialized as a single line (no indent) via the `jsonfmt` byte format.
pub fn save(
    namespace: &str,
    key: &str,
    value: &serde_json::Value,
    repo_root: &Path,
) -> Result<()> {
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
                if p.extension().and_then(|x| x.to_str()) == Some("json") {
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
                if p.extension().and_then(|x| x.to_str()) == Some("json") {
                    fs::remove_file(&p)?;
                    removed += 1;
                }
            }
        }
    }
    Ok(removed)
}

/// Remove the entire cache tree, returning the count of *.json entries.
pub fn clear_all(repo_root: &Path) -> Result<usize> {
    let root = repo_root.join(CACHE_DIR_NAME);
    if !root.is_dir() {
        return Ok(0);
    }
    let count = walkdir::WalkDir::new(&root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("json"))
        .count();
    fs::remove_dir_all(&root)?;
    Ok(count)
}
