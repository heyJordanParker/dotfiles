//! Many agents call `trace` against one repository at the same time. Every
//! call must answer from the bytes on disk, and the calls must not each pay
//! for the same index update: one process updates, the rest read its result.

use std::fs;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tracer_cli_tests::{standard_repo, trace, trace_env, Fixture};

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
    let one = f.trace(&["grep", "zzqq_nomatch", "src"]);
    one.ok();

    f.write("src/second.py", "def second():\n    return 2\n");
    let started = std::time::Instant::now();
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let root = f.root.clone();
            thread::spawn(move || trace(&root, ["grep", "zzqq_nomatch", "src"]))
        })
        .collect();
    let runs: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    let eight = started.elapsed();

    for run in &runs {
        run.ok();
        assert!(
            run.stderr.is_empty(),
            "stderr must stay empty: {}",
            run.stderr
        );
    }
    let budget = one.elapsed * 3 + Duration::from_millis(500);
    assert!(
        eight <= budget,
        "eight concurrent calls took {eight:?}; one call after an added file took {:?}",
        one.elapsed
    );
}

#[test]
fn concurrent_agents_share_one_parseable_directory_baseline() {
    let f = standard_repo();
    let session_id = format!(
        "directory-baseline-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let handles: Vec<_> = (0..8)
        .map(|agent| {
            let root = f.root.clone();
            let session_id = session_id.clone();
            thread::spawn(move || {
                let agent_id = format!("agent-{agent}");
                trace_env(
                    &root,
                    ["context", "src/app.py"],
                    &[
                        ("AGENT_SESSION_ID", session_id.as_str()),
                        ("TRACER_AGENT_ID", agent_id.as_str()),
                    ],
                )
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap().ok();
    }

    let path = f
        .root
        .join(".tracer-cache/sessions")
        .join(&session_id)
        .join("directories.json");
    let text = fs::read_to_string(&path).expect("directories.json exists");
    let directories: serde_json::Value =
        serde_json::from_str(&text).expect("directories.json parses");
    assert_eq!(
        directories.as_object().unwrap().len(),
        1,
        "all agents must share the one directory entry: {directories}"
    );
    assert!(directories.get("src/").is_some(), "{directories}");
}

/// Calls surfacing one directory at the same moment show its listing to the
/// Agent exactly once: the record is re-read under the session lock, so no two
/// calls both claim the first showing.
#[test]
fn concurrent_calls_show_a_directory_listing_once() {
    let f = standard_repo();
    f.trace(&["cache", "build", "."]).ok();
    let session_id = format!(
        "listing-once-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let root = f.root.clone();
            let session_id = session_id.clone();
            thread::spawn(move || {
                trace_env(
                    &root,
                    ["context", "src/app.py"],
                    &[("AGENT_SESSION_ID", session_id.as_str())],
                )
            })
        })
        .collect();
    let runs: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    for run in &runs {
        run.ok();
    }
    let listed = runs.iter().filter(|run| run.stdout.contains("  entries: ")).count();
    assert_eq!(listed, 1, "the listing must show exactly once across concurrent calls");
}

/// A primer must finish when its warm cache learns a newly tracked directory.
#[test]
fn warm_context_avoids_deadlock() {
    let f = standard_repo();

    for directory in 0..40 {
        for source in 0..15 {
            let path = format!("layer_{directory}/source_{source}.py");
            let body = if directory == 0 && source == 0 {
                "value = 1\n".to_owned()
            } else {
                let previous_directory = if source == 0 {
                    directory - 1
                } else {
                    directory
                };
                let previous_source = if source == 0 { 14 } else { source - 1 };
                format!(
                    "from layer_{previous_directory}.source_{previous_source} import value\nvalue = value + 1\n"
                )
            };
            f.write(&path, &body);
        }
    }
    f.git(&["add", "."]);
    f.commit("add parallel layout sources");

    for attempt in 1..=20 {
        f.trace(&["cache", "build", "."]).ok();
        let path = format!("added-{attempt}/source.py");
        f.write(&path, "def source():\n    return 1\n");
        f.git(&["add", &path]);

        let mut child = Command::new(tracer_cli_tests::trace_bin())
            .arg("context")
            .current_dir(&f.root)
            .env("HOME", &f.root)
            .env("RAYON_NUM_THREADS", "2")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let started = Instant::now();
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if started.elapsed() >= Duration::from_secs(10) {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("context attempt {attempt} exceeded the 10-second deadline");
            }
            thread::sleep(Duration::from_millis(10));
        };
        assert!(
            status.success(),
            "context attempt {attempt} failed: {status}"
        );
    }
}
