//! `trace docs reset` — clears the current session's surfaced-docs state so a
//! subsequent `trace docs <path>` re-surfaces docs as new. This is the seam the
//! Codex compaction/clear hook drives: after a context reset drops injected
//! rule text from the model, the surfaced-docs state must reset so the rules
//! re-inject instead of being skipped as already-loaded.
//!
//! Pins the contract:
//!   - after surfacing docs then resetting, a follow-up surfacing returns the
//!     docs as new again (not in `already_loaded`)
//!   - the reset is a clean no-op when no session is active
//!   - the append-only `events.jsonl` history is preserved across the reset —
//!     only the materialized `view.json` is cleared
//!
//! No tracer internals are linked — the only contract is the CLI's observable
//! surface plus the documented on-disk shape.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use tracer_cli_tests::{trace_bin, Fixture};

#[allow(non_upper_case_globals)] // project naming rule bans ALL_CAPS for our own identifiers
static reset_seq: AtomicU64 = AtomicU64::new(0);

fn fresh_session_id(tag: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let seq = reset_seq.fetch_add(1, Ordering::SeqCst);
    format!("trace-docs-reset-{tag}-{nanos}-{seq}")
}

fn log_dir(repo_root: &Path, session_id: &str, agent_id: &str) -> PathBuf {
    repo_root
        .join(".tracer-cache")
        .join("sessions")
        .join(session_id)
        .join(agent_id)
}

fn read_events_jsonl(repo_root: &Path, session_id: &str, agent_id: &str) -> Vec<serde_json::Value> {
    let path = log_dir(repo_root, session_id, agent_id).join("events.jsonl");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return vec![];
    };
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("events.jsonl line not valid JSON"))
        .collect()
}

fn read_view(repo_root: &Path, session_id: &str, agent_id: &str) -> serde_json::Value {
    let path = log_dir(repo_root, session_id, agent_id).join("view.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("view.json missing at {}: {e}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("view.json is not valid JSON ({e}): {text}"))
}

fn docs_repo() -> Fixture {
    let f = Fixture::new();
    f.write("Claude.md", "# Root rules\n\nProject root.\n");
    f.write("sub/Claude.md", "# Sub rules\n\nThis dir has rules.\n");
    f.write(
        "sub/util.py",
        "def helper(v):\n    if v > 0:\n        return v + 1\n    return 0\n",
    );
    f.commit("init docs repo");
    f
}

// --- the core re-surface contract -----------------------------------------

#[test]
fn surfacing_after_reset_returns_docs_as_new_again() {
    // The defining behavior: surface docs, reset, surface again → the second
    // surfacing must return the full chain as `docs` (new), with nothing in
    // `already_loaded`. This is what makes rules re-inject after a Codex
    // compaction drops them from context.
    let f = docs_repo();
    let sid = fresh_session_id("re-surface");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    // 1. First surfacing: the full chain is new.
    let first = f.trace_env(&["docs", "sub/util.py", "--json"], &env);
    first.ok();
    assert_eq!(first.json()["doc_count"].as_i64().unwrap(), 2);

    // 2. Second surfacing without a reset: nothing new, both already_loaded.
    let pre_reset = f.trace_env(&["docs", "sub/util.py", "--json"], &env);
    pre_reset.ok();
    let v = pre_reset.json();
    assert_eq!(
        v["doc_count"].as_i64().unwrap(),
        0,
        "without a reset the chain is already in the log: {v}"
    );
    assert_eq!(v["already_loaded"].as_array().unwrap().len(), 2);

    // 3. Reset.
    let reset = f.trace_env(&["docs", "reset", "--json"], &env);
    reset.ok();
    let rv = reset.json();
    assert_eq!(rv["scope"].as_str().unwrap(), "reset", "{rv}");
    assert_eq!(rv["session_active"].as_bool().unwrap(), true, "{rv}");
    assert_eq!(
        rv["cleared_count"].as_i64().unwrap(),
        2,
        "reset must report the two surfaced docs it cleared: {rv}"
    );
    assert_eq!(rv["source"].as_str().unwrap(), "trace_docs_reset", "{rv}");

    // 4. Surfacing after the reset: the full chain is new AGAIN — nothing
    //    skipped as already_loaded.
    let after = f.trace_env(&["docs", "sub/util.py", "--json"], &env);
    after.ok();
    let av = after.json();
    assert_eq!(
        av["doc_count"].as_i64().unwrap(),
        2,
        "after a reset the chain must re-surface as new: {av}"
    );
    assert!(
        av.get("already_loaded").is_none(),
        "after a reset nothing must be reported already_loaded: {av}"
    );
}

#[test]
fn custom_source_lands_on_the_reset_response() {
    // The Codex hook stamps the calling surface via `--source`. The value
    // round-trips into the response so the hook can confirm its own call.
    let f = docs_repo();
    let sid = fresh_session_id("source");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    f.trace_env(&["docs", "sub/util.py", "--json"], &env).ok();

    let r = f.trace_env(
        &["docs", "reset", "--source", "codex_compact_hook", "--json"],
        &env,
    );
    r.ok();
    let v = r.json();
    assert_eq!(v["source"].as_str().unwrap(), "codex_compact_hook", "{v}");
    assert_eq!(v["cleared_count"].as_i64().unwrap(), 2, "{v}");
}

// --- no-op when no session is active --------------------------------------

#[test]
fn reset_is_a_clean_noop_when_no_session_is_active() {
    // Standalone tracer use: no AGENT_SESSION_ID / CODEX_THREAD_ID /
    // CLAUDE_CODE_SESSION_ID → the log no-ops, so reset succeeds with a
    // structured "nothing cleared" answer rather than erroring or writing any
    // cache.
    let f = docs_repo();
    let r = std::process::Command::new(trace_bin())
        .args(["docs", "reset", "--json"])
        .current_dir(&f.root)
        .env_remove("AGENT_SESSION_ID")
        .env_remove("CODEX_THREAD_ID")
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .output()
        .expect("spawn trace");
    assert!(
        r.status.success(),
        "standalone trace docs reset must succeed:\n{}",
        String::from_utf8_lossy(&r.stderr)
    );
    let stdout = String::from_utf8_lossy(&r.stdout);
    let v: serde_json::Value =
        serde_json::from_str(&stdout).expect("standalone reset must still return JSON");
    assert_eq!(v["scope"].as_str().unwrap(), "reset", "{v}");
    assert_eq!(
        v["session_active"].as_bool().unwrap(),
        false,
        "no session id ⇒ session_active false: {v}"
    );
    assert_eq!(
        v["cleared_count"].as_i64().unwrap(),
        0,
        "no session id ⇒ nothing cleared: {v}"
    );
    // The no-op must not have created any sessions directory.
    assert!(
        !f.root.join(".tracer-cache").join("sessions").exists(),
        "standalone reset must write no session log"
    );
}

#[test]
fn reset_before_anything_surfaces_clears_nothing() {
    // Active session but the log is empty (no docs surfaced yet) → reset is
    // still a clean no-op reporting zero cleared, and writes no view.
    let f = docs_repo();
    let sid = fresh_session_id("empty");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    let r = f.trace_env(&["docs", "reset", "--json"], &env);
    r.ok();
    let v = r.json();
    assert_eq!(v["session_active"].as_bool().unwrap(), true, "{v}");
    assert_eq!(
        v["cleared_count"].as_i64().unwrap(),
        0,
        "an empty log clears nothing: {v}"
    );
    assert!(
        read_events_jsonl(&f.root, &sid, "root").is_empty(),
        "reset on an empty log must append no event"
    );
}

// --- append-only history preserved ----------------------------------------

#[test]
fn reset_preserves_append_only_history_and_clears_only_the_view() {
    // The append-only `events.jsonl` is history; reset must never rewrite it.
    // After surface → reset → surface, the events log carries every event in
    // append order (2 doc_injection, 1 context_reset, 2 doc_injection), while
    // the materialized view.json holds only the re-surfaced set.
    let f = docs_repo();
    let sid = fresh_session_id("history");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    f.trace_env(&["docs", "sub/util.py", "--json"], &env).ok();
    let after_first = read_events_jsonl(&f.root, &sid, "root");
    assert_eq!(after_first.len(), 2, "first surfacing records two events");

    f.trace_env(&["docs", "reset", "--json"], &env).ok();
    f.trace_env(&["docs", "sub/util.py", "--json"], &env).ok();

    let events = read_events_jsonl(&f.root, &sid, "root");
    let kinds: Vec<&str> = events
        .iter()
        .map(|e| e["kind"].as_str().unwrap())
        .collect();
    assert_eq!(
        kinds,
        vec![
            "doc_injection",
            "doc_injection",
            "context_reset",
            "doc_injection",
            "doc_injection",
        ],
        "events.jsonl must preserve every event in append order across the reset: {events:?}"
    );

    // The reset event carries the source and a sha256 content hash like every
    // other event, with an empty path (it is not about one doc).
    let reset_event = &events[2];
    assert_eq!(reset_event["source"].as_str().unwrap(), "trace_docs_reset");
    assert_eq!(reset_event["path"].as_str().unwrap(), "");
    assert!(
        reset_event["content_hash"]
            .as_str()
            .unwrap()
            .starts_with("sha256:"),
        "reset event must carry a sha256 content hash: {reset_event}"
    );

    // The view holds only the two re-surfaced docs — not the pre-reset set
    // doubled. Reset cleared it; the post-reset surfacing repopulated it.
    let view = read_view(&f.root, &sid, "root");
    assert_eq!(
        view["emitted"].as_object().unwrap().len(),
        2,
        "view must hold only the re-surfaced set, not the accumulated total: {view}"
    );
}
