//! Many agents call `trace` against one repository at the same time. Every
//! call must answer from the bytes on disk, and the calls must not each pay
//! for the same index update: one process updates, the rest read its result.

use std::fs;
use std::thread;
use std::time::Duration;
use tracer_cli_tests::{standard_repo, trace, Fixture};

fn mtime_index(fixture: &Fixture) -> String {
    let entries = fs::read_dir(fixture.root.join(".tracer-cache/file")).unwrap();
    let path = entries
        .flatten()
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("mtime_index_"))
        })
        .expect("the mtime index exists after a call");
    fs::read_to_string(path).unwrap()
}

/// Two processes that each learn a file at the same moment both land in the
/// mtime index: the update merges onto what is on disk under the maintenance
/// lock, so neither write erases the other's.
#[test]
fn two_concurrent_calls_both_land_in_the_mtime_index() {
    let f = standard_repo();
    f.trace(&["context"]).ok();
    f.write("src/one.py", "def one():\n    return 1\n");
    f.write("src/two.py", "def two():\n    return 2\n");

    let root_a = f.root.clone();
    let root_b = f.root.clone();
    let a = thread::spawn(move || trace(&root_a, ["structure", "src/one.py", "--json"]));
    let b = thread::spawn(move || trace(&root_b, ["structure", "src/two.py", "--json"]));
    a.join().unwrap().ok();
    b.join().unwrap().ok();

    let index = mtime_index(&f);
    assert!(
        index.contains("src/one.py") && index.contains("src/two.py"),
        "one concurrent write erased the other's entry:\n{index}"
    );
}

/// Eight calls arriving right after a file appears cost about one update,
/// not eight: the first holder updates the indexes and the others wait for
/// it and read the result. Every call answers cleanly, with nothing on stderr.
#[test]
fn eight_concurrent_calls_after_an_added_file_cost_about_one_call() {
    let f = standard_repo();
    f.trace(&["context"]).ok();
    f.write("src/first.py", "def first():\n    return 1\n");
    let one = f.trace(&["grep", "zzqq_nomatch", "--path", "src"]);
    one.ok();

    f.write("src/second.py", "def second():\n    return 2\n");
    let started = std::time::Instant::now();
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let root = f.root.clone();
            thread::spawn(move || trace(&root, ["grep", "zzqq_nomatch", "--path", "src"]))
        })
        .collect();
    let runs: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    let eight = started.elapsed();

    for run in &runs {
        run.ok();
        assert!(run.stderr.is_empty(), "stderr must stay empty: {}", run.stderr);
    }
    let budget = one.elapsed * 3 + Duration::from_millis(500);
    assert!(
        eight <= budget,
        "eight concurrent calls took {eight:?}; one call after an added file took {:?}",
        one.elapsed
    );
}
