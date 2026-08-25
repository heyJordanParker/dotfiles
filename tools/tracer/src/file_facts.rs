//! Per-file facts layer: per-file extraction + `file/` namespace owner.
//!
//! The CCN backend is always the tree-sitter AST walker.
//!
//! NO lite-facts shortcut: every file that appears in any listing gets
//! real parsed per-function CCN, real function count, real max CCN. `get`
//! always does the real extraction on cache miss.
//!
//! Cache-entry serialization order is fixed: path, language, loc, function_count,
//! cyclomatic_complexity_total, cyclomatic_complexity_max, rank, mtime_ns,
//! size_bytes, extraction  (extraction LAST).
//!
//! An entry holds only what its key determines. The key is
//! sha256(schema, contents, relpath), so the git fields — which move with HEAD
//! and with the working tree, never with the bytes — are not serialized here:
//! `git_activity` owns them, keys its own cache by HEAD, and every resolve
//! path joins them on through `with_git`. Storing them in this entry left a
//! committed file rendering `modified (N commits)` until its bytes changed.

use crate::cache;
use crate::ccn;
use crate::extraction::{self, ExtractionResult};
use crate::git_activity::{self, GitActivity};
use crate::repo_context;
use rayon::prelude::*;
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct FileFacts {
    pub path: String,
    pub language: Option<String>,
    pub loc: i64,
    pub function_count: i64,
    pub cyclomatic_complexity_total: i64,
    pub cyclomatic_complexity_max: i64,
    pub rank: String,
    pub extraction: Option<ExtractionResult>,
    pub last_modified: Option<String>,
    pub last_author: Option<String>,
    pub commits_30d: i64,
    pub first_seen: Option<String>,
    pub commit_count: i64,
    pub rename_from: Option<String>,
    pub working_state: Option<String>,
    pub present_in: Vec<String>,
    pub last_subject: Option<String>,
    pub top_author: Option<String>,
    pub co_changed: Vec<(String, i64)>,
    pub mtime_ns: i64,
    pub size_bytes: i64,
}

fn opt(o: &Option<String>) -> Value {
    match o {
        Some(s) => json!(s),
        None => Value::Null,
    }
}

impl FileFacts {
    /// All scalar fields, then `extraction` appended last.
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert("path".into(), json!(self.path));
        m.insert("language".into(), opt(&self.language));
        m.insert("loc".into(), json!(self.loc));
        m.insert("function_count".into(), json!(self.function_count));
        m.insert(
            "cyclomatic_complexity_total".into(),
            json!(self.cyclomatic_complexity_total),
        );
        m.insert(
            "cyclomatic_complexity_max".into(),
            json!(self.cyclomatic_complexity_max),
        );
        m.insert("rank".into(), json!(self.rank));
        m.insert("mtime_ns".into(), json!(self.mtime_ns));
        m.insert("size_bytes".into(), json!(self.size_bytes));
        m.insert(
            "extraction".into(),
            match &self.extraction {
                Some(e) => e.to_json(),
                None => Value::Null,
            },
        );
        Value::Object(m)
    }

    pub fn from_json(v: &Value) -> Option<FileFacts> {
        let s = |k: &str| v.get(k).and_then(|x| x.as_str()).map(|x| x.to_string());
        let n = |k: &str| v.get(k).and_then(|x| x.as_i64()).unwrap_or(0);
        Some(FileFacts {
            path: s("path")?,
            language: s("language"),
            loc: n("loc"),
            function_count: n("function_count"),
            cyclomatic_complexity_total: n("cyclomatic_complexity_total"),
            cyclomatic_complexity_max: n("cyclomatic_complexity_max"),
            rank: s("rank").unwrap_or_else(|| "unknown".into()),
            extraction: v
                .get("extraction")
                .filter(|e| !e.is_null())
                .map(ExtractionResult::from_json),
            // Git fields are not in the entry; `with_git` fills them from the
            // HEAD-keyed map on every resolve path.
            last_modified: None,
            last_author: None,
            commits_30d: 0,
            first_seen: None,
            commit_count: 0,
            rename_from: None,
            working_state: None,
            present_in: Vec::new(),
            last_subject: None,
            top_author: None,
            co_changed: Vec::new(),
            mtime_ns: n("mtime_ns"),
            size_bytes: n("size_bytes"),
        })
    }
}

/// Join the git facts onto code facts. `git_activity` owns every one of them
/// and keys its cache by HEAD, so this runs on the cache-hit paths and on
/// fresh extraction alike. A path the map does not carry — no history, no
/// working-tree state, no deploy-branch presence — keeps the empty values,
/// which render as `no-history`.
fn with_git(
    mut facts: FileFacts,
    rel: &str,
    git_map: &HashMap<String, GitActivity>,
) -> FileFacts {
    let git = match git_map.get(rel) {
        Some(g) => g,
        None => return facts,
    };
    facts.last_modified = git.last_modified.clone();
    facts.last_author = git.last_author.clone();
    facts.commits_30d = git.commits_30d;
    facts.first_seen = git.first_seen.clone();
    facts.commit_count = git.commit_count;
    facts.rename_from = git.rename_from.clone();
    facts.working_state = git.working_state.clone();
    facts.present_in = git.present_in.clone();
    facts.last_subject = git.last_subject.clone();
    facts.top_author = git.top_author.clone();
    facts.co_changed = git.co_changed.clone();
    facts
}

/// Complexity rank bucket (low / medium / high / critical) for a CCN total.
pub fn rank(complexity: i64) -> &'static str {
    if complexity < 10 {
        "low"
    } else if complexity < 30 {
        "medium"
    } else if complexity < 80 {
        "high"
    } else {
        "critical"
    }
}

/// CCN scalars from already-computed per-function facts. Falls back to scc
/// when there are no facts, and to `scc_loc` when the per-function loc sum
/// is zero.
fn ccn_scalars(
    functions: &[ccn::FunctionFact],
    scc_data: &Value,
) -> (i64, i64, i64, i64) {
    if !functions.is_empty() {
        let ccn_total: i64 =
            functions.iter().map(|f| f.cyclomatic_complexity).sum();
        let ccn_max: i64 = functions
            .iter()
            .map(|f| f.cyclomatic_complexity)
            .max()
            .unwrap_or(0);
        let loc_sum: i64 = functions.iter().map(|f| f.nloc).sum();
        let scc_loc = scc_data.get("loc").and_then(|x| x.as_i64()).unwrap_or(0);
        let loc = if loc_sum != 0 { loc_sum } else { scc_loc };
        return (ccn_total, ccn_max, loc, functions.len() as i64);
    }
    let scc_ccn = scc_data.get("ccn").and_then(|x| x.as_i64()).unwrap_or(0);
    let scc_loc = scc_data.get("loc").and_then(|x| x.as_i64()).unwrap_or(0);
    (scc_ccn, 0, scc_loc, 0)
}

/// Extract per-file facts. `git` is precomputed by the caller.
///
/// Single-parse: the CCN backend and the import/export extractor share ONE
/// `tree_sitter::Tree` when the grammars coincide (py/ts/php — the only
/// languages with both a CCN spec and an extractor), eliminating a second
/// parse of the dominant cost on those files. For CCN-only languages
/// (go/rust/...) extraction is None anyway, so there is no second parse to
/// eliminate.
fn extract_facts(
    path: &Path,
    repo_root: &Path,
    git: &GitActivity,
    scc_data: &Value,
    source_bytes: &[u8],
) -> FileFacts {
    let relative = cache::relative_to_root(path, repo_root);
    let path_str = path.to_string_lossy().to_string();

    // One parse, shared. `ccn::lang_for_path` is the single source of truth
    // for the grammar; the extractor's grammar for py/ts/php is identical.
    let (functions, extraction): (Vec<ccn::FunctionFact>, Option<extraction::ExtractionResult>) =
        match ccn::lang_for_path(&path_str) {
            Some((lang_name, language)) => {
                let mut parser = tree_sitter::Parser::new();
                if parser.set_language(&language).is_err() {
                    (Vec::new(), None)
                } else if let Some(tree) = parser.parse(source_bytes, None) {
                    let facts =
                        ccn::facts_from_tree(&tree, source_bytes, lang_name);
                    // Reuse the tree for extraction whenever the extractor
                    // uses the same grammar `ccn::lang_for_path` resolved —
                    // every supported extension now shares its CCN grammar
                    // with its extractor, so this eliminates the second parse
                    // for all of them.
                    let lower = path_str.to_lowercase();
                    let extr = if extraction::is_supported(path) {
                        if lower.ends_with(".py") {
                            Some(extraction::python::extract_from_tree(
                                &tree,
                                source_bytes,
                            ))
                        } else if lower.ends_with(".php") {
                            Some(extraction::php::extract_from_tree(
                                &tree,
                                source_bytes,
                            ))
                        } else if lower.ends_with(".ts")
                            || lower.ends_with(".js")
                            || lower.ends_with(".tsx")
                            || lower.ends_with(".jsx")
                        {
                            let is_tsx = lower.ends_with(".tsx")
                                || lower.ends_with(".jsx");
                            Some(extraction::typescript::extract_from_tree(
                                &tree,
                                source_bytes,
                                is_tsx,
                            ))
                        } else if lower.ends_with(".rs") {
                            Some(extraction::rust::extract_from_tree(&tree, source_bytes))
                        } else if lower.ends_with(".go") {
                            Some(extraction::go::extract_from_tree(&tree, source_bytes))
                        } else if lower.ends_with(".rb") {
                            Some(extraction::ruby::extract_from_tree(&tree, source_bytes))
                        } else if lower.ends_with(".java") {
                            Some(extraction::java::extract_from_tree(&tree, source_bytes))
                        } else if lower.ends_with(".c") || lower.ends_with(".h") {
                            Some(extraction::c::extract_from_tree(&tree, source_bytes))
                        } else {
                            // is_supported but not a shared-grammar ext:
                            // fall back to its own parse (rare/none).
                            extraction::extract(source_bytes, &path_str)
                        }
                    } else {
                        None
                    };
                    (facts, extr)
                } else {
                    (Vec::new(), None)
                }
            }
            None => {
                // No CCN grammar (e.g. markdown/json). Extraction also has
                // none for these, so just the scc fallback path.
                (
                    Vec::new(),
                    if extraction::is_supported(path) {
                        extraction::extract(source_bytes, &path_str)
                    } else {
                        None
                    },
                )
            }
        };

    let (ccn_total, ccn_max, loc, function_count) =
        ccn_scalars(&functions, scc_data);

    let (mtime_ns, size_bytes) = match fs::metadata(path) {
        Ok(md) => (mtime_ns_of(&md), md.len() as i64),
        Err(_) => (0, 0),
    };

    let language = extraction
        .as_ref()
        .map(|e| e.language.clone())
        .or_else(|| {
            scc_data
                .get("language")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string())
        });

    FileFacts {
        path: relative,
        language,
        loc,
        function_count,
        cyclomatic_complexity_total: ccn_total,
        cyclomatic_complexity_max: ccn_max,
        rank: rank(ccn_total).to_string(),
        extraction,
        last_modified: git.last_modified.clone(),
        last_author: git.last_author.clone(),
        commits_30d: git.commits_30d,
        first_seen: git.first_seen.clone(),
        commit_count: git.commit_count,
        rename_from: git.rename_from.clone(),
        working_state: git.working_state.clone(),
        present_in: git.present_in.clone(),
        last_subject: git.last_subject.clone(),
        top_author: git.top_author.clone(),
        co_changed: git.co_changed.clone(),
        mtime_ns,
        size_bytes,
    }
}

#[cfg(unix)]
fn mtime_ns_of(md: &fs::Metadata) -> i64 {
    use std::os::unix::fs::MetadataExt;
    md.mtime() as i64 * 1_000_000_000 + md.mtime_nsec() as i64
}
#[cfg(not(unix))]
fn mtime_ns_of(md: &fs::Metadata) -> i64 {
    md.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos() as i64)
        .unwrap_or(0)
}

fn mtime_index_key() -> String {
    // Includes SCHEMA_VERSION so a binary upgrade that bumps the schema
    // also rotates this index — the per-file hashes the index serves are
    // schema-namespaced, so an index from the previous schema would point
    // at unreachable cache entries (or worse, key the architecture
    // fingerprint off old hashes and serve a stale graph).
    format!(
        "mtime_index_v1__schema{}__{}",
        cache::SCHEMA_VERSION,
        cache::active_ccn_backend()
    )
}

fn mtime_index_load(repo_root: &Path) -> Map<String, Value> {
    cache::load(cache::NAMESPACE_FILE, &mtime_index_key(), repo_root)
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default()
}

fn mtime_index_record(
    repo_root: &Path,
    rel: &str,
    mtime_ns: i64,
    size: i64,
    key: &str,
) {
    let mut idx = mtime_index_load(repo_root);
    idx.insert(
        rel.to_string(),
        json!({"mtime_ns": mtime_ns, "size": size, "key": key}),
    );
    let _ = cache::save(
        cache::NAMESPACE_FILE,
        &mtime_index_key(),
        &Value::Object(idx),
        repo_root,
    );
}

/// Facts for one file. NO lite-facts: always real extraction on a miss.
pub fn get(path: &Path, repo_root: &Path) -> Option<FileFacts> {
    let p = path.canonicalize().ok()?;
    if !p.is_file() {
        return None;
    }
    let md = fs::metadata(&p).ok()?;
    let stat_mtime = mtime_ns_of(&md);
    let stat_size = md.len() as i64;
    let rel = cache::relative_to_root(&p, repo_root);
    // Memoized per repo root, so the per-file callers (`glob`, `diff`,
    // `blame`, `structure`) build it once for the whole run.
    let git_map = git_activity::bulk_cached(repo_root);

    // 1. mtime fast path.
    let idx = mtime_index_load(repo_root);
    if let Some(entry) = idx.get(&rel) {
        let im = entry.get("mtime_ns").and_then(|x| x.as_i64());
        let is = entry.get("size").and_then(|x| x.as_i64());
        if im == Some(stat_mtime) && is == Some(stat_size) {
            if let Some(k) = entry.get("key").and_then(|x| x.as_str()) {
                if let Some(cv) = cache::load(cache::NAMESPACE_FILE, k, repo_root) {
                    if let Some(f) = FileFacts::from_json(&cv) {
                        return Some(with_git(f, &rel, &git_map));
                    }
                }
            }
        }
    }

    // 2. content-hash cache entry.
    let data = fs::read(&p).ok()?;
    let key = cache::file_hash_from_bytes(&data, &p, repo_root);
    if let Some(cv) = cache::load(cache::NAMESPACE_FILE, &key, repo_root) {
        if let Some(f) = FileFacts::from_json(&cv) {
            mtime_index_record(repo_root, &rel, stat_mtime, stat_size, &key);
            return Some(with_git(f, &rel, &git_map));
        }
    }

    // 3. fresh real extraction.
    let git = git_map.get(&rel).cloned().unwrap_or_else(GitActivity::empty);
    let scc_map = repo_context::per_file_metrics(repo_root);
    let scc_data = scc_map.get(&rel).cloned().unwrap_or_else(|| json!({}));
    let facts = extract_facts(&p, repo_root, &git, &scc_data, &data);
    let _ = cache::save(
        cache::NAMESPACE_FILE,
        &key,
        &facts.to_json(),
        repo_root,
    );
    mtime_index_record(repo_root, &rel, stat_mtime, stat_size, &key);
    Some(facts)
}

/// Bulk resolver — the ONLY correct path for directory/repo-wide commands
/// (`list`, `info <dir>`, `tree`, `context`, status/diff aggregation).
///
/// The defect this replaces: calling per-file `get()` in a loop made every
/// one of N files (a) re-load+parse the whole mtime index, (b) re-write the
/// whole mtime index + fsync, (c) rebuild the whole git-activity map, (d)
/// re-load the whole scc map — O(N²) parse + an O(N²) fsync write storm
/// (62s / pathological on a 3000-file repo). Bulk maps are hoisted once,
/// never rebuilt per-iteration (the loop hot-path ban in Claude.md).
///
/// Fix, with no lite-facts shortcut:
///   - git map, scc map, mtime index, working-state: loaded ONCE.
///   - in-memory mtime fast-path: unchanged files skip read+hash+extract.
///   - parallel extraction for true misses only.
///   - the mtime index is written ONCE at the end (no per-file rewrite,
///     no rayon write race), atomically via the existing cache::save.
/// Returns rel -> FileFacts for every readable input path.
pub fn get_batch(
    paths: &[PathBuf],
    repo_root: &Path,
) -> HashMap<String, FileFacts> {
    let git_map = git_activity::bulk_cached(repo_root);
    let scc_map = repo_context::per_file_metrics(repo_root);
    let index = mtime_index_load(repo_root);

    #[derive(Clone)]
    struct Job {
        abs: PathBuf,
        rel: String,
        mtime_ns: i64,
        size: i64,
    }
    let mut jobs: Vec<Job> = Vec::with_capacity(paths.len());
    for p in paths {
        // Resolve to (abs, rel) without a per-file `canonicalize()` syscall.
        // Batch inputs are tracked paths under `repo_root` (git-ls-files
        // results joined onto the root, or `repo_root.join(rel)` from command
        // code), so the absolute path and the repo-relative key follow
        // lexically. `fs::metadata` (one stat the loop already needs) both
        // confirms the file exists and supplies mtime + size; its `is_file`
        // replaces the canonicalize-time `a.is_file()` non-file filter.
        let (abs, rel) = resolve_under_root(p, repo_root);
        let md = match fs::metadata(&abs) {
            Ok(m) if m.is_file() => m,
            _ => continue,
        };
        jobs.push(Job {
            rel,
            mtime_ns: mtime_ns_of(&md),
            size: md.len() as i64,
            abs,
        });
    }

    // Parallel resolve. Each job yields (rel, FileFacts, Option<new index
    // entry>). Index entries are merged single-threaded afterward and the
    // index is persisted exactly once.
    let resolved: Vec<(String, FileFacts, Option<(String, i64, i64, String)>)> =
        jobs
            .par_iter()
            .filter_map(|j| {
                // (1) in-memory mtime fast-path — no I/O beyond the cached
                // entry load when (mtime,size) match the once-loaded index.
                if let Some(entry) = index.get(&j.rel) {
                    let im = entry.get("mtime_ns").and_then(|x| x.as_i64());
                    let is = entry.get("size").and_then(|x| x.as_i64());
                    if im == Some(j.mtime_ns) && is == Some(j.size) {
                        if let Some(k) =
                            entry.get("key").and_then(|x| x.as_str())
                        {
                            if let Some(cv) = cache::load(
                                cache::NAMESPACE_FILE,
                                k,
                                repo_root,
                            ) {
                                if let Some(f) = FileFacts::from_json(&cv) {
                                    return Some((
                                        j.rel.clone(),
                                        with_git(f, &j.rel, &git_map),
                                        None,
                                    ));
                                }
                            }
                        }
                    }
                }
                // (2) content-hash cache hit.
                let data = fs::read(&j.abs).ok()?;
                let key =
                    cache::file_hash_from_bytes(&data, &j.abs, repo_root);
                if let Some(cv) =
                    cache::load(cache::NAMESPACE_FILE, &key, repo_root)
                {
                    if let Some(f) = FileFacts::from_json(&cv) {
                        return Some((
                            j.rel.clone(),
                            with_git(f, &j.rel, &git_map),
                            Some((
                                j.rel.clone(),
                                j.mtime_ns,
                                j.size,
                                key,
                            )),
                        ));
                    }
                }
                // (3) fresh real extraction (no lite-facts).
                let git = git_map
                    .get(&j.rel)
                    .cloned()
                    .unwrap_or_else(GitActivity::empty);
                let scc_data = scc_map
                    .get(&j.rel)
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                let facts = extract_facts(
                    &j.abs, repo_root, &git, &scc_data, &data,
                );
                let _ = cache::save(
                    cache::NAMESPACE_FILE,
                    &key,
                    &facts.to_json(),
                    repo_root,
                );
                Some((
                    j.rel.clone(),
                    facts,
                    Some((j.rel.clone(), j.mtime_ns, j.size, key)),
                ))
            })
            .collect();

    // Merge index updates and persist ONCE (was the O(N²) fsync storm).
    let mut new_index = index;
    let mut out = HashMap::with_capacity(resolved.len());
    let mut dirty = false;
    for (rel, facts, upd) in resolved {
        if let Some((r, mt, sz, key)) = upd {
            new_index.insert(
                r,
                json!({"mtime_ns": mt, "size": sz, "key": key}),
            );
            dirty = true;
        }
        out.insert(rel, facts);
    }
    if dirty {
        let _ = cache::save(
            cache::NAMESPACE_FILE,
            &mtime_index_key(),
            &Value::Object(new_index),
            repo_root,
        );
    }
    out
}

/// Facts for many files in input order, via the batch resolver (single
/// index write, in-memory fast-path).
pub fn get_many(paths: &[PathBuf], repo_root: &Path) -> Vec<FileFacts> {
    let mut map = get_batch(paths, repo_root);
    // Preserve input order for callers that relied on it. The rel key is the
    // same lexical resolution `get_batch` keyed the map by — no canonicalize.
    paths
        .iter()
        .filter_map(|p| {
            let (_, rel) = resolve_under_root(p, repo_root);
            map.remove(&rel)
        })
        .collect()
}

/// Resolve a batch input path to `(absolute, repo-relative)` lexically — no
/// `canonicalize()` syscall. Batch inputs are tracked paths under
/// `repo_root`: an absolute path keeps its bytes and strips `repo_root` for
/// the rel key; a relative path is joined onto `repo_root` (the batch
/// resolver's paths are repo-relative). When the path is not under
/// `repo_root` the absolute path string is the rel key, matching the
/// long-standing `relative_to_root` fallback so out-of-root inputs behave
/// unchanged. The lexical strip uses `repo_root` as given; callers pass the
/// worktree root, which is already canonical from `worktree_root_for`.
fn resolve_under_root(p: &Path, repo_root: &Path) -> (PathBuf, String) {
    let abs = if p.is_absolute() {
        p.to_path_buf()
    } else {
        repo_root.join(p)
    };
    let rel = match abs.strip_prefix(repo_root) {
        Ok(r) => r.to_string_lossy().to_string(),
        Err(_) => abs.to_string_lossy().to_string(),
    };
    (abs, rel)
}

/// relpath -> file hash, using the mtime index fast path for unchanged
/// files.
pub fn file_hashes_for(
    paths: &[PathBuf],
    repo_root: &Path,
) -> std::collections::BTreeMap<String, String> {
    let idx = mtime_index_load(repo_root);
    let mut out = std::collections::BTreeMap::new();
    let mut misses: Vec<(PathBuf, String)> = Vec::new();
    for p in paths {
        if !p.is_file() {
            continue;
        }
        let abs = match p.canonicalize() {
            Ok(a) => a,
            Err(_) => continue,
        };
        let rel = cache::relative_to_root(&abs, repo_root);
        let md = match fs::metadata(&abs) {
            Ok(m) => m,
            Err(_) => continue,
        };
        let mt = mtime_ns_of(&md);
        let sz = md.len() as i64;
        if let Some(entry) = idx.get(&rel) {
            if entry.get("mtime_ns").and_then(|x| x.as_i64()) == Some(mt)
                && entry.get("size").and_then(|x| x.as_i64()) == Some(sz)
            {
                if let Some(k) = entry.get("key").and_then(|x| x.as_str()) {
                    out.insert(rel, k.to_string());
                    continue;
                }
            }
        }
        misses.push((abs, rel));
    }
    let hashed: Vec<(String, String)> = misses
        .par_iter()
        .filter_map(|(p, rel)| {
            cache::file_hash(p, repo_root).ok().map(|h| (rel.clone(), h))
        })
        .collect();
    for (rel, h) in hashed {
        out.insert(rel, h);
    }
    out
}
