//! Speed assertions. Every command gets a wall-clock budget; cache-backed
//! commands additionally get a cold-vs-warm pair so a lost cache is caught.
//!
//! Thresholds are intentionally loose — they are regression tripwires, not
//! micro-benchmarks. On this machine the
//! observed times are ~0.1–0.3s; the budgets below leave generous headroom
//! for a loaded CI box while still failing on an order-of-magnitude
//! regression or a cache that silently stopped working. Tune `SLOW`/`FAST`
//! up if a constrained runner flakes — the cold-vs-warm *ratio* check
//! survives threshold tuning.

use std::time::Duration;
use tracer_cli_tests::{standard_repo, Fixture};

/// A repo big enough that re-extracting every file is unmistakably slower
/// than serving a warm cache. The dead-cache signal needs this: on
/// `standard_repo()` (five trivial files) a no-op cache is invisible
/// because cold and warm are both ~instant. Hundreds of branchy files make
/// "cache silently stopped working" measurable as warm ≫ should-be.
fn large_repo() -> Fixture {
    let f = Fixture::new();
    for d in 0..12 {
        for i in 0..30 {
            f.write(
                &format!("pkg{d:02}/mod{i:02}.py"),
                "def f(a, b, c):\n\
                 \x20   if a and b:\n\
                 \x20       return 1\n\
                 \x20   for i in range(c):\n\
                 \x20       if i % 2 == 0 or i == c:\n\
                 \x20           return i\n\
                 \x20   return 0\n",
            );
        }
    }
    f.commit("large multi-package repo (360 branchy files)");
    f
}

const SLOW: Duration = Duration::from_secs(10); // cold / heavy commands
const FAST: Duration = Duration::from_secs(5); // warm / light commands

#[test]
fn doctor_is_fast() {
    let f = standard_repo();
    f.trace(&["doctor"]).ok().within(FAST);
}

#[test]
fn read_warm_is_fast() {
    let f = standard_repo();
    f.trace(&["cache", "build", "."]).ok();
    f.trace(&["read", "src/app.py"]).ok().within(FAST);
}

#[test]
fn info_cold_then_warm() {
    let f = standard_repo();
    f.trace(&["cache", "clear", "--all"]).ok();
    let cold = f.trace(&["info", "src/app.py"]);
    cold.ok().within(SLOW);
    let warm = f.trace(&["info", "src/app.py"]);
    warm.ok().within(FAST);
}

#[test]
fn cache_build_cold_within_budget() {
    let f = standard_repo();
    f.trace(&["cache", "clear", "--all"]).ok();
    f.trace(&["cache", "build", "."]).ok().within(SLOW);
}

#[test]
fn cache_build_warm_is_fast() {
    let f = standard_repo();
    f.trace(&["cache", "build", "."]).ok();
    f.trace(&["cache", "build", "."]).ok().within(FAST);
}

#[test]
fn architecture_query_warm_is_fast() {
    let f = standard_repo();
    f.trace(&["cache", "build", "."]).ok();
    f.trace(&["callers", "helper"]).ok().within(FAST);
    f.trace(&["downstream", "--path", ".", "--json"])
        .ok()
        .within(FAST);
}

#[test]
fn search_commands_within_budget() {
    let f = standard_repo();
    f.trace(&["grep", "helper", "--path", "."]).ok().within(SLOW);
    f.trace(&["glob", "**/*.py", "."]).ok().within(SLOW);
    f.trace(&["find", "*.py", "."]).ok().within(SLOW);
}

#[test]
fn git_commands_within_budget() {
    let f = standard_repo();
    f.trace(&["history", "src/app.py"]).ok().within(SLOW);
    f.trace(&["blame", "src/app.py"]).ok().within(SLOW);
    f.trace(&["status"]).ok().within(SLOW);
}

#[test]
fn primer_within_budget() {
    let f = standard_repo();
    f.trace(&["cache", "clear", "--all"]).ok();
    f.trace(&["context"]).ok().within(SLOW);
}

/// Dead-cache detector. Sized so a non-functioning cache is *measurable*:
/// a whole-repo `info` over 360 branchy files re-extracts every one of
/// them when cold. With a working cache the warm pass reuses all 360
/// entries and is dramatically faster; a cache that silently stopped
/// working re-extracts on every run, so warm stays as slow as cold and
/// this fails. The ratio is the signal that survives absolute-threshold
/// tuning — not a micro-benchmark.
#[test]
fn warm_whole_repo_is_far_faster_than_cold_on_a_large_repo() {
    let f = large_repo();
    f.trace(&["cache", "clear", "--all"]).ok();

    let cold = f.trace(&["info", ".", "--json"]);
    cold.ok();
    // Sanity: the fixture really is large (a dead cache on five files would
    // not move the needle, so the fixture size is itself the test).
    // large_repo() writes exactly 12 packages × 30 modules = 360 .py
    // files; `info .` analyzes every one. Pinning the exact count both
    // documents the fixture size and fails loudly if the generator or the
    // directory walk ever drops files.
    assert_eq!(
        cold.json()["file_count"].as_i64().unwrap(),
        360,
        "large_repo() must yield exactly 360 analyzed files: {} files",
        cold.json()["file_count"]
    );

    let warm = f.trace(&["info", ".", "--json"]);
    warm.ok();

    // Loose tripwire: a working warm pass is far under cold. If the cache
    // is dead, warm ≈ cold (full re-extraction) and this fails. The 60%
    // ceiling is generous headroom — a healthy warm pass on this fixture
    // is a small fraction of cold, not 60% of it.
    let ceiling = cold.elapsed.mul_f64(0.60).max(Duration::from_millis(400));
    assert!(
        warm.elapsed <= ceiling,
        "warm whole-repo info ({:?}) not far below cold ({:?}) on a \
         360-file repo — the cache is not serving warm reads",
        warm.elapsed,
        cold.elapsed
    );
}
