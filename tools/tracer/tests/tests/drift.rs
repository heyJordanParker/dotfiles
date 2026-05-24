//! `trace context prime --observed-from <path>` — the drift detector.
//!
//! Black-box: spawn `trace` with an isolated `HOME` so the user-global
//! CLAUDE.md lives inside the fixture; the session log
//! directory lives at the fixture root, which IS the repo root
//! (`Fixture::new` runs `git init`). Each test owns a fresh session id so
//! logs never collide.
//!
//! The detector's contract:
//!   - no observation supplied (no `--observed-from`) → behaves exactly
//!     like the pre-drift `context prime` (no drift block in output, no
//!     drift event in the log)
//!   - empty stdin / empty observed input → same as no observation
//!   - predicted == observed → no `context_prime_drift` event, output has no
//!     `drift` key, view stable
//!   - predicted ≠ observed → one `context_prime_drift` event, output carries
//!     `drift` block with `missing` + `extra`, view rewritten to observed
//!     paths + their content hashes
//!   - malformed observed input → exit non-zero, no log
//!     mutation
//!
//! All five run against the same hermetic fixture pattern from the
//! sibling `context_prime.rs` suite (shared session id discipline,
//! log directory under
//! `<fixture_root>/.tracer-cache/sessions/<sid>/root/`).

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use tracer_cli_tests::Fixture;

#[allow(non_upper_case_globals)] // project naming rule bans ALL_CAPS for our own identifiers
static drift_seq: AtomicU64 = AtomicU64::new(0);

fn fresh_session_id(tag: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let seq = drift_seq.fetch_add(1, Ordering::SeqCst);
    format!("trace-drift-test-{tag}-{nanos}-{seq}")
}

fn log_dir(home: &PathBuf, session_id: &str) -> PathBuf {
    home.join(".tracer-cache")
        .join("sessions")
        .join(session_id)
        .join("root")
}

fn read_events(home: &PathBuf, session_id: &str) -> Vec<serde_json::Value> {
    let path = log_dir(home, session_id).join("events.jsonl");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return vec![];
    };
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("events.jsonl line is valid JSON"))
        .collect()
}

fn read_view(home: &PathBuf, session_id: &str) -> serde_json::Value {
    let path = log_dir(home, session_id).join("view.json");
    let text = std::fs::read_to_string(&path).expect("view.json should exist after context prime");
    serde_json::from_str(&text).expect("view.json is valid JSON")
}

fn isolated(tag: &str) -> (Fixture, PathBuf, String) {
    let f = Fixture::new();
    let home = f.root.clone();
    std::fs::create_dir_all(home.join(".claude")).unwrap();
    let sid = fresh_session_id(tag);
    (f, home, sid)
}

fn env_pairs<'a>(home: &'a str, sid: &'a str) -> Vec<(&'a str, &'a str)> {
    vec![("HOME", home), ("CLAUDE_CODE_SESSION_ID", sid)]
}

/// `sha256:<hex>` of `s` — same shape the log persists. Computed in
/// the test with the same algorithm the binary uses so observed hashes
/// land in the view byte-for-byte.
fn sha256_hex(s: &str) -> String {
    use sha2::{Digest, Sha256};
    format!("sha256:{}", hex::encode(Sha256::digest(s.as_bytes())))
}

// --- no drift --------------------------------------------------------------

#[test]
fn no_drift_when_predicted_equals_observed() {
    let (f, home, sid) = isolated("no-drift");
    let content = "# Project rules\n\nSingle root file.\n";
    f.write("CLAUDE.md", content);
    f.commit("add root claude.md");

    let predicted_path = f.root.canonicalize().unwrap().join("CLAUDE.md");
    let observed_json = serde_json::json!({
        "paths": [{
            "path": predicted_path.to_string_lossy(),
            "content_hash": sha256_hex(content),
            "size": content.len(),
        }]
    });
    let observed_path = f.write("observed.json", &observed_json.to_string());

    let home_str = home.to_string_lossy().into_owned();
    let observed_arg = observed_path.to_string_lossy().into_owned();
    let run = f.trace_env(
        &[
            "context", "prime",
            "--reason",
            "session_start",
            "--observed-from",
            &observed_arg,
            "--json",
        ],
        &env_pairs(&home_str, &sid),
    );
    run.ok();

    let v = run.json();
    assert_eq!(v["mirrored_count"], 1);
    assert!(
        v.get("drift").is_none() || v["drift"].is_null(),
        "no drift means no drift block in output: {v}"
    );

    let events = read_events(&home, &sid);
    let drift_events: Vec<_> = events
        .iter()
        .filter(|e| e["kind"] == "context_prime_drift")
        .collect();
    assert!(
        drift_events.is_empty(),
        "predicted == observed means no context_prime_drift event, got {drift_events:?}"
    );
    // The doc_injection event from the prediction is still there.
    let injection_events: Vec<_> = events
        .iter()
        .filter(|e| e["kind"] == "doc_injection")
        .collect();
    assert_eq!(injection_events.len(), 1, "prediction event still present: {events:?}");
}

// --- drift detected --------------------------------------------------------

#[test]
fn drift_records_event_and_reconciles_view() {
    let (f, home, sid) = isolated("drift");
    f.write("CLAUDE.md", "# Project rules\n");
    f.commit("add root claude.md");

    let extra_content = "# An extra doc the primer didn't predict\n";
    let extra_path = "/abs/path/observed-only.md";
    let extra_hash = sha256_hex(extra_content);

    // Observed set: only the extra doc — predicted CLAUDE.md is missing.
    let observed_json = serde_json::json!({
        "paths": [{
            "path": extra_path,
            "content_hash": extra_hash,
            "size": extra_content.len(),
        }]
    });
    let observed_file = f.write("observed.json", &observed_json.to_string());

    let home_str = home.to_string_lossy().into_owned();
    let observed_arg = observed_file.to_string_lossy().into_owned();
    let run = f.trace_env(
        &[
            "context", "prime",
            "--reason",
            "session_start",
            "--observed-from",
            &observed_arg,
            "--json",
        ],
        &env_pairs(&home_str, &sid),
    );
    run.ok();

    let v = run.json();
    let drift = &v["drift"];
    assert_eq!(drift["source"], "context_prime_drift", "drift block in output: {v}");
    assert_eq!(drift["predicted_count"], 1);
    assert_eq!(drift["observed_count"], 1);
    let missing: BTreeSet<String> = drift["missing"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_string())
        .collect();
    let extra: BTreeSet<String> = drift["extra"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().to_string())
        .collect();
    assert_eq!(missing.len(), 1, "exactly one predicted path went missing: {missing:?}");
    assert!(
        missing.iter().any(|p| p.ends_with("CLAUDE.md")),
        "predicted CLAUDE.md should be in missing: {missing:?}"
    );
    assert!(
        extra.contains(extra_path),
        "extra path should appear in extra set: {extra:?}"
    );

    // Ledger event: exactly one context_prime_drift entry, source matches.
    let events = read_events(&home, &sid);
    let drift_events: Vec<_> = events
        .iter()
        .filter(|e| e["kind"] == "context_prime_drift")
        .collect();
    assert_eq!(
        drift_events.len(),
        1,
        "one context_prime_drift event per drift detection: {events:?}"
    );
    assert_eq!(drift_events[0]["source"], "context_prime_drift");
    assert!(
        drift_events[0]["content_hash"]
            .as_str()
            .unwrap()
            .starts_with("sha256:"),
        "drift event carries a content hash of its payload: {}",
        drift_events[0]
    );

    // View reconciled: predicted CLAUDE.md gone, observed path present
    // with its real content hash from the hook input.
    let view = read_view(&home, &sid);
    let emitted = view["emitted"].as_object().unwrap();
    assert!(
        !emitted.keys().any(|k| k.ends_with("CLAUDE.md")),
        "predicted-but-not-observed entry was reconciled out: {emitted:?}"
    );
    assert_eq!(
        emitted[extra_path].as_str().unwrap(),
        extra_hash,
        "observed entry landed in view with its real content hash: {emitted:?}"
    );
}

// --- malformed input -------------------------------------------------------

#[test]
fn malformed_observed_input_fails_loud_no_log_mutation() {
    let (f, home, sid) = isolated("malformed");
    f.write("CLAUDE.md", "# Project\n");
    f.commit("add claude.md");

    // Not JSON.
    let observed_file = f.write("observed.json", "this is not valid json {{{");

    let home_str = home.to_string_lossy().into_owned();
    let observed_arg = observed_file.to_string_lossy().into_owned();
    let run = f.trace_env(
        &[
            "context", "prime",
            "--reason",
            "session_start",
            "--observed-from",
            &observed_arg,
            "--json",
        ],
        &env_pairs(&home_str, &sid),
    );
    assert_ne!(run.code, 0, "malformed input must fail loudly: stdout={} stderr={}", run.stdout, run.stderr);
    assert!(
        run.combined().contains("--observed-from") || run.combined().contains("JSON"),
        "error mentions the failing surface: {}",
        run.combined()
    );

    // Ledger must not carry a context_prime_drift event — the prediction may
    // have been recorded before the drift step ran, but no drift event
    // should land on malformed input.
    let events = read_events(&home, &sid);
    let drift_events: Vec<_> = events
        .iter()
        .filter(|e| e["kind"] == "context_prime_drift")
        .collect();
    assert!(
        drift_events.is_empty(),
        "malformed input must not produce a drift event: {drift_events:?}"
    );
}

// --- empty stdin input -----------------------------------------------------

#[test]
fn empty_observed_input_skips_detection() {
    let (f, home, sid) = isolated("empty");
    f.write("CLAUDE.md", "# Project\n");
    f.commit("add claude.md");

    // Empty file — the hook's "I had nothing to report" signal.
    let observed_file = f.write("observed.json", "   \n  \n");

    let home_str = home.to_string_lossy().into_owned();
    let observed_arg = observed_file.to_string_lossy().into_owned();
    let run = f.trace_env(
        &[
            "context", "prime",
            "--reason",
            "session_start",
            "--observed-from",
            &observed_arg,
            "--json",
        ],
        &env_pairs(&home_str, &sid),
    );
    run.ok();

    let v = run.json();
    assert_eq!(v["mirrored_count"], 1);
    assert!(
        v.get("drift").is_none() || v["drift"].is_null(),
        "empty input must not emit a drift block: {v}"
    );

    let events = read_events(&home, &sid);
    let drift_events: Vec<_> = events
        .iter()
        .filter(|e| e["kind"] == "context_prime_drift")
        .collect();
    assert!(
        drift_events.is_empty(),
        "empty input must not emit a drift event: {drift_events:?}"
    );
}

// --- reconciliation idempotence --------------------------------------------

#[test]
fn second_identical_drift_call_is_a_noop_on_view() {
    let (f, home, sid) = isolated("idempotent");
    f.write("CLAUDE.md", "# Project rules\n");
    f.commit("add claude.md");

    let extra_content = "# observed-only\n";
    let extra_path = "/abs/path/observed-only.md";
    let extra_hash = sha256_hex(extra_content);
    let observed_json = serde_json::json!({
        "paths": [{
            "path": extra_path,
            "content_hash": extra_hash,
            "size": extra_content.len(),
        }]
    });
    let observed_file = f.write("observed.json", &observed_json.to_string());

    let home_str = home.to_string_lossy().into_owned();
    let observed_arg = observed_file.to_string_lossy().into_owned();
    let args = [
        "context", "prime",
        "--reason",
        "session_start",
        "--observed-from",
        &observed_arg,
        "--json",
    ];

    f.trace_env(&args, &env_pairs(&home_str, &sid)).ok();
    let view_after_first = read_view(&home, &sid);

    f.trace_env(&args, &env_pairs(&home_str, &sid)).ok();
    let view_after_second = read_view(&home, &sid);

    assert_eq!(
        view_after_first["emitted"], view_after_second["emitted"],
        "view must be stable across identical drift calls"
    );

    // Each call appends its own drift event — append-only history is the
    // documented contract. Two calls → two drift events, view unchanged.
    let events = read_events(&home, &sid);
    let drift_events: Vec<_> = events
        .iter()
        .filter(|e| e["kind"] == "context_prime_drift")
        .collect();
    assert_eq!(
        drift_events.len(),
        2,
        "append-only log: each drift call appends an event, got {drift_events:?}"
    );
}
