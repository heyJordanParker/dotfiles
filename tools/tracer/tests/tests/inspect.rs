//! The promise: the numbers about a file are exact, and an input that is not
//! ordinary source still gets an answer.
//!
//! `stats`, `info`, `list`, `read` and `logs` are what an agent asks when it
//! wants facts about files rather than relationships between them. Every
//! count here is asserted as an exact value on a fixture whose right answer
//! is known by construction — a presence-only check passes even when every
//! number is wrong. The degenerate inputs (no repo, empty directory, binary
//! bytes, prose, a 3,000-function file) are here because each one used to
//! crash or return a wrong number.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;
use tracer_cli_tests::{standard_repo, trace, trace_env, Fixture};

fn backend_path(fixture: &Fixture, case: &str, name: &str, script: &str) -> String {
    let directory = format!(".tracer-cache/info-context-backends/{case}");
    let path = fixture.write(&format!("{directory}/{name}"), script);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    format!(
        "{}:{}",
        fixture.path(&directory),
        std::env::var("PATH").expect("test runner PATH")
    )
}

#[test]
fn info_context_observes_snapshot_inputs_without_duplicate_work() {
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

    for workers in ["1", "4"] {
        let fixture = standard_repo();
        let markers = fixture.root.join(".tracer-cache/info-context-overlap");
        fs::create_dir_all(&markers).unwrap();
        let status_started = markers.join("status-started");
        let scc_started = markers.join("scc-started");
        let status_saw_scc = markers.join("status-saw-scc");
        let scc_saw_status = markers.join("scc-saw-status");
        let status_count = markers.join("status-count");
        let scc_count = markers.join("scc-count");
        let case = format!("info-context-overlap-{workers}");
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
            &["info", "src/app.py", "--json"],
            &[("PATH", path.as_str()), ("RAYON_NUM_THREADS", workers)],
        );
        result.ok();
        let document = result.json();
        let file = fs::canonicalize(fixture.path("src/app.py"))
            .unwrap()
            .to_string_lossy()
            .to_string();
        assert_eq!(document.as_object().unwrap().len(), 4, "{document:#}");
        assert_eq!(document["query"], serde_json::json!({"file": file}));
        assert_eq!(
            document["results"],
            serde_json::json!([{
                "name": "main",
                "cyclomatic_complexity": 4,
                "nloc": 8,
                "start_line": 5,
                "end_line": 12,
            }])
        );
        assert_eq!(
            document["counts"],
            serde_json::json!({
                "functions": 1,
                "loc": 8,
                "ccn_total": 4,
                "ccn_max_function": 4,
                "rank": "low",
            })
        );
        assert_eq!(
            document["context"]["repo"],
            serde_json::json!({
                "total_files": 7,
                "median_file_ccn": 0,
                "complexity_p95": 1,
            })
        );
        let file_context = &document["context"]["files"][&file];
        assert_eq!(document["context"].as_object().unwrap().len(), 2);
        assert_eq!(document["context"]["files"].as_object().unwrap().len(), 1);
        assert_eq!(file_context.as_object().unwrap().len(), 6);
        assert_eq!(file_context["language"], "python");
        assert!(file_context["nearest_doc"].is_null());
        assert!(file_context["leading_comment"].is_null());
        assert_eq!(file_context["top_callers"], serde_json::json!([]));
        assert_eq!(
            file_context["dependencies"],
            serde_json::json!([
                {"module": "src.util", "confidence": "EXTRACTED"},
            ])
        );
        assert!(file_context["shoulder"]
            .as_str()
            .unwrap()
            .contains("loc: 8 · ccn: 4 low"));
        assert_eq!(fs::read(&status_count).unwrap(), b"x", "workers={workers}");
        assert_eq!(fs::read(&scc_count).unwrap(), b"x", "workers={workers}");

        if workers != "1" {
            assert!(
                scc_saw_status.exists(),
                "workers={workers}: scc started before its repository input scan"
            );
        }
    }
}

#[test]
fn path_outside_any_repo_still_works() {
    // A directory that is NOT inside a git repo. tracer must degrade
    // gracefully (no git history) rather than crash.
    let tmp = std::env::temp_dir().join(format!(
        "trace-norepo-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let nested = tmp.join("source");
    fs::create_dir_all(&nested).unwrap();
    fs::write(
        nested.join("lone.py"),
        "def choose(value):\n    if value:\n        return 1\n    return 0\n",
    )
    .unwrap();

    let survey = trace(&tmp, ["stats", ".", "--json"]);
    survey.ok();
    let v = survey.view();
    // No git, scc still classifies the one lone.py: exactly one Python
    // file, 4 loc, 1 file-level complexity. (total_files and distribution
    // are likewise fixed; top_complex carries an absolute temp path that
    // varies per run, so only the deterministic facets are pinned.)
    assert_eq!(
        v["files"].as_i64().unwrap(),
        1,
        "survey outside a repo must count exactly the one file: {v}"
    );
    assert_eq!(
        v["languages"],
        serde_json::json!({"Python": {"files": 1, "loc": 4, "complexity": 1}}),
        "survey languages outside a repo must be exact: {}",
        v["languages"]
    );
    assert_eq!(
        v["distribution"],
        serde_json::json!({"median": 1, "p75": 1, "p90": 1, "p95": 1, "max": 1}),
        "survey distribution outside a repo must preserve source complexity: {}",
        v["distribution"]
    );

    let info = trace(
        &tmp,
        ["info", &nested.join("lone.py").to_string_lossy(), "--json"],
    );
    info.ok();
    let iv = info.view();
    // `choose` has one branch: exactly one function, four lines, CCN 2.
    assert_eq!(
        iv["functions"].as_i64().unwrap(),
        1,
        "info failed to analyze a file outside any repo"
    );
    assert_eq!(
        iv["ccn_total"].as_i64().unwrap(),
        2,
        "choose() must preserve its base and branch CCN outside a repo"
    );
    assert_eq!(iv["ccn_max_function"].as_i64().unwrap(), 2);
    assert_eq!(iv["loc"].as_i64().unwrap(), 4);

    let directory = trace(&tmp, ["info", &nested.to_string_lossy()]);
    directory.ok();
    assert!(
        directory.stdout.contains("lone.py") && directory.stdout.contains("Top fns: choose() L1"),
        "human directory digest outside a repository lost its file facts: {}",
        directory.stdout
    );

    assert!(
        !tmp.join(".tracer-cache").exists() && !nested.join(".tracer-cache").exists(),
        "inspection outside a repo must not create a cache anywhere in the fixture"
    );

    fs::remove_dir_all(&tmp).ok();
}

/// Survey numbers are asserted exactly on a fully controlled fixture, not
/// merely "the fields exist". Three Python files with known scc complexity:
/// two `if`-bearing (scc complexity 1 each) and one branchless (0). A
/// presence-only check would pass even if every number were wrong.
#[test]
fn stats_reports_exact_numbers_on_known_fixture() {
    let f = Fixture::new();
    f.write(
        "src/a.py",
        "def a(x):\n    if x:\n        return 1\n    return 0\n",
    );
    f.write(
        "src/b.py",
        "def b(x):\n    if x:\n        return 1\n    return 0\n",
    );
    f.write("src/c.py", "def c():\n    return 1\n");
    f.commit("three known python files");

    let r = f.trace(&["stats", ".", "--json"]);
    r.ok();
    let v = r.view();

    assert_eq!(
        v["files"].as_i64().unwrap(),
        3,
        "exactly three source files: {}",
        r.stdout
    );
    let py = &v["languages"]["Python"];
    assert_eq!(py["files"].as_i64().unwrap(), 3, "3 Python files: {py}");
    assert_eq!(
        py["loc"].as_i64().unwrap(),
        10,
        "scc Python LOC must be exactly 10 (4+4+2): {py}"
    );
    assert_eq!(
        py["complexity"].as_i64().unwrap(),
        2,
        "scc total complexity must be exactly 2 (1 per `if`, two ifs): {py}"
    );

    let dist = &v["distribution"];
    assert_eq!(
        dist["max"].as_i64().unwrap(),
        1,
        "max per-file ccn is 1: {dist}"
    );
    assert_eq!(
        dist["median"].as_i64().unwrap(),
        1,
        "median of [0,1,1] (sorted) is 1: {dist}"
    );

    // The exact (path, complexity) set of the per-file rows — order among
    // equal-complexity rows is scc-emission order (not contract), so assert
    // the set, not positions.
    let mut got: Vec<(String, i64)> = v["top_complex"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            let p = e["path"].as_str().unwrap();
            let name = p.rsplit('/').next().unwrap().to_string();
            (name, e["complexity"].as_i64().unwrap())
        })
        .collect();
    got.sort();
    assert_eq!(
        got,
        vec![
            ("a.py".to_string(), 1),
            ("b.py".to_string(), 1),
            ("c.py".to_string(), 0),
        ],
        "per-file complexity rows wrong: {}",
        r.stdout
    );
}

#[test]
fn empty_directory_info_does_not_crash() {
    let f = Fixture::new();
    f.write("seed.py", "pass\n");
    f.commit("seed");
    // Git cannot track an empty dir, so create it post-commit; it physically
    // exists for click's exists=True check.
    fs::create_dir_all(f.root.join("hollow")).unwrap();
    let r = f.trace(&["info", "hollow", "--json"]);
    r.ok();
    assert_eq!(
        r.json()["counts"]["files"].as_i64().unwrap(),
        0,
        "empty dir should report zero files: {}",
        r.stdout
    );
}

#[test]
fn directory_info_preserves_rows_across_fact_batches() {
    let f = Fixture::new();
    for index in 0..513 {
        f.write(
            &format!("scope/nested/file-{index:03}.py"),
            &format!(
                "def function_{index}(value):\n    if value:\n        return 1\n    return 0\n"
            ),
        );
    }
    f.write("outside.py", "def outside():\n    return 0\n");
    f.commit("files across two fact batches");

    let tracked = f.trace(&["info", "scope", "--json"]);
    tracked.ok();
    let tracked_json = tracked.json();
    assert_eq!(tracked_json["counts"]["files"], 513);
    assert_eq!(tracked_json["counts"]["ccn_total"], 1026);
    assert_eq!(tracked_json["counts"]["loc"], 2052);
    let tracked_rows = tracked_json["results"].as_array().unwrap();
    assert_eq!(tracked_rows.len(), 513);
    for (index, row) in tracked_rows.iter().enumerate() {
        let relative = format!("nested/file-{index:03}.py");
        assert_eq!(row["file"], relative);
        assert_eq!(
            row["abs_path"].as_str().unwrap(),
            fs::canonicalize(f.root.join("scope").join(&relative))
                .unwrap()
                .to_string_lossy()
        );
        assert_eq!(row["loc"], 4);
        assert_eq!(row["cyclomatic_complexity_total"], 2);
        assert_eq!(row["function_count"], 1);
        assert_eq!(row["rank"], "low");
        assert!(tracked_json["context"]["files"][&relative]["shoulder"]
            .as_str()
            .unwrap()
            .contains("loc: 4 · ccn: 2 low"));
    }
}

#[test]
fn directory_human_digest_reuses_selected_file_facts_and_root() {
    let f = Fixture::new();
    f.write(".gitignore", ".tracer-cache/\n");
    for index in 0..21 {
        let mut source = format!("# Purpose {index}\ndef hot_{index}(value):\n");
        for branch in 0..=index {
            source.push_str(&format!(
                "    if value == {branch}:\n        return {branch}\n"
            ));
        }
        source.push_str("    return -1\n");
        f.write(&format!("scope/file-{index:03}.py"), &source);
    }
    f.commit("human directory digest fixture");

    let trace_events = f.root.join(".tracer-cache/git-trace.json");
    fs::create_dir_all(trace_events.parent().unwrap()).unwrap();
    let trace_events_text = trace_events.to_string_lossy().to_string();
    let run = trace_env(
        &f.root,
        ["info", "scope"],
        &[("GIT_TRACE2_EVENT", trace_events_text.as_str())],
    );
    run.ok();

    let selected: Vec<usize> = (1..=20).rev().collect();
    let mut last = 0;
    for index in selected {
        let file = format!("file-{index:03}.py");
        let position = run
            .stdout
            .find(&file)
            .unwrap_or_else(|| panic!("selected digest omitted {file}:\n{}", run.stdout));
        assert!(
            position >= last,
            "selected files are out of order:\n{}",
            run.stdout
        );
        last = position;
        assert!(
            run.stdout.contains(&format!("Purpose: Purpose {index}")),
            "purpose omitted for {file}:\n{}",
            run.stdout
        );
        assert!(
            run.stdout.contains(&format!("Top fns: hot_{index}() L2")),
            "function digest omitted for {file}:\n{}",
            run.stdout
        );
    }
    assert!(
        !run.stdout.contains("file-000.py"),
        "the twenty-file display limit changed:\n{}",
        run.stdout
    );

    let events = fs::read_to_string(&trace_events).expect("git trace2 event log");
    let root_lookups = events
        .lines()
        .filter(|line| line.contains("\"argv\":[\"git\",\"rev-parse\",\"--show-toplevel\"]"))
        .count();
    assert_eq!(
        root_lookups, 1,
        "human directory digest repeated the worktree-root lookup per selected file ({root_lookups} calls):\n{events}"
    );
}

#[test]
fn file_info_orders_dependencies_before_the_fifteen_row_limit() {
    let f = Fixture::new();
    f.write("pkg/__init__.py", "");

    let mut imports = String::new();
    for index in (0..18).rev() {
        let name = format!("target_{index:02}");
        f.write(
            &format!("pkg/{name}.py"),
            &format!("def {name}():\n    return {index}\n"),
        );
        imports.push_str(&format!("from pkg.{name} import {name}\n"));
    }
    imports.push_str("from pkg.target_17 import target_17\n");
    imports.push_str("\ndef entry():\n    return target_17()\n");
    f.write("pkg/entry.py", &imports);
    f.write("pkg/other.py", "def other():\n    return -1\n");
    f.write(
        "pkg/unrelated.py",
        "from pkg.other import other\n\ndef unrelated():\n    return other()\n",
    );
    f.commit("dependency ordering fixture");

    let expected: Vec<serde_json::Value> = (0..15)
        .map(|index| {
            serde_json::json!({
                "module": format!("pkg.target_{index:02}"),
                "confidence": "EXTRACTED",
            })
        })
        .collect();

    let read_dependencies = || {
        let run = f.trace(&["info", "pkg/entry.py", "--json"]);
        run.ok();
        let document = run.json();
        document["context"]["files"]
            .as_object()
            .and_then(|files| files.values().next())
            .and_then(|file| file["dependencies"].as_array())
            .unwrap_or_else(|| panic!("file info omitted dependencies: {}", run.stdout))
            .clone()
    };

    let cold = read_dependencies();
    let warm = read_dependencies();
    f.trace(&["cache", "clear", "--all"]).ok();
    f.trace(&["cache", "build", "."]).ok();
    let rebuilt = read_dependencies();

    assert_eq!(cold, expected, "cold dependency subset is not stable");
    assert_eq!(warm, expected, "warm dependency subset is not stable");
    assert_eq!(rebuilt, expected, "rebuilt dependency subset is not stable");
}

#[test]
fn repeated_imports_keep_the_strongest_dependency_confidence() {
    let f = Fixture::new();
    f.write("pkg/__init__.py", "");
    f.write("pkg/target.py", "def target():\n    return 1\n");
    f.write(
        "pkg/entry.py",
        "import pkg.target\nfrom pkg.target import target\n\ndef entry():\n    return target()\n",
    );
    f.commit("module and named import");

    for _ in 0..2 {
        let run = f.trace(&["info", "pkg/entry.py", "--json"]);
        run.ok();
        let document = run.json();
        let file = document["context"]["files"]
            .as_object()
            .and_then(|files| files.values().next())
            .expect("info has one file context");
        assert_eq!(
            file["dependencies"],
            serde_json::json!([{"module": "pkg.target", "confidence": "EXTRACTED"}]),
            "the named import must retain the stronger confidence: {}",
            run.stdout
        );
    }
}

#[test]
fn file_info_ranks_top_callers_before_limiting() {
    let f = Fixture::new();
    f.write("target.py", "def target():\n    return 1\n");
    f.write(
        "a_leaf.py",
        "from target import target\n\ndef leaf():\n    return target()\n",
    );
    f.write(
        "z_hub.py",
        "from target import target\n\ndef hub():\n    return target()\n",
    );
    f.write(
        "z_user.py",
        "from z_hub import hub\n\ndef user():\n    return hub()\n",
    );
    f.commit("top callers fixture");

    let run = f.trace(&["info", "target.py", "--json"]);
    run.ok();
    let document = run.json();
    let callers = document["context"]["files"]
        .as_object()
        .and_then(|files| files.values().next())
        .and_then(|file| file["top_callers"].as_array())
        .unwrap_or_else(|| panic!("file info omitted top callers: {}", run.stdout));
    let caller_files: Vec<&str> = callers
        .iter()
        .map(|caller| caller["source_file"].as_str().unwrap())
        .collect();

    assert_eq!(
        caller_files,
        vec!["z_hub.py", "a_leaf.py"],
        "the hub importer ranks ahead of the earlier path because it has its own importer"
    );
}

#[test]
fn empty_directory_list_is_clean() {
    let f = Fixture::new();
    f.write("seed.py", "pass\n");
    f.commit("seed");
    fs::create_dir_all(f.root.join("hollow")).unwrap();
    let r = f.trace(&["list", "hollow", "--json"]);
    r.ok();
    let v = r.view();
    assert!(v["files"].as_array().unwrap().is_empty());
    assert!(v["directories"].as_array().unwrap().is_empty());
}

#[test]
fn binary_file_read_does_not_crash() {
    let f = Fixture::new();
    f.write("seed.py", "pass\n");
    f.write_bytes(
        "blob.dat",
        &[0x00, 0x01, 0x02, 0xff, 0xfe, 0x00, b'B', b'I', b'N'],
    );
    f.commit("with binary");
    // Binary read succeeds (exit 0) with replacement chars.
    let r = f.trace(&["read", "blob.dat"]);
    r.ok();
    assert!(r.stdout.contains("blob.dat"), "{}", r.stdout);
}

#[test]
fn non_source_file_info_yields_zero_complexity() {
    let f = Fixture::new();
    f.write("notes.md", "# Title\n\nProse, no code.\n");
    f.commit("docs");
    let r = f.trace(&["info", "notes.md", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(
        v["functions"].as_i64().unwrap(),
        0,
        "markdown has no functions: {}",
        r.stdout
    );
}

#[test]
fn very_large_file_is_analyzed_correctly() {
    let f = Fixture::new();
    let mut big = String::with_capacity(200_000);
    for i in 0..3000 {
        big.push_str(&format!(
            "def fn{i}(a, b):\n    if a and b:\n        return a\n    return b\n\n"
        ));
    }
    f.write("huge.py", &big);
    f.commit("huge file");
    let r = f.trace(&["info", "huge.py", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(
        v["functions"].as_i64().unwrap(),
        3000,
        "large-file function count wrong: {}",
        v["functions"]
    );
    // Each fn: base 1 + if(1) + short-circuit `and`(1) = 3. The exact
    // aggregate is 3000 * 3 = 9000 — a lower bound would pass even if
    // the `and` were silently dropped (which would give 6000).
    assert_eq!(
        v["ccn_max_function"].as_i64().unwrap(),
        3,
        "each fn must be exactly 3 (if + `and` over base 1): {}",
        v["ccn_max_function"]
    );
    assert_eq!(
        v["ccn_total"].as_i64().unwrap(),
        9000,
        "large-file ccn aggregate must be exactly 3000*3: {}",
        v["ccn_total"]
    );
}
