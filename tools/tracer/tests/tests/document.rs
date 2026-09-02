//! The promise: one document shape, and one way to fail.
//!
//! Every `--json` answer is `{query, context, results, counts}`, `--filter`
//! projects it without ever dropping `context`, and a question with no answer
//! says so on stderr with a non-zero exit instead of returning an empty
//! result an agent reads as "this does not exist". Nothing here trusts
//! internals — only exit code, stdout, stderr, and JSON shape.

use tracer_cli_tests::{standard_repo, Fixture};

/// Parse the one filtered document on stdout.
fn filtered(stdout: &str) -> serde_json::Value {
    serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("filtered output must be valid JSON ({e}): {stdout}"))
}

/// A scalar jq selector returns exactly that scalar under `results` — and it
/// is the same value the unfiltered document carries.
#[test]
fn filter_extracts_scalar_matching_unfiltered_json() {
    let f = standard_repo();
    let plain = f.trace(&["info", f.path("src/app.py").as_str(), "--json"]);
    plain.ok();
    let want = plain.json()["counts"]["rank"].clone();
    // src/app.py's CCN (4) ranks "low" for this fixture — pin the exact
    // value so this also guards the unfiltered rank, not just round-trip.
    assert_eq!(want, "low", "fixture sanity: app.py rank must be low");

    let r = f.trace(&[
        "info",
        f.path("src/app.py").as_str(),
        "--json",
        "--filter",
        ".counts.rank",
    ]);
    r.ok();
    let got = filtered(&r.stdout);
    assert_eq!(
        got["results"], want,
        "--filter '.counts.rank' must equal the unfiltered rank"
    );
}

/// The enrichment cannot be filtered away. `.results` is the expression
/// agents reach for most, and it is exactly the one that used to drop the
/// per-file context — the filtered document still carries it.
#[test]
fn filter_projecting_rows_still_carries_context() {
    let f = standard_repo();
    let plain = f.trace(&["grep", "helper", "--path", ".", "--json"]);
    plain.ok();
    let want = plain.json();

    let r = f.trace(&[
        "grep", "helper", "--path", ".", "--json", "--filter", ".results",
    ]);
    r.ok();
    let got = filtered(&r.stdout);
    assert_eq!(
        got["results"], want["results"],
        "the rows must be exactly what the program selected"
    );
    assert_eq!(
        got["context"], want["context"],
        "context must ride along with a row projection"
    );
    assert!(
        !got["context"]["files"].as_object().unwrap().is_empty(),
        "the per-file enrichment must survive: {got}"
    );
}

/// Identity `.` round-trips the whole document under `results`, and context
/// is still emitted beside it.
#[test]
fn filter_identity_preserves_value() {
    let f = standard_repo();
    let plain = f.trace(&["grep", "helper", "--path", ".", "--json"]);
    plain.ok();
    let want = plain.json();

    let r = f.trace(&["grep", "helper", "--path", ".", "--json", "--filter", "."]);
    r.ok();
    let got = filtered(&r.stdout);
    assert_eq!(got["results"], want, "identity filter changed the document");
    assert_eq!(got["context"], want["context"]);
}

/// A selector producing many values collects them into the `results` array —
/// one document out, however many values the program produced.
#[test]
fn filter_collects_every_result_beside_context() {
    let f = standard_repo();
    let plain = f.trace(&["info", f.path("src/app.py").as_str(), "--json"]);
    plain.ok();
    let fn_count = plain.json()["results"].as_array().unwrap().len();
    // src/app.py defines exactly one function (main); pin it so the
    // collected count below is checked against a known value, not a
    // value the same command produced.
    assert_eq!(fn_count, 1, "fixture sanity: app.py has exactly one function");

    let r = f.trace(&[
        "info",
        f.path("src/app.py").as_str(),
        "--json",
        "--filter",
        ".results[].name",
    ]);
    r.ok();
    let got = filtered(&r.stdout);
    assert_eq!(
        got["results"], "main",
        "a single produced value reads as that value: {}",
        r.stdout
    );
    assert!(!got["context"].is_null(), "context must be present: {got}");
}

/// `--filter` requires `--json` explicitly, and the check runs *before* the
/// command: the error is on stderr with a non-zero exit and NO human output
/// leaks to stdout first.
#[test]
fn filter_without_json_fails_fast_with_no_output() {
    let f = standard_repo();
    let r = f.trace(&["info", f.path("src/app.py").as_str(), "--filter", ".rank"]);
    assert_ne!(r.code, 0, "expected non-zero exit");
    assert!(
        r.combined().contains("--filter requires --json"),
        "missing explicit requirement message: {}",
        r.combined()
    );
    assert!(
        !r.stdout.contains("File:") && r.stdout.trim().is_empty(),
        "human output leaked before the guard error: {:?}",
        r.stdout
    );
}

/// An invalid jq program fails loud (non-zero) with a filter diagnostic —
/// never a partial or silent result.
#[test]
fn filter_invalid_program_errors_loud() {
    let f = standard_repo();
    let r = f.trace(&[
        "info",
        f.path("src/app.py").as_str(),
        "--json",
        "--filter",
        ".[",
    ]);
    assert_ne!(r.code, 0, "invalid jq must not exit 0");
    assert!(
        r.combined().contains("--filter"),
        "error should name the filter: {}",
        r.combined()
    );
}

/// `--filter` is global: it works on a different command family with a
/// different document shape.
#[test]
fn filter_is_global_across_commands() {
    let f = standard_repo();
    let plain = f.trace(&["grep", "helper", "--path", ".", "--json"]);
    plain.ok();
    let want = plain.json()["counts"]["matches"].as_i64().unwrap();

    let r = f.trace(&[
        "grep", "helper", "--path", ".", "--json", "--filter", ".counts.matches",
    ]);
    r.ok();
    let got = filtered(&r.stdout);
    assert_eq!(got["results"].as_i64().unwrap(), want);
}

/// A command with no JSON form (`doctor`) rejects `--filter` with the same
/// explicit requirement — the contract is uniform, not per-command.
#[test]
fn filter_on_command_without_json_is_rejected() {
    let f = standard_repo();
    let r = f.trace(&["doctor", "--filter", ".anything"]);
    assert_ne!(r.code, 0, "doctor --filter must not exit 0");
    assert!(
        r.combined().contains("--filter requires --json"),
        "no-JSON command should give the same explicit error: {}",
        r.combined()
    );
}

// --- a question with no answer says so --------------------------------------

/// A path that does not exist is exit 2 and a named error, never an empty
/// document. An agent reads an empty result as "this does not exist"; it must
/// read a missing file as "you asked for the wrong path".
#[test]
fn missing_file_read_exits_2() {
    let f = Fixture::new();
    f.write("real.py", "pass\n");
    f.commit("c");
    let r = f.trace(&["read", f.path("does_not_exist.py").as_str()]);
    r.code_is(2);
    assert!(r.combined().contains("file not found"), "{}", r.combined());
}

/// `history`'s file argument is optional (file / file+symbol / `--contains`
/// modes), so a missing file is a runtime not-found rather than the argument
/// parser's exit 2. Either way it is non-zero with the same message.
#[test]
fn missing_file_history_fails_with_clear_error() {
    let f = Fixture::new();
    f.write("real.py", "pass\n");
    f.commit("c");
    let r = f.trace(&["history", f.path("ghost.py").as_str()]);
    assert_ne!(r.code, 0, "expected non-zero exit:\n{}", r.combined());
    assert!(r.combined().contains("file not found"), "{}", r.combined());
}

/// A symbol the graph does not know is exit 2, not zero callers.
#[test]
fn missing_symbol_callers_exits_2() {
    let f = Fixture::new();
    f.write("a.py", "def a():\n    return 1\n");
    f.commit("c");
    f.trace(&["cache", "build", "."]).ok();
    f.trace(&["callers", "totally_absent_symbol"]).code_is(2);
}
