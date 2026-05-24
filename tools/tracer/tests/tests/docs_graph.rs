//! Docs-graph behavior projected from the unified `architecture/` cache
//! entry: build, cache reuse, invalidation on doc-file or HEAD changes,
//! `@include` edges, conditional `paths:` frontmatter promotion.
//!
//! After the unification, the docs graph no longer owns its own
//! `.tracer-cache/docs/` namespace — its nodes and edges live in the
//! single `architecture/` entry, keyed jointly over per-file content
//! hashes AND git HEAD + doc-file mtime aggregate. These tests therefore
//! probe the public `trace docs --graph` JSON shape (preserved) rather
//! than the on-disk namespace directly.
//!
//! Namespace-level concerns — `cache stats` per-namespace breakdown,
//! `cache clear --namespace`, schema-version invalidation of the on-disk
//! entry — are covered for the unified entry in `cache_and_backend.rs`.

use std::fs;
use std::time::Duration;
use tracer_cli_tests::Fixture;

fn arch_entries(f: &Fixture) -> Vec<std::path::PathBuf> {
    let dir = f.root.join(".tracer-cache/architecture");
    if !dir.is_dir() {
        return vec![];
    }
    let mut out: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("json"))
        .collect();
    out.sort();
    out
}

fn fixture_with_docs() -> Fixture {
    let f = Fixture::new();
    f.write("Claude.md", "# top\n");
    f.write("sub/Claude.md", "# sub\n");
    f.write(".claude/rules/r1.md", "# rule one\n");
    f.write(
        ".claude/rules/r_cond.md",
        "---\npaths:\n  - \"*.py\"\n---\n# conditional\n",
    );
    f.commit("seed docs");
    f
}

#[test]
fn first_call_builds_and_writes_an_entry() {
    let f = fixture_with_docs();
    assert!(
        arch_entries(&f).is_empty(),
        "no architecture entry before build"
    );
    let r = f.trace(&["docs", "--graph", "--json"]);
    r.ok();
    let v = r.json();
    assert_eq!(v["node_count"].as_i64().unwrap(), 4);
    let entries = arch_entries(&f);
    assert_eq!(
        entries.len(),
        1,
        "exactly one architecture entry after first build, got {entries:?}"
    );
}

#[test]
fn second_call_serves_from_cache() {
    let f = fixture_with_docs();
    let first = f.trace(&["docs", "--graph", "--json"]).json();
    let first_built_at = first["graph"]["built_at_ms"].as_u64().unwrap();
    // Sleep just long enough that a rebuild would produce a strictly larger
    // built_at_ms; if the value is identical, the entry was served from cache.
    std::thread::sleep(Duration::from_millis(5));
    let second = f.trace(&["docs", "--graph", "--json"]).json();
    assert_eq!(
        second["graph"]["built_at_ms"].as_u64().unwrap(),
        first_built_at,
        "built_at_ms changed on second call — cache was not hit"
    );
    let entries = arch_entries(&f);
    assert_eq!(entries.len(), 1, "second call wrote a second entry");
}

#[test]
fn touching_a_doc_file_invalidates_the_entry() {
    let f = fixture_with_docs();
    let first = f.trace(&["docs", "--graph", "--json"]).json();
    let first_built_at = first["graph"]["built_at_ms"].as_u64().unwrap();
    let first_aggregate = first["graph"]["mtime_aggregate"].as_str().unwrap().to_string();

    // Sleep past the filesystem mtime resolution to guarantee a tick, then
    // touch Claude.md by rewriting its bytes.
    std::thread::sleep(Duration::from_millis(20));
    f.write("Claude.md", "# top updated\n");

    let second = f.trace(&["docs", "--graph", "--json"]).json();
    assert_ne!(
        second["graph"]["mtime_aggregate"].as_str().unwrap(),
        first_aggregate,
        "mtime aggregate did not change after touching Claude.md"
    );
    assert!(
        second["graph"]["built_at_ms"].as_u64().unwrap() > first_built_at,
        "built_at_ms did not advance after invalidation"
    );
}

#[test]
fn moving_git_head_invalidates_the_entry() {
    let f = fixture_with_docs();
    let first = f.trace(&["docs", "--graph", "--json"]).json();
    let first_head = first["graph"]["head"].as_str().unwrap().to_string();
    let first_built_at = first["graph"]["built_at_ms"].as_u64().unwrap();

    // New commit — same doc files, different HEAD. Sleep to ensure built_at_ms
    // can strictly advance.
    std::thread::sleep(Duration::from_millis(5));
    f.write("unrelated.txt", "anything\n");
    f.commit("move head");

    let second = f.trace(&["docs", "--graph", "--json"]).json();
    assert_ne!(
        second["graph"]["head"].as_str().unwrap(),
        first_head,
        "git HEAD did not change after a second commit"
    );
    assert!(
        second["graph"]["built_at_ms"].as_u64().unwrap() > first_built_at,
        "moving HEAD did not invalidate the architecture cache entry"
    );
}

#[test]
fn at_include_directives_produce_edges() {
    let f = Fixture::new();
    f.write("included.md", "# included content\n");
    f.write("Claude.md", "# top\n@include included.md\n");
    f.commit("seed includes");

    let v = f.trace(&["docs", "--graph", "--json"]).json();
    let edges = v["graph"]["edges"].as_array().unwrap();
    assert_eq!(edges.len(), 1, "expected exactly one include edge: {v}");
    assert_eq!(edges[0]["source"].as_str().unwrap(), "Claude.md");
    assert_eq!(edges[0]["target"].as_str().unwrap(), "included.md");
    assert_eq!(edges[0]["relation"].as_str().unwrap(), "includes");

    // The include target also lands as a node (kind = `include`).
    let nodes = v["graph"]["nodes"].as_array().unwrap();
    let inc = nodes
        .iter()
        .find(|n| n["path"].as_str() == Some("included.md"))
        .expect("included.md missing from nodes");
    assert_eq!(inc["kind"].as_str().unwrap(), "include");
}

#[test]
fn conditional_rule_frontmatter_promotes_kind_and_attaches_globs() {
    let f = Fixture::new();
    f.write(
        ".claude/rules/r_cond.md",
        "---\npaths:\n  - \"src/**/*.py\"\n  - \"app/*.tsx\"\n---\n# rule body\n",
    );
    f.write(".claude/rules/r_uncond.md", "# plain rule\n");
    f.commit("seed rules");

    let v = f.trace(&["docs", "--graph", "--json"]).json();
    let nodes = v["graph"]["nodes"].as_array().unwrap();

    let cond = nodes
        .iter()
        .find(|n| n["path"].as_str() == Some(".claude/rules/r_cond.md"))
        .expect("conditional rule missing");
    assert_eq!(cond["kind"].as_str().unwrap(), "rules_conditional");
    let globs: Vec<&str> = cond["paths_globs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap())
        .collect();
    assert_eq!(globs, vec!["src/**/*.py", "app/*.tsx"]);

    let uncond = nodes
        .iter()
        .find(|n| n["path"].as_str() == Some(".claude/rules/r_uncond.md"))
        .expect("unconditional rule missing");
    assert_eq!(uncond["kind"].as_str().unwrap(), "rules_unconditional");
    assert!(uncond["paths_globs"].is_null());
}
