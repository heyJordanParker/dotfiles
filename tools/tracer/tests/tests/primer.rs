//! `trace context` with no arguments — the session-start primer. Eight
//! sections; assert each is present and the cache-warming side effect works.

use tracer_cli_tests::{standard_repo, Fixture};

/// The primer's Layout line carries an AST-derived per-directory CCN
/// aggregate (the `file_facts::get_batch` path, no lite-facts). Assert it
/// is the exact sum on a controlled fixture — presence of the line is not
/// enough; a wrong aggregate (lite-facts, stale cache) must fail.
///
/// Fixture: src/ has two files. a.py = base 1 + if(1) = 2; c.py = base 1.
/// The directory aggregate is exactly 3.
#[test]
fn primer_layout_ccn_aggregate_is_exact() {
    let f = Fixture::new();
    f.write("src/a.py", "def a(x):\n    if x:\n        return 1\n    return 0\n");
    f.write("src/c.py", "def c():\n    return 1\n");
    f.commit("two known python files");

    let r = f.trace(&["context"]);
    r.ok();
    let layout_line = r
        .stdout
        .lines()
        .find(|l| l.contains("📁 src/"))
        .unwrap_or_else(|| panic!("primer Layout missing src/:\n{}", r.stdout));
    assert!(
        layout_line.contains("2 files"),
        "Layout file count wrong: {layout_line}"
    );
    assert!(
        layout_line.contains("ccn=3"),
        "Layout CCN aggregate must be exactly 3 (a.py 2 + c.py 1); \
         a lite-facts or stale value would differ: {layout_line}"
    );

    // repo_context footer is computed (scc) and deterministic for this
    // fixture: two branchless-or-single-if python files → p95 0, median 0.
    let footer = r
        .stdout
        .lines()
        .find(|l| l.starts_with("repo_context:"))
        .expect("primer must end with a repo_context footer");
    assert!(
        footer.contains("complexity_p95=0") && footer.contains("median=0"),
        "repo_context footer numbers wrong for the known fixture: {footer}"
    );
}

#[test]
fn primer_emits_all_sections() {
    let f = standard_repo();
    let r = f.trace(&["context"]);
    r.ok();
    for section in [
        "## Environment",
        "## Identity",
        "## Tech Stack",
        "## Layout",
        "## Common Directories",
        "## Git",
        "## Rules",
        "## Spine",
    ] {
        assert!(
            r.stdout.contains(section),
            "primer missing `{section}`:\n{}",
            r.stdout
        );
    }
    assert!(r.stdout.contains("repo_context:"), "{}", r.stdout);
}

#[test]
fn primer_environment_reports_repo_and_git_user() {
    let f = standard_repo();
    let r = f.trace(&["context"]);
    r.ok();
    assert!(r.stdout.contains("repo root:"), "{}", r.stdout);
    assert!(r.stdout.contains("git repository: yes"), "{}", r.stdout);
    // The hermetic fixture commits as "Tracer Test".
    assert!(
        r.stdout.contains("git user: Tracer Test"),
        "primer did not surface fixture git user:\n{}",
        r.stdout
    );
}

#[test]
fn primer_identity_detects_languages() {
    let f = standard_repo();
    let r = f.trace(&["context"]);
    r.ok();
    assert!(r.stdout.contains("Languages:"), "{}", r.stdout);
    assert!(
        r.stdout.contains("Python"),
        "scc should classify the .py files as Python:\n{}",
        r.stdout
    );
}

#[test]
fn primer_tech_stack_detects_pyproject() {
    let f = standard_repo();
    let r = f.trace(&["context"]);
    r.ok();
    assert!(
        r.stdout.contains("pyproject.toml"),
        "tech-stack section missed pyproject.toml:\n{}",
        r.stdout
    );
}

#[test]
fn primer_warms_cache_as_side_effect() {
    let f = standard_repo();
    // Cache starts empty; the primer's first invocation builds it.
    f.trace(&["context"]).ok();
    let v = f.trace(&["cache", "stats", "--json"]).json();
    // The primer warms the file cache over standard_repo()'s fixed tree:
    // exactly 8 file/ entries, 1 architecture/ entry. A primer that
    // stopped warming, or warmed a different file set, fails this.
    assert_eq!(
        v["file"]["entries"].as_i64().unwrap(),
        8,
        "primer must warm exactly 8 file-cache entries: {v}"
    );
    assert_eq!(
        v["architecture"]["entries"].as_i64().unwrap(),
        1,
        "primer must warm exactly 1 architecture-cache entry: {v}"
    );
}

#[test]
fn primer_force_directory_flag_is_quiet() {
    let f = standard_repo();
    let r = f.trace(&["context", "--directory"]);
    r.ok();
    assert!(
        r.stdout.trim().is_empty(),
        "--directory with no path should produce no output:\n{}",
        r.stdout
    );
}
