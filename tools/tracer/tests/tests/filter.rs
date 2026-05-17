//! `--filter` — the in-process jq (jaq) over a command's JSON value.
//!
//! Black-box contract for the global filter option: it requires `--json`
//! explicitly, fails fast before the command runs when misused, streams
//! every jq result, preserves the value under identity, and is genuinely
//! global (works across command families, including ones with no JSON form
//! where it must error). Nothing here trusts internals — only exit code,
//! stdout, stderr, and JSON shape.

use tracer_cli_tests::standard_repo;

/// A scalar jq selector returns exactly that scalar — and it is the same
/// value the unfiltered `--json` document carries (no hardcoded rank).
#[test]
fn filter_extracts_scalar_matching_unfiltered_json() {
    let f = standard_repo();
    let plain = f.trace(&["info", f.path("src/app.py").as_str(), "--json"]);
    plain.ok();
    let want = plain.json()["rank"].clone();
    // src/app.py's CCN (4) ranks "low" for this fixture — pin the exact
    // value so this also guards the unfiltered rank, not just round-trip.
    assert_eq!(want, "low", "fixture sanity: app.py rank must be low");

    let r = f.trace(&[
        "info",
        f.path("src/app.py").as_str(),
        "--json",
        "--filter",
        ".rank",
    ]);
    r.ok();
    let got: serde_json::Value =
        serde_json::from_str(r.stdout.trim()).expect("filtered scalar must be valid JSON");
    assert_eq!(got, want, "--filter '.rank' must equal the unfiltered .rank");
}

/// Identity `.` round-trips the value: filtered output parses to the same
/// value the unfiltered `--json` carries. Pins that the filter path emits
/// through the stable formatter without mutating the value.
#[test]
fn filter_identity_preserves_value() {
    let f = standard_repo();
    let plain = f.trace(&["grep", "helper", "--path", ".", "--json"]);
    plain.ok();
    let want = plain.json();

    let r = f.trace(&["grep", "helper", "--path", ".", "--json", "--filter", "."]);
    r.ok();
    let got: serde_json::Value =
        serde_json::from_str(r.stdout.trim()).expect("identity filter must be valid JSON");
    assert_eq!(got, want, "identity filter changed the value");
}

/// A selector producing many results streams one JSON value per line; the
/// count matches the array it iterated.
#[test]
fn filter_streams_one_json_value_per_result() {
    let f = standard_repo();
    let plain = f.trace(&["info", f.path("src/app.py").as_str(), "--json"]);
    plain.ok();
    let fn_count = plain.json()["functions"].as_array().unwrap().len();
    // src/app.py defines exactly one function (main); pin it so the
    // streamed-line count below is checked against a known value, not a
    // value the same command produced.
    assert_eq!(fn_count, 1, "fixture sanity: app.py has exactly one function");

    let r = f.trace(&[
        "info",
        f.path("src/app.py").as_str(),
        "--json",
        "--filter",
        ".functions[].name",
    ]);
    r.ok();
    let lines: Vec<&str> = r.stdout.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(
        lines.len(),
        fn_count,
        "one streamed value per function expected: {}",
        r.stdout
    );
    for line in &lines {
        serde_json::from_str::<serde_json::Value>(line)
            .unwrap_or_else(|e| panic!("streamed line not valid JSON ({e}): {line}"));
    }
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
    let want = plain.json()["match_count"].as_i64().unwrap();

    let r = f.trace(&[
        "grep", "helper", "--path", ".", "--json", "--filter", ".match_count",
    ]);
    r.ok();
    let got: i64 = r
        .stdout
        .trim()
        .parse()
        .unwrap_or_else(|_| panic!("expected a bare number: {:?}", r.stdout));
    assert_eq!(got, want);
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
