//! Per-file commands: doctor, read, info, structure, tree, list, survey,
//! context (file mode). Correctness on both human and `--json` output.

use tracer_cli_tests::{standard_repo, Fixture};

/// Run a raw git command in `root` with the suite's hermetic env plus a
/// fixed author+committer date, so a fixture can place a commit at an
/// explicit point in time. The shared `Fixture::git` clears the env and
/// pins no date; controlling the date here keeps that harness contract
/// untouched while letting one test span two age buckets.
fn git_at_date(root: &std::path::Path, date: &str, args: &[&str]) {
    let status = std::process::Command::new("git")
        .args(args)
        .current_dir(root)
        .env_clear()
        .env("PATH", "/opt/homebrew/bin:/usr/bin:/bin:/usr/local/bin")
        .env("HOME", root)
        .env("GIT_AUTHOR_NAME", "Tracer Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Tracer Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .env("GIT_AUTHOR_DATE", date)
        .env("GIT_COMMITTER_DATE", date)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap_or_else(|e| panic!("git {args:?} failed to spawn: {e}"));
    assert!(
        status.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&status.stderr)
    );
}

#[test]
fn facts_carry_first_commit_age_and_changed_together() {
    // The facts date the first and the last commit separately, and name the
    // files that change together. Two dated commits make it deterministic:
    // the file is created in 2024 alongside a sibling, then modified today.
    let f = Fixture::new();
    f.write(
        "core.py",
        "def feature(x):\n    if x:\n        return 1\n    return 0\n",
    );
    f.write("sibling.py", "x = 1\n");
    // First commit: long ago, both files together (the co-change pair).
    git_at_date(&f.root, "2024-01-01T12:00:00", &["add", "-A"]);
    git_at_date(
        &f.root,
        "2024-01-01T12:00:00",
        &["commit", "--quiet", "-m", "create core + sibling"],
    );
    // Second commit: today, touching core.py only — moves last_modified to
    // the `today` bucket while first_seen stays in the old bucket.
    f.write(
        "core.py",
        "def feature(x):\n    if x:\n        return 2\n    return 0\n",
    );
    f.commit("touch core today");

    let r = f.trace(&["read", "core.py", "--json"]);
    r.ok();
    let v = r.view();
    let git = &v["files"]["core.py"]["git"];
    // The first commit's age is time-relative (2024 → "N years ago"); the
    // last is today.
    let first = git["first_commit"].as_str().unwrap();
    assert!(first.ends_with("years ago") || first.ends_with("year ago"), "{git}");
    let last = git["last_commit"].as_str().unwrap();
    assert!(last.starts_with("today") || last.starts_with("yesterday"), "{git}");
    assert_eq!(git["usually_changed_with"], serde_json::json!(["sibling.py"]), "{git}");
}

#[test]
fn facts_split_lifetime_commits_from_recent_ones() {
    // `commits` is the lifetime total and `commits_last_30_days` the recent
    // velocity — independent signals. A file with three lifetime commits,
    // only one of them recent, must carry 3 and 1.
    let f = Fixture::new();
    // Two old commits, well outside the 30-day window.
    f.write(
        "svc.py",
        "def a(x):\n    if x:\n        return 1\n    return 0\n",
    );
    git_at_date(&f.root, "2024-01-01T12:00:00", &["add", "-A"]);
    git_at_date(
        &f.root,
        "2024-01-01T12:00:00",
        &["commit", "--quiet", "-m", "create"],
    );
    f.write(
        "svc.py",
        "def a(x):\n    if x:\n        return 2\n    return 0\n",
    );
    git_at_date(&f.root, "2024-02-01T12:00:00", &["add", "-A"]);
    git_at_date(
        &f.root,
        "2024-02-01T12:00:00",
        &["commit", "--quiet", "-m", "old edit"],
    );
    // One recent commit, today — inside the 30-day window.
    f.write(
        "svc.py",
        "def a(x):\n    if x:\n        return 3\n    return 0\n",
    );
    f.commit("recent edit");

    let r = f.trace(&["read", "svc.py", "--json"]);
    r.ok();
    let v = r.view();
    let git = &v["files"]["svc.py"]["git"];
    assert_eq!(git["commits"], 3, "{git}");
    assert_eq!(git["commits_last_30_days"], 1, "{git}");
}

#[test]
fn doctor_reports_all_required_binaries_present() {
    let f = standard_repo();
    let r = f.trace(&["doctor"]);
    r.ok();
    // The suite's environment has every external dependency installed; if it
    // didn't, every other test would be meaningless. Assert the contract.
    assert!(r.stdout.contains("Platform:"), "{}", r.stdout);
    assert!(
        r.stdout.contains("All required binaries installed."),
        "doctor did not report a clean environment:\n{}",
        r.stdout
    );
    for bin in ["ast-grep", "scc", "ctags", "git", "rg"] {
        assert!(
            r.stdout.contains(bin),
            "doctor omitted {bin}:\n{}",
            r.stdout
        );
    }
}

#[test]
fn timing_keeps_context_stdout_identical() {
    let f = standard_repo();
    let plain = f.trace(&["context", "src/util.py", "--no-record"]);
    plain.ok();
    let timed = f.trace_env(
        &["context", "src/util.py", "--no-record"],
        &[("TRACE_TIMING", "1")],
    );
    timed.ok();
    assert_eq!(timed.stdout, plain.stdout);
    for phase in ["timing freshness ", "timing decode ", "timing total "] {
        assert!(
            timed.stderr.contains(phase),
            "missing {phase}: {}",
            timed.stderr
        );
    }
}

#[test]
fn read_whole_file_human_has_front_matter_and_line_numbers() {
    let f = standard_repo();
    let r = f.trace(&["read", "src/util.py"]);
    r.ok();
    assert!(
        r.stdout.starts_with("# src/util.py\n---\nfile: src/util.py\n"),
        "missing front matter:\n{}",
        r.stdout
    );
    assert!(r.stdout.contains("def helper"), "{}", r.stdout);
    assert!(
        r.stdout.contains("L1"),
        "missing line numbering:\n{}",
        r.stdout
    );
}

#[test]
fn read_json_shape_is_stable() {
    let f = standard_repo();
    let r = f.trace(&["read", "src/util.py", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(v["results"][0]["file"], "src/util.py");
    assert_eq!(v["results"][0]["source"], "worktree");
    // src/util.py is a known four-line file; `read` line-numbers it as
    // L1..L4. Its facts are the settled single-commit fixture's: one commit
    // (in the last 30 days), no deploy branch, CCN 2 (one `if`), the files
    // committed alongside it as the changed-together set, the fixed author,
    // the init subject. The graph counts come from the relations index,
    // which is built on demand, so `read` carries them with no prior
    // `cache build`: src/app.py imports util.py, and util.py imports nothing.
    assert_eq!(
        v["results"][0]["content"].as_str().unwrap(),
        "L1: def helper(v):\nL2:     if v > 0:\nL3:         return v + 1\nL4:     return 0\n",
        "read content must be the exact line-numbered fixture file: {}",
        v["results"][0]["content"]
    );
    let facts = &v["files"]["src/util.py"];
    for (key, want) in [
        ("file", serde_json::json!("src/util.py")),
        ("lines", serde_json::json!(4)),
        ("cyclomatic_complexity", serde_json::json!(2)),
        ("complexity_rank", serde_json::json!("low")),
        ("imported_by", serde_json::json!(1)),
        ("imports", serde_json::json!(0)),
    ] {
        assert_eq!(facts[key], want, "`{key}` wrong: {facts:#}");
    }
    let git = &facts["git"];
    assert_eq!(git["status"], "unmodified", "{facts:#}");
    assert_eq!(git["commits"], 1, "{facts:#}");
    assert_eq!(git["commits_last_30_days"], 1, "{facts:#}");
    assert_eq!(git["main_author"], "Tracer Test", "{facts:#}");
    assert!(
        git["last_commit"].as_str().unwrap().ends_with("by Tracer Test: init standard repo"),
        "{facts:#}"
    );
    assert!(git["on_deploy_branches"].is_null(), "{facts:#}");
    assert_eq!(git["usually_changed_with"].as_array().unwrap().len(), 3, "{facts:#}");
    assert_eq!(facts["directory"]["path"], "src/", "{facts:#}");
    assert!(facts["nested_memories"].is_null());
}

#[test]
fn facts_count_importers_and_imports_from_the_subject_file() {
    let f = standard_repo();

    let incoming = f.trace(&["read", "src/util.py", "--json"]);
    incoming.ok();
    let util = &incoming.view()["files"]["src/util.py"];
    assert_eq!(
        (util["imported_by"].clone(), util["imports"].clone()),
        (serde_json::json!(1), serde_json::json!(0)),
        "util.py is imported by app.py and imports no internal file: {util}"
    );

    let outgoing = f.trace(&["read", "src/app.py", "--json"]);
    outgoing.ok();
    let app = &outgoing.view()["files"]["src/app.py"];
    assert_eq!(
        (app["imported_by"].clone(), app["imports"].clone()),
        (serde_json::json!(0), serde_json::json!(1)),
        "app.py imports util.py and no internal file imports app.py: {app}"
    );
}

#[test]
fn search_signpost_counts_mentioning_files_without_claiming_resolved_callers() {
    let f = Fixture::new();
    f.write("lib.py", "def dispatch(job):\n    return job\n");
    f.write(
        "app.py",
        "from lib import dispatch\n\ndef run(first, second):\n    dispatch(first)\n    return dispatch(second)\n",
    );
    f.commit("two references in one mentioning file");

    let r = f.trace(&["grep", "dispatch", ".", "--json"]);
    r.ok();
    assert_eq!(
        r.view()["signpost"].as_str().unwrap_or(""),
        "dispatch: 1 definition (function) \u{00b7} mentioned in 1 file \u{2192} trace callers dispatch",
        "the index knows one mentioning file without resolving its two call sites: {}",
        r.stdout
    );

    f.write("other.py", "from lib import dispatch\n\ndispatch(None)\n");
    f.commit("second mentioning file");
    let plural = f.trace(&["grep", "dispatch", ".", "--json"]);
    plural.ok();
    assert_eq!(
        plural.view()["signpost"].as_str().unwrap_or(""),
        "dispatch: 1 definition (function) \u{00b7} mentioned in 2 files \u{2192} trace callers dispatch",
        "plural grammar must follow the number of mentioning files: {}",
        plural.stdout
    );
}

#[test]
fn callers_keeps_complete_facts_beyond_one_resolve_chunk() {
    let f = Fixture::new();
    f.write("target.py", "def shared_target(value):\n    return value\n");
    for index in 0..513 {
        f.write(
            &format!("callers/caller_{index:03}.py"),
            &format!(
                "from target import shared_target\n\ndef caller_{index:03}(value):\n    shared_target(value)\n    return shared_target(value + 1)\n"
            ),
        );
    }
    f.commit("broad callers");

    let r = f.trace(&["callers", "shared_target", "--limit", "2000", "--json"]);
    r.ok();
    let v = r.view();
    let callers = v["results"][0]["callers"]
        .as_array()
        .expect("shared_target must contain its caller rows");
    let files = v["files"]
        .as_object()
        .expect("callers context must contain a files object");

    assert_eq!(
        callers.len(),
        1026,
        "both calls in every file must remain visible"
    );
    assert_eq!(
        files.len(),
        513,
        "duplicate caller rows must resolve to one facts entry per mentioning file"
    );
    for index in 0..513 {
        let path = format!("callers/caller_{index:03}.py");
        let facts = &files[&path];
        assert_eq!(facts["file"], path.as_str(), "{path} lost its facts: {facts}");
        assert_eq!(facts["lines"], 5, "{facts}");
        assert_eq!(facts["cyclomatic_complexity"], 1, "{facts}");
        assert_eq!(facts["imported_by"], 0, "{facts}");
        assert_eq!(facts["imports"], 1, "{facts}");
        assert_eq!(facts["git"]["commits"], 1, "{facts}");
    }
}

/// A file whose rendered read runs far past the size budget. No comments,
/// blank lines, or separators, so `clean` drops nothing and a cleaned line
/// number equals its source line number.
fn oversized_python(lines: usize) -> String {
    (1..=lines)
        .map(|n| format!("value_{n} = {n} + 1000000\n"))
        .collect()
}

/// The `[trimmed at …]` line out of a content stream, or None.
fn trim_marker_of(content: &str) -> Option<&str> {
    content.lines().find(|l| l.starts_with("[trimmed at "))
}

#[test]
fn read_over_budget_trims_at_a_line_and_marks_the_cut() {
    // The trim must land on a whole line, and the marker must name the real
    // last line shown and the file's true total — an agent that receives a
    // trimmed read must not be able to mistake it for the whole file.
    let f = standard_repo();
    f.write("big.py", &oversized_python(2000));
    f.commit("add big.py");

    let r = f.trace(&["read", "big.py", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(v["results"][0]["truncated"], true);
    assert_eq!(v["results"][0]["total_lines"], 2000);
    let last_shown = v["results"][0]["shown_lines"][1].as_i64().unwrap();
    assert!(
        last_shown > 0 && last_shown < 2000,
        "expected a partial window, got {last_shown}"
    );

    let content = v["results"][0]["content"].as_str().unwrap();
    let marker = trim_marker_of(content).unwrap_or_else(|| panic!("no trim marker in:\n{content}"));
    assert!(
        marker.contains(&format!("[trimmed at L{last_shown} of 2000 ")),
        "marker must name the last line shown and the true total: {marker}"
    );
    // The cut is on a line boundary: the last shown line is present whole
    // and the one after it is absent.
    assert!(
        content.contains(&format!("value_{last_shown} = ")),
        "last shown line was cut mid-line"
    );
    assert!(
        !content.contains(&format!("value_{} = ", last_shown + 1)),
        "line past the cut leaked in"
    );
}

#[test]
fn read_trim_marker_command_returns_the_next_window() {
    // The marker's whole value is that continuing costs no thinking. A
    // marker suggesting a command that errors or returns the wrong window is
    // worse than no marker, so run the command it prints, verbatim.
    let f = standard_repo();
    f.write("big.py", &oversized_python(2000));
    f.commit("add big.py");

    let first = f.trace(&["read", "big.py", "--json"]);
    first.ok();
    let v = first.view();
    let last_shown = v["results"][0]["shown_lines"][1].as_i64().unwrap();
    let marker = trim_marker_of(v["results"][0]["content"].as_str().unwrap()).expect("trim marker");

    let command = marker
        .split_once("continue: ")
        .expect("marker must name the continue command")
        .1
        .trim_end_matches(']');
    let printed: Vec<&str> = command.split_whitespace().collect();
    assert_eq!(
        printed[0], "trace",
        "the marker must print a runnable trace command: {command}"
    );

    let mut args: Vec<&str> = printed[1..].to_vec();
    args.push("--json");
    let next = f.trace(&args);
    next.ok();
    let v2 = next.view();
    assert_eq!(
        v2["results"][0]["shown_lines"][0].as_i64().unwrap(),
        last_shown + 1,
        "the suggested command must resume on the line after the cut"
    );
}

#[test]
fn read_under_budget_carries_no_marker() {
    let f = standard_repo();
    let r = f.trace(&["read", "src/util.py", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(v["results"][0]["truncated"], false);
    assert_eq!(v["results"][0]["total_lines"], 4);
    assert_eq!(v["results"][0]["shown_lines"], serde_json::json!([1, 4]));
    assert!(trim_marker_of(v["results"][0]["content"].as_str().unwrap()).is_none());
}

#[test]
fn read_all_flag_returns_the_whole_file() {
    let f = standard_repo();
    f.write("big.py", &oversized_python(2000));
    f.commit("add big.py");

    let r = f.trace(&["read", "big.py", "--all", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(v["results"][0]["truncated"], false);
    assert_eq!(v["results"][0]["shown_lines"], serde_json::json!([1, 2000]));
    let content = v["results"][0]["content"].as_str().unwrap();
    assert!(
        trim_marker_of(content).is_none(),
        "--all must not mark a trim"
    );
    assert!(
        content.contains("value_2000 = "),
        "--all must return the last line"
    );
}

#[test]
fn read_raw_over_budget_is_trimmed_and_marked() {
    // `--raw` skips cleaning, not truthfulness. It also skips the `L<n>: `
    // prefixes, so the marker is the only line number the output carries.
    let f = standard_repo();
    f.write("big.py", &oversized_python(2000));
    f.commit("add big.py");

    let r = f.trace(&["read", "big.py", "--raw", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(v["results"][0]["truncated"], true);
    let marker = trim_marker_of(v["results"][0]["content"].as_str().unwrap())
        .expect("trim marker under --raw");
    assert!(
        marker.ends_with("--raw]"),
        "the continue command must keep --raw: {marker}"
    );

    let last_shown = v["results"][0]["shown_lines"][1].as_i64().unwrap();
    let next = f.trace(&[
        "read",
        "big.py",
        "--lines",
        &format!("{}:2000", last_shown + 1),
        "--raw",
        "--json",
    ]);
    next.ok();
    assert_eq!(
        next.view()["results"][0]["shown_lines"][0]
            .as_i64()
            .unwrap(),
        last_shown + 1
    );
}

#[test]
fn read_cuts_a_line_longer_than_the_budget() {
    // A minified bundle is one line longer than the whole budget. Returned
    // whole, a megabyte line reaches the harness, which cuts it to a preview;
    // cut here, the line keeps its `L<n>: ` prefix, the payload says it was
    // cut, and the marker names the command for the whole line.
    let f = standard_repo();
    f.write(
        "bundle.js",
        &format!("var x = \"{}\";\n", "y".repeat(40_000)),
    );
    f.commit("add bundle.js");

    let r = f.trace(&["read", "bundle.js", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(v["results"][0]["truncated"], true);
    assert_eq!(v["results"][0]["total_lines"], 1);
    let content = v["results"][0]["content"].as_str().unwrap();
    assert!(content.starts_with("L1: var x = \"yyy"), "{}", &content[..80]);
    assert!(
        content.contains("[trimmed at L1 of 1 — whole: trace read bundle.js --all]"),
        "the cut must name the command for the whole line"
    );

    let text = f.trace(&["read", "bundle.js", "--budget", "3000"]);
    text.ok();
    let size = text.stdout.chars().count();
    assert!(size <= 3000, "read spent {size} characters of a 3000 budget");

    let whole = f.trace(&["read", "bundle.js", "--all"]);
    whole.ok();
    assert!(whole.stdout.contains(&"y".repeat(40_000)), "--all returns the whole line");
}

#[test]
fn read_trim_marker_echoes_the_path_the_caller_used() {
    // The marker's command has to run from the caller's own directory. A
    // repo-relative path is not runnable for a file addressed any other way,
    // so the marker repeats the argument as given.
    let f = standard_repo();
    f.write("big.py", &oversized_python(2000));
    f.commit("add big.py");

    let r = f.trace(&["read", "./big.py", "--json"]);
    r.ok();
    let v = r.view();
    let marker = trim_marker_of(v["results"][0]["content"].as_str().unwrap()).expect("trim marker");
    assert!(
        marker.contains("trace read ./big.py --lines"),
        "the marker must repeat the caller's path: {marker}"
    );
}

#[test]
fn read_trim_marker_quotes_a_path_with_whitespace() {
    let f = standard_repo();
    f.write("spaced name.py", &oversized_python(2000));
    f.commit("add spaced name.py");

    let r = f.trace(&["read", "spaced name.py", "--json"]);
    r.ok();
    let v = r.view();
    let marker = trim_marker_of(v["results"][0]["content"].as_str().unwrap()).expect("trim marker");
    assert!(
        marker.contains("trace read 'spaced name.py' --lines"),
        "a path with whitespace must be shell-quoted: {marker}"
    );
}

#[test]
fn read_between_over_budget_continues_inside_the_anchor_section() {
    // The window to continue is the rest of the anchored section, never the
    // rest of the file.
    let f = standard_repo();
    f.write("big.py", &oversized_python(2000));
    f.commit("add big.py");

    let r = f.trace(&[
        "read",
        "big.py",
        "--between",
        "value_100 ",
        "value_1900 ",
        "--json",
    ]);
    r.ok();
    let v = r.view();
    assert_eq!(v["results"][0]["truncated"], true);
    assert_eq!(
        v["results"][0]["between_resolved_lines"],
        serde_json::json!([100, 1900])
    );
    let last_shown = v["results"][0]["shown_lines"][1].as_i64().unwrap();
    let marker = trim_marker_of(v["results"][0]["content"].as_str().unwrap()).expect("trim marker");
    assert!(
        marker.contains(&format!("--lines {}:1900]", last_shown + 1)),
        "the continue window must end at the anchor, not at the file end: {marker}"
    );
}

#[test]
fn read_at_ref_over_budget_keeps_the_ref_in_the_continue_command() {
    let f = standard_repo();
    f.write("big.py", &oversized_python(2000));
    f.commit("add big.py");

    let r = f.trace(&["read", "big.py", "--at", "HEAD", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(v["results"][0]["truncated"], true);
    let marker = trim_marker_of(v["results"][0]["content"].as_str().unwrap()).expect("trim marker");
    assert!(
        marker.ends_with("--at HEAD]"),
        "the continue command must stay on the ref: {marker}"
    );
}

#[test]
fn read_empty_selection_reports_no_window() {
    let f = standard_repo();
    let r = f.trace(&["read", "src/util.py", "--lines", "500:600", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(v["results"][0]["truncated"], false);
    assert!(v["results"][0]["shown_lines"].is_null());
    assert_eq!(v["results"][0]["total_lines"], 4);
}

#[test]
fn read_multi_file_caps_each_file_separately() {
    // One large file must not starve the others of their content.
    let f = standard_repo();
    f.write("big.py", &oversized_python(2000));
    f.commit("add big.py");

    let r = f.trace(&[
        "read",
        "big.py",
        "src/util.py",
        "--lines",
        "1:5000",
        "--json",
    ]);
    r.ok();
    let v = r.view();
    let files = v["results"].as_array().unwrap();
    assert_eq!(files.len(), 2);
    assert_eq!(files[0]["truncated"], true);
    assert_eq!(files[1]["truncated"], false);
    assert_eq!(files[1]["shown_lines"], serde_json::json!([1, 4]));
}

#[test]
fn read_method_scopes_to_one_function() {
    // app.py's main() spans L5-L12 (def..return None). Extraction must
    // return exactly that span, line-numbered from 5, with the leading
    // imports excluded and no trailing code.
    let f = standard_repo();
    let r = f.trace(&["read", "src/app.py", "--method", "main", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(v["method"], "main");
    let body = v["results"][0]["content"].as_str().unwrap();
    assert_eq!(
        body,
        concat!(
            "L 5: def main(x):\n",
            "L 6:     \"\"\"Entry point with real branching.\"\"\"\n",
            "L 7:     if x:\n",
            "L 8:         return helper(x)\n",
            "L 9:     for i in range(10):\n",
            "L10:         if i % 2 == 0:\n",
            "L11:             print(i)\n",
            "L12:     return None\n",
        ),
        "method extraction returned the wrong span:\n{body:?}"
    );
}

#[test]
fn read_method_bare_name_reaches_a_php_method_with_its_markers() {
    // The function table names a PHP method `Class::name`. The bare name an
    // Agent types must reach it, and the span must start at the attribute
    // above the signature with the docblock pulled in, and stop there: the
    // member above opens with a one-line docblock and is code, not comment.
    let f = Fixture::new();
    f.write(
        "src/Account.php",
        concat!(
            "<?php\n",
            "final class Account {\n",
            "  /** @internal */ public function id(): int { return 1; }\n",
            "\n",
            "  /** @internal */\n",
            "  #[Internal]\n",
            "  public function rename(string $name): void {\n",
            "    $this->name = $name;\n",
            "  }\n",
            "}\n",
        ),
    );
    f.commit("php method");
    let r = f.trace(&["read", "src/Account.php", "--method", "rename", "--json"]);
    r.ok();
    let body = r.view()["results"][0]["content"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        body,
        concat!(
            "L5:   /** @internal */\n",
            "L6:   #[Internal]\n",
            "L7:   public function rename(string $name): void {\n",
            "L8:     $this->name = $name;\n",
            "L9:   }\n",
        ),
        "bare-name method read returned the wrong span:\n{body:?}"
    );
}

#[test]
fn read_method_at_ref_extracts_committed_body() {
    // The method must be carved out of the *committed* file, not the
    // worktree: the worktree version of fn has a different body and an
    // extra function that must not appear.
    let f = Fixture::new();
    f.write(
        "m.py",
        "def target(n):\n    if n:\n        return n\n    return 0\n",
    );
    f.commit("v1");
    f.write(
        "m.py",
        concat!(
            "def target(n):\n",
            "    return n * 2\n",
            "\n",
            "def added_later():\n",
            "    return 1\n",
        ),
    );
    let r = f.trace(&["read", "m.py", "--method", "target", "--at", "HEAD", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(v["results"][0]["source"], "ref");
    assert_eq!(v["method"], "target");
    let body = v["results"][0]["content"].as_str().unwrap();
    assert_eq!(
        body,
        concat!(
            "L1: def target(n):\n",
            "L2:     if n:\n",
            "L3:         return n\n",
            "L4:     return 0\n",
        ),
        "method-at-ref returned worktree body or wrong span:\n{body:?}"
    );
    assert!(
        !body.contains("added_later") && !body.contains("n * 2"),
        "method-at-ref leaked worktree-only content:\n{body}"
    );
}

#[test]
fn read_multi_file_returns_one_payload_per_file() {
    // Two positional files with a shared scope (--lines) must come back as
    // the `results` rows, one payload each, in argument order, each
    // carrying that file's own clamped content.
    let f = Fixture::new();
    f.write("a.py", "AONE = 1\nATWO = 2\nATHREE = 3\n");
    f.write("b.py", "BONE = 10\nBTWO = 20\nBTHREE = 30\n");
    f.commit("two files");
    let r = f.trace(&["read", "a.py", "b.py", "--lines", "1:2", "--json"]);
    r.ok();
    let v = r.view();
    let files = v["results"]
        .as_array()
        .expect("multi-file read must carry one row per file");
    assert_eq!(files.len(), 2, "expected exactly two file payloads: {v}");
    assert_eq!(files[0]["file"], "a.py");
    assert_eq!(files[1]["file"], "b.py");
    assert_eq!(
        files[0]["content"].as_str().unwrap(),
        "L1: AONE = 1\nL2: ATWO = 2\n",
        "first file content wrong:\n{}",
        files[0]["content"]
    );
    assert_eq!(
        files[1]["content"].as_str().unwrap(),
        "L1: BONE = 10\nL2: BTWO = 20\n",
        "second file content wrong:\n{}",
        files[1]["content"]
    );
}

#[test]
fn read_line_range_in_range_returns_exact_lines() {
    // src/util.py is exactly:
    //   L1: def helper(v):
    //   L2:     if v > 0:
    //   L3:         return v + 1
    //   L4:     return 0
    let f = standard_repo();
    let r = f.trace(&["read", "src/util.py", "--lines", "2:3", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(v["lines"][0], 2);
    assert_eq!(v["lines"][1], 3);
    let content = v["results"][0]["content"].as_str().unwrap();
    // Exactly lines 2 and 3, line-numbered, nothing else.
    assert_eq!(
        content, "L2:     if v > 0:\nL3:         return v + 1\n",
        "in-range read returned wrong content:\n{content:?}"
    );
}

#[test]
fn read_line_range_clamps_out_of_range_upper_bound() {
    // Upper bound past EOF must clamp to the last real line (4), not
    // fabricate or truncate. The echoed `lines` still reflects the request.
    let f = standard_repo();
    let r = f.trace(&["read", "src/util.py", "--lines", "3:99", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(v["lines"][0], 3);
    assert_eq!(v["lines"][1], 99);
    let content = v["results"][0]["content"].as_str().unwrap();
    assert_eq!(
        content, "L3:         return v + 1\nL4:     return 0\n",
        "out-of-range upper bound did not clamp to EOF:\n{content:?}"
    );
}

#[test]
fn read_line_range_entirely_past_eof_is_empty() {
    // A whole range beyond the file: clamping yields nothing. The request
    // is still echoed; the body is empty (not the whole file, not an error).
    let f = standard_repo();
    let r = f.trace(&["read", "src/util.py", "--lines", "10:20", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(v["lines"][0], 10);
    assert_eq!(v["lines"][1], 20);
    assert_eq!(
        v["results"][0]["content"].as_str().unwrap(),
        "",
        "a range entirely past EOF must yield empty content, got:\n{}",
        v["results"][0]["content"]
    );
}

#[test]
fn read_line_range_reversed_is_rejected() {
    // L2:L1 is not a valid range — it must fail explicitly (exit 2),
    // never silently swap the bounds or return the whole file.
    let f = standard_repo();
    let r = f.trace(&["read", "src/util.py", "--lines", "3:1", "--json"]);
    r.code_is(2);
    assert!(
        r.combined().contains("end line 1 must be >= start line 3"),
        "reversed range gave the wrong error:\n{}",
        r.combined()
    );
}

#[test]
fn read_raw_skips_fluff_stripping() {
    let f = Fixture::new();
    f.write(
        "lic.py",
        "# Copyright (c) 2026 Someone. All rights reserved.\n\
         # License: MIT\n\ndef keep():\n    return 1\n",
    );
    f.commit("add licensed file");
    let stripped = f.trace(&["read", "lic.py"]);
    stripped.ok();
    assert!(
        !stripped.stdout.contains("All rights reserved"),
        "license header should be stripped by default:\n{}",
        stripped.stdout
    );
    let raw = f.trace(&["read", "lic.py", "--raw"]);
    raw.ok();
    assert!(
        raw.stdout.contains("All rights reserved"),
        "--raw must preserve the header:\n{}",
        raw.stdout
    );
}

#[test]
fn read_cleaning_removes_only_standalone_generated_comment_banners() {
    let f = Fixture::new();
    f.write(
        "instructions.md",
        "# License and copyright notes\n\
         * See the license file\n\
         Generated bytes await the integrated gate.\n\
         This Markdown instruction says do not edit the source text.\n\
         # Do not edit production\n\
         * Do not edit production\n",
    );
    f.write(
        "generated.rs",
        "// Code generated by tracer. DO NOT EDIT.\n\
         const MARKER: &str = \"generated by\";\n\
         // Code generated by tracer. DO NOT EDIT.\n\
         const INSTRUCTION: &str = \"do not edit\";\n\
         // Generated bytes are preserved here.\n\
         // Do not edit this variable while iterating.\n\
         const RAW: &str = r#\"\n\
         // Code generated by tracer. DO NOT EDIT.\n\
         \"#;\n\
         /* Generated by tracer. */ const VALUE: usize = 1;\n",
    );
    f.write(
        "licensed.rs",
        "#!/usr/bin/env rust-script\n\
         \n\
         /*\n\
          * Copyright (c) 2026 Example\n\
          * License: MIT\n\
          */\n\
         fn licensed() {}\n",
    );
    f.write(
        "Cargo.lock",
        "# This file is automatically @generated by Cargo.\n\
         # It is not intended for manual editing.\n\
         version = 4\n",
    );
    f.write(
        "scoped.rs",
        "// Code generated by tracer.\n\
         const START: usize = 1;\n\
         // Code generated by tracer.\n\
         fn body() -> usize { START }\n\
         const END: usize = 2;\n",
    );
    f.write(
        "uncertain.rs",
        "/* Generated by tracer.\n\
         /* nested */\n\
         */\n\
         const TEXT: &str = \"// Code generated by tracer.\";\n",
    );
    f.write(
        "first-line-nested.rs",
        "/* Generated by tracer. /* nested */\n\
         */\n\
         const KEPT: usize = 1;\n",
    );
    f.write(
        "ordinary-license.rs",
        "// Keep license_cache intact.\n\
         // See the license file before changing this.\n\
         const KEPT: usize = 1;\n",
    );
    f.write(
        "line-endings.rs",
        "// Code generated by tracer.\r\nconst CRLF: usize = 1;\r// Generated by tracer.\rconst BARE: usize = 2;\r",
    );
    f.commit("add read cleaning fixtures");

    let prose = f.trace(&["read", "instructions.md"]);
    prose.ok();
    assert!(
        prose
            .stdout
            .contains("L3: Generated bytes await the integrated gate."),
        "ordinary prose containing marker-like text disappeared:\n{}",
        prose.stdout
    );
    assert!(
        prose
            .stdout
            .contains("L4: This Markdown instruction says do not edit the source text."),
        "Markdown instruction containing a generated marker disappeared:\n{}",
        prose.stdout
    );
    assert!(
        prose.stdout.contains("L1: # License and copyright notes")
            && prose.stdout.contains("L2: * See the license file")
            && prose.stdout.contains("L5: # Do not edit production")
            && prose.stdout.contains("L6: * Do not edit production"),
        "Markdown heading or list instruction disappeared:\n{}",
        prose.stdout
    );

    let prose_json = f.trace(&["read", "instructions.md", "--json"]);
    prose_json.ok();
    assert_eq!(
        prose_json.json()["results"][0]["content"].as_str().unwrap(),
        "L1: # License and copyright notes\n\
         L2: * See the license file\n\
         L3: Generated bytes await the integrated gate.\n\
         L4: This Markdown instruction says do not edit the source text.\n\
         L5: # Do not edit production\n\
         L6: * Do not edit production\n",
        "JSON read must preserve complete Markdown instructions"
    );

    let source_human = f.trace(&["read", "generated.rs"]);
    source_human.ok();
    assert!(
        source_human
            .stdout
            .contains("L 5: // Generated bytes are preserved here.")
            && source_human
                .stdout
                .contains("L 6: // Do not edit this variable while iterating.")
            && source_human
                .stdout
                .contains("L 8: // Code generated by tracer. DO NOT EDIT.")
            && source_human
                .stdout
                .contains("L10: /* Generated by tracer. */ const VALUE: usize = 1;"),
        "ordinary source comments disappeared:\n{}",
        source_human.stdout
    );
    assert!(
        !source_human
            .stdout
            .contains("L 1: // Code generated by tracer"),
        "leading generated banner survived cleaning:\n{}",
        source_human.stdout
    );

    let source = f.trace(&["read", "generated.rs", "--json"]);
    source.ok();
    let document = source.json();
    for slot in ["query", "context", "results", "counts"] {
        assert!(
            document.get(slot).is_some(),
            "read JSON lost the {slot} slot: {document}"
        );
    }
    let content = document["results"][0]["content"].as_str().unwrap();
    assert_eq!(
        content,
        "L 2: const MARKER: &str = \"generated by\";\n\
         L 3: // Code generated by tracer. DO NOT EDIT.\n\
         L 4: const INSTRUCTION: &str = \"do not edit\";\n\
         L 5: // Generated bytes are preserved here.\n\
         L 6: // Do not edit this variable while iterating.\n\
         L 7: const RAW: &str = r#\"\n\
         L 8: // Code generated by tracer. DO NOT EDIT.\n\
         L 9: \"#;\n\
         L10: /* Generated by tracer. */ const VALUE: usize = 1;\n",
        "cleaned source must preserve strings and source line numbers around the removed banner"
    );
    assert_eq!(
        document["context"]["files"]["generated.rs"]["file"], "generated.rs",
        "cleaned JSON read lost passive file context: {document}"
    );

    let raw = f.trace(&["read", "generated.rs", "--raw"]);
    raw.ok();
    assert!(
        raw.stdout
            .contains("// Code generated by tracer. DO NOT EDIT."),
        "--raw must preserve the generated banner:\n{}",
        raw.stdout
    );

    let inner_range = f.trace(&["read", "generated.rs", "--lines", "8:8"]);
    inner_range.ok();
    assert!(
        inner_range
            .stdout
            .contains("L8: // Code generated by tracer. DO NOT EDIT."),
        "a scoped read inside a raw string became a new header:\n{}",
        inner_range.stdout
    );

    let licensed = f.trace(&["read", "licensed.rs"]);
    licensed.ok();
    assert!(
        licensed.stdout.contains("#!/usr/bin/env rust-script")
            && licensed.stdout.contains("L7: fn licensed() {}")
            && !licensed.stdout.contains("Copyright")
            && !licensed.stdout.contains("License: MIT"),
        "complete leading block license header was not removed conservatively:\n{}",
        licensed.stdout
    );

    let lock = f.trace(&["read", "Cargo.lock"]);
    lock.ok();
    assert!(
        lock.stdout.contains("L3: version = 4")
            && !lock.stdout.contains("automatically @generated")
            && !lock.stdout.contains("manual editing"),
        "Cargo.lock's real generated header was not removed as one leading comment block:\n{}",
        lock.stdout
    );

    let committed_range = f.trace(&["read", "generated.rs", "--lines", "1:3", "--at", "HEAD"]);
    committed_range.ok();
    assert!(
        committed_range.stdout.contains("L2: const MARKER")
            && committed_range
                .stdout
                .contains("L3: // Code generated by tracer")
            && !committed_range
                .stdout
                .contains("L1: // Code generated by tracer"),
        "ref range did not use the original source's header eligibility:\n{}",
        committed_range.stdout
    );

    let method = f.trace(&["read", "scoped.rs", "--method", "body"]);
    method.ok();
    assert!(
        method.stdout.contains("L3: // Code generated by tracer.")
            && method.stdout.contains("L4: fn body() -> usize { START }"),
        "a method selection reclassified its attached body comment as a header:\n{}",
        method.stdout
    );

    let between = f.trace(&["read", "scoped.rs", "--between", "START", "END"]);
    between.ok();
    assert!(
        between.stdout.contains("L3: // Code generated by tracer.")
            && !between.stdout.contains("L1: // Code generated by tracer."),
        "an anchor selection did not retain original-source header eligibility:\n{}",
        between.stdout
    );

    let uncertain = f.trace(&["read", "uncertain.rs"]);
    uncertain.ok();
    assert!(
        uncertain.stdout.contains("L1: /* Generated by tracer.")
            && uncertain.stdout.contains("L2: /* nested */")
            && uncertain
                .stdout
                .contains("L4: const TEXT: &str = \"// Code generated by tracer.\";"),
        "ambiguous comments or source strings were removed:\n{}",
        uncertain.stdout
    );

    let first_line_nested = f.trace(&["read", "first-line-nested.rs"]);
    first_line_nested.ok();
    assert!(
        first_line_nested
            .stdout
            .contains("L1: /* Generated by tracer. /* nested */")
            && first_line_nested
                .stdout
                .contains("L3: const KEPT: usize = 1;"),
        "a nested opener on the first block line was guessed to be a notice:\n{}",
        first_line_nested.stdout
    );

    let ordinary_license = f.trace(&["read", "ordinary-license.rs"]);
    ordinary_license.ok();
    assert!(
        ordinary_license
            .stdout
            .contains("L1: // Keep license_cache intact.")
            && ordinary_license
                .stdout
                .contains("L2: // See the license file before changing this."),
        "ordinary leading license instructions were removed:\n{}",
        ordinary_license.stdout
    );

    let crlf = f.trace(&["read", "line-endings.rs", "--lines", "1:2"]);
    crlf.ok();
    assert!(
        crlf.stdout.contains("L2: const CRLF: usize = 1;")
            && !crlf.stdout.contains("L1: // Code generated by tracer."),
        "CRLF selection lost original-source header line numbers:\n{}",
        crlf.stdout
    );
    let bare_cr = f.trace(&["read", "line-endings.rs", "--lines", "2:2"]);
    bare_cr.ok();
    assert!(
        bare_cr.stdout.contains(
            "L2: const CRLF: usize = 1;\r// Generated by tracer.\rconst BARE: usize = 2;\r"
        ),
        "bare CR bytes did not remain content in the established LF line:\n{}",
        bare_cr.stdout
    );
}

#[test]
fn read_preserves_mixed_line_source_identity_across_modes() {
    let f = Fixture::new();
    let mut source = String::from(
        "// Generated by tracer.\r// header\r// header\n\
         const START: usize = 1;\n\
         fn picked() -> usize {\r\n\
             START\r\
         }\n\
         const END: usize = 2;\n",
    );
    for n in 1..=600 {
        source.push_str(&format!("// padding {n:03} {}", "x".repeat(80)));
        source.push_str(match n % 3 {
            0 => "\r\n",
            1 => "\r",
            _ => "\n",
        });
    }
    f.write("mixed.rs", &source);
    f.commit("add mixed line source");

    let whole = f.trace(&["read", "mixed.rs", "--all", "--json"]);
    whole.ok();
    let whole_content = whole.json()["results"][0]["content"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(whole_content.contains("L  1: // Generated by tracer.\r// header\r// header"));
    assert!(whole_content.contains("L  2: const START: usize = 1;"));
    assert!(whole_content.contains("L  3: fn picked() -> usize {"));
    assert!(whole_content.contains("L  4: START\r}"));

    let range = f.trace(&["read", "mixed.rs", "--lines", "2:3", "--json"]);
    range.ok();
    assert_eq!(
        range.json()["results"][0]["content"],
        "L2: const START: usize = 1;\nL3: fn picked() -> usize {\r\n"
    );

    let method = f.trace(&["read", "mixed.rs", "--method", "picked", "--json"]);
    method.ok();
    let method_content = method.json()["results"][0]["content"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(method_content.contains("L3: fn picked() -> usize {"));
    assert!(method_content.contains("L4: START\r}"));

    let between = f.trace(&[
        "read",
        "mixed.rs",
        "--between",
        "const START",
        "const END",
        "--json",
    ]);
    between.ok();
    let between_json = between.json();
    let between_content = between_json["results"][0]["content"].as_str().unwrap();
    assert!(between_content.contains("L2: const START: usize = 1;"));
    assert!(between_content.contains("L5: const END: usize = 2;"));
    assert_eq!(
        between_json["results"][0]["between_resolved_lines"],
        serde_json::json!([2, 5])
    );

    let historical = f.trace(&[
        "read",
        "mixed.rs",
        "--between",
        "const START",
        "const END",
        "--at",
        "HEAD",
        "--json",
    ]);
    historical.ok();
    assert_eq!(
        historical.json()["results"][0]["between_resolved_lines"],
        serde_json::json!([2, 5])
    );

    let raw = f.trace(&["read", "mixed.rs", "--raw", "--all", "--json"]);
    raw.ok();
    assert_eq!(raw.json()["results"][0]["content"], source);

    let budgeted = f.trace(&["read", "mixed.rs", "--raw", "--json"]);
    budgeted.ok();
    let budgeted_json = budgeted.json();
    assert_eq!(budgeted_json["results"][0]["truncated"], true);
    let emitted = budgeted_json["results"][0]["content"]
        .as_str()
        .unwrap()
        .split("\n[trimmed at ")
        .next()
        .unwrap();
    let emitted_lines = emitted.lines().count();
    assert_eq!(
        budgeted_json["results"][0]["shown_lines"][1]
            .as_u64()
            .unwrap() as usize,
        emitted_lines
    );

    f.write("mixed.rs", &source.replace("START\r", "START + 1\r"));
    let diff = f.trace(&["read", "mixed.rs", "--at", "HEAD", "--diff", "--json"]);
    diff.ok();
    assert!(
        diff.json()["context"]["files"]["mixed.rs"]["symbol_diff"]["changed"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["name"] == "picked")
    );
}

#[test]
fn read_between_matches_anchors_without_line_terminators() {
    let f = Fixture::new();
    f.write("anchors.txt", "before\nSTART\nkeep\nEND\ntrailing\n");
    f.commit("add anchored section");

    let read = f.trace(&[
        "read",
        "anchors.txt",
        "--between",
        "^START$",
        "^END$",
        "--raw",
        "--json",
    ]);
    read.ok();
    let document = read.json();
    assert_eq!(document["results"][0]["content"], "START\nkeep\nEND\n");
    assert_eq!(
        document["results"][0]["between_resolved_lines"],
        serde_json::json!([2, 4])
    );
}

#[test]
fn read_keeps_ambiguous_leading_comments() {
    let f = Fixture::new();
    f.write(
        "plain.rs",
        "// License checking happens in verify().\nfn verify() {}\n",
    );
    f.write(
        "settings.toml",
        "# License key configuration\nlicense_key = \"test\"\n",
    );
    f.write(
        "comments.lua",
        "-- Generated by tracer.\n--[[\nimportant explanatory text\n]]\nlocal value = 1\n",
    );
    f.write(
        "equals.LUA",
        "-- Generated by tracer.\n--[=[\nimportant explanatory text\n]=]\nlocal value = 1\n",
    );
    f.commit("add ambiguous leading comments");

    let rust = f.trace(&["read", "plain.rs"]);
    rust.ok();
    assert!(rust
        .stdout
        .contains("License checking happens in verify()."));

    let toml = f.trace(&["read", "settings.toml"]);
    toml.ok();
    assert!(toml.stdout.contains("License key configuration"));

    for file in ["comments.lua", "equals.LUA"] {
        let lua = f.trace(&["read", file]);
        lua.ok();
        assert!(
            lua.stdout.contains("Generated by tracer."),
            "{}",
            lua.stdout
        );
        assert!(
            lua.stdout.contains("important explanatory text"),
            "{}",
            lua.stdout
        );
        assert!(lua.stdout.contains("local value = 1"), "{}", lua.stdout);
    }
}

#[test]
fn read_at_ref_reads_committed_content() {
    let f = Fixture::new();
    f.write("v.py", "VALUE = 1\n");
    f.commit("v1");
    f.write("v.py", "VALUE = 2\n");
    let r = f.trace(&["read", "v.py", "--at", "HEAD", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(v["results"][0]["source"], "ref");
    assert!(
        v["results"][0]["content"]
            .as_str()
            .unwrap()
            .contains("VALUE = 1"),
        "ref read returned worktree content:\n{}",
        v["results"][0]["content"]
    );
}

#[test]
fn read_diff_requires_at_ref() {
    let f = standard_repo();
    let r = f.trace(&["read", "src/util.py", "--diff"]);
    r.code_is(2);
    assert!(
        r.combined().contains("--diff requires --at"),
        "{}",
        r.combined()
    );
}

#[test]
fn read_symbol_diff_partitions_added_removed_changed() {
    // Commit a module with three top-level functions, then in the worktree:
    //   - remove `gone`
    //   - add `fresh`
    //   - change the body of `kept` (signature/name unchanged)
    //   - leave `stable` byte-identical
    // `read --at HEAD --diff` compares the ref against the worktree, so the
    // added/removed/changed sets are exactly that partition.
    let f = Fixture::new();
    f.write(
        "mod.py",
        concat!(
            "def kept(n):\n",
            "    return n + 1\n",
            "\n",
            "def gone():\n",
            "    return 0\n",
            "\n",
            "def stable(x):\n",
            "    return x\n",
        ),
    );
    f.commit("v1");
    f.write(
        "mod.py",
        concat!(
            "def kept(n):\n",
            "    return n * 100\n",
            "\n",
            "def stable(x):\n",
            "    return x\n",
            "\n",
            "def fresh():\n",
            "    return 42\n",
        ),
    );
    let r = f.trace(&["read", "mod.py", "--at", "HEAD", "--diff", "--json"]);
    r.ok();
    let v = r.view();
    let d = &v["files"]["mod.py"]["symbol_diff"];
    assert!(
        !d.is_null(),
        "symbol_diff missing on a supported file:\n{v}"
    );

    let names = |arr: &serde_json::Value| -> Vec<String> {
        let mut out: Vec<String> = arr
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["name"].as_str().unwrap().to_string())
            .collect();
        out.sort();
        out
    };
    assert_eq!(names(&d["added"]), vec!["fresh"], "added set wrong: {d}");
    assert_eq!(names(&d["removed"]), vec!["gone"], "removed set wrong: {d}");
    assert_eq!(
        names(&d["changed"]),
        vec!["kept"],
        "changed set must be exactly the body-mutated symbol: {d}"
    );
    // `stable` is byte-identical: it must appear in none of the three sets.
    for set in ["added", "removed", "changed"] {
        assert!(
            !names(&d[set]).contains(&"stable".to_string()),
            "byte-identical symbol leaked into {set}: {d}"
        );
    }
    let added = d["added"][0].clone();
    assert_eq!(added["kind"], "function", "added kind wrong: {added}");
    let changed = d["changed"][0].clone();
    assert_eq!(changed["name"], "kept");
    assert_eq!(changed["kind"], "function");
    // `kept` is the first symbol in both the committed file and the
    // worktree (`def kept(n):` is line 1 in each), so the changed entry's
    // ref/worktree lines are an exact, hand-determinable 1/1 — not a
    // lower bound.
    assert_eq!(
        changed["ref_line"].as_i64().unwrap(),
        1,
        "kept is the first def in the committed file (L1): {changed}"
    );
    assert_eq!(
        changed["worktree_line"].as_i64().unwrap(),
        1,
        "kept is still the first def in the worktree (L1): {changed}"
    );
    // The added/removed entries also carry exact lines on this fixture:
    // worktree `fresh` is the 3rd block (L7), committed `gone` was the
    // 2nd block (L4).
    assert_eq!(added["name"], "fresh");
    assert_eq!(
        added["line"].as_i64().unwrap(),
        7,
        "fresh is at worktree L7: {added}"
    );
    let removed = d["removed"][0].clone();
    assert_eq!(removed["name"], "gone");
    assert_eq!(removed["kind"], "function");
    assert_eq!(
        removed["line"].as_i64().unwrap(),
        4,
        "gone was at committed L4: {removed}"
    );
}

#[test]
fn read_between_anchors_returns_exact_section_and_resolved_lines() {
    // Anchors are matched as regexes against whole lines. The section runs
    // from the first start-anchor match through (and including) the first
    // end-anchor match after it. Resolved lines are 1-indexed and inclusive.
    let f = Fixture::new();
    f.write(
        "block.py",
        concat!(
            "import os\n",      // L1
            "# region start\n", // L2  <- start anchor
            "def inside():\n",  // L3
            "    return os\n",  // L4
            "# region end\n",   // L5  <- end anchor
            "TRAILING = 1\n",   // L6
        ),
    );
    f.commit("anchored");
    let r = f.trace(&[
        "read",
        "block.py",
        "--between",
        "region start",
        "region end",
        "--json",
    ]);
    r.ok();
    let v = r.view();
    assert_eq!(v["between"][0], "region start");
    assert_eq!(v["between"][1], "region end");
    assert_eq!(
        v["results"][0]["between_resolved_lines"][0], 2,
        "section must start at the start-anchor line: {v}"
    );
    assert_eq!(
        v["results"][0]["between_resolved_lines"][1], 5,
        "section must end at the end-anchor line (inclusive): {v}"
    );
    let content = v["results"][0]["content"].as_str().unwrap();
    assert_eq!(
        content,
        concat!(
            "L2: # region start\n",
            "L3: def inside():\n",
            "L4:     return os\n",
            "L5: # region end\n",
        ),
        "anchor section content wrong (must exclude L1 and L6):\n{content:?}"
    );
}

#[test]
fn info_file_json_reports_complexity_and_graph_fields() {
    let f = standard_repo();
    let r = f.trace(&["info", "src/app.py", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(v["file"].as_str().unwrap().ends_with("src/app.py"), true);
    // standard_repo()'s main(x): base 1 + if(1) + for(1) + if(1) = 4,
    // exactly one function. Hand-verifiable McCabe — no lower bound. The
    // facts sit under the repo-relative path, as on every other command.
    assert_eq!(v["functions"].as_i64().unwrap(), 1);
    let facts = &v["files"]["src/app.py"];
    assert_eq!(facts["cyclomatic_complexity"], 4, "main(): if + for + if over base 1 = 4: {facts}");
    assert_eq!(facts["max_function_complexity"], 4, "{facts}");
    assert_eq!(facts["functions"], 1, "{facts}");
    assert_eq!(facts["imports"], 1, "{facts}");
    let main_fn = v["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["name"].as_str() == Some("main"))
        .expect("main present");
    assert_eq!(main_fn["cyclomatic_complexity"].as_i64().unwrap(), 4);
    // repo_context over the fixed 7-entry fixture tree is deterministic.
    assert_eq!(
        v["repo"],
        serde_json::json!({"total_files": 7, "median_file_ccn": 0, "complexity_p95": 1}),
        "info file repo context must be exact for the known fixture: {}",
        v["repo"]
    );
}

#[test]
fn info_directory_aggregates_files() {
    let f = standard_repo();
    let r = f.trace(&["info", "src", "--json"]);
    r.ok();
    let v = r.view();
    assert!(v["directory"].as_str().unwrap().ends_with("src"));
    // src holds exactly app.py, util.py, front.tsx, consts.ts. The row count
    // reads from `counts`, because `context` owns the `files` name in the
    // flattened view.
    let counts = &r.json()["counts"];
    assert_eq!(
        counts["files"].as_i64().unwrap(),
        4,
        "src has app.py, util.py, front.tsx, consts.ts: {}",
        counts["files"]
    );
    // Exact aggregate: app.py 4 + util.py 2 + front.tsx 2 + consts.ts 0.
    assert_eq!(
        counts["cyclomatic_complexity"], 8,
        "src aggregate CCN (4+2+2+0): {counts}"
    );
    assert_eq!(counts["lines"], 21, "src aggregate lines (12+1+4+4): {counts}");
    // Each row also carries `abs_path` — the fixture's temp directory path,
    // which varies per run — and the file's git facts; the rest of every row
    // is fully determinable. Pin the exact deterministic projection.
    let files_proj: Vec<serde_json::Value> = v["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            assert_eq!(e["git"]["commits"], 1, "every row keeps its git facts: {e}");
            serde_json::json!({
                "file": e["file"],
                "lines": e["lines"],
                "cyclomatic_complexity": e["cyclomatic_complexity"],
                "functions": e["functions"],
                "complexity_rank": e["complexity_rank"],
            })
        })
        .collect();
    assert_eq!(
        serde_json::Value::Array(files_proj),
        serde_json::json!([
            {"file": "app.py",    "lines": 12, "cyclomatic_complexity": 4, "functions": 1, "complexity_rank": "low"},
            {"file": "consts.ts", "lines": 1, "cyclomatic_complexity": 0, "functions": 0, "complexity_rank": "low"},
            {"file": "front.tsx", "lines": 4, "cyclomatic_complexity": 2, "functions": 1, "complexity_rank": "low"},
            {"file": "util.py",   "lines": 4, "cyclomatic_complexity": 2, "functions": 1, "complexity_rank": "low"}
        ]),
        "info directory rows (minus the per-run abs_path) must be exact: {}",
        v["results"]
    );
    // exempt-(a): every entry's abs_path is the fixture temp dir, distinct
    // per run; the tightest stable invariant is that it ends with the
    // base-relative file path.
    for e in v["results"].as_array().unwrap() {
        let abs = e["abs_path"].as_str().unwrap();
        let file = e["file"].as_str().unwrap();
        assert!(
            abs.ends_with(&format!("src/{file}")),
            "abs_path must end with src/{file}: {abs}"
        );
    }
    assert_eq!(
        v["repo"],
        serde_json::json!({"total_files": 7, "median_file_ccn": 0, "complexity_p95": 1}),
        "info directory repo context must be exact: {}",
        v["repo"]
    );
    // The absolute temp path goes through macOS's `/var` symlink; the same
    // four files must still be found.
    let absolute = f.trace(&["info", f.path("src").as_str(), "--json"]);
    absolute.ok();
    assert_eq!(absolute.json()["counts"]["files"], 4, "{}", absolute.stdout);
}

#[test]
fn info_brief_truncates_function_table() {
    let f = Fixture::new();
    let mut src = String::new();
    for i in 0..8 {
        src.push_str(&format!(
            "def fn{i}(a, b):\n    if a and b:\n        return a\n    return b\n\n"
        ));
    }
    f.write("many.py", &src);
    f.commit("many fns");
    let brief = f.trace(&["info", "many.py", "--brief"]);
    brief.ok();
    // many.py has exactly 8 identical 1-`if` functions. `--brief` shows
    // the top 3 and reports the remaining 5 — both header and footer are
    // exact, hand-determinable strings, not an either/or.
    assert!(
        brief
            .stdout
            .contains("Functions (top 3 by complexity of 8):"),
        "--brief header must name the top-3-of-8 truncation exactly:\n{}",
        brief.stdout
    );
    assert!(
        brief.stdout.contains("… 5 more (run without --brief to see all)"),
        "--brief footer must report exactly the 5 hidden functions:\n{}",
        brief.stdout
    );
}

#[test]
fn structure_lists_imports_and_symbols_json() {
    let f = standard_repo();
    let r = f.trace(&["structure", "src/app.py", "--json"]);
    r.ok();
    let v = r.view();
    // app.py's imports are exactly `import os` (L1) and
    // `from src.util import helper` (L2) — a full (module, symbol, line)
    // tuple set, in source order, nothing else.
    let imports: Vec<(String, Option<String>, i64)> = v["imports"]
        .as_array()
        .expect("imports must be an array")
        .iter()
        .map(|i| {
            (
                i["module"].as_str().unwrap().to_string(),
                i["symbol"].as_str().map(|s| s.to_string()),
                i["line"].as_i64().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        imports,
        vec![
            ("os".to_string(), None, 1),
            ("src.util".to_string(), Some("helper".to_string()), 2),
        ],
        "app.py import set must be exactly os + src.util.helper: {:?}",
        v["imports"]
    );
    // The only module-level symbol is main() at L5.
    let mut exports: Vec<(String, String, i64)> = v["exports"]
        .as_array()
        .expect("exports must be an array")
        .iter()
        .map(|e| {
            (
                e["name"].as_str().unwrap().to_string(),
                e["kind"].as_str().unwrap().to_string(),
                e["line"].as_i64().unwrap(),
            )
        })
        .collect();
    exports.sort();
    assert_eq!(
        exports,
        vec![("main".to_string(), "function".to_string(), 5)],
        "app.py exports exactly main() at L5: {:?}",
        v["exports"]
    );
    assert_eq!(
        v["symbols"].as_i64().unwrap(),
        1,
        "app.py has one symbol: {}",
        v
    );
}

#[test]
fn structure_exports_are_the_exact_module_level_set() {
    // The tree-sitter export extractor reports module-level defs/classes
    // only — nested functions and a function-local name must not appear.
    // Exact name+kind+line tuples are asserted, so a wrong line or a
    // leaked nested symbol fails the test.
    let f = Fixture::new();
    f.write(
        "api.py",
        concat!(
            "import os\n",             // L1
            "\n",                      // L2
            "def public_fn(x):\n",     // L3
            "    def nested():\n",     // L4  (must NOT be exported)
            "        return 1\n",      // L5
            "    return nested()\n",   // L6
            "\n",                      // L7
            "class PublicClass:\n",    // L8
            "    def method(self):\n", // L9  (method, not a module export)
            "        return 2\n",      // L10
        ),
    );
    f.commit("api");
    let r = f.trace(&["structure", "api.py", "--json"]);
    r.ok();
    let v = r.view();
    let mut exports: Vec<(String, String, i64)> = v["exports"]
        .as_array()
        .expect("exports must be an array")
        .iter()
        .map(|e| {
            (
                e["name"].as_str().unwrap().to_string(),
                e["kind"].as_str().unwrap().to_string(),
                e["line"].as_i64().unwrap(),
            )
        })
        .collect();
    exports.sort();
    assert_eq!(
        exports,
        vec![
            ("PublicClass".to_string(), "class".to_string(), 8),
            ("public_fn".to_string(), "function".to_string(), 3),
        ],
        "export set must be exactly the two module-level symbols at their \
         real lines, with nested()/method() excluded: {:?}",
        v["exports"]
    );
}

#[test]
fn structure_falls_back_to_ctags_for_non_ast_language() {
    // Bash has a CCN walker but no tree-sitter import/export extractor (it is
    // not in `extraction::supported_extensions`), so `exports` is empty while
    // ctags still surfaces the symbols. This pins the fallback path:
    // structure stays useful for a CCN-covered language the architecture
    // extractor does not cover. (Go was the prior example here; it now has a
    // first-class extractor, so the no-extractor example moved to bash.)
    let f = Fixture::new();
    f.write(
        "util.sh",
        concat!(
            "#!/usr/bin/env bash\n",
            "\n",
            "alpha() {\n",
            "    return 1\n",
            "}\n",
            "\n",
            "beta() {\n",
            "    if [ \"$1\" -gt 0 ]; then\n",
            "        return \"$1\"\n",
            "    fi\n",
            "    return 0\n",
            "}\n",
        ),
    );
    f.commit("bash file");
    let r = f.trace(&["structure", "util.sh", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(
        v["exports"].as_array().unwrap().len(),
        0,
        "bash has no tree-sitter export extractor — exports must be empty: {}",
        v["exports"]
    );
    // ctags surfaces exactly two symbols for this bash file, each at its real
    // line: the functions alpha (L3) and beta (L7). Pinned as an exact
    // (name, kind, line) set.
    let mut symbols: Vec<(String, String, i64)> = v["symbols_by_kind"]
        .as_object()
        .unwrap()
        .values()
        .flat_map(|arr| arr.as_array().unwrap())
        .map(|s| {
            (
                s["name"].as_str().unwrap().to_string(),
                s["kind"].as_str().unwrap().to_string(),
                s["line"].as_i64().unwrap(),
            )
        })
        .collect();
    symbols.sort();
    assert_eq!(
        symbols,
        vec![
            ("alpha".to_string(), "function".to_string(), 3),
            ("beta".to_string(), "function".to_string(), 7),
        ],
        "ctags fallback symbol set must be exactly alpha/beta at their lines: {}",
        v
    );
    assert_eq!(
        v["symbols"].as_i64().unwrap(),
        2,
        "the symbol count must be exactly the two ctags symbols: {}",
        v
    );
}

#[test]
fn tree_json_carries_repo_context_and_ranks() {
    let f = standard_repo();
    let r = f.trace(&["tree", "src", "--json"]);
    r.ok();
    let v = r.view();
    // standard_repo()'s src/ holds exactly four files; every fact below is
    // hand-verifiable from the fixture source. Each row names its path and
    // the file's facts sit in context under that path.
    let files_norm: Vec<serde_json::Value> = v["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            let facts = &v["files"][e["path"].as_str().unwrap()];
            assert_eq!(facts["git"]["commits"], 1, "{facts}");
            serde_json::json!({
                "path": e["path"],
                "cyclomatic_complexity": facts["cyclomatic_complexity"],
                "max_function_complexity": facts["max_function_complexity"],
                "lines": facts["lines"],
                "complexity_rank": facts["complexity_rank"],
                "imported_by": facts["imported_by"],
                "imports": facts["imports"],
            })
        })
        .collect();
    assert_eq!(
        serde_json::Value::Array(files_norm),
        serde_json::json!([
            {"path": "app.py",    "cyclomatic_complexity": 4, "max_function_complexity": 4, "lines": 12, "complexity_rank": "low", "imported_by": 0, "imports": 1},
            {"path": "consts.ts", "cyclomatic_complexity": 0, "max_function_complexity": 0, "lines": 1, "complexity_rank": "low", "imported_by": 1, "imports": 0},
            {"path": "front.tsx", "cyclomatic_complexity": 2, "max_function_complexity": 2, "lines": 4, "complexity_rank": "low", "imported_by": 0, "imports": 1},
            {"path": "util.py",   "cyclomatic_complexity": 2, "max_function_complexity": 2, "lines": 4, "complexity_rank": "low", "imported_by": 1, "imports": 0}
        ]),
        "tree rows must be the exact four-file fixture set: {}",
        v["results"]
    );
    // The repo context over the whole 7-entry fixture tree: median CCN 0,
    // p95 1 — deterministic for this fixed source.
    assert_eq!(
        v["repo"],
        serde_json::json!({"total_files": 7, "median_file_ccn": 0, "complexity_p95": 1}),
        "tree repo context must be exact for the known fixture: {}",
        v["repo"]
    );
}

#[test]
fn tree_human_marks_root_and_entries() {
    let f = standard_repo();
    let r = f.trace(&["tree", "src"]);
    r.ok();
    assert!(r.stdout.contains("repo_context:"), "{}", r.stdout);
    assert!(r.stdout.contains("app.py"), "{}", r.stdout);
}

#[test]
fn tree_recurses_into_nested_directories() {
    // tree is recursive: a file two levels below the base must appear with
    // its full base-relative path. A non-recursive walk would miss `deep.py`
    // entirely — this fails if recursion regresses.
    let f = Fixture::new();
    f.write("proj/top.py", "def t():\n    return 1\n");
    f.write("proj/a/mid.py", "def m():\n    return 2\n");
    f.write("proj/a/b/deep.py", "def d():\n    return 3\n");
    f.commit("nested tree");
    let r = f.trace(&["tree", "proj", "--json"]);
    r.ok();
    let v = r.view();
    let mut paths: Vec<String> = v["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["path"].as_str().unwrap().to_string())
        .collect();
    paths.sort();
    assert_eq!(
        paths,
        vec![
            "a/b/deep.py".to_string(),
            "a/mid.py".to_string(),
            "top.py".to_string(),
        ],
        "tree must recurse through every nested directory: {:?}",
        v["results"]
    );
}

#[test]
fn tree_keeps_every_fact_beyond_one_resolve_chunk() {
    let f = Fixture::new();
    for index in 0..513 {
        f.write(
            &format!("src/file_{index:03}.py"),
            &format!("def file_{index:03}(value):\n    return value\n"),
        );
    }
    f.commit("large tree");

    let r = f.trace(&["tree", "src", "--depth", "1", "--json"]);
    r.ok();
    let v = r.view();
    let rows = v["results"]
        .as_array()
        .expect("tree results must be an array");
    let files = v["files"]
        .as_object()
        .expect("tree context must contain a files object");

    assert_eq!(
        rows.len(),
        513,
        "every selected file must remain in the tree"
    );
    assert_eq!(files.len(), 513, "every tree row must retain its facts");
    assert_eq!(v["repo"]["total_files"], 513);
    assert_eq!(v["repo"]["median_file_ccn"], 0);
    assert_eq!(v["repo"]["complexity_p95"], 0);

    for (index, row) in rows.iter().enumerate() {
        let path = format!("file_{index:03}.py");
        assert_eq!(row, &serde_json::json!({"path": path}), "row {index}");
        let facts = &files[&path];
        assert_eq!(
            serde_json::json!({
                "lines": facts["lines"],
                "cyclomatic_complexity": facts["cyclomatic_complexity"],
                "max_function_complexity": facts["max_function_complexity"],
                "complexity_rank": facts["complexity_rank"],
                "commits": facts["git"]["commits"],
            }),
            serde_json::json!({
                "lines": 2,
                "cyclomatic_complexity": 1,
                "max_function_complexity": 1,
                "complexity_rank": "low",
                "commits": 1,
            }),
            "{path} must retain its exact facts: {facts}"
        );
    }
}

#[test]
fn list_directory_json_separates_dirs_and_files() {
    let f = standard_repo();
    let r = f.trace(&["list", ".", "--json"]);
    r.ok();
    let v = r.view();
    // standard_repo()'s root has exactly three sub-directories (docs, lib,
    // src) and one top-level file (pyproject.toml). Every field is
    // hand-verifiable: child_count is the disk's direct-children number,
    // tracked_files the tracked subtree total, and the aggregate complexity
    // (docs 0, lib widget.php 3, src 4+2+2+0=8).
    let dirs = v["directories"].as_array().unwrap();
    let dir_proj: Vec<serde_json::Value> = dirs
        .iter()
        .map(|d| {
            serde_json::json!({
                "name": d["name"],
                "child_count": d["child_count"],
                "tracked_files": d["tracked_files"],
                "cyclomatic_complexity": d["cyclomatic_complexity"],
                "uncommitted": d["uncommitted"],
            })
        })
        .collect();
    assert_eq!(
        serde_json::Value::Array(dir_proj),
        serde_json::json!([
            {"name": "docs", "child_count": 1, "tracked_files": 1, "cyclomatic_complexity": 0, "uncommitted": false},
            {"name": "lib",  "child_count": 1, "tracked_files": 1, "cyclomatic_complexity": 3, "uncommitted": false},
            {"name": "src",  "child_count": 4, "tracked_files": 4, "cyclomatic_complexity": 8, "uncommitted": false}
        ]),
        "list directories must be the exact three-dir fixture set: {}",
        v["directories"]
    );
    // The fixture commits at test run time, so the newest commit reads as
    // today (or yesterday across a UTC midnight) — the same age wording a
    // file's `last_commit` uses.
    for d in dirs {
        let last = d["last_commit"].as_str().unwrap();
        assert!(last == "today" || last == "yesterday", "{d}");
    }
    // The one root file carries all three column groups: stat always, code
    // because scc indexes TOML (loc 3, ccn 0 low), git because the file is
    // committed. Sizes and mtimes are run-time values, so the stat group is
    // asserted by presence and positivity, the code and git groups exactly.
    let files = v["files"].as_array().unwrap();
    assert_eq!(
        files.len(),
        1,
        "exactly pyproject.toml at root: {}",
        v["files"]
    );
    let file = &files[0];
    assert_eq!(file["name"], "pyproject.toml");
    assert!(
        file["stat"]["size_bytes"].as_i64().unwrap() > 0
            && file["stat"]["mtime_ns"].as_i64().unwrap() > 0
            && !file["stat"]["mtime"].as_str().unwrap().is_empty(),
        "stat group must always be present and populated: {file}"
    );
    assert_eq!(
        serde_json::json!({
            "lines": file["code"]["lines"],
            "cyclomatic_complexity": file["code"]["cyclomatic_complexity"],
            "complexity_rank": file["code"]["complexity_rank"],
        }),
        serde_json::json!({"lines": 3, "cyclomatic_complexity": 0, "complexity_rank": "low"}),
        "code group must carry scc's TOML metrics: {file}"
    );
    assert_eq!(
        file["git"]["commits"], 1,
        "git group must carry the one hermetic commit: {file}"
    );
    assert_eq!(v["entries"].as_i64().unwrap(), 4, "{v}");
    assert_eq!(v["limited"], false, "{v}");
}

#[test]
fn list_keeps_rows_aggregates_and_git_facts_beyond_one_resolve_chunk() {
    let f = Fixture::new();
    for index in 0..513 {
        let directory = if index < 257 { "alpha" } else { "beta" };
        f.write(
            &format!("{directory}/module_{index:03}.py"),
            &format!("def module_{index:03}(value):\n    if value:\n        return value\n    return 0\n"),
        );
    }
    f.write("root.py", "def root():\n    return 1\n");
    f.write("settings.toml", "enabled = true\n");
    f.commit("broad list");

    let r = f.trace(&["list", ".", "--json"]);
    r.ok();
    let v = r.view();

    let directories: Vec<serde_json::Value> = v["directories"]
        .as_array()
        .unwrap()
        .iter()
        .map(|directory| {
            serde_json::json!({
                "name": directory["name"],
                "child_count": directory["child_count"],
                "tracked_files": directory["tracked_files"],
                "cyclomatic_complexity": directory["cyclomatic_complexity"],
                "uncommitted": directory["uncommitted"],
            })
        })
        .collect();
    assert_eq!(
        serde_json::Value::Array(directories),
        serde_json::json!([
            {"name": "alpha", "child_count": 257, "tracked_files": 257, "cyclomatic_complexity": 514, "uncommitted": false},
            {"name": "beta", "child_count": 256, "tracked_files": 256, "cyclomatic_complexity": 512, "uncommitted": false},
        ]),
        "directory aggregates must survive a facts set larger than one chunk: {}",
        v["directories"]
    );

    let files = v["files"].as_array().unwrap();
    assert_eq!(
        files
            .iter()
            .map(|file| file["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["root.py", "settings.toml"],
        "direct files must keep their sorted rows: {}",
        v["files"]
    );
    assert_eq!(
        files[0]["code"]["cyclomatic_complexity"], 1,
        "source facts must survive: {}",
        files[0]
    );
    assert_eq!(
        files[1]["code"]["lines"], 1,
        "SCC-only facts must survive: {}",
        files[1]
    );
    for file in files {
        assert_eq!(
            file["git"]["commits"], 1,
            "passive git context must survive: {file}"
        );
    }
    assert_eq!(
        v["entries"], 4,
        "entry count must remain unchanged: {v}"
    );
    assert_eq!(
        v["limited"], false,
        "unbounded listing must remain unbounded: {v}"
    );
}

/// The row source is the disk, not the git universe: a gitignored artifact
/// directory lists its files with the stat group, code and git groups absent
/// (no language resolves, git holds nothing). This is the `tests/.runs`
/// case that motivated the rebuild.
#[test]
fn list_shows_gitignored_artifact_files_stat_only() {
    let f = standard_repo();
    f.write(".gitignore", "runs/\n");
    f.commit("ignore runs");
    f.write("runs/test-001.txt", "run output one\n");
    f.write("runs/test-002.txt", "run output two\n");

    let r = f.trace(&["list", "runs", "--json"]);
    r.ok();
    let v = r.view();
    let names: Vec<&str> = v["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        vec!["test-001.txt", "test-002.txt"],
        "gitignored artifacts must list from disk: {}",
        v["files"]
    );
    for file in v["files"].as_array().unwrap() {
        assert!(
            file["stat"]["size_bytes"].as_i64().unwrap() > 0,
            "stat group always present: {file}"
        );
        assert!(
            file["code"].is_null(),
            "no language resolves for .txt: {file}"
        );
        assert!(
            file["git"].is_null(),
            "git holds nothing for an ignored file: {file}"
        );
    }
    assert_eq!(v["entries"].as_i64().unwrap(), 2, "{v}");
}

/// `--recent` orders by filesystem mtime newest-first; `--limit` caps the
/// entries after ordering, directories and files alike, the way
/// `ls -t | head` does, while `entries` keeps the pre-cap total.
#[test]
fn list_recent_orders_by_mtime_and_limit_keeps_total() {
    let f = standard_repo();
    f.write(".gitignore", "runs/\n");
    f.commit("ignore runs");
    f.write("runs/older.txt", "old\n");
    let older = f.path("runs/older.txt");
    let past = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
    let file = std::fs::File::options().append(true).open(&older).unwrap();
    file.set_modified(past).unwrap();
    f.write("runs/newer.txt", "new\n");

    let r = f.trace(&["list", "runs", "--recent", "--limit", "1", "--json"]);
    r.ok();
    let v = r.view();
    let names: Vec<&str> = v["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        vec!["newer.txt"],
        "--recent --limit 1 must keep only the newest file: {}",
        v["files"]
    );
    assert_eq!(v["entries"].as_i64().unwrap(), 2, "pre-cap total: {v}");
    assert_eq!(v["limited"], true, "{v}");

    let h = f.trace(&["list", "runs", "--recent", "--limit", "1"]);
    h.ok();
    assert!(
        h.stdout.contains("entries=2 (showing 1)"),
        "human form must carry the pre-cap total: {}",
        h.stdout
    );

    std::fs::create_dir(f.path("runs/newest")).unwrap();
    let v = f.trace(&["list", "runs", "--recent", "--limit", "1", "--json"]).view();
    assert_eq!(v["directories"][0]["name"], "newest", "{v}");
    assert_eq!(v["files"].as_array().unwrap().len(), 0, "{v}");
    assert_eq!(v["entries"], 3, "{v}");
}

#[test]
fn list_fits_the_budget() {
    let f = standard_repo();
    f.write(".gitignore", "runs/\n");
    f.commit("ignore runs");
    for index in 0..400 {
        std::fs::create_dir_all(f.path(&format!("runs/session-{index:04}"))).unwrap();
    }
    let r = f.trace(&["list", "runs", "--budget", "2000"]);
    r.ok();
    assert!(r.stdout.chars().count() <= 2000, "{} characters:\n{}", r.stdout.chars().count(), r.stdout);
    assert!(r.stdout.contains("./: 400 directories"), "{}", r.stdout);
    assert!(r.stdout.contains("--budget 0]"), "{}", r.stdout);
}

#[test]
fn status_and_stats_fit_the_budget() {
    let f = standard_repo();
    for index in 0..150 {
        f.write(&format!("area_{}/module_{index:03}.py", index % 5), "value = 1\n");
    }
    for index in 0..120 {
        f.write(&format!("package_{index:03}/module.py"), "value = 1\n");
    }
    f.commit("many files");
    for index in 0..150 {
        f.write(&format!("area_{}/module_{index:03}.py", index % 5), "value = 2\n");
    }
    for (command, budget) in [(vec!["status"], 3000), (vec!["stats", "."], 3000)] {
        let mut args = command.clone();
        let budget_arg = budget.to_string();
        args.extend(["--budget", budget_arg.as_str()]);
        let r = f.trace(&args);
        r.ok();
        assert!(r.stdout.chars().count() <= budget, "{command:?}: {} characters:\n{}", r.stdout.chars().count(), r.stdout);
        assert!(r.stdout.contains("--budget 0]"), "{command:?}:\n{}", r.stdout);
    }
}

#[test]
fn tree_names_every_directory() {
    let f = Fixture::new();
    f.write("skills/plan/SKILL.md", "# plan\n");
    f.write("skills/trace/SKILL.md", "# trace\n");
    f.write("skills/trace/references/commands.md", "# commands\n");
    f.commit("skills");
    let r = f.trace(&["tree", "skills"]);
    r.ok();
    let body: Vec<&str> = r.stdout.lines().skip(1).take_while(|line| !line.is_empty()).collect();
    assert_eq!(
        body.iter().map(|line| line.split("  {").next().unwrap()).collect::<Vec<_>>(),
        [
            "  plan/",
            "    · SKILL.md",
            "  trace/",
            "    · SKILL.md",
            "    references/",
            "      · commands.md",
        ],
        "{}",
        r.stdout
    );
}

/// A nested checkout under the base is its own scope: never a row set,
/// always named.
#[test]
fn list_names_nested_checkout_as_scope() {
    let f = standard_repo();
    f.git(&["init", "--quiet", "vendorland"]);
    f.write("vendorland/lib.js", "vendored()\n");

    let r = f.trace(&["list", ".", "--json"]);
    r.ok();
    let v = r.view();
    let dir_names: Vec<&str> = v["directories"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["name"].as_str().unwrap())
        .collect();
    assert!(
        !dir_names.contains(&"vendorland"),
        "a nested checkout is not a directory row: {dir_names:?}"
    );
    let nested: Vec<&str> = v["nested_repos"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap())
        .collect();
    assert_eq!(nested, vec!["vendorland"], "{v}");

    let h = f.trace(&["list", "."]);
    h.ok();
    assert!(
        h.stdout
            .contains("nested repository (its own search scope): vendorland"),
        "{}",
        h.stdout
    );
}

#[test]
fn list_is_strictly_one_level_deep() {
    // list collapses each sub-directory to a single entry and never
    // recurses: a nested file must appear nowhere in the file list, and a
    // second-level directory must not surface as a top-level directory.
    let f = Fixture::new();
    f.write("root_file.py", "ROOT = 1\n");
    f.write("pkg/inner_file.py", "INNER = 2\n");
    f.write("pkg/sub/deep_file.py", "DEEP = 3\n");
    f.commit("nested for list");
    let r = f.trace(&["list", ".", "--json"]);
    r.ok();
    let v = r.view();

    let file_names: Vec<&str> = v["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        file_names,
        vec!["root_file.py"],
        "list must show only the direct file at this level, not nested ones: {:?}",
        v["files"]
    );

    let dir_names: Vec<&str> = v["directories"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        dir_names,
        vec!["pkg"],
        "list must show only the first-level directory, never `sub`: {:?}",
        v["directories"]
    );

    // The single `pkg` entry aggregates BOTH nested files (one level down
    // and two levels down) — proof it summarized the subtree without
    // listing it.
    let pkg = &v["directories"][0];
    assert_eq!(pkg["name"], "pkg");
    assert_eq!(
        pkg["tracked_files"].as_i64().unwrap(),
        2,
        "pkg must aggregate inner_file.py + sub/deep_file.py: {pkg}"
    );
}

#[test]
fn stats_json_has_distribution_and_languages() {
    let f = standard_repo();
    let r = f.trace(&["stats", ".", "--json"]);
    r.ok();
    let v = r.view();
    // standard_repo() commits exactly 7 files; scc's per-language counts,
    // loc and complexity over that fixed tree are deterministic.
    assert_eq!(
        v["files"].as_i64().unwrap(),
        7,
        "standard_repo has 7 files: {}",
        v
    );
    let lang = |name: &str, files: i64, loc: i64, cx: i64, v: &serde_json::Value| {
        let l = &v["languages"][name];
        assert_eq!(l["files"].as_i64().unwrap(), files, "{name} files: {}", v);
        assert_eq!(l["lines_of_code"].as_i64().unwrap(), loc, "{name} lines_of_code: {}", v);
        assert_eq!(
            l["cyclomatic_complexity"].as_i64().unwrap(),
            cx,
            "{name} complexity: {}",
            v
        );
    };
    lang("Python", 2, 13, 4, &v); // app.py + util.py; CCN 4+0(file-level) via scc
    lang("TypeScript", 2, 5, 0, &v); // front.tsx + consts.ts
    lang("Markdown", 1, 2, 0, &v); // docs/readme.md
    lang("PHP", 1, 8, 1, &v); // lib/widget.php
    lang("TOML", 1, 3, 0, &v); // pyproject.toml
    assert_eq!(
        v["languages"].as_object().unwrap().len(),
        5,
        "exactly five languages in standard_repo: {}",
        v
    );
    let d = &v["distribution"];
    assert_eq!(
        d["median"].as_i64().unwrap(),
        0,
        "distribution.median: {}",
        v
    );
    assert_eq!(d["p75"].as_i64().unwrap(), 1, "distribution.p75: {}", v);
    assert_eq!(d["p90"].as_i64().unwrap(), 1, "distribution.p90: {}", v);
    assert_eq!(d["p95"].as_i64().unwrap(), 1, "distribution.p95: {}", v);
    assert_eq!(d["max"].as_i64().unwrap(), 3, "distribution.max: {}", v);
    // top_complex carries every file with its real (basename, language,
    // loc, complexity). Those per-file values are exact and hand-verified
    // for standard_repo, so the *content* of the list is pinned as
    // an exact multiset. The ordering contract is "complexity descending"
    // (a stable sort): the unique max (app.py, 3) is always first, and
    // complexity is monotonically non-increasing down the list. The
    // relative order *within* an equal-complexity run is NOT pinned: it
    // mirrors scc's per-language file emission order, which scc does not
    // guarantee stable run-to-run (observed flipping front.tsx/consts.ts
    // between runs) — that is the one genuinely non-deterministic axis.
    let rows: Vec<(&str, &str, i64, i64)> = v["top_complex"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            (
                e["path"].as_str().unwrap().rsplit('/').next().unwrap(),
                e["language"].as_str().unwrap(),
                e["lines"].as_i64().unwrap(),
                e["cyclomatic_complexity"].as_i64().unwrap(),
            )
        })
        .collect();
    let mut got = rows.clone();
    got.sort();
    let mut want = vec![
        ("app.py", "Python", 12, 3),
        ("util.py", "Python", 4, 1),
        ("widget.php", "PHP", 8, 1),
        ("front.tsx", "TypeScript", 4, 0),
        ("consts.ts", "TypeScript", 1, 0),
        ("readme.md", "Markdown", 3, 0),
        ("pyproject.toml", "TOML", 3, 0),
    ];
    want.sort();
    assert_eq!(
        got, want,
        "stats top_complex must list exactly these files with exact lines and complexity: {}",
        v
    );
    // app.py is the unique most-complex file → always first.
    assert_eq!(
        (rows[0].0, rows[0].3),
        ("app.py", 3),
        "the unique max-complexity file must sort first: {}",
        v
    );
    // The list is ordered by complexity descending (the documented
    // contract) — any inversion fails here regardless of the tie axis.
    for w in rows.windows(2) {
        assert!(
            w[0].3 >= w[1].3,
            "top_complex must be complexity-descending, got {:?} then {:?}: {}",
            w[0],
            w[1],
            v
        );
    }
}

/// A fixture whose tree carries two ancestor Claude.md docs, used by the
/// `docs` command + `read` docs-toggle tests.
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

/// Unique per-test session id so the cross-invocation session-dedupe state
/// (under `<repo>/.tracer-cache/sessions/<id>`, where `<repo>` is each
/// test's hermetic fixture root) never collides across the parallel suite.
/// Per-test fixtures are themselves throwaway tempdirs deleted on drop, so
/// no `$HOME`-scoped wipe is needed.
fn fresh_session_id(tag: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("trace-test-{tag}-{nanos}")
}

/// Docs arrive as Markdown, one `## <path>` section each, nearest first.
#[test]
fn docs_command_sends_markdown_nearest_first() {
    let f = docs_repo();
    let r = f.trace(&["docs", "sub/util.py"]);
    r.ok();
    let sub = r.stdout.find("## sub/Claude.md\n\n# Sub rules");
    let root = r.stdout.find("## Claude.md\n\n# Root rules");
    assert!(
        sub.is_some() && root.is_some() && sub < root,
        "the nearest doc comes first, each under its own heading:\n{}",
        r.stdout
    );
}

/// A doc that does not fit the budget is named, not sent, and not recorded,
/// so the next call with room sends it whole.
#[test]
fn docs_that_do_not_fit_are_named_and_offered_again() {
    let f = docs_repo();
    f.write("sub/Claude.md", &format!("# Sub rules\n{}\n", "Long rule text. ".repeat(400)));
    f.commit("long sub rules");
    let sid = fresh_session_id("docs-budget");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    let tight = f.trace_env(&["docs", "sub/util.py", "--budget", "1000"], &env);
    tight.ok();
    assert!(tight.stdout.chars().count() <= 1000, "{}", tight.stdout);
    assert!(
        tight.stdout.contains("## Claude.md") && tight.stdout.contains("- sub/Claude.md ("),
        "the short root doc is sent whole and the long one named:\n{}",
        tight.stdout
    );
    assert!(!tight.stdout.contains("Long rule text"), "{}", tight.stdout);

    let roomy = f.trace_env(&["docs", "sub/util.py", "--budget", "0"], &env);
    roomy.ok();
    assert!(
        roomy.stdout.contains("## sub/Claude.md") && !roomy.stdout.contains("## Claude.md"),
        "the named doc is offered again and the sent one is not:\n{}",
        roomy.stdout
    );
}

/// `docs prime <file>` records exactly the files the harness loaded, so
/// tracer does not send them again.
#[test]
fn prime_records_the_files_the_harness_loaded() {
    let f = docs_repo();
    let sid = fresh_session_id("prime-files");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];
    f.trace_env(&["docs", "prime", &f.path("Claude.md")], &env).ok();

    let r = f.trace_env(&["docs", "sub/util.py"], &env);
    r.ok();
    assert!(
        r.stdout.contains("## sub/Claude.md") && !r.stdout.contains("## Claude.md"),
        "the primed root doc is not sent again:\n{}",
        r.stdout
    );
}

#[test]
fn docs_command_json_shape() {
    let f = docs_repo();
    let r = f.trace(&["docs", "sub/util.py", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(v["path"], "sub/util.py");
    assert_eq!(v["directory_scoped"], false);
    assert_eq!(v["docs"], 2);
    let docs = v["results"].as_array().unwrap();
    assert!(docs
        .iter()
        .any(|d| d["content"].as_str().unwrap().contains("Root rules")));
}

#[test]
fn docs_command_directory_scoped() {
    let f = docs_repo();
    let r = f.trace(&["docs", "sub", "--directory", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(v["directory_scoped"], true);
    // docs_repo() plants exactly two ancestor Claude.md docs (root + sub/);
    // directory-scoped `docs sub` resolves both, so the count is exactly 2.
    assert_eq!(
        v["docs"].as_i64().unwrap(),
        2,
        "directory-scoped docs over sub/ must resolve exactly Claude.md + sub/Claude.md: {}",
        v
    );
    let paths: Vec<&str> = v["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["path"].as_str().unwrap())
        .collect();
    assert_eq!(
        paths,
        vec!["sub/Claude.md", "Claude.md"],
        "directory-scoped docs paths must be the exact ancestor set, nearest first: {:?}",
        paths
    );
}

#[test]
fn read_default_suppresses_injection() {
    let f = docs_repo();
    let r = f.trace(&["read", "sub/util.py"]);
    r.ok();
    assert!(
        !r.stdout.contains("Root rules"),
        "default read must not inject project docs:\n{}",
        r.stdout
    );
    assert!(
        r.stdout.contains("def helper"),
        "code body missing:\n{}",
        r.stdout
    );
}

#[test]
fn read_docs_flag_forces_injection() {
    let f = docs_repo();
    let r = f.trace(&["read", "sub/util.py", "--docs"]);
    r.ok();
    assert!(
        r.stdout.contains("Root rules"),
        "--docs must inject project docs:\n{}",
        r.stdout
    );
}

#[test]
fn read_default_json_omits_nested_memories() {
    let f = docs_repo();
    let def = f.trace(&["read", "sub/util.py", "--json"]);
    def.ok();
    assert!(
        def.view()["files"]["sub/util.py"]["nested_memories"].is_null(),
        "default read --json must not surface nested memories:\n{}",
        def.stdout
    );
    let on = f.trace(&["read", "sub/util.py", "--docs", "--json"]);
    on.ok();
    // exempt-(b): this test's contract is the JSON envelope shape — that
    // `--docs` adds the `nested_memories` key and the default omits it
    // (the paired .is_none() assertion above). The exact resolved docs
    // content for docs_repo() is pinned by value in the dedicated
    // `docs_command_json_shape` test (doc_count == 2 plus the Root-rules
    // body), so pinning it again here would duplicate that contract; the
    // key-presence half is what this test uniquely covers.
    assert!(
        on.view()["files"]["sub/util.py"]["nested_memories"].is_array(),
        "--docs --json must include nested memories:\n{}",
        on.stdout
    );
}

#[test]
fn docs_then_read_share_session_dedupe() {
    let f = docs_repo();
    let sid = fresh_session_id("docs-read");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    let first = f.trace_env(&["docs", "sub/util.py"], &env);
    first.ok();
    assert!(first.stdout.contains("Root rules"), "{}", first.stdout);

    // Same session: a doc surfaced by `docs` must not be re-emitted by a
    // `read --docs` (forced injection) — session dedupe suppresses it.
    let second = f.trace_env(&["read", "sub/util.py", "--docs"], &env);
    second.ok();
    assert!(
        !second.stdout.contains("Root rules") && !second.stdout.contains("Sub rules"),
        "read re-emitted a doc already surfaced by docs in the same session:\n{}",
        second.stdout
    );
    assert!(second.stdout.contains("def helper"), "{}", second.stdout);
}

#[test]
fn read_then_docs_share_session_dedupe() {
    let f = docs_repo();
    let sid = fresh_session_id("read-docs");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    let first = f.trace_env(&["read", "sub/util.py", "--docs"], &env);
    first.ok();
    assert!(first.stdout.contains("Root rules"), "{}", first.stdout);

    // Same session: docs already surfaced by `read --docs` are not sent
    // again, and with nothing new there is nothing to print.
    let second = f.trace_env(&["docs", "sub/util.py"], &env);
    second.ok();
    assert!(
        second.stdout.is_empty(),
        "docs re-surfaced a doc already in the session manifest:\n{}",
        second.stdout
    );
}

#[test]
fn context_file_mode_surfaces_rows_on_every_touch() {
    // File mode's headline is the summary. On a file's FIRST
    // surfacing in a session it also emits the once-per-session methods +
    // directory-listing lines; on the SECOND surfacing those are deduped away
    // and the output collapses to exactly the summary. This pins both the
    // summary-as-headline contract and the per-session first-touch dedup.
    let f = standard_repo();
    // Warm the cache so graph counts are populated.
    f.trace(&["cache", "build", "."]).ok();
    let sid = fresh_session_id("ctx-file-dedup");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    let first = f.trace_env(&["context", "src/app.py"], &env);
    first.ok();
    assert!(
        first.stdout.contains("def main") && first.stdout.contains("\ndirectory:\n  path: src/\n"),
        "first surfacing must carry rows and the directory facts:\n{}",
        first.stdout
    );

    let second = f.trace_env(&["context", "src/app.py"], &env);
    second.ok();
    assert_eq!(first.stdout, second.stdout);
}

/// A repo with a file nested two directories deep alongside a sibling, used
/// to pin the first-touch methods line, the one-level directory listing, and
/// the non-recursive (immediate-parent-only) rule.
fn nested_repo() -> Fixture {
    let f = Fixture::new();
    f.write(
        "app/controllers/orders.py",
        "def create(x):\n    if x:\n        return 1\n    return 0\n\ndef cancel():\n    return 2\n",
    );
    f.write("app/controllers/users.py", "def show():\n    return 9\n");
    f.write("app/top.py", "x = 1\n");
    f.commit("init nested repo");
    f
}

#[test]
fn context_file_mode_renders_source_rows() {
    let f = nested_repo();
    let sid = fresh_session_id("first-symbols");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    let r = f.trace_env(&["context", "app/controllers/orders.py"], &env);
    r.ok();
    assert!(
        r.stdout.contains("def create(x): …") && r.stdout.contains("def cancel(): …"),
        "context must render the file's declarations in source order:\n{}",
        r.stdout
    );
}

#[test]
fn first_touch_of_a_file_surfaces_its_immediate_directory_listing() {
    // The first time a file is surfaced, its passive context includes the
    // one-level listing of the file's immediate parent directory — the file's
    // neighbours — and only that one directory.
    let f = nested_repo();
    let sid = fresh_session_id("first-dir");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    let r = f.trace_env(&["context", "app/controllers/orders.py"], &env);
    r.ok();
    assert!(
        r.stdout.contains(
            "\ndirectory:\n  path: app/controllers/\n  imported_by: 0\n  imports: 0\n  entries: [orders.py, users.py]\n---\n"
        ),
        "first surfacing must list the immediate parent directory's files:\n{}",
        r.stdout
    );
}

#[test]
fn directory_listing_is_immediate_parent_only_never_ancestors() {
    // A nested file surfaces ONLY its immediate parent directory, never the
    // ancestor chain. Touching app/controllers/orders.py lists controllers/
    // and must NOT list app/ or the repo root.
    let f = nested_repo();
    let sid = fresh_session_id("non-recursive");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    let r = f.trace_env(&["context", "app/controllers/orders.py"], &env);
    r.ok();
    assert!(
        r.stdout.contains("\n  path: app/controllers/\n"),
        "the immediate parent directory must be listed:\n{}",
        r.stdout
    );
    // Exactly one directory block — never the ancestor chain.
    let directory_blocks = r.stdout.lines().filter(|l| *l == "directory:").count();
    assert_eq!(
        directory_blocks, 1,
        "exactly one directory block (the immediate parent), never the chain:\n{}",
        r.stdout
    );
    assert!(
        !r.stdout.contains("path: app/\n"),
        "ancestor directory app/ must NOT be listed (non-recursive rule):\n{}",
        r.stdout
    );
}

#[test]
fn directory_listing_surfaces_on_every_touch() {
    let f = nested_repo();
    let sid = fresh_session_id("dir-dedup");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    let first = f.trace_env(&["context", "app/controllers/orders.py"], &env);
    first.ok();
    assert!(
        first.stdout.contains("\n  path: app/controllers/\n"),
        "first file in the directory must surface the listing:\n{}",
        first.stdout
    );

    let second = f.trace_env(&["context", "app/controllers/users.py"], &env);
    second.ok();
    assert!(
        second.stdout.contains("\n  path: app/controllers/\n"),
        "a neighbour in the directory must repeat its listing:\n{}",
        second.stdout
    );
    assert!(
        second.stdout.contains("def show(): …"),
        "the neighbour's source row must surface:\n{}",
        second.stdout
    );
}

#[test]
fn context_directory_argument_surfaces_its_one_level_listing() {
    let f = nested_repo();
    let sid = fresh_session_id("dir-arg");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    let first = f.trace_env(&["context", "app/controllers"], &env);
    first.ok();
    assert_eq!(
        first.stdout.trim(),
        "---\ndirectory:\n  path: app/controllers/\n  imported_by: 0\n  imports: 0\n  entries: [orders.py, users.py]\n---",
        "a directly-touched directory must emit exactly its one-level listing:\n{}",
        first.stdout
    );

    let second = f.trace_env(&["context", "app/controllers"], &env);
    second.ok();
    assert_eq!(second.stdout, first.stdout);
}

#[test]
fn directory_facts_carry_graph_counts_and_annotations() {
    let f = Fixture::new();
    f.write(
        "composer.json",
        r#"{"autoload":{"psr-4":{"App\\":"src/"}}}"#,
    );
    f.write(
        "src/pain/Entity.php",
        concat!(
            "<?php\n",
            "namespace App\\pain;\n",
            "#[Entity]\n",
            "final class Entity {\n",
            "    #[Field] public string $first = '';\n",
            "    #[Field] public string $second = '';\n",
            "    #[Internal] public function hidden(): void {}\n",
            "}\n",
        ),
    );
    f.write("src/pain/notes.md", "# Notes\n");
    f.write(
        "src/outside/Use.php",
        concat!(
            "<?php\n",
            "namespace App\\Outside;\n",
            "use App\\pain\\Entity;\n",
            "final class Use { public function make(): Entity { return new Entity(); } }\n",
        ),
    );
    f.write(
        "src/no-types/View.php",
        concat!(
            "<?php\n",
            "namespace App\\NoTypes;\n",
            "use App\\pain\\Entity;\n",
            "function view(): Entity { return new Entity(); }\n",
        ),
    );
    f.commit("directory summary fixture");

    // src/pain/ is imported by the two other directories' files, imports
    // nothing, and counts its PHP attributes. Every file command shows the
    // same directory facts, in the YAML front matter and under the same keys
    // in JSON.
    let expected = "directory:\n  path: src/pain/\n  imported_by: 2\n  imports: 0\n  annotations: {Field: 2, Entity: 1, Internal: 1}\n";
    let expected_json = serde_json::json!({
        "path": "src/pain/",
        "imported_by": 2,
        "imports": 0,
        "annotations": {"Field": 2, "Entity": 1, "Internal": 1},
    });
    let directory_of = |facts: &serde_json::Value| {
        let mut directory = facts["directory"].clone();
        for key in ["entries", "total_entries", "at_session_start"] {
            directory.as_object_mut().map(|o| o.remove(key));
        }
        directory
    };
    let read = f.trace(&["read", "src/pain/Entity.php"]);
    read.ok();
    assert!(read.stdout.contains(expected), "{}", read.stdout);

    let context = f.trace(&["context", "src/pain/Entity.php", "--no-record"]);
    context.ok();
    assert!(context.stdout.contains(expected), "{}", context.stdout);

    // A search lists many files, so each carries its own facts and no
    // directory block.
    let grep = f.trace(&["grep", "Entity", "src/pain", "--json"]);
    grep.ok();
    let grep_view = grep.view();
    let grep_facts = &grep_view["files"]["src/pain/Entity.php"];
    assert_eq!(grep_facts["imported_by"], 2, "{grep_view}");
    assert!(grep_facts["directory"].is_null(), "{grep_view}");

    f.write(
        "src/pain/Entity.php",
        concat!(
            "<?php\n",
            "namespace App\\pain;\n",
            "#[Entity]\n",
            "final class Entity {\n",
            "    #[Field] public string $first = '';\n",
            "    #[Field] public string $second = '';\n",
            "    #[Internal] public function hidden(): void {}\n",
            "}\n",
            "// changed\n",
        ),
    );
    let diff = f.trace(&["diff", "--json"]);
    diff.ok();
    // `diff` lists its changed files' directories once, beside the files.
    let diff_view = diff.view();
    assert_eq!(
        diff_view["directories"]["src/pain/"],
        serde_json::json!({"files": 2, "imported_by": 2, "imports": 0}),
        "{diff_view}"
    );

    let info = f.trace(&["info", "src/pain/Entity.php", "--json"]);
    info.ok();
    let info_view = info.view();
    assert_eq!(
        directory_of(&info_view["files"]["src/pain/Entity.php"]),
        expected_json,
        "{info_view}"
    );

    // A file with no import graph of its own still shows its directory's.
    let markdown = f.trace(&["read", "src/pain/notes.md", "--json"]);
    markdown.ok();
    let markdown_view = markdown.view();
    assert_eq!(
        directory_of(&markdown_view["files"]["src/pain/notes.md"]),
        expected_json,
        "{markdown_view}"
    );
    assert!(markdown_view["files"]["src/pain/notes.md"]["imported_by"].is_null());

    let no_types = f.trace(&["read", "src/no-types/View.php", "--json"]);
    no_types.ok();
    let no_types_view = no_types.view();
    assert_eq!(
        directory_of(&no_types_view["files"]["src/no-types/View.php"]),
        serde_json::json!({"path": "src/no-types/", "imported_by": 0, "imports": 1}),
        "{no_types_view}"
    );
    assert!(no_types_view["files"]["src/pain/Entity.php"]["annotations"].is_null());
}

// ---- signature fidelity (PHP 8 attributes / 8.4 hooks, TS modifiers, Python annotations) ----

/// Find the single symbol in `v["symbols_by_kind"]` whose `name` matches —
/// returns the per-symbol JSON object so the test can assert on its rich
/// signature fields (visibility, return_type, parameters, attributes, etc).
fn find_symbol<'a>(v: &'a serde_json::Value, name: &str) -> &'a serde_json::Value {
    let kinds = v["symbols_by_kind"].as_object().expect("symbols_by_kind");
    for arr in kinds.values() {
        for s in arr.as_array().expect("kind array") {
            if s["name"].as_str() == Some(name) {
                return s;
            }
        }
    }
    panic!(
        "no symbol named {name} in:\n{}",
        serde_json::to_string_pretty(&v["symbols_by_kind"]).unwrap()
    );
}

#[test]
fn structure_php_class_carries_attributes_extends_implements() {
    let f = Fixture::new();
    f.write(
        "src/Funnel.php",
        concat!(
            "<?php\n",
            "namespace App;\n",
            "#[Entity]\n",
            "class Funnel extends Model implements UrlRoutable {\n",
            "  public function show(): string { return ''; }\n",
            "}\n",
        ),
    );
    f.commit("php class");
    let r = f.trace(&["structure", "src/Funnel.php", "--json"]);
    r.ok();
    let v = r.view();
    let cls = find_symbol(&v, "Funnel");
    assert_eq!(cls["kind"].as_str().unwrap(), "class", "{}", cls);
    assert_eq!(
        cls["header"], "#[Entity]\nclass Funnel extends Model implements UrlRoutable { … }",
        "{}",
        cls
    );
}

#[test]
fn structure_php_method_carries_visibility_return_type_attributes_and_typed_params() {
    let f = Fixture::new();
    f.write(
        "src/M.php",
        concat!(
            "<?php\n",
            "class M {\n",
            "  #[Route('GET','/x')]\n",
            "  public static function validateSlug(string $slug, ?int $excludeId = null): ?string { return null; }\n",
            "}\n",
        ),
    );
    f.commit("php method");
    let r = f.trace(&["structure", "src/M.php", "--json"]);
    r.ok();
    let v = r.view();
    let m = find_symbol(&v, "validateSlug");
    assert_eq!(m["header"], "#[Route('GET','/x')]\npublic static function validateSlug(string $slug, ?int $excludeId = null): ?string { … }", "{}", m);
}

#[test]
fn structure_php_84_hooked_property_surfaces_with_attribute_and_accessors() {
    // PHP 8.4 property hooks: `public int $id { get => ...; set { ... } }`.
    // ctags does not emit these properties at all today, so the structure
    // command must backfill them from tree-sitter. The Schema attribute on
    // the property and both accessor hooks must round-trip.
    let f = Fixture::new();
    f.write(
        "src/H.php",
        concat!(
            "<?php\n",
            "class H {\n",
            "  #[Schema(type: 'string', label: 'Name')]\n",
            "  public string $name { get => 'x'; set { $this->v = $value; } }\n",
            "}\n",
        ),
    );
    f.commit("php hooks");
    let r = f.trace(&["structure", "src/H.php", "--json"]);
    r.ok();
    let v = r.view();
    let prop = find_symbol(&v, "$name");
    assert_eq!(prop["kind"].as_str().unwrap(), "property", "{}", prop);
    assert_eq!(
        prop["header"],
        "#[Schema(type: 'string', label: 'Name')]\npublic string $name { get => …; set { … } }",
        "{}",
        prop
    );
}

/// A PHP class whose members carry the access markers an Agent must not
/// miss: a docblock `@internal`, attributes on the class, a property and a
/// method (one spanning lines), and a plain public member beside them.
fn php_markers_repo() -> Fixture {
    let f = Fixture::new();
    f.write(
        "src/Account.php",
        concat!(
            "<?php\n",
            "namespace App;\n",
            "/**\n",
            " * An account.\n",
            " *\n",
            " * @internal only Billing constructs this\n",
            " */\n",
            "#[Internal(reason: 'billing only')]\n",
            "final class Account {\n",
            "  #[Encrypted]\n",
            "  public string $secret = '';\n",
            "  /** @deprecated use rename() */\n",
            "  public string $name = '';\n",
            "  #[Internal(\n",
            "    reason: 'admin console only',\n",
            "  )]\n",
            "  public function rename(string $name): void { $this->name = $name; }\n",
            "  public function exportSecret(): string { return $this->secret; }\n",
            "  public function __construct(public readonly bool $requires) {}\n",
            "}\n",
        ),
    );
    f.commit("php access markers");
    f
}

#[test]
fn structure_php_json_carries_doc_tags() {
    let f = php_markers_repo();
    let r = f.trace(&["structure", "src/Account.php", "--json"]);
    r.ok();
    let v = r.view();
    let cls = find_symbol(&v, "Account");
    assert_eq!(
        cls["annotations"],
        serde_json::json!(["@internal", "Internal"]),
        "{}",
        cls
    );
    let name = find_symbol(&v, "$name");
    assert_eq!(
        name["annotations"],
        serde_json::json!(["@deprecated"]),
        "{}",
        name
    );
    let plain = find_symbol(&v, "exportSecret");
    assert_eq!(plain["annotations"], serde_json::json!([]), "{}", plain);
}

#[test]
fn structure_json_carries_declaration_flags_and_annotations_at_the_row_top_level() {
    let f = php_markers_repo();
    let r = f.trace(&["structure", "src/Account.php", "--json"]);
    r.ok();
    let view = r.view();
    let class = find_symbol(&view, "Account");
    assert_eq!(
        class["annotations"],
        serde_json::json!(["@internal", "Internal"]),
        "{class}"
    );
    let requires = find_symbol(&view, "$requires");
    assert_eq!(requires["kind"], "property", "{requires}");
    assert_eq!(requires["name"], "$requires", "{requires}");
}

#[test]
fn structure_php_text_shows_markers_types_and_return_types() {
    // The text view an Agent reads must carry what the JSON carries: the
    // marker prefix, a property's type, a method's return type, and the
    // complexity of a method whose span starts at its attribute rather than
    // at the line ctags reports.
    let f = php_markers_repo();
    let r = f.trace(&["structure", "src/Account.php"]);
    r.ok();
    for expected in [
        "#[Internal(reason: 'billing only')]",
        "public string $secret = '';",
        "public string $name = '';",
        "public function rename(string $name): void { … }  // complexity 1",
        "public function exportSecret(): string { … }  // complexity 1",
    ] {
        assert!(
            r.stdout.contains(expected),
            "structure text must carry {expected:?}:\n{}",
            r.stdout
        );
    }
}

#[test]
fn first_touch_of_a_php_file_surfaces_its_access_markers() {
    // The briefing line is the surface most Agents act on. A member that
    // reads as plain `public` there, when the source marks it #[Internal]
    // or @deprecated, misleads every Agent that trusts the line.
    let f = php_markers_repo();
    let sid = fresh_session_id("php-markers");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];
    let r = f.trace_env(&["context", "src/Account.php"], &env);
    r.ok();
    assert!(
        r.stdout.contains("#[Internal(reason: 'billing only')]")
            && r.stdout.contains("public string $secret = '';")
            && r.stdout
                .contains("public function exportSecret(): string { … }  // complexity 1"),
        "the briefing line must carry every access marker in source order:\n{}",
        r.stdout
    );
}

#[test]
fn structure_ts_class_carries_decorators_generics_and_implements() {
    let f = Fixture::new();
    f.write(
        "src/svc.ts",
        concat!(
            "@Injectable()\n",
            "export class Svc<T extends Foo> implements Base, Other {\n",
            "  private readonly count: number = 0;\n",
            "  public async run(@Inject('X') id: number, name?: string): Promise<T> { return null!; }\n",
            "}\n",
        ),
    );
    f.commit("ts class");
    let r = f.trace(&["structure", "src/svc.ts", "--json"]);
    r.ok();
    let v = r.view();
    let cls = find_symbol(&v, "Svc");
    assert_eq!(
        cls["header"],
        "@Injectable()\nexport class Svc<T extends Foo> implements Base, Other { … }",
        "{}",
        cls
    );

    let field = find_symbol(&v, "count");
    assert_eq!(
        field["header"], "private readonly count: number = 0;",
        "{}",
        field
    );

    let m = find_symbol(&v, "run");
    assert_eq!(
        m["header"], "public async run(@Inject('X') id: number, name?: string): Promise<T> { … }",
        "{}",
        m
    );
}

#[test]
fn structure_ts_member_decorators_stay_on_their_own_member() {
    // A member's decorators belong to that member alone: an inline
    // `@Input() name` keeps its decorator, a decorator above `run` does not
    // leak onto the undecorated `make` below it.
    let f = Fixture::new();
    f.write(
        "src/widget.ts",
        concat!(
            "export class Widget {\n",
            "  @Input() name: string = '';\n",
            "  @HostListener('click')\n",
            "  public async run(a: string): Promise<void> {}\n",
            "  static make(): Widget { return new Widget(); }\n",
            "}\n",
        ),
    );
    f.commit("ts member decorators");
    let r = f.trace(&["structure", "src/widget.ts", "--json"]);
    r.ok();
    let v = r.view();
    let name = find_symbol(&v, "name");
    assert_eq!(
        name["annotations"],
        serde_json::json!(["Input"]),
        "{}",
        name
    );
    let run = find_symbol(&v, "run");
    assert_eq!(
        run["annotations"],
        serde_json::json!(["HostListener"]),
        "{}",
        run
    );
    let make = find_symbol(&v, "make");
    assert_eq!(make["annotations"], serde_json::json!([]), "{}", make);
}

#[test]
fn structure_ts_interface_carries_extends_generics_and_field_types() {
    let f = Fixture::new();
    f.write(
        "src/iface.ts",
        concat!(
            "export interface User<T> extends Base<T> {\n",
            "  id: number;\n",
            "  readonly name: string;\n",
            "}\n",
        ),
    );
    f.commit("ts iface");
    let r = f.trace(&["structure", "src/iface.ts", "--json"]);
    r.ok();
    let v = r.view();
    let iface = find_symbol(&v, "User");
    assert_eq!(iface["kind"].as_str().unwrap(), "interface", "{}", iface);
    assert_eq!(
        iface["header"], "export interface User<T> extends Base<T> { … }",
        "{}",
        iface
    );

    let id = find_symbol(&v, "id");
    assert_eq!(id["header"], "id: number;", "{}", id);
    let name = find_symbol(&v, "name");
    assert_eq!(name["header"], "readonly name: string;", "{}", name);
}

#[test]
fn structure_python_function_carries_decorators_annotations_and_defaults() {
    let f = Fixture::new();
    f.write(
        "api.py",
        concat!(
            "from dataclasses import dataclass\n",
            "\n",
            "@dataclass\n",
            "class User(Base):\n",
            "    id: int\n",
            "\n",
            "    @classmethod\n",
            "    async def create(cls, seed: int, count: int = 0) -> \"User\":\n",
            "        return cls()\n",
            "\n",
            "def free(x: int, y: str = \"z\", *args, **kwargs) -> bool:\n",
            "    return True\n",
        ),
    );
    f.commit("py module");
    let r = f.trace(&["structure", "api.py", "--json"]);
    r.ok();
    let v = r.view();

    let cls = find_symbol(&v, "User");
    assert_eq!(
        cls["header"], "@dataclass\nclass User(Base): …",
        "{}",
        cls
    );

    let create = find_symbol(&v, "create");
    assert_eq!(
        create["header"],
        "@classmethod\nasync def create(cls, seed: int, count: int = 0) -> \"User\": …",
        "{}",
        create
    );

    let free = find_symbol(&v, "free");
    assert_eq!(
        free["header"], "def free(x: int, y: str = \"z\", *args, **kwargs) -> bool: …",
        "{}",
        free
    );
}

#[test]
fn structure_existing_fields_remain_with_their_existing_shapes() {
    // Regression contract: every declaration record keeps the structure
    // command's shared header-row shape.
    let f = standard_repo();
    let r = f.trace(&["structure", "src/app.py", "--json"]);
    r.ok();
    let v = r.view();
    let main = find_symbol(&v, "main");
    assert!(main.get("name").is_some(), "name: {}", main);
    assert!(main.get("kind").is_some(), "kind: {}", main);
    assert!(main.get("line").is_some(), "line: {}", main);
    assert!(main.get("header_line").is_some(), "header_line: {}", main);
    assert!(main.get("end_line").is_some(), "end_line: {}", main);
    assert!(main.get("container").is_some(), "container: {}", main);
    assert!(main.get("parent").is_some(), "parent: {}", main);
    assert!(main.get("header").is_some(), "header: {}", main);
    assert!(main.get("annotations").is_some(), "annotations: {}", main);
    assert!(
        main["cyclomatic_complexity"].is_i64(),
        "cyclomatic_complexity must be an integer: {}",
        main
    );
}

/// A path committed to git but then deleted from the working tree (without
/// `git rm`) lingers in git's index. `git ls-files` still reports it, but it
/// is gone from disk. Every file-listing command must agree with the working
/// tree, never with the stale index: `list` and `tree` must not show the
/// deleted file or a directory whose only content was deleted, and `find`
/// must agree. This pins the single shared deletion policy across the
/// commands that route through the file enumerator.
#[test]
fn listing_commands_exclude_files_deleted_from_disk_but_kept_in_index() {
    let f = Fixture::new();
    f.write("hooks/kept.sh", "#!/bin/sh\necho kept\n");
    f.write("hooks/ghost.sh", "#!/bin/sh\necho ghost\n");
    f.write("hooks/gone-dir/orphan.sh", "#!/bin/sh\necho orphan\n");
    f.commit("commit hooks");

    // Delete from disk WITHOUT staging the deletion — the index keeps the
    // entries, mirroring an `rm`'d-but-never-`git rm`'d working tree.
    std::fs::remove_file(f.root.join("hooks/ghost.sh")).unwrap();
    std::fs::remove_dir_all(f.root.join("hooks/gone-dir")).unwrap();

    // git's index still carries all three — the condition under test.
    let r = f.trace(&["list", "hooks", "--json"]);
    r.ok();
    let v = r.view();

    let files: Vec<&str> = v["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        files,
        vec!["kept.sh"],
        "list must show only the on-disk file, never the deleted-in-index ghost: {}",
        v["files"]
    );
    let dirs: Vec<&str> = v["directories"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["name"].as_str().unwrap())
        .collect();
    assert!(
        dirs.is_empty(),
        "list must not show a directory whose only content was deleted: {}",
        v["directories"]
    );

    // tree routes through the same enumerator — same survivor set.
    let rt = f.trace(&["tree", "hooks", "--json"]);
    rt.ok();
    let vt = rt.view();
    let tree_files: Vec<&str> = vt["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["path"].as_str().unwrap())
        .collect();
    assert_eq!(
        tree_files,
        vec!["kept.sh"],
        "tree must agree with the working tree, not the stale index: {}",
        vt["results"]
    );

    // find was already correct and must stay correct.
    let rf = f.trace(&["find", "*.sh", "hooks", "--json"]);
    rf.ok();
    let vf = rf.view();
    let found: Vec<&str> = vf["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["path"].as_str().unwrap())
        .collect();
    assert_eq!(
        found,
        vec!["hooks/kept.sh"],
        "find must return only the on-disk match: {}",
        vf["results"]
    );
}

#[test]
fn co_change_ignores_a_commit_over_the_file_cap() {
    // Co-change pairs every file in a commit with every other, so a commit's
    // cost is quadratic in its file count and a sweep touching everything
    // raises every pair by one — ranking nothing while burying the couplings
    // that mean something. A commit over the cap contributes nothing, and the
    // genuine two-file pair survives it.
    let f = Fixture::new();
    f.write("pair_a.py", "a = 1\n");
    f.write("pair_b.py", "b = 1\n");
    f.commit("the real coupling");
    for i in 0..120 {
        f.write(&format!("bulk/mod_{i:03}.py"), "x = 1\n");
    }
    f.write("pair_a.py", "a = 2\n");
    f.commit("sweep touching 121 files");

    let r = f.trace(&["read", "pair_a.py", "--json"]);
    r.ok();
    let git = r.view()["files"]["pair_a.py"]["git"].clone();
    assert_eq!(
        git["usually_changed_with"],
        serde_json::json!(["pair_b.py"]),
        "the two-file coupling must survive the sweep, and a commit over the cap must contribute no co-change: {git}"
    );
}

#[test]
fn warm_file_commands_time_only_the_tracked_git_batch() {
    let f = Fixture::new();
    f.write("entry.py", "class Entry:\n    pass\n");
    f.commit("seed");
    f.trace(&["context", "entry.py", "--no-record"]).ok();
    std::thread::sleep(std::time::Duration::from_secs(2));
    f.trace(&["context", "entry.py", "--no-record"]).ok();

    for args in [
        vec!["context", "entry.py", "--no-record"],
        vec!["read", "entry.py"],
        vec!["info", "entry.py"],
    ] {
        let run = f.trace_env(&args, &[("TRACE_TIMING", "1")]);
        run.ok();
        assert_eq!(
            run.stderr.matches("timing git status").count(),
            1,
            "{}",
            run.stderr
        );
        assert_eq!(
            run.stderr.matches("timing git rev-parse HEAD").count(),
            1,
            "{}",
            run.stderr
        );
        assert_eq!(
            run.stderr.matches("timing git for-each-ref").count(),
            1,
            "{}",
            run.stderr
        );
        assert!(!run.stderr.contains("ls-files"), "{}", run.stderr);
        assert!(!run.stderr.contains("show-toplevel"), "{}", run.stderr);
    }

    let stats = f.trace_env(&["stats"], &[("TRACE_TIMING", "1")]);
    stats.ok();
    assert!(!stats.stderr.contains("timing git"), "{}", stats.stderr);

    let status = f.trace_env(&["status"], &[("TRACE_TIMING", "1")]);
    status.ok();
    assert_eq!(
        status.stderr.matches("timing git status").count(),
        1,
        "{}",
        status.stderr
    );
    assert!(!status.stderr.contains("ls-files"), "{}", status.stderr);
}

#[test]
fn context_keeps_untracked_and_staged_rename_states() {
    let f = Fixture::new();
    f.write("tracked.py", "value = 1\n");
    f.commit("seed");
    f.write("untracked.py", "value = 2\n");
    let untracked = f.trace(&["context", "untracked.py", "--no-record"]);
    untracked.ok();
    assert!(
        untracked.stdout.contains("untracked"),
        "{}",
        untracked.stdout
    );

    f.git(&["mv", "tracked.py", "renamed.py"]);
    let renamed = f.trace(&["context", "renamed.py", "--no-record"]);
    renamed.ok();
    assert!(renamed.stdout.contains("renamed"), "{}", renamed.stdout);
    assert!(renamed.stdout.contains("renamed_from: tracked.py"), "{}", renamed.stdout);
    assert!(renamed.stdout.contains("commits: 1"), "{}", renamed.stdout);
}

#[test]
fn status_and_list_name_partly_staged_and_untracked_files() {
    let f = Fixture::new();
    f.write("tracked.py", "value = 1\n");
    f.write("partly.py", "value = 1\n");
    f.commit("seed");
    f.write("tracked.py", "value = 2\n");
    f.git(&["add", "tracked.py"]);
    f.write("tracked.py", "value = 3\n");
    f.write("partly.py", "value = 2\n");
    f.git(&["add", "partly.py"]);
    f.write("partly.py", "value = 3\n");
    f.write("untracked.py", "value = 4\n");
    f.trace(&["info", "tracked.py", "--json"]).ok();

    let status = f.trace(&["status"]);
    status.ok();
    for expected in [
        "3 files with uncommitted state:",
        "## modified\n  partly.py · partly staged  {",
        "\n  tracked.py · partly staged  {",
        "## untracked\n  untracked.py  {",
        "git: untracked}",
    ] {
        assert!(status.stdout.contains(expected), "{expected:?}:\n{}", status.stdout);
    }

    let list = f.trace(&["list", "."]);
    list.ok();
    for (name, state) in [
        ("partly.py", "git: modified"),
        ("tracked.py", "git: modified"),
        ("untracked.py", "git: untracked"),
    ] {
        let line = list
            .stdout
            .lines()
            .find(|line| line.trim_start().starts_with(name))
            .unwrap_or_else(|| panic!("{name} missing:\n{}", list.stdout));
        assert!(line.contains(state), "{line}");
    }
}

#[test]
fn shallow_clone_graft_commit_leaves_no_history() {
    // A graft commit has no parent to diff against, so git reports the whole
    // working tree as added. Left in, every file in a shallow clone claims the
    // clone date as its first commit, HEAD's author as its owner, and the
    // alphabetical head of the tree as its co-change. Grafts leave the walk,
    // so a depth-1 clone carries no history at all — while presence, which is
    // a fact about the current refs rather than about history, stays.
    let f = Fixture::new();
    f.write("core.py", "def a():\n    return 1\n");
    f.write("sibling.py", "x = 1\n");
    f.commit("first");
    f.write("core.py", "def a():\n    return 2\n");
    f.commit("second");

    // `--depth` is silently ignored for a plain local path — git warns and
    // clones the whole history — so the source must be addressed as file://,
    // and the fixture asserts it really came out shallow.
    let url = format!("file://{}", f.root.to_string_lossy());
    let clone = f.root.join("clone");
    f.git(&["clone", "--depth=1", &url, clone.to_str().unwrap()]);
    assert!(
        clone.join(".git").join("shallow").exists(),
        "fixture is not a shallow clone — the source must be addressed as file://"
    );

    let r = tracer_cli_tests::trace(&clone, ["read", "core.py", "--json"]);
    r.ok();
    let git = r.view()["files"]["core.py"]["git"].clone();
    assert_eq!(git["commits"], 0, "a graft commit must leave no history: {git}");
    assert!(git["first_commit"].is_null(), "a graft commit dates nothing: {git}");
    assert!(
        git["usually_changed_with"].is_null(),
        "a graft commit must contribute no co-change: {git}"
    );
    assert!(git["main_author"].is_null(), "a graft commit must attribute no owner: {git}");
}

#[test]
fn structure_reports_a_failing_ctags_instead_of_thinning_its_answer() {
    // `structure` takes its symbols from universal-ctags and backfills from the
    // tree-sitter cache when ctags returns none, which is what keeps .tsx
    // useful. A ctags that runs and fails lands on that same empty-symbol path,
    // so the command used to exit 0 with a thinner answer and nothing saying
    // the tool had failed. That is not hypothetical: on linux-x86_64 an absent
    // ctags reaches it, because a missing binary does not surface as a spawn
    // error there the way it does on mac-arm64 and linux-arm64.
    let f = Fixture::new();
    f.write("m.lua", "function alpha()\n    return 1\nend\n");
    f.commit("lua file");

    let shim_dir = f.root.join("shim");
    std::fs::create_dir_all(&shim_dir).unwrap();
    let shim = shim_dir.join("ctags");
    std::fs::write(&shim, "#!/bin/sh\necho 'ctags: boom' >&2\nexit 1\n").unwrap();
    std::fs::set_permissions(
        &shim,
        <std::fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o755),
    )
    .unwrap();

    // Prepended to the inherited PATH, never a fixed list: the harness resolves
    // `trace` on PATH too, and a hand-written list drops wherever it lives —
    // on macOS that silently runs Apple's own /usr/bin/trace instead.
    let path = format!(
        "{}:{}",
        shim_dir.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let r = f.trace_env(&["structure", "m.lua", "--json"], &[("PATH", &path)]);
    assert_ne!(
        r.code,
        0,
        "a ctags that fails must not pass for a thin answer: {}",
        r.combined()
    );
    assert!(
        r.combined().contains("ctags failed"),
        "the failure must name ctags: {}",
        r.combined()
    );
}

#[test]
fn surface_rows_repeat_on_context_read_info_and_blame() {
    let f = Fixture::new();
    f.write(
        "entry.php",
        "<?php\nclass Entry {\n    public static function create(array $values = []): static {\n        return new static();\n    }\n}\n",
    );
    f.commit("entry surface");
    let session = fresh_session_id("surface-rows");
    let env = [("CLAUDE_CODE_SESSION_ID", session.as_str())];

    let first = f.trace_env(&["context", "entry.php"], &env);
    let second = f.trace_env(&["context", "entry.php"], &env);
    first.ok();
    second.ok();
    for output in [&first.stdout, &second.stdout] {
        assert!(output.contains("class Entry"), "{output}");
        assert!(
            output.contains("function create(array $values = []): static"),
            "{output}"
        );
        assert!(!output.contains("[symbols:"), "{output}");
        assert!(!output.contains("annotated:"), "{output}");
    }
    assert_eq!(first.stdout, second.stdout);

    let read = f.trace(&["read", "entry.php", "--method", "create", "--json"]);
    read.ok();
    assert!(read.stdout.contains("class Entry"), "{}", read.stdout);
    assert!(
        read.stdout
            .contains("function create(array $values = []): static"),
        "{}",
        read.stdout
    );
    assert!(read.json()["context"]["files"]["entry.php"]["surface"].is_array());

    let info = f.trace(&["info", "entry.php", "--json"]);
    info.ok();
    assert!(info.json()["context"]["files"]
        .as_object()
        .unwrap()
        .values()
        .next()
        .unwrap()["surface"]
        .is_array());

    let blame = f.trace(&["blame", "entry.php", "create", "--json"]);
    blame.ok();
    assert_eq!(
        blame.json()["context"]["files"]
            .as_object()
            .unwrap()
            .values()
            .next()
            .unwrap()["surface"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn surface_rows_follow_windows_history_budgets_and_batch_json() {
    let f = Fixture::new();
    f.write(
        "entry.php",
        "<?php\nclass Entry {\n    #[Field]\n    public static function create(): static {\n        return new static();\n    }\n\n    public function other(): void {}\n}\n",
    );
    f.write("a.php", "<?php\nclass A {}\n");
    f.write("b.php", "<?php\nclass B {}\n");
    f.commit("surface rows");

    let window = f.trace(&["context", "entry.php", "--offset", "4", "--limit", "1"]);
    window.ok();
    assert!(window.stdout.contains("class Entry"), "{}", window.stdout);
    assert!(
        window.stdout.contains("function create(): static"),
        "{}",
        window.stdout
    );
    assert!(
        !window.stdout.contains("function other(): void"),
        "{}",
        window.stdout
    );

    let read_window = f.trace(&["read", "entry.php", "--lines", "4:5"]);
    read_window.ok();
    assert!(
        read_window.stdout.contains("class Entry"),
        "{}",
        read_window.stdout
    );
    assert!(
        read_window.stdout.contains("function create(): static"),
        "{}",
        read_window.stdout
    );
    assert!(
        !read_window.stdout.contains("function other(): void"),
        "{}",
        read_window.stdout
    );

    f.write(
        "entry.php",
        "<?php\nclass Entry {\n    public static function create(): static {\n        return new static();\n    }\n\n    public function added(): void {}\n}\n",
    );
    let historical = f.trace(&["read", "entry.php", "--at", "HEAD", "--json"]);
    historical.ok();
    let historical_json = historical.json();
    let historical_surface = &historical_json["context"]["files"]
        .as_object()
        .expect("historical read files")
        .values()
        .next()
        .expect("historical read file")["surface"];
    assert!(
        historical_surface.to_string().contains("create"),
        "{historical_surface}"
    );
    assert!(
        !historical_surface.to_string().contains("added"),
        "historical rows must come from --at content: {historical_surface}"
    );

    let large = format!("class Big:\n{}", oversized_python(2000));
    f.write("big.py", &large);
    let budget = f.trace(&["read", "big.py", "--json"]);
    budget.ok();
    assert_eq!(budget.view()["results"][0]["truncated"], true);
    assert!(budget.stdout.contains("class Big"), "{}", budget.stdout);
    assert!(budget.json()["context"]["files"]
        .as_object()
        .unwrap()
        .values()
        .next()
        .unwrap()["surface"]
        .is_array());

    let batch = f.trace(&["context", "a.php", "b.php", "--no-record", "--json"]);
    batch.ok();
    assert!(batch.view()["results"][0]["content"]
        .as_str()
        .unwrap()
        .contains("class A"));
    assert!(batch.view()["results"][1]["content"]
        .as_str()
        .unwrap()
        .contains("class B"));
    assert!(batch.json()["context"]["files"]["a.php"]["surface"].is_array());
    assert!(batch.json()["context"]["files"]["b.php"]["surface"].is_array());
}

#[test]
fn defines_uses_the_inline_surface_row() {
    let f = Fixture::new();
    f.write(
        "constants.ts",
        "export const labels = [\n    'save',\n    'cancel',\n];\n",
    );
    f.commit("inline declaration rows");

    let structure = f.trace(&["structure", "constants.ts", "--json"]);
    structure.ok();
    let structure_view = structure.view();
    let header = structure_view["symbols_by_kind"]["constant"][0]["header"]
        .as_str()
        .unwrap();
    // `--json` keeps the header whole; the row elides the table like a body.
    assert!(header.contains("'cancel'"), "{header}");
    let inline = "export const labels = …";

    let defines = f.trace(&["defines", "labels"]);
    defines.ok();
    assert!(
        defines.stdout.contains(&format!("      L1    {inline}")),
        "defines did not use the structure row's inline renderer:\n{}",
        defines.stdout
    );
}

#[test]
fn warm_directory_listing_uses_one_read_dir_without_file_facts() {
    let f = Fixture::new();
    for index in 0..300 {
        f.write(&format!("entries/file_{index:03}.php"), "<?php\n");
    }
    f.trace(&["context", "entries"]).ok();

    let run = f.trace_env(&["context", "entries"], &[("TRACE_TIMING", "1")]);
    run.ok();
    assert!(
        run.stdout.contains("  entries: [file_000.php, file_001.php,"),
        "{}",
        run.stdout
    );
    assert!(run.stdout.contains("\n  total_entries: 300\n"), "{}", run.stdout);
    assert!(
        !run.stderr.contains("timing facts"),
        "directory listing must not resolve file facts: {}",
        run.stderr
    );
}

/// A file whose rows alone overflow a small budget, and whose content runs
/// far past it: 60 functions, each with a long signature.
fn crowded_python() -> String {
    (0..60)
        .map(|n| {
            format!(
                "def function_{n:02}(first_argument, second_argument, third_argument, fourth_argument):\n    if first_argument:\n        return second_argument\n    return third_argument + fourth_argument\n\n"
            )
        })
        .collect()
}

#[test]
fn read_fits_the_budget_and_names_every_declaration() {
    let f = Fixture::new();
    f.write("crowded.py", &crowded_python());
    f.commit("crowded file");

    let r = f.trace(&["read", "crowded.py", "--budget", "4000"]);
    r.ok();
    let size = r.stdout.chars().count();
    assert!(size <= 4000, "read spent {size} characters of a 4000 budget:\n{}", r.stdout);
    // The content stops early, so the later functions are named by their
    // rows alone.
    for n in 0..60 {
        assert!(r.stdout.contains(&format!("function_{n:02}")), "function_{n:02} is unnamed:\n{}", r.stdout);
    }
    assert!(r.stdout.contains("[trimmed at L"), "the cut must say where it landed:\n{}", r.stdout);

    let whole = f.trace(&["read", "crowded.py", "--budget", "0"]);
    whole.ok();
    assert!(!whole.stdout.contains("[trimmed at L"), "--budget 0 is unbounded:\n{}", whole.stdout);
    assert!(whole.stdout.contains("L299:     return third_argument"), "{}", whole.stdout);
}

#[test]
fn context_names_every_declaration_within_the_budget() {
    let f = Fixture::new();
    f.write("crowded.py", &crowded_python());
    f.commit("crowded file");

    let r = f.trace(&["context", "crowded.py", "--no-record", "--budget", "2500"]);
    r.ok();
    let size = r.stdout.chars().count();
    assert!(size <= 2500, "context spent {size} characters of a 2500 budget:\n{}", r.stdout);
    for n in 0..60 {
        assert!(r.stdout.contains(&format!("function_{n:02}")), "function_{n:02} is unnamed:\n{}", r.stdout);
    }
    assert!(r.stdout.contains("--budget 0]"), "shortened rows must name the whole command:\n{}", r.stdout);
}

/// A file with more declarations than even one name per row can fit still
/// names where they are: one line per parent, then counts per kind.
#[test]
fn a_file_too_large_for_its_names_fits_the_budget_by_parent() {
    let f = Fixture::new();
    let many: String = (0..3000).map(|n| format!("def generated_{n}(x):\n    return x\n")).collect();
    f.write("generated.py", &many);
    f.commit("generated");

    for (command, budget) in [(vec!["context", "generated.py", "--no-record"], 3000), (vec!["info", "generated.py"], 6000)] {
        let mut args = command.clone();
        args.extend(["--budget", if budget == 3000 { "3000" } else { "6000" }]);
        let r = f.trace(&args);
        r.ok();
        let size = r.stdout.chars().count();
        assert!(size <= budget, "{command:?} spent {size} characters of a {budget} budget");
        assert!(r.stdout.contains("top level: 3000 functions"), "{command:?}:\n{}", r.stdout);
        assert!(r.stdout.contains("--budget 0]"), "{command:?} must name the whole command:\n{}", r.stdout);
    }

    let properties: String = (0..3000).map(|n| format!("  field_{n} = {n};\n")).collect();
    f.write("holder.ts", &format!("class Holder {{\n{properties}}}\n"));
    f.commit("holder");
    let r = f.trace(&["context", "holder.ts", "--no-record", "--budget", "3000"]);
    r.ok();
    assert!(r.stdout.contains("L1 Holder: 3000 properties"), "{}", r.stdout);
}

/// A missing path is named and the run exits 2, as every path argument does.
#[test]
fn context_names_a_missing_path() {
    let f = Fixture::new();
    f.write("present.py", "value = 1\n");
    f.commit("one file");

    let r = f.trace(&["context", "missing.py"]);
    r.code_is(2);
    assert!(r.stderr.contains("'missing.py' does not exist"), "{}", r.stderr);
}

/// A data table's rows are data: every row elides a multi-line initializer
/// like a body, not only grep's group lines.
#[test]
fn rows_elide_a_multi_line_initializer() {
    let out = structure_text(
        "table.rs",
        "pub const TABLE: &[(&str, u32)] = &[\n    (\"a\", 1),\n    (\"b\", 2),\n    (\"c\", 3),\n];\n",
    );
    assert!(out.contains("pub const TABLE: &[(&str, u32)] = …"), "{out}");
    assert!(!out.contains("(\"b\", 2)"), "{out}");
}

#[test]
fn rows_keep_a_field_under_an_attribute_with_an_equals_sign() {
    let out = structure_text(
        "row.rs",
        "pub struct Row {\n    #[serde(rename = \"Code\")]\n    code: i64,\n}\n",
    );
    assert!(out.contains("code: i64"), "{out}");
    assert!(!out.contains("rename = …"), "{out}");
}

fn structure_text(file: &str, source: &str) -> String {
    let f = Fixture::new();
    f.write(file, source);
    f.commit("shapes");
    let r = f.trace(&["structure", file]);
    r.ok();
    r.stdout.clone()
}

#[test]
fn typescript_inline_object_types_print_their_members_once() {
    let out = structure_text(
        "shapes.ts",
        "export function load(options: { id: number; name: string }): { ok: boolean } {\n  return { ok: true };\n}\n\nexport type Config = {\n  host: string;\n  port: number;\n};\n",
    );
    assert!(
        out.contains("L1    export function load(options: { … }): { … } { … }")
            && out.contains("\nL1      id: number;\n")
            && out.contains("\nL5    export type Config = { … };\nL6      host: string;\nL7      port: number;\n"),
        "an inline object type is `{{ … }}` in the header and its members are nested rows:\n{out}"
    );
    assert_eq!(out.matches("id: number").count(), 1, "{out}");
}

#[test]
fn typescript_function_locals_are_not_rows() {
    let out = structure_text(
        "locals.ts",
        "export function load(id: number): number {\n  const local = id + 1;\n  let other = local * 2;\n  return other;\n}\n",
    );
    assert!(!out.contains("const local") && !out.contains("let other"), "{out}");
    assert!(out.contains("L1    export function load(id: number): number { … }"), "{out}");
}

#[test]
fn rust_struct_like_variants_nest_their_fields_once() {
    let out = structure_text(
        "event.rs",
        "pub enum Event {\n    Created { id: u64, name: String },\n    Deleted(u64),\n}\n",
    );
    assert!(
        out.contains("L2      Created { … }\nL2        id: u64\nL2        name: String\nL3      Deleted(u64)\n"),
        "{out}"
    );
}

#[test]
fn python_headers_stop_at_the_colon() {
    let out = structure_text(
        "compute.py",
        "def compute(first, second):\n    # Adds the two values.\n    return first + second\n",
    );
    assert!(out.contains("L1    def compute(first, second): …"), "{out}");
    assert!(!out.contains("# Adds"), "the body's comment is not the header:\n{out}");
}

#[test]
fn php_nullsafe_calls_are_callers() {
    let f = Fixture::new();
    f.write(
        "Store.php",
        "<?php\nclass Store\n{\n    public function total(?Order $order): void\n    {\n        $order?->recalculateStats();\n    }\n}\n",
    );
    f.write("Order.php", "<?php\nclass Order\n{\n    public function recalculateStats(): void {}\n}\n");
    f.commit("nullsafe call");
    let r = f.trace(&["callers", "recalculateStats"]);
    r.ok();
    assert!(
        r.stdout.contains("\n    Store.php  ") && r.stdout.contains("public function total(?Order $order): void { … }  // complexity 1 @ L6"),
        "a `?->` call is a caller:\n{}",
        r.stdout
    );
}

#[test]
fn callers_fit_the_budget_and_name_each_caller_once() {
    let f = Fixture::new();
    f.write("util.py", "def helper():\n    return 1\n");
    for index in 0..60 {
        f.write(
            &format!("use_{index:02}.py"),
            "from util import helper\n\ndef run():\n    helper()\n    helper()\n    return helper()\n",
        );
    }
    f.commit("many callers");

    let whole = f.trace(&["callers", "helper"]);
    whole.ok();
    assert_eq!(whole.stdout.matches("def run(): …").count(), 60, "{}", whole.stdout);
    assert!(whole.stdout.contains("@ L4, L5, L6"), "{}", whole.stdout);

    let r = f.trace(&["callers", "helper", "--budget", "2500"]);
    r.ok();
    assert!(r.stdout.chars().count() <= 2500, "{} characters:\n{}", r.stdout.chars().count(), r.stdout);
    for index in 0..60 {
        assert!(r.stdout.contains(&format!("use_{index:02}.py")), "use_{index:02}.py unnamed:\n{}", r.stdout);
    }
    assert!(r.stdout.contains("--budget 0]"), "{}", r.stdout);
}

#[test]
fn list_and_diff_take_several_paths() {
    let f = Fixture::new();
    f.write("a/one.py", "x = 1\n");
    f.write("b/two.py", "y = 2\n");
    f.write("c/three.py", "z = 3\n");
    f.commit("three directories");
    f.write("a/one.py", "x = 4\n");
    f.write("b/two.py", "y = 5\n");
    f.write("c/three.py", "z = 6\n");

    let r = f.trace(&["list", "a", "b"]);
    r.ok();
    assert!(r.stdout.contains("one.py") && r.stdout.contains("two.py"), "{}", r.stdout);

    let r = f.trace(&["diff", "a", "b", "--json"]);
    r.ok();
    let mut changed: Vec<String> = r.json()["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["path"].as_str().unwrap().to_string())
        .collect();
    changed.sort();
    assert_eq!(changed, vec!["a/one.py", "b/two.py"], "{}", r.stdout);

    let r = f.trace(&["list", "a", "missing"]);
    r.code_is(2);
    assert!(r.stdout.contains("one.py") && r.combined().contains("missing"), "{}", r.combined());
}

#[test]
fn read_names_the_docs_not_loaded_like_context() {
    let f = Fixture::new();
    f.write("sub/Claude.md", "# Sub\n\n- A rule for sub.\n");
    f.write("sub/x.py", "def x():\n    return 1\n");
    f.commit("nested doc");

    let context = f.trace(&["context", "sub/x.py", "--no-record"]);
    context.ok();
    assert!(context.stdout.contains("docs_not_loaded") && context.stdout.contains("sub/Claude.md"), "{}", context.stdout);
    let read = f.trace(&["read", "sub/x.py"]);
    read.ok();
    assert!(read.stdout.contains("docs_not_loaded") && read.stdout.contains("sub/Claude.md"), "{}", read.stdout);
}
