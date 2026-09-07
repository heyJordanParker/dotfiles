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
use crate::{memo, repo_context};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileFacts {
    pub path: String,
    pub language: Option<String>,
    pub loc: i64,
    pub function_count: i64,
    pub cyclomatic_complexity_total: i64,
    pub cyclomatic_complexity_max: i64,
    pub rank: String,
    pub functions: Vec<ccn::FunctionFact>,
    #[serde(skip)]
    pub last_modified: Option<String>,
    #[serde(skip)]
    pub last_author: Option<String>,
    #[serde(skip)]
    pub commits_30d: i64,
    #[serde(skip)]
    pub first_seen: Option<String>,
    #[serde(skip)]
    pub commit_count: i64,
    #[serde(skip)]
    pub commit_count_is_floor: bool,
    #[serde(skip)]
    pub rename_from: Option<String>,
    #[serde(skip)]
    pub working_state: Option<String>,
    #[serde(skip)]
    pub present_in: Vec<&'static str>,
    #[serde(skip)]
    pub last_subject: Option<String>,
    #[serde(skip)]
    pub top_author: Option<String>,
    #[serde(skip)]
    pub co_changed: Vec<(Arc<str>, i64)>,
    pub mtime_ns: i64,
    pub size_bytes: i64,
    pub extraction: Option<ExtractionResult>,
}

/// Files resolved per `get_batch` call. The bulk resolver returns every
/// input's whole `FileFacts` — extraction included — in one map, so a caller
/// spanning thousands of files would hold every declaration and reference in
/// all of them at once. Chunking bounds that to the chunk; the maps the
/// resolver hoists are memoized per repo root, so the only per-chunk cost is
/// re-reading the mtime index.
///
/// This is the one bound on how much of a repository is resident at a time.
/// Every caller that can span a whole repository — search enrichment and the
/// relations index build — walks its inputs through it.
pub const RESOLVE_CHUNK: usize = 512;

impl FileFacts {
    /// All scalar fields, then `extraction` appended last.
    pub fn to_json(&self) -> Value {
        serde_json::to_value(self).expect("FileFacts is serializable")
    }

    fn from_bytes(bytes: &[u8]) -> Option<FileFacts> {
        let facts: FileFacts = serde_json::from_slice(bytes).ok()?;
        if facts.rank != rank(facts.cyclomatic_complexity_total) {
            return None;
        }
        if facts.functions.is_empty() {
            if facts.function_count != 0 || facts.cyclomatic_complexity_max != 0 {
                return None;
            }
        } else {
            let total: i64 = facts
                .functions
                .iter()
                .map(|function| function.cyclomatic_complexity)
                .sum();
            let max = facts
                .functions
                .iter()
                .map(|function| function.cyclomatic_complexity)
                .max()
                .unwrap_or(0);
            if facts.function_count != facts.functions.len() as i64
                || facts.cyclomatic_complexity_total != total
                || facts.cyclomatic_complexity_max != max
            {
                return None;
            }
        }
        Some(facts)
    }
}

/// Join the git facts onto code facts. `git_activity` owns every one of them
/// and keys its cache by HEAD, so this runs on the cache-hit paths and on
/// fresh extraction alike. A path the map does not carry — no history, no
/// working-tree state, no deploy-branch presence — keeps the empty values,
/// which render as `no-history`.
fn with_git(mut facts: FileFacts, rel: &str, git_map: &HashMap<String, GitActivity>) -> FileFacts {
    let git = match git_map.get(rel) {
        Some(g) => g,
        None => return facts,
    };
    facts.last_modified = git.last_modified.clone();
    facts.last_author = git.last_author.clone();
    facts.commits_30d = git.commits_30d;
    facts.first_seen = git.first_seen.clone();
    facts.commit_count = git.commit_count;
    facts.commit_count_is_floor = git.commit_count_is_floor;
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
    scc_data: Option<&repo_context::FileMetrics>,
) -> (i64, i64, i64, i64) {
    if !functions.is_empty() {
        let ccn_total: i64 = functions.iter().map(|f| f.cyclomatic_complexity).sum();
        let ccn_max: i64 = functions
            .iter()
            .map(|f| f.cyclomatic_complexity)
            .max()
            .unwrap_or(0);
        let loc_sum: i64 = functions.iter().map(|f| f.nloc).sum();
        let scc_loc = scc_data.map(|data| data.loc).unwrap_or(0);
        let loc = if loc_sum != 0 { loc_sum } else { scc_loc };
        return (ccn_total, ccn_max, loc, functions.len() as i64);
    }
    let scc_ccn = scc_data.map(|data| data.ccn).unwrap_or(0);
    let scc_loc = scc_data.map(|data| data.loc).unwrap_or(0);
    (scc_ccn, 0, scc_loc, 0)
}

fn requires_scc(functions: &[ccn::FunctionFact], extraction: &Option<ExtractionResult>) -> bool {
    functions.is_empty()
        || functions.iter().map(|function| function.nloc).sum::<i64>() == 0
        || extraction.is_none()
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
    scc_data: Option<&repo_context::FileMetrics>,
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
                    let facts = ccn::facts_from_tree(&tree, source_bytes, lang_name);
                    // Reuse the tree for extraction whenever the extractor
                    // uses the same grammar `ccn::lang_for_path` resolved —
                    // every supported extension now shares its CCN grammar
                    // with its extractor, so this eliminates the second parse
                    // for all of them.
                    let lower = path_str.to_lowercase();
                    let extr = if extraction::is_supported(path) {
                        if lower.ends_with(".py") {
                            Some(extraction::python::extract_from_tree(&tree, source_bytes))
                        } else if lower.ends_with(".php") {
                            Some(extraction::php::extract_from_tree(&tree, source_bytes))
                        } else if lower.ends_with(".ts")
                            || lower.ends_with(".js")
                            || lower.ends_with(".tsx")
                            || lower.ends_with(".jsx")
                        {
                            let is_tsx = lower.ends_with(".tsx") || lower.ends_with(".jsx");
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

    let (ccn_total, ccn_max, loc, function_count) = ccn_scalars(&functions, scc_data);

    let (mtime_ns, size_bytes) = match fs::metadata(path) {
        Ok(md) => (mtime_ns_of(&md), md.len() as i64),
        Err(_) => (0, 0),
    };

    let language = extraction
        .as_ref()
        .map(|e| e.language.clone())
        .or_else(|| scc_data.map(|data| data.language.clone()));

    FileFacts {
        path: relative,
        language,
        loc,
        function_count,
        cyclomatic_complexity_total: ccn_total,
        cyclomatic_complexity_max: ccn_max,
        rank: rank(ccn_total).to_string(),
        functions,
        last_modified: git.last_modified.clone(),
        last_author: git.last_author.clone(),
        commits_30d: git.commits_30d,
        first_seen: git.first_seen.clone(),
        commit_count: git.commit_count,
        commit_count_is_floor: git.commit_count_is_floor,
        rename_from: git.rename_from.clone(),
        working_state: git.working_state.clone(),
        present_in: git.present_in.clone(),
        last_subject: git.last_subject.clone(),
        top_author: git.top_author.clone(),
        co_changed: git.co_changed.clone(),
        mtime_ns,
        size_bytes,
        extraction,
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

/// The stat fields the fast path trusts in place of hashing the bytes.
///
/// `mtime_ns` and `size` alone can all stay identical across a real content
/// change: `touch -r`, a timestamp-preserving copy, and two writes inside one
/// clock tick each produce a same-size file with a restored mtime, and the
/// index then serves the previous content's facts forever. `ctime_ns` closes
/// that — the kernel sets it on every inode write and no userland tool can
/// put it back — and `inode` closes the replace-by-rename case where a new
/// file takes the old path with a copied timestamp. All four come from the
/// one `stat` the caller already makes.
#[derive(Clone, Copy, PartialEq)]
struct Stamp {
    mtime_ns: i64,
    size: i64,
    ctime_ns: i64,
    inode: i64,
}

#[cfg(unix)]
fn stamp_of(md: &fs::Metadata) -> Stamp {
    use std::os::unix::fs::MetadataExt;
    Stamp {
        mtime_ns: mtime_ns_of(md),
        size: md.len() as i64,
        ctime_ns: md.ctime() as i64 * 1_000_000_000 + md.ctime_nsec() as i64,
        inode: md.ino() as i64,
    }
}

#[cfg(not(unix))]
fn stamp_of(md: &fs::Metadata) -> Stamp {
    Stamp {
        mtime_ns: mtime_ns_of(md),
        size: md.len() as i64,
        ctime_ns: md
            .created()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_nanos() as i64)
            .unwrap_or(0),
        inode: 0,
    }
}

/// One index row: the stamp plus the content key it was written for.
///
/// Typed rather than a `serde_json::Value`, for the reason the relations
/// index and the git-activity map are: every row read as a `Value` is five
/// boxed nodes, so a 22,702-file index cost more to hold than the file it
/// came from. A missing field defaults to `-1` so a row written by an older
/// binary, which carries no `ctime_ns` or `inode`, can never match a real
/// stamp.
#[derive(Clone, Serialize, Deserialize)]
struct IndexEntry {
    #[serde(default = "absent")]
    mtime_ns: i64,
    #[serde(default = "absent")]
    size: i64,
    #[serde(default = "absent")]
    ctime_ns: i64,
    #[serde(default = "absent")]
    inode: i64,
    #[serde(default)]
    key: String,
}

fn absent() -> i64 {
    -1
}

fn stamp_entry(s: &Stamp, key: &str) -> IndexEntry {
    IndexEntry {
        mtime_ns: s.mtime_ns,
        size: s.size,
        ctime_ns: s.ctime_ns,
        inode: s.inode,
        key: key.to_string(),
    }
}

/// The entry's cache key when every stamp field matches, else None — the one
/// predicate all three read paths share, so none of them can drift into
/// comparing fewer fields than the others.
fn stamp_matches<'a>(entry: &'a IndexEntry, s: &Stamp) -> Option<&'a str> {
    if entry.mtime_ns == s.mtime_ns
        && entry.size == s.size
        && entry.ctime_ns == s.ctime_ns
        && entry.inode == s.inode
        && !entry.key.is_empty()
    {
        return Some(&entry.key);
    }
    None
}

fn mtime_index_key() -> String {
    // Includes SCHEMA_VERSION so a binary upgrade that bumps the schema
    // also rotates this index — the per-file hashes the index serves are
    // schema-namespaced, so an index from the previous schema would point
    // at unreachable cache entries (or worse, hand the relations index a
    // stale content key and have it skip a file that really moved).
    format!(
        "mtime_index_v2__schema{}__{}",
        cache::SCHEMA_VERSION,
        cache::active_ccn_backend()
    )
}

/// Process-wide memo of the mtime index, keyed by repo root — the same memo
/// `git_activity::bulk_cached` and `repo_context::load_or_compute` keep for
/// the other two whole-repo maps this layer reads.
///
/// Without it, the index is the one bulk map still re-read and re-parsed per
/// file: `get` consults it on entry and `get_batch` loads it once, so a
/// command resolving N files one at a time paid N whole-index parses and, on
/// the content-hash path, N whole-index writes. That is the O(N²) storm
/// `get_batch`'s contract describes, and memoizing here removes it for every
/// caller rather than requiring each one to reach for the bulk resolver.
///
/// Writes go through `mtime_index_record` and `mtime_index_store`, which
/// update the memo in the same lock they write under, so a later read in the
/// same process never serves a superseded index.
type MtimeIndex = HashMap<String, IndexEntry>;

static MTIME_MEMO: memo::Memo<MtimeIndex> = OnceLock::new();

fn mtime_index_load(repo_root: &Path) -> Arc<MtimeIndex> {
    memo::get_or_build(&MTIME_MEMO, repo_root, || {
        cache::load_bytes(cache::NAMESPACE_FILE, &mtime_index_key(), repo_root)
            .and_then(|b| serde_json::from_slice::<MtimeIndex>(&b).ok())
            .unwrap_or_default()
    })
}

/// Persist `index` and make it the memo's current value under the same lock,
/// so no reader can observe the pre-write index after the write returns.
///
/// The index is wrapped for the save and unwrapped for the memo rather than
/// cloned for one of them: on a repo with thousands of entries this runs on
/// every single-file cache miss.
fn mtime_index_store(repo_root: &Path, index: MtimeIndex) {
    let key = mtime_index_key();
    if let Ok(document) = serde_json::to_value(&index) {
        let _ = cache::save(cache::NAMESPACE_FILE, &key, &document, repo_root);
    }
    // Same sweep the relations index and the git-activity map run: the key
    // carries a schema and a backend, so a bump rotates it and leaves the
    // superseded index in the namespace forever. next.js was carrying a
    // 4.4 MB orphan beside its live 5.7 MB index.
    cache::evict_prefixed(cache::NAMESPACE_FILE, "mtime_index_", &key, repo_root);
    memo::replace(&MTIME_MEMO, repo_root, index);
}

/// Facts for one file — a batch of one.
///
/// The three-step resolve (stat fast path, content-hash entry, fresh
/// extraction) lives once, in `get_batch`. This path only differs in that its
/// caller hands over a path a person typed, so it canonicalizes first;
/// `get_batch`'s own inputs are already tracked paths under the root.
pub fn get(path: &Path, repo_root: &Path) -> Option<FileFacts> {
    let p = path.canonicalize().ok()?;
    if !p.is_file() {
        return None;
    }
    get_batch(&[p], repo_root).into_values().next()
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
///   - supplied or complete git map, scc map, mtime index: loaded ONCE.
///   - in-memory mtime fast-path: unchanged files skip read+hash+extract.
///   - parallel extraction for true misses only.
///   - the mtime index is written ONCE at the end (no per-file rewrite,
///     no rayon write race), atomically via the existing cache::save.
/// Returns rel -> FileFacts for every readable input path.
pub fn get_batch(paths: &[PathBuf], repo_root: &Path) -> HashMap<String, FileFacts> {
    let can_persist = repo_root.join(".git").exists();
    let git_map = git_activity::bulk_cached(repo_root);
    let scc: OnceLock<Arc<repo_context::Payload>> = OnceLock::new();
    let index = mtime_index_load(repo_root);

    #[derive(Clone)]
    struct Job {
        abs: PathBuf,
        rel: String,
        git_rel: String,
        stamp: Stamp,
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
        let git_rel = abs
            .canonicalize()
            .ok()
            .and_then(|path| {
                path.strip_prefix(repo_root)
                    .ok()
                    .map(|path| path.to_string_lossy().to_string())
            })
            .unwrap_or_else(|| rel.clone());
        jobs.push(Job {
            rel,
            git_rel,
            stamp: stamp_of(&md),
            abs,
        });
    }

    // Parallel resolve. Each job yields (rel, FileFacts, Option<new index
    // entry>). Index entries are merged single-threaded afterward and the
    // index is persisted exactly once.
    let resolved: Vec<(String, FileFacts, Option<(String, Stamp, String)>)> = jobs
        .par_iter()
        .filter_map(|j| {
            // (1) in-memory stat fast-path — no I/O beyond the cached
            // entry load when the stamp matches the once-loaded index.
            if let Some(k) = index.get(&j.rel).and_then(|e| stamp_matches(e, &j.stamp)) {
                if let Some(bytes) = cache::load_bytes(cache::NAMESPACE_FILE, k, repo_root) {
                    if let Some(f) = FileFacts::from_bytes(&bytes) {
                        return Some((j.rel.clone(), with_git(f, &j.git_rel, &git_map), None));
                    }
                }
            }
            // (2) content-hash cache hit.
            let data = fs::read(&j.abs).ok()?;
            let key = cache::file_hash_from_bytes(&data, &j.abs, repo_root);
            if let Some(bytes) = cache::load_bytes(cache::NAMESPACE_FILE, &key, repo_root) {
                if let Some(f) = FileFacts::from_bytes(&bytes) {
                    return Some((
                        j.rel.clone(),
                        with_git(f, &j.git_rel, &git_map),
                        Some((j.rel.clone(), j.stamp, key)),
                    ));
                }
            }
            // (3) fresh real extraction (no lite-facts).
            let git = git_map
                .get(&j.git_rel)
                .cloned()
                .unwrap_or_else(GitActivity::empty);
            let scc_data = scc
                .get_or_init(|| repo_context::metrics(repo_root))
                .per_file
                .get(&j.git_rel);
            let facts = extract_facts(&j.abs, repo_root, &git, scc_data, &data);
            if !can_persist
                || (requires_scc(&facts.functions, &facts.extraction)
                    && !scc.get().unwrap().available)
            {
                return Some((j.rel.clone(), facts, None));
            }
            let _ = cache::save(cache::NAMESPACE_FILE, &key, &facts.to_json(), repo_root);
            Some((j.rel.clone(), facts, Some((j.rel.clone(), j.stamp, key))))
        })
        .collect();

    // Merge index updates and persist ONCE (was the O(N²) fsync storm).
    let mut new_index: Option<MtimeIndex> = None;
    let mut out = HashMap::with_capacity(resolved.len());
    for (rel, facts, upd) in resolved {
        if let Some((r, stamp, key)) = upd {
            new_index
                .get_or_insert_with(|| (*index).clone())
                .insert(r, stamp_entry(&stamp, &key));
        }
        out.insert(rel, facts);
    }
    if let Some(new_index) = new_index {
        mtime_index_store(repo_root, new_index);
    }
    out
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
pub(crate) fn resolve_under_root(p: &Path, repo_root: &Path) -> (PathBuf, String) {
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
        // One stat per path, the way `get_batch` resolves: `is_file()` and
        // `canonicalize()` were two further syscalls per file, and
        // `canonicalize` walks every path component. Over a 3,198-file
        // repository that cost more than the answer, and every caller of this
        // function pays it on every call — it is the freshness check.
        let (abs, rel) = resolve_under_root(p, repo_root);
        let md = match fs::metadata(&abs) {
            Ok(m) if m.is_file() => m,
            _ => continue,
        };
        let stamp = stamp_of(&md);
        if let Some(k) = idx.get(&rel).and_then(|e| stamp_matches(e, &stamp)) {
            out.insert(rel, k.to_string());
            continue;
        }
        misses.push((abs, rel));
    }
    let hashed: Vec<(String, String)> = misses
        .par_iter()
        .filter_map(|(p, rel)| {
            cache::file_hash(p, repo_root)
                .ok()
                .map(|h| (rel.clone(), h))
        })
        .collect();
    for (rel, h) in hashed {
        out.insert(rel, h);
    }
    out
}
