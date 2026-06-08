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

/// The Spine ranks by transitive dependent count, not raw direct in-edges.
///
/// Fixture is a dependency chain: a,b,c,d each import `mid`; `mid` imports
/// `leaf`. Direct import in-edges: `mid` = 4 (a,b,c,d), `leaf` = 1 (mid).
/// Transitive dependents: `mid` = {a,b,c,d} = 4; `leaf` = {mid,a,b,c,d} = 5.
///
/// Under raw-direct ranking `mid` (4) would outrank `leaf` (1). Under
/// transitive ranking `leaf` (5) outranks `mid` (4) — `leaf` is the true
/// load-bearing node because everything ultimately rests on it. This asserts
/// the flip: `leaf`'s row appears before `mid`'s in the Spine, and the
/// transitive column carries the higher count for `leaf`.
#[test]
fn primer_spine_ranks_by_transitive_dependents() {
    let f = Fixture::new();
    f.write("src/leaf.ts", "export const leaf = 1;\n");
    f.write(
        "src/mid.ts",
        "import { leaf } from './leaf';\nexport const mid = leaf + 1;\n",
    );
    for name in ["a", "b", "c", "d"] {
        f.write(
            &format!("src/{name}.ts"),
            &format!("import {{ mid }} from './mid';\nexport const {name} = mid;\n"),
        );
    }
    f.commit("dependency chain");
    f.trace(&["cache", "build", "."]).ok();

    let r = f.trace(&["context"]);
    r.ok();
    let spine: Vec<&str> = r
        .stdout
        .lines()
        .skip_while(|l| !l.contains("## Spine"))
        .take_while(|l| !l.starts_with("repo_context:"))
        .collect();
    let block = spine.join("\n");

    // The header now carries a `transitive` column alongside `direct`.
    assert!(
        block.contains("transitive"),
        "Spine table must carry a transitive-dependents column:\n{block}"
    );

    let leaf_pos = block
        .find("leaf @")
        .unwrap_or_else(|| panic!("leaf row missing from Spine:\n{block}"));
    let mid_pos = block
        .find("mid @")
        .unwrap_or_else(|| panic!("mid row missing from Spine:\n{block}"));
    assert!(
        leaf_pos < mid_pos,
        "leaf (transitive 5) must rank above mid (transitive 4) under \
         transitive-dependent ranking; raw-direct ranking would invert this:\n{block}"
    );

    // The leaf row shows direct=1, transitive=5; the mid row direct=4,
    // transitive=4. Pin the leaf row's two counts exactly.
    let leaf_line = spine
        .iter()
        .find(|l| l.contains("leaf @"))
        .expect("leaf line");
    let nums: Vec<&str> = leaf_line.split_whitespace().collect();
    // Row shape: "<rank> <direct> <transitive> <kind> <label> @ <source>".
    assert_eq!(nums[1], "1", "leaf direct in-edges = 1 (only mid): {leaf_line}");
    assert_eq!(
        nums[2], "5",
        "leaf transitive dependents = 5 (mid,a,b,c,d): {leaf_line}"
    );
}

/// A conditional rule whose `paths:` glob matches a working-tree-dirty file,
/// and that is not already in the session's context, is surfaced in the Rules
/// section as "Applies to your changes, not loaded".
///
/// No session id is set, so the session log no-ops and nothing is "loaded" —
/// every applicable rule is therefore unloaded and must surface.
#[test]
fn primer_surfaces_applicable_unloaded_conditional_rule() {
    let f = Fixture::new();
    f.write("src/app.py", "def main():\n    return 1\n");
    f.write(
        ".claude/rules/python.md",
        "---\npaths: \"**/*.py\"\n---\nPython style rules.\n",
    );
    f.commit("seed with a conditional python rule");
    f.trace(&["cache", "build", "."]).ok();

    // Dirty a file the rule's glob matches.
    f.write("src/app.py", "def main():\n    if True:\n        return 1\n    return 0\n");

    let r = f.trace(&["context"]);
    r.ok();
    assert!(
        r.stdout.contains("Applies to your changes, not loaded"),
        "primer must surface an applicable-but-unloaded conditional rule:\n{}",
        r.stdout
    );
    assert!(
        r.stdout
            .contains(".claude/rules/python.md (matches **/*.py)"),
        "the surfaced line must name the rule and the glob that matched:\n{}",
        r.stdout
    );
}

/// The applicable-rules warning is silent when nothing applies: a conditional
/// rule whose glob does not match any dirty file is not surfaced.
#[test]
fn primer_applicable_rules_silent_when_no_dirty_match() {
    let f = Fixture::new();
    f.write("src/app.py", "def main():\n    return 1\n");
    f.write(
        ".claude/rules/typescript.md",
        "---\npaths: \"**/*.ts\"\n---\nTypeScript rules.\n",
    );
    f.commit("seed with a ts-only conditional rule");
    f.trace(&["cache", "build", "."]).ok();

    // Dirty a python file — the .ts-scoped rule must not surface.
    f.write("src/app.py", "def main():\n    if True:\n        return 1\n    return 0\n");

    let r = f.trace(&["context"]);
    r.ok();
    assert!(
        !r.stdout.contains("Applies to your changes, not loaded"),
        "a conditional rule whose glob matches no dirty file must not surface:\n{}",
        r.stdout
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
