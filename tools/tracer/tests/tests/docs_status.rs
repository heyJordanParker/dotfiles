//! `trace docs status [path]` and the agent-facing context-awareness
//! surfaces over the session log.
//!
//! Pins the contract three consumers depend on:
//!   - `trace docs status` (no path) — full session manifest with source
//!     attribution; empty when no docs have been loaded yet
//!   - `trace docs status <path>` — partitions the path's ancestor chain
//!     into `loaded` (with source) and `not_loaded`
//!   - `trace context <file>` appends a one-line docs hint naming the
//!     in-context / not-loaded counts for that path's Claude.md ancestors
//!
//! No tracer internals are linked — the only contract is the CLI's
//! observable surface.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use tracer_cli_tests::Fixture;

#[allow(non_upper_case_globals)] // project naming rule bans ALL_CAPS for our own identifiers
static status_seq: AtomicU64 = AtomicU64::new(0);

fn fresh_session_id(tag: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let seq = status_seq.fetch_add(1, Ordering::SeqCst);
    format!("trace-docs-status-{tag}-{nanos}-{seq}")
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

// --- `trace docs status` (no path) — full session manifest ----------------

#[test]
fn session_status_is_empty_before_anything_loads() {
    let f = docs_repo();
    let sid = fresh_session_id("empty");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    let r = f.trace_env(&["docs", "status", "--json"], &env);
    r.ok();
    let v = r.json();

    assert_eq!(v["scope"].as_str().unwrap(), "session");
    assert_eq!(v["session_active"].as_bool().unwrap(), true);
    assert_eq!(v["loaded_count"].as_i64().unwrap(), 0);
    assert!(
        v["loaded"].as_array().unwrap().is_empty(),
        "empty manifest must serialize an empty array: {v}"
    );
    assert!(
        v["by_source"].as_object().unwrap().is_empty(),
        "by_source must be empty before any loads: {v}"
    );
}

#[test]
fn session_status_reports_every_doc_loaded_so_far_with_source() {
    let f = docs_repo();
    let sid = fresh_session_id("full");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    // Seed the log via `docs load` — surfaces both Claude.md
    // ancestors under source `trace_inject_hook`.
    f.trace_env(
        &["docs", "load", "sub/util.py", "--source", "trace_inject_hook", "--json"],
        &env,
    )
    .ok();

    let r = f.trace_env(&["docs", "status", "--json"], &env);
    r.ok();
    let v = r.json();

    assert_eq!(v["loaded_count"].as_i64().unwrap(), 2);
    let loaded = v["loaded"].as_array().unwrap();
    for entry in loaded {
        for key in ["path", "source", "kind", "size", "content_hash"] {
            assert!(
                entry.get(key).is_some(),
                "loaded entry missing `{key}`: {entry}"
            );
        }
        assert_eq!(
            entry["source"].as_str().unwrap(),
            "trace_inject_hook",
            "source must reflect the original load surface: {entry}"
        );
    }
    let by_source = v["by_source"].as_object().unwrap();
    assert_eq!(by_source["trace_inject_hook"].as_i64().unwrap(), 2);
}

#[test]
fn session_status_without_session_id_reports_inactive_session() {
    let f = docs_repo();
    let r = std::process::Command::new(tracer_cli_tests::trace_bin())
        .args(["docs", "status", "--json"])
        .current_dir(&f.root)
        .env_remove("AGENT_SESSION_ID")
        .env_remove("CODEX_THREAD_ID")
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .output()
        .expect("spawn trace");
    assert!(
        r.status.success(),
        "standalone status must succeed:\n{}",
        String::from_utf8_lossy(&r.stderr)
    );
    let v: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&r.stdout))
            .expect("standalone status must return JSON");
    assert_eq!(v["session_active"].as_bool().unwrap(), false);
    assert_eq!(v["loaded_count"].as_i64().unwrap(), 0);
}

// --- `trace docs status <path>` — partition the ancestor chain ------------

#[test]
fn path_status_with_nothing_loaded_reports_full_chain_as_not_loaded() {
    let f = docs_repo();
    let sid = fresh_session_id("path-empty");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    let r = f.trace_env(&["docs", "status", "sub/util.py", "--json"], &env);
    r.ok();
    let v = r.json();

    assert_eq!(v["scope"].as_str().unwrap(), "path");
    assert_eq!(v["loaded_count"].as_i64().unwrap(), 0);
    assert_eq!(
        v["not_loaded_count"].as_i64().unwrap(),
        2,
        "both Claude.md ancestors must be not_loaded before anything surfaces: {v}"
    );
    assert_eq!(v["chain_size"].as_i64().unwrap(), 2);

    // Pure read — must NOT have recorded anything. A follow-up status with
    // the same session must still report the chain as not_loaded.
    let r2 = f.trace_env(&["docs", "status", "sub/util.py", "--json"], &env);
    r2.ok();
    let v2 = r2.json();
    assert_eq!(
        v2["not_loaded_count"].as_i64().unwrap(),
        2,
        "status is a pure read — must not record emissions: {v2}"
    );
}

#[test]
fn path_status_with_partial_load_partitions_correctly() {
    // Seed only the ROOT Claude.md via `docs load` against the repo root;
    // then `docs status sub/util.py` must report root as loaded and the
    // sub-dir Claude.md as not_loaded.
    let f = docs_repo();
    let sid = fresh_session_id("partial");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    f.trace_env(
        &["docs", "load", ".", "--source", "trace_inject_hook", "--json"],
        &env,
    )
    .ok();

    let r = f.trace_env(&["docs", "status", "sub/util.py", "--json"], &env);
    r.ok();
    let v = r.json();

    assert_eq!(
        v["loaded_count"].as_i64().unwrap(),
        1,
        "only the root Claude.md was loaded — must show as loaded: {v}"
    );
    assert_eq!(
        v["not_loaded_count"].as_i64().unwrap(),
        1,
        "the sub-dir Claude.md was not loaded — must show as not_loaded: {v}"
    );

    let loaded = v["loaded"].as_array().unwrap();
    assert_eq!(loaded[0]["path"].as_str().unwrap(), "Claude.md");
    assert_eq!(
        loaded[0]["source"].as_str().unwrap(),
        "trace_inject_hook",
        "loaded entry must carry the original source: {}",
        loaded[0]
    );

    let not_loaded = v["not_loaded"].as_array().unwrap();
    assert_eq!(not_loaded[0]["path"].as_str().unwrap(), "sub/Claude.md");
}

#[test]
fn path_status_with_everything_loaded_has_empty_not_loaded() {
    let f = docs_repo();
    let sid = fresh_session_id("full-chain");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    f.trace_env(
        &["docs", "load", "sub/util.py", "--source", "trace_inject_hook", "--json"],
        &env,
    )
    .ok();

    let r = f.trace_env(&["docs", "status", "sub/util.py", "--json"], &env);
    r.ok();
    let v = r.json();

    assert_eq!(v["loaded_count"].as_i64().unwrap(), 2);
    assert_eq!(v["not_loaded_count"].as_i64().unwrap(), 0);
    assert!(v["not_loaded"].as_array().unwrap().is_empty());
}

// --- per-Read context-awareness hint via `trace context <file>` -----------

#[test]
fn context_file_mode_appends_docs_hint_when_nothing_loaded() {
    let f = docs_repo();
    let sid = fresh_session_id("hint-empty");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    let r = f.trace_env(&["context", &f.path("sub/util.py")], &env);
    r.ok();
    let combined = r.combined();
    assert!(
        combined.contains("[docs: 0/2 in context"),
        "context must surface `docs: 0/2 in context` for a path with two unloaded ancestors: {combined}"
    );
    assert!(
        combined.contains("not loaded: Claude.md, sub/Claude.md"),
        "context must name the unloaded ancestors: {combined}"
    );
}

#[test]
fn context_file_mode_hint_reflects_partial_load() {
    let f = docs_repo();
    let sid = fresh_session_id("hint-partial");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    // Load only the root Claude.md.
    f.trace_env(
        &["docs", "load", ".", "--source", "trace_inject_hook", "--json"],
        &env,
    )
    .ok();

    let r = f.trace_env(&["context", &f.path("sub/util.py")], &env);
    r.ok();
    let combined = r.combined();
    assert!(
        combined.contains("[docs: 1/2 in context"),
        "with root Claude.md loaded, hint must show 1/2: {combined}"
    );
    assert!(
        combined.contains("not loaded: sub/Claude.md"),
        "hint must name the still-unloaded ancestor: {combined}"
    );
}

#[test]
fn context_file_mode_hint_omits_not_loaded_when_everything_in_context() {
    let f = docs_repo();
    let sid = fresh_session_id("hint-full");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    f.trace_env(
        &["docs", "load", "sub/util.py", "--source", "trace_inject_hook", "--json"],
        &env,
    )
    .ok();

    let r = f.trace_env(&["context", &f.path("sub/util.py")], &env);
    r.ok();
    let combined = r.combined();
    assert!(
        combined.contains("[docs: 2/2 in context"),
        "with full chain loaded, hint must show 2/2: {combined}"
    );
    assert!(
        !combined.contains("not loaded:"),
        "hint must omit the `not loaded:` tail when nothing is missing: {combined}"
    );
}

// --- per-file %-read coverage in the session manifest ----------------------
//
// `trace context <file>` records which line range the agent read (the read
// tool's offset/limit); the session manifest surfaces, per file, the fraction
// of the file's lines read so far this session. Coverage accumulates the union
// of every read — partial reads add up; overlaps are not double-counted; a
// whole-file read is 100%; a file never read is 0%.

/// A repo with a deterministic 100-line file plus a Claude.md the manifest can
/// also carry as a doc-injected (never-read) entry.
fn coverage_repo() -> Fixture {
    let f = Fixture::new();
    f.write("Claude.md", "# Root rules\n\nProject root.\n");
    let body: String = (1..=100).map(|n| format!("line {n}\n")).collect();
    f.write("hundred.txt", &body);
    f.commit("init coverage repo");
    f
}

/// The `read_fraction` the session manifest reports for the loaded entry whose
/// path ends with `filename`, or `None` when the file is absent from the
/// manifest (a file never touched at all).
fn read_fraction_for(status: &serde_json::Value, filename: &str) -> Option<f64> {
    status["loaded"].as_array()?.iter().find_map(|e| {
        let p = e["path"].as_str()?;
        if p.ends_with(filename) {
            e["read_fraction"].as_f64()
        } else {
            None
        }
    })
}

fn status_json(f: &Fixture, env: &[(&str, &str)]) -> serde_json::Value {
    let r = f.trace_env(&["docs", "status", "--json"], env);
    r.ok();
    r.json()
}

#[test]
fn single_partial_read_reports_its_fraction() {
    let f = coverage_repo();
    let sid = fresh_session_id("cov-single");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    // Read lines 1–50 of the 100-line file.
    f.trace_env(
        &["context", &f.path("hundred.txt"), "--offset", "1", "--limit", "50"],
        &env,
    )
    .ok();

    let frac = read_fraction_for(&status_json(&f, &env), "hundred.txt")
        .expect("read file must appear in the manifest with coverage");
    assert!(
        (frac - 0.5).abs() < 1e-9,
        "a single read of 50/100 lines must report 0.5, got {frac}"
    );
}

#[test]
fn two_non_overlapping_reads_sum_to_full_coverage() {
    let f = coverage_repo();
    let sid = fresh_session_id("cov-union");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    f.trace_env(
        &["context", &f.path("hundred.txt"), "--offset", "1", "--limit", "50"],
        &env,
    )
    .ok();
    f.trace_env(
        &["context", &f.path("hundred.txt"), "--offset", "51", "--limit", "50"],
        &env,
    )
    .ok();

    let frac = read_fraction_for(&status_json(&f, &env), "hundred.txt").unwrap();
    assert!(
        (frac - 1.0).abs() < 1e-9,
        "two non-overlapping reads (1–50, 51–100) must report 1.0, got {frac}"
    );
}

#[test]
fn overlapping_reads_count_the_true_union_not_double() {
    let f = coverage_repo();
    let sid = fresh_session_id("cov-overlap");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    // 1–60 then 40–80 → union is 1–80 = 80 lines, not 60+41 double-counted.
    f.trace_env(
        &["context", &f.path("hundred.txt"), "--offset", "1", "--limit", "60"],
        &env,
    )
    .ok();
    f.trace_env(
        &["context", &f.path("hundred.txt"), "--offset", "40", "--limit", "41"],
        &env,
    )
    .ok();

    let frac = read_fraction_for(&status_json(&f, &env), "hundred.txt").unwrap();
    assert!(
        (frac - 0.8).abs() < 1e-9,
        "overlapping reads (1–60, 40–80) must report the union 0.8, got {frac}"
    );
}

#[test]
fn whole_file_read_reports_full_coverage() {
    let f = coverage_repo();
    let sid = fresh_session_id("cov-whole");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    // No --offset/--limit → whole file. This is also the shell-read path
    // (cat/head with no parsed range), so a shell read is counted identically
    // to a native whole-file read.
    f.trace_env(&["context", &f.path("hundred.txt")], &env).ok();

    let frac = read_fraction_for(&status_json(&f, &env), "hundred.txt").unwrap();
    assert!(
        (frac - 1.0).abs() < 1e-9,
        "a whole-file read must report 1.0, got {frac}"
    );
}

#[test]
fn doc_injected_but_never_read_file_reports_zero_coverage() {
    let f = coverage_repo();
    let sid = fresh_session_id("cov-zero");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    // Surface the root Claude.md via doc injection — it lands in the manifest
    // but was never read as a file, so its coverage is 0%.
    f.trace_env(
        &["docs", "load", ".", "--source", "trace_inject_hook", "--json"],
        &env,
    )
    .ok();

    let frac = read_fraction_for(&status_json(&f, &env), "Claude.md")
        .expect("doc-injected file must appear in the manifest");
    assert!(
        frac.abs() < 1e-9,
        "a file never read must report 0.0, got {frac}"
    );
}

// --- `--no-record`: render the shoulder without recording a read -----------
//
// An Edit/Write touches a file but is not a read of it. The enrich hook fires
// the same `trace context <file>` shoulder for those tools, but with
// `--no-record` so the file's read coverage reflects only genuine reads — an
// edit no longer masquerades as a whole-file read.

#[test]
fn no_record_does_not_record_a_read() {
    let f = coverage_repo();
    let sid = fresh_session_id("cov-no-record");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    // Simulate an Edit: render the file shoulder but skip the read-record.
    f.trace_env(&["context", &f.path("hundred.txt"), "--no-record"], &env)
        .ok();

    assert!(
        read_fraction_for(&status_json(&f, &env), "hundred.txt").is_none(),
        "an edit (--no-record) records no read — the file must be absent from the manifest"
    );
}

#[test]
fn no_record_still_renders_the_shoulder() {
    let f = coverage_repo();
    let sid = fresh_session_id("no-record-shoulder");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    let r = f.trace_env(&["context", &f.path("hundred.txt"), "--no-record"], &env);
    r.ok();
    let combined = r.combined();
    assert!(
        combined.contains("[docs:"),
        "an edit must still get the file's architectural shoulder (docs-awareness line): {combined}"
    );
}

#[test]
fn no_record_does_not_suppress_a_later_genuine_read() {
    let f = coverage_repo();
    let sid = fresh_session_id("no-record-then-read");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    // Edit first (no record), then a genuine whole-file read.
    f.trace_env(&["context", &f.path("hundred.txt"), "--no-record"], &env)
        .ok();
    f.trace_env(&["context", &f.path("hundred.txt")], &env).ok();

    let frac = read_fraction_for(&status_json(&f, &env), "hundred.txt")
        .expect("the genuine read must record coverage even after a prior no-record edit");
    assert!(
        (frac - 1.0).abs() < 1e-9,
        "the genuine whole-file read must report full coverage, got {frac}"
    );
}
