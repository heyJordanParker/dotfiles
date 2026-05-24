//! Session-context log: the on-disk contract for the third
//! tracer cache namespace at `<repo>/.tracer-cache/sessions/<sid>/<aid>/`.
//!
//! Black-box: assertions go through the CLI surface (`trace docs`,
//! `trace read --docs`) and the documented on-disk shape (events.jsonl +
//! view.json). Pre-existing cross-command dedupe behavior is pinned in
//! `per_file_commands.rs`; this file covers the log's own
//! contract — event schema, materialized view, dedup-by-content-hash,
//! per-agent isolation, and concurrent-writer safety.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use tracer_cli_tests::{trace_bin, Fixture};

#[allow(non_upper_case_globals)] // project naming rule bans ALL_CAPS for our own identifiers
static log_seq: AtomicU64 = AtomicU64::new(0);

fn fresh_session_id(tag: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let seq = log_seq.fetch_add(1, Ordering::SeqCst);
    format!("trace-log-test-{tag}-{nanos}-{seq}")
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
        .map(|l| {
            serde_json::from_str(l).unwrap_or_else(|e| {
                panic!("events.jsonl line is not valid JSON ({e}): {l}")
            })
        })
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

// --- append + materialize --------------------------------------------------

#[test]
fn docs_command_appends_events_and_materializes_view() {
    let f = docs_repo();
    let sid = fresh_session_id("append");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    f.trace_env(&["docs", "sub/util.py"], &env).ok();

    let events = read_events_jsonl(&f.root, &sid, "root");
    assert_eq!(
        events.len(),
        2,
        "expected one event per surfaced doc (root + sub), got {}: {:?}",
        events.len(),
        events
    );
    for ev in &events {
        for key in [
            "ts",
            "path",
            "kind",
            "source",
            "size",
            "content_hash",
            "visible_as",
        ] {
            assert!(
                ev.get(key).is_some(),
                "event missing required field `{key}`: {ev}"
            );
        }
        assert_eq!(ev["kind"], "doc_injection", "{ev}");
        assert_eq!(ev["source"], "trace_docs", "{ev}");
        assert!(
            ev["content_hash"].as_str().unwrap().starts_with("sha256:"),
            "content_hash must be sha256-prefixed: {ev}"
        );
    }

    let view = read_view(&f.root, &sid, "root");
    let emitted = view["emitted"].as_object().unwrap();
    assert_eq!(
        emitted.len(),
        2,
        "view must hold one entry per surfaced doc, got {:?}",
        emitted
    );
    let visible: BTreeSet<&str> = events
        .iter()
        .map(|e| e["visible_as"].as_str().unwrap())
        .collect();
    assert!(
        visible.contains("Claude.md") && visible.contains("sub/Claude.md"),
        "expected both Claude.md ancestors emitted, got {visible:?}"
    );
}

// --- dedup against prior events --------------------------------------------

#[test]
fn second_invocation_does_not_re_record_already_surfaced_docs() {
    let f = docs_repo();
    let sid = fresh_session_id("dedup");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    f.trace_env(&["docs", "sub/util.py"], &env).ok();
    let first = read_events_jsonl(&f.root, &sid, "root");
    assert_eq!(first.len(), 2);
    let first_view = read_view(&f.root, &sid, "root");
    let first_emitted = first_view["emitted"].as_object().unwrap().clone();

    // Same session → walk-up suppresses already-surfaced docs, log
    // appends nothing, view is unchanged.
    f.trace_env(&["docs", "sub/util.py"], &env).ok();
    let after_second = read_events_jsonl(&f.root, &sid, "root");
    assert_eq!(
        after_second.len(),
        2,
        "already-surfaced docs must not re-record: events grew {} → {}",
        first.len(),
        after_second.len()
    );
    let second_view = read_view(&f.root, &sid, "root");
    assert_eq!(
        second_view["emitted"].as_object().unwrap(),
        &first_emitted,
        "view must remain stable across repeated invocations"
    );
}

// --- per-agent isolation ---------------------------------------------------

#[test]
fn different_agents_in_one_session_keep_separate_logs() {
    let f = docs_repo();
    let sid = fresh_session_id("agents");
    let env_alpha = [
        ("CLAUDE_CODE_SESSION_ID", sid.as_str()),
        ("TRACER_AGENT_ID", "alpha"),
    ];
    let env_beta = [
        ("CLAUDE_CODE_SESSION_ID", sid.as_str()),
        ("TRACER_AGENT_ID", "beta"),
    ];

    // Alpha surfaces docs first.
    let alpha_first = f.trace_env(&["docs", "sub/util.py"], &env_alpha);
    alpha_first.ok();
    assert!(alpha_first.stdout.contains("Root rules"), "{}", alpha_first.stdout);

    // Beta in the same session must still see the docs — its log is
    // independent and starts empty.
    let beta_first = f.trace_env(&["docs", "sub/util.py"], &env_beta);
    beta_first.ok();
    assert!(
        beta_first.stdout.contains("Root rules"),
        "beta agent must surface docs independently:\n{}",
        beta_first.stdout
    );

    // Each agent's log is in its own directory.
    let alpha_events = read_events_jsonl(&f.root, &sid, "alpha");
    let beta_events = read_events_jsonl(&f.root, &sid, "beta");
    assert_eq!(alpha_events.len(), 2, "alpha events: {alpha_events:?}");
    assert_eq!(beta_events.len(), 2, "beta events: {beta_events:?}");
    // The `root` default bucket must not have been touched by either agent.
    assert!(
        !log_dir(&f.root, &sid, "root").exists(),
        "agent-scoped runs must not write to the root bucket"
    );

    // Second alpha run is deduped against alpha's prior events, not beta's.
    f.trace_env(&["docs", "sub/util.py"], &env_alpha).ok();
    assert_eq!(read_events_jsonl(&f.root, &sid, "alpha").len(), 2);
    assert_eq!(read_events_jsonl(&f.root, &sid, "beta").len(), 2);
}

#[test]
fn missing_agent_id_defaults_to_root_bucket() {
    let f = docs_repo();
    let sid = fresh_session_id("default-agent");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    f.trace_env(&["docs", "sub/util.py"], &env).ok();

    assert!(
        log_dir(&f.root, &sid, "root").is_dir(),
        "absent TRACER_AGENT_ID must default to the `root` bucket"
    );
    assert_eq!(read_events_jsonl(&f.root, &sid, "root").len(), 2);
}

// --- concurrent writers (flock contract) -----------------------------------

#[test]
fn concurrent_writers_produce_no_corruption() {
    // 8 same-session parallel `trace docs` invocations against the same
    // log directory. The flock'd read+append+materialize
    // must keep events.jsonl line-valid and view.json single-object-valid.
    let f = docs_repo();
    let sid = fresh_session_id("concurrent");
    let bin = trace_bin();
    let root = f.root.clone();

    let mut handles = Vec::new();
    for _ in 0..8 {
        let bin = bin.clone();
        let root = root.clone();
        let sid = sid.clone();
        handles.push(thread::spawn(move || {
            let out = Command::new(&bin)
                .args(["docs", "sub/util.py"])
                .current_dir(&root)
                .env("CLAUDE_CODE_SESSION_ID", &sid)
                .output()
                .expect("spawn trace");
            assert!(
                out.status.success(),
                "trace docs failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }));
    }
    for h in handles {
        h.join().unwrap();
    }

    let events = read_events_jsonl(&f.root, &sid, "root");
    // Every line parsed as JSON via read_events_jsonl — file is line-valid.
    // Dedup-by-hash means at most 2 events (root + sub Claude.md). The
    // observed count is 1 ≤ n ≤ 2 depending on whether any racer raced
    // through after another already materialized the view.
    assert!(
        events.len() <= 2,
        "concurrent dedupe failed — got {} events, expected ≤ 2: {:?}",
        events.len(),
        events
    );
    assert!(
        !events.is_empty(),
        "at least one writer must have appended"
    );

    let view = read_view(&f.root, &sid, "root");
    let emitted = view["emitted"].as_object().expect("view.emitted is object");
    assert!(
        emitted.len() <= 2 && !emitted.is_empty(),
        "view.emitted size out of range: {:?}",
        emitted
    );
}

// --- read_file event (cross-tool dedup foundation) -------------------------

#[test]
fn context_file_arg_records_read_file_event() {
    let f = docs_repo();
    let sid = fresh_session_id("read");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    f.trace_env(&["context", "sub/util.py"], &env).ok();

    let events = read_events_jsonl(&f.root, &sid, "root");
    let read_events: Vec<&serde_json::Value> = events
        .iter()
        .filter(|e| e["kind"] == "read_file")
        .collect();
    assert_eq!(
        read_events.len(),
        1,
        "expected one read_file event for the file-arg context call, got: {events:?}"
    );
    let event = read_events[0];
    for key in ["ts", "path", "kind", "source", "size", "content_hash", "visible_as"] {
        assert!(
            event.get(key).is_some(),
            "read_file event missing required field `{key}`: {event}"
        );
    }
    assert_eq!(event["source"], "agent_read", "{event}");
    assert!(
        event["content_hash"].as_str().unwrap().starts_with("sha256:"),
        "content_hash must be sha256-prefixed: {event}"
    );
    assert!(
        event["path"].as_str().unwrap().ends_with("sub/util.py"),
        "path must point at the file just read: {event}"
    );

    let view = read_view(&f.root, &sid, "root");
    let emitted = view["emitted"].as_object().unwrap();
    let matches: Vec<&String> = emitted
        .keys()
        .filter(|k| k.ends_with("sub/util.py"))
        .collect();
    assert_eq!(
        matches.len(),
        1,
        "view must record the read file path, got: {emitted:?}"
    );
}

#[test]
fn second_context_call_does_not_re_record_unchanged_read() {
    let f = docs_repo();
    let sid = fresh_session_id("read-dedup");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    f.trace_env(&["context", "sub/util.py"], &env).ok();
    let first = read_events_jsonl(&f.root, &sid, "root");
    let first_reads = first.iter().filter(|e| e["kind"] == "read_file").count();
    assert_eq!(first_reads, 1);

    // Same content + same session: no new read_file event.
    f.trace_env(&["context", "sub/util.py"], &env).ok();
    let second = read_events_jsonl(&f.root, &sid, "root");
    let second_reads = second.iter().filter(|e| e["kind"] == "read_file").count();
    assert_eq!(
        second_reads, 1,
        "unchanged read must not re-record, got {second_reads} events"
    );
}

// --- standalone (no session id) --------------------------------------------

#[test]
fn no_session_id_means_no_log_is_written() {
    let f = docs_repo();
    // No CLAUDE_CODE_SESSION_ID / CLAUDE_SESSION_ID / TRACER_SESSION_ID
    // set — standalone tracer use must still render docs and never write
    // a log.
    let r = Command::new(trace_bin())
        .args(["docs", "sub/util.py"])
        .current_dir(&f.root)
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .env_remove("CLAUDE_SESSION_ID")
        .env_remove("TRACER_SESSION_ID")
        .output()
        .expect("spawn trace");
    assert!(
        r.status.success(),
        "standalone trace docs must succeed:\n{}",
        String::from_utf8_lossy(&r.stderr)
    );
    let stdout = String::from_utf8_lossy(&r.stdout);
    assert!(
        stdout.contains("Root rules"),
        "standalone run must still surface docs:\n{stdout}"
    );
    // No <repo>/.tracer-cache/sessions/ tree was created — the fixture's
    // repo root holds the cache, and with no session id the storage path
    // is unreachable. The positive proof "rendered docs + never touched
    // the sessions tree" is already exercised by the dedup + view tests
    // above.
    assert!(
        !f.root.join(".tracer-cache").join("sessions").exists(),
        "standalone run must not create the sessions tree at the repo root"
    );
}

#[test]
fn no_repo_root_means_no_log_is_written() {
    // Standalone tracer use OUTSIDE any git repo: cwd is a fresh tempdir
    // with no `.git`, so `cache::worktree_root_for` returns None. The
    // log module's second no-op trigger kicks in — docs
    // still render, no sessions tree is created.
    let scratch = std::env::temp_dir().join(format!(
        "trace-no-repo-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&scratch).unwrap();
    std::fs::write(scratch.join("Claude.md"), "# Root\n").unwrap();
    std::fs::write(scratch.join("util.py"), "def f():\n    return 1\n").unwrap();
    let sid = fresh_session_id("no-repo");

    let r = Command::new(trace_bin())
        .args(["docs", "util.py"])
        .current_dir(&scratch)
        .env("CLAUDE_CODE_SESSION_ID", &sid)
        .output()
        .expect("spawn trace");
    assert!(
        r.status.success(),
        "trace docs outside a git repo must still succeed:\n{}",
        String::from_utf8_lossy(&r.stderr)
    );
    let stdout = String::from_utf8_lossy(&r.stdout);
    assert!(
        stdout.contains("Root"),
        "render must succeed even with no repo root:\n{stdout}"
    );
    assert!(
        !scratch.join(".tracer-cache").join("sessions").exists(),
        "no repo root resolvable → no sessions tree must be created"
    );

    let _ = std::fs::remove_dir_all(&scratch);
}

// --- archive lifecycle (subagent stop) -------------------------------------
//
// Contract being pinned: when `archive-subagent-log.sh` moves a
// subagent's active log from `sessions/<sid>/<aid>/` to
// `sessions/<sid>/archived/<aid>/`, the on-disk events and view survive
// the move and the tracer's read path follows them — `trace docs` against
// the archived log sees the same set of already-emitted
// paths as before the move.

fn dotfiles_root() -> PathBuf {
    // The test binary lives in tools/tracer/tests/target/...; walk up four
    // levels to reach the dotfiles repo root so the hook script path is
    // anchored even when CARGO_TARGET_DIR or HOME changes.
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // manifest = tools/tracer/tests
    manifest
        .parent() // tools/tracer
        .and_then(|p| p.parent()) // tools
        .and_then(|p| p.parent()) // dotfiles
        .expect("dotfiles root reachable from CARGO_MANIFEST_DIR")
        .to_path_buf()
}

fn archive_hook() -> PathBuf {
    dotfiles_root()
        .join("packages/claude/hooks/archive-subagent-log.sh")
}

/// Spawn the archive hook with cwd = the test's repo root, which is how
/// the harness invokes it in production (the hook resolves the repo root
/// via `git -C "$PWD" rev-parse --show-toplevel`).
fn run_archive_hook(repo_root: &Path, session_id: &str, agent_id: &str) {
    let status = Command::new("bash")
        .arg(archive_hook())
        .arg(session_id)
        .arg(agent_id)
        .current_dir(repo_root)
        .status()
        .expect("spawn archive-subagent-log.sh");
    assert!(
        status.success(),
        "archive hook exited non-zero for sid={session_id} aid={agent_id}"
    );
}

#[test]
fn archive_hook_moves_active_log_under_archived_subdir() {
    let f = docs_repo();
    let sid = fresh_session_id("archive-move");
    let aid = "subagent-alpha";
    let env = [
        ("CLAUDE_CODE_SESSION_ID", sid.as_str()),
        ("TRACER_AGENT_ID", aid),
    ];

    // Populate the subagent's active log.
    f.trace_env(&["docs", "sub/util.py"], &env).ok();
    let active_before = log_dir(&f.root, &sid, aid);
    assert!(
        active_before.is_dir(),
        "active log must exist before archive: {active_before:?}"
    );
    let pre_events = read_events_jsonl(&f.root, &sid, aid);
    let pre_view = read_view(&f.root, &sid, aid);
    assert_eq!(pre_events.len(), 2, "log should hold 2 doc events pre-archive");

    // Archive it.
    run_archive_hook(&f.root, &sid, aid);

    // Active dir gone, archived dir present, contents preserved.
    assert!(
        !log_dir(&f.root, &sid, aid).exists(),
        "active log dir must be removed after archive"
    );
    let archived_dir = f.root
        .join(".tracer-cache")
        .join("sessions")
        .join(&sid)
        .join("archived")
        .join(aid);
    assert!(
        archived_dir.is_dir(),
        "archived log dir must exist after archive: {archived_dir:?}"
    );
    let archived_events_path = archived_dir.join("events.jsonl");
    let archived_view_path = archived_dir.join("view.json");
    let archived_events_text = std::fs::read_to_string(&archived_events_path)
        .expect("archived events.jsonl readable");
    let archived_events: Vec<serde_json::Value> = archived_events_text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(
        archived_events.len(),
        pre_events.len(),
        "archived events.jsonl must preserve every event"
    );
    let archived_view: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&archived_view_path).unwrap()).unwrap();
    assert_eq!(
        archived_view, pre_view,
        "archived view.json must match the pre-archive projection"
    );
}

#[test]
fn read_path_follows_archived_log_when_active_is_absent() {
    let f = docs_repo();
    let sid = fresh_session_id("archive-read-fallback");
    let aid = "subagent-beta";
    let env = [
        ("CLAUDE_CODE_SESSION_ID", sid.as_str()),
        ("TRACER_AGENT_ID", aid),
    ];

    // Populate then archive.
    f.trace_env(&["docs", "sub/util.py"], &env).ok();
    run_archive_hook(&f.root, &sid, aid);
    assert!(
        !log_dir(&f.root, &sid, aid).exists(),
        "sanity: active log gone post-archive"
    );

    // A fresh `trace docs` against the same (session, agent) must observe
    // the archived view and skip already-emitted docs. The render still
    // prints the docs (it always prints; the log only
    // suppresses *re*-recording), but no new events are written — the read
    // path resolved against the archived directory.
    f.trace_env(&["docs", "sub/util.py"], &env).ok();

    // Active dir must remain absent — no second-write resurrection of the
    // active path on a read-only query.
    assert!(
        !log_dir(&f.root, &sid, aid).exists(),
        "active log must not be re-created by a read that hits the archive"
    );

    // Archived events.jsonl is unchanged: 2 events, exactly the originals.
    let archived_dir = f.root
        .join(".tracer-cache")
        .join("sessions")
        .join(&sid)
        .join("archived")
        .join(aid);
    let archived_events_text =
        std::fs::read_to_string(archived_dir.join("events.jsonl")).expect("archived events");
    let archived_events_count = archived_events_text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .count();
    assert_eq!(
        archived_events_count, 2,
        "archived events must be stable across post-archive reads"
    );
}

#[test]
fn archive_hook_is_a_no_op_for_a_subagent_that_never_wrote_a_log() {
    // A subagent that returned without surfacing any docs or reads never
    // creates `sessions/<sid>/<aid>/`. The hook must exit cleanly and not
    // create an empty `archived/<aid>/` placeholder.
    let f = docs_repo();
    let sid = fresh_session_id("archive-noop");
    let aid = "subagent-never-ran";
    let archived_dir = f.root
        .join(".tracer-cache")
        .join("sessions")
        .join(&sid)
        .join("archived")
        .join(aid);

    run_archive_hook(&f.root, &sid, aid);

    assert!(
        !log_dir(&f.root, &sid, aid).exists(),
        "active log must remain absent"
    );
    assert!(
        !archived_dir.exists(),
        "hook must not create an empty archived dir for a subagent with no log"
    );
}

#[test]
fn archive_hook_replaces_existing_archived_copy_on_double_stop() {
    // If a subagent re-stops (e.g. retry / re-dispatch with the same id),
    // a stale archived copy from the prior stop must be removed before the
    // current active log is moved into place. The contract: second-stop
    // wins; the archived dir always reflects the most recent run.
    let f = docs_repo();
    let sid = fresh_session_id("archive-double");
    let aid = "subagent-gamma";
    let env = [
        ("CLAUDE_CODE_SESSION_ID", sid.as_str()),
        ("TRACER_AGENT_ID", aid),
    ];

    // First run + archive.
    f.trace_env(&["docs", "sub/util.py"], &env).ok();
    run_archive_hook(&f.root, &sid, aid);
    let archived_dir = f.root
        .join(".tracer-cache")
        .join("sessions")
        .join(&sid)
        .join("archived")
        .join(aid);
    let first_view = std::fs::read_to_string(archived_dir.join("view.json"))
        .expect("first archived view readable");

    // Second run writes a fresh active log via a path the
    // first run did not surface. The archived view holds {Claude.md,
    // sub/Claude.md}; running `trace docs` from a deeper subdir adds new
    // Claude.md ancestors, forcing fresh events into the active
    // log that the archived path never saw.
    f.write(
        "sub/inner/Claude.md",
        "# Inner rules\n\nInner-deep rules.\n",
    );
    f.write(
        "sub/inner/leaf.py",
        "def leaf(v):\n    return v\n",
    );
    f.commit("add deeper inner rules");
    f.trace_env(&["docs", "sub/inner/leaf.py"], &env).ok();
    assert!(
        log_dir(&f.root, &sid, aid).is_dir(),
        "fresh active log written for the second run"
    );

    // Second archive must replace the first.
    run_archive_hook(&f.root, &sid, aid);
    let second_view = std::fs::read_to_string(archived_dir.join("view.json"))
        .expect("second archived view readable");
    assert_ne!(
        first_view, second_view,
        "second stop must replace the archived copy with the latest run's view"
    );
    assert!(
        !log_dir(&f.root, &sid, aid).exists(),
        "active log removed by second archive"
    );
}
