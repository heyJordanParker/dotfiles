//! Many agents call `trace` against one repository at the same time. Every
//! call must answer from the bytes on disk, and the calls must not each pay
//! for the same index update: one process updates, the rest read its result.

use std::fs;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tracer_cli_tests::{
    schema_directory, standard_repo, trace, trace_env, Fixture, PUBLISHED_SCHEMA_VERSION,
};

fn mtime_index(fixture: &Fixture) -> String {
    fs::read_to_string(
        schema_directory(&fixture.root, PUBLISHED_SCHEMA_VERSION).join("mtime_index_v2__ast.json"),
    )
    .expect("the mtime index exists after a call")
}

/// An agent runs several `trace` calls side by side, each with its own docs
/// hook, so one session's doc deliveries take turns: no two calls send the
/// same lines of a long doc.
#[test]
fn concurrent_doc_deliveries_never_send_the_same_lines_twice() {
    let fixture = Fixture::new();
    let rules: String = (1..=600)
        .map(|n| format!("- Rule {n}: one line of a doc too long for one message.\n"))
        .collect();
    fixture.write("Claude.md", &format!("# Rules\n{rules}"));
    fixture.write("x.py", "x = 1\n");
    fixture.commit("long doc");
    let session = format!(
        "concurrent-docs-{}",
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
    );

    let mut sent: Vec<usize> = Vec::new();
    for _ in 0..4 {
        let calls: Vec<_> = (0..6)
            .map(|_| {
                let root = fixture.root.clone();
                let session = session.clone();
                thread::spawn(move || {
                    trace_env(
                        &root,
                        ["docs", "x.py", "--budget", "2000"],
                        &[("CLAUDE_CODE_SESSION_ID", session.as_str())],
                    )
                })
            })
            .collect();
        for call in calls {
            let run = call.join().unwrap();
            run.ok();
            sent.extend(
                run.stdout
                    .lines()
                    .filter_map(|line| line.strip_prefix("- Rule "))
                    .filter_map(|rest| rest.split(':').next()?.parse::<usize>().ok()),
            );
        }
    }
    let mut unique = sent.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(sent.len(), unique.len(), "a rule was sent to two calls");
    assert_eq!(unique, (1..=unique.len()).collect::<Vec<_>>(), "the doc arrives in order, with no gap");
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

/// Two installed builds at different schemas update their own indexes side by
/// side: an update holds only the lock in its own schema's directory, so a
/// build of a neighbor schema, or one from before schema directories at any
/// schema, holding its lock mid-update never stalls it.
#[test]
fn an_index_update_never_waits_on_another_schemas_lock() {
    let f = standard_repo();
    f.trace(&["cache", "build", "."]).ok();
    let file_namespace = f.root.join(".tracer-cache/file");
    let held: Vec<fs::File> = [
        file_namespace.join(".maintain.lock"),
        file_namespace.join(format!(".maintain__schema{PUBLISHED_SCHEMA_VERSION}.lock")),
        schema_directory(&f.root, PUBLISHED_SCHEMA_VERSION - 1).join(".maintain.lock"),
        schema_directory(&f.root, PUBLISHED_SCHEMA_VERSION + 1).join(".maintain.lock"),
    ]
    .iter()
    .map(|path| {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let lock = fs::File::options()
            .create(true)
            .truncate(false)
            .write(true)
            .open(path)
            .unwrap();
        lock.lock().unwrap();
        lock
    })
    .collect();
    f.write("src/added.py", "def added():\n    return 1\n");

    let mut child = Command::new(tracer_cli_tests::trace_bin())
        .args(["grep", "added", "src"])
        .current_dir(&f.root)
        .env("HOME", &f.root)
        .env("TRACE_TIMING", "1")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let started = Instant::now();
    while child.try_wait().unwrap().is_none() {
        if started.elapsed() >= Duration::from_secs(10) {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("the index update waited 10 seconds on another schema's lock");
        }
        thread::sleep(Duration::from_millis(10));
    }
    let run = child.wait_with_output().unwrap();
    drop(held);
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(run.status.success(), "the call failed: {stderr}");
    assert!(
        stderr.contains("timing lock maintain "),
        "the call took no index lock, so it proved nothing about waiting:\n{stderr}"
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

/// Eight `history --contains` calls on a repository nobody indexed walk its
/// commits once: the first holder walks them, and the rest read its index.
#[test]
fn eight_concurrent_history_searches_walk_the_commits_once() {
    let f = standard_repo();
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let root = f.root.clone();
            thread::spawn(move || {
                trace_env(&root, ["history", "--contains", "def", "--json"], &[("TRACE_TIMING", "1")])
            })
        })
        .collect();
    let runs: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    let mut walks = 0;
    for run in &runs {
        run.ok();
        walks += run.stderr.matches("timing git log --no-walk=unsorted ").count();
    }
    assert_eq!(walks, 1, "the commits were walked {walks} times");
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
    let key = format!("{}/src/", f.root.canonicalize().unwrap().display());
    assert!(directories.get(&key).is_some(), "{directories}");
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

/// The microseconds one `TRACE_TIMING` phase took across a call's stderr.
fn phase_micros(stderr: &str, phase: &str) -> u128 {
    let prefix = format!("timing {phase} ");
    stderr
        .lines()
        .filter_map(|line| line.strip_prefix(&prefix)?.parse::<u128>().ok())
        .sum()
}

/// Claude Code sends one Agent's `Read`s in parallel, and each runs a windowed
/// `context` on its own file. Each call resolves its `calls:` block before it
/// takes the Agent's lock, so a call waits on the others only for their
/// compare, print, and save, never for their resolution. Measured on this
/// fixture: holding the lock across the resolution made five of six calls
/// wait 40% to 67% of their render; resolving first, the longest wait was
/// 0.3%, so the 10% bound sits well clear of both.
#[test]
fn one_agents_parallel_windowed_calls_never_wait_on_each_others_calls() {
    let f = Fixture::new();
    let methods: Vec<String> = (0..16).map(|m| format!("record{m}")).collect();
    let declared: String = methods
        .iter()
        .map(|method| format!("    public function {method}(int $cents): int\n    {{\n        return $cents;\n    }}\n"))
        .collect();
    f.write("ledger.php", &format!("<?php\nclass Ledger {{\n{declared}}}\n"));
    let called: String = methods
        .iter()
        .map(|method| format!("        $this->ledger->{method}($cents);\n"))
        .collect();
    for n in 0..6 {
        f.write(
            &format!("checkout{n}.php"),
            &format!("<?php\nclass Checkout{n} {{\n    public function __construct(private Ledger $ledger) {{}}\n    public function pay(int $cents): void\n    {{\n{called}    }}\n}}\n"),
        );
    }
    for n in 0..100 {
        let called: String = methods
            .iter()
            .map(|method| format!("        $ledger->{method}(-$cents - {n});\n"))
            .collect();
        f.write(
            &format!("refund{n}.php"),
            &format!("<?php\nclass Refund{n} {{\n    public function back(Ledger $ledger, int $cents): void\n    {{\n{called}    }}\n}}\n"),
        );
    }
    f.commit("many callers");
    f.trace(&["cache", "build", "."]).ok();
    let session_id = format!(
        "parallel-windows-{}",
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()
    );

    let handles: Vec<_> = (0..6)
        .map(|n| {
            let root = f.root.clone();
            let session_id = session_id.clone();
            let file = format!("checkout{n}.php");
            thread::spawn(move || {
                trace_env(
                    &root,
                    ["context", file.as_str(), "--offset", "6", "--limit", "16", "--no-record"],
                    &[("AGENT_SESSION_ID", session_id.as_str()), ("TRACE_TIMING", "1")],
                )
            })
        })
        .collect();
    let runs: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    for run in &runs {
        run.ok();
        assert!(run.stdout.contains("Ledger::record15"), "the window resolves its calls:\n{}", run.stdout);
        assert!(run.stderr.contains("timing lock session shown "), "each new file takes the lock:\n{}", run.stderr);
    }
    let waits: Vec<(u128, u128)> = runs
        .iter()
        .map(|run| (phase_micros(&run.stderr, "lock session shown"), phase_micros(&run.stderr, "render")))
        .collect();
    assert!(
        waits.iter().all(|(lock, render)| lock * 10 < *render),
        "a call waited on another's calls: (lock µs, render µs) per call: {waits:?}"
    );
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
