//! Freshness contract: every call reflects the bytes on disk at the moment it
//! runs. The caches exist to make that fast, never to make it approximate, so
//! each test here drives one way the fast path could serve a superseded answer
//! and pins the recompute.

use std::process::Command;
use std::thread;
use tracer_cli_tests::{standard_repo, trace, Fixture};

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
    f.write("src/a.py", "def alpha(v):\n    if v:\n        return 1\n    return 0\n");
    f.commit("init");

    f.trace(&["structure", "src/a.py"]).ok();

    let stamp = f.write("stamp", "");
    copy_mtime(&f.root.join("src/a.py"), &stamp);
    // Same byte length, different declaration.
    f.write("src/a.py", "def gamma(v):\n    if v:\n        return 1\n    return 0\n");
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
    f.write("src/a.py", "def alpha(v):\n    if v:\n        return 1\n    return 0\n");
    f.commit("init");

    f.trace(&["cache", "build", "."]).ok();
    f.trace(&["defines", "alpha"]).ok();

    let stamp = f.write("stamp", "");
    copy_mtime(&f.root.join("src/a.py"), &stamp);
    f.write("src/a.py", "def gamma(v):\n    if v:\n        return 1\n    return 0\n");
    copy_mtime(&stamp, &f.root.join("src/a.py"));

    // `defines` answers from the relations index alone, so that index has to
    // have re-absorbed the file for this to hold.
    f.trace(&["defines", "gamma"]).ok();
    f.trace(&["defines", "alpha"]).code_is(2);
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
    for entry in std::fs::read_dir(dir).expect("cache namespace exists").flatten() {
        let path = entry.path();
        let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        if name.starts_with(prefix) && name.ends_with(extension) {
            std::fs::remove_file(&path).unwrap();
            removed += 1;
        }
    }
    assert!(removed > 0, "nothing matched {prefix}*.{extension} in {dir:?}");
}

fn caller_count(shoulder: &str) -> i64 {
    let at = shoulder
        .find("callers: ")
        .unwrap_or_else(|| panic!("shoulder carries no caller count:\n{shoulder}"));
    shoulder[at + "callers: ".len()..]
        .split(|c: char| !c.is_ascii_digit())
        .next()
        .and_then(|d| d.parse().ok())
        .unwrap_or_else(|| panic!("caller count is not a number:\n{shoulder}"))
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
    let a = thread::spawn(move || trace(&root_a, ["structure", "src/one.py"]).stdout);
    let b = thread::spawn(move || trace(&root_b, ["structure", "src/two.py"]).stdout);
    a.join().unwrap();
    b.join().unwrap();

    let one = f.trace(&["structure", "src/one.py"]).ok().stdout.clone();
    let two = f.trace(&["structure", "src/two.py"]).ok().stdout.clone();
    assert!(one.contains("one"), "src/one.py lost its facts to the race:\n{one}");
    assert!(two.contains("two"), "src/two.py lost its facts to the race:\n{two}");
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
