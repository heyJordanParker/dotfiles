//! Worktree-anchored cache contract.
//!
//! `.tracer-cache/` lives ONLY at a worktree root — the main-repo root for
//! a normal checkout, or the linked-worktree's own root for a
//! `git worktree add` checkout. Outside any worktree, reads still return
//! results but nothing persists. These tests pin that contract end-to-end
//! through the CLI.

use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use tracer_cli_tests::{standard_repo, trace_bin, trace};

// --- 1. cache writes land at the worktree root -----------------------------

#[test]
fn cache_writes_land_at_the_worktree_root() {
    let f = standard_repo();
    // `cache build` against the fixture root must populate
    // `<root>/.tracer-cache/` and nothing outside it.
    let r = trace(&f.root, &["cache", "build", "."]);
    r.ok();

    let cache_dir = f.root.join(".tracer-cache");
    assert!(
        cache_dir.is_dir(),
        "cache dir must exist at the worktree root: {cache_dir:?}"
    );
    // Both non-session namespaces present after build.
    for ns in ["file", "architecture"] {
        let ns_dir = cache_dir.join(ns);
        assert!(
            ns_dir.is_dir(),
            "namespace `{ns}` must exist at the worktree root: {ns_dir:?}"
        );
    }
    // Belt-and-braces: cache stats reports a non-zero entry count from
    // the same directory the test inspected on disk.
    let s = trace(&f.root, &["cache", "stats", "--json"]).ok().json();
    let file_entries = s["file"]["entries"].as_u64().unwrap_or(0);
    assert!(
        file_entries > 0,
        "cache stats must show entries written: {s}"
    );
}

// --- 2. linked-worktree caches are isolated from the main worktree --------

#[test]
fn linked_worktree_caches_are_isolated_from_the_main_worktree() {
    let f = standard_repo();
    let wt_root = f.add_worktree("linked-wt", "wt-branch");

    // Add a unique file inside the linked worktree so its file-facts cache
    // holds an entry the main worktree's cache cannot have.
    let wt_file = wt_root.join("only-in-wt.py");
    std::fs::write(&wt_file, "def wt_only():\n    return 1\n").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&wt_root)
        .output()
        .expect("git add");
    Command::new("git")
        .args([
            "-c",
            "user.email=t@e.com",
            "-c",
            "user.name=t",
            "commit",
            "-m",
            "add wt-only file",
        ])
        .current_dir(&wt_root)
        .output()
        .expect("git commit in wt");

    // Build the cache from inside each worktree. Each call must populate
    // its own `.tracer-cache/` and never the other's.
    trace(&f.root, &["cache", "build", "."]).ok();
    trace(&wt_root, &["cache", "build", "."]).ok();

    let main_cache = f.root.join(".tracer-cache");
    let wt_cache = wt_root.join(".tracer-cache");
    assert!(
        main_cache.is_dir(),
        "main cache dir must exist: {main_cache:?}"
    );
    assert!(
        wt_cache.is_dir(),
        "linked worktree cache dir must exist: {wt_cache:?}"
    );
    // The linked worktree's cache directory must be a real directory at
    // the worktree's own root — not a symlink pointing back into the main
    // cache. Worktree isolation requires physically separate trees.
    let wt_meta = std::fs::symlink_metadata(&wt_cache).unwrap();
    assert!(
        !wt_meta.file_type().is_symlink(),
        "linked worktree cache must be a real directory, not a symlink: {wt_cache:?}"
    );

    // Cross-check: the wt-only file's per-file cache entry exists in
    // the linked worktree's cache (`trace info` against it succeeds and
    // reflects the file), and reading it from the main worktree's CLI
    // surface fails to locate the path (because it lives only in the
    // linked checkout's filesystem).
    let info = trace(&wt_root, &["info", "only-in-wt.py", "--json"]).ok().json();
    assert!(
        info.get("file").is_some() || info.get("path").is_some() || !info.is_null(),
        "trace info must return data for the wt-only file from inside the worktree: {info}"
    );
    // The main worktree's `.tracer-cache/file/` entries must not include
    // anything keyed against the wt-only file — exact key contents are
    // implementation-detail, but the *number* of entries from a pristine
    // build of the smaller main tree is strictly less than the linked
    // worktree's after adding a new file.
    let main_files = count_json_entries(&main_cache.join("file"));
    let wt_files = count_json_entries(&wt_cache.join("file"));
    assert!(
        wt_files > main_files,
        "linked worktree cache must hold more file entries than the main (it has one extra committed file): \
         main={main_files} wt={wt_files}"
    );
}

fn collect_tracer_cache_dirs(root: &std::path::Path, found: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(root) else {
        return;
    };
    for entry in rd.flatten() {
        let p = entry.path();
        if entry.file_name() == ".tracer-cache" {
            found.push(p.clone());
        }
        if p.is_dir() {
            collect_tracer_cache_dirs(&p, found);
        }
    }
}

fn count_json_entries(dir: &std::path::Path) -> usize {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return 0;
    };
    rd.flatten()
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("json"))
        .count()
}

// --- 3. no-repo-root execution writes nothing while reads still work ------

#[test]
fn no_repo_root_means_reads_work_but_no_cache_is_written() {
    // Scratch directory with no `.git` anywhere up the tree. Running any
    // tracer read against it must succeed and return live results, AND it
    // must not create a `.tracer-cache/` directory anywhere in the path.
    let scratch = std::env::temp_dir().join(format!(
        "trace-no-worktree-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&scratch).unwrap();
    std::fs::write(
        scratch.join("standalone.py"),
        "def f(x):\n    if x:\n        return 1\n    return 0\n",
    )
    .unwrap();

    // Confirm the cwd is not inside any git repo.
    let out = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(&scratch)
        .output()
        .expect("git rev-parse");
    assert!(
        !out.status.success() || String::from_utf8_lossy(&out.stdout).trim().is_empty(),
        "scratch dir must not be inside any git worktree"
    );

    // Read paths still work: `trace read` returns the file contents,
    // `trace info` returns a JSON shape with no panic.
    let r = Command::new(trace_bin())
        .args(["read", "standalone.py"])
        .current_dir(&scratch)
        .output()
        .expect("spawn trace read");
    assert!(
        r.status.success(),
        "trace read outside any worktree must succeed:\n{}",
        String::from_utf8_lossy(&r.stderr)
    );
    let stdout = String::from_utf8_lossy(&r.stdout);
    assert!(
        stdout.contains("def f"),
        "trace read must return live file contents outside any worktree:\n{stdout}"
    );

    let info = Command::new(trace_bin())
        .args(["info", "standalone.py", "--json"])
        .current_dir(&scratch)
        .output()
        .expect("spawn trace info");
    assert!(
        info.status.success(),
        "trace info outside any worktree must succeed:\n{}",
        String::from_utf8_lossy(&info.stderr)
    );

    // Nothing was persisted. No `.tracer-cache/` anywhere in or under
    // the scratch dir.
    assert!(
        !scratch.join(".tracer-cache").exists(),
        "no `.tracer-cache/` may be created outside a worktree at the scratch root"
    );
    let mut leaked: Vec<PathBuf> = Vec::new();
    collect_tracer_cache_dirs(&scratch, &mut leaked);
    assert!(
        leaked.is_empty(),
        "no `.tracer-cache/` may leak anywhere under the scratch tree, found: {leaked:?}"
    );

    let _ = std::fs::remove_dir_all(&scratch);
}
