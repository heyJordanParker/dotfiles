//! Git-archaeology commands: history (whole-file, function, pickaxe modes),
//! blame (file / symbol / lines scopes), diff (file + symbol mode, default
//! and explicit base, rename lifecycle), status (blast-radius ordering).
//!
//! The ordering assertions here are exact, not membership-only: the whole
//! point of these commands is "look at the load-bearing thing first", so a
//! command whose ranking inverted must fail the suite, not pass it.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use tracer_cli_tests::{
    schema_directory, standard_repo, trace, Fixture, PUBLISHED_SCHEMA_VERSION,
};

/// The git facts `info` shows for one repo-relative path.
fn info_git(f: &Fixture, path: &str) -> serde_json::Value {
    let value = f.trace(&["info", path, "--json"]).ok().view();
    value["files"][path]["git"].clone()
}

/// The deploy branches a status document's facts name for `path`; none is `[]`.
fn deploy_branches(document: &serde_json::Value, path: &str) -> serde_json::Value {
    match &document["files"][path]["git"]["on_deploy_branches"] {
        serde_json::Value::Null => serde_json::json!([]),
        branches => branches.clone(),
    }
}

#[test]
fn deployment_refs_are_observed_once_and_refresh_independently() {
    let f = Fixture::new();
    f.write(".gitignore", ".tracer-cache/\n");
    f.write("base.py", "BASE = 1\n");
    f.commit("base");
    f.write("deployed.py", "DEPLOYED = 1\n");
    f.commit("add deployed file");

    // Warm the HEAD-keyed history before any deployment ref exists, then hold
    // HEAD fixed while only live refs and working-tree state move.
    info_git(&f, "deployed.py");
    for r#ref in [
        "refs/remotes/origin/production",
        "refs/remotes/origin/staging",
        "refs/remotes/origin/main",
        "refs/remotes/origin/master",
        "refs/remotes/origin/main-extra",
        "refs/remotes/unrelated/main",
    ] {
        f.git(&["update-ref", r#ref, "HEAD"]);
    }
    f.write("deployed.py", "DEPLOYED = 2\n");

    let row = |run: &tracer_cli_tests::Run| {
        run.ok();
        run.view()
    };
    let assert_live = |document: &serde_json::Value, refs: &[&str]| {
        let state = document["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["path"] == "deployed.py")
            .map(|row| row["state"].clone())
            .expect("deployed.py status row");
        assert_eq!(state, "modified", "working state changed: {document}");
        assert_eq!(
            deploy_branches(document, "deployed.py"),
            serde_json::json!(refs),
            "deployment presence changed incorrectly: {document}"
        );
    };

    let trace_events = f.root.join(".git/live-refs-trace.json");
    let trace_events_text = trace_events.to_string_lossy().to_string();
    let initial = f.trace_env(
        &["status", "--json"],
        &[("GIT_TRACE2_EVENT", trace_events_text.as_str())],
    );
    assert_live(&row(&initial), &["main", "prod", "staging"]);

    let events = std::fs::read_to_string(&trace_events).expect("git trace2 event log");
    let ref_rev_parses = events
        .lines()
        .filter(|line| {
            [
                "origin/production",
                "origin/staging",
                "origin/main",
                "origin/master",
            ]
            .iter()
            .any(|r#ref| line.contains(&format!("\"argv\":[\"git\",\"rev-parse\",\"{ref}\"]")))
        })
        .count();
    let ref_observations = events
        .lines()
        .filter(|line| {
            line.contains("\"argv\":[\"git\",\"for-each-ref\"")
                && line.contains("refs/remotes/origin/production")
                && line.contains("refs/remotes/origin/staging")
                && line.contains("refs/remotes/origin/main")
                && line.contains("refs/remotes/origin/master")
        })
        .count();
    assert_eq!(
        ref_rev_parses, 0,
        "expected no ref-specific rev-parse launches, found {ref_rev_parses}; events:\n{events}"
    );
    assert_eq!(
        ref_observations, 1,
        "expected one bounded for-each-ref observation, found {ref_observations}; events:\n{events}"
    );

    f.git(&["update-ref", "refs/remotes/origin/production", "HEAD^"]);
    assert_live(&row(&f.trace(&["status", "--json"])), &["main", "staging"]);

    f.git(&["update-ref", "refs/remotes/origin/staging", "HEAD^"]);
    assert_live(&row(&f.trace(&["status", "--json"])), &["main"]);

    f.git(&["update-ref", "refs/remotes/origin/main", "HEAD^"]);
    assert_live(&row(&f.trace(&["status", "--json"])), &["main"]);

    f.git(&["update-ref", "-d", "refs/remotes/origin/master"]);
    assert_live(&row(&f.trace(&["status", "--json"])), &[]);
}

#[test]
fn deployment_presence_uses_the_observed_tip_when_the_ref_moves() {
    let f = Fixture::new();
    f.write(".gitignore", ".tracer-cache/\ntest-bin/\n");
    let existing = "existing\tline\nbreak.py";
    let added = "added\tline\nbreak.py";
    f.write(existing, "VALUE = 1\n");
    f.commit("first deployment tree");
    f.write(added, "VALUE = 1\n");
    f.commit("second deployment tree");
    f.git(&["update-ref", "refs/remotes/origin/production", "HEAD^"]);
    f.write(existing, "VALUE = 2\n");
    f.write(added, "VALUE = 2\n");

    let actual_git = String::from_utf8(
        Command::new("which")
            .arg("git")
            .output()
            .expect("find git executable")
            .stdout,
    )
    .unwrap()
    .trim()
    .to_string();
    let wrapper_directory = f.root.join("test-bin");
    std::fs::create_dir_all(&wrapper_directory).unwrap();
    let observed_output = f.root.join(".git/presence-observed");
    let wrapper = wrapper_directory.join("git");
    std::fs::write(
        &wrapper,
        format!(
            "#!/bin/sh\nif [ \"$1\" = \"for-each-ref\" ]; then\n  '{}' \"$@\" > '{}'\n  '{}' update-ref refs/remotes/origin/production HEAD\n  cat '{}'\nelse\n  exec '{}' \"$@\"\nfi\n",
            actual_git,
            observed_output.display(),
            actual_git,
            observed_output.display(),
            actual_git,
        ),
    )
    .unwrap();
    std::fs::set_permissions(
        &wrapper,
        <std::fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o755),
    )
    .unwrap();
    let path = format!(
        "{}:{}",
        wrapper_directory.display(),
        std::env::var("PATH").unwrap_or_default()
    );

    let rows = |run: &tracer_cli_tests::Run| {
        run.ok();
        run.view()
    };
    let presence = |document: &serde_json::Value, path: &str| {
        let row = document["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["path"] == path)
            .unwrap_or_else(|| panic!("missing status row for {path:?}: {document}"));
        assert_eq!(row["state"], "modified", "working state changed: {row}");
        deploy_branches(document, path)
    };

    let observed = rows(&f.trace_env(&["status", "--json"], &[("PATH", &path)]));
    assert_eq!(presence(&observed, existing), serde_json::json!(["prod"]));
    assert_eq!(presence(&observed, added), serde_json::json!([]));

    let moved = rows(&f.trace_env(&["status", "--json"], &[("PATH", &path)]));
    assert_eq!(presence(&moved, existing), serde_json::json!(["prod"]));
    assert_eq!(presence(&moved, added), serde_json::json!(["prod"]));
}

#[test]
fn untracked_machine_paths_keep_tabs_newlines_and_unicode() {
    let f = standard_repo();
    let path = "nested/café\tline\nbreak.py";
    f.write(path, "VALUE = 1\n");

    let untracked = info_git(&f, path);
    assert_eq!(
        untracked["status"], "untracked",
        "machine-readable porcelain lost the exact path: {untracked}"
    );
    f.commit("commit machine path");
    let historical = info_git(&f, path);
    assert_eq!(
        historical["commits"], 1,
        "NUL-delimited history lost the exact path: {historical}"
    );
}

#[test]
fn bounded_history_renders_commit_count_and_age_as_lower_bounds() {
    let f = Fixture::new();
    let mut stream = String::new();
    for index in 0..=4000 {
        let blob_mark = index * 2 + 1;
        let commit_mark = index * 2 + 2;
        let body = format!("VALUE = {index}\n");
        stream.push_str(&format!(
            "blob\nmark :{blob_mark}\ndata {}\n{body}",
            body.len()
        ));
        stream.push_str(&format!(
            "commit refs/heads/master\nmark :{commit_mark}\nauthor Tracer Test <trace@example.test> 1700000000 +0000\ncommitter Tracer Test <trace@example.test> 1700000000 +0000\ndata 4\nstep\n"
        ));
        if index > 0 {
            stream.push_str(&format!("from :{}\n", commit_mark - 2));
        }
        stream.push_str(&format!("M 100644 :{blob_mark} bounded.py\n"));
        if index == 4000 {
            stream.push_str(&format!("M 100644 :{blob_mark} one.py\n"));
        }
        stream.push('\n');
    }
    let mut child = Command::new("git")
        .args(["fast-import", "--quiet"])
        .current_dir(&f.root)
        .stdin(Stdio::piped())
        .spawn()
        .expect("spawn git fast-import");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stream.as_bytes())
        .unwrap();
    assert!(child.wait().unwrap().success(), "git fast-import failed");
    f.write("bounded.py", "VALUE = 4000\n");
    f.write("one.py", "VALUE = 4000\n");

    let bounded = info_git(&f, "bounded.py");
    assert_eq!(
        (bounded["commits_at_least"].clone(), bounded["commits"].clone()),
        (serde_json::json!(4000), serde_json::Value::Null),
        "bounded history rendered as an exact count: {bounded}"
    );
    assert!(
        bounded["first_commit"].as_str().unwrap().starts_with("at least "),
        "bounded history rendered an exact lifetime age: {bounded}"
    );
    let one = info_git(&f, "one.py");
    assert_eq!(
        (one["status"].clone(), one["commits_at_least"].clone()),
        (serde_json::json!("untracked"), serde_json::json!(1)),
        "a one-entry history floor claimed the file was new: {one}"
    );
}

#[test]
fn shallow_graft_path_that_looks_like_a_header_cannot_frame_history() {
    let f = Fixture::new();
    let header_path = "COMMIT|abc|2024-01-01|A|x";
    f.write(header_path, "HEADER = 1\n");
    f.write("ordinary.py", "ORDINARY = 1\n");
    f.commit("graft");

    let url = format!("file://{}", f.root.to_string_lossy());
    let clone = f.root.join("clone");
    f.git(&["clone", "--depth=1", &url, clone.to_str().unwrap()]);
    assert!(clone.join(".git").join("shallow").exists());

    for path in [header_path, "ordinary.py"] {
        let value = tracer_cli_tests::trace(&clone, ["info", path, "--json"])
            .ok()
            .view();
        let git = &value["files"][path]["git"];
        assert_eq!(
            git["commits"], 0,
            "a skipped graft path framed history for {path:?}: {git}"
        );
        assert!(git["main_author"].is_null(), "skipped graft attributed an owner: {git}");
        assert!(
            git["usually_changed_with"].is_null(),
            "skipped graft created co-change facts: {git}"
        );
    }
}

fn repo_with_history() -> Fixture {
    let f = Fixture::new();
    f.write("mod.py", "def alpha():\n    return 1\n");
    f.commit("add alpha");
    f.write(
        "mod.py",
        "def alpha():\n    return 2\n\n\ndef beta():\n    return 3\n",
    );
    f.commit("bump alpha, add beta");
    f
}

#[test]
fn history_whole_file_json_shape() {
    let f = repo_with_history();
    let r = f.trace(&["history", "mod.py", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(v["mode"], "file");
    assert_eq!(v["file"], "mod.py");
    // repo_with_history() commits mod.py exactly twice; both are recent.
    assert_eq!(
        v["commits"].as_i64().unwrap(),
        2,
        "exactly two commits: {}",
        v
    );
    assert_eq!(
        v["commits_30d"].as_i64().unwrap(),
        2,
        "both commits are recent: {}",
        v
    );
    assert_eq!(v["last_author"], "Tracer Test");
    assert_eq!(v["last_subject"], "bump alpha, add beta");
    assert_eq!(v["top_author"], "Tracer Test");
    // Newest commit first, then the original.
    let subjects: Vec<&str> = v["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["subject"].as_str().unwrap())
        .collect();
    assert_eq!(
        subjects,
        vec!["bump alpha, add beta", "add alpha"],
        "recent_commits: {}",
        v
    );
    for c in v["results"].as_array().unwrap() {
        assert_eq!(c["author"], "Tracer Test");
    }
    // The fixture's sole author owns all 6 lines of the final file.
    let blame = v["top_blame_authors"].as_array().unwrap();
    assert_eq!(blame.len(), 1, "one author: {}", v);
    assert_eq!(blame[0]["author"], "Tracer Test");
    assert_eq!(
        blame[0]["lines"].as_i64().unwrap(),
        6,
        "6 lines in mod.py: {}",
        v
    );
}

#[test]
fn history_function_mode_returns_symbol_line_history() {
    let f = repo_with_history();
    let r = f.trace(&["history", "mod.py", "alpha", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(v["mode"], "function");
    assert_eq!(v["symbol"], "alpha");
    // alpha() was introduced in "add alpha" and its body changed in
    // "bump alpha, add beta": git log -L on alpha sees exactly both,
    // newest first.
    let subjects: Vec<&str> = v["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["subject"].as_str().unwrap())
        .collect();
    assert_eq!(
        subjects,
        vec!["bump alpha, add beta", "add alpha"],
        "alpha -L history must be exactly its two touching commits: {}",
        v
    );
}

#[test]
fn history_pickaxe_mode_finds_string_introduction() {
    let f = repo_with_history();
    let r = f.trace(&["history", "--contains", "beta", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(v["mode"], "contains");
    assert_eq!(v["pattern"], "beta");
    // "beta" enters the repo in exactly one commit: the second one. The
    // pickaxe must report that single commit and point at the line + the
    // enclosing symbol where the token appears.
    assert_eq!(
        v["commits"].as_i64().unwrap(),
        1,
        "beta added in one commit: {}",
        v
    );
    let commits = v["results"].as_array().unwrap();
    assert_eq!(commits.len(), 1, "exactly one pickaxe commit: {}", v);
    assert_eq!(commits[0]["subject"], "bump alpha, add beta");
    assert_eq!(commits[0]["author"], "Tracer Test");
    let matches = commits[0]["matches"].as_array().unwrap();
    assert_eq!(matches.len(), 1, "one matching line: {}", v);
    assert_eq!(matches[0]["path"], "mod.py");
    assert_eq!(
        matches[0]["line"].as_i64().unwrap(),
        5,
        "def beta() is on line 5: {}",
        v
    );
    assert_eq!(matches[0]["enclosing_symbol"], "beta");
}

#[test]
fn history_contains_names_the_declaration_grep_names_without_ctags() {
    let f = Fixture::new();
    f.write(
        "ledger.rs",
        concat!(
            "pub struct Ledger;\n",
            "impl Ledger {\n",
            "    #[must_use]\n",
            "    pub fn total(&self) -> u32 {\n",
            "        7 // tally_marker\n",
            "    }\n",
            "}\n",
        ),
    );
    f.commit("ledger total");
    let grep = f.trace(&["grep", "tally_marker", "--json"]);
    grep.ok();
    let named = grep.view()["results"][0]["declaration"]["name"].clone();
    assert_eq!(named, "total", "{}", grep.stdout);

    let shim_dir = f.root.join("shim");
    std::fs::create_dir_all(&shim_dir).unwrap();
    let shim = shim_dir.join("ctags");
    std::fs::write(&shim, "#!/bin/sh\necho 'ctags: boom' >&2\nexit 1\n").unwrap();
    std::fs::set_permissions(
        &shim,
        <std::fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o755),
    )
    .unwrap();
    let path = format!("{}:{}", shim_dir.display(), std::env::var("PATH").unwrap_or_default());
    let r = f.trace_env(
        &["history", "--contains", "tally_marker", "--json"],
        &[("PATH", &path), ("TRACE_TIMING", "1")],
    );
    r.ok();
    assert!(!r.stderr.contains("timing ctags"), "{}", r.stderr);
    let v = r.view();
    assert_eq!(v["results"][0]["matches"][0]["enclosing_symbol"], named, "{v}");
}

#[test]
fn history_contains_is_mutually_exclusive_with_file() {
    let f = repo_with_history();
    let r = f.trace(&["history", "mod.py", "--contains", "alpha"]);
    // history has optional/multi-mode args: argument-conflict and not-found
    // are explicit runtime errors — non-zero exit with a clear stderr
    // message, not the pathval exit-2 path required-arg commands use.
    assert_ne!(r.code, 0, "expected non-zero exit:\n{}", r.combined());
    assert!(
        r.combined().contains("mutually exclusive"),
        "{}",
        r.combined()
    );
}

#[test]
fn history_missing_file_fails_with_clear_error() {
    let f = repo_with_history();
    let r = f.trace(&["history", "no_such_file.py"]);
    assert_ne!(r.code, 0, "expected non-zero exit:\n{}", r.combined());
    assert!(r.combined().contains("file not found"), "{}", r.combined());
}

#[test]
fn blame_whole_file_json_regions() {
    let f = repo_with_history();
    let r = f.trace(&["blame", "mod.py", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(v["file"], "mod.py");
    assert_eq!(v["scope"], "file");
    // mod.py's final 6 lines blame to exactly two commits: line 1
    // (`def alpha():`, unchanged since "add alpha") and lines 2-6
    // (rewritten/added by "bump alpha, add beta"). Dates are not pinned —
    // a commit made near a UTC day boundary renders a different local
    // calendar date than `git log` does, so the date is environment-
    // dependent; the region partition, authorship and subjects are not.
    assert_eq!(
        v["regions"].as_i64().unwrap(),
        2,
        "two blame regions: {}",
        v
    );
    assert_eq!(v["lines"].as_i64().unwrap(), 6, "6 lines: {}", v);
    let regions = v["results"].as_array().unwrap();
    let shape: Vec<(i64, i64, &str, &str)> = regions
        .iter()
        .map(|r| {
            (
                r["line_start"].as_i64().unwrap(),
                r["line_end"].as_i64().unwrap(),
                r["author"].as_str().unwrap(),
                r["subject"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        shape,
        vec![
            (1, 1, "Tracer Test", "add alpha"),
            (2, 6, "Tracer Test", "bump alpha, add beta"),
        ],
        "blame regions must partition the file exactly by commit: {}",
        v
    );
}

#[test]
fn blame_symbol_scope_narrows_to_function() {
    let f = repo_with_history();
    let r = f.trace(&["blame", "mod.py", "beta", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(v["scope"], "symbol");
    assert_eq!(v["symbol"], "beta");
    // beta() occupies lines 5-6 of the final file and was wholly
    // introduced by the second commit, so it blames to a single region
    // spanning exactly that range. (Date is environment-dependent — see
    // blame_whole_file_json_regions — so it is not pinned.)
    assert_eq!(
        v["line_range"]["start"].as_i64().unwrap(),
        5,
        "beta starts at L5: {}",
        v
    );
    assert_eq!(
        v["line_range"]["end"].as_i64().unwrap(),
        6,
        "beta ends at L6: {}",
        v
    );
    assert_eq!(v["regions"].as_i64().unwrap(), 1, "one region: {}", v);
    assert_eq!(v["lines"].as_i64().unwrap(), 2, "beta is 2 lines: {}", v);
    let r = &v["results"][0];
    assert_eq!(r["line_start"].as_i64().unwrap(), 5);
    assert_eq!(r["line_end"].as_i64().unwrap(), 6);
    assert_eq!(r["author"], "Tracer Test");
    assert_eq!(r["subject"], "bump alpha, add beta");
}

#[test]
fn blame_lines_scope() {
    let f = repo_with_history();
    let r = f.trace(&["blame", "mod.py", "--lines", "1:2", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(v["scope"], "lines");
    assert_eq!(v["line_range"]["start"], 1);
    assert_eq!(v["line_range"]["end"], 2);
    // Lines 1-2 straddle the two commits: L1 from "add alpha", L2 from
    // "bump alpha, add beta" — two single-line regions. (Date is
    // environment-dependent, see blame_whole_file_json_regions.)
    assert_eq!(
        v["regions"].as_i64().unwrap(),
        2,
        "two regions across L1:2: {}",
        v
    );
    assert_eq!(v["lines"].as_i64().unwrap(), 2);
    let shape: Vec<(i64, i64, &str, &str)> = v["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["line_start"].as_i64().unwrap(),
                r["line_end"].as_i64().unwrap(),
                r["author"].as_str().unwrap(),
                r["subject"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        shape,
        vec![
            (1, 1, "Tracer Test", "add alpha"),
            (2, 2, "Tracer Test", "bump alpha, add beta"),
        ],
        "line-scoped blame must partition L1:2 by commit: {}",
        v
    );
}

#[test]
fn blame_symbol_and_lines_mutually_exclusive() {
    let f = repo_with_history();
    let r = f.trace(&["blame", "mod.py", "beta", "--lines", "1:2"]);
    // blame, like history, reports argument-conflict and unknown-symbol as
    // explicit runtime errors: non-zero exit with a clear stderr message.
    assert_ne!(r.code, 0, "expected non-zero exit:\n{}", r.combined());
    assert!(
        r.combined().contains("mutually exclusive"),
        "{}",
        r.combined()
    );
}

#[test]
fn blame_unknown_symbol_fails_with_clear_error() {
    let f = repo_with_history();
    let r = f.trace(&["blame", "mod.py", "no_such_symbol"]);
    assert_ne!(r.code, 0, "expected non-zero exit:\n{}", r.combined());
    assert!(r.combined().contains("not found"), "{}", r.combined());
}

#[test]
fn diff_file_mode_against_base_ref() {
    let f = Fixture::new();
    f.write("a.py", "X = 1\n");
    f.commit("base");
    // Branch so HEAD diverges from the base ref.
    f.git(&["branch", "base-ref"]);
    f.write("a.py", "X = 2\n");
    f.write("b.py", "Y = 1\n");
    f.commit("diverge");
    let r = f.trace(&["diff", "--base", "base-ref", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(v["granularity"], "file");
    let paths: Vec<&str> = v["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["path"].as_str().unwrap())
        .collect();
    // The diverge commit adds exactly a.py and b.py against base-ref; the
    // changed-file set is exactly those two, deterministically ordered.
    assert_eq!(
        paths,
        vec!["a.py", "b.py"],
        "diff vs base-ref must report exactly a.py and b.py: {:?}",
        paths
    );
}

#[test]
fn diff_symbol_mode_reports_symbol_states() {
    let f = Fixture::new();
    f.write("m.py", "def kept():\n    return 1\n");
    f.commit("base");
    f.git(&["branch", "base-ref"]);
    f.write(
        "m.py",
        "def kept():\n    return 1\n\n\ndef added():\n    return 2\n",
    );
    f.commit("add symbol");
    let r = f.trace(&["diff", "--base", "base-ref", "--symbols", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(v["granularity"], "symbol");
    let symbols: Vec<serde_json::Value> = v["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| serde_json::json!({"name": s["name"], "state": s["state"]}))
        .collect();
    // The only change vs base-ref is the newly added `added()`; `kept()`
    // is byte-identical and must not appear. Exactly one changed symbol.
    assert_eq!(
        serde_json::Value::Array(symbols),
        serde_json::json!([{"name": "added", "state": "added"}]),
        "symbol-mode diff must report exactly the one added symbol: {}",
        v["results"]
    );
}

#[test]
fn diff_unknown_base_ref_exits_2() {
    let f = standard_repo();
    let r = f.trace(&["diff", "--base", "no_such_ref_zzz"]);
    r.code_is(2);
    assert!(r.combined().contains("not found"), "{}", r.combined());
}

// --- The working tree, which is the default -----------------------------
//
// `trace diff` with no --base answers "what have I changed": staged,
// unstaged, and untracked, against HEAD. Plain `git diff` shows unstaged
// only, `--cached` staged only, and `git diff HEAD` both but no new files —
// so an agent asking this question with git gets a third of the answer and
// no sign that the rest exists.

#[test]
fn diff_default_scope_is_the_whole_working_tree_with_lines() {
    let f = standard_repo();
    f.write("src/util.py", "def helper(v):\n    return v + 99\n");
    f.git(&["add", "src/util.py"]);
    f.write("src/app.py", "import os\n\n\ndef main(x):\n    return 1\n");
    f.write("brand_new.py", "NEW = 1\n");

    let r = f.trace(&["diff", "--json"]);
    r.ok();
    let v = r.view();
    let by_path: std::collections::BTreeMap<&str, &serde_json::Value> = v["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| (row["path"].as_str().unwrap(), row))
        .collect();
    assert_eq!(
        by_path.keys().copied().collect::<Vec<_>>(),
        vec!["brand_new.py", "src/app.py", "src/util.py"],
        "staged, unstaged and untracked must all be in one answer: {}",
        r.stdout
    );

    // The staged edit's own lines, not just its name.
    let staged = by_path["src/util.py"]["lines"].as_str().unwrap();
    assert!(
        staged.contains("-        return v + 1") && staged.contains("+    return v + 99"),
        "the staged file's changed lines must be reported: {staged:?}"
    );
    // An untracked file's whole content is the change.
    let fresh = by_path["brand_new.py"]["lines"].as_str().unwrap();
    assert!(
        fresh.contains("+NEW = 1"),
        "an untracked file's content must be reported as added lines: {fresh:?}"
    );
    assert_eq!(by_path["brand_new.py"]["status"], "added");
}

#[test]
fn diff_before_the_first_commit_reports_every_file_as_added() {
    let f = Fixture::new();
    f.write("first.py", "VALUE = 1\n");
    f.write("staged.py", "OTHER = 2\n");
    f.git(&["add", "staged.py"]);

    let r = f.trace(&["diff", "--json"]);
    r.ok();
    let mut rows: Vec<(String, String)> = r.view()["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| (row["path"].as_str().unwrap().to_string(), row["status"].as_str().unwrap().to_string()))
        .collect();
    rows.sort();
    assert_eq!(
        rows,
        vec![("first.py".to_string(), "added".to_string()), ("staged.py".to_string(), "added".to_string())],
        "{}",
        r.stdout
    );
}

#[test]
fn diff_reports_annotated_declarations_touched_and_removed() {
    let f = Fixture::new();
    f.write(
        "Cart.php",
        "<?php\n#[Entity]\nclass Cart {\n    #[Field]\n    public string $billing;\n\n    #[Internal]\n    public function run(): string {\n        return 'old';\n    }\n\n    #[Internal]\n    public function old(): string {\n        return 'old';\n    }\n}\n",
    );
    f.write(
        "Svc.php",
        "<?php\nclass Svc {\n    #[Internal]\n    public function run(): string {\n        return 'old';\n    }\n\n    #[Internal]\n    public function old(): string {\n        return 'old';\n    }\n}\n",
    );
    f.commit("annotated declarations");
    f.write(
        "Cart.php",
        "<?php\n#[Entity(label: 'changed')]\nclass Cart {\n    #[Field]\n    public string $billing = 'paid';\n\n    #[Internal]\n    public function run(): string {\n        return 'new';\n    }\n}\n",
    );
    f.write(
        "Svc.php",
        "<?php\nclass Svc {\n    #[Internal]\n    public function run(): string {\n        return 'new';\n    }\n}\n",
    );
    f.write("Added.php", "<?php\n#[Entity]\nclass Added {}\n");

    let text = f.trace(&["diff"]);
    text.ok();
    assert!(
        text.stdout
            .contains("touches: L4 #[Internal] public function run(): string { … }")
            && text
                .stdout
                .contains("touches: L5 #[Field] public string $billing = 'paid';")
            && text
                .stdout
                .contains("touches: L3 #[Entity(label: 'changed')] class Cart { … }"),
        "edits must name every touched declaration row:\n{}",
        text.stdout
    );
    assert!(
        text.stdout
            .contains("removed: L9 #[Internal] public function old(): string { … }"),
        "deleted method must be named by its row:\n{}",
        text.stdout
    );
    assert!(
        text.stdout
            .contains("added: L3 #[Entity] class Added { … }"),
        "added class must be named by its row:\n{}",
        text.stdout
    );

    let json = f.trace(&["diff", "--json"]);
    json.ok();
    let rows = json.view()["results"].as_array().unwrap().clone();
    let svc = rows.iter().find(|row| row["path"] == "Svc.php").unwrap();
    assert!(svc["touches"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["name"] == "run"));
    assert_eq!(svc["removed"][0]["name"], "old");
}

#[test]
fn diff_json_file_context_omits_annotations() {
    let f = Fixture::new();
    f.write("changed.php", "<?php\n#[Entity]\nclass Changed {}\n");
    f.commit("base change");
    f.write("changed.php", "<?php\n#[Entity]\nclass Changed { public string $value = 'changed'; }\n");
    let result = f.trace(&["diff", "changed.php", "--json"]);
    result.ok();
    let view = result.view();
    let files = view["files"]
        .as_object()
        .unwrap_or_else(|| panic!("missing file context: {view:#?}"));
    let file = files
        .get("changed.php")
        .unwrap_or_else(|| panic!("missing changed.php context: {files:#?}"));
    assert!(file.get("annotations").is_none(), "{}", result.stdout);
}

#[test]
fn diff_surfaces_changed_added_removed_and_touched_rows() {
    let f = Fixture::new();
    f.write(
        "Shelf.php",
        "<?php\nclass Shelf {\n    #[Deprecated]\n    public function delete(string $id): void {}\n\n    public function save(string $first): void {}\n\n    public function removed(): void {}\n\n    public function touched(): void {\n        $value = 'remove me';\n    }\n}\n",
    );
    f.commit("shelf methods");
    f.write(
        "Shelf.php",
        "<?php\nclass Shelf {\n    #[Deprecated(since: '2.0')]\n    public function delete(string $id): void {}\n\n    public function save(string $last): void {}\n\n    public function added(): void {}\n\n    public function touched(): void {\n    }\n}\n",
    );

    let json = f.trace(&["diff", "--json"]);
    json.ok();
    let view = json.view();
    let row = view["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["path"] == "Shelf.php")
        .expect("Shelf.php diff row missing");
    assert!(
        row["changed"]
            .as_array()
            .unwrap()
            .iter()
            .any(|change| change["before"]["name"] == "delete"
                && change["after"]["name"] == "delete")
            && row["changed"]
                .as_array()
                .unwrap()
                .iter()
                .any(|change| change["before"]["name"] == "save"
                    && change["after"]["name"] == "save"),
        "{}",
        json.stdout
    );
    assert!(
        row["removed"]
            .as_array()
            .unwrap()
            .iter()
            .any(|declaration| declaration["name"] == "removed")
            && row["added"]
                .as_array()
                .unwrap()
                .iter()
                .any(|declaration| declaration["name"] == "added")
            && row["touches"]
                .as_array()
                .unwrap()
                .iter()
                .any(|declaration| declaration["name"] == "touched"),
        "{}",
        json.stdout
    );

    let text = f.trace(&["diff"]);
    text.ok();
    assert!(
        text.stdout.contains("changed:")
            && text.stdout.contains("removed:")
            && text.stdout.contains("added:")
            && text.stdout.contains("touches:"),
        "{}",
        text.stdout
    );
}

#[test]
fn diff_path_argument_scopes_to_one_file() {
    let f = standard_repo();
    f.write("src/util.py", "def helper(v):\n    return 0\n");
    f.write("src/app.py", "X = 1\n");

    let r = f.trace(&["diff", "src/util.py", "--json"]);
    r.ok();
    let v = r.view();
    let paths: Vec<&str> = v["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["path"].as_str().unwrap())
        .collect();
    assert_eq!(
        paths,
        vec!["src/util.py"],
        "a path argument must scope the answer to that file: {}",
        r.stdout
    );
}

#[test]
fn diff_base_ref_still_compares_committed_history() {
    // With --base the question is the review question — what this branch
    // changed against that ref — and the working tree is not part of it.
    let f = standard_repo();
    f.git(&["checkout", "-q", "-b", "feature"]);
    f.write("src/util.py", "def helper(v):\n    return 7\n");
    f.commit("committed on the branch");
    f.write("src/app.py", "UNCOMMITTED = 1\n");

    let r = f.trace(&["diff", "--base", "master", "--json"]);
    r.ok();
    let v = r.view();
    let paths: Vec<&str> = v["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["path"].as_str().unwrap())
        .collect();
    assert_eq!(
        paths,
        vec!["src/util.py"],
        "--base compares commits, so the uncommitted file stays out: {}",
        r.stdout
    );
}

// --- Exact load-bearing ordering: file mode ----------------------------
//
// Hand-determinable fixture. `direct_dependents` is the count of import
// edges whose target is the module owning the file; the file-mode key is
// (direct_dependents desc, ccn_total desc).
//
//   core.py    imported by two modules (a.py, b.py)  -> 2 dependents
//   midtier.py imported by one module  (a.py)         -> 1 dependent
//   leaf.py    imported by nobody, trivial body       -> 0 dependents
//
// All three are modified after the base branch point, so all three are in
// the changed set. The only correct order, most-load-bearing first, is
// core.py, midtier.py, leaf.py. Any inversion of the ranking flips this.

fn repo_load_bearing_files() -> Fixture {
    let f = Fixture::new();
    f.write(
        "core.py",
        "def core(x):\n    if x:\n        return 1\n    return 0\n",
    );
    f.write(
        "midtier.py",
        "def mid(x):\n    if x:\n        return 2\n    return 0\n",
    );
    f.write("leaf.py", "VALUE = 1\n");
    f.write(
        "a.py",
        "from core import core\nfrom midtier import mid\n\n\ndef a():\n    return core(1) + mid(1)\n",
    );
    f.write(
        "b.py",
        "from core import core\n\n\ndef b():\n    return core(2)\n",
    );
    f.commit("base graph");
    f.git(&["branch", "base-ref"]);
    // Touch all three leaf-of-interest files so each lands in the changed
    // set; keep the dependency edges intact.
    f.write(
        "core.py",
        "def core(x):\n    if x:\n        return 11\n    return 0\n",
    );
    f.write(
        "midtier.py",
        "def mid(x):\n    if x:\n        return 22\n    return 0\n",
    );
    f.write("leaf.py", "VALUE = 2\n");
    f.commit("modify all three");
    f
}

#[test]
fn diff_file_mode_orders_load_bearing_first_exactly() {
    let f = repo_load_bearing_files();
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["diff", "--base", "base-ref", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(v["granularity"], "file");
    let rows = v["results"].as_array().unwrap();

    // Pull the three files of interest in emitted order.
    let order: Vec<&str> = rows
        .iter()
        .map(|x| x["path"].as_str().unwrap())
        .filter(|p| matches!(*p, "core.py" | "midtier.py" | "leaf.py"))
        .collect();
    assert_eq!(
        order,
        vec!["core.py", "midtier.py", "leaf.py"],
        "load-bearing order wrong; full rows: {}",
        serde_json::to_string_pretty(rows).unwrap()
    );

    // Pin the discriminating attribute so a future change that keeps the
    // order by accident (e.g. all-zero dependents) still fails.
    let dep = |name: &str| -> i64 {
        rows.iter().find(|x| x["path"] == name).unwrap()["direct_dependents"]
            .as_i64()
            .unwrap()
    };
    assert_eq!(dep("core.py"), 2, "core is imported by a.py and b.py");
    assert_eq!(dep("midtier.py"), 1, "midtier is imported by a.py only");
    assert_eq!(dep("leaf.py"), 0, "leaf is imported by nobody");
}

// --- Exact load-bearing ordering: symbol mode --------------------------
//
// Symbol-mode key is (direct_dependents desc, state_weight desc) where
// removed=2, added=1, changed=0. Fixture: a hub symbol depended on by two
// call sites is *changed*; a brand-new isolated symbol is *added*. Even
// though "added" has a higher state weight than "changed", the changed
// hub has 2 dependents vs the added symbol's 0 — dependents dominate, so
// the hub must sort first. This proves the primary key is dependents, not
// state.

fn repo_load_bearing_symbols() -> Fixture {
    let f = Fixture::new();
    f.write(
        "hub.py",
        "def hub(x):\n    if x:\n        return 1\n    return 0\n",
    );
    f.write(
        "u1.py",
        "from hub import hub\n\n\ndef u1():\n    return hub(1)\n",
    );
    f.write(
        "u2.py",
        "from hub import hub\n\n\ndef u2():\n    return hub(2)\n",
    );
    f.commit("base symbols");
    f.git(&["branch", "base-ref"]);
    // Move hub() down a line so it is detected as `changed` (line moved),
    // and add a fresh isolated symbol nobody depends on.
    f.write(
        "hub.py",
        "# shifted\ndef hub(x):\n    if x:\n        return 1\n    return 0\n\n\ndef fresh():\n    return 9\n",
    );
    f.commit("change hub line, add fresh");
    f
}

#[test]
fn diff_symbol_mode_orders_load_bearing_first_exactly() {
    let f = repo_load_bearing_symbols();
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["diff", "--base", "base-ref", "--symbols", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(v["granularity"], "symbol");
    let rows = v["results"].as_array().unwrap();

    let order: Vec<(&str, &str, i64)> = rows
        .iter()
        .filter(|s| matches!(s["name"].as_str().unwrap(), "hub" | "fresh"))
        .map(|s| {
            (
                s["name"].as_str().unwrap(),
                s["state"].as_str().unwrap(),
                s["direct_dependents"].as_i64().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        order,
        vec![("hub", "changed", 2), ("fresh", "added", 0)],
        "changed hub (2 dependents) must outrank added fresh (0 dependents) — \
         dependents is the primary key, not state weight; full rows: {}",
        serde_json::to_string_pretty(rows).unwrap()
    );
}

// --- Exact blast-radius ordering: status -------------------------------
//
// status key is (-callers, -ccn_total, state_rank, path). Dirty three
// files in the same fixture:
//
//   hub.py   imported by u1.py + u2.py -> 2 callers (highest blast radius)
//   mid.py   imported by u1.py only    -> 1 caller
//   solo.py  imported by nobody        -> 0 callers
//
// All modified, so state_rank ties; callers is the discriminator. The
// only correct order is hub.py, mid.py, solo.py.

#[test]
fn status_orders_by_blast_radius_exactly() {
    let f = Fixture::new();
    f.write(
        "hub.py",
        "def hub(x):\n    if x:\n        return 1\n    return 0\n",
    );
    f.write(
        "mid.py",
        "def mid(x):\n    if x:\n        return 2\n    return 0\n",
    );
    f.write("solo.py", "VALUE = 1\n");
    f.write(
        "u1.py",
        "from hub import hub\nfrom mid import mid\n\n\ndef u1():\n    return hub(1) + mid(1)\n",
    );
    f.write(
        "u2.py",
        "from hub import hub\n\n\ndef u2():\n    return hub(2)\n",
    );
    f.commit("base");
    f.trace(&["cache", "build", "."]).ok();
    // Dirty all three of interest (modified state for each).
    f.write(
        "hub.py",
        "def hub(x):\n    if x:\n        return 99\n    return 0\n",
    );
    f.write(
        "mid.py",
        "def mid(x):\n    if x:\n        return 88\n    return 0\n",
    );
    f.write("solo.py", "VALUE = 2\n");

    let r = f.trace(&["status", "--json"]);
    r.ok();
    let v = r.view();
    let entries = v["results"].as_array().unwrap();
    let order: Vec<(&str, i64)> = entries
        .iter()
        .filter(|e| matches!(e["path"].as_str().unwrap(), "hub.py" | "mid.py" | "solo.py"))
        .map(|e| {
            let path = e["path"].as_str().unwrap();
            (path, v["files"][path]["imported_by"].as_i64().unwrap())
        })
        .collect();
    assert_eq!(
        order,
        vec![("hub.py", 2), ("mid.py", 1), ("solo.py", 0)],
        "blast-radius order wrong; full entries: {}",
        serde_json::to_string_pretty(entries).unwrap()
    );
}

#[test]
fn status_counts_only_non_ambiguous_imports() {
    let f = Fixture::new();
    f.write("one.py", "def one():\n    return 1\n");
    f.write("two.py", "def two():\n    return 2\n");
    f.write("a/shared.py", "def shared():\n    return 1\n");
    f.write("b/shared.py", "def shared():\n    return 2\n");
    f.write(
        "changed.py",
        "from one import one\nfrom two import two\nfrom shared import shared\n\ndef changed():\n    return one() + two() + shared()\n",
    );
    f.commit("three imports with one ambiguous target");
    f.write(
        "changed.py",
        "from one import one\nfrom two import two\nfrom shared import shared\n\ndef changed():\n    return one() + two() + shared() + 1\n",
    );

    let status = f.trace(&["status", "--json"]);
    status.ok();
    let view = status.view();
    let changed = &view["files"]["changed.py"];
    assert_eq!(
        changed["imports"].as_i64(),
        Some(2),
        "only the two resolved imports count: {}",
        status.stdout
    );
}

// --- Rename lifecycle, end to end --------------------------------------
//
// One file is renamed across a commit with its content carried forward.
// Three observable surfaces must each reflect the rename exactly:
//   1. `diff` against the pre-rename base reports status "renamed" with
//      rename_from = the old path.
//   2. `history` on the new path follows content across the rename: the
//      rename_chain contains the old path, and the pre-rename commit is
//      still in the count.
//   3. The inline lifecycle summary (here via `status` after a further
//      working-tree edit, and via the settled `diff` row's
//      summary) reflects the renamed state, not "new file".

fn repo_renamed_file() -> Fixture {
    let f = Fixture::new();
    f.write(
        "old_name.py",
        "def feature(x):\n    if x:\n        return 1\n    return 0\n",
    );
    f.write("caller.py", "from old_name import feature\n");
    f.commit("add old_name");
    f.git(&["branch", "base-ref"]);
    // Rename with content carried forward (git detects R via -M).
    f.git(&["mv", "old_name.py", "new_name.py"]);
    f.write("caller.py", "from new_name import feature\n");
    f.commit("rename old_name -> new_name");
    f
}

#[test]
fn diff_reports_rename_with_prior_path() {
    let f = repo_renamed_file();
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["diff", "--base", "base-ref", "--json"]);
    r.ok();
    let v = r.view();
    let row = v["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["path"] == "new_name.py")
        .expect("renamed file must appear at its new path");
    assert_eq!(row["status"], "renamed", "row: {row}");
    assert_eq!(
        row["rename_from"], "old_name.py",
        "rename_from must carry the prior path: {row}"
    );
}

#[test]
fn history_follows_content_across_rename() {
    let f = repo_renamed_file();
    let r = f.trace(&["history", "new_name.py", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(v["file"], "new_name.py");
    let chain: Vec<&str> = v["rename_chain"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap())
        .collect();
    assert_eq!(
        chain,
        vec!["old_name.py"],
        "rename_chain must follow content back to the prior path: {}",
        v["rename_chain"]
    );
    // repo_renamed_file() makes exactly two commits and `history` on the
    // new path follows content across the rename, so it counts both — the
    // pre-rename "add old_name" and the "rename" commit: exactly 2, not a
    // lower bound. last_subject/top_author are equally hand-determinable.
    assert_eq!(
        v["commits"].as_i64().unwrap(),
        2,
        "history must count exactly the two commits (incl. pre-rename): {}",
        v
    );
    assert_eq!(v["last_subject"], "rename old_name -> new_name");
    assert_eq!(v["top_author"], "Tracer Test");
    assert_eq!(v["last_author"], "Tracer Test");
}

#[test]
fn staged_rename_keeps_history_in_history_and_context() {
    let f = Fixture::new();
    f.write("old_name.py", "VALUE = 1\n");
    f.commit("add old name");
    f.write("old_name.py", "VALUE = 2\n");
    f.commit("change old name");
    f.git(&["mv", "old_name.py", "new_name.py"]);

    let history = f.trace(&["history", "new_name.py", "--json"]);
    history.ok();
    assert_eq!(history.view()["commits"], 2, "{}", history.stdout);

    let context = f.trace(&["context", "new_name.py", "--json"]);
    context.ok();
    assert_eq!(context.view()["files"]["new_name.py"]["git"]["commits"], 2, "{}", context.stdout);
}

#[test]
fn rename_lifecycle_summary_reflects_renamed_state() {
    let f = repo_renamed_file();
    f.trace(&["cache", "build", "."]).ok();

    // Settled state: the diff row's summary must label
    // the file as renamed-from the old path, never as a fresh/new file.
    let r = f.trace(&["diff", "--base", "base-ref", "--json"]);
    r.ok();
    let v = r.view();
    assert!(
        v["results"]
            .as_array()
            .unwrap()
            .iter()
            .any(|x| x["path"] == "new_name.py"),
        "the renamed file must be in the changed set: {}",
        r.stdout
    );
    // The settled diff row's facts are fully deterministic for this
    // hermetic fixture: renamed from the prior path, two commits (both
    // recent) followed across the rename, the carried-forward complexity of
    // 2 (feature() has one `if`), caller.py importing it and changed in the
    // same rename commit, the fixed author, and the rename commit's subject.
    let facts = &v["files"]["new_name.py"];
    let git = &facts["git"];
    assert_eq!(git["renamed_from"], "old_name.py", "{facts}");
    assert_eq!(git["commits"], 2, "{facts}");
    assert_eq!(git["commits_last_30_days"], 2, "{facts}");
    assert_eq!(git["usually_changed_with"], serde_json::json!(["caller.py"]), "{facts}");
    assert_eq!(git["main_author"], "Tracer Test", "{facts}");
    assert!(
        git["last_commit"].as_str().unwrap().ends_with("by Tracer Test: rename old_name -> new_name"),
        "{facts}"
    );
    assert_eq!(
        (facts["imported_by"].clone(), facts["imports"].clone(), facts["cyclomatic_complexity"].clone()),
        (serde_json::json!(1), serde_json::json!(0), serde_json::json!(2)),
        "{facts}"
    );

    f.git(&["mv", "new_name.py", "third_name.py"]);
    let rs = f.trace(&["status", "--json"]);
    rs.ok();
    let sv = rs.view();
    let renamed_path = sv["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["state"] == "renamed")
        .expect("uncommitted rename must appear in status as state=renamed")["path"]
        .as_str()
        .unwrap()
        .to_string();
    let renamed = &sv["files"][renamed_path.as_str()];
    assert_eq!(renamed["git"]["status"], "renamed", "{renamed}");
    assert_eq!(renamed["git"]["renamed_from"], "new_name.py", "{renamed}");
    assert_eq!(renamed["git"]["commits"], 2, "{renamed}");
    assert_eq!(renamed["git"]["usually_changed_with"], serde_json::json!(["caller.py"]), "{renamed}");
    assert_eq!(renamed["imported_by"], 1, "{renamed}");
    assert_eq!(renamed["cyclomatic_complexity"], 2, "{renamed}");
}

#[test]
fn status_clean_tree_reports_clean() {
    let f = standard_repo();
    let r = f.trace(&["status", "--json"]);
    r.ok();
    assert_eq!(r.json()["counts"]["files"], 0);
    assert!(r.view()["results"].as_array().unwrap().is_empty());
}

#[test]
fn status_lists_dirty_files_with_intelligence() {
    let f = standard_repo();
    f.trace(&["cache", "build", "."]).ok();
    f.write("src/util.py", "def helper(v):\n    return v + 99\n");
    f.write("newfile.py", "Z = 0\n");
    let r = f.trace(&["status", "--json"]);
    r.ok();
    let v = r.view();
    // The dirty set excludes tracer's self-ignored cache: src/util.py is
    // modified and newfile.py is untracked. status orders by blast radius.
    assert_eq!(
        r.json()["counts"]["files"],
        2,
        "dirty set must be exactly util.py + newfile.py: {}",
        r.stdout
    );
    let paths: Vec<&str> = v["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["path"].as_str().unwrap())
        .collect();
    assert_eq!(
        paths,
        vec!["src/util.py", "newfile.py"],
        "status entry order must be blast-radius then stable: {:?}",
        paths
    );
    let entries = v["results"].as_array().unwrap();
    let modified = entries
        .iter()
        .find(|e| e["path"] == "src/util.py")
        .expect("modified util.py missing");
    assert_eq!(modified["state"], "modified");
    // The rewritten body `def helper(v): return v + 99` is one function
    // with no decision nodes — CCN exactly 1.
    assert_eq!(
        v["files"]["src/util.py"]["cyclomatic_complexity"], 1,
        "rewritten branchless helper must have complexity exactly 1: {modified}"
    );
}

#[test]
fn status_state_filter() {
    let f = standard_repo();
    f.write("untracked_only.py", "pass\n");
    let r = f.trace(&["status", "--state", "untracked", "--json"]);
    r.ok();
    let v = r.view();
    for e in v["results"].as_array().unwrap() {
        assert_eq!(e["state"], "untracked", "state filter leaked: {e}");
    }
}

/// Staging is one more word per file. An agent that reads "modified" and
/// commits finds it committed nothing, or half of what it meant.
#[test]
fn status_carries_the_staging_word_per_file() {
    let f = standard_repo();
    f.write("src/util.py", "def helper(v):\n    return 1\n");
    f.git(&["add", "src/util.py"]);
    f.write("src/app.py", "X = 1\n");
    f.write("lib/widget.php", "<?php\n$half = 1;\n");
    f.git(&["add", "lib/widget.php"]);
    f.write("lib/widget.php", "<?php\n$half = 2;\n");
    f.write("fresh.py", "pass\n");

    let r = f.trace(&["status", "--json"]);
    r.ok();
    let v = r.view();
    let staging: std::collections::BTreeMap<&str, Option<&str>> = v["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| (e["path"].as_str().unwrap(), e["staging"].as_str()))
        .collect();
    assert_eq!(staging["src/util.py"], Some("staged"));
    assert_eq!(staging["src/app.py"], Some("unstaged"));
    assert_eq!(
        staging["lib/widget.php"],
        Some("partly staged"),
        "a file staged and then edited again is partly staged: {}",
        r.stdout
    );
    assert_eq!(
        staging["fresh.py"], None,
        "an untracked file has nothing staged to describe: {}",
        r.stdout
    );

    let h = f.trace(&["status"]);
    h.ok();
    assert!(
        h.stdout.contains("src/util.py \u{00b7} staged"),
        "the human line must carry the staging word:\n{}",
        h.stdout
    );
}

#[test]
fn status_preserves_every_row_across_the_resolve_bound() {
    let f = Fixture::new();
    f.write(".gitignore", ".tracer-cache/\n");
    for index in 0..514 {
        f.write(
            &format!("file_{index:03}.py"),
            &format!("VALUE = {index}\n"),
        );
    }
    f.commit("base");

    f.git(&["mv", "file_000.py", "renamed.py"]);
    f.git(&["rm", "file_001.py"]);
    for index in 2..514 {
        f.write(
            &format!("file_{index:03}.py"),
            &format!("def value():\n    return {index}\n"),
        );
    }
    f.write("fresh.py", "FRESH = 1\n");
    f.git(&["add", "file_002.py"]);
    f.write("file_002.py", "def value():\n    return 999\n");
    f.git(&["add", "file_003.py"]);

    let r = f.trace(&["status", "--json"]);
    r.ok();
    let v = r.view();
    let rows = v["results"].as_array().unwrap();
    assert_eq!(
        r.json()["counts"]["files"], 515,
        "every dirty row must survive: {}",
        r.stdout
    );
    assert_eq!(rows.len(), 515, "result count must match counts.files");

    let row = |path: &str| {
        rows.iter()
            .find(|entry| entry["path"] == path)
            .unwrap_or_else(|| panic!("missing status row for {path}"))
    };
    assert_eq!(row("renamed.py")["state"], "renamed");
    assert_eq!(row("file_001.py")["state"], "deleted");
    assert_eq!(row("fresh.py")["state"], "untracked");
    assert_eq!(row("file_002.py")["staging"], "partly staged");
    assert_eq!(row("file_003.py")["staging"], "staged");
    let last = &v["files"]["file_513.py"];
    assert_eq!(
        serde_json::json!({
            "lines": last["lines"],
            "cyclomatic_complexity": last["cyclomatic_complexity"],
            "complexity_rank": last["complexity_rank"],
            "imported_by": last["imported_by"],
        }),
        serde_json::json!({"lines": 2, "cyclomatic_complexity": 1, "complexity_rank": "low", "imported_by": 0}),
        "last chunk must retain its facts: {last}"
    );
}

/// `history --commit <ref>` answers what `git show -s --format=full` was
/// reached for: the body, not just the subject.
#[test]
fn history_commit_mode_returns_the_whole_commit() {
    let f = Fixture::new();
    f.write("mod.py", "def alpha():\n    return 1\n");
    f.commit("first");
    f.write("mod.py", "def alpha():\n    return 2\n");
    f.write("added.py", "B = 1\n");
    f.git(&["add", "-A"]);
    f.git(&[
        "commit",
        "--quiet",
        "-m",
        "bump alpha\n\nThe body says why: the old value was wrong.",
    ]);

    let r = f.trace(&["history", "--commit", "HEAD", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(v["mode"], "commit");
    assert_eq!(v["subject"], "bump alpha");
    assert_eq!(
        v["body"], "The body says why: the old value was wrong.",
        "the body is the reason a subject cannot carry: {}",
        r.stdout
    );
    assert_eq!(v["author"], "Tracer Test");
    assert_eq!(
        v["parents"].as_array().unwrap().len(),
        1,
        "a non-merge commit has exactly one parent: {}",
        r.stdout
    );
    let files: std::collections::BTreeMap<&str, &str> = v["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            (
                row["path"].as_str().unwrap(),
                row["status"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(files["mod.py"], "modified");
    assert_eq!(files["added.py"], "added");
    let lines = v["lines"].as_str().unwrap();
    assert!(
        lines.contains("-    return 1") && lines.contains("+    return 2"),
        "the commit's changed lines must be reported: {lines}"
    );

    let unknown = f.trace(&["history", "--commit", "no_such_ref_zzz"]);
    assert_ne!(unknown.code, 0, "an unknown ref must fail loud");
    assert!(
        unknown.combined().contains("commit not found"),
        "{}",
        unknown.combined()
    );
}

/// `--contains` is literal. A pattern with regex syntax in it finds nothing
/// under `-S`, which reads as "this never changed" — `--regex` runs `-G`.
#[test]
fn history_regex_searches_past_changes_by_pattern() {
    let f = Fixture::new();
    f.write("conf.py", "TIMEOUT = 30\n");
    f.commit("add timeout");
    f.write("conf.py", "TIMEOUT = 45\n");
    f.commit("raise timeout");

    let literal = f.trace(&["history", "--contains", "TIMEOUT = [0-9]+", "--json"]);
    literal.ok();
    assert_eq!(
        literal.view()["commits"].as_i64().unwrap(),
        0,
        "a literal search for regex syntax finds nothing: {}",
        literal.stdout
    );

    let regex = f.trace(&[
        "history",
        "--contains",
        "TIMEOUT = [0-9]+",
        "--regex",
        "--json",
    ]);
    regex.ok();
    assert_eq!(
        regex.view()["commits"].as_i64().unwrap(),
        2,
        "--regex must match both commits that touched the value: {}",
        regex.stdout
    );
}

/// `grep --at <ref>` searches a commit. ripgrep reads the disk, so the past
/// was reachable only by `git show <ref>:<path>` — 40,433 times in the
/// census.
#[test]
fn grep_at_ref_searches_a_commit_not_the_worktree() {
    let f = Fixture::new();
    f.write("app.py", "SECRET_TOKEN = 'old'\n");
    f.commit("with the token");
    f.write("app.py", "SECRET_TOKEN = None\n");
    f.commit("token removed");

    let now = f.trace(&["grep", "'old'", ".", "--json"]);
    now.ok();
    assert_eq!(
        now.view()["matches"].as_i64().unwrap(),
        0,
        "the worktree no longer holds it: {}",
        now.stdout
    );

    let past = f.trace(&["grep", "'old'", ".", "--at", "HEAD~1", "--json"]);
    past.ok();
    let v = past.view();
    assert_eq!(v["matches"].as_i64().unwrap(), 1, "{}", past.stdout);
    assert_eq!(v["results"][0]["file"], "app.py");
    assert_eq!(v["results"][0]["line"].as_i64().unwrap(), 1);
    assert!(
        v["results"][0]["snippet"]
            .as_str()
            .unwrap()
            .contains("SECRET_TOKEN"),
        "{}",
        past.stdout
    );
}

#[test]
fn diff_reports_the_repository_that_holds_the_path() {
    let target = standard_repo();
    target.write("src/util.py", "def helper(v):\n    return 0\n");
    let elsewhere = Fixture::new();
    elsewhere.write("other.py", "X = 1\n");
    elsewhere.commit("other");

    let r = elsewhere.trace(&["diff", &target.path("src"), "--json"]);
    r.ok();
    let v = r.view();
    let paths: Vec<&str> = v["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["path"].as_str().unwrap())
        .collect();
    assert_eq!(paths, vec!["src/util.py"], "{}", r.stdout);

    elsewhere
        .trace(&["diff", &target.path("src"), "other.py"])
        .code_is(2);
}

fn git_in(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env_clear()
        .env("PATH", "/opt/homebrew/bin:/usr/bin:/bin:/usr/local/bin")
        .env("HOME", dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn partial_clone_answers_without_fetching_and_still_reads_old_versions() {
    let origin = Fixture::new();
    origin.write("app.py", "VALUE = 'first'\n");
    origin.commit("first");
    origin.write("app.py", "VALUE = 'second'\n");
    origin.write("lib/util.py", "def helper():\n    return 1\n");
    origin.commit("second");
    origin.git(&["config", "uploadpack.allowFilter", "true"]);
    origin.git(&["config", "uploadpack.allowAnySHA1InWant", "true"]);
    let clone = origin.root.join("partial");
    origin.git(&[
        "clone",
        "--quiet",
        "--filter=tree:0",
        &format!("file://{}", origin.root.display()),
        clone.to_str().unwrap(),
    ]);

    let objects = git_in(&clone, &["count-objects", "-v"]);
    let r = trace(&clone, &["grep", "helper", "."]);
    r.ok();
    assert!(r.stdout.contains("lib/util.py"), "{}", r.stdout);
    assert_eq!(git_in(&clone, &["count-objects", "-v"]), objects, "a background scan fetched");
    let stored_history = std::fs::read_dir(schema_directory(&clone, PUBLISHED_SCHEMA_VERSION))
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("git_activity_v2__"))
        .count();
    assert_eq!(stored_history, 0, "an unfinished history walk was stored");

    let old = trace(&clone, &["read", "app.py", "--at", "HEAD~1", "--raw"]);
    old.ok();
    assert!(old.stdout.contains("VALUE = 'first'"), "{}", old.stdout);
}
