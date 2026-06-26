//! Session-context log — the third tracer cache namespace.
//!
//! `<repo>/.tracer-cache/sessions/<session_id>/<agent_id>/` holds:
//!   - `events.jsonl` — append-only event log, one JSON object per line
//!   - `view.json`    — materialized projection: emitted (canonical path → content hash)
//!   - `.lock`        — flock'd across read + append + materialize
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
//! harness layer — this module never writes the archived path, only reads
//! it as a fallback when the active path is absent. Writes
//! (`record_emission`, `record_read`) always target the active directory.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use super::drift;
use super::nested_memory::{self, LoadedMemory};
use crate::cache;

const AGENT_ID_DEFAULT: &str = "root";

/// Event kinds. Extensible by intent — Read tracking and future surfaces
/// add their own variants without breaking the on-disk JSONL shape (older
/// readers parse to `Value` and ignore unknown kinds).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    DocInjection,
    ReadFile,
    DirectorySurfaced,
    ContextPrimeDrift,
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

fn agent_id() -> String {
    std::env::var("TRACER_AGENT_ID")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| AGENT_ID_DEFAULT.to_string())
}

/// Worktree root resolved from the process cwd. `None` when cwd is not
/// inside any git worktree (or git is unavailable) — the second no-op
/// trigger that keeps standalone tracer use valid outside any repo. The
/// `worktree_root_for` resolver returns the linked worktree's own root
/// for paths inside a `git worktree add` checkout, so per-worktree caches
/// stay isolated from the main repo's cache.
fn repo_root() -> Option<PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    cache::worktree_root_for(&cwd)
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

/// Archived log directory for the current (session, agent).
/// Subagent stores are moved here on subagent stop by the
/// `archive_subagent_log.py` hook so the active sessions directory stays
/// bounded over a long-running orchestrator's lifetime. Reads fall back
/// here when the active directory is absent — writes never target this
/// path.
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

fn content_hash(content: &str) -> String {
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

/// Paths whose content the current (session, agent) log
/// already surfaced. Shape-compatible with the prior flat session-dedupe
/// set so call sites pass it straight into `nested_memory::load_for_file`.
/// Reads from the active log when present, else falls back
/// to the archived one.
pub fn loaded_paths() -> BTreeSet<String> {
    let Some(dir) = read_log_dir() else {
        return BTreeSet::new();
    };
    let view_path = dir.join("view.json");
    if !view_path.is_file() {
        return BTreeSet::new();
    }
    load_view(&view_path).emitted.into_keys().collect()
}

/// Record a doc-injection emission: append one event per memory not already
/// in the view (deduped by content hash), update the view, fsync via rename.
/// No-op when the session id is absent. Lock failures swallow — never blocks
/// the caller's render path.
pub fn record_emission(memories: &[LoadedMemory], source: &str) {
    if memories.is_empty() {
        return;
    }
    let Some(dir) = log_dir() else {
        return;
    };
    if fs::create_dir_all(&dir).is_err() {
        return;
    }

    let lock_path = dir.join(".lock");
    let lock_fh = match fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&lock_path)
    {
        Ok(fh) => fh,
        Err(_) => return,
    };
    let _ = rustix::fs::flock(&lock_fh, rustix::fs::FlockOperation::LockExclusive);

    let view_path = dir.join("view.json");
    let events_path = dir.join("events.jsonl");
    let mut view = load_view(&view_path);

    let now = unix_ms();
    let triggering_tool = std::env::var("TRACER_TRIGGERING_TOOL").ok();
    let triggering_command = std::env::var("TRACER_TRIGGERING_COMMAND").ok();

    let mut new_events: Vec<Event> = Vec::new();
    for m in memories {
        let hash = content_hash(&m.content);
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
        let _ = save_view(&view_path, &view);
    }

    let _ = rustix::fs::flock(&lock_fh, rustix::fs::FlockOperation::Unlock);
}

/// Record a `read_file` event for a path the agent just read, accumulating
/// which line range was read (`lines`, a 1-based inclusive `(start, end)`;
/// `None` means the whole file). Returns whether this is the file's first
/// surfacing in the session (so the caller can attach first-touch-only context
/// like the file's method list). No-op when the session id is absent or the
/// file is missing.
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
    lines: Option<(usize, usize)>,
) -> bool {
    let Some(dir) = log_dir() else {
        return true;
    };
    let Ok(content) = fs::read_to_string(file_path) else {
        return false;
    };
    if fs::create_dir_all(&dir).is_err() {
        return true;
    }

    let lock_path = dir.join(".lock");
    let lock_fh = match fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&lock_path)
    {
        Ok(fh) => fh,
        Err(_) => return true,
    };
    let _ = rustix::fs::flock(&lock_fh, rustix::fs::FlockOperation::LockExclusive);

    let view_path = dir.join("view.json");
    let events_path = dir.join("events.jsonl");
    let mut view = load_view(&view_path);

    let canonical = file_path
        .canonicalize()
        .unwrap_or_else(|_| file_path.to_path_buf())
        .to_string_lossy()
        .to_string();
    let hash = content_hash(&content);
    let total_lines = content.lines().count();

    let first_touch = view.emitted.get(&canonical) != Some(&hash);

    // Coverage: a content change (or first touch) resets the accumulator to
    // this read's range; an unchanged repeat read merges its range into the
    // running union. The range is the read tool's offset/limit translated to a
    // 1-based inclusive span, clamped to the file's real line count; `None`
    // covers the whole file (a shell `cat` records identically to a native
    // whole-file read).
    let (start, end) = match lines {
        Some((s, e)) => (s.max(1), e.min(total_lines)),
        None => (1, total_lines),
    };
    {
        let cov = view.coverage.entry(canonical.clone()).or_default();
        if first_touch || cov.total_lines != total_lines {
            cov.total_lines = total_lines;
            cov.read.clear();
        }
        if total_lines > 0 && start <= end {
            merge_range(&mut cov.read, start, end);
        }
    }

    if first_touch {
        view.emitted.insert(canonical.clone(), hash.clone());
        let event = Event {
            ts: unix_ms(),
            path: canonical,
            kind: EventKind::ReadFile,
            source: source.to_string(),
            size: content.len(),
            content_hash: hash,
            triggering_tool: std::env::var("TRACER_TRIGGERING_TOOL").ok(),
            triggering_command: std::env::var("TRACER_TRIGGERING_COMMAND").ok(),
            visible_as: file_path.to_string_lossy().to_string(),
        };
        let _ = append_events(&events_path, &[event]);
    }
    // Always persist: the coverage accumulator advances even when the emitted
    // projection (and thus the event log) is unchanged on a repeat read.
    let _ = save_view(&view_path, &view);

    let _ = rustix::fs::flock(&lock_fh, rustix::fs::FlockOperation::Unlock);
    first_touch
}

/// Record a directory's first surfacing in the session, returning whether
/// this is its first touch (so the caller can attach the one-level file
/// listing once). The directory is keyed in the same `view.emitted` map as
/// read files, under a fixed `dir:` marker hash — a directory has no content
/// to hash, and the marker keeps its key from ever colliding with a file's
/// content hash at the same path. Mirrors `record_read`'s lock discipline and
/// no-session semantics: with no active session every touch is a first touch.
pub fn record_directory_touch(dir_path: &std::path::Path, source: &str) -> bool {
    let Some(dir) = log_dir() else {
        return true;
    };
    if fs::create_dir_all(&dir).is_err() {
        return true;
    }

    let lock_path = dir.join(".lock");
    let lock_fh = match fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&lock_path)
    {
        Ok(fh) => fh,
        Err(_) => return true,
    };
    let _ = rustix::fs::flock(&lock_fh, rustix::fs::FlockOperation::LockExclusive);

    let view_path = dir.join("view.json");
    let events_path = dir.join("events.jsonl");
    let mut view = load_view(&view_path);

    let canonical = dir_path
        .canonicalize()
        .unwrap_or_else(|_| dir_path.to_path_buf())
        .to_string_lossy()
        .to_string();
    let marker = content_hash(&format!("dir:{canonical}"));

    let first_touch = view.emitted.get(&canonical) != Some(&marker);
    if first_touch {
        view.emitted.insert(canonical.clone(), marker.clone());
        let event = Event {
            ts: unix_ms(),
            path: canonical,
            kind: EventKind::DirectorySurfaced,
            source: source.to_string(),
            size: 0,
            content_hash: marker,
            triggering_tool: std::env::var("TRACER_TRIGGERING_TOOL").ok(),
            triggering_command: std::env::var("TRACER_TRIGGERING_COMMAND").ok(),
            visible_as: dir_path.to_string_lossy().to_string(),
        };
        let _ = append_events(&events_path, &[event]);
        let _ = save_view(&view_path, &view);
    }

    let _ = rustix::fs::flock(&lock_fh, rustix::fs::FlockOperation::Unlock);
    first_touch
}

/// Record a `context_prime_drift` event and reconcile the view to observed
/// reality. The view's `emitted` map is rewritten so any predicted path
/// not in the observed set is removed, and every observed path lands with
/// its real content hash from the input contract. The drift event itself
/// is appended once with the full diff payload — append-only history is
/// preserved while the view (the "what the context primer recorded" projection)
/// flips to what Claude Code actually injected.
///
/// No-op when the session id is absent. Lock failures swallow, matching
/// `record_emission`.
pub fn record_context_prime_drift(report: &drift::Report, observed: &drift::Observed, source: &str) {
    let Some(dir) = log_dir() else {
        return;
    };
    if fs::create_dir_all(&dir).is_err() {
        return;
    }

    let lock_path = dir.join(".lock");
    let lock_fh = match fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&lock_path)
    {
        Ok(fh) => fh,
        Err(_) => return,
    };
    let _ = rustix::fs::flock(&lock_fh, rustix::fs::FlockOperation::LockExclusive);

    let view_path = dir.join("view.json");
    let events_path = dir.join("events.jsonl");
    let mut view = load_view(&view_path);

    // Reconcile: drop predicted-but-not-observed entries, add observed-
    // but-not-predicted entries with their hook-supplied content hash.
    // Touch only context-primer paths (predicted ∪ observed) so any
    // ReadFile entries already in the view from earlier in the session
    // stay untouched — those are unrelated to the context primer.
    let touched: BTreeSet<String> = report
        .predicted
        .iter()
        .chain(report.observed.iter())
        .cloned()
        .collect();
    for path in &report.missing {
        if touched.contains(path) {
            view.emitted.remove(path);
        }
    }
    for doc in &observed.paths {
        view.emitted.insert(doc.path.clone(), doc.content_hash.clone());
    }

    let payload = serde_json::to_string(report).unwrap_or_else(|_| "{}".to_string());
    let event = Event {
        ts: unix_ms(),
        path: String::new(),
        kind: EventKind::ContextPrimeDrift,
        source: source.to_string(),
        size: payload.len(),
        content_hash: content_hash(&payload),
        triggering_tool: std::env::var("TRACER_TRIGGERING_TOOL").ok(),
        triggering_command: std::env::var("TRACER_TRIGGERING_COMMAND").ok(),
        visible_as: payload,
    };
    let _ = append_events(&events_path, &[event]);
    let _ = save_view(&view_path, &view);

    let _ = rustix::fs::flock(&lock_fh, rustix::fs::FlockOperation::Unlock);
}

/// Reset the surfaced-docs state for the current (session, agent): clear the
/// view's `emitted` map so a subsequent `trace docs` re-surfaces every doc as
/// new, and append one `context_reset` event recording the cleared set. The
/// append-only `events.jsonl` is preserved — only the materialized projection
/// is reconciled, mirroring `record_context_prime_drift`.
///
/// This is the seam the Codex compaction/clear hook drives: after a context
/// reset drops injected rule text from the model, the surfaced-docs state must
/// reset so the rules re-inject instead of being skipped as already-loaded.
///
/// Returns the number of paths cleared. A clean no-op (returns 0) when the
/// session id is absent, no repo root is resolvable, or nothing was surfaced —
/// keeping standalone tracer use valid. Lock failures swallow, matching
/// `record_emission`.
pub fn record_context_reset(source: &str) -> usize {
    let Some(dir) = log_dir() else {
        return 0;
    };
    let view_path = dir.join("view.json");
    // Nothing surfaced yet (no view on disk) → clean no-op, no event, no dir.
    if !view_path.is_file() {
        return 0;
    }

    let lock_path = dir.join(".lock");
    let lock_fh = match fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&lock_path)
    {
        Ok(fh) => fh,
        Err(_) => return 0,
    };
    let _ = rustix::fs::flock(&lock_fh, rustix::fs::FlockOperation::LockExclusive);

    let events_path = dir.join("events.jsonl");
    let mut view = load_view(&view_path);
    let cleared: Vec<String> = view.emitted.keys().cloned().collect();

    if cleared.is_empty() {
        let _ = rustix::fs::flock(&lock_fh, rustix::fs::FlockOperation::Unlock);
        return 0;
    }

    view.emitted.clear();
    // A context reset drops everything the agent had, including read state —
    // the coverage accumulator resets alongside the emitted projection.
    view.coverage.clear();

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
    let _ = append_events(&events_path, &[event]);
    let _ = save_view(&view_path, &view);

    let _ = rustix::fs::flock(&lock_fh, rustix::fs::FlockOperation::Unlock);
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

/// The materialized view as a JSON value — exposed for tests and future
/// query callers that want the projection directly. Reads from the active
/// log when present, else falls back to the archived one.
pub fn view() -> Value {
    let Some(dir) = read_log_dir() else {
        return json!({"emitted": {}});
    };
    let path = dir.join("view.json");
    serde_json::to_value(load_view(&path)).unwrap_or_else(|_| json!({"emitted": {}}))
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
        let size = ev
            .get("size")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;
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
