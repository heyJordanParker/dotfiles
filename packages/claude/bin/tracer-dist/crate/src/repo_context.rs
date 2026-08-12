//! Repo-wide scc context: disk-cached complexity_p95 and language summary.
//!
//! One `scc --format json --by-file <root>` invocation produces summary
//! stats (total_files, median_file_ccn, complexity_p95), a per-file
//! {ccn, loc, language} map, and per-language aggregate rows. Disk-cached
//! under `repo_context_v3_{head}` in the file namespace.

use crate::cache;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};

const CACHE_KEY_PREFIX: &str = "repo_context_v3_";

fn git_head(repo_root: &Path) -> Option<String> {
    let out = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo_root)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn empty_payload() -> Value {
    json!({
        "summary": {"total_files": 0, "median_file_ccn": 0, "complexity_p95": 0},
        "per_file": {},
        "languages": [],
    })
}

/// Compute the repo-context payload from a single scc invocation.
fn compute(repo_root: &Path) -> Value {
    let out = match Command::new("scc")
        .args(["--format", "json", "--by-file"])
        .arg(repo_root)
        .output()
    {
        Ok(o) if o.status.success() => o.stdout,
        _ => return empty_payload(),
    };
    let data: Value = match serde_json::from_slice(&out) {
        Ok(v) => v,
        Err(_) => return empty_payload(),
    };
    let arr = match data.as_array() {
        Some(a) => a,
        None => return empty_payload(),
    };
    let root_resolved = repo_root
        .canonicalize()
        .unwrap_or_else(|_| repo_root.to_path_buf());

    let mut complexities: Vec<i64> = Vec::new();
    let mut per_file = serde_json::Map::new();
    let mut languages: Vec<Value> = Vec::new();

    for lang_block in arr {
        let language = lang_block.get("Name").cloned().unwrap_or(Value::Null);
        languages.push(json!({
            "Name": language,
            "Count": lang_block.get("Count").cloned().unwrap_or(json!(0)),
            "Code": lang_block.get("Code").cloned().unwrap_or(json!(0)),
            "Complexity": lang_block.get("Complexity").cloned().unwrap_or(json!(0)),
        }));
        if let Some(files) = lang_block.get("Files").and_then(|f| f.as_array()) {
            for fe in files {
                let ccn = fe.get("Complexity").and_then(|x| x.as_i64()).unwrap_or(0);
                let loc = fe.get("Code").and_then(|x| x.as_i64()).unwrap_or(0);
                let location = fe
                    .get("Location")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                if location.is_empty() {
                    continue;
                }
                complexities.push(ccn);
                let rel = match Path::new(&location).strip_prefix(&root_resolved) {
                    Ok(r) => r.to_string_lossy().to_string(),
                    Err(_) => location.clone(),
                };
                per_file.insert(
                    rel,
                    json!({"ccn": ccn, "loc": loc, "language": language}),
                );
            }
        }
    }

    if complexities.is_empty() {
        return empty_payload();
    }
    let mut sorted_c = complexities.clone();
    sorted_c.sort_unstable();
    let p95_idx = (((sorted_c.len() as f64) * 0.95) as i64 - 1).max(0) as usize;
    json!({
        "summary": {
            "total_files": complexities.len(),
            "median_file_ccn": median_int(&sorted_c),
            "complexity_p95": sorted_c[p95_idx],
        },
        "per_file": Value::Object(per_file),
        "languages": languages,
    })
}

/// Integer median: average of the two middle values for even counts,
/// truncated toward zero.
fn median_int(sorted: &[i64]) -> i64 {
    let n = sorted.len();
    if n == 0 {
        return 0;
    }
    if n % 2 == 1 {
        sorted[n / 2]
    } else {
        // statistics.median returns a float (a+b)/2; int() truncates.
        ((sorted[n / 2 - 1] + sorted[n / 2]) as f64 / 2.0) as i64
    }
}

/// Process-wide memo of the scc payload, keyed by repo root. The payload is
/// a function of git HEAD, stable within a single CLI invocation, so the
/// `git rev-parse HEAD` key lookup, the disk load, and the JSON parse run at
/// most once per root — `language_summary`, `per_file_metrics`, and
/// `repo_context` all share it. The lock is held across the compute so a
/// cold cache runs `scc` exactly once even when parallel callers race.
fn load_or_compute(repo_root: &Path) -> Value {
    static MEMO: OnceLock<Mutex<HashMap<PathBuf, Value>>> = OnceLock::new();
    let memo = MEMO.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = memo.lock().unwrap();
    if let Some(cached) = guard.get(repo_root) {
        return cached.clone();
    }
    let computed = load_or_compute_uncached(repo_root);
    guard.insert(repo_root.to_path_buf(), computed.clone());
    computed
}

fn load_or_compute_uncached(repo_root: &Path) -> Value {
    // Same worktree gate as the architecture graph: `cache::save` writes only
    // where `repo_root/.git` is, so outside a worktree the scc pass and its
    // per-file map are recomputed on every call and never persist.
    if !repo_root.join(".git").exists() {
        return empty_payload();
    }
    match git_head(repo_root) {
        None => compute(repo_root),
        Some(head) => {
            let key = format!("{CACHE_KEY_PREFIX}{head}");
            if let Some(cached) = cache::load(cache::NAMESPACE_FILE, &key, repo_root) {
                if cached.get("summary").is_some()
                    && cached.get("per_file").is_some()
                    && cached.get("languages").is_some()
                {
                    return cached;
                }
            }
            let payload = compute(repo_root);
            let _ = cache::save(cache::NAMESPACE_FILE, &key, &payload, repo_root);
            payload
        }
    }
}

/// The repo-context summary object.
pub fn repo_context(path: &Path) -> Value {
    let root = cache::worktree_root_for(path).unwrap_or_else(|| cache::display_root(path));
    load_or_compute(&root)
        .get("summary")
        .cloned()
        .unwrap_or_else(|| json!({}))
}

/// Per-file scc metrics: relpath -> {ccn, loc, language}.
pub fn per_file_metrics(repo_root: &Path) -> HashMap<String, Value> {
    let payload = load_or_compute(repo_root);
    payload
        .get("per_file")
        .and_then(|v| v.as_object())
        .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default()
}

/// Per-language summary rows: [{Name, Count, Code, Complexity}].
pub fn language_summary(repo_root: &Path) -> Vec<Value> {
    load_or_compute(repo_root)
        .get("languages")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
}
