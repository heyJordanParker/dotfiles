//! The missing-model error paths — runs unconditionally.
//!
//! These pin end-state 3: a review with no model present fails obviously
//! (non-zero exit, a message naming the download command — never a panic or a
//! silent/cryptic failure), and `doctor` reports the model missing and exits
//! non-zero. The user-overridable model path (`PROMPT_REVIEWER_MODEL`) is the
//! lever: it points at a guaranteed-absent file so these run on any machine
//! without disturbing the real cached model.

use prompt_reviewer_cli_tests::{review_env, with_missing_model};

#[test]
fn review_with_no_model_exits_nonzero_naming_download() {
    let env = with_missing_model();
    let r = review_env(
        ["a prompt to review", "--instructions", "judge clarity"],
        &[(env.0.as_str(), env.1.as_str())],
    );
    // Non-zero exit, never a panic. anyhow's Error prints to stderr; the
    // process exits 1, not via an abort.
    r.code_is(1);
    let out = r.combined();
    assert!(
        out.contains("not present"),
        "the error must say the model is not present:\n{out}"
    );
    assert!(
        out.contains("review-prompt download"),
        "the error must name the download command:\n{out}"
    );
    // Loud and specific — not a silent or cryptic failure.
    assert!(
        !out.trim().is_empty(),
        "the error must not be silent:\n{out}"
    );
}

#[test]
fn doctor_with_no_model_reports_absent_and_exits_nonzero() {
    let env = with_missing_model();
    let r = review_env(["doctor"], &[(env.0.as_str(), env.1.as_str())]);
    r.code_is(1);
    let out = r.combined();
    assert!(
        out.contains("Not present"),
        "doctor must report the model is not present:\n{out}"
    );
    assert!(
        out.contains("review-prompt download"),
        "doctor must name the download command when the model is absent:\n{out}"
    );
}

#[test]
fn missing_model_error_carries_the_overridden_path() {
    // The message reflects the path actually in effect (the override), so a
    // user pointed at a custom location is told the right place.
    let env = with_missing_model();
    let r = review_env(["doctor"], &[(env.0.as_str(), env.1.as_str())]);
    r.code_is(1);
    assert!(
        r.combined().contains(env.1.as_str()),
        "doctor must print the overridden model path {}:\n{}",
        env.1,
        r.combined()
    );
}
