//! `doctor` (present case) and the `download` command wiring.
//!
//! The doctor-absent case lives in `missing_model.rs`; here is the present
//! case (model-gated) and proof that `download` is a real, recognized
//! subcommand.

use prompt_reviewer_cli_tests::{model_present, review};

#[test]
fn doctor_reports_present_when_the_model_is_on_disk() {
    if !model_present() {
        eprintln!("skipping: model not present");
        return;
    }
    let r = review(["doctor"]);
    r.ok();
    let out = r.combined();
    assert!(
        out.contains("Present"),
        "doctor must report the model present:\n{out}"
    );
    // The fixed identity is surfaced so a reader sees exactly which weights are
    // installed.
    assert!(
        out.contains("gemma-4-E4B-it"),
        "doctor must name the fixed model identity:\n{out}"
    );
}

#[test]
fn download_is_a_recognized_subcommand() {
    // `download --help` resolves the subcommand and prints its own help —
    // proof it is wired, without triggering a 5.3 GB download.
    let r = review(["download", "--help"]);
    r.ok();
    let out = r.combined();
    assert!(
        out.contains("Download the fixed model"),
        "`download` must be a recognized subcommand with its own help:\n{out}"
    );
}

#[test]
fn download_help_does_not_mention_fetch() {
    let r = review(["download", "--help"]);
    r.ok();
    assert!(
        !r.combined().contains("fetch"),
        "the download command's help must not mention `fetch`:\n{}",
        r.combined()
    );
}
