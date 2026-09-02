//! `trace docs --graph`: doc-file nodes, `@include` edges, conditional
//! `paths:` frontmatter promotion, and the freshness contract.
//!
//! The doc graph is built in memory on every call and persists nothing.
//! The walk covers doc files only, so there is nothing worth caching and
//! no cached entry that can go stale. These tests pin both halves: the
//! answer always reflects the current tree, and no cache entry is written.

use std::time::Duration;
use tracer_cli_tests::Fixture;

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
fn the_graph_names_every_doc_and_caches_nothing() {
    let f = fixture_with_docs();
    let r = f.trace(&["docs", "--graph", "--json"]);
    r.ok();
    let v = r.view();
    let paths: Vec<&str> = v["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["path"].as_str().unwrap())
        .collect();
    assert_eq!(
        paths,
        vec![
            ".claude/rules/r1.md",
            ".claude/rules/r_cond.md",
            "Claude.md",
            "sub/Claude.md",
        ],
        "the graph must carry every recognized doc file: {v}"
    );
    assert!(
        !f.root.join(".tracer-cache/architecture").exists(),
        "the doc graph must persist nothing — the answer is rebuilt per call"
    );
}

#[test]
fn every_call_rebuilds_from_the_current_tree() {
    let f = fixture_with_docs();
    let first = f.trace(&["docs", "--graph", "--json"]).view();
    let first_built_at = first["built_at_ms"].as_u64().unwrap();
    // Sleep past the millisecond clock so a rebuild produces a strictly
    // larger built_at_ms.
    std::thread::sleep(Duration::from_millis(5));
    let second = f.trace(&["docs", "--graph", "--json"]).view();
    assert!(
        second["built_at_ms"].as_u64().unwrap() > first_built_at,
        "built_at_ms did not advance — an answer was served from a cache"
    );
    assert_eq!(
        second["nodes"], first["nodes"],
        "an unchanged tree must yield the identical node set"
    );
}

#[test]
fn touching_a_doc_file_changes_the_answer() {
    let f = fixture_with_docs();
    let first = f.trace(&["docs", "--graph", "--json"]).view();
    let first_built_at = first["built_at_ms"].as_u64().unwrap();
    let first_aggregate = first["mtime_aggregate"].as_str().unwrap().to_string();

    // Sleep past the filesystem mtime resolution to guarantee a tick, then
    // touch Claude.md by rewriting its bytes.
    std::thread::sleep(Duration::from_millis(20));
    f.write("Claude.md", "# top updated\n");

    let second = f.trace(&["docs", "--graph", "--json"]).view();
    assert_ne!(
        second["mtime_aggregate"].as_str().unwrap(),
        first_aggregate,
        "mtime aggregate did not change after touching Claude.md"
    );
    assert!(
        second["built_at_ms"].as_u64().unwrap() > first_built_at,
        "built_at_ms did not advance after invalidation"
    );
}

#[test]
fn moving_git_head_changes_the_answer() {
    let f = fixture_with_docs();
    let first = f.trace(&["docs", "--graph", "--json"]).view();
    let first_head = first["head"].as_str().unwrap().to_string();
    let first_built_at = first["built_at_ms"].as_u64().unwrap();

    // New commit — same doc files, different HEAD. Sleep to ensure built_at_ms
    // can strictly advance.
    std::thread::sleep(Duration::from_millis(5));
    f.write("unrelated.txt", "anything\n");
    f.commit("move head");

    let second = f.trace(&["docs", "--graph", "--json"]).view();
    assert_ne!(
        second["head"].as_str().unwrap(),
        first_head,
        "git HEAD did not change after a second commit"
    );
    assert!(
        second["built_at_ms"].as_u64().unwrap() > first_built_at,
        "moving HEAD did not invalidate the architecture cache entry"
    );
}

#[test]
fn at_include_directives_produce_edges() {
    let f = Fixture::new();
    f.write("included.md", "# included content\n");
    f.write("Claude.md", "# top\n@include included.md\n");
    f.commit("seed includes");

    let v = f.trace(&["docs", "--graph", "--json"]).view();
    let edges = v["edges"].as_array().unwrap();
    assert_eq!(edges.len(), 1, "expected exactly one include edge: {v}");
    assert_eq!(edges[0]["source"].as_str().unwrap(), "Claude.md");
    assert_eq!(edges[0]["target"].as_str().unwrap(), "included.md");
    assert_eq!(edges[0]["relation"].as_str().unwrap(), "includes");

    // The include target also lands as a node (kind = `include`).
    let nodes = v["nodes"].as_array().unwrap();
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

    let v = f.trace(&["docs", "--graph", "--json"]).view();
    let nodes = v["nodes"].as_array().unwrap();

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
