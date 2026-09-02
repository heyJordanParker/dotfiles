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
use tracer_cli_tests::{trace, Fixture};

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
    fs::create_dir_all(&tmp).unwrap();
    fs::write(tmp.join("lone.py"), "def f(x):\n    return x\n").unwrap();

    let survey = trace(&tmp, ["stats", ".", "--json"]);
    survey.ok();
    let v = survey.view();
    // No git, scc still classifies the one lone.py: exactly one Python
    // file, 2 loc, 0 file-level complexity. (total_files and distribution
    // are likewise fixed; top_complex carries an absolute temp path that
    // varies per run, so only the deterministic facets are pinned.)
    assert_eq!(
        v["files"].as_i64().unwrap(),
        1,
        "survey outside a repo must count exactly the one file: {v}"
    );
    assert_eq!(
        v["languages"],
        serde_json::json!({"Python": {"files": 1, "loc": 2, "complexity": 0}}),
        "survey languages outside a repo must be exact: {}",
        v["languages"]
    );
    assert_eq!(
        v["distribution"],
        serde_json::json!({"median": 0, "p75": 0, "p90": 0, "p95": 0, "max": 0}),
        "survey distribution outside a repo must be all-zero: {}",
        v["distribution"]
    );

    let info = trace(&tmp, ["info", &tmp.join("lone.py").to_string_lossy(), "--json"]);
    info.ok();
    let iv = info.view();
    // `def f(x): return x` — no branches: exactly one function, CCN 1.
    assert_eq!(
        iv["functions"].as_i64().unwrap(),
        1,
        "info failed to analyze a file outside any repo"
    );
    assert_eq!(
        iv["ccn_total"].as_i64().unwrap(),
        1,
        "branchless f() must have CCN exactly 1 outside a repo"
    );
    assert_eq!(iv["ccn_max_function"].as_i64().unwrap(), 1);

    fs::remove_dir_all(&tmp).ok();
}

/// Survey numbers are asserted exactly on a fully controlled fixture, not
/// merely "the fields exist". Three Python files with known scc complexity:
/// two `if`-bearing (scc complexity 1 each) and one branchless (0). A
/// presence-only check would pass even if every number were wrong.
#[test]
fn stats_reports_exact_numbers_on_known_fixture() {
    let f = Fixture::new();
    f.write("src/a.py", "def a(x):\n    if x:\n        return 1\n    return 0\n");
    f.write("src/b.py", "def b(x):\n    if x:\n        return 1\n    return 0\n");
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
    assert_eq!(dist["max"].as_i64().unwrap(), 1, "max per-file ccn is 1: {dist}");
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
    f.write_bytes("blob.dat", &[0x00, 0x01, 0x02, 0xff, 0xfe, 0x00, b'B', b'I', b'N']);
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
