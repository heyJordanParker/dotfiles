//! Search commands: grep (ripgrep-backed), pattern (ast-grep-backed),
//! find (basename fnmatch and full-path glob). Correctness on human
//! and `--json` output plus the major flags.

use tracer_cli_tests::{standard_repo, Fixture};

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
    assert_eq!(rc["total_files"].as_i64().unwrap(), 8, "repo_context: {}", gv);
    assert_eq!(rc["median_file_ccn"].as_i64().unwrap(), 0, "repo_context: {}", gv);
    assert_eq!(rc["complexity_p95"].as_i64().unwrap(), 1, "repo_context: {}", gv);

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
    let lm = git["last_modified"].as_str().expect("last_modified present");
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
        "pattern", "def $N($$$A): $$$B", "-l", "python", "--path", ".", "--json",
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
            "pattern", "def $N($$$A): $$$B", "-l", "python", "--path", ".", "--json",
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

    let want = "dispatch is a function \u{00b7} 1 callers \u{00b7} 1 definitions \
                \u{2192} trace callers dispatch";
    for args in [
        vec!["grep", "dispatch", "--path", ".", "--json"],
        vec!["pattern", "dispatch($$$A)", "-l", "python", "--path", ".", "--json"],
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

/// One language table for both searches: `tsx` means the same thing on each.
/// ripgrep has no `tsx` type, so `grep -l tsx` used to return zero matches
/// with no error while `pattern -l tsx` worked.
#[test]
fn a_language_name_means_the_same_on_grep_and_pattern() {
    let f = Fixture::new();
    f.write("src/comp.tsx", "export function Widget() {\n  return 1;\n}\n");
    f.write("src/other.py", "def widget():\n    return 1\n");
    f.commit("tsx and py");

    let g = f.trace(&["grep", "Widget", "--path", ".", "-l", "tsx", "--json"]);
    g.ok();
    let grep_files: Vec<String> = g.view()["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["file"].as_str().unwrap().trim_start_matches("./").to_string())
        .collect();
    assert_eq!(
        grep_files,
        vec!["src/comp.tsx".to_string()],
        "grep -l tsx must search the .tsx file: {}",
        g.stdout
    );

    let p = f.trace(&[
        "pattern", "function Widget() { $$$B }", "-l", "tsx", "--path", ".", "--json",
    ]);
    p.ok();
    let pattern_files: Vec<String> = p.view()["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["file"].as_str().unwrap().trim_start_matches("./").to_string())
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
        vec!["pattern", "def $N(): $$$B", "-l", "nosuchlang", "--path", "."],
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
            "def build(q):\n",          // L1
            "    return (q\n",          // L2
            "        .filter()\n",      // L3
            "        .save())\n",       // L4
        ),
    );
    f.commit("chained call");
    let r = f.trace(&[
        "pattern", "$X.save($$$A)", "-l", "python", "--path", ".", "--json",
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
                v["files"][file]["file_complexity"]["ccn_total"].as_i64().unwrap(),
                v["files"][file]["file_complexity"]["rank"].as_str().unwrap(),
            )
        })
        .collect();
    hits.sort();
    assert_eq!(
        hits,
        vec![
            ("src/app.py", 5, 4, "low"),
            ("src/util.py", 1, 2, "low"),
        ],
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
    f.write("node_modules/pkg/keep.py", "def vendored():\n    return 2\n");
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
        "find", "*.py", ".", "--path", "*/src/*", "--exclude", "*util*", "--json",
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
    assert_eq!(got, vec!["src/app.py".to_string(), "src/util.py".to_string()]);
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
    assert_eq!(got, vec!["src/app.py".to_string(), "src/util.py".to_string()]);
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
            !v["files"][path]["shoulder"].as_str().unwrap_or("").is_empty(),
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
    f.git(&["-C", "themes/vendortheme", "commit", "--quiet", "-m", "vendor"]);

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
    f.git(&["-C", "themes/vendortheme", "commit", "--quiet", "-m", "vendor"]);

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
