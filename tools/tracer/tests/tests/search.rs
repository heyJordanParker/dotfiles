//! Search commands: grep (ripgrep-backed), pattern (ast-grep-backed),
//! find (basename fnmatch and full-path glob). Correctness on human
//! and `--json` output plus the major flags.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;
use tracer_cli_tests::{standard_repo, trace, Fixture};

fn backend_path(fixture: &Fixture, case: &str, name: &str, script: &str) -> String {
    let directory = format!("backends/{case}");
    let path = fixture.write(&format!("{directory}/{name}"), script);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    format!(
        "{}:{}",
        fixture.path(&directory),
        std::env::var("PATH").expect("test runner PATH")
    )
}

/// A grep match's enrichment is not merely "an object" — its reported
/// complexity must equal what `info` independently reports for that exact
/// file. Two commands, one ground truth: a wrong enrichment join (stale
/// cache, wrong file keyed, lite-facts) fails this equality.
#[test]
fn grep_enrichment_complexity_equals_info_for_the_same_file() {
    let f = standard_repo();
    let g = f.trace(&["grep", "helper", "--path", ".", "--json"]);
    g.ok();
    let gv = g.view();
    assert_eq!(gv["pattern"], "helper");
    // The repo context over standard_repo is deterministic; pin its values
    // so a wrong enrichment join (the bug this test exists to catch) can't
    // hide behind a merely-an-object check.
    let rc = &gv["repo"];
    assert_eq!(
        rc["total_files"].as_i64().unwrap(),
        7,
        "repo_context: {}",
        gv
    );
    assert_eq!(
        rc["median_file_ccn"].as_i64().unwrap(),
        0,
        "repo_context: {}",
        gv
    );
    assert_eq!(
        rc["complexity_p95"].as_i64().unwrap(),
        1,
        "repo_context: {}",
        gv
    );

    // The match in src/util.py: `helper` first appears on the def line,
    // which is line 1 of src/util.py — exact, not "is a number".
    let m = gv["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["file"].as_str().unwrap().ends_with("src/util.py"))
        .expect("grep must hit src/util.py for 'helper'");
    assert_eq!(
        m["line"].as_i64().unwrap(),
        1,
        "helper's first occurrence is the def on src/util.py L1: {m}"
    );

    // Ground truth from `info` on that same file.
    let i = f.trace(&["info", "src/util.py", "--json"]);
    i.ok();
    let iv = i.view();
    let want_total = iv["ccn_total"].as_i64().unwrap();
    let want_max = iv["ccn_max_function"].as_i64().unwrap();
    let want_rank = iv["rank"].as_str().unwrap();
    // standard_repo()'s helper(v): base 1 + if(1) = exactly 2.
    assert_eq!(want_total, 2, "fixture sanity: helper() CCN is 2");

    // The enrichment lives in the per-file context, keyed by the same path
    // the row carries.
    let file = m["file"].as_str().unwrap();
    let fc = &gv["files"][file]["file_complexity"];
    assert_eq!(
        fc["ccn_total"].as_i64().unwrap(),
        want_total,
        "grep enrichment ccn_total ({}) != info ccn_total ({want_total}) \
         for src/util.py — enrichment is not cross-consistent",
        fc["ccn_total"]
    );
    assert_eq!(
        fc["ccn_max_function"].as_i64().unwrap(),
        want_max,
        "grep enrichment ccn_max ({}) != info ccn_max ({want_max})",
        fc["ccn_max_function"]
    );
    assert_eq!(
        fc["rank"].as_str().unwrap(),
        want_rank,
        "grep enrichment rank ({}) != info rank ({want_rank})",
        fc["rank"]
    );
}

/// The git enrichment block on a grep match must equal the file's actual
/// git facts — cross-checked against the fixture's known committer, not
/// just asserted present.
#[test]
fn grep_git_enrichment_reports_actual_commit_facts() {
    let f = standard_repo();
    let g = f.trace(&["grep", "helper", "--path", ".", "--json"]);
    g.ok();
    let gv = g.view();
    let m = gv["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["file"].as_str().unwrap().ends_with("src/util.py"))
        .expect("grep must hit src/util.py");
    let git = &gv["files"][m["file"].as_str().unwrap()]["git"];
    // The hermetic fixture commits exactly once, authored "Tracer Test".
    assert_eq!(
        git["last_author"].as_str().unwrap(),
        "Tracer Test",
        "grep git enrichment lost the fixture author: {git}"
    );
    assert_eq!(
        git["commits_30d"].as_i64().unwrap(),
        1,
        "the fixture has exactly one recent commit touching util.py: {git}"
    );
    // exempt-(a): last_modified is the calendar date of the fixture's
    // commit, generated at test run time; it shifts day-to-day and at the
    // UTC boundary, so the tightest stable invariant is the YYYY-MM-DD
    // shape rather than a fixed value.
    let lm = git["last_modified"]
        .as_str()
        .expect("last_modified present");
    assert!(
        lm.len() == 10
            && lm.as_bytes()[4] == b'-'
            && lm.as_bytes()[7] == b'-'
            && lm.chars().filter(|c| *c == '-').count() == 2
            && lm.chars().all(|c| c.is_ascii_digit() || c == '-'),
        "last_modified must be a YYYY-MM-DD date: {git}"
    );
}

/// The search-ordering defect, pinned. The backends (`rg`/`sg --json`)
/// walk files in parallel and emit per-file blocks in nondeterministic
/// order. Repeating the *identical* query must yield a byte-identical
/// document; this assertion fails if ordering regresses to unstable.
#[test]
fn grep_match_ordering_is_byte_identical_across_repeated_runs() {
    let f = Fixture::new();
    // Many files, same token — maximizes the cross-file ordering surface
    // the parallel walker would otherwise shuffle run-to-run.
    for i in 0..40 {
        f.write(&format!("d{i:02}/m.py"), "x = 1\ny = \"helper\"\n");
    }
    f.commit("many files one token");

    let first = f.trace(&["grep", "helper", "--path", ".", "--json"]);
    first.ok();
    // 40 files, the token on exactly one line of each → exactly 40
    // matches. The sanity gate is itself exact, not a floor.
    assert_eq!(
        first.json()["counts"]["matches"].as_i64().unwrap(),
        40,
        "fixture sanity: 40 files, one match each: {}",
        first.json()["counts"]["matches"]
    );
    // Several repeats; ALL must be byte-identical to the first.
    for run in 0..6 {
        let again = f.trace(&["grep", "helper", "--path", ".", "--json"]);
        again.ok();
        assert_eq!(
            again.stdout, first.stdout,
            "grep output not deterministic on identical run #{run} \
             — search-ordering instability regressed"
        );
    }
}

/// Same determinism contract for the AST search path (`struct`), which
/// shares the enrichment/ordering code with grep.
#[test]
fn pattern_match_ordering_is_byte_identical_across_repeated_runs() {
    let f = Fixture::new();
    for i in 0..25 {
        f.write(
            &format!("p{i:02}/mod.py"),
            "def alpha(a):\n    return a\n\ndef beta(b):\n    return b\n",
        );
    }
    f.commit("many py modules");
    let first = f.trace(&[
        "pattern",
        "def $N($$$A): $$$B",
        "-l",
        "python",
        "--path",
        ".",
        "--json",
    ]);
    first.ok();
    // 25 modules × 2 defs (alpha, beta) each → exactly 50 matches. The
    // sanity gate is exact, not a floor.
    assert_eq!(
        first.json()["counts"]["matches"].as_i64().unwrap(),
        50,
        "fixture sanity: 25 modules × 2 defs = 50: {}",
        first.stdout
    );
    for run in 0..5 {
        let again = f.trace(&[
            "pattern",
            "def $N($$$A): $$$B",
            "-l",
            "python",
            "--path",
            ".",
            "--json",
        ]);
        again.ok();
        assert_eq!(
            again.stdout, first.stdout,
            "struct output not deterministic on identical run #{run}"
        );
    }
}

#[test]
fn grep_lang_filter_restricts_results() {
    let f = standard_repo();
    let r = f.trace(&["grep", "CONST", "--path", ".", "-l", "ts", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(v["lang"], "ts");
    for m in v["results"].as_array().unwrap() {
        let file = m["file"].as_str().unwrap();
        assert!(
            file.ends_with(".ts") || file.ends_with(".tsx"),
            "lang filter leaked non-ts file: {file}"
        );
    }
}

/// A search whose word is a name the graph knows ends with the command that
/// answers the question whole. A structural pattern matches one call shape,
/// so its result is partial in silence; the signpost is what turns that into
/// `trace callers <name>`.
#[test]
fn search_signposts_the_graph_command_for_a_known_symbol() {
    let f = Fixture::new();
    f.write("lib.py", "def dispatch(job):\n    return job\n");
    f.write(
        "app.py",
        "from lib import dispatch\n\ndef run(j):\n    return dispatch(j)\n",
    );
    f.commit("one call site");
    f.trace(&["cache", "build", "."]).ok();

    let want = "dispatch is a function \u{00b7} 1 mentioning file \u{00b7} 1 definitions \
                \u{2192} trace callers dispatch";
    for args in [
        vec!["grep", "dispatch", "--path", ".", "--json"],
        vec![
            "pattern",
            "dispatch($$$A)",
            "-l",
            "python",
            "--path",
            ".",
            "--json",
        ],
    ] {
        let r = f.trace(&args);
        r.ok();
        assert_eq!(
            r.view()["signpost"].as_str().unwrap_or(""),
            want,
            "{args:?} must name the graph command: {}",
            r.stdout
        );
    }

    // The same line reaches a human caller, after the summary.
    let h = f.trace(&["grep", "dispatch", "--path", "."]);
    h.ok();
    assert!(
        h.stdout.contains("\u{2192} trace callers dispatch"),
        "the human render must carry the signpost:\n{}",
        h.stdout
    );
}

/// A search word the graph does not know says nothing extra. `Entry` is
/// prose inside app.py's docstring, not a declared name.
#[test]
fn search_signpost_is_absent_for_a_word_that_is_not_a_symbol() {
    let f = standard_repo();
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["grep", "Entry", "--path", ".", "--json"]);
    r.ok();
    assert!(
        r.view()["signpost"].is_null(),
        "a plain text search must carry no signpost: {}",
        r.stdout
    );
}

#[test]
fn bare_name_and_equivalent_regex_keep_the_same_search_rows() {
    let f = standard_repo();
    f.trace(&["cache", "build", "."]).ok();
    let bare = f.trace(&["grep", "helper", "--path", ".", "--json"]);
    let regex = f.trace(&["grep", "(?:helper)", "--path", ".", "--json"]);
    bare.ok();
    regex.ok();
    let bare = bare.json();
    let regex = regex.json();
    assert_eq!(bare["results"], regex["results"]);
    assert_eq!(bare["counts"], regex["counts"]);
    assert_eq!(bare["context"]["files"], regex["context"]["files"]);
    assert!(bare["context"]["signpost"].is_string());
    assert!(regex["context"]["signpost"].is_null());
}

#[test]
fn search_context_observations_overlap_without_duplicate_work() {
    let real_git = String::from_utf8(
        Command::new("which")
            .arg("git")
            .output()
            .expect("locate git")
            .stdout,
    )
    .unwrap()
    .trim()
    .to_string();
    let real_scc = String::from_utf8(
        Command::new("which")
            .arg("scc")
            .output()
            .expect("locate scc")
            .stdout,
    )
    .unwrap()
    .trim()
    .to_string();

    for (name, args) in [
        ("grep", vec!["grep", "helper", "--path", ".", "--json"]),
        (
            "pattern",
            vec![
                "pattern",
                "def $N($$$A): $$$B",
                "-l",
                "python",
                "--path",
                ".",
                "--json",
            ],
        ),
    ] {
        for workers in ["1", "4"] {
            let fixture = standard_repo();
            let markers = fixture.root.join(".tracer-cache/search-context-overlap");
            fs::create_dir_all(&markers).unwrap();
            let status_started = markers.join("status-started");
            let scc_started = markers.join("scc-started");
            let status_saw_scc = markers.join("status-saw-scc");
            let scc_saw_status = markers.join("scc-saw-status");
            let status_count = markers.join("status-count");
            let scc_count = markers.join("scc-count");
            let case = format!("context-overlap-{name}-{workers}");
            let wait_attempts = if workers == "1" { 0 } else { 100 };

            let path = backend_path(
                &fixture,
                &case,
                "git",
                &format!(
                    "#!/bin/sh\nif [ \"$1\" = status ]; then\n  printf x >> '{status_count}'\n  touch '{status_started}'\n  attempts=0\n  while [ ! -e '{scc_started}' ] && [ \"$attempts\" -lt {wait_attempts} ]; do\n    sleep 0.01\n    attempts=$((attempts + 1))\n  done\n  if [ -e '{scc_started}' ]; then touch '{status_saw_scc}'; fi\nfi\nexec '{real_git}' \"$@\"\n",
                    status_count = status_count.display(),
                    status_started = status_started.display(),
                    scc_started = scc_started.display(),
                    status_saw_scc = status_saw_scc.display(),
                ),
            );
            let same_path = backend_path(
                &fixture,
                &case,
                "scc",
                &format!(
                    "#!/bin/sh\nprintf x >> '{scc_count}'\ntouch '{scc_started}'\nattempts=0\nwhile [ ! -e '{status_started}' ] && [ \"$attempts\" -lt {wait_attempts} ]; do\n  sleep 0.01\n  attempts=$((attempts + 1))\ndone\nif [ -e '{status_started}' ]; then touch '{scc_saw_status}'; fi\nexec '{real_scc}' \"$@\"\n",
                    scc_count = scc_count.display(),
                    scc_started = scc_started.display(),
                    status_started = status_started.display(),
                    scc_saw_status = scc_saw_status.display(),
                ),
            );
            assert_eq!(same_path, path, "both wrappers must share one PATH entry");

            let result = fixture.trace_env(
                &args,
                &[("PATH", path.as_str()), ("RAYON_NUM_THREADS", workers)],
            );
            result.ok();
            let document = result.json();
            assert!(document["query"].is_object(), "{document:#}");
            assert!(document["context"].is_object(), "{document:#}");
            assert!(document["results"].is_array(), "{document:#}");
            assert!(document["counts"].is_object(), "{document:#}");
            assert_eq!(
                fs::read(&status_count).unwrap(),
                b"x",
                "{name}/{workers}: enrichment must reuse the signpost's complete status observation"
            );
            assert_eq!(fs::read(&scc_count).unwrap(), b"x", "{name}/{workers}");

            if workers != "1" {
                assert!(
                    scc_saw_status.exists(),
                    "{name}/{workers}: scc started before its repository input scan"
                );
            }
        }
    }
}

#[test]
fn bare_searches_reuse_signpost_state_for_enrichment() {
    let real_git = String::from_utf8(
        Command::new("which")
            .arg("git")
            .output()
            .expect("locate git")
            .stdout,
    )
    .unwrap()
    .trim()
    .to_string();

    for (name, args, has_signpost) in [
        (
            "known-grep",
            vec!["grep", "helper", "--path", ".", "--json"],
            true,
        ),
        (
            "unknown-grep",
            vec!["grep", "Entry", "--path", ".", "--json"],
            false,
        ),
        (
            "known-pattern",
            vec![
                "pattern",
                "helper($A)",
                "-l",
                "python",
                "--path",
                ".",
                "--json",
            ],
            true,
        ),
        (
            "unknown-pattern",
            vec![
                "pattern",
                "def $N($$$A): $$$B",
                "-l",
                "python",
                "--path",
                ".",
                "--json",
            ],
            false,
        ),
    ] {
        let fixture = standard_repo();
        let arguments = fixture.path(&format!("{name}-git-arguments"));
        let path = backend_path(
            &fixture,
            &format!("bare-state-{name}"),
            "git",
            &format!(
                "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{arguments}'\nexec '{real_git}' \"$@\"\n"
            ),
        );
        let expected = fixture.trace(&args);
        expected.ok();
        let observed = fixture.trace_env(&args, &[("PATH", path.as_str())]);
        observed.ok();
        assert_eq!(
            observed.json(),
            expected.json(),
            "{name}: reordered context work changed the search document"
        );
        assert_eq!(
            observed.json()["context"]["signpost"].is_string(),
            has_signpost,
            "{name}: signpost meaning changed"
        );

        let invocations = fs::read_to_string(arguments).unwrap();
        assert_eq!(
            invocations
                .lines()
                .filter(|line| line.starts_with("status --porcelain=v1 -z --untracked-files=all"))
                .count(),
            1,
            "{name}: signpost must establish one complete status observation:\n{invocations}"
        );
        assert_eq!(
            invocations
                .lines()
                .filter(|line| line.starts_with("status --porcelain=v1 -z --untracked-files=no"))
                .count(),
            0,
            "{name}: enrichment repeated tracked state after signpost:\n{invocations}"
        );
        assert!(
            !invocations.contains("--literal-pathspecs ls-files --others"),
            "{name}: enrichment repeated exact untracked state after signpost:\n{invocations}"
        );
        assert_eq!(
            invocations
                .lines()
                .filter(|line| *line == "rev-parse HEAD")
                .count(),
            1,
            "{name}: enrichment rebuilt complete history after signpost:\n{invocations}"
        );
        assert_eq!(
            invocations
                .lines()
                .filter(|line| line.starts_with("for-each-ref --format="))
                .count(),
            1,
            "{name}: enrichment repeated deployment presence after signpost:\n{invocations}"
        );
    }
}

#[test]
fn search_resolves_git_state_once_for_all_matched_files() {
    let real_git = String::from_utf8(
        Command::new("which")
            .arg("git")
            .output()
            .expect("locate git")
            .stdout,
    )
    .unwrap()
    .trim()
    .to_string();

    for (name, args, files) in [
        (
            "grep",
            vec!["grep", "(?:needle)", "--path", ".", "--json"],
            513,
        ),
        (
            "pattern",
            vec!["pattern", "$X", "-l", "python", "--path", "src", "--json"],
            3,
        ),
    ] {
        let fixture = Fixture::new();
        for index in 0..files {
            fixture.write(
                &format!("src/matched-{index:03}.py"),
                &format!("needle = {index}\n"),
            );
        }
        fixture.write("outside/unrelated.py", "other = 1\n");
        fixture.commit("matched source files");

        let arguments = fixture.path(&format!("{name}-git-arguments"));
        let path = backend_path(
            &fixture,
            &format!("selected-state-{name}"),
            "git",
            &format!(
                "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{arguments}'\nexec '{real_git}' \"$@\"\n",
                arguments = arguments,
            ),
        );
        let result = fixture.trace_env(&args, &[("PATH", &path)]);
        result.ok();
        let document = result.json();
        assert_eq!(document["counts"]["files"], files, "{name}: {document:#}");
        assert_eq!(
            document["context"]["files"].as_object().unwrap().len(),
            files as usize,
            "{name}: every matched file keeps one complete context"
        );

        let invocations = fs::read_to_string(arguments).unwrap();
        assert_eq!(
            invocations
                .lines()
                .filter(|line| line.starts_with("status --porcelain=v1 -z --untracked-files=no"))
                .count(),
            0,
            "{name}: selected enrichment bypassed the complete snapshot observation:\n{invocations}"
        );
        assert_eq!(
            invocations
                .lines()
                .filter(|line| line.starts_with("status --porcelain=v1 -z --untracked-files=all"))
                .count(),
            1,
            "{name}: repository snapshot did not enumerate its complete input set:\n{invocations}"
        );
        assert!(
            invocations
            .lines()
                .all(|line| !line.starts_with("--literal-pathspecs ls-files --others")),
            "{name}: selected enrichment repeated untracked discovery after the snapshot:\n{invocations}"
        );
    }
}

#[test]
fn search_selected_state_matches_the_complete_git_view() {
    let fixture = Fixture::new();
    fixture.write(".gitignore", "*.ignored\n");
    for path in [
        "src/modified.py",
        "src/staged.py",
        "src/partial.py",
        "src/recreated.py",
        "src/clean.py",
        "outside/rename-source.py",
    ] {
        fixture.write(path, "needle = 1\n");
    }
    fixture.commit("initial states");

    fixture.write("src/modified.py", "needle = 2\n");
    fixture.write("src/staged.py", "needle = 2\n");
    fixture.git(&["add", "src/staged.py"]);
    fixture.write("src/partial.py", "needle = 2\n");
    fixture.git(&["add", "src/partial.py"]);
    fixture.write("src/partial.py", "needle = 3\n");
    fixture.git(&["mv", "outside/rename-source.py", "src/renamed.py"]);
    fixture.git(&["rm", "--quiet", "src/recreated.py"]);
    fixture.write("src/recreated.py", "needle = 2\n");
    for path in [
        "src/[literal]*?.py",
        "src/tab\tname.py",
        "src/line\nname.py",
        "src/unicodé-雪.py",
    ] {
        fixture.write(path, "needle = 1\n");
    }
    fixture.write("src/lx.py", "other = 1\n");
    fixture.write("outside/unrelated.py", "needle = 1\n");
    fixture.write("src/hidden.ignored", "needle = 1\n");
    fixture.write(".tracer-cache/selected.py", "needle = 1\n");

    let run = || {
        let result = fixture.trace(&["grep", "(?:needle)", "--path", "src", "--json"]);
        result.ok();
        result.json()
    };
    let cold = run();
    let warm = run();
    assert_eq!(cold, warm, "cold and warm selected state diverged");
    let files = cold["context"]["files"].as_object().unwrap();
    for (path, selected) in files {
        let result = fixture.trace(&["info", path, "--json"]);
        result.ok();
        let complete = result.json();
        let complete_file = complete["context"]["files"]
            .as_object()
            .unwrap()
            .values()
            .next()
            .unwrap();
        assert_eq!(
            selected["shoulder"], complete_file["shoulder"],
            "{path}: selected Git context differs from complete info: {complete:#}"
        );
        assert_eq!(
            selected["nearest_doc"], complete_file["nearest_doc"],
            "{path}: selected nearest doc differs from complete info: {complete:#}"
        );
        assert_eq!(
            selected["file_complexity"],
            serde_json::json!({
                "ccn_total": complete["counts"]["ccn_total"],
                "ccn_max_function": complete["counts"]["ccn_max_function"],
                "loc": complete["counts"]["loc"],
                "rank": complete["counts"]["rank"],
            }),
            "{path}: selected complexity differs from complete info: {complete:#}"
        );
    }
    let shoulder = |path: &str| {
        files[path]["shoulder"]
            .as_str()
            .unwrap_or_else(|| panic!("missing shoulder for {path:?}: {files:#?}"))
    };
    for path in ["src/modified.py", "src/staged.py", "src/partial.py"] {
        assert!(
            shoulder(path).contains("git: modified"),
            "{path}: {}",
            shoulder(path)
        );
    }
    assert!(
        shoulder("src/renamed.py").contains("git: renamed (uncommitted)"),
        "{}",
        shoulder("src/renamed.py")
    );
    for path in [
        "src/recreated.py",
        "src/[literal]*?.py",
        "src/tab\tname.py",
        "src/line\nname.py",
        "src/unicodé-雪.py",
    ] {
        assert!(
            shoulder(path).contains("git: untracked"),
            "{path}: {}",
            shoulder(path)
        );
    }
    assert!(!files.contains_key("outside/unrelated.py"));
    assert!(
        !files.contains_key("src/lx.py"),
        "the glob-magic decoy must not enter exact selected state"
    );

    let ignored = fixture.trace(&[
        "grep",
        "(?:needle)",
        "--path",
        "src/hidden.ignored",
        "--json",
    ]);
    ignored.ok();
    let ignored = ignored.json();
    assert_eq!(ignored["counts"]["matches"], 1);
    assert!(
        !ignored["context"]["files"]["src/hidden.ignored"]["shoulder"]
            .as_str()
            .unwrap()
            .contains("untracked"),
        "ignored files must not become untracked: {ignored:#}"
    );

    let cache = fixture.trace(&[
        "grep",
        "(?:needle)",
        "--path",
        ".tracer-cache/selected.py",
        "--json",
    ]);
    cache.ok();
    let cache = cache.json();
    assert_eq!(cache["counts"]["matches"], 1);
    assert!(
        !cache["context"]["files"][".tracer-cache/selected.py"]["shoulder"]
            .as_str()
            .unwrap()
            .contains("untracked"),
        "cache-path collapse changed: {cache:#}"
    );

    fixture.write("src/clean.py", "needle = 2\n");
    let changed = run();
    assert!(
        changed["context"]["files"]["src/clean.py"]["shoulder"]
            .as_str()
            .unwrap()
            .contains("git: modified"),
        "warm search missed a later working-tree change: {changed:#}"
    );
}

#[test]
fn failed_selected_untracked_observation_falls_back_to_complete_state() {
    let fixture = Fixture::new();
    fixture.write("tracked.py", "other = 1\n");
    fixture.commit("tracked base");
    fixture.write("selected.py", "needle = 1\n");

    let real_git = String::from_utf8(
        Command::new("which")
            .arg("git")
            .output()
            .expect("locate git")
            .stdout,
    )
    .unwrap()
    .trim()
    .to_string();
    let arguments = fixture.path("failed-selected-git-arguments");
    let path = backend_path(
        &fixture,
        "failed-selected-state",
        "git",
        &format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{arguments}'\nif [ \"$1\" = \"--literal-pathspecs\" ]; then exit 9; fi\nexec '{real_git}' \"$@\"\n"
        ),
    );
    let result = fixture.trace_env(
        &["grep", "(?:needle)", "--path", ".", "--json"],
        &[("PATH", &path)],
    );
    result.ok();
    let document = result.json();
    assert_eq!(document["counts"]["matches"], 1);
    let file = document["results"][0]["file"].as_str().unwrap();
    assert!(
        document["context"]["files"][file]["shoulder"]
            .as_str()
            .unwrap_or_else(|| panic!("selected shoulder missing: {document:#}"))
            .contains("git: untracked"),
        "fallback lost complete live state: {document:#}"
    );
    let invocations = fs::read_to_string(arguments).unwrap();
    assert_eq!(
        invocations
            .lines()
            .filter(|line| line.contains("--literal-pathspecs ls-files --others"))
            .count(),
        0,
        "complete snapshot made a selected untracked probe unnecessary:\n{invocations}"
    );
    assert!(
        invocations.contains("status --porcelain=v1 -z --untracked-files=all"),
        "failed selected state did not fall back to the complete observation:\n{invocations}"
    );
}

#[test]
fn search_selected_state_preserves_repository_boundaries() {
    let unborn = Fixture::new();
    unborn.write("new.py", "needle = 1\n");
    let result = unborn.trace(&["grep", "(?:needle)", "--path", "new.py", "--json"]);
    result.ok();
    let document = result.json();
    let file = document["results"][0]["file"].as_str().unwrap();
    assert!(
        document["context"]["files"][file]["shoulder"]
            .as_str()
            .unwrap()
            .contains("git: untracked"),
        "unborn repository lost its live state: {document:#}"
    );

    let target = unborn.write("target.py", "needle = 2\n");
    let link = unborn.root.join("link.py");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let result = unborn.trace(&["grep", "(?:needle)", "--path", "link.py", "--json"]);
    result.ok();
    let document = result.json();
    assert_eq!(document["results"][0]["file"], "link.py");
    assert!(
        document["context"]["files"].get("link.py").is_some(),
        "symlink result lost its matching context key: {document:#}"
    );

    let outside = unborn.root.with_file_name(format!(
        "{}-outside",
        unborn.root.file_name().unwrap().to_string_lossy()
    ));
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("standalone.py"), "needle = 3\n").unwrap();
    let result = trace(&outside, ["grep", "(?:needle)", "--path", ".", "--json"]);
    result.ok();
    let document = result.json();
    assert_eq!(document["counts"]["matches"], 1);
    let file = document["results"][0]["file"].as_str().unwrap();
    assert!(
        document["context"]["files"][file]["shoulder"]
            .as_str()
            .unwrap()
            .contains("git: no-history"),
        "outside-root fact identity changed: {document:#}"
    );
    fs::remove_dir_all(outside).unwrap();

    let submodule = Fixture::new();
    submodule.write("module.py", "needle = 4\n");
    submodule.commit("submodule source");
    let outer = Fixture::new();
    outer.write("outer.py", "outer = 1\n");
    outer.commit("outer source");
    outer.git(&[
        "-c",
        "protocol.file.allow=always",
        "submodule",
        "add",
        "--quiet",
        submodule.root.to_str().unwrap(),
        "vendor",
    ]);
    outer.commit("add submodule");
    let result = outer.trace(&["grep", "(?:needle)", "--path", "vendor/module.py", "--json"]);
    result.ok();
    let document = result.json();
    assert_eq!(document["results"][0]["file"], "vendor/module.py");
    assert_eq!(
        document["context"]["files"]["vendor/module.py"]["git"]["last_author"], "Tracer Test",
        "submodule search used the outer repository's activity: {document:#}"
    );
}

#[test]
fn symlinked_search_uses_canonical_git_and_complexity_facts() {
    let f = Fixture::new();
    f.write("real/source.py", "needle = 1\n");
    f.commit("real source");
    std::os::unix::fs::symlink(f.root.join("real"), f.root.join("linked"))
        .expect("directory symlink");

    let search = |path: &str| {
        let run = f.trace(&["grep", "needle", "--path", path, "--json"]);
        run.ok();
        let document = run.json();
        let file = document["results"][0]["file"]
            .as_str()
            .expect("one matched file");
        document["context"]["files"][file].clone()
    };
    let real = search("real");
    let linked = search("linked");
    for value in [&real, &linked] {
        assert!(value["file_complexity"].is_object(), "missing complexity: {value}");
        assert!(
            value["git"]["last_author"].as_str().is_some_and(|author| !author.is_empty())
                && value["git"]["last_modified"].as_str().is_some_and(|date| !date.is_empty()),
            "missing Git history: {value}"
        );
        assert!(value["shoulder"].as_str().is_some_and(|shoulder| !shoulder.is_empty()));
    }
    assert_eq!(linked["file_complexity"], real["file_complexity"]);
    assert_eq!(linked["git"], real["git"]);
    assert_eq!(linked["shoulder"], real["shoulder"]);
}

/// One language table for both searches: `tsx` means the same thing on each.
/// ripgrep has no `tsx` type, so `grep -l tsx` used to return zero matches
/// with no error while `pattern -l tsx` worked.
#[test]
fn a_language_name_means_the_same_on_grep_and_pattern() {
    let f = Fixture::new();
    f.write(
        "src/comp.tsx",
        "export function Widget() {\n  return 1;\n}\n",
    );
    f.write("src/other.py", "def widget():\n    return 1\n");
    f.commit("tsx and py");

    let g = f.trace(&["grep", "Widget", "--path", ".", "-l", "tsx", "--json"]);
    g.ok();
    let grep_files: Vec<String> = g.view()["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| {
            m["file"]
                .as_str()
                .unwrap()
                .trim_start_matches("./")
                .to_string()
        })
        .collect();
    assert_eq!(
        grep_files,
        vec!["src/comp.tsx".to_string()],
        "grep -l tsx must search the .tsx file: {}",
        g.stdout
    );

    let p = f.trace(&[
        "pattern",
        "function Widget() { $$$B }",
        "-l",
        "tsx",
        "--path",
        ".",
        "--json",
    ]);
    p.ok();
    let pattern_files: Vec<String> = p.view()["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| {
            m["file"]
                .as_str()
                .unwrap()
                .trim_start_matches("./")
                .to_string()
        })
        .collect();
    assert_eq!(
        pattern_files, grep_files,
        "both searches must reach the same file set for one language name: {}",
        p.stdout
    );
}

/// An unknown language is refused by name. Silence is the dangerous answer:
/// an empty result reads as "this does not exist".
#[test]
fn an_unknown_language_is_refused_by_name() {
    let f = standard_repo();
    for args in [
        vec!["grep", "helper", "--path", ".", "-l", "nosuchlang"],
        vec![
            "pattern",
            "def $N(): $$$B",
            "-l",
            "nosuchlang",
            "--path",
            ".",
        ],
    ] {
        let r = f.trace(&args);
        r.code_is(2);
        assert!(
            r.combined().contains("unknown language") && r.combined().contains("python"),
            "{args:?} must refuse by name and list the accepted ones: {}",
            r.combined()
        );
    }
}

/// A match spanning several lines is reported at the line holding the
/// pattern's own word, not at the line the expression starts on.
#[test]
fn a_multi_line_match_is_reported_at_the_line_of_the_name() {
    let f = Fixture::new();
    f.write(
        "chain.py",
        concat!(
            "def build(q):\n",     // L1
            "    return (q\n",     // L2
            "        .filter()\n", // L3
            "        .save())\n",  // L4
        ),
    );
    f.commit("chained call");
    let r = f.trace(&[
        "pattern",
        "$X.save($$$A)",
        "-l",
        "python",
        "--path",
        ".",
        "--json",
    ]);
    r.ok();
    let v = r.view();
    assert_eq!(
        v["results"][0]["line"].as_i64().unwrap(),
        4,
        "the chained .save() lives on L4, not on the L2 the expression starts on: {}",
        r.stdout
    );
    let snippet = v["results"][0]["snippet"].as_str().unwrap();
    assert!(
        snippet.contains(".save()") && !snippet.contains(".filter()"),
        "the snippet is that same line, not the whole expression: {snippet:?}"
    );
}

#[test]
fn grep_no_matches_is_clean_exit() {
    let f = standard_repo();
    let r = f.trace(&["grep", "zzz_no_such_token_qqq", "--path", "."]);
    r.ok();
    assert!(r.stdout.contains("(no matches)"), "{}", r.stdout);
}

#[test]
fn search_failures_are_not_successful_empty_documents() {
    let f = standard_repo();
    for args in [
        vec!["grep", "[", "--path", ".", "--json"],
        vec![
            "grep",
            "helper",
            "--path",
            ".",
            "--at",
            "no-such-revision",
            "--json",
        ],
    ] {
        let result = f.trace(&args);
        assert_ne!(
            result.code, 0,
            "{args:?} must fail, not return an empty document: {}",
            result.stdout
        );
        assert!(
            !result.stderr.trim().is_empty(),
            "{args:?} must disclose its backend diagnostic"
        );
        assert!(
            result.stdout.trim().is_empty(),
            "{args:?} must not emit partial success: {}",
            result.stdout
        );
    }
}

#[test]
fn search_backend_failures_and_malformed_output_are_disclosed() {
    let f = standard_repo();
    let path = backend_path(
        &f,
        "rg-status",
        "rg",
        "#!/bin/sh\necho planted-rg-failure >&2\nexit 9\n",
    );
    let result = f.trace_env(
        &["grep", "helper", "--path", ".", "--json"],
        &[("PATH", &path)],
    );
    assert_ne!(result.code, 0, "failed rg must not become an empty success");
    assert!(
        result.stderr.contains("planted-rg-failure"),
        "{}",
        result.stderr
    );

    let calls = f.path("remaining-rg-failure-calls");
    let first_finished = f.path("remaining-rg-first-finished");
    let path = backend_path(
        &f,
        "remaining-rg-status",
        "rg",
        &format!(
            "#!/bin/sh\nprintf x >> '{calls}'\nif [ -e '{first_finished}' ]; then\n  echo planted-remaining-rg-failure >&2\n  exit 9\nfi\ntouch '{first_finished}'\nprintf 'src/app.py\\0'\n"
        ),
    );
    let result = f.trace_env(
        &[
            "pattern",
            "main(helper($X))",
            "-l",
            "python",
            "--path",
            ".",
            "--json",
        ],
        &[("PATH", &path)],
    );
    assert_ne!(
        result.code, 0,
        "failed remaining-literal rg must not become an empty success"
    );
    assert!(
        result.stderr.contains("planted-remaining-rg-failure"),
        "{}",
        result.stderr
    );
    assert_eq!(
        fs::read(calls).unwrap().len(),
        2,
        "a non-capacity remaining-literal failure must not be retried"
    );
    assert!(
        result.stdout.trim().is_empty(),
        "a failed remaining-literal filter must not emit partial success"
    );

    let path = backend_path(
        &f,
        "git-status",
        "git",
        "#!/bin/sh\necho planted-git-failure >&2\nexit 9\n",
    );
    let result = f.trace_env(
        &["grep", "helper", "--path", ".", "--at", "HEAD", "--json"],
        &[("PATH", &path)],
    );
    assert_ne!(
        result.code, 0,
        "failed git grep must not become an empty success"
    );
    assert!(
        result.stderr.contains("planted-git-failure"),
        "{}",
        result.stderr
    );

    let sentinel = f.path("sg-status-invoked");
    let path = backend_path(
        &f,
        "sg-status",
        "sg",
        &format!(
            "#!/bin/sh\nprintf x >> '{}'\necho planted-sg-failure >&2\nexit 9\n",
            sentinel
        ),
    );
    let result = f.trace_env(
        &[
            "pattern",
            "def $N($$$A): $$$B",
            "-l",
            "python",
            "--path",
            ".",
            "--json",
        ],
        &[("PATH", &path)],
    );
    assert_ne!(result.code, 0, "failed sg must not become an empty success");
    assert!(
        result.stderr.contains("planted-sg-failure"),
        "{}",
        result.stderr
    );
    assert!(
        std::path::Path::new(&sentinel).exists(),
        "sg status fixture was not invoked"
    );
    assert_eq!(
        fs::read(&sentinel).unwrap().len(),
        1,
        "a non-capacity failure must not be retried"
    );
    assert!(
        result.stdout.trim().is_empty(),
        "partial or malformed sg output must not be emitted"
    );

    let path = backend_path(
        &f,
        "rg-malformed",
        "rg",
        "#!/bin/sh\nprintf '%s\\n' '{bad json'\nexit 0\n",
    );
    let result = f.trace_env(
        &["grep", "helper", "--path", ".", "--json"],
        &[("PATH", &path)],
    );
    assert_ne!(result.code, 0, "malformed rg JSON must fail");
    assert!(
        result.stderr.contains("malformed JSON"),
        "{}",
        result.stderr
    );
}

#[test]
fn search_backends_disclose_parse_cleanup_and_bounded_diagnostics() {
    let f = standard_repo();

    let path = backend_path(
        &f,
        "rg-missing-fields",
        "rg",
        "#!/bin/sh\necho '{\"type\":\"match\",\"data\":{}}'\n",
    );
    let result = f.trace_env(
        &["grep", "helper", "--path", ".", "--json"],
        &[("PATH", &path)],
    );
    assert_ne!(result.code, 0);
    assert!(result.stderr.contains("missing"), "{}", result.stderr);

    let path = backend_path(
        &f,
        "git-malformed",
        "git",
        "#!/bin/sh\necho 'not-a-git-grep-record'\nexit 0\n",
    );
    let result = f.trace_env(
        &["grep", "helper", "--path", ".", "--at", "HEAD", "--json"],
        &[("PATH", &path)],
    );
    assert_ne!(result.code, 0);
    assert!(
        result.stderr.contains("git grep wrote"),
        "{}",
        result.stderr
    );

    let path = backend_path(&f, "rg-partial-status", "rg", "#!/bin/sh\necho '{\"type\":\"match\",\"data\":{\"path\":{\"text\":\"src/util.py\"},\"lines\":{\"text\":\"helper\"},\"line_number\":1,\"submatches\":[{\"start\":0}]}}'\necho partial-status >&2\nexit 7\n");
    let result = f.trace_env(
        &["grep", "helper", "--path", ".", "--json"],
        &[("PATH", &path)],
    );
    assert_ne!(result.code, 0);
    assert!(
        result.stdout.trim().is_empty(),
        "partial match escaped: {}",
        result.stdout
    );
    assert!(
        result.stderr.contains("partial-status"),
        "{}",
        result.stderr
    );

    let path = backend_path(
        &f,
        "rg-continuing-malformed",
        "rg",
        "#!/bin/sh\necho '{bad json'\nexec sleep 20\n",
    );
    let result = f.trace_env(
        &["grep", "helper", "--path", ".", "--json"],
        &[("PATH", &path)],
    );
    assert_ne!(result.code, 0);
    assert!(
        result.elapsed.as_secs() < 5,
        "malformed backend was abandoned for {:?}",
        result.elapsed
    );

    let sentinel = f.path("sg-malformed-invoked");
    let path = backend_path(
        &f,
        "sg-malformed",
        "sg",
        &format!(
            "#!/bin/sh\ntouch '{}'\necho '{{bad json'\nexec sleep 20\n",
            sentinel
        ),
    );
    let result = f.trace_env(
        &[
            "pattern",
            "def $N($$$A): $$$B",
            "-l",
            "python",
            "--path",
            ".",
            "--json",
        ],
        &[("PATH", &path)],
    );
    assert_ne!(result.code, 0);
    assert!(
        result.stderr.contains("malformed JSON"),
        "{}",
        result.stderr
    );
    assert!(
        std::path::Path::new(&sentinel).exists(),
        "sg malformed fixture was not invoked"
    );
    assert!(
        result.elapsed.as_secs() < 5,
        "malformed sg was abandoned for {:?}",
        result.elapsed
    );

    for (case, backend, args) in [
        (
            "rg-large",
            "rg",
            vec!["grep", "helper", "--path", ".", "--json"],
        ),
        (
            "git-large",
            "git",
            vec!["grep", "helper", "--path", ".", "--at", "HEAD", "--json"],
        ),
        (
            "prefilter-large",
            "rg",
            vec![
                "pattern",
                "def $N($$$A): $$$B",
                "-l",
                "python",
                "--path",
                ".",
                "--json",
            ],
        ),
    ] {
        let path = backend_path(
            &f,
            case,
            backend,
            "#!/bin/sh\ni=0; while [ $i -lt 10000 ]; do printf x >&2; i=$((i+1)); done\nexit 8\n",
        );
        let result = f.trace_env(&args, &[("PATH", &path)]);
        assert_ne!(result.code, 0, "{case}");
        assert!(
            result
                .stderr
                .contains("stderr truncated at 4096 of 10000 bytes"),
            "{case}: {}",
            result.stderr
        );
    }
}

#[test]
fn pattern_validation_is_not_suppressed_by_an_empty_prefilter() {
    let f = Fixture::new();
    f.write("source.py", "value = 1\n");
    f.commit("no structural candidates");
    let arguments = f.root.join(".tracer-cache/sg-empty-arguments");
    fs::create_dir_all(arguments.parent().unwrap()).unwrap();
    let path = backend_path(
        &f,
        "empty-pattern-candidates",
        "sg",
        &format!(
            "#!/bin/sh\nprintf 'CALL\\0' >> '{}'\nprintf '%s\\0' \"$@\" >> '{}'\nPATH=${{PATH#*:}} exec sg \"$@\"\n",
            arguments.display(),
            arguments.display()
        ),
    );
    let invalid = f.trace_env(
        &[
            "pattern",
            "alpha beta",
            "-l",
            "python",
            "--path",
            ".",
            "--json",
        ],
        &[("PATH", &path)],
    );
    assert_ne!(invalid.code, 0, "invalid syntax must reach ast-grep");
    assert!(
        !invalid.stderr.trim().is_empty(),
        "invalid syntax diagnostic missing"
    );
    let valid = f.trace_env(
        &[
            "pattern",
            "def missing($$$A): $$$B",
            "-l",
            "python",
            "--path",
            ".",
            "--json",
        ],
        &[("PATH", &path)],
    );
    valid.ok();
    let document = valid.json();
    assert_eq!(document.as_object().unwrap().len(), 4, "{document}");
    for slot in ["query", "context", "results", "counts"] {
        assert!(document.get(slot).is_some(), "missing {slot}: {document}");
    }
    assert_eq!(document["counts"]["matches"], 0);
    assert_eq!(document["results"].as_array().unwrap().len(), 0);
    assert_eq!(document["context"]["files"].as_object().unwrap().len(), 0);
    assert_eq!(document["query"]["path"], ".");
    assert_eq!(document["context"]["repo"]["total_files"], 2);

    let arguments = fs::read(arguments).unwrap();
    let arguments: Vec<&[u8]> = arguments
        .split(|byte| *byte == 0)
        .filter(|argument| !argument.is_empty())
        .collect();
    assert_eq!(
        arguments
            .iter()
            .filter(|argument| **argument == b"CALL")
            .count(),
        2,
        "valid and invalid patterns must both reach ast-grep: {arguments:?}"
    );
    assert_eq!(
        arguments
            .iter()
            .filter(|argument| **argument == b"--stdin")
            .count(),
        2,
        "empty candidates must validate against empty stdin: {arguments:?}"
    );
    assert!(
        !arguments.iter().any(|argument| *argument == b"."),
        "empty candidates must not send the requested corpus to ast-grep: {arguments:?}"
    );
}

#[test]
fn missing_search_backend_is_a_spawn_failure() {
    let f = standard_repo();
    let empty = f.path("empty-path");
    fs::create_dir_all(&empty).unwrap();
    let result = f.trace_env(
        &["grep", "helper", "--path", ".", "--json"],
        &[("PATH", &empty)],
    );
    assert_ne!(result.code, 0);
    assert!(
        result.stderr.contains("failed to start ripgrep"),
        "{}",
        result.stderr
    );
}

#[test]
fn grep_keeps_valid_empty_and_leading_dash_patterns() {
    let f = Fixture::new();
    f.write("source.py", "dash = '-needle'\n");
    f.commit("leading dash search");
    let empty = f.trace(&["grep", "no-such-token", "--path", ".", "--json"]);
    empty.ok();
    let document = empty.json();
    assert_eq!(document.as_object().unwrap().len(), 4, "{document}");
    for slot in ["query", "context", "results", "counts"] {
        assert!(document.get(slot).is_some(), "missing {slot}: {document}");
    }
    let leading_dash = f.trace(&["grep", "--path", ".", "--json", "--", "-needle"]);
    leading_dash.ok();
    assert_eq!(
        leading_dash.json()["counts"]["matches"],
        1,
        "{}",
        leading_dash.stdout
    );
}

#[test]
fn grep_preserves_unicode_snippets_and_historical_scope() {
    let f = Fixture::new();
    let line = format!("{}needle{}", "é".repeat(200), "界".repeat(200));
    f.write("src/a:b.py", &format!("old_token = true\n{line}\n"));
    f.commit("old search state");
    f.write("src/a:b.py", "new_token = true\n");
    f.commit("new search state");

    let unicode = f.trace(&["grep", "needle", "--path", ".", "--at", "HEAD~1", "--json"]);
    unicode.ok();
    let result = &unicode.json()["results"][0];
    assert_eq!(
        result["file"], "src/a:b.py",
        "colon filename was split: {result}"
    );
    let snippet = result["snippet"].as_str().unwrap();
    assert!(
        snippet.contains("needle"),
        "Unicode byte offset lost match: {snippet}"
    );

    let old = f.trace(&[
        "grep",
        "old_token",
        "--path",
        ".",
        "--at",
        "HEAD~1",
        "--json",
    ]);
    old.ok();
    assert_eq!(old.json()["counts"]["matches"], 1);
    let current = f.trace(&["grep", "old_token", "--path", ".", "--json"]);
    current.ok();
    assert_eq!(current.json()["counts"]["matches"], 0);
}

#[test]
fn historical_grep_uses_machine_delimiters_for_path_and_source_text() {
    let f = Fixture::new();
    let file = "src/colon:\ttab\nnewline.py";
    f.write(
        file,
        "def helper():\n    return \"https://example.test:8443/path\"\n",
    );
    f.commit("delimiter-shaped path and source");

    for (pattern, expected) in [
        ("helper", "def helper():"),
        ("https://", "    return \"https://example.test:8443/path\""),
    ] {
        let result = f.trace(&["grep", pattern, "--path", ".", "--at", "HEAD", "--json"]);
        result.ok();
        let row = &result.json()["results"][0];
        assert_eq!(
            row["file"], file,
            "filename delimiters were parsed as fields: {row}"
        );
        assert_eq!(
            row["snippet"], expected,
            "source colon was parsed as a field: {row}"
        );
        assert_eq!(row["line"], if pattern == "helper" { 1 } else { 2 });
    }
}

#[test]
fn literal_free_pattern_reaches_ast_grep_without_a_candidate_scan() {
    let f = Fixture::new();
    f.write("source.py", "value = call(1)\n");
    f.commit("literal-free pattern");
    let path = backend_path(
        &f,
        "literal-free-rg",
        "rg",
        "#!/bin/sh\necho candidate-scan-was-not-skipped >&2\nexit 9\n",
    );
    let result = f.trace_env(
        &["pattern", "$X", "-l", "python", "--path", ".", "--json"],
        &[("PATH", &path)],
    );
    result.ok();
    assert!(
        result.json()["counts"]["matches"].as_u64().unwrap() > 0,
        "{}",
        result.stdout
    );
}

#[test]
fn pattern_delegates_every_literal_filter_to_ripgrep_before_ast_grep() {
    let f = Fixture::new();
    let unusual = "src/-leading\n界.py";
    let ordinary = "src/ordinary.py";
    f.write(unusual, "result = dispatch(marker(value), finish(value))\n");
    f.write(
        "src/missing-finish.py",
        "result = dispatch(marker(value), value)\n",
    );
    f.write(
        ordinary,
        "result = dispatch(marker(value), finish(value))\n",
    );
    f.write("src/dispatch-only.py", "result = dispatch(value)\n");
    f.commit("literal intersections");

    let backend_directory = f.path("native-filter-backends");
    fs::create_dir_all(&backend_directory).unwrap();
    let rg_arguments = f.path("rg-native-filter-arguments");
    let sg_arguments = f.path("sg-native-filter-arguments");
    let rg = f.write(
        "native-filter-backends/rg",
        &format!(
            "#!/bin/sh\nprintf 'CALL\\0' >> '{rg_arguments}'\nprintf '%s\\0' \"$@\" >> '{rg_arguments}'\ncase \" $* \" in\n  *' dispatch '*) printf '%s\\0' '{unusual}' 'src/missing-finish.py' '{ordinary}' 'src/dispatch-only.py' ;;\n  *' marker '*) printf '%s\\0' '{ordinary}' '{unusual}' 'src/missing-finish.py' ;;\n  *' finish '*) printf '%s\\0' '{ordinary}' '{unusual}' ;;\nesac\n"
        ),
    );
    fs::set_permissions(rg, fs::Permissions::from_mode(0o755)).unwrap();
    let sg = f.write(
        "native-filter-backends/sg",
        &format!(
            "#!/bin/sh\nprintf '%s\\0' \"$@\" >> '{sg_arguments}'\nPATH=${{PATH#*:}} exec sg \"$@\"\n"
        ),
    );
    fs::set_permissions(sg, fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!(
        "{}:{}",
        backend_directory,
        std::env::var("PATH").expect("test runner PATH")
    );

    let result = f.trace_env(
        &[
            "pattern",
            "dispatch(marker($X), finish($Y))",
            "-l",
            "python",
            "--path",
            ".",
            "--json",
        ],
        &[("PATH", &path)],
    );
    result.ok();
    let document = result.json();
    assert_eq!(document.as_object().unwrap().len(), 4, "{document}");
    for slot in ["query", "context", "results", "counts"] {
        assert!(document.get(slot).is_some(), "missing {slot}: {document}");
    }
    assert_eq!(
        document["query"]["pattern"],
        "dispatch(marker($X), finish($Y))"
    );
    assert_eq!(document["query"]["lang"], "python");
    assert_eq!(document["query"]["path"], ".");
    assert_eq!(document["counts"]["matches"], 2, "{}", result.stdout);
    assert_eq!(document["counts"]["files"], 2, "{}", result.stdout);
    let mut rows: Vec<(&str, i64, &str)> = document["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            (
                row["file"].as_str().unwrap(),
                row["line"].as_i64().unwrap(),
                row["snippet"].as_str().unwrap(),
            )
        })
        .collect();
    rows.sort();
    assert_eq!(
        rows,
        vec![
            (unusual, 1, "dispatch(marker(value), finish(value))"),
            (ordinary, 1, "dispatch(marker(value), finish(value))"),
        ]
    );
    let mut context_files: Vec<&str> = document["context"]["files"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    context_files.sort();
    let mut expected_context = vec![unusual, ordinary];
    expected_context.sort();
    assert_eq!(context_files, expected_context);

    let rg_arguments = fs::read(rg_arguments).unwrap();
    let rg_arguments: Vec<&[u8]> = rg_arguments
        .split(|byte| *byte == 0)
        .filter(|argument| !argument.is_empty())
        .collect();
    let rg_calls: Vec<Vec<&[u8]>> = rg_arguments
        .split(|argument| *argument == b"CALL")
        .skip(1)
        .map(|call| call.to_vec())
        .collect();
    assert_eq!(
        rg_calls.len(),
        3,
        "the broad scan and both remaining literals must run in ripgrep: {rg_arguments:?}"
    );
    let marker_candidates: Vec<&[u8]> = rg_calls[1]
        .iter()
        .copied()
        .filter(|argument| argument.ends_with(b".py"))
        .collect();
    assert_eq!(
        marker_candidates,
        vec![
            unusual.as_bytes(),
            b"src/missing-finish.py",
            ordinary.as_bytes(),
            b"src/dispatch-only.py",
        ],
        "the second literal must receive every broad candidate in original order"
    );
    let finish_candidates: Vec<&[u8]> = rg_calls[2]
        .iter()
        .copied()
        .filter(|argument| argument.ends_with(b".py"))
        .collect();
    assert_eq!(
        finish_candidates,
        vec![
            unusual.as_bytes(),
            b"src/missing-finish.py",
            ordinary.as_bytes(),
        ],
        "the third literal must receive the prior intersection in broad-candidate order"
    );

    let sg_arguments = fs::read(sg_arguments).unwrap();
    let sg_candidates: Vec<&[u8]> = sg_arguments
        .split(|byte| *byte == 0)
        .filter(|argument| argument.ends_with(b".py"))
        .collect();
    assert_eq!(
        sg_candidates,
        vec![unusual.as_bytes(), ordinary.as_bytes()],
        "ast-grep must receive the final intersection in broad-candidate order"
    );
}

#[test]
fn pattern_multi_literal_prefilter_keeps_exact_real_matches() {
    let f = standard_repo();
    let result = f.trace(&[
        "pattern",
        "if $CONDITION:\n    return $VALUE",
        "-l",
        "python",
        "--path",
        ".",
        "--json",
    ]);
    result.ok();
    let document = result.json();
    let mut rows: Vec<(&str, i64, &str)> = document["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            (
                row["file"].as_str().unwrap(),
                row["line"].as_i64().unwrap(),
                row["snippet"].as_str().unwrap(),
            )
        })
        .collect();
    rows.sort();
    assert_eq!(
        rows,
        vec![("src/app.py", 7, "if x:"), ("src/util.py", 2, "if v > 0:"),],
        "real ripgrep and ast-grep must preserve the exact structural answer"
    );
    assert_eq!(document["counts"]["matches"], 2);
    assert_eq!(document["counts"]["files"], 2);
    assert_eq!(document["context"]["files"].as_object().unwrap().len(), 2);
}

#[test]
fn pattern_splits_only_after_the_platform_refuses_the_full_argument_list() {
    let f = Fixture::new();
    for i in 0..4500 {
        f.write(
            &format!("src/{}-{i:04}/module.py", "wide".repeat(53)),
            "needle = tail\n",
        );
    }
    f.commit("argument-list boundary");

    let rg_arguments = f.path("rg-capacity-arguments");
    let sg_arguments = f.path("sg-capacity-arguments");
    let backend_directory = f.path("capacity-backends");
    fs::create_dir_all(&backend_directory).unwrap();
    let rg = f.write(
        "capacity-backends/rg",
        &format!(
            "#!/bin/sh\nprintf 'CALL\\0' >> '{rg_arguments}'\nprintf '%s\\0' \"$@\" >> '{rg_arguments}'\nPATH=${{PATH#*:}} exec rg \"$@\"\n"
        ),
    );
    fs::set_permissions(rg, fs::Permissions::from_mode(0o755)).unwrap();
    let sg = f.write(
        "capacity-backends/sg",
        &format!(
            "#!/bin/sh\nprintf '%s\\0' \"$@\" >> '{}'\nexit 1\n",
            sg_arguments
        ),
    );
    fs::set_permissions(sg, fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!(
        "{}:{}",
        backend_directory,
        std::env::var("PATH").expect("test runner PATH")
    );

    let result = f.trace_env(
        &[
            "pattern",
            "needle + tail",
            "-l",
            "python",
            "--path",
            ".",
            "--json",
        ],
        &[("PATH", &path)],
    );
    result.ok();
    let document = result.json();
    assert_eq!(document["counts"]["matches"], 0, "{}", result.stdout);
    assert_eq!(document["counts"]["files"], 0, "{}", result.stdout);
    assert_eq!(document["context"]["files"].as_object().unwrap().len(), 0);

    let rg_arguments = fs::read(rg_arguments).unwrap();
    let rg_arguments: Vec<&[u8]> = rg_arguments
        .split(|byte| *byte == 0)
        .filter(|argument| !argument.is_empty())
        .collect();
    assert!(
        rg_arguments
            .iter()
            .filter(|argument| **argument == b"CALL")
            .count()
            > 2,
        "the remaining-literal argv must split only after the real full spawn is refused"
    );
    let rg_candidate_arguments: Vec<&[u8]> = rg_arguments
        .iter()
        .copied()
        .filter(|argument| argument.ends_with(b"/module.py"))
        .collect();
    assert_eq!(
        rg_candidate_arguments.len(),
        4500,
        "each broad candidate must be searched exactly once across split ripgrep batches"
    );
    let rg_candidates: std::collections::HashSet<&[u8]> =
        rg_candidate_arguments.iter().copied().collect();
    assert_eq!(
        rg_candidates.len(),
        4500,
        "every broad candidate must survive the remaining-literal ripgrep phase"
    );

    let sg_arguments = fs::read(sg_arguments).unwrap();
    let sg_candidate_arguments: Vec<&[u8]> = sg_arguments
        .split(|byte| *byte == 0)
        .filter(|argument| argument.ends_with(b"/module.py"))
        .collect();
    assert_eq!(
        sg_candidate_arguments.len(),
        4500,
        "each surviving candidate must reach ast-grep exactly once across split batches"
    );
    let sg_candidates: std::collections::HashSet<&[u8]> =
        sg_candidate_arguments.iter().copied().collect();
    assert_eq!(
        sg_candidates.len(),
        4500,
        "every candidate reached ast-grep once"
    );
}

#[test]
fn pattern_ast_search_python() {
    let f = standard_repo();
    let r = f.trace(&[
        "pattern",
        "def $NAME($$$ARGS): $$$BODY",
        "-l",
        "python",
        "--path",
        ".",
        "--json",
    ]);
    r.ok();
    let v = r.view();
    assert_eq!(v["lang"], "python");
    // standard_repo() has exactly two python function defs matching
    // `def $NAME($$$ARGS): $$$BODY`: main() at src/app.py L5 and
    // helper() at src/util.py L1. Both file and count are exact, and the
    // per-match complexity enrichment must equal each file's real CCN.
    assert_eq!(v["matches"].as_i64().unwrap(), 2, "two python defs: {}", v);
    assert_eq!(
        v["files"].as_object().unwrap().len(),
        2,
        "in two files: {}",
        v
    );
    let mut hits: Vec<(&str, i64, i64, &str)> = v["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| {
            let file = m["file"].as_str().unwrap();
            (
                file,
                m["line"].as_i64().unwrap(),
                v["files"][file]["file_complexity"]["ccn_total"]
                    .as_i64()
                    .unwrap(),
                v["files"][file]["file_complexity"]["rank"]
                    .as_str()
                    .unwrap(),
            )
        })
        .collect();
    hits.sort();
    assert_eq!(
        hits,
        vec![("src/app.py", 5, 4, "low"), ("src/util.py", 1, 2, "low"),],
        "struct match set (file, line, ccn_total, rank) must be exact: {}",
        v
    );
}

#[test]
fn pattern_requires_lang() {
    let f = standard_repo();
    let r = f.trace(&["pattern", "def $X(): $$$B", "--path", "."]);
    // click marks -l/--lang required → usage error, exit 2.
    r.code_is(2);
}

#[test]
fn find_matches_basename_pattern() {
    let f = standard_repo();
    let r = f.trace(&["find", "*.py", ".", "--json"]);
    r.ok();
    let v = r.view();
    let paths: Vec<&str> = v["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["path"].as_str().unwrap())
        .collect();
    // standard_repo() contains exactly two .py files; the basename match
    // set is exactly those two, deterministically sorted.
    let mut sorted = paths.clone();
    sorted.sort();
    assert_eq!(
        sorted,
        vec!["src/app.py", "src/util.py"],
        "find *.py must return exactly the two fixture .py files: {:?}",
        paths
    );
}

/// `find` must prune a vendored/ignored directory the same way the
/// full-path glob is proven to (`glob_gitignored_paths_excluded`). A
/// gitignored `node_modules/` containing a matching basename must NOT
/// appear; a tracked sibling with the same basename must.
#[test]
fn find_prunes_vendored_ignored_directory() {
    let f = Fixture::new();
    f.write("src/keep.py", "def kept():\n    return 1\n");
    f.write(
        "node_modules/pkg/keep.py",
        "def vendored():\n    return 2\n",
    );
    f.write("vendor/lib/keep.py", "def vendored2():\n    return 3\n");
    f.write(".gitignore", "node_modules/\nvendor/\n");
    f.commit("tracked source plus ignored vendor trees");

    let r = f.trace(&["find", "keep.py", ".", "--json"]);
    r.ok();
    let v = r.view();
    let paths: Vec<&str> = v["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["path"].as_str().unwrap())
        .collect();
    // Only the tracked src/keep.py survives: the node_modules/ and
    // vendor/ copies are gitignored and pruned, so the match set is
    // exactly the one tracked path.
    assert_eq!(
        paths,
        vec!["src/keep.py"],
        "find must return exactly the tracked src/keep.py, pruning ignored trees: {paths:?}"
    );
}

#[test]
fn find_path_filter_and_exclude() {
    let f = standard_repo();
    let r = f.trace(&[
        "find",
        "*.py",
        ".",
        "--path",
        "*/src/*",
        "--exclude",
        "*util*",
        "--json",
    ]);
    r.ok();
    let v = r.view();
    let paths: Vec<&str> = v["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["path"].as_str().unwrap())
        .collect();
    // standard_repo()'s src/ has four files; *.py narrows to app.py +
    // util.py, --path */src/* keeps both, --exclude *util* drops util.py:
    // exactly src/app.py remains.
    assert_eq!(
        paths,
        vec!["src/app.py"],
        "find *.py under */src/* excluding *util* must be exactly src/app.py: {:?}",
        paths
    );
}

#[test]
fn find_type_directory() {
    let f = standard_repo();
    let r = f.trace(&["find", "src", ".", "--type", "d", "--json"]);
    r.ok();
    let v = r.view();
    let entries: Vec<serde_json::Value> = v["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| serde_json::json!({"path": e["path"], "kind": e["kind"]}))
        .collect();
    // The only directory named `src` in standard_repo() is the top-level
    // src/; --type d returns exactly that one entry.
    assert_eq!(
        serde_json::Value::Array(entries),
        serde_json::json!([{"path": "src", "kind": "directory"}]),
        "find src --type d must return exactly the one src directory: {}",
        v["results"]
    );
}

#[test]
fn find_missing_base_exits_2() {
    let f = standard_repo();
    let r = f.trace(&["find", "*.py", f.path("nope-dir").as_str()]);
    r.code_is(2);
    assert!(r.combined().contains("does not exist"), "{}", r.combined());
}

#[test]
fn find_path_pattern_recurses_on_double_star() {
    let f = standard_repo();
    let r = f.trace(&["find", "**/*.py", ".", "--json"]);
    r.ok();
    let v = r.view();
    let mut got: Vec<String> = v["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["path"].as_str().unwrap().to_string())
        .collect();
    got.sort();
    assert_eq!(
        got,
        vec!["src/app.py".to_string(), "src/util.py".to_string()]
    );
}

#[test]
fn find_path_pattern_excludes_gitignored_paths() {
    let f = Fixture::new();
    f.write("keep.py", "pass\n");
    f.write("node_modules/skip.py", "pass\n");
    f.write(".gitignore", "node_modules/\n");
    f.commit("with gitignore");
    let r = f.trace(&["find", "**/*.py", ".", "--json"]);
    r.ok();
    let v = r.view();
    let matches: Vec<&str> = v["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["path"].as_str().unwrap())
        .collect();
    // keep.py is the only non-ignored .py file; node_modules/skip.py is
    // gitignored. The match set is exactly [keep.py].
    assert_eq!(
        matches,
        vec!["keep.py"],
        "find must return exactly keep.py, excluding the gitignored copy: {:?}",
        matches
    );
}

#[test]
fn find_path_pattern_matches_a_directory_segment() {
    // The question a path pattern answers and a basename pattern cannot:
    // "the .py files under src/", where the directory is part of the match.
    let f = standard_repo();
    let r = f.trace(&["find", "src/*.py", ".", "--json"]);
    r.ok();
    let v = r.view();
    let mut got: Vec<String> = v["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["path"].as_str().unwrap().to_string())
        .collect();
    got.sort();
    assert_eq!(
        got,
        vec!["src/app.py".to_string(), "src/util.py".to_string()]
    );
    // The same pattern shape must not match a file of that name elsewhere:
    // `lib/*.php` is a different directory and returns its own file only.
    let r = f.trace(&["find", "lib/*.php", ".", "--json"]);
    r.ok();
    let got: Vec<String> = r.view()["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["path"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(got, vec!["lib/widget.php".to_string()]);
}

#[test]
fn find_path_pattern_rows_carry_ccn_and_shoulder() {
    // Every row carries context — there is no bare-path mode to fall into.
    // `**/*.py` over standard_repo is exactly src/app.py and src/util.py,
    // deterministically sorted, and each carries that file's real CCN and
    // rank: app.py main() = 4 (low), util.py helper() = 2 (low).
    let f = standard_repo();
    let r = f.trace(&["find", "**/*.py", ".", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(v["matches"].as_i64().unwrap(), 2, "two .py files: {}", v);
    let rows: Vec<(&str, i64, &str)> = v["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| {
            (
                m["path"].as_str().unwrap(),
                m["ccn_total"].as_i64().unwrap(),
                m["ccn_rank"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        rows,
        vec![("src/app.py", 4, "low"), ("src/util.py", 2, "low")],
        "find rows (path, ccn_total, ccn_rank) must be exact and sorted: {}",
        v
    );
    for m in v["results"].as_array().unwrap() {
        let path = m["path"].as_str().unwrap();
        assert!(
            !v["files"][path]["shoulder"]
                .as_str()
                .unwrap_or("")
                .is_empty(),
            "every row must reach a non-empty lifecycle shoulder: {m}"
        );
    }
    // Human form is `<path>  [ccn=<n> <rank>] <shoulder>`; the shoulder
    // segment after the ccn bracket must be present and non-empty.
    let h = f.trace(&["find", "**/*.py", "."]);
    h.ok();
    let line = h
        .stdout
        .lines()
        .find(|l| l.contains("src/util.py"))
        .expect("util.py must appear in the human output");
    let after = line.split("] ").nth(1).unwrap_or("").trim();
    assert!(
        line.contains("[ccn=2 low]") && !after.is_empty(),
        "human line must carry the ccn bracket and a non-empty shoulder: {line:?}"
    );
}

#[test]
fn find_base_that_is_file_exits_2() {
    let f = standard_repo();
    let r = f.trace(&["find", "**/*.py", f.path("src/app.py").as_str()]);
    r.code_is(2);
    assert!(r.combined().contains("not a directory"), "{}", r.combined());
}

#[test]
fn find_results_are_deterministically_sorted() {
    let f = standard_repo();
    let a = f.trace(&["find", "**/*", "."]);
    a.ok();
    let b = f.trace(&["find", "**/*", "."]);
    b.ok();
    assert_eq!(a.stdout, b.stdout, "find output is not deterministic");
}

#[test]
fn find_resolves_every_row_across_fact_chunks() {
    let f = Fixture::new();
    for index in 0..513 {
        f.write(
            &format!("src/file_{index:03}.py"),
            &format!("def value_{index}():\n    return {index}\n"),
        );
    }
    f.commit("more files than one fact chunk");

    let r = f.trace(&[
        "find",
        "**/*.py",
        ".",
        "--limit",
        "513",
        "--sort",
        "complexity",
        "--json",
    ]);
    r.ok();
    let v = r.view();
    let rows = v["results"].as_array().unwrap();
    assert_eq!(v["total"], 513);
    assert_eq!(v["matches"], 513);
    assert_eq!(v["truncated"], false);
    assert_eq!(rows.first().unwrap()["path"], "src/file_000.py");
    assert_eq!(rows.last().unwrap()["path"], "src/file_512.py");
    for row in rows {
        let path = row["path"].as_str().unwrap();
        assert_eq!(row["ccn_total"], 1, "wrong complexity for {path}");
        assert_eq!(row["ccn_rank"], "low", "wrong rank for {path}");
        assert!(
            v["files"][path]["shoulder"]
                .as_str()
                .is_some_and(|shoulder| !shoulder.is_empty()),
            "missing passive context for {path}"
        );
    }
}

/// A snippet stays a snippet on a minified single-line file: past the cap
/// the rendered match is a character window around the submatch offset
/// `rg --json` reports, ellipsized on the cut side(s) — never the whole
/// line. Guards the 27KB-per-match failure on vendored bundles.
#[test]
fn grep_snippet_windows_long_minified_line() {
    let f = Fixture::new();
    let line = format!("{}needle_token_here{}", "x".repeat(3000), "y".repeat(3000));
    f.write("bundle.js", &format!("{line}\n"));
    f.commit("minified bundle");
    let r = f.trace(&["grep", "needle_token_here", "--path", ".", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(v["matches"].as_i64().unwrap(), 1, "{v}");
    let snippet = v["results"][0]["snippet"].as_str().unwrap();
    assert!(
        snippet.contains("needle_token_here"),
        "window must contain the match: {snippet}"
    );
    // 240-char window plus one ellipsis per cut side.
    assert_eq!(
        snippet.chars().count(),
        242,
        "mid-line match is the full window with both ellipses: {snippet}"
    );
    assert!(
        snippet.starts_with('\u{2026}') && snippet.ends_with('\u{2026}'),
        "both cut sides carry an ellipsis: {snippet}"
    );
}

/// The window applies only past the cap — an ordinary source line renders
/// whole, exactly as before.
#[test]
fn grep_snippet_keeps_short_lines_whole() {
    let f = standard_repo();
    let r = f.trace(&["grep", "helper", "--path", ".", "--json"]);
    r.ok();
    let v = r.view();
    let m = v["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["file"].as_str().unwrap().ends_with("src/util.py"))
        .expect("grep must hit src/util.py");
    assert_eq!(
        m["snippet"].as_str().unwrap(),
        "def helper(v):",
        "short line must render whole: {m}"
    );
}

/// A vendored checkout under the base is its own search scope. An empty
/// result must name it — "(no matches)" alone reads as "the code does not
/// exist" when it actually sits one scope down.
#[test]
fn find_empty_result_names_nested_repos() {
    let f = standard_repo();
    f.write(".gitignore", "themes/\n");
    f.commit("ignore themes");
    f.git(&["init", "--quiet", "themes/vendortheme"]);
    f.write("themes/vendortheme/bundle.min.js", "vendored_token()\n");
    f.git(&["-C", "themes/vendortheme", "add", "-A"]);
    f.git(&[
        "-C",
        "themes/vendortheme",
        "commit",
        "--quiet",
        "-m",
        "vendor",
    ]);

    let r = f.trace(&["find", "*.min.js", ".", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(v["matches"].as_i64().unwrap(), 0, "{v}");
    let nested: Vec<&str> = v["nested_repos"]
        .as_array()
        .expect("empty result over a base with a nested checkout carries nested_repos")
        .iter()
        .map(|x| x.as_str().unwrap())
        .collect();
    assert_eq!(nested, vec!["themes/vendortheme"], "{v}");

    // Scoped inside the nested repo, the same pattern matches.
    let scoped = f.trace(&["find", "*.min.js", "themes/vendortheme", "--json"]);
    scoped.ok();
    assert_eq!(
        scoped.json()["counts"]["matches"].as_i64().unwrap(),
        1,
        "{}",
        scoped.stdout
    );

    let h = f.trace(&["find", "*.min.js", "."]);
    h.ok();
    assert!(
        h.stdout.contains("(no matches)")
            && h.stdout
                .contains("nested repository (its own search scope): themes/vendortheme"),
        "{}",
        h.stdout
    );
}

/// Same scope fact on the text-search path: rg respects the outer
/// gitignore, so a vendored checkout's content is unreachable from above —
/// the empty result names the checkout.
#[test]
fn grep_empty_result_names_nested_repos() {
    let f = standard_repo();
    f.write(".gitignore", "themes/\n");
    f.commit("ignore themes");
    f.git(&["init", "--quiet", "themes/vendortheme"]);
    f.write("themes/vendortheme/bundle.min.js", "vendored_token()\n");
    f.git(&["-C", "themes/vendortheme", "add", "-A"]);
    f.git(&[
        "-C",
        "themes/vendortheme",
        "commit",
        "--quiet",
        "-m",
        "vendor",
    ]);

    let r = f.trace(&["grep", "vendored_token", "--path", ".", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(v["matches"].as_i64().unwrap(), 0, "{v}");
    let nested: Vec<&str> = v["nested_repos"]
        .as_array()
        .expect("empty grep over a base with a nested checkout carries nested_repos")
        .iter()
        .map(|x| x.as_str().unwrap())
        .collect();
    assert_eq!(nested, vec!["themes/vendortheme"], "{v}");

    // Scoped inside the nested repo, the same pattern matches.
    let scoped = f.trace(&[
        "grep",
        "vendored_token",
        "--path",
        f.path("themes/vendortheme").as_str(),
        "--json",
    ]);
    scoped.ok();
    assert_eq!(
        scoped.json()["counts"]["matches"].as_i64().unwrap(),
        1,
        "{}",
        scoped.stdout
    );
}
