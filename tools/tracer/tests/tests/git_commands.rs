//! Git-archaeology commands: history (whole-file, function, pickaxe modes),
//! blame (file / symbol / lines scopes), diff (file + symbol mode, default
//! and explicit base, rename lifecycle), status (blast-radius ordering).
//!
//! The ordering assertions here are exact, not membership-only: the whole
//! point of these commands is "look at the load-bearing thing first", so a
//! command whose ranking inverted must fail the suite, not pass it.

use tracer_cli_tests::{normalize_age, standard_repo, Fixture};

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
    let v = r.json();
    assert_eq!(v["mode"], "file");
    assert_eq!(v["file"], "mod.py");
    // repo_with_history() commits mod.py exactly twice; both are recent.
    assert_eq!(v["commit_count"].as_i64().unwrap(), 2, "exactly two commits: {}", v);
    assert_eq!(v["commits_30d"].as_i64().unwrap(), 2, "both commits are recent: {}", v);
    assert_eq!(v["last_author"], "Tracer Test");
    assert_eq!(v["last_subject"], "bump alpha, add beta");
    assert_eq!(v["top_author"], "Tracer Test");
    // Newest commit first, then the original.
    let subjects: Vec<&str> = v["recent_commits"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["subject"].as_str().unwrap())
        .collect();
    assert_eq!(subjects, vec!["bump alpha, add beta", "add alpha"], "recent_commits: {}", v);
    for c in v["recent_commits"].as_array().unwrap() {
        assert_eq!(c["author"], "Tracer Test");
    }
    // The fixture's sole author owns all 6 lines of the final file.
    let blame = v["top_blame_authors"].as_array().unwrap();
    assert_eq!(blame.len(), 1, "one author: {}", v);
    assert_eq!(blame[0]["author"], "Tracer Test");
    assert_eq!(blame[0]["lines"].as_i64().unwrap(), 6, "6 lines in mod.py: {}", v);
}

#[test]
fn history_function_mode_returns_symbol_line_history() {
    let f = repo_with_history();
    let r = f.trace(&["history", "mod.py", "alpha", "--json"]);
    r.ok();
    let v = r.json();
    assert_eq!(v["mode"], "function");
    assert_eq!(v["symbol"], "alpha");
    // alpha() was introduced in "add alpha" and its body changed in
    // "bump alpha, add beta": git log -L on alpha sees exactly both,
    // newest first.
    let subjects: Vec<&str> = v["commits"]
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
    let v = r.json();
    assert_eq!(v["mode"], "contains");
    assert_eq!(v["pattern"], "beta");
    // "beta" enters the repo in exactly one commit: the second one. The
    // pickaxe must report that single commit and point at the line + the
    // enclosing symbol where the token appears.
    assert_eq!(v["commit_count"].as_i64().unwrap(), 1, "beta added in one commit: {}", v);
    let commits = v["commits"].as_array().unwrap();
    assert_eq!(commits.len(), 1, "exactly one pickaxe commit: {}", v);
    assert_eq!(commits[0]["subject"], "bump alpha, add beta");
    assert_eq!(commits[0]["author"], "Tracer Test");
    let matches = commits[0]["matches"].as_array().unwrap();
    assert_eq!(matches.len(), 1, "one matching line: {}", v);
    assert_eq!(matches[0]["path"], "mod.py");
    assert_eq!(matches[0]["line"].as_i64().unwrap(), 5, "def beta() is on line 5: {}", v);
    assert_eq!(matches[0]["enclosing_symbol"], "beta");
}

#[test]
fn history_contains_is_mutually_exclusive_with_file() {
    let f = repo_with_history();
    let r = f.trace(&["history", "mod.py", "--contains", "alpha"]);
    // history has optional/multi-mode args: argument-conflict and not-found
    // are explicit runtime errors — non-zero exit with a clear stderr
    // message, not the pathval exit-2 path required-arg commands use.
    assert_ne!(r.code, 0, "expected non-zero exit:\n{}", r.combined());
    assert!(r.combined().contains("mutually exclusive"), "{}", r.combined());
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
    let v = r.json();
    assert_eq!(v["file"], "mod.py");
    assert_eq!(v["scope"], "file");
    // mod.py's final 6 lines blame to exactly two commits: line 1
    // (`def alpha():`, unchanged since "add alpha") and lines 2-6
    // (rewritten/added by "bump alpha, add beta"). Dates are not pinned —
    // a commit made near a UTC day boundary renders a different local
    // calendar date than `git log` does, so the date is environment-
    // dependent; the region partition, authorship and subjects are not.
    assert_eq!(v["region_count"].as_i64().unwrap(), 2, "two blame regions: {}", v);
    assert_eq!(v["line_count"].as_i64().unwrap(), 6, "6 lines: {}", v);
    let regions = v["regions"].as_array().unwrap();
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
    let v = r.json();
    assert_eq!(v["scope"], "symbol");
    assert_eq!(v["symbol"], "beta");
    // beta() occupies lines 5-6 of the final file and was wholly
    // introduced by the second commit, so it blames to a single region
    // spanning exactly that range. (Date is environment-dependent — see
    // blame_whole_file_json_regions — so it is not pinned.)
    assert_eq!(v["line_range"]["start"].as_i64().unwrap(), 5, "beta starts at L5: {}", v);
    assert_eq!(v["line_range"]["end"].as_i64().unwrap(), 6, "beta ends at L6: {}", v);
    assert_eq!(v["region_count"].as_i64().unwrap(), 1, "one region: {}", v);
    assert_eq!(v["line_count"].as_i64().unwrap(), 2, "beta is 2 lines: {}", v);
    let r = &v["regions"][0];
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
    let v = r.json();
    assert_eq!(v["scope"], "lines");
    assert_eq!(v["line_range"]["start"], 1);
    assert_eq!(v["line_range"]["end"], 2);
    // Lines 1-2 straddle the two commits: L1 from "add alpha", L2 from
    // "bump alpha, add beta" — two single-line regions. (Date is
    // environment-dependent, see blame_whole_file_json_regions.)
    assert_eq!(v["region_count"].as_i64().unwrap(), 2, "two regions across L1:2: {}", v);
    assert_eq!(v["line_count"].as_i64().unwrap(), 2);
    let shape: Vec<(i64, i64, &str, &str)> = v["regions"]
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
    assert!(r.combined().contains("mutually exclusive"), "{}", r.combined());
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
    let v = r.json();
    assert_eq!(v["granularity"], "file");
    let paths: Vec<&str> = v["files"]
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
    let v = r.json();
    assert_eq!(v["granularity"], "symbol");
    let symbols: Vec<serde_json::Value> = v["symbols"]
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
        v["symbols"]
    );
}

#[test]
fn diff_unknown_base_ref_exits_2() {
    let f = standard_repo();
    let r = f.trace(&["diff", "--base", "no_such_ref_zzz"]);
    r.code_is(2);
    assert!(r.combined().contains("not found"), "{}", r.combined());
}

// --- Default-base mode -------------------------------------------------
//
// `trace diff` with no --base falls back to the hardcoded
// remote-development ref. That ref never exists in a hermetic fixture, so
// the contract being pinned here is the *absence* path: exit 2, an
// explicit stderr error that names the unresolved ref and tells the
// caller to pass --base. This is the only place the default-base code
// path is exercised at all.

#[test]
fn diff_default_base_unresolvable_exits_2_with_named_ref() {
    let f = standard_repo();
    let r = f.trace(&["diff"]);
    r.code_is(2);
    let out = r.combined();
    // The hardcoded default is the remote-development ref; the message
    // must name the exact ref it tried and could not resolve, and must
    // not be a generic failure.
    assert!(
        out.contains("origin/development"),
        "default-base error must name the unresolved ref:\n{out}"
    );
    assert!(
        out.contains("not found"),
        "default-base error must state the ref was not found:\n{out}"
    );
    assert!(
        out.contains("--base"),
        "default-base error must tell the caller how to recover:\n{out}"
    );
}

#[test]
fn diff_default_base_unresolvable_in_json_mode_still_exits_2() {
    // --json must not change the unresolvable-default contract: the verify
    // gate runs before any value is produced, so the exit code and the
    // stderr message are identical with or without --json, and stdout
    // carries no partial JSON document.
    let f = standard_repo();
    let r = f.trace(&["diff", "--json"]);
    r.code_is(2);
    assert!(
        r.combined().contains("origin/development"),
        "{}",
        r.combined()
    );
    assert!(
        r.stdout.trim().is_empty(),
        "no JSON should be emitted when the default base is unresolvable: {:?}",
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
    let v = r.json();
    assert_eq!(v["granularity"], "file");
    let rows = v["files"].as_array().unwrap();

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
        rows.iter()
            .find(|x| x["path"] == name)
            .unwrap()["direct_dependents"]
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
    let v = r.json();
    assert_eq!(v["granularity"], "symbol");
    let rows = v["symbols"].as_array().unwrap();

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
    let v = r.json();
    let entries = v["entries"].as_array().unwrap();
    let order: Vec<(&str, i64)> = entries
        .iter()
        .filter(|e| matches!(e["path"].as_str().unwrap(), "hub.py" | "mid.py" | "solo.py"))
        .map(|e| (e["path"].as_str().unwrap(), e["callers"].as_i64().unwrap()))
        .collect();
    assert_eq!(
        order,
        vec![("hub.py", 2), ("mid.py", 1), ("solo.py", 0)],
        "blast-radius order wrong; full entries: {}",
        serde_json::to_string_pretty(entries).unwrap()
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
//   3. The inline lifecycle shoulder (here via `status` after a further
//      working-tree edit, and via the settled `diff` row's
//      passive_context) reflects the renamed state, not "new file".

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
    let v = r.json();
    let row = v["files"]
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
    let v = r.json();
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
        v["commit_count"].as_i64().unwrap(),
        2,
        "history must count exactly the two commits (incl. pre-rename): {}",
        v
    );
    assert_eq!(v["last_subject"], "rename old_name -> new_name");
    assert_eq!(v["top_author"], "Tracer Test");
    assert_eq!(v["last_author"], "Tracer Test");
}

#[test]
fn rename_lifecycle_shoulder_reflects_renamed_state() {
    let f = repo_renamed_file();
    f.trace(&["cache", "build", "."]).ok();

    // Settled state: the diff row's passive_context shoulder must label
    // the file as renamed-from the old path, never as a fresh/new file.
    let r = f.trace(&["diff", "--base", "base-ref", "--json"]);
    r.ok();
    let v = r.json();
    let row = v["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["path"] == "new_name.py")
        .unwrap();
    // The settled diff-row shoulder is fully deterministic for this
    // hermetic fixture: renamed-from the prior path, local-only, churn of
    // two commits (both within 30 days of the hermetic commit time), the
    // carried-forward CCN of 2 (feature() has one `if`), the co-changed
    // caller.py (touched in the same rename commit), the fixed hermetic
    // author, and the rename commit's subject. The age component is
    // normalized; the rest is pinned exactly — including the churn and
    // changed-together fields the canonical shoulder now carries.
    assert_eq!(
        normalize_age(row["passive_context"].as_str().unwrap()),
        "[git: renamed-from old_name.py \u{00b7} age: <AGE> \u{00b7} presence: local-only \u{00b7} churn: 2 commits, 2/30d \u{00b7} loc: 4 \u{00b7} ccn: 2 low \u{00b7} together: caller.py \u{00b7} owner: Tracer Test \u{00b7} last: rename old_name -> new_name]",
        "settled rename shoulder must be exact: {}",
        row["passive_context"]
    );

    // Dirty state: a renamed-but-uncommitted move surfaces as the
    // uncommitted-rename lifecycle label in the status shoulder.
    f.git(&["mv", "new_name.py", "third_name.py"]);
    let rs = f.trace(&["status", "--json"]);
    rs.ok();
    let sv = rs.json();
    let renamed_entry = sv["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["state"] == "renamed")
        .expect("uncommitted rename must appear in status as state=renamed");
    // The uncommitted-rename status shoulder is likewise fully
    // deterministic: renamed (uncommitted), local-only, churn of zero (the
    // moved-but-uncommitted path has no commits of its own yet), the one
    // caller (caller.py imports feature), zero dependents, carried CCN 2.
    // No age and no changed-together on this path — pinned exactly.
    assert_eq!(
        renamed_entry["shoulder"].as_str().unwrap(),
        "[git: renamed (uncommitted) \u{00b7} presence: local-only \u{00b7} churn: 0 commits, 0/30d \u{00b7} callers: 1 \u{00b7} dependents: 0 \u{00b7} loc: 4 \u{00b7} ccn: 2 low]",
        "uncommitted-rename shoulder must be exact: {}",
        renamed_entry["shoulder"]
    );
}

#[test]
fn status_clean_tree_reports_clean() {
    let f = standard_repo();
    let r = f.trace(&["status", "--json"]);
    r.ok();
    let v = r.json();
    assert_eq!(v["count"], 0);
    assert!(v["entries"].as_array().unwrap().is_empty());
}

#[test]
fn status_lists_dirty_files_with_intelligence() {
    let f = standard_repo();
    f.trace(&["cache", "build", "."]).ok();
    f.write("src/util.py", "def helper(v):\n    return v + 99\n");
    f.write("newfile.py", "Z = 0\n");
    let r = f.trace(&["status", "--json"]);
    r.ok();
    let v = r.json();
    // The dirty set is exactly three, hand-verifiable: src/util.py
    // (modified), newfile.py (untracked), and the .tracer-cache/ directory
    // the preceding `cache build` wrote (untracked). status orders by
    // blast radius, so util.py (1 caller) leads, then the two zero-impact
    // untracked entries in their stable order.
    assert_eq!(
        v["count"].as_i64().unwrap(),
        3,
        "dirty set must be exactly util.py + newfile.py + .tracer-cache/: {}",
        r.stdout
    );
    let paths: Vec<&str> = v["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["path"].as_str().unwrap())
        .collect();
    assert_eq!(
        paths,
        vec!["src/util.py", ".tracer-cache/", "newfile.py"],
        "status entry order must be blast-radius then stable: {:?}",
        paths
    );
    let entries = v["entries"].as_array().unwrap();
    let modified = entries
        .iter()
        .find(|e| e["path"] == "src/util.py")
        .expect("modified util.py missing");
    assert_eq!(modified["state"], "modified");
    // The rewritten body `def helper(v): return v + 99` is one function
    // with no decision nodes — CCN exactly 1.
    assert_eq!(
        modified["ccn_total"].as_i64().unwrap(),
        1,
        "rewritten branchless helper must have CCN exactly 1: {modified}"
    );
}

#[test]
fn status_state_filter() {
    let f = standard_repo();
    f.write("untracked_only.py", "pass\n");
    let r = f.trace(&["status", "--state", "untracked", "--json"]);
    r.ok();
    let v = r.json();
    for e in v["entries"].as_array().unwrap() {
        assert_eq!(e["state"], "untracked", "state filter leaked: {e}");
    }
}
