//! Freshness contract: every call reflects the bytes on disk at the moment it
//! runs. The caches exist to make that fast, never to make it approximate, so
//! each test here drives one way the fast path could serve a superseded answer
//! and pins the recompute.

use std::process::Command;
use std::thread;
use tracer_cli_tests::{standard_repo, trace, Fixture, Run};

/// Assert one `structure --json` response reached the symbol declaration, not
/// merely text that happened to include the requested name in its filename.
fn assert_structure_symbol(run: &Run, expected: &str) {
    run.ok();
    let document = run.json();
    let exports = document["results"]["exports"]
        .as_array()
        .expect("structure JSON results.exports is an array");
    assert!(
        exports.iter().any(|export| {
            export["name"] == expected && export["kind"] == "function" && export["line"] == 1
        }),
        "structure JSON results.exports omitted {expected:?}: {exports:?}"
    );
}

/// Copy `from`'s mtime onto `to`, the way `touch -r`, `rsync -t`, `cp -p`, and
/// every archive extractor do.
fn copy_mtime(from: &std::path::Path, to: &std::path::Path) {
    let out = Command::new("touch")
        .arg("-r")
        .arg(from)
        .arg(to)
        .output()
        .expect("touch -r spawns");
    assert!(out.status.success(), "touch -r failed");
}

/// Break 1: same size, same mtime, different content. The pre-ctime index
/// matched on `(mtime_ns, size)` alone and served the previous extraction
/// forever — `trace structure` listed the old symbol name against the new
/// file.
#[test]
fn a_same_size_edit_with_a_restored_mtime_is_still_seen() {
    let f = Fixture::new();
    f.write(
        "src/a.py",
        "def alpha(v):\n    if v:\n        return 1\n    return 0\n",
    );
    f.commit("init");

    f.trace(&["structure", "src/a.py"]).ok();

    let stamp = f.write("stamp", "");
    copy_mtime(&f.root.join("src/a.py"), &stamp);
    // Same byte length, different declaration.
    f.write(
        "src/a.py",
        "def gamma(v):\n    if v:\n        return 1\n    return 0\n",
    );
    copy_mtime(&stamp, &f.root.join("src/a.py"));

    let out = f.trace(&["structure", "src/a.py"]).ok().stdout.clone();
    assert!(
        out.contains("gamma") && !out.contains("alpha"),
        "stale extraction served after a same-size edit with a restored mtime:\n{out}"
    );
}

/// The same restored-mtime edit must also reach the relations index, which
/// absorbs each file's declarations off the per-file entry this stat index
/// serves. A stale stamp there means `trace defines` answers from the old
/// bytes.
#[test]
fn the_relations_index_sees_a_restored_mtime_edit_too() {
    let f = Fixture::new();
    f.write(
        "src/a.py",
        "def alpha(v):\n    if v:\n        return 1\n    return 0\n",
    );
    f.commit("init");

    f.trace(&["cache", "build", "."]).ok();
    f.trace(&["defines", "alpha"]).ok();

    let stamp = f.write("stamp", "");
    copy_mtime(&f.root.join("src/a.py"), &stamp);
    f.write(
        "src/a.py",
        "def gamma(v):\n    if v:\n        return 1\n    return 0\n",
    );
    copy_mtime(&stamp, &f.root.join("src/a.py"));

    // `defines` answers from the relations index alone, so that index has to
    // have re-absorbed the file for this to hold.
    f.trace(&["defines", "gamma"]).ok();
    f.trace(&["defines", "alpha"]).code_is(2);
}

#[test]
fn declaration_edits_reresolve_an_untouched_importer() {
    let f = Fixture::new();
    f.write("left.py", "def selected():\n    return 1\n");
    f.write("right.py", "def waiting():\n    return 2\n");
    f.write("cached.py", "def cached_target():\n    return 3\n");
    f.write(
        "caller.py",
        "from missing import selected\n\ndef use():\n    return selected()\n",
    );
    f.commit("initial declarations");
    f.trace(&["cache", "build", "."]).ok();
    let importer_entry = file_fact_entry(&f, "caller.py");
    let mut cached_importer: serde_json::Value = serde_json::from_slice(
        &std::fs::read(&importer_entry).expect("cached importer is readable"),
    )
    .expect("cached importer is JSON");
    cached_importer["extraction"]["imports"][0]["module"] =
        serde_json::Value::String("cached".to_string());
    cached_importer["extraction"]["imports"][0]["symbol"] =
        serde_json::Value::String("cached_target".to_string());
    std::fs::write(
        &importer_entry,
        serde_json::to_vec(&cached_importer).expect("cached importer serializes"),
    )
    .expect("cached importer is distinguishable from source");

    let before = f.trace(&["callers", "left", "--json"]);
    before.ok();
    assert!(module_callers(&before).contains(&"caller.py".to_string()));

    f.write("left.py", "def retired():\n    return 1\n");
    f.write("right.py", "def selected():\n    return 2\n");

    let warm = f.trace(&["callers", "cached", "--json"]);
    warm.ok();
    let warm_callers = module_callers(&warm);
    assert_eq!(
        warm_callers,
        vec!["caller.py".to_string()],
        "warm index did not follow the untouched importer's cached extraction"
    );
    let old_target = f.trace(&["callers", "left", "--json"]);
    old_target.ok();
    assert!(
        !module_callers(&old_target).contains(&"caller.py".to_string()),
        "warm index kept the untouched importer on its old target"
    );
    assert_eq!(
        std::fs::read_to_string(f.root.join("caller.py")).expect("caller source is readable"),
        "from missing import selected\n\ndef use():\n    return selected()\n",
        "the importer source changed during the cached-extraction proof"
    );

    remove_matching(&f.root.join(".tracer-cache/file"), "relations_", "json");
    let fresh = f.trace(&["callers", "cached", "--json"]);
    fresh.ok();
    assert_eq!(
        module_callers(&fresh),
        warm_callers,
        "warm and rebuilt indexes disagreed after declaration edits"
    );
}

#[test]
fn a_v2_relations_index_cannot_override_current_resolution() {
    let f = Fixture::new();
    f.write("a.py", "def selected():\n    return 1\n");
    f.write(
        "caller.py",
        "from a import selected\n\ndef use():\n    return selected()\n",
    );
    f.commit("initial imports");
    f.trace(&["cache", "build", "."]).ok();

    let current = relations_entry(&f);
    let mut poisoned: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&current).expect("relations index is readable"))
            .expect("relations index is JSON");
    let file = poisoned["files"]
        .as_array()
        .and_then(|files| files.iter().position(|path| path == "a.py"))
        .expect("edges table includes a.py");
    poisoned["built"][file][0] = serde_json::Value::String("wrong-content-key".to_string());
    std::fs::write(
        &current,
        serde_json::to_vec(&poisoned).expect("poisoned v2 index serializes"),
    )
    .expect("poisoned edges index is written");

    let callers = f.trace(&["callers", "a", "--json"]);
    callers.ok();
    assert_eq!(
        module_callers(&callers),
        vec!["caller.py".to_string()],
        "a stale relations provenance entry overrode current import resolution"
    );
}

fn module_callers(run: &Run) -> Vec<String> {
    run.view()["results"]
        .as_array()
        .expect("callers results are rows")
        .iter()
        .flat_map(|row| row["callers"].as_array().into_iter().flatten())
        .filter_map(|caller| caller["source_file"].as_str().map(str::to_string))
        .collect()
}

fn file_fact_entry(fixture: &Fixture, relative_path: &str) -> std::path::PathBuf {
    let directory = fixture.root.join(".tracer-cache/file");
    let index = std::fs::read_dir(&directory)
        .expect("file cache exists")
        .flatten()
        .find(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("mtime_index_v2__")
        })
        .expect("mtime index exists");
    let document: serde_json::Value =
        serde_json::from_slice(&std::fs::read(index.path()).expect("mtime index is readable"))
            .expect("mtime index is JSON");
    let key = document[relative_path]["key"]
        .as_str()
        .unwrap_or_else(|| panic!("mtime index omitted {relative_path}: {document}"));
    directory.join(format!("{key}.json"))
}

fn relations_entry(fixture: &Fixture) -> std::path::PathBuf {
    std::fs::read_dir(fixture.root.join(".tracer-cache/file"))
        .expect("file cache exists")
        .flatten()
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .starts_with("relations_edges_v1__")
        })
        .expect("relations index exists")
}

fn relations_symbols_entry(fixture: &Fixture) -> std::path::PathBuf {
    std::fs::read_dir(fixture.root.join(".tracer-cache/file"))
        .expect("file cache exists")
        .flatten()
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .starts_with("relations_symbols_v1__")
        })
        .expect("relations symbols entry exists")
}

#[test]
fn edge_only_commands_do_not_read_symbols_and_callers_rebuilds_them() {
    let f = standard_repo();
    f.trace(&["cache", "build"]).ok();
    let before = f.trace(&["callers", "helper", "--json"]);
    before.ok();
    let before_rows = before.view()["results"].clone();
    let symbols = relations_symbols_entry(&f);
    std::fs::remove_file(&symbols).expect("remove symbols entry");

    f.trace(&["read", "src/util.py", "--json"]).ok();
    f.trace(&["info", "src/util.py", "--json"]).ok();
    f.trace(&["usages", "--path", "src", "--json"]).ok();
    assert!(
        !symbols.exists(),
        "an edge-only command loaded or rewrote the symbols entry"
    );

    let callers = f.trace(&["callers", "helper", "--json"]);
    callers.ok();
    assert_eq!(
        callers.view()["results"], before_rows,
        "callers rebuilt a different answer after the symbols entry was deleted"
    );
    let rebuilt = std::fs::read(&symbols).expect("callers recreated the symbols entry");
    f.trace(&["callers", "helper", "--json"]).ok();
    assert_eq!(
        std::fs::read(&symbols).expect("symbols entry remains readable"),
        rebuilt,
        "a second callers query rebuilt symbols again"
    );
}

#[test]
fn a_symbols_table_mismatch_rebuilds_the_current_symbol_rows() {
    let f = Fixture::new();
    f.write("target.py", "def selected():\n    return 1\n");
    f.write(
        "caller.py",
        "from target import selected\n\ndef use():\n    return selected()\n",
    );
    f.commit("symbol table fixture");
    f.trace(&["cache", "build", "."]).ok();
    let symbols = relations_symbols_entry(&f);
    let mut poisoned: serde_json::Value = serde_json::from_slice(
        &std::fs::read(&symbols).expect("symbols entry is readable"),
    )
    .expect("symbols entry is JSON");
    poisoned["table"] = serde_json::Value::String("not-the-file-table".to_string());
    poisoned["symbols"] = serde_json::json!({});
    std::fs::write(
        &symbols,
        serde_json::to_vec(&poisoned).expect("poisoned symbols serialize"),
    )
    .expect("poisoned symbols are written");

    let callers = f.trace(&["callers", "selected", "--json"]);
    callers.ok();
    let defines = f.trace(&["defines", "selected", "--json"]);
    defines.ok();
    f.trace(&["cache", "clear", "--all"]).ok();
    let fresh_callers = f.trace(&["callers", "selected", "--json"]);
    fresh_callers.ok();
    let fresh_defines = f.trace(&["defines", "selected", "--json"]);
    fresh_defines.ok();

    assert_eq!(callers.view()["results"], fresh_callers.view()["results"]);
    assert_eq!(defines.view()["results"], fresh_defines.view()["results"]);
}

#[test]
fn rebuilding_symbols_keeps_each_call_site_once() {
    let f = Fixture::new();
    f.write("target.py", "def selected():\n    return 1\n");
    f.write(
        "caller.py",
        "from target import selected\n\ndef use():\n    return selected() + selected() + selected() + selected() + selected()\n",
    );
    f.commit("five call sites");
    f.trace(&["cache", "build", "."]).ok();
    let before = f.trace(&["callers", "selected", "--json"]);
    before.ok();
    let before_rows = before.view()["results"].clone();
    let rows = before_rows.as_array().expect("callers results are rows");
    let calls: Vec<&serde_json::Value> = rows
        .iter()
        .flat_map(|row| row["callers"].as_array().into_iter().flatten())
        .collect();
    assert_eq!(calls.len(), 5, "five call sites must yield five rows: {before_rows}");

    std::fs::remove_file(relations_symbols_entry(&f)).expect("remove symbols entry");
    let rebuilt = f.trace(&["callers", "selected", "--json"]);
    rebuilt.ok();
    assert_eq!(rebuilt.view()["results"], before_rows);
}

#[test]
fn a_relations_update_rewrites_both_entries() {
    let f = standard_repo();
    f.trace(&["cache", "build"]).ok();
    let edges = relations_entry(&f);
    let symbols = relations_symbols_entry(&f);
    let before_edges = std::fs::read(&edges).expect("edges entry readable");
    let before_symbols = std::fs::read(&symbols).expect("symbols entry readable");

    f.write("src/util.py", "def renamed(v):\n    return v + 1\n");
    f.trace(&["defines", "renamed", "--json"]).ok();

    assert_ne!(before_edges, std::fs::read(&edges).expect("edges rewritten"));
    assert_ne!(before_symbols, std::fs::read(&symbols).expect("symbols rewritten"));
    f.trace(&["defines", "helper", "--json"]).code_is(2);
}

#[test]
fn a_legacy_relations_entry_is_swept_without_being_read() {
    let f = standard_repo();
    f.trace(&["cache", "build"]).ok();
    let legacy = f
        .root
        .join(".tracer-cache/file/relations_v3__schema18.json");
    std::fs::write(&legacy, b"not-json").expect("plant legacy entry");

    f.write("src/util.py", "def legacy_sweep(v):\n    return v + 1\n");
    f.trace(&["defines", "legacy_sweep", "--json"]).ok();
    assert!(!legacy.exists(), "legacy relations entry was not swept");
}

/// Break 2: a file's caller count depends on files that did not change. The
/// per-file line must be exactly as fresh as the graph, so adding a call in
/// another file moves it with no touch to the file being read.
#[test]
fn adding_a_call_in_another_file_moves_the_read_files_caller_count() {
    let f = standard_repo();
    f.trace(&["cache", "build"]).ok();
    let before = f.trace(&["context", "src/util.py"]).ok().stdout.clone();
    let callers_before = caller_count(&before);

    f.write(
        "src/second.py",
        "from src.util import helper\n\n\ndef other():\n    return helper(2)\n",
    );
    f.commit("add a second caller");
    f.trace(&["cache", "build"]).ok();

    let after = f.trace(&["context", "src/util.py"]).ok().stdout.clone();
    let callers_after = caller_count(&after);
    assert_eq!(
        callers_after,
        callers_before + 1,
        "caller count did not move when another file gained a call\nbefore: {before}\nafter: {after}"
    );
}

/// The relations index is a cache, never a source of truth: the per-file
/// entries are. Delete the index and the same answer comes back, rebuilt
/// from those entries, and the index is written again.
#[test]
fn deleting_the_relations_index_alone_changes_no_answer() {
    let f = standard_repo();
    f.trace(&["cache", "build"]).ok();
    let before = f.trace(&["context", "src/util.py"]).ok().stdout.clone();

    remove_matching(&f.root.join(".tracer-cache/file"), "relations_", "json");

    let after = f.trace(&["context", "src/util.py"]).ok().stdout.clone();
    assert_eq!(
        caller_count(&after),
        caller_count(&before),
        "the rebuilt index answered differently from the one that was deleted"
    );
    // Rebuilt and persisted, so the next call pays nothing.
    remove_matching(&f.root.join(".tracer-cache/file"), "relations_", "json");
}

fn remove_matching(dir: &std::path::Path, prefix: &str, extension: &str) {
    let mut removed = 0;
    for entry in std::fs::read_dir(dir)
        .expect("cache namespace exists")
        .flatten()
    {
        let path = entry.path();
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        if name.starts_with(prefix) && name.ends_with(extension) {
            std::fs::remove_file(&path).unwrap();
            removed += 1;
        }
    }
    assert!(
        removed > 0,
        "nothing matched {prefix}*.{extension} in {dir:?}"
    );
}

fn caller_count(shoulder: &str) -> i64 {
    let at = shoulder
        .find("incoming: ")
        .unwrap_or_else(|| panic!("shoulder carries no incoming count:\n{shoulder}"));
    shoulder[at + "incoming: ".len()..]
        .split(|c: char| !c.is_ascii_digit())
        .next()
        .and_then(|d| d.parse().ok())
        .unwrap_or_else(|| panic!("incoming count is not a number:\n{shoulder}"))
}

/// A large file is indexed like any other: no size threshold decides whether
/// an agent gets symbols.
///
/// A tree-sitter tree costs a multiple of its source, but the multiple tracks
/// node density, not bytes: real 2.3 MB files measured 76 MB (a minified
/// bundle) and 145 MB (generated C), while a synthetic 11 MB file of nothing
/// but tiny function declarations measured 1.24 GB. A byte threshold set from
/// the synthetic case refused `php-src/Zend/zend_vm_execute.h`, which is 2.3
/// MB of committed C carrying 322 declarations. Memory is bounded where the
/// files are held, not by refusing to read them.
#[test]
fn a_large_file_is_indexed_like_any_other() {
    let f = Fixture::new();
    let line = "export function f_LINE() { return 1; }\n";
    let mut big = String::with_capacity(3_000_000);
    let mut n = 0;
    while big.len() < 2_500_000 {
        big.push_str(&line.replace("LINE", &n.to_string()));
        n += 1;
    }
    f.write("src/huge.js", &big);
    f.commit("one file well past any plausible threshold");

    f.trace(&["cache", "build", "."]).ok();
    let v = f.trace(&["info", "src/huge.js", "--json"]).view();
    let key = v["file"].as_str().expect("info echoes the file it read");
    let shoulder = v["files"][key]["shoulder"].as_str().unwrap();
    assert!(
        !shoulder.contains("unparsed"),
        "a large file must still be parsed: {shoulder}"
    );
    assert!(
        v["functions"].as_i64().unwrap() > 1000,
        "a large file's functions must be counted: {}",
        v["functions"]
    );
    // Its declarations reach the index, so an agent can find them.
    f.trace(&["defines", "f_7"]).ok();
}

/// Break 3: the git facts are keyed by HEAD, which an uncommitted edit does
/// not move. The working-tree state must still flip to `modified`.
#[test]
fn an_uncommitted_edit_reports_modified_without_moving_head() {
    let f = standard_repo();
    let clean = f.trace(&["context", "src/util.py"]).ok().stdout.clone();
    assert!(
        !clean.contains("modified"),
        "committed file reported dirty before any edit:\n{clean}"
    );

    f.write(
        "src/util.py",
        "def helper(v):\n    if v > 0:\n        return v + 2\n    return 0\n",
    );

    let dirty = f.trace(&["context", "src/util.py"]).ok().stdout.clone();
    assert!(
        dirty.contains("modified"),
        "uncommitted edit did not surface as modified:\n{dirty}"
    );
}

/// Break 4: two processes writing the index race on read-clone-insert-store,
/// and the loser's entry is dropped. That is safe — a lost entry costs a
/// rehash, never a wrong answer — so both files must resolve correctly after
/// the race.
#[test]
fn concurrent_calls_on_disjoint_files_both_resolve() {
    let f = standard_repo();
    f.write("src/one.py", "def one():\n    return 1\n");
    f.write("src/two.py", "def two():\n    return 2\n");
    f.commit("two more files");

    let root_a = f.root.clone();
    let root_b = f.root.clone();
    let a = thread::spawn(move || trace(&root_a, ["structure", "src/one.py", "--json"]));
    let b = thread::spawn(move || trace(&root_b, ["structure", "src/two.py", "--json"]));
    let one_race = a.join().unwrap();
    let two_race = b.join().unwrap();
    assert_structure_symbol(&one_race, "one");
    assert_structure_symbol(&two_race, "two");

    let one = f.trace(&["structure", "src/one.py", "--json"]);
    let two = f.trace(&["structure", "src/two.py", "--json"]);
    assert_structure_symbol(&one, "one");
    assert_structure_symbol(&two, "two");
}

/// The concurrent assertion must reject one failed child even when the other
/// child returns the expected declaration. This is an actual pair of CLI
/// subprocesses; catching the assertion lets the test pin that negative case.
#[test]
fn concurrent_result_assertion_rejects_one_failed_process() {
    let f = Fixture::new();
    f.write("src/one.py", "def one():\n    return 1\n");
    f.commit("one file");

    let root_a = f.root.clone();
    let root_b = f.root.clone();
    let one = thread::spawn(move || trace(&root_a, ["structure", "src/one.py", "--json"]));
    let failed = thread::spawn(move || trace(&root_b, ["structure", "src/missing.py", "--json"]));
    let one = one.join().unwrap();
    let failed = failed.join().unwrap();

    assert_structure_symbol(&one, "one");
    assert_ne!(
        failed.code,
        0,
        "the deliberately failing subprocess unexpectedly succeeded:\n{}",
        failed.combined()
    );
    let rejected = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        assert_structure_symbol(&failed, "two");
    }));
    assert!(
        rejected.is_err(),
        "a failed concurrent subprocess passed the result assertion"
    );
}

/// Break 5: a file rewritten immediately after a warming call. The index holds
/// the pre-write stamp against the pre-write hash, so the next call must miss
/// and re-extract rather than serve what it recorded.
#[test]
fn a_rewrite_right_after_a_warming_call_is_seen_by_the_next_one() {
    let f = Fixture::new();
    f.write("src/a.py", "def alpha(v):\n    return v\n");
    f.commit("init");
    f.trace(&["structure", "src/a.py"]).ok();

    f.write(
        "src/a.py",
        "def alpha(v):\n    if v:\n        return v\n    return 0\n\n\ndef beta():\n    return 1\n",
    );

    let out = f.trace(&["structure", "src/a.py"]).ok().stdout.clone();
    assert!(out.contains("beta"), "the rewrite was not seen:\n{out}");
}
