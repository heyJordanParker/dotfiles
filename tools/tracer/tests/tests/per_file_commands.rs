//! Per-file commands: doctor, read, info, structure, tree, list, survey,
//! context (file mode). Correctness on both human and `--json` output.

use tracer_cli_tests::{normalize_age, standard_repo, Fixture};

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
fn shoulder_carries_first_seen_range_and_changed_together() {
    // Proves the two newest signals on the canonical shoulder: the age field
    // spans `first_seen → last_modified` (created→modified) when the file's
    // first and last commits fall in different age buckets, and the
    // `together:` field carries the co-changed files. Two dated commits make
    // the range deterministic: the file is created ~400 days ago (a `1y…`
    // bucket) alongside a sibling, then modified today.
    let f = Fixture::new();
    f.write("core.py", "def feature(x):\n    if x:\n        return 1\n    return 0\n");
    f.write("sibling.py", "x = 1\n");
    // First commit: long ago, both files together (the co-change pair).
    git_at_date(&f.root, "2024-01-01T12:00:00", &["add", "-A"]);
    git_at_date(&f.root, "2024-01-01T12:00:00", &["commit", "--quiet", "-m", "create core + sibling"]);
    // Second commit: today, touching core.py only — moves last_modified to
    // the `today` bucket while first_seen stays in the old bucket.
    f.write("core.py", "def feature(x):\n    if x:\n        return 2\n    return 0\n");
    f.commit("touch core today");

    let r = f.trace(&["read", "core.py", "--json"]);
    r.ok();
    let v = r.json();
    let shoulder = v["passive_context"].as_str().unwrap();
    // first_seen drives the left side of the age range: an old created bucket
    // joined by `→` to the fresh modified bucket. The exact old bucket is
    // time-relative (≈1y); assert the structural range form and that the
    // right side is a fresh-commit bucket.
    assert!(
        shoulder.contains("\u{00b7} age: ")
            && shoulder.contains('\u{2192}')
            && (shoulder.contains("\u{2192}today") || shoulder.contains("\u{2192}1d")),
        "shoulder age must span first_seen \u{2192} last_modified: {shoulder:?}"
    );
    // co_changed surfaces as `together: sibling.py` (the only co-change pair).
    assert!(
        shoulder.contains("\u{00b7} together: sibling.py \u{00b7}"),
        "shoulder must carry the changed-together file from co_changed: {shoulder:?}"
    );
}

#[test]
fn shoulder_churn_velocity_diverges_from_lifetime_total() {
    // The churn field carries BOTH the lifetime commit total and the recent
    // velocity (commits in the last 30 days) — and they are independent
    // signals. A file with three lifetime commits, only one of them recent,
    // must read `churn: 3 commits, 1/30d`: the velocity is NOT the total. This
    // pins the velocity signal in the case that actually exercises it (an old,
    // settled file with one recent touch), not the same-run fixture where
    // total and velocity coincide.
    let f = Fixture::new();
    // Two old commits, well outside the 30-day window.
    f.write("svc.py", "def a(x):\n    if x:\n        return 1\n    return 0\n");
    git_at_date(&f.root, "2024-01-01T12:00:00", &["add", "-A"]);
    git_at_date(&f.root, "2024-01-01T12:00:00", &["commit", "--quiet", "-m", "create"]);
    f.write("svc.py", "def a(x):\n    if x:\n        return 2\n    return 0\n");
    git_at_date(&f.root, "2024-02-01T12:00:00", &["add", "-A"]);
    git_at_date(&f.root, "2024-02-01T12:00:00", &["commit", "--quiet", "-m", "old edit"]);
    // One recent commit, today — inside the 30-day window.
    f.write("svc.py", "def a(x):\n    if x:\n        return 3\n    return 0\n");
    f.commit("recent edit");

    let r = f.trace(&["read", "svc.py", "--json"]);
    r.ok();
    let v = r.json();
    let shoulder = v["passive_context"].as_str().unwrap();
    assert!(
        shoulder.contains("\u{00b7} churn: 3 commits, 1/30d \u{00b7}"),
        "churn must carry lifetime total 3 and recent velocity 1 (they diverge): {shoulder:?}"
    );
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
        assert!(r.stdout.contains(bin), "doctor omitted {bin}:\n{}", r.stdout);
    }
}

#[test]
fn read_whole_file_human_has_passive_context_and_line_numbers() {
    let f = standard_repo();
    let r = f.trace(&["read", "src/util.py"]);
    r.ok();
    assert!(r.stdout.contains("# src/util.py"), "{}", r.stdout);
    assert!(r.stdout.contains("[git:"), "missing passive shoulder:\n{}", r.stdout);
    assert!(r.stdout.contains("def helper"), "{}", r.stdout);
    assert!(r.stdout.contains("L1"), "missing line numbering:\n{}", r.stdout);
}

#[test]
fn read_json_shape_is_stable() {
    let f = standard_repo();
    let r = f.trace(&["read", "src/util.py", "--json"]);
    r.ok();
    let v = r.json();
    assert_eq!(v["file"], "src/util.py");
    assert_eq!(v["source"], "worktree");
    // src/util.py is a known four-line file; `read` line-numbers it as
    // L1..L4. The passive_context shoulder is the settled single-commit
    // hermetic string: new (1 commit), local-only, churn of one commit (in
    // the last 30 days), CCN 2 (one `if`), the three files committed in the
    // same "init standard repo" commit as the top changed-together set
    // (co_changed surfacing in the shoulder), the fixed author, the init
    // subject. The age component is normalized; the rest is pinned exactly.
    assert_eq!(
        v["content"].as_str().unwrap(),
        "L1: def helper(v):\nL2:     if v > 0:\nL3:         return v + 1\nL4:     return 0\n",
        "read content must be the exact line-numbered fixture file: {}",
        v["content"]
    );
    assert_eq!(
        normalize_age(v["passive_context"].as_str().unwrap()),
        "[git: new (1 commit) \u{00b7} age: <AGE> \u{00b7} presence: local-only \u{00b7} churn: 1 commit, 1/30d \u{00b7} ccn: 2 low \u{00b7} together: readme.md, widget.php, pyproject.toml \u{00b7} owner: Tracer Test \u{00b7} last: init standard repo]",
        "read passive_context must be the exact settled shoulder: {}",
        v["passive_context"]
    );
    assert!(v.get("nested_memories").is_none());
}

#[test]
fn read_method_scopes_to_one_function() {
    // app.py's main() spans L5-L12 (def..return None). Extraction must
    // return exactly that span, line-numbered from 5, with the leading
    // imports excluded and no trailing code.
    let f = standard_repo();
    let r = f.trace(&["read", "src/app.py", "main", "--json"]);
    r.ok();
    let v = r.json();
    assert_eq!(v["method"], "main");
    let body = v["content"].as_str().unwrap();
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
    let r = f.trace(&["read", "m.py", "target", "--at", "HEAD", "--json"]);
    r.ok();
    let v = r.json();
    assert_eq!(v["source"], "ref");
    assert_eq!(v["method"], "target");
    let body = v["content"].as_str().unwrap();
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
    // Two positional files with a shared scope (--lines) must come back
    // under a `files` array, one payload each, in argument order, each
    // carrying that file's own clamped content.
    let f = Fixture::new();
    f.write("a.py", "AONE = 1\nATWO = 2\nATHREE = 3\n");
    f.write("b.py", "BONE = 10\nBTWO = 20\nBTHREE = 30\n");
    f.commit("two files");
    let r = f.trace(&["read", "a.py", "b.py", "--lines", "1:2", "--json"]);
    r.ok();
    let v = r.json();
    let files = v["files"]
        .as_array()
        .expect("multi-file read must nest payloads under `files`");
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
    let v = r.json();
    assert_eq!(v["lines"][0], 2);
    assert_eq!(v["lines"][1], 3);
    let content = v["content"].as_str().unwrap();
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
    let v = r.json();
    assert_eq!(v["lines"][0], 3);
    assert_eq!(v["lines"][1], 99);
    let content = v["content"].as_str().unwrap();
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
    let v = r.json();
    assert_eq!(v["lines"][0], 10);
    assert_eq!(v["lines"][1], 20);
    assert_eq!(
        v["content"].as_str().unwrap(),
        "",
        "a range entirely past EOF must yield empty content, got:\n{}",
        v["content"]
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
fn read_at_ref_reads_committed_content() {
    let f = Fixture::new();
    f.write("v.py", "VALUE = 1\n");
    f.commit("v1");
    f.write("v.py", "VALUE = 2\n");
    let r = f.trace(&["read", "v.py", "--at", "HEAD", "--json"]);
    r.ok();
    let v = r.json();
    assert_eq!(v["source"], "ref");
    assert!(
        v["content"].as_str().unwrap().contains("VALUE = 1"),
        "ref read returned worktree content:\n{}",
        v["content"]
    );
}

#[test]
fn read_diff_requires_at_ref() {
    let f = standard_repo();
    let r = f.trace(&["read", "src/util.py", "--diff"]);
    r.code_is(2);
    assert!(r.combined().contains("--diff requires --at"), "{}", r.combined());
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
    let v = r.json();
    let d = &v["symbol_diff"];
    assert!(!d.is_null(), "symbol_diff missing on a supported file:\n{v}");

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
    assert_eq!(added["line"].as_i64().unwrap(), 7, "fresh is at worktree L7: {added}");
    let removed = d["removed"][0].clone();
    assert_eq!(removed["name"], "gone");
    assert_eq!(removed["kind"], "function");
    assert_eq!(removed["line"].as_i64().unwrap(), 4, "gone was at committed L4: {removed}");
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
            "import os\n",          // L1
            "# region start\n",     // L2  <- start anchor
            "def inside():\n",      // L3
            "    return os\n",      // L4
            "# region end\n",       // L5  <- end anchor
            "TRAILING = 1\n",       // L6
        ),
    );
    f.commit("anchored");
    let r = f.trace(&[
        "read", "block.py", "--between", "region start", "region end", "--json",
    ]);
    r.ok();
    let v = r.json();
    assert_eq!(v["between"][0], "region start");
    assert_eq!(v["between"][1], "region end");
    assert_eq!(
        v["between_resolved_lines"][0], 2,
        "section must start at the start-anchor line: {v}"
    );
    assert_eq!(
        v["between_resolved_lines"][1], 5,
        "section must end at the end-anchor line (inclusive): {v}"
    );
    let content = v["content"].as_str().unwrap();
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
    let v = r.json();
    assert_eq!(v["file"].as_str().unwrap().ends_with("src/app.py"), true);
    // standard_repo()'s main(x): base 1 + if(1) + for(1) + if(1) = 4,
    // exactly one function. Hand-verifiable McCabe — no lower bound.
    assert_eq!(v["function_count"].as_i64().unwrap(), 1);
    assert_eq!(
        v["cyclomatic_complexity_total"].as_i64().unwrap(),
        4,
        "main(): if + for + if over base 1 = 4: {}",
        v["cyclomatic_complexity_total"]
    );
    assert_eq!(v["cyclomatic_complexity_max"].as_i64().unwrap(), 4);
    let main_fn = v["functions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["name"].as_str() == Some("main"))
        .expect("main present");
    assert_eq!(main_fn["cyclomatic_complexity"].as_i64().unwrap(), 4);
    // repo_context over the fixed 8-entry fixture tree is deterministic.
    assert_eq!(
        v["repo_context"],
        serde_json::json!({"total_files": 8, "median_file_ccn": 0, "complexity_p95": 1}),
        "info file repo_context must be exact for the known fixture: {}",
        v["repo_context"]
    );
}

#[test]
fn info_directory_aggregates_files() {
    let f = standard_repo();
    let r = f.trace(&["info", "src", "--json"]);
    r.ok();
    let v = r.json();
    assert!(v["directory"].as_str().unwrap().ends_with("src"));
    // src holds exactly app.py, util.py, front.tsx, consts.ts.
    assert_eq!(
        v["file_count"].as_i64().unwrap(),
        4,
        "src has app.py, util.py, front.tsx, consts.ts: {}",
        v["file_count"]
    );
    // Exact aggregate: app.py 4 + util.py 2 + front.tsx 2 + consts.ts 0.
    assert_eq!(
        v["cyclomatic_complexity_total"].as_i64().unwrap(),
        8,
        "src aggregate CCN (4+2+2+0): {}",
        v["cyclomatic_complexity_total"]
    );
    // Each files[] entry also carries `abs_path` — the fixture's temp
    // directory path, which varies per run; the rest of every entry is
    // fully determinable. Pin the exact deterministic projection (all
    // fields except abs_path) plus the deterministic repo_context.
    let files_proj: Vec<serde_json::Value> = v["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            serde_json::json!({
                "file": e["file"],
                "loc": e["loc"],
                "cyclomatic_complexity_total": e["cyclomatic_complexity_total"],
                "function_count": e["function_count"],
                "rank": e["rank"],
                "passive_context": normalize_age(e["passive_context"].as_str().unwrap()),
            })
        })
        .collect();
    assert_eq!(
        serde_json::Value::Array(files_proj),
        serde_json::json!([
            {"file": "app.py",   "loc": 8, "cyclomatic_complexity_total": 4, "function_count": 1, "rank": "low", "passive_context": "[git: new (1 commit) \u{00b7} age: <AGE> \u{00b7} churn: 1 commit, 1/30d \u{00b7} ccn: 4 low]"},
            {"file": "consts.ts","loc": 1, "cyclomatic_complexity_total": 0, "function_count": 0, "rank": "low", "passive_context": "[git: new (1 commit) \u{00b7} age: <AGE> \u{00b7} churn: 1 commit, 1/30d \u{00b7} ccn: 0 low]"},
            {"file": "front.tsx","loc": 3, "cyclomatic_complexity_total": 2, "function_count": 1, "rank": "low", "passive_context": "[git: new (1 commit) \u{00b7} age: <AGE> \u{00b7} churn: 1 commit, 1/30d \u{00b7} ccn: 2 low]"},
            {"file": "util.py",  "loc": 4, "cyclomatic_complexity_total": 2, "function_count": 1, "rank": "low", "passive_context": "[git: new (1 commit) \u{00b7} age: <AGE> \u{00b7} churn: 1 commit, 1/30d \u{00b7} ccn: 2 low]"}
        ]),
        "info directory files[] (minus the per-run abs_path) must be exact: {}",
        v["files"]
    );
    // exempt-(a): every entry's abs_path is the fixture temp dir, distinct
    // per run; the tightest stable invariant is that it ends with the
    // base-relative file path.
    for e in v["files"].as_array().unwrap() {
        let abs = e["abs_path"].as_str().unwrap();
        let file = e["file"].as_str().unwrap();
        assert!(
            abs.ends_with(&format!("src/{file}")),
            "abs_path must end with src/{file}: {abs}"
        );
    }
    assert_eq!(
        v["repo_context"],
        serde_json::json!({"total_files": 8, "median_file_ccn": 0, "complexity_p95": 1}),
        "info directory repo_context must be exact: {}",
        v["repo_context"]
    );
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
        brief.stdout.contains("Functions (top 3 by complexity of 8):"),
        "--brief header must name the top-3-of-8 truncation exactly:\n{}",
        brief.stdout
    );
    assert!(
        brief.stdout.contains("… 5 more (use --full to see all)"),
        "--brief footer must report exactly the 5 hidden functions:\n{}",
        brief.stdout
    );
}

#[test]
fn structure_lists_imports_and_symbols_json() {
    let f = standard_repo();
    let r = f.trace(&["structure", "src/app.py", "--json"]);
    r.ok();
    let v = r.json();
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
    assert_eq!(v["symbol_count"].as_i64().unwrap(), 1, "app.py has one symbol: {}", v);
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
            "import os\n",                  // L1
            "\n",                           // L2
            "def public_fn(x):\n",          // L3
            "    def nested():\n",          // L4  (must NOT be exported)
            "        return 1\n",           // L5
            "    return nested()\n",        // L6
            "\n",                           // L7
            "class PublicClass:\n",         // L8
            "    def method(self):\n",      // L9  (method, not a module export)
            "        return 2\n",           // L10
        ),
    );
    f.commit("api");
    let r = f.trace(&["structure", "api.py", "--json"]);
    r.ok();
    let v = r.json();
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
    let v = r.json();
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
        v["symbol_count"].as_i64().unwrap(),
        2,
        "symbol_count must be exactly the two ctags symbols: {}",
        v
    );
}

#[test]
fn tree_json_carries_repo_context_and_ranks() {
    let f = standard_repo();
    let r = f.trace(&["tree", "src", "--json"]);
    r.ok();
    let v = r.json();
    // standard_repo()'s src/ holds exactly four files; every field below is
    // hand-verifiable from the fixture source. tree's files[] carries no
    // absolute path (unlike `info` directory mode), so the whole array is
    // deterministic and pinned exactly. passive_context is the compact
    // single-commit hermetic shoulder "new (1 commit) · <age>"; the age
    // token is normalized via normalize_age (the one exempt-(a) axis) and
    // everything else is exact.
    let files_norm: Vec<serde_json::Value> = v["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            serde_json::json!({
                "path": e["path"],
                "ccn_total": e["ccn_total"],
                "ccn_max_function": e["ccn_max_function"],
                "loc": e["loc"],
                "rank": e["rank"],
                "passive_context": normalize_age(e["passive_context"].as_str().unwrap()),
            })
        })
        .collect();
    assert_eq!(
        serde_json::Value::Array(files_norm),
        serde_json::json!([
            {"path": "app.py",   "ccn_total": 4, "ccn_max_function": 4, "loc": 8, "rank": "low", "passive_context": "[git: new (1 commit) \u{00b7} age: <AGE> \u{00b7} churn: 1 commit, 1/30d \u{00b7} ccn: 4 low]"},
            {"path": "consts.ts","ccn_total": 0, "ccn_max_function": 0, "loc": 1, "rank": "low", "passive_context": "[git: new (1 commit) \u{00b7} age: <AGE> \u{00b7} churn: 1 commit, 1/30d \u{00b7} ccn: 0 low]"},
            {"path": "front.tsx","ccn_total": 2, "ccn_max_function": 2, "loc": 3, "rank": "low", "passive_context": "[git: new (1 commit) \u{00b7} age: <AGE> \u{00b7} churn: 1 commit, 1/30d \u{00b7} ccn: 2 low]"},
            {"path": "util.py",  "ccn_total": 2, "ccn_max_function": 2, "loc": 4, "rank": "low", "passive_context": "[git: new (1 commit) \u{00b7} age: <AGE> \u{00b7} churn: 1 commit, 1/30d \u{00b7} ccn: 2 low]"}
        ]),
        "tree files[] must be the exact four-file fixture set: {}",
        v["files"]
    );
    // repo_context over the whole 8-entry fixture tree: median CCN 0,
    // p95 1 — deterministic for this fixed source.
    assert_eq!(
        v["repo_context"],
        serde_json::json!({"total_files": 8, "median_file_ccn": 0, "complexity_p95": 1}),
        "tree repo_context must be exact for the known fixture: {}",
        v["repo_context"]
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
    let v = r.json();
    let mut paths: Vec<String> = v["files"]
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
        v["files"]
    );
}

#[test]
fn list_directory_json_separates_dirs_and_files() {
    let f = standard_repo();
    let r = f.trace(&["list", ".", "--json"]);
    r.ok();
    let v = r.json();
    // standard_repo()'s root has exactly three sub-directories (docs, lib,
    // src) and one top-level file (pyproject.toml). Every field is
    // hand-verifiable: file_count per dir, the aggregate ccn (docs 0,
    // lib widget.php 3, src 4+2+2+0=8). last_modified is the single
    // hermetic commit's date — see the assertion below for why it is the
    // one field asserted by shape, not value.
    let dirs = v["directories"].as_array().unwrap();
    let dir_proj: Vec<serde_json::Value> = dirs
        .iter()
        .map(|d| {
            serde_json::json!({
                "name": d["name"],
                "file_count": d["file_count"],
                "ccn_total": d["ccn_total"],
                "has_uncommitted": d["has_uncommitted"],
            })
        })
        .collect();
    assert_eq!(
        serde_json::Value::Array(dir_proj),
        serde_json::json!([
            {"name": "docs", "file_count": 1, "ccn_total": 0, "has_uncommitted": false},
            {"name": "lib",  "file_count": 1, "ccn_total": 3, "has_uncommitted": false},
            {"name": "src",  "file_count": 4, "ccn_total": 8, "has_uncommitted": false}
        ]),
        "list directories must be the exact three-dir fixture set: {}",
        v["directories"]
    );
    // exempt-(a): last_modified is the calendar date of the fixture's
    // commit, generated at test run time; it shifts day-to-day and at the
    // UTC boundary, so the tightest stable invariant is the YYYY-MM-DD
    // shape, asserted here for every directory entry.
    for d in dirs {
        let lm = d["last_modified"].as_str().unwrap();
        assert!(
            lm.len() == 10 && lm.as_bytes()[4] == b'-' && lm.as_bytes()[7] == b'-'
                && lm.chars().filter(|c| *c == '-').count() == 2
                && lm.chars().all(|c| c.is_ascii_digit() || c == '-'),
            "last_modified must be a YYYY-MM-DD date: {lm}"
        );
    }
    let root_files: Vec<serde_json::Value> = v["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            serde_json::json!({
                "name": e["name"],
                "rank": e["rank"],
                "ccn_total": e["ccn_total"],
                "passive_context": normalize_age(e["passive_context"].as_str().unwrap()),
            })
        })
        .collect();
    assert_eq!(
        serde_json::Value::Array(root_files),
        serde_json::json!([
            {"name": "pyproject.toml", "rank": "low", "ccn_total": 0, "passive_context": "[git: new (1 commit) \u{00b7} age: <AGE> \u{00b7} churn: 1 commit, 1/30d \u{00b7} ccn: 0 low]"}
        ]),
        "list root files must be exactly pyproject.toml: {}",
        v["files"]
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
    let v = r.json();

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
        pkg["file_count"].as_i64().unwrap(),
        2,
        "pkg must aggregate inner_file.py + sub/deep_file.py: {pkg}"
    );
}

#[test]
fn survey_json_has_distribution_and_languages() {
    let f = standard_repo();
    let r = f.trace(&["survey", ".", "--json"]);
    r.ok();
    let v = r.json();
    // standard_repo() commits exactly 7 files; scc's per-language counts,
    // loc and complexity over that fixed tree are deterministic.
    assert_eq!(v["total_files"].as_i64().unwrap(), 7, "standard_repo has 7 files: {}", v);
    let lang = |name: &str, files: i64, loc: i64, cx: i64, v: &serde_json::Value| {
        let l = &v["languages"][name];
        assert_eq!(l["files"].as_i64().unwrap(), files, "{name} files: {}", v);
        assert_eq!(l["loc"].as_i64().unwrap(), loc, "{name} loc: {}", v);
        assert_eq!(l["complexity"].as_i64().unwrap(), cx, "{name} complexity: {}", v);
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
    assert_eq!(d["median"].as_i64().unwrap(), 0, "distribution.median: {}", v);
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
                e["loc"].as_i64().unwrap(),
                e["complexity"].as_i64().unwrap(),
            )
        })
        .collect();
    let mut got = rows.clone();
    got.sort();
    let mut want = vec![
        ("app.py", "Python", 9, 3),
        ("util.py", "Python", 4, 1),
        ("widget.php", "PHP", 8, 1),
        ("front.tsx", "TypeScript", 4, 0),
        ("consts.ts", "TypeScript", 1, 0),
        ("readme.md", "Markdown", 2, 0),
        ("pyproject.toml", "TOML", 3, 0),
    ];
    want.sort();
    assert_eq!(
        got, want,
        "survey top_complex must list exactly these files with exact loc+complexity: {}",
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
            w[0], w[1], v
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

#[test]
fn docs_command_returns_deduped_ancestor_set_human() {
    let f = docs_repo();
    let r = f.trace(&["docs", "sub/util.py"]);
    r.ok();
    assert!(
        r.stdout.contains("# docs · sub/util.py"),
        "missing header:\n{}",
        r.stdout
    );
    assert!(r.stdout.contains("Root rules"), "missing root doc:\n{}", r.stdout);
    assert!(r.stdout.contains("Sub rules"), "missing sub doc:\n{}", r.stdout);
}

#[test]
fn docs_command_json_shape() {
    let f = docs_repo();
    let r = f.trace(&["docs", "sub/util.py", "--json"]);
    r.ok();
    let v = r.json();
    assert_eq!(v["path"], "sub/util.py");
    assert_eq!(v["directory_scoped"], false);
    assert_eq!(v["doc_count"], 2);
    let docs = v["docs"].as_array().unwrap();
    assert!(docs.iter().any(|d| d["content"]
        .as_str()
        .unwrap()
        .contains("Root rules")));
}

#[test]
fn docs_command_directory_scoped() {
    let f = docs_repo();
    let r = f.trace(&["docs", "sub", "--directory", "--json"]);
    r.ok();
    let v = r.json();
    assert_eq!(v["directory_scoped"], true);
    // docs_repo() plants exactly two ancestor Claude.md docs (root + sub/);
    // directory-scoped `docs sub` resolves both, so the count is exactly 2.
    assert_eq!(
        v["doc_count"].as_i64().unwrap(),
        2,
        "directory-scoped docs over sub/ must resolve exactly Claude.md + sub/Claude.md: {}",
        v
    );
    let paths: Vec<&str> = v["docs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["path"].as_str().unwrap())
        .collect();
    assert_eq!(
        paths,
        vec!["Claude.md", "sub/Claude.md"],
        "directory-scoped docs paths must be the exact ancestor set: {:?}",
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
    assert!(r.stdout.contains("def helper"), "code body missing:\n{}", r.stdout);
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
        def.json().get("nested_memories").is_none(),
        "default read --json must not surface a nested_memories field:\n{}",
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
        on.json().get("nested_memories").is_some(),
        "--docs --json must include nested_memories:\n{}",
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

    // Same session: docs already surfaced by `read --docs` must not have
    // their content re-emitted; they appear in the `already in context`
    // section instead.
    let second = f.trace_env(&["docs", "sub/util.py"], &env);
    second.ok();
    assert!(
        !second.stdout.contains("Root rules") && !second.stdout.contains("Sub rules"),
        "docs re-surfaced content for a doc already in the session manifest:\n{}",
        second.stdout
    );
    assert!(
        second.stdout.contains("already in context"),
        "expected the already-in-context section listing the skipped docs:\n{}",
        second.stdout
    );
}

#[test]
fn context_file_mode_second_touch_collapses_to_the_shoulder() {
    // File mode's headline is the passive-context shoulder. On a file's FIRST
    // surfacing in a session it also emits the once-per-session methods +
    // directory-listing lines; on the SECOND surfacing those are deduped away
    // and the output collapses to exactly the shoulder. This pins both the
    // shoulder-as-headline contract and the per-session first-touch dedup.
    let f = standard_repo();
    // Warm the cache so graph counts are populated.
    f.trace(&["cache", "build", "."]).ok();
    let sid = fresh_session_id("ctx-file-dedup");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    // First touch: shoulder + symbols + dir listing (more than one line).
    let first = f.trace_env(&["context", "src/app.py"], &env);
    first.ok();
    assert!(
        first.stdout.contains("[symbols:") && first.stdout.contains("[dir src/:"),
        "first surfacing must carry the methods and directory-listing lines:\n{}",
        first.stdout
    );

    // Second touch in the same session: collapses to the single shoulder line.
    let second = f.trace_env(&["context", "src/app.py"], &env);
    second.ok();
    let line = second.stdout.trim();
    assert!(line.starts_with("[git:"), "unexpected shoulder:\n{line}");
    assert_eq!(
        line.lines().count(),
        1,
        "second surfacing must collapse to one shoulder line:\n{line}"
    );
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
fn first_touch_of_a_file_surfaces_its_symbols() {
    // The first time a file is surfaced in a session, its passive context
    // includes a `[symbols: …]` line naming the file's declarations, so one
    // touch gives the agent the file's shape without a second structure call.
    let f = nested_repo();
    let sid = fresh_session_id("first-symbols");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    let r = f.trace_env(&["context", "app/controllers/orders.py"], &env);
    r.ok();
    assert!(
        r.stdout.contains("[symbols: create(x) ccn=2; cancel() ccn=1]"),
        "first surfacing must list the file's declarations, with signatures, in source order:\n{}",
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
        r.stdout.contains("[dir controllers/: orders.py, users.py]"),
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
        r.stdout.contains("[dir controllers/:"),
        "the immediate parent directory must be listed:\n{}",
        r.stdout
    );
    assert!(
        !r.stdout.contains("[dir app/:"),
        "ancestor directory app/ must NOT be listed (non-recursive rule):\n{}",
        r.stdout
    );
    // Exactly one directory line — never the ancestor chain.
    let dir_lines = r.stdout.lines().filter(|l| l.starts_with("[dir ")).count();
    assert_eq!(
        dir_lines, 1,
        "exactly one directory line (the immediate parent), never the chain:\n{}",
        r.stdout
    );
}

#[test]
fn directory_listing_surfaces_once_per_session_across_neighbours() {
    // The directory listing is deduped per session: surfacing a second file in
    // the SAME directory does not repeat that directory's listing, while the
    // second file still gets its own first-touch symbols line.
    let f = nested_repo();
    let sid = fresh_session_id("dir-dedup");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    let first = f.trace_env(&["context", "app/controllers/orders.py"], &env);
    first.ok();
    assert!(
        first.stdout.contains("[dir controllers/:"),
        "first file in the directory must surface the listing:\n{}",
        first.stdout
    );

    let second = f.trace_env(&["context", "app/controllers/users.py"], &env);
    second.ok();
    assert!(
        !second.stdout.contains("[dir controllers/:"),
        "a neighbour in the already-surfaced directory must NOT repeat the listing:\n{}",
        second.stdout
    );
    assert!(
        second.stdout.contains("[symbols: show() ccn=1]"),
        "the neighbour's own first-touch symbols line must still surface:\n{}",
        second.stdout
    );
}

#[test]
fn context_directory_argument_surfaces_its_one_level_listing() {
    // Surfacing a directory directly (not via a file inside it) emits that one
    // directory's one-level listing on first touch, deduped on the second.
    let f = nested_repo();
    let sid = fresh_session_id("dir-arg");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    let first = f.trace_env(&["context", "app/controllers"], &env);
    first.ok();
    assert_eq!(
        first.stdout.trim(),
        "[dir controllers/: orders.py, users.py]",
        "a directly-touched directory must emit exactly its one-level listing:\n{}",
        first.stdout
    );

    let second = f.trace_env(&["context", "app/controllers"], &env);
    second.ok();
    assert!(
        second.stdout.trim().is_empty(),
        "a second touch of the same directory must emit nothing:\n{}",
        second.stdout
    );
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
    let v = r.json();
    let cls = find_symbol(&v, "Funnel");
    assert_eq!(cls["kind"].as_str().unwrap(), "class", "{}", cls);
    assert_eq!(cls["extends"].as_str().unwrap(), "Model", "{}", cls);
    assert_eq!(
        cls["implements"].as_array().unwrap(),
        &vec![serde_json::json!("UrlRoutable")],
        "{}",
        cls
    );
    let attrs = cls["attributes"].as_array().expect("class attributes");
    assert_eq!(attrs.len(), 1, "{}", cls);
    assert_eq!(attrs[0]["name"].as_str().unwrap(), "Entity", "{}", cls);
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
    let v = r.json();
    let m = find_symbol(&v, "validateSlug");
    assert_eq!(m["visibility"].as_str().unwrap(), "public", "{}", m);
    assert_eq!(m["return_type"].as_str().unwrap(), "?string", "{}", m);
    let mods: Vec<&str> = m["modifiers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap())
        .collect();
    assert!(mods.contains(&"public") && mods.contains(&"static"), "{}", m);
    let params = m["parameters"].as_array().expect("parameters");
    assert_eq!(params.len(), 2, "{}", m);
    assert_eq!(params[0]["name"].as_str().unwrap(), "slug", "{}", m);
    assert_eq!(params[0]["type"].as_str().unwrap(), "string", "{}", m);
    assert_eq!(params[1]["name"].as_str().unwrap(), "excludeId", "{}", m);
    assert_eq!(params[1]["type"].as_str().unwrap(), "?int", "{}", m);
    assert_eq!(params[1]["default"].as_str().unwrap(), "null", "{}", m);
    let attrs = m["attributes"].as_array().expect("method attributes");
    assert_eq!(attrs.len(), 1, "{}", m);
    assert_eq!(attrs[0]["name"].as_str().unwrap(), "Route", "{}", m);
    assert!(
        attrs[0]["source"].as_str().unwrap().contains("/x"),
        "attribute source must preserve argument text: {}",
        m
    );
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
    let v = r.json();
    let prop = find_symbol(&v, "name");
    assert_eq!(prop["kind"].as_str().unwrap(), "property", "{}", prop);
    assert_eq!(prop["visibility"].as_str().unwrap(), "public", "{}", prop);
    assert_eq!(prop["type"].as_str().unwrap(), "string", "{}", prop);
    let attrs = prop["attributes"].as_array().expect("property attributes");
    assert_eq!(attrs.len(), 1, "{}", prop);
    assert_eq!(attrs[0]["name"].as_str().unwrap(), "Schema", "{}", prop);
    let hooks = prop["hooks"].as_array().expect("hooks");
    let accessors: Vec<&str> = hooks
        .iter()
        .map(|h| h["accessor"].as_str().unwrap())
        .collect();
    assert_eq!(accessors, vec!["get", "set"], "{}", prop);
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
    let v = r.json();
    let cls = find_symbol(&v, "Svc");
    assert_eq!(cls["type_parameters"].as_str().unwrap(), "<T extends Foo>", "{}", cls);
    let imps: Vec<&str> = cls["implements"]
        .as_array()
        .expect("implements")
        .iter()
        .map(|s| s.as_str().unwrap())
        .collect();
    assert_eq!(imps, vec!["Base", "Other"], "{}", cls);
    let decos = cls["decorators"].as_array().expect("class decorators");
    assert_eq!(decos.len(), 1, "{}", cls);
    assert!(
        decos[0]["source"].as_str().unwrap().starts_with("@Injectable"),
        "{}",
        cls
    );

    let field = find_symbol(&v, "count");
    assert_eq!(field["visibility"].as_str().unwrap(), "private", "{}", field);
    assert_eq!(field["type"].as_str().unwrap(), "number", "{}", field);
    assert_eq!(field["default"].as_str().unwrap(), "0", "{}", field);

    let m = find_symbol(&v, "run");
    assert_eq!(m["visibility"].as_str().unwrap(), "public", "{}", m);
    assert_eq!(m["return_type"].as_str().unwrap(), "Promise<T>", "{}", m);
    assert_eq!(m["async"].as_bool().unwrap(), true, "{}", m);
    let params = m["parameters"].as_array().expect("ts parameters");
    assert_eq!(params.len(), 2, "{}", m);
    assert_eq!(params[0]["name"].as_str().unwrap(), "id", "{}", m);
    assert_eq!(params[0]["type"].as_str().unwrap(), "number", "{}", m);
    let p0_decos = params[0]["decorators"]
        .as_array()
        .expect("parameter decorators");
    assert_eq!(p0_decos.len(), 1, "{}", m);
    assert!(
        p0_decos[0]["source"]
            .as_str()
            .unwrap()
            .starts_with("@Inject"),
        "{}",
        m
    );
    assert_eq!(params[1]["optional"].as_bool().unwrap(), true, "{}", m);
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
    let v = r.json();
    let iface = find_symbol(&v, "User");
    assert_eq!(iface["kind"].as_str().unwrap(), "interface", "{}", iface);
    assert_eq!(iface["type_parameters"].as_str().unwrap(), "<T>", "{}", iface);
    let ext: Vec<&str> = iface["extends"]
        .as_array()
        .expect("extends")
        .iter()
        .map(|s| s.as_str().unwrap())
        .collect();
    assert_eq!(ext, vec!["Base<T>"], "{}", iface);

    let id = find_symbol(&v, "id");
    assert_eq!(id["type"].as_str().unwrap(), "number", "{}", id);
    let name = find_symbol(&v, "name");
    assert_eq!(name["type"].as_str().unwrap(), "string", "{}", name);
    let mods: Vec<&str> = name["modifiers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap())
        .collect();
    assert!(mods.contains(&"readonly"), "{}", name);
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
    let v = r.json();

    let cls = find_symbol(&v, "User");
    let bases: Vec<&str> = cls["bases"]
        .as_array()
        .expect("class bases")
        .iter()
        .map(|s| s.as_str().unwrap())
        .collect();
    assert_eq!(bases, vec!["Base"], "{}", cls);
    let cls_decos = cls["decorators"].as_array().expect("class decorators");
    assert_eq!(cls_decos.len(), 1, "{}", cls);
    assert_eq!(cls_decos[0]["source"].as_str().unwrap(), "@dataclass", "{}", cls);

    let create = find_symbol(&v, "create");
    assert_eq!(create["async"].as_bool().unwrap(), true, "{}", create);
    assert_eq!(
        create["return_type"].as_str().unwrap(),
        "\"User\"",
        "{}",
        create
    );
    let create_decos = create["decorators"].as_array().expect("method decorators");
    assert_eq!(create_decos.len(), 1, "{}", create);
    assert_eq!(
        create_decos[0]["source"].as_str().unwrap(),
        "@classmethod",
        "{}",
        create
    );
    let cparams = create["parameters"].as_array().expect("create parameters");
    assert_eq!(cparams.len(), 3, "{}", create);
    assert_eq!(cparams[1]["type"].as_str().unwrap(), "int", "{}", create);
    assert_eq!(cparams[2]["default"].as_str().unwrap(), "0", "{}", create);

    let free = find_symbol(&v, "free");
    assert_eq!(free["return_type"].as_str().unwrap(), "bool", "{}", free);
    let fparams = free["parameters"].as_array().expect("free parameters");
    assert_eq!(fparams.len(), 4, "{}", free);
    assert_eq!(fparams[1]["default"].as_str().unwrap(), "\"z\"", "{}", free);
    assert_eq!(fparams[2]["variadic"].as_bool().unwrap(), true, "{}", free);
    assert_eq!(
        fparams[3]["keyword_variadic"].as_bool().unwrap(),
        true,
        "{}",
        free
    );
}

#[test]
fn structure_existing_fields_remain_with_their_existing_shapes() {
    // Regression contract: every field structure already emitted (name,
    // kind, line, scope, scope_kind, signature, cyclomatic_complexity)
    // keeps its existing name and semantics after the signature-merge.
    // Any rename or shape change breaks this test.
    let f = standard_repo();
    let r = f.trace(&["structure", "src/app.py", "--json"]);
    r.ok();
    let v = r.json();
    let main = find_symbol(&v, "main");
    assert!(main.get("name").is_some(), "name: {}", main);
    assert!(main.get("kind").is_some(), "kind: {}", main);
    assert!(main.get("line").is_some(), "line: {}", main);
    assert!(main.get("scope").is_some(), "scope: {}", main);
    assert!(main.get("scope_kind").is_some(), "scope_kind: {}", main);
    assert!(main.get("signature").is_some(), "signature: {}", main);
    // cyclomatic_complexity is conditional on a function-kind ctags match;
    // app.py's main() qualifies, so the field must be present and integer.
    assert!(
        main["cyclomatic_complexity"].is_i64(),
        "cyclomatic_complexity must remain an integer: {}",
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
    let v = r.json();

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
    let vt = rt.json();
    let tree_files: Vec<&str> = vt["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["path"].as_str().unwrap())
        .collect();
    assert_eq!(
        tree_files,
        vec!["kept.sh"],
        "tree must agree with the working tree, not the stale index: {}",
        vt["files"]
    );

    // find was already correct and must stay correct.
    let rf = f.trace(&["find", "*.sh", "hooks", "--json"]);
    rf.ok();
    let vf = rf.json();
    let found: Vec<&str> = vf["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["path"].as_str().unwrap())
        .collect();
    assert_eq!(
        found,
        vec!["hooks/kept.sh"],
        "find must return only the on-disk match: {}",
        vf["entries"]
    );
}
