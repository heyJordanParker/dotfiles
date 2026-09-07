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
use std::{
    io::Read,
    process::{Command, Stdio},
};

use sha2::{Digest, Sha256};
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
    let v = r.view();

    assert_eq!(v["scope"].as_str().unwrap(), "session");
    assert_eq!(v["session_active"].as_bool().unwrap(), true);
    assert_eq!(v["loaded"].as_array().unwrap().len() as i64, 0);
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
        &[
            "docs",
            "load",
            "sub/util.py",
            "--source",
            "trace_inject_hook",
            "--json",
        ],
        &env,
    )
    .ok();

    let r = f.trace_env(&["docs", "status", "--json"], &env);
    r.ok();
    let v = r.view();

    assert_eq!(v["loaded"].as_array().unwrap().len() as i64, 2);
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
    let v: serde_json::Value = serde_json::from_str(&String::from_utf8_lossy(&r.stdout))
        .expect("standalone status must return JSON");
    assert_eq!(v["context"]["session_active"].as_bool().unwrap(), false);
    assert_eq!(v["results"]["loaded"].as_array().unwrap().len() as i64, 0);
}

// --- `trace docs status <path>` — partition the ancestor chain ------------

#[test]
fn path_status_with_nothing_loaded_reports_full_chain_as_not_loaded() {
    let f = docs_repo();
    let sid = fresh_session_id("path-empty");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    let r = f.trace_env(&["docs", "status", "sub/util.py", "--json"], &env);
    r.ok();
    let v = r.view();

    assert_eq!(v["scope"].as_str().unwrap(), "path");
    assert_eq!(v["loaded"].as_array().unwrap().len() as i64, 0);
    assert_eq!(
        v["not_loaded"].as_array().unwrap().len() as i64,
        2,
        "both Claude.md ancestors must be not_loaded before anything surfaces: {v}"
    );
    assert_eq!(r.json()["counts"]["chain"].as_i64().unwrap(), 2);

    // Pure read — must NOT have recorded anything. A follow-up status with
    // the same session must still report the chain as not_loaded.
    let r2 = f.trace_env(&["docs", "status", "sub/util.py", "--json"], &env);
    r2.ok();
    let v2 = r2.view();
    assert_eq!(
        v2["not_loaded"].as_array().unwrap().len() as i64,
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
        &[
            "docs",
            "load",
            ".",
            "--source",
            "trace_inject_hook",
            "--json",
        ],
        &env,
    )
    .ok();

    let r = f.trace_env(&["docs", "status", "sub/util.py", "--json"], &env);
    r.ok();
    let v = r.view();

    assert_eq!(
        v["loaded"].as_array().unwrap().len() as i64,
        1,
        "only the root Claude.md was loaded — must show as loaded: {v}"
    );
    assert_eq!(
        v["not_loaded"].as_array().unwrap().len() as i64,
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
        &[
            "docs",
            "load",
            "sub/util.py",
            "--source",
            "trace_inject_hook",
            "--json",
        ],
        &env,
    )
    .ok();

    let r = f.trace_env(&["docs", "status", "sub/util.py", "--json"], &env);
    r.ok();
    let v = r.view();

    assert_eq!(v["loaded"].as_array().unwrap().len() as i64, 2);
    assert_eq!(v["not_loaded"].as_array().unwrap().len() as i64, 0);
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
        &[
            "docs",
            "load",
            ".",
            "--source",
            "trace_inject_hook",
            "--json",
        ],
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
        &[
            "docs",
            "load",
            "sub/util.py",
            "--source",
            "trace_inject_hook",
            "--json",
        ],
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
    r.view()
}

#[test]
fn single_partial_read_reports_its_fraction() {
    let f = coverage_repo();
    let sid = fresh_session_id("cov-single");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    // Read lines 1–50 of the 100-line file.
    f.trace_env(
        &[
            "context",
            &f.path("hundred.txt"),
            "--offset",
            "1",
            "--limit",
            "50",
        ],
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
        &[
            "context",
            &f.path("hundred.txt"),
            "--offset",
            "1",
            "--limit",
            "50",
        ],
        &env,
    )
    .ok();
    f.trace_env(
        &[
            "context",
            &f.path("hundred.txt"),
            "--offset",
            "51",
            "--limit",
            "50",
        ],
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
        &[
            "context",
            &f.path("hundred.txt"),
            "--offset",
            "1",
            "--limit",
            "60",
        ],
        &env,
    )
    .ok();
    f.trace_env(
        &[
            "context",
            &f.path("hundred.txt"),
            "--offset",
            "40",
            "--limit",
            "41",
        ],
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
        &[
            "docs",
            "load",
            ".",
            "--source",
            "trace_inject_hook",
            "--json",
        ],
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
fn batch_no_record_does_not_record_any_read() {
    let f = coverage_repo();
    let sid = fresh_session_id("cov-batch-no-record");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    let r = f.trace_env(
        &[
            "context",
            &f.path("hundred.txt"),
            &f.path("Claude.md"),
            "--no-record",
            "--json",
        ],
        &env,
    );
    r.ok();
    assert_eq!(r.json()["counts"]["files"], 2);
    let status = status_json(&f, &env);
    assert!(read_fraction_for(&status, "hundred.txt").is_none());
    assert!(read_fraction_for(&status, "Claude.md").is_none());
}

#[test]
fn context_batch_json_preserves_input_order_and_accounts_for_missing_paths() {
    let f = coverage_repo();
    let r = f.trace(&[
        "context",
        "hundred.txt",
        "missing.txt",
        "Claude.md",
        "--no-record",
        "--json",
    ]);
    r.ok();
    let document = r.json();
    assert_eq!(
        document["query"]["paths"],
        serde_json::json!(["hundred.txt", "missing.txt", "Claude.md"])
    );
    let rows = document["results"].as_array().expect("batch rows");
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0]["file"], "hundred.txt");
    assert!(rows[0]["content"].as_str().unwrap().contains("[git:"));
    assert!(rows[0]["error"].is_null());
    assert_eq!(rows[1]["file"], "missing.txt");
    assert_eq!(rows[1]["content"], "");
    assert!(rows[1]["error"].as_str().unwrap().contains("unavailable"));
    assert_eq!(rows[2]["file"], "Claude.md");
    assert!(rows[2]["content"].as_str().unwrap().contains("[git:"));
    assert_eq!(
        document["counts"],
        serde_json::json!({"files": 3, "unavailable": 1})
    );
    assert!(document["context"].is_object());
}

#[test]
fn context_batch_requires_no_record_and_rejects_single_path_options() {
    let f = coverage_repo();
    f.trace(&["context", "hundred.txt", "Claude.md", "--json"])
        .code_is(1);
    f.trace(&[
        "context",
        "hundred.txt",
        "Claude.md",
        "--no-record",
        "--offset",
        "1",
        "--json",
    ])
    .code_is(1);
    f.trace(&[
        "context",
        "hundred.txt",
        "Claude.md",
        "--no-record",
        "--limit",
        "1",
        "--json",
    ])
    .code_is(1);
    f.trace(&[
        "context",
        "hundred.txt",
        "Claude.md",
        "--no-record",
        "--directory",
        "--json",
    ])
    .code_is(1);
}

#[test]
fn context_single_path_json_uses_the_batch_document() {
    let f = coverage_repo();
    let r = f.trace(&["context", "hundred.txt", "--no-record", "--json"]);
    r.ok();
    let document = r.json();
    assert_eq!(
        document["query"]["paths"],
        serde_json::json!(["hundred.txt"])
    );
    assert_eq!(document["results"].as_array().unwrap().len(), 1);
    assert_eq!(
        document["counts"],
        serde_json::json!({"files": 1, "unavailable": 0})
    );
}

#[test]
fn context_json_without_a_path_is_refused_without_printing_the_primer() {
    let f = coverage_repo();
    let r = f.trace(&["context", "--json"]);
    r.code_is(1);
    assert!(
        r.stdout.is_empty(),
        "unexpected primer or JSON: {}",
        r.stdout
    );
    assert!(
        r.stderr
            .contains("context --json requires at least one path"),
        "{}",
        r.stderr
    );
}

#[test]
fn context_without_a_path_still_prints_the_human_primer() {
    let f = coverage_repo();
    let r = f.trace(&["context"]);
    r.ok();
    assert!(r.stdout.contains("## Environment"), "{}", r.stdout);
    assert!(r.stdout.contains("## Identity"), "{}", r.stdout);
    assert!(r.stdout.contains("repo_context:"), "{}", r.stdout);
    assert!(serde_json::from_str::<serde_json::Value>(&r.stdout).is_err());
}

#[test]
fn context_batch_human_labels_each_path_and_marks_unavailable_rows() {
    let f = coverage_repo();
    let r = f.trace(&["context", "hundred.txt", "missing.txt", "--no-record"]);
    r.ok();
    assert!(
        r.stdout.contains("== hundred.txt ==\n[git:"),
        "{}",
        r.stdout
    );
    assert!(
        r.stdout.contains("== missing.txt ==\n[unavailable:"),
        "{}",
        r.stdout
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

#[test]
fn native_read_records_only_the_emitted_range() {
    let f = coverage_repo();
    let sid = fresh_session_id("native-read-range");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    f.trace_env(&["read", "hundred.txt", "--lines", "1:30", "--json"], &env)
        .ok();

    let frac = read_fraction_for(&status_json(&f, &env), "hundred.txt")
        .expect("a successful native read must appear in the manifest");
    assert!(
        (frac - 0.3).abs() < 1e-9,
        "a 30-line read of a 100-line file must report 0.3, got {frac}"
    );
}

#[test]
fn filtered_native_read_does_not_record_source_coverage() {
    let f = coverage_repo();
    let sid = fresh_session_id("native-read-filter");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    f.trace_env(
        &["read", "hundred.txt", "--json", "--filter", ".counts.files"],
        &env,
    )
    .ok();

    assert!(
        read_fraction_for(&status_json(&f, &env), "hundred.txt").is_none(),
        "a filtered projection must not claim the source was read"
    );
}

#[test]
fn failed_filter_does_not_record_source_coverage() {
    let f = coverage_repo();
    let sid = fresh_session_id("native-read-filter-error");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    let read = f.trace_env(&["read", "hundred.txt", "--json", "--filter", ".["], &env);
    assert_ne!(read.code, 0, "an invalid filter must fail");

    assert!(
        read_fraction_for(&status_json(&f, &env), "hundred.txt").is_none(),
        "a failed filter must not claim the source was read"
    );
}

#[test]
fn historical_native_read_does_not_record_worktree_coverage() {
    let f = coverage_repo();
    let sid = fresh_session_id("native-read-history");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    f.trace_env(&["read", "hundred.txt", "--at", "HEAD", "--json"], &env)
        .ok();

    assert!(
        read_fraction_for(&status_json(&f, &env), "hundred.txt").is_none(),
        "a historical read must not claim the working-tree source was read"
    );
}

#[test]
fn trimmed_native_read_records_only_the_shown_lines() {
    let f = Fixture::new();
    let body: String = (1..=100)
        .map(|n| format!("line {n:03} {}\n", "x".repeat(1_000)))
        .collect();
    f.write("large.txt", &body);
    f.commit("add a large file");
    let sid = fresh_session_id("native-read-trim");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    let read = f.trace_env(&["read", "large.txt", "--lines", "1:100", "--json"], &env);
    read.ok();
    let shown_end = read.json()["results"][0]["shown_lines"][1]
        .as_f64()
        .expect("trimmed read must report its last shown source line");
    assert!(
        read.json()["results"][0]["truncated"].as_bool().unwrap(),
        "fixture must cross the read output budget"
    );

    let frac = read_fraction_for(&status_json(&f, &env), "large.txt")
        .expect("a successful native read must appear in the manifest");
    assert!(
        (frac - shown_end / 100.0).abs() < 1e-9,
        "coverage must count only the emitted lines, got {frac} for lines 1-{shown_end}"
    );
}

#[test]
fn native_read_continuations_merge_their_line_union() {
    let f = coverage_repo();
    let sid = fresh_session_id("native-read-union");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    f.trace_env(&["read", "hundred.txt", "--lines", "1:60", "--json"], &env)
        .ok();
    f.trace_env(&["read", "hundred.txt", "--lines", "40:80", "--json"], &env)
        .ok();

    let frac = read_fraction_for(&status_json(&f, &env), "hundred.txt").unwrap();
    assert!(
        (frac - 0.8).abs() < 1e-9,
        "native continuations must count the 1-80 union once, got {frac}"
    );
}

#[test]
fn empty_native_read_does_not_record_whole_file_coverage() {
    let f = coverage_repo();
    let sid = fresh_session_id("native-read-empty");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    let read = f.trace_env(
        &["read", "hundred.txt", "--lines", "101:110", "--json"],
        &env,
    );
    read.ok();
    assert!(
        read.json()["results"][0]["shown_lines"].is_null(),
        "the out-of-range selection must emit no source lines"
    );

    assert!(
        read_fraction_for(&status_json(&f, &env), "hundred.txt").is_none(),
        "an empty read must not turn None into whole-file coverage"
    );
}

#[test]
fn closed_stdout_does_not_record_human_or_json_delivery() {
    let f = coverage_repo();
    let sid = fresh_session_id("native-read-closed-stdout");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    for args in [
        vec!["read", "hundred.txt", "--docs"],
        vec!["read", "hundred.txt", "--docs", "--json"],
    ] {
        let mut child = Command::new(tracer_cli_tests::trace_bin())
            .args(args)
            .current_dir(&f.root)
            .env("HOME", &f.root)
            .env("CLAUDE_CODE_SESSION_ID", sid.as_str())
            .env_remove("AGENT_SESSION_ID")
            .env_remove("CODEX_THREAD_ID")
            .env_remove("TRACER_AGENT_ID")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn trace");
        drop(child.stdout.take().expect("child stdout"));
        let output = child.wait_with_output().expect("wait for trace");
        assert!(
            !output.status.success(),
            "a closed stdout must make the read delivery fail"
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("Broken pipe"),
            "failed delivery must name the broken stdout: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let status = status_json(&f, &env);
    assert!(
        read_fraction_for(&status, "hundred.txt").is_none(),
        "failed output must not record source coverage"
    );
    assert!(
        status["loaded"].as_array().unwrap().is_empty(),
        "failed output must not record documentation delivery: {status}"
    );
}

#[test]
fn source_edit_after_output_starts_keeps_the_captured_hash() {
    let f = Fixture::new();
    let source: String = (1..=1_000)
        .map(|n| format!("line {n:04} {}\n", "x".repeat(1_000)))
        .collect();
    let captured_hash = format!("sha256:{}", hex::encode(Sha256::digest(source.as_bytes())));
    f.write("large.txt", &source);
    f.commit("add large source");
    let sid = fresh_session_id("native-read-source-edit");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    let mut child = Command::new(tracer_cli_tests::trace_bin())
        .args(["read", "large.txt", "--all", "--json"])
        .current_dir(&f.root)
        .env("HOME", &f.root)
        .env("CLAUDE_CODE_SESSION_ID", sid.as_str())
        .env_remove("AGENT_SESSION_ID")
        .env_remove("CODEX_THREAD_ID")
        .env_remove("TRACER_AGENT_ID")
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn trace");
    let mut stdout = child.stdout.take().expect("child stdout");
    let mut first_byte = [0_u8; 1];
    stdout
        .read_exact(&mut first_byte)
        .expect("first output byte");

    let replacement = "replacement\n".repeat(10);
    let replacement_hash = format!(
        "sha256:{}",
        hex::encode(Sha256::digest(replacement.as_bytes()))
    );
    f.write("large.txt", &replacement);

    let mut remaining = Vec::new();
    stdout
        .read_to_end(&mut remaining)
        .expect("drain trace output");
    assert!(child.wait().expect("wait for trace").success());

    let status = status_json(&f, &env);
    let loaded = status["loaded"].as_array().unwrap();
    let entry = loaded
        .iter()
        .find(|entry| entry["path"].as_str().unwrap_or("").ends_with("large.txt"))
        .expect("read file must be recorded after successful output");
    assert_eq!(entry["content_hash"], captured_hash);
    assert_ne!(entry["content_hash"], replacement_hash);
}

#[test]
fn cleaned_read_coverage_records_only_delivered_source_spans() {
    let f = Fixture::new();
    f.write(
        "licensed.rs",
        "#!/usr/bin/env rust-script\n\n/*\n * Copyright (c) 2026 Example\n * License: MIT\n */\nfn licensed() {}\n",
    );
    f.write(
        "decorated.rs",
        "const FIRST: usize = 1;\n// ----------------\n\n\n\nconst LAST: usize = 2;\n",
    );
    f.write(
        "plain.rs",
        "const FIRST: usize = 1;\nconst LAST: usize = 2;\n",
    );
    f.commit("add exact delivery coverage fixtures");
    let sid = fresh_session_id("native-read-exact-delivery");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    f.trace_env(&["read", "licensed.rs", "--json"], &env).ok();
    let licensed_fraction = read_fraction_for(&status_json(&f, &env), "licensed.rs").unwrap();
    assert!(
        (licensed_fraction - 3.0 / 7.0).abs() < 1e-9,
        "cleaned delivery must cover shebang, retained blank, and function only, got {licensed_fraction}"
    );

    f.trace_env(
        &["read", "licensed.rs", "--lines", "3:6", "--raw", "--json"],
        &env,
    )
    .ok();
    let licensed_after_raw = read_fraction_for(&status_json(&f, &env), "licensed.rs").unwrap();
    assert!(
        (licensed_after_raw - 1.0).abs() < 1e-9,
        "raw delivery of the omitted notice must complete coverage, got {licensed_after_raw}"
    );

    f.trace_env(&["read", "decorated.rs", "--json"], &env).ok();
    let decorated_fraction = read_fraction_for(&status_json(&f, &env), "decorated.rs").unwrap();
    assert!(
        (decorated_fraction - 4.0 / 6.0).abs() < 1e-9,
        "coverage must exclude the decorative line and collapsed blank, got {decorated_fraction}"
    );

    f.trace_env(&["read", "plain.rs", "--json"], &env).ok();
    let plain_fraction = read_fraction_for(&status_json(&f, &env), "plain.rs").unwrap();
    assert!(
        (plain_fraction - 1.0).abs() < 1e-9,
        "a full read with no omissions must remain fully covered, got {plain_fraction}"
    );
}
