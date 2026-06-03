//! Argument and usage behavior — runs unconditionally, no model required.
//!
//! Pins the CLI's invocation surface: the three refinements the user signed
//! off on (the `download` command name, a single `--stdin` for the prompt, no
//! instructions-stdin flag) plus the input mutual-exclusion and missing-input
//! errors for both inputs.

use prompt_reviewer_cli_tests::review;

#[test]
fn help_names_download_not_fetch() {
    let r = review(["--help"]);
    r.ok();
    assert!(
        r.stdout.contains("download"),
        "--help must list the `download` command:\n{}",
        r.stdout
    );
    assert!(
        !r.stdout.contains("fetch"),
        "--help must not mention `fetch` anywhere:\n{}",
        r.stdout
    );
}

#[test]
fn help_offers_single_stdin_for_the_prompt() {
    let r = review(["--help"]);
    r.ok();
    assert!(
        r.stdout.contains("--stdin"),
        "--help must offer --stdin for the prompt:\n{}",
        r.stdout
    );
    // The prompt's stdin flag is the bare `--stdin`; there is no second,
    // instructions-scoped stdin flag.
    assert!(
        !r.stdout.contains("--instructions-stdin"),
        "--help must not offer an instructions-stdin flag:\n{}",
        r.stdout
    );
}

#[test]
fn help_offers_instructions_inline_and_file_only() {
    let r = review(["--help"]);
    r.ok();
    assert!(
        r.stdout.contains("--instructions "),
        "--help must offer --instructions:\n{}",
        r.stdout
    );
    assert!(
        r.stdout.contains("--instructions-file"),
        "--help must offer --instructions-file:\n{}",
        r.stdout
    );
}

#[test]
fn fetch_is_no_longer_a_subcommand() {
    // `fetch` is not a recognized command. clap treats the bare word as the
    // positional PROMPT, so the run falls into the review path and fails on
    // the now-required instructions — never on a download attempt. The point
    // the assertion pins: no `download` work runs, and the error is the
    // missing-instructions usage error, proving `fetch` is not dispatched.
    let r = review(["fetch"]);
    r.failed();
    assert!(
        r.combined().contains("no instructions given"),
        "`fetch` must be treated as a prompt, not a subcommand:\n{}",
        r.combined()
    );
}

#[test]
fn no_arguments_prints_help() {
    // arg_required_else_help: a bare invocation shows usage rather than
    // running an empty review.
    let r = review(Vec::<String>::new());
    assert!(
        r.combined().contains("Usage:"),
        "a bare invocation must print usage:\n{}",
        r.combined()
    );
}

#[test]
fn prompt_given_two_ways_is_rejected() {
    let r = review(["a prompt", "--file", "/tmp/whatever", "--instructions", "x"]);
    r.failed();
    assert!(
        r.combined().contains("exactly one way"),
        "two prompt sources must be rejected:\n{}",
        r.combined()
    );
}

#[test]
fn no_prompt_at_all_is_rejected() {
    // Instructions present, no prompt in any form.
    let r = review(["--instructions", "judge clarity"]);
    r.failed();
    assert!(
        r.combined().contains("no prompt given"),
        "a missing prompt must be reported:\n{}",
        r.combined()
    );
    assert!(
        r.combined().contains("--stdin"),
        "the missing-prompt error must name --stdin:\n{}",
        r.combined()
    );
}

#[test]
fn instructions_given_two_ways_is_rejected() {
    let r = review([
        "a prompt",
        "--instructions",
        "inline text",
        "--instructions-file",
        "/tmp/whatever",
    ]);
    r.failed();
    assert!(
        r.combined().contains("exactly one way"),
        "two instruction sources must be rejected:\n{}",
        r.combined()
    );
}

#[test]
fn no_instructions_at_all_is_rejected() {
    let r = review(["a prompt"]);
    r.failed();
    assert!(
        r.combined().contains("no instructions given"),
        "missing instructions must be reported:\n{}",
        r.combined()
    );
    // The error names the only two ways instructions can be supplied — never a
    // stdin form, which no longer exists for instructions.
    assert!(
        r.combined().contains("--instructions")
            && r.combined().contains("--instructions-file"),
        "the missing-instructions error must name both instruction flags:\n{}",
        r.combined()
    );
    assert!(
        !r.combined().contains("--instructions-stdin"),
        "the missing-instructions error must not name an instructions-stdin form:\n{}",
        r.combined()
    );
}
