//! Session-context log — the third tracer cache namespace.
//!
//! `<repo>/.tracer-cache/sessions/<session_id>/<agent_id>/` holds:
//!   - `events.jsonl` — append-only event log, one JSON object per line
//!   - `view.json`    — materialized projection: emitted (canonical path → content hash)
//!   - `shown.json`   — what the Agent was last shown, by kind: each directory
//!     listing's hash and each file's facts (canonical path → value)
//!   - `.lock`        — flock'd across read + append + materialize, and by a
//!     `ShownRecord` from its first gate that must write until it is saved or
//!     dropped
//!
//! The single source of session-context state for the tracer. Replaces the
//! flat path-set dedupe that previously lived in `nested_memory.rs` and is
//! the surface every future session-context consumer (read tracking, drift
//! detection, doc graph) talks to.
//!
//! Session id resolution is reused verbatim from `nested_memory::session_id()`;
//! agent id comes from `TRACER_AGENT_ID` and defaults to `"root"`. The
//! store no-ops when the session id is absent OR when no repo root is
//! resolvable from the current working directory — both keep standalone
//! tracer use valid (no Claude-Code env wired in; not invoked inside a git
//! repo).
//!
//! Subagent stop archives the active log to
//! `<repo>/.tracer-cache/sessions/<session_id>/archived/<agent_id>/` via the
//! `archive_subagent_log.py` hook. The move is a directory rename at the
//! harness layer. Reads fall back to the archived log while the active one is
//! absent; the first write of a resumed Subagent renames it back, so its
//! record carries on whole. Writes always target the active directory.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use super::nested_memory::{self, LoadedMemory};
use crate::{cache, relations::DirectoryMetrics};

const AGENT_ID_DEFAULT: &str = "root";
const SHOWN: &str = "shown.json";

static DIRECTORY_BASELINES: OnceLock<Mutex<BTreeMap<String, DirectoryMetrics>>> = OnceLock::new();

/// What a text command shows an Agent once per context, until it changes: a
/// directory's entries, keyed by the directory, and a file's facts, keyed by
/// the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShownKind {
    Listing,
    Facts,
}

/// What a gated value is to the Agent beside the value it was last shown.
pub enum Shown {
    New,
    Same,
    /// Only the lines that changed, nested as in the value; a removed line
    /// is `null`.
    Changed(Map<String, Value>),
}

/// One call's gates and the values they found new or changed, not yet
/// recorded as shown. The first gate that must write takes the Agent's
/// `.lock`, and every later gate in the call compares under it, until the
/// record is saved or dropped, so a call beside it compares against what it
/// saves. Dropped unsaved, it writes nothing.
#[must_use = "save the record once the text that shows it is flushed"]
#[derive(Default)]
pub struct ShownRecord {
    values: Vec<(ShownKind, String, Value)>,
    held: Option<(PathBuf, fs::File)>,
}

impl ShownRecord {
    /// What `value` is to this Agent as `key`'s `kind` beside the value it
    /// was last shown since its context reset; a value not `Same` joins the
    /// record. Always `New` without a session, so standalone use shows
    /// everything every time. A gate that finds nothing changed before the
    /// record holds the lock reads one file and takes no lock. `value` is
    /// shaped as the front matter prints it, so each line `Changed` returns
    /// is one printed line, never part of a flow mapping.
    pub fn shown(&mut self, kind: ShownKind, key: &str, value: &Value) -> Shown {
        if log_dir().is_none() {
            return Shown::New;
        }
        let compare = |record: &BTreeMap<ShownKind, BTreeMap<String, Value>>| {
            match record.get(&kind).and_then(|keys| keys.get(key)) {
                None => Shown::New,
                Some(last) if last == value => Shown::Same,
                Some(last) => changed(last, value).map_or(Shown::New, Shown::Changed),
            }
        };
        if self.held.is_none() {
            if let Shown::Same = compare(&load_shown()) {
                return Shown::Same;
            }
            let Some(dir) = writable_log_dir() else {
                return Shown::New;
            };
            let Some(lock) = lock(&dir.join(".lock"), "lock session shown") else {
                return Shown::New;
            };
            self.held = Some((dir, lock));
        }
        // A concurrent call may have saved it before this record held the lock.
        let comparison = compare(&load_shown());
        if !matches!(comparison, Shown::Same) {
            self.values.push((kind, key.to_string(), value.clone()));
        }
        comparison
    }

    /// Record every value as shown. Call it only after the text that shows
    /// them is flushed, so a call that dies first records nothing.
    pub fn save(self) {
        let Some((dir, _lock)) = self.held else {
            return;
        };
        if self.values.is_empty() {
            return;
        }
        let mut record = load_shown();
        for (kind, key, value) in self.values {
            record.entry(kind).or_default().insert(key, value);
        }
        if let Ok(mut temp) = tempfile::Builder::new().prefix(".shown.").tempfile_in(&dir) {
            if temp.write_all(crate::jsonfmt::to_compact(&record).as_bytes()).is_ok() {
                let _ = crate::timing::phase("session shown", || temp.persist(dir.join(SHOWN)));
            }
        }
    }
}

/// Event kinds. Extensible by intent — Read tracking and future surfaces
/// add their own variants without breaking the on-disk JSONL shape (older
/// readers parse to `Value` and ignore unknown kinds).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    DocInjection,
    ReadFile,
    ContextReset,
}

/// One log event. Schema fields match the spec:
/// ts, path, kind, source, size, content_hash, triggering_tool,
/// triggering_command, visible_as.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub ts: u128,
    pub path: String,
    pub kind: EventKind,
    pub source: String,
    pub size: usize,
    pub content_hash: String,
    pub triggering_tool: Option<String>,
    pub triggering_command: Option<String>,
    pub visible_as: String,
}

/// Materialized view: emitted documents in this (session, agent) scope keyed
/// by canonical path → content hash. Anything already present is considered
/// already-surfaced; the same path with new content gets re-emitted.
///
/// `coverage` is the parallel per-file read-coverage accumulator keyed by the
/// same canonical path: how much of each file the agent has actually read this
/// session. `#[serde(default)]` keeps older `view.json` files (written before
/// coverage existed) loadable as an empty map.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct View {
    pub emitted: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub coverage: std::collections::BTreeMap<String, ReadCoverage>,
    /// The docs delivered whole at their emitted content, so reading part of
    /// one later never makes it partial.
    #[serde(default)]
    pub whole: BTreeSet<String>,
}

/// Accumulated line-read coverage for one file in this (session, agent) scope.
/// `total_lines` is the file's line count at the recorded content; `read` is
/// the union of every 1-based inclusive line range the agent has read, kept
/// sorted and disjoint so the covered-line count is the plain sum of each
/// range's length. The accumulator resets to the latest read when the file's
/// content changes (a new content state starts coverage fresh).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReadCoverage {
    pub total_lines: usize,
    pub read: Vec<[usize; 2]>,
}

impl ReadCoverage {
    /// Lines read so far — the sum of the disjoint ranges' lengths.
    pub fn lines_read(&self) -> usize {
        self.read.iter().map(|[s, e]| e - s + 1).sum()
    }

    /// Fraction of the file's lines read, in `[0.0, 1.0]`. A zero-line file is
    /// trivially fully read.
    pub fn fraction(&self) -> f64 {
        if self.total_lines == 0 {
            1.0
        } else {
            self.lines_read() as f64 / self.total_lines as f64
        }
    }

    pub fn complete(&self) -> bool {
        self.lines_read() >= self.total_lines
    }

    pub fn first_unread(&self) -> usize {
        match self.read.first() {
            Some([1, end]) => end + 1,
            _ => 1,
        }
    }
}

/// Merge a 1-based inclusive `[start, end]` line range into a sorted, disjoint
/// set of ranges, coalescing any overlap OR adjacency so the set stays minimal
/// and the covered-line count is the plain sum of each range's length. Reading
/// 1–60 then 40–80 yields `[[1, 80]]` (80 lines), never a double-counted 1–60
/// plus 40–80.
fn merge_range(ranges: &mut Vec<[usize; 2]>, start: usize, end: usize) {
    let mut merged_start = start;
    let mut merged_end = end;
    let mut out: Vec<[usize; 2]> = Vec::with_capacity(ranges.len() + 1);
    let mut inserted = false;
    for &[s, e] in ranges.iter() {
        if e + 1 < merged_start {
            // Existing range lies entirely before the new one — keep as-is.
            out.push([s, e]);
        } else if merged_end + 1 < s {
            // Existing range lies entirely after the merged one. Flush the
            // merged range once, then keep the rest.
            if !inserted {
                out.push([merged_start, merged_end]);
                inserted = true;
            }
            out.push([s, e]);
        } else {
            // Overlap or adjacency — absorb into the merged range.
            merged_start = merged_start.min(s);
            merged_end = merged_end.max(e);
        }
    }
    if !inserted {
        out.push([merged_start, merged_end]);
    }
    *ranges = out;
}

static AGENT: OnceLock<String> = OnceLock::new();

/// Name the agent from `--agent`, which a hook writes into a Subagent's own
/// `trace` commands: its shell carries the session id but no agent id.
pub fn set_agent(id: String) {
    let _ = AGENT.set(id);
}

fn agent_id() -> String {
    AGENT
        .get()
        .cloned()
        .or_else(|| std::env::var("TRACER_AGENT_ID").ok())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| AGENT_ID_DEFAULT.to_string())
}

static SESSION_HOME: OnceLock<PathBuf> = OnceLock::new();

/// Keep the session record where the session's own shell started, before
/// `-C` moves the working directory: the Hooks reset and archive it there.
pub fn set_session_home(dir: PathBuf) {
    let _ = SESSION_HOME.set(dir);
}

/// Worktree root of the session's directory: the one `-C` left behind, else
/// the process cwd. `None` when it is not inside any git worktree (or git is
/// unavailable) — the second no-op trigger that keeps standalone tracer use
/// valid outside any repo. The `worktree_root_for` resolver returns the
/// linked worktree's own root for paths inside a `git worktree add`
/// checkout, so per-worktree caches stay isolated from the main repo's cache.
fn repo_root() -> Option<PathBuf> {
    let home = match SESSION_HOME.get() {
        Some(dir) => dir.clone(),
        None => std::env::current_dir().ok()?,
    };
    cache::worktree_root_for(&home)
}

/// Active log directory for the current (session, agent).
/// `None` when the session id is absent or no repo root is resolvable —
/// the two no-op triggers. Writes always target this path.
fn log_dir() -> Option<PathBuf> {
    let sid = nested_memory::session_id()?;
    Some(
        repo_root()?
            .join(".tracer-cache")
            .join("sessions")
            .join(sid)
            .join(agent_id()),
    )
}

/// What a directory's imports were when this session first surfaced it, only
/// where they differ from `current`: the counts then, and the directories it
/// now imports from or no longer does. `None` when nothing changed. The first
/// call for a directory records its baseline, keyed by its absolute path so
/// two repositories' `src/` never share one.
pub fn at_session_start(repo_root: &Path, dir: &str, current: &DirectoryMetrics) -> Option<Map<String, Value>> {
    let baseline = directory_baseline(&repo_root.join(dir).to_string_lossy(), current);
    let mut since = Map::new();
    if baseline.imported_by != current.imported_by {
        since.insert("imported_by".into(), baseline.imported_by.into());
    }
    if baseline.imports != current.imports {
        since.insert("imports".into(), baseline.imports.into());
    }
    let added: Vec<&String> = current
        .imported_directories
        .difference(&baseline.imported_directories)
        .collect();
    let removed: Vec<&String> = baseline
        .imported_directories
        .difference(&current.imported_directories)
        .collect();
    if !added.is_empty() {
        since.insert("now_imports_from".into(), json!(added));
    }
    if !removed.is_empty() {
        since.insert("no_longer_imports_from".into(), json!(removed));
    }
    (!since.is_empty()).then_some(since)
}

/// The directory metrics captured when this session first surfaced each
/// directory. The file is deliberately session-wide: every agent reviewing
/// the same change sees the same before value.
pub fn directory_baseline(dir: &str, current: &DirectoryMetrics) -> DirectoryMetrics {
    let Some(session_id) = nested_memory::session_id() else {
        return current.clone();
    };
    let Some(repo_root) = repo_root() else {
        return current.clone();
    };
    let session_dir = repo_root
        .join(".tracer-cache")
        .join("sessions")
        .join(session_id);
    let path = session_dir.join("directories.json");
    let memo = DIRECTORY_BASELINES.get_or_init(|| Mutex::new(BTreeMap::new()));
    if let Some(baseline) = memo
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(dir)
        .cloned()
    {
        return baseline;
    }

    let mut stored: BTreeMap<String, DirectoryMetrics> = fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    if let Some(baseline) = stored.get(dir).cloned() {
        *memo.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = stored;
        return baseline;
    }
    if fs::create_dir_all(&session_dir).is_err() {
        return current.clone();
    }
    let Some(_lock) = lock(&session_dir.join(".lock"), "lock session directories") else {
        return current.clone();
    };
    stored = fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    if let Some(baseline) = stored.get(dir).cloned() {
        *memo.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = stored;
        return baseline;
    }
    let baseline = current.clone();
    stored.insert(dir.to_string(), baseline.clone());
    if let Ok(value) = serde_json::to_value(&stored) {
        if let Ok(mut temp) = tempfile::Builder::new()
            .prefix(".directories.")
            .tempfile_in(&session_dir)
        {
            if temp
                .write_all(crate::jsonfmt::to_compact(&value).as_bytes())
                .is_ok()
            {
                let _ = crate::timing::phase("session directories", || temp.persist(&path));
            }
        }
    }
    *memo.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = stored;
    baseline
}

/// The lines of `new` that differ from `old` when both are mappings: a
/// top-level key, or one key inside a mapping, the levels `yamlfmt::block`
/// prints one per line. Anything deeper is one line. `None` when either is
/// not a mapping.
fn changed(old: &Value, new: &Value) -> Option<Map<String, Value>> {
    let (Value::Object(old), Value::Object(new)) = (old, new) else {
        return None;
    };
    let leaves = |old: &Map<String, Value>, new: &Map<String, Value>| -> Map<String, Value> {
        new.keys()
            .chain(old.keys())
            .filter(|key| old.get(*key) != new.get(*key))
            .map(|key| (key.clone(), new.get(key).cloned().unwrap_or(Value::Null)))
            .collect()
    };
    let mut lines = leaves(old, new);
    for (key, line) in lines.iter_mut() {
        if let (Some(Value::Object(before)), Some(Value::Object(after))) = (old.get(key), new.get(key)) {
            *line = Value::Object(leaves(before, after));
        }
    }
    Some(lines)
}

/// Every value this Agent was last shown, by kind.
fn load_shown() -> BTreeMap<ShownKind, BTreeMap<String, Value>> {
    read_log_dir()
        .and_then(|dir| fs::read_to_string(dir.join(SHOWN)).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// The active log directory, ready for a write. A Subagent resumed after its
/// log was archived takes the archived log back first, so its first write
/// carries its record on rather than starting it over.
fn writable_log_dir() -> Option<PathBuf> {
    let dir = log_dir()?;
    if let Some(archived) = archived_log_dir().filter(|archived| !dir.is_dir() && archived.is_dir()) {
        let _ = fs::rename(archived, &dir);
    }
    fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// Holds the lock file at `path` until the returned handle drops, timed as
/// `phase`. Not re-entrant: a second take, in this process or another, waits
/// for the first to drop. `None` when the file cannot be opened or locked.
fn lock(path: &Path, phase: &str) -> Option<fs::File> {
    let file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
        .ok()?;
    crate::timing::phase(phase, || {
        rustix::fs::flock(&file, rustix::fs::FlockOperation::LockExclusive)
    })
    .ok()?;
    Some(file)
}

/// Archived log directory for the current (session, agent).
/// Subagent stores are moved here on subagent stop by the
/// `archive_subagent_log.py` hook so the active sessions directory stays
/// bounded over a long-running orchestrator's lifetime. Reads fall back
/// here when the active directory is absent, and `writable_log_dir` moves it
/// back before a write.
fn archived_log_dir() -> Option<PathBuf> {
    let sid = nested_memory::session_id()?;
    Some(
        repo_root()?
            .join(".tracer-cache")
            .join("sessions")
            .join(sid)
            .join("archived")
            .join(agent_id()),
    )
}

/// Directory to read this (session, agent)'s log from: the
/// active path when present, else the archived path. Returns `None` when
/// the session id is absent, no repo root is resolvable, or neither
/// directory exists.
fn read_log_dir() -> Option<PathBuf> {
    let active = log_dir()?;
    if active.is_dir() {
        return Some(active);
    }
    let archived = archived_log_dir()?;
    if archived.is_dir() {
        return Some(archived);
    }
    None
}

pub fn content_hash(content: &str) -> String {
    let digest = Sha256::digest(content.as_bytes());
    format!("sha256:{}", hex::encode(digest))
}

fn unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn load_view(path: &std::path::Path) -> View {
    fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str::<View>(&t).ok())
        .unwrap_or_default()
}

/// Atomic write: temp in same dir, rename into place. Mirrors `cache::save`.
fn save_view(path: &std::path::Path, view: &View) -> Result<()> {
    let parent = path.parent().expect("view path has a parent");
    let value = serde_json::to_value(view)?;
    let mut tmp = tempfile::Builder::new()
        .prefix(".view.")
        .tempfile_in(parent)?;
    tmp.write_all(crate::jsonfmt::to_compact(&value).as_bytes())?;
    tmp.persist(path).map_err(|e| anyhow::anyhow!(e))?;
    Ok(())
}

fn append_events(path: &std::path::Path, events: &[Event]) -> Result<()> {
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    for e in events {
        let line = crate::jsonfmt::to_compact(&serde_json::to_value(e)?);
        writeln!(f, "{line}")?;
    }
    Ok(())
}

/// Whether a session id is currently resolvable. `false` means the
/// log is in no-op mode (standalone tracer use) — surfaces
/// that report on it render an empty-but-valid response rather than
/// failing.
pub fn session_active() -> bool {
    nested_memory::session_id().is_some()
}

/// Paths the current (session, agent) holds whole: surfaced, and either sent
/// whole or read to their last line. A doc seen only in part is not loaded.
/// Shape-compatible with the prior flat session-dedupe set so call sites pass
/// it straight into `nested_memory::load_for_file`. Reads from the active log
/// when present, else falls back to the archived one.
pub fn loaded_paths() -> BTreeSet<String> {
    let View { emitted, coverage, whole } = current_view();
    emitted
        .into_keys()
        .filter(|path| whole.contains(path) || coverage.get(path).is_none_or(ReadCoverage::complete))
        .collect()
}

/// Where the Agent stopped in each path it read in part: the content hash it
/// read and the lines it has.
pub fn partial_reads() -> BTreeMap<String, (String, ReadCoverage)> {
    let View { emitted, coverage, whole } = current_view();
    coverage
        .into_iter()
        .filter(|(path, read)| !read.complete() && !whole.contains(path))
        .filter_map(|(path, read)| emitted.get(&path).map(|hash| (path, (hash.clone(), read))))
        .collect()
}

/// Holds this (session, agent)'s doc delivery to one call at a time, from the
/// read of what it holds to the record of what it sent, so calls running side
/// by side send the next lines rather than the same ones. The lock sits beside
/// the agent's log directory, so taking it never re-creates an archived log.
/// Released on drop; `None` with no session.
pub fn delivery_lock() -> Option<fs::File> {
    let session = log_dir()?.parent()?.to_path_buf();
    fs::create_dir_all(&session).ok()?;
    lock(&session.join(format!("{}.delivery.lock", agent_id())), "lock doc delivery")
}

fn current_view() -> View {
    read_log_dir()
        .map(|dir| load_view(&dir.join("view.json")))
        .unwrap_or_default()
}

pub fn has_read(file_path: &std::path::Path, content_hash: &str, start: usize, end: usize) -> bool {
    let Some(dir) = read_log_dir() else {
        return false;
    };
    let view = load_view(&dir.join("view.json"));
    let canonical = file_path
        .canonicalize()
        .unwrap_or_else(|_| file_path.to_path_buf())
        .to_string_lossy()
        .to_string();
    view.emitted.get(&canonical).map(String::as_str) == Some(content_hash)
        && view
            .coverage
            .get(&canonical)
            .is_some_and(|coverage| coverage.read.iter().any(|[s, e]| *s <= start && end <= *e))
}

/// Record a doc-injection emission: append one event per memory not already
/// in the view (deduped by content hash), update the view, fsync via rename.
/// No-op when the session id is absent. Lock failures swallow — never blocks
/// the caller's render path.
pub fn record_emission(memories: &[LoadedMemory], source: &str) {
    if memories.is_empty() {
        return;
    }
    let Some(dir) = writable_log_dir() else {
        return;
    };
    let Some(_lock) = lock(&dir.join(".lock"), "lock session docs") else {
        return;
    };

    let view_path = dir.join("view.json");
    let events_path = dir.join("events.jsonl");
    let mut view = load_view(&view_path);

    let now = unix_ms();
    let triggering_tool = std::env::var("TRACER_TRIGGERING_TOOL").ok();
    let triggering_command = std::env::var("TRACER_TRIGGERING_COMMAND").ok();

    let mut new_events: Vec<Event> = Vec::new();
    let mut completed = false;
    for m in memories {
        let hash = content_hash(&m.content);
        completed |= view.whole.insert(m.path.clone());
        if view.emitted.get(&m.path) == Some(&hash) {
            continue;
        }
        view.emitted.insert(m.path.clone(), hash.clone());
        new_events.push(Event {
            ts: now,
            path: m.path.clone(),
            kind: EventKind::DocInjection,
            source: source.to_string(),
            size: m.size,
            content_hash: hash,
            triggering_tool: triggering_tool.clone(),
            triggering_command: triggering_command.clone(),
            visible_as: m.relative_path.clone(),
        });
    }

    if !new_events.is_empty() {
        let _ = append_events(&events_path, &new_events);
    }
    if !new_events.is_empty() || completed {
        let _ = save_view(&view_path, &view);
    }
}

/// Record a `read_file` event for a path the agent just read, accumulating
/// which line range was read (`lines`, a 1-based inclusive `(start, end)`;
/// `None` means the whole file). No-op when the session id is absent.
/// Captured source identity, byte count, and line count come from its caller,
/// so bookkeeping never reopens a file after that caller delivered it.
///
/// Two states coexist. The `emitted` projection dedups by content hash —
/// mirroring `record_emission`, a follow-up doc-injection or read against the
/// same content appends no second event and the `first_touch` return is
/// `false`. The `coverage` accumulator, by contrast, merges every read's range
/// into the running union even on a repeat read of the same content, so reading
/// 1–50 then 51–100 reaches 100% rather than collapsing to a single touch. A
/// content change (new hash) resets coverage to the latest read.
///
/// First-touch semantics: a newly-inserted (path, hash) is the first
/// surfacing → `true`. An unchanged repeat read is not → `false`. With no
/// active session there is no log to dedup against, so every touch is a
/// first touch → `true`, keeping standalone `trace context <file>` fully
/// informative.
pub fn record_read(
    file_path: &std::path::Path,
    source: &str,
    content_hash: &str,
    content_size: usize,
    total_lines: usize,
    spans: &[Option<(usize, usize)>],
) {
    let Some(dir) = writable_log_dir() else {
        return;
    };
    let Some(_lock) = lock(&dir.join(".lock"), "lock session view") else {
        return;
    };

    let view_path = dir.join("view.json");
    let events_path = dir.join("events.jsonl");
    let mut view = load_view(&view_path);

    let canonical = file_path
        .canonicalize()
        .unwrap_or_else(|_| file_path.to_path_buf())
        .to_string_lossy()
        .to_string();
    let hash = content_hash.to_string();

    let first_touch = view.emitted.get(&canonical) != Some(&hash);

    // Coverage: a content change (or first touch) resets the accumulator to
    // this read's range; an unchanged repeat read merges its range into the
    // running union. The range is the read tool's offset/limit translated to a
    // 1-based inclusive span, clamped to the file's real line count; `None`
    // covers the whole file (a shell `cat` records identically to a native
    // whole-file read).
    {
        let cov = view.coverage.entry(canonical.clone()).or_default();
        if first_touch || cov.total_lines != total_lines {
            cov.total_lines = total_lines;
            cov.read.clear();
        }
        for span in spans {
            let (start, end) = match span {
                Some((s, e)) => ((*s).max(1), (*e).min(total_lines)),
                None => (1, total_lines),
            };
            if total_lines > 0 && start <= end {
                merge_range(&mut cov.read, start, end);
            }
        }
    }

    if first_touch {
        view.whole.remove(&canonical);
        view.emitted.insert(canonical.clone(), hash.clone());
        let event = Event {
            ts: unix_ms(),
            path: canonical,
            kind: EventKind::ReadFile,
            source: source.to_string(),
            size: content_size,
            content_hash: hash,
            triggering_tool: std::env::var("TRACER_TRIGGERING_TOOL").ok(),
            triggering_command: std::env::var("TRACER_TRIGGERING_COMMAND").ok(),
            visible_as: file_path.to_string_lossy().to_string(),
        };
        let _ = append_events(&events_path, &[event]);
    }
    // Always persist: the coverage accumulator advances even when the emitted
    // projection (and thus the event log) is unchanged on a repeat read.
    let _ = crate::timing::phase("session view", || save_view(&view_path, &view));
}

/// Clear everything the current (session, agent) record says the Agent holds,
/// because its context dropped it: every kind it was shown once, and the docs
/// view's `emitted`, `coverage`, and `whole`, so the next call shows each
/// again. One `context_reset` event records the docs cleared; the append-only
/// `events.jsonl` is preserved.
///
/// This is the seam the compaction and clear hooks drive.
///
/// Returns the number of docs cleared. A clean no-op that writes nothing when
/// no session is active or the Agent has no record yet. Lock failures swallow,
/// matching `record_emission`.
pub fn record_context_reset(source: &str) -> usize {
    if read_log_dir().is_none() {
        return 0;
    }
    let Some(dir) = writable_log_dir() else {
        return 0;
    };
    let Some(_lock) = lock(&dir.join(".lock"), "lock session reset") else {
        return 0;
    };
    let _ = fs::remove_file(dir.join(SHOWN));

    let view_path = dir.join("view.json");
    let cleared: Vec<String> = load_view(&view_path).emitted.into_keys().collect();
    if cleared.is_empty() {
        return 0;
    }

    let payload = serde_json::to_string(&cleared).unwrap_or_else(|_| "[]".to_string());
    let event = Event {
        ts: unix_ms(),
        path: String::new(),
        kind: EventKind::ContextReset,
        source: source.to_string(),
        size: payload.len(),
        content_hash: content_hash(&payload),
        triggering_tool: std::env::var("TRACER_TRIGGERING_TOOL").ok(),
        triggering_command: std::env::var("TRACER_TRIGGERING_COMMAND").ok(),
        visible_as: payload,
    };
    let _ = append_events(&dir.join("events.jsonl"), &[event]);
    let _ = save_view(&view_path, &View::default());
    cleared.len()
}

/// All events in the current (session, agent) log, in
/// append order. Surface for future consumers (drift detector, doc graph).
/// Tests use it to pin event schema. Reads from the active
/// log when present, else falls back to the archived one.
pub fn events() -> Vec<Value> {
    let Some(dir) = read_log_dir() else {
        return vec![];
    };
    let path = dir.join("events.jsonl");
    let Ok(text) = fs::read_to_string(&path) else {
        return vec![];
    };
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .collect()
}

/// One entry in the session's current "what the agent has" manifest.
/// Combines the projection (`view.json` — canonical path + content hash)
/// with the events-log attribution (most recent `source`, `kind`,
/// `visible_as`, `size`). Empty when the session id is absent.
#[derive(Debug, Clone, Serialize)]
pub struct LoadedEntry {
    pub path: String,
    pub visible_as: String,
    pub kind: String,
    pub size: usize,
    pub content_hash: String,
    pub source: String,
    /// File's line count at the recorded content; `0` when it was surfaced by
    /// doc-injection only (never read as a file).
    pub total_lines: usize,
    /// Lines the agent has read this session — the union of every read range.
    pub lines_read: usize,
    /// Fraction of the file's lines read, in `[0.0, 1.0]`. `0.0` for a file
    /// that was surfaced but never read.
    pub read_fraction: f64,
}

/// Every path the (session, agent) log has surfaced, joined
/// against the events log so each entry carries its latest `source`,
/// `kind`, `size`, and `visible_as`. Most recent event for a path wins on
/// source — matches `commands::docs::prior_source_map`'s semantics so
/// status and load agree on attribution. Returns an empty vec when the
/// session is absent.
pub fn loaded_entries() -> Vec<LoadedEntry> {
    let Some(dir) = read_log_dir() else {
        return vec![];
    };
    let view_path = dir.join("view.json");
    if !view_path.is_file() {
        return vec![];
    }
    let view = load_view(&view_path);
    let coverage = view.coverage;

    // Build path -> latest event attribution from the append-only log. One
    // pass; later events overwrite earlier ones for the same path. Drift
    // events have an empty path and contribute no attribution.
    let mut latest: std::collections::BTreeMap<String, (String, String, usize, String)> =
        std::collections::BTreeMap::new();
    for ev in events() {
        let path = match ev.get("path").and_then(|p| p.as_str()) {
            Some(p) if !p.is_empty() => p.to_string(),
            _ => continue,
        };
        let source = ev
            .get("source")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        let kind = ev
            .get("kind")
            .and_then(|v| v.as_str())
            .unwrap_or("doc_injection")
            .to_string();
        let size = ev.get("size").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let visible_as = ev
            .get("visible_as")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        latest.insert(path, (source, kind, size, visible_as));
    }

    let mut out: Vec<LoadedEntry> = Vec::with_capacity(view.emitted.len());
    for (path, content_hash) in view.emitted {
        let (source, kind, size, visible_as) = latest
            .get(&path)
            .cloned()
            .unwrap_or_else(|| ("unknown".into(), "doc_injection".into(), 0, path.clone()));
        let cov = coverage.get(&path);
        out.push(LoadedEntry {
            path,
            visible_as,
            kind,
            size,
            content_hash,
            source,
            total_lines: cov.map(|c| c.total_lines).unwrap_or(0),
            lines_read: cov.map(|c| c.lines_read()).unwrap_or(0),
            read_fraction: cov.map(|c| c.fraction()).unwrap_or(0.0),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Coverage from a single sequence of merged reads — the pure
    /// interval-union math the session log accumulates per file.
    fn cov(total: usize, reads: &[(usize, usize)]) -> ReadCoverage {
        let mut c = ReadCoverage {
            total_lines: total,
            read: Vec::new(),
        };
        for &(s, e) in reads {
            merge_range(&mut c.read, s, e);
        }
        c
    }

    #[test]
    fn single_partial_read_is_half() {
        let c = cov(100, &[(1, 50)]);
        assert_eq!(c.lines_read(), 50);
        assert!((c.fraction() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn two_non_overlapping_reads_reach_full() {
        let c = cov(100, &[(1, 50), (51, 100)]);
        assert_eq!(c.lines_read(), 100);
        assert!((c.fraction() - 1.0).abs() < 1e-9);
        // Adjacent ranges coalesce into one minimal span.
        assert_eq!(c.read, vec![[1, 100]]);
    }

    #[test]
    fn overlapping_reads_count_the_union_not_double() {
        let c = cov(100, &[(1, 60), (40, 80)]);
        assert_eq!(c.lines_read(), 80, "1–60 ∪ 40–80 is 80 lines, not 101");
        assert_eq!(c.read, vec![[1, 80]]);
    }

    #[test]
    fn whole_file_read_is_full() {
        let c = cov(100, &[(1, 100)]);
        assert!((c.fraction() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn never_read_file_is_zero() {
        let c = cov(100, &[]);
        assert_eq!(c.lines_read(), 0);
        assert_eq!(c.fraction(), 0.0);
    }

    #[test]
    fn out_of_order_reads_with_a_gap_stay_disjoint() {
        // 51–100 then 1–50: the merge keeps the set sorted and, since they are
        // adjacent, coalesces to one span. A genuine gap stays two spans.
        assert_eq!(cov(100, &[(51, 100), (1, 50)]).read, vec![[1, 100]]);
        let gapped = cov(100, &[(1, 10), (90, 100)]);
        assert_eq!(gapped.read, vec![[1, 10], [90, 100]]);
        assert_eq!(gapped.lines_read(), 21);
    }
}
