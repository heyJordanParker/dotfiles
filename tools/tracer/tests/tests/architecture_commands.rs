//! Architecture-graph commands: callers, defines, symbols, upstream,
//! downstream (both symbol and `--path` modes). These read only the
//! `architecture/` cache namespace.
//!
//! The contract these tests pin is not "the expected node appears" — a graph
//! that links everything to everything would pass that. They pin:
//!   * absence — a known-unrelated symbol is asserted NOT a caller / dependency
//!     / dependent (false-positive detection),
//!   * exact transitive reach at explicit depths over a hand-built A→B→C→D
//!     chain (a traversal that ignores depth or collapses to direct-only
//!     fails),
//!   * edge confidence (EXTRACTED vs INFERRED) on a fixture that produces
//!     more than one class,
//!   * exact path-mode centrality / coupling ordering on a fixture whose
//!     ranking is hand-determinable.

use tracer_cli_tests::{standard_repo, Fixture};

// ---------------------------------------------------------------------------
// Fixtures with hand-determined expected graphs
// ---------------------------------------------------------------------------

/// A strict four-module Python import chain `a → b → c → d`, each module
/// importing exactly the next module's single function and calling it.
///
/// Hand-determined graph (verified against the built binary):
///   * forward (dependencies) resolves to *symbol* nodes:
///     `pkg/a.py::a_fn` → `pkg/b.py::b_fn` → `pkg/c.py::c_fn` → `pkg/d.py::d_fn`
///   * reverse (dependents) resolves to *module* nodes:
///     `pkg/d.py::d_fn` ← `module::pkg.c` ← `module::pkg.b` ← `module::pkg.a`
///
/// The directional kind asymmetry (symbol forward, module reverse) is
/// inherent to the graph model — edge sources are modules, edge targets are
/// resolved symbols — and is asserted exactly, not normalized away.
fn chain_repo() -> Fixture {
    let f = Fixture::new();
    f.write("pkg/__init__.py", "");
    f.write(
        "pkg/a.py",
        "from pkg.b import b_fn\n\ndef a_fn(x):\n    return b_fn(x)\n",
    );
    f.write(
        "pkg/b.py",
        "from pkg.c import c_fn\n\ndef b_fn(x):\n    return c_fn(x)\n",
    );
    f.write(
        "pkg/c.py",
        "from pkg.d import d_fn\n\ndef c_fn(x):\n    return d_fn(x)\n",
    );
    f.write("pkg/d.py", "def d_fn(x):\n    return x + 1\n");
    // `lone.py` is an island: defined, never imported, imports nothing
    // internal. It must never appear in any chain query — the absence anchor.
    f.write("lone.py", "def lone_fn():\n    return 0\n");
    f.commit("chain repo");
    f
}

/// Helper: collect `node_id`s from a dependency/dependent array.
fn node_ids(arr: &serde_json::Value) -> Vec<String> {
    arr.as_array()
        .unwrap()
        .iter()
        .map(|d| d["node_id"].as_str().unwrap().to_string())
        .collect()
}

// ---------------------------------------------------------------------------
// callers
// ---------------------------------------------------------------------------

#[test]
fn callers_resolves_cross_file_importer() {
    let f = standard_repo();
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "helper", "--json"]);
    r.ok();
    let v = r.json();
    // helper is defined in src/util.py and imported by src/app.py.
    let entry = &v["src/util.py::helper"];
    assert_eq!(entry["symbol"], "helper");
    let callers = entry["callers"].as_array().unwrap();
    assert!(
        callers
            .iter()
            .any(|c| c["source_file"].as_str() == Some("src/app.py")),
        "app.py should be a caller of helper: {:?}",
        callers
    );
}

#[test]
fn callers_excludes_unrelated_symbol() {
    // Absence guard: `lone_fn` imports nothing and is imported by nobody, so
    // no module may be reported as its caller. A graph that links everything
    // would report a bogus caller here.
    let f = chain_repo();
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "lone_fn", "--json"]);
    r.ok();
    let v = r.json();
    let callers = v["lone.py::lone_fn"]["callers"].as_array().unwrap();
    assert!(
        callers.is_empty(),
        "lone_fn has no importer; callers must be empty, got {:?}",
        callers
    );

    // And `d_fn`'s only caller is module::pkg.c — never pkg.a / pkg.b /
    // anything else. Asserting the *exact* caller set catches an
    // over-connected graph.
    let r = f.trace(&["callers", "d_fn", "--json"]);
    r.ok();
    let v = r.json();
    let callers = v["pkg/d.py::d_fn"]["callers"].as_array().unwrap();
    let ids = node_ids(&v["pkg/d.py::d_fn"]["callers"]);
    assert_eq!(
        ids,
        vec!["module::pkg.c".to_string()],
        "d_fn's only caller is module::pkg.c: {:?}",
        callers
    );
    assert!(
        !ids.iter().any(|i| i == "module::pkg.a" || i == "module::pkg.b"),
        "transitive importers must not appear as direct callers: {:?}",
        ids
    );
}

#[test]
fn callers_unknown_symbol_exits_2() {
    let f = standard_repo();
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "NoSuchSymbol_zzz"]);
    r.code_is(2);
    assert!(r.combined().contains("not found"), "{}", r.combined());
}

// ---------------------------------------------------------------------------
// defines
// ---------------------------------------------------------------------------

#[test]
fn defines_locates_symbol_definition() {
    let f = standard_repo();
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["defines", "helper", "--json"]);
    r.ok();
    let v = r.json();
    assert_eq!(v["symbol"], "helper");
    // helper is defined in exactly one fixture file (src/util.py); the
    // absence assertion below proves no other file defines it, so the
    // count is exactly 1.
    assert_eq!(
        v["definition_count"].as_i64().unwrap(),
        1,
        "helper is defined exactly once in the fixture: {}",
        v
    );
    let defs = v["definitions"].as_array().unwrap();
    assert!(
        defs.iter()
            .any(|d| d["source_file"].as_str() == Some("src/util.py")),
        "helper should be defined in src/util.py: {:?}",
        defs
    );
    // Absence: helper is not defined in any other fixture file.
    assert!(
        !defs.iter().any(|d| {
            let sf = d["source_file"].as_str().unwrap_or("");
            sf == "src/app.py" || sf == "lib/widget.php" || sf == "src/front.tsx"
        }),
        "helper must only be defined in src/util.py: {:?}",
        defs
    );
}

#[test]
fn defines_unknown_symbol_exits_2() {
    let f = standard_repo();
    f.trace(&["cache", "build", "."]).ok();
    f.trace(&["defines", "definitely_absent_qqq"]).code_is(2);
}

// ---------------------------------------------------------------------------
// symbols
// ---------------------------------------------------------------------------

#[test]
fn symbols_lists_module_level_symbols_of_a_file() {
    let f = standard_repo();
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["symbols", "src/util.py", "--json"]);
    r.ok();
    let v = r.json();
    assert_eq!(v["file"], "src/util.py");
    let names: Vec<&str> = v["symbols"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["label"].as_str().unwrap())
        .collect();
    // src/util.py defines exactly one module-level symbol, `helper`; the
    // exact set is pinned (this also subsumes the cross-file absence
    // check, kept below as an explicit guard against a regression that
    // would add foreign symbols).
    assert_eq!(
        names,
        vec!["helper"],
        "src/util.py symbols must be exactly [helper]: {:?}",
        names
    );
    // Absence: a symbol that lives in a different file must not be listed
    // for src/util.py.
    assert!(
        !names.contains(&"main") && !names.contains(&"render"),
        "src/util.py must not list symbols from other files: {:?}",
        names
    );
}

// ---------------------------------------------------------------------------
// upstream — transitive dependencies, exact depth
// ---------------------------------------------------------------------------

#[test]
fn upstream_symbol_mode_returns_dependencies() {
    let f = standard_repo();
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["upstream", "main", "--json"]);
    r.ok();
    let v = r.json();
    let deps = v["src/app.py::main"]["dependencies"].as_array().unwrap();
    assert!(
        deps.iter()
            .any(|d| d["node_id"].as_str() == Some("src/util.py::helper")),
        "main should depend on helper: {:?}",
        deps
    );
}

#[test]
fn upstream_transitive_reach_is_exact_per_depth() {
    // a_fn → b_fn → c_fn → d_fn. Forward edges resolve to symbol nodes.
    // depth N must include exactly the first N hops — no more, no fewer.
    let f = chain_repo();
    f.trace(&["cache", "build", "."]).ok();

    let at = |depth: &str| -> Vec<String> {
        let r = f.trace(&["upstream", "a_fn", "--depth", depth, "--json"]);
        r.ok();
        let v = r.json();
        let mut ids = node_ids(&v["pkg/a.py::a_fn"]["dependencies"]);
        ids.sort();
        ids
    };

    // Depth 1: only the direct dependency.
    assert_eq!(
        at("1"),
        vec!["pkg/b.py::b_fn".to_string()],
        "depth 1 must be direct-only"
    );
    // Depth 2: intermediate reach.
    assert_eq!(
        at("2"),
        vec!["pkg/b.py::b_fn".to_string(), "pkg/c.py::c_fn".to_string()],
        "depth 2 must reach exactly two hops"
    );
    // Depth 3 (== full chain length): full reach.
    assert_eq!(
        at("3"),
        vec![
            "pkg/b.py::b_fn".to_string(),
            "pkg/c.py::c_fn".to_string(),
            "pkg/d.py::d_fn".to_string()
        ],
        "depth 3 must reach the whole chain"
    );
    // Depth 9 (over-deep): identical to full reach — no phantom nodes, the
    // island never appears.
    assert_eq!(at("9"), at("3"), "over-deep traversal must not invent nodes");
    assert!(
        !at("9").iter().any(|i| i.contains("lone")),
        "the unrelated island must never appear in a dependency chain"
    );
}

#[test]
fn upstream_missing_arg_exits_2() {
    let f = standard_repo();
    let r = f.trace(&["upstream"]);
    r.code_is(2);
    assert!(r.combined().contains("SYMBOL or --path"), "{}", r.combined());
}

// ---------------------------------------------------------------------------
// downstream — transitive dependents, exact depth
// ---------------------------------------------------------------------------

#[test]
fn downstream_symbol_mode_returns_dependents() {
    let f = standard_repo();
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["downstream", "helper", "--json"]);
    r.ok();
    let v = r.json();
    let dependents = v["src/util.py::helper"]["dependents"].as_array().unwrap();
    assert!(
        !dependents.is_empty(),
        "helper has a dependent (app.py imports it): {:?}",
        dependents
    );
}

#[test]
fn downstream_transitive_reach_is_exact_per_depth() {
    // d_fn ← pkg.c ← pkg.b ← pkg.a. Reverse edges resolve to module nodes.
    // This is the case that exposed the dead-end-after-one-hop defect:
    // before the fix, depth 2/3/9 all returned only `module::pkg.c`.
    let f = chain_repo();
    f.trace(&["cache", "build", "."]).ok();

    let at = |depth: &str| -> Vec<String> {
        let r = f.trace(&["downstream", "d_fn", "--depth", depth, "--json"]);
        r.ok();
        let v = r.json();
        let mut ids = node_ids(&v["pkg/d.py::d_fn"]["dependents"]);
        ids.sort();
        ids
    };

    assert_eq!(
        at("1"),
        vec!["module::pkg.c".to_string()],
        "depth 1 must be the direct dependent only"
    );
    assert_eq!(
        at("2"),
        vec!["module::pkg.b".to_string(), "module::pkg.c".to_string()],
        "depth 2 must climb exactly two hops"
    );
    assert_eq!(
        at("3"),
        vec![
            "module::pkg.a".to_string(),
            "module::pkg.b".to_string(),
            "module::pkg.c".to_string()
        ],
        "depth 3 must climb the whole chain"
    );
    assert_eq!(at("9"), at("3"), "over-deep traversal must not invent nodes");
    assert!(
        !at("9").iter().any(|i| i.contains("lone")),
        "the unrelated island must never appear in a dependent chain"
    );
}

#[test]
fn downstream_excludes_unrelated_symbol() {
    // Absence: the island has no dependents at any depth.
    let f = chain_repo();
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["downstream", "lone_fn", "--depth", "9", "--json"]);
    r.ok();
    let v = r.json();
    let dependents = v["lone.py::lone_fn"]["dependents"].as_array().unwrap();
    assert!(
        dependents.is_empty(),
        "lone_fn is imported by nobody; dependents must be empty: {:?}",
        dependents
    );
}

#[test]
fn downstream_missing_arg_exits_2() {
    let f = standard_repo();
    let r = f.trace(&["downstream"]);
    r.code_is(2);
    assert!(r.combined().contains("SYMBOL or --path"), "{}", r.combined());
}

// ---------------------------------------------------------------------------
// edge confidence
// ---------------------------------------------------------------------------

#[test]
fn callers_reports_confidence_classes() {
    // One fixture, two confidence classes:
    //   * clean.py: `from uniq import only_here` — module `uniq` resolves and
    //     the symbol resolves → EXTRACTED.
    //   * imp.py: `from external_unresolvable_pkg import rare_unique_name` —
    //     module path does not resolve but the uniquely-named symbol does
    //     (1 candidate, no target module) → INFERRED.
    let f = Fixture::new();
    f.write("uniq.py", "def only_here():\n    return 1\n");
    f.write(
        "clean.py",
        "from uniq import only_here\n\ndef use():\n    return only_here()\n",
    );
    f.write("target.py", "def rare_unique_name():\n    return 1\n");
    f.write(
        "imp.py",
        "from external_unresolvable_pkg import rare_unique_name\n\ndef u():\n    return rare_unique_name()\n",
    );
    f.commit("confidence repo");
    f.trace(&["cache", "build", "."]).ok();

    let r = f.trace(&["callers", "only_here", "--json"]);
    r.ok();
    let v = r.json();
    let callers = v["uniq.py::only_here"]["callers"].as_array().unwrap();
    assert_eq!(callers.len(), 1, "only_here has one caller: {:?}", callers);
    assert_eq!(
        callers[0]["confidence"].as_str(),
        Some("EXTRACTED"),
        "resolvable module+symbol import must be EXTRACTED: {:?}",
        callers[0]
    );

    let r = f.trace(&["callers", "rare_unique_name", "--json"]);
    r.ok();
    let v = r.json();
    let callers = v["target.py::rare_unique_name"]["callers"]
        .as_array()
        .unwrap();
    assert_eq!(callers.len(), 1, "rare has one caller: {:?}", callers);
    assert_eq!(
        callers[0]["confidence"].as_str(),
        Some("INFERRED"),
        "unresolvable-module symbol import must be INFERRED: {:?}",
        callers[0]
    );
}

// ---------------------------------------------------------------------------
// path mode — exact centrality / coupling ordering
// ---------------------------------------------------------------------------

#[test]
fn downstream_path_mode_ranks_central_nodes_exactly() {
    // In the chain a→b→c→d, the most-depended-on symbol is d_fn (3 transitive
    // dependents), then c_fn (2), then b_fn (1). a_fn has 0 dependents and
    // must not rank. Exact ordering — not "is an array".
    let f = chain_repo();
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["downstream", "--path", ".", "--json"]);
    r.ok();
    let v = r.json();
    assert_eq!(v["mode"], "downstream");
    let rows = v["results"].as_array().unwrap();

    let triples: Vec<(String, i64, i64)> = rows
        .iter()
        .map(|x| {
            (
                x["node_id"].as_str().unwrap().to_string(),
                x["rank"].as_i64().unwrap(),
                x["transitive_dependents"].as_i64().unwrap(),
            )
        })
        .collect();

    assert_eq!(
        triples,
        vec![
            ("pkg/d.py::d_fn".to_string(), 1, 3),
            ("pkg/c.py::c_fn".to_string(), 2, 2),
            ("pkg/b.py::b_fn".to_string(), 3, 1),
        ],
        "centrality ranking must be exactly d_fn > c_fn > b_fn by \
         transitive dependents; got {:?}",
        triples
    );
    // a_fn is a leaf importer with no dependents — it must not rank.
    assert!(
        !triples.iter().any(|(id, _, _)| id == "pkg/a.py::a_fn"),
        "a_fn has no dependents and must not appear: {:?}",
        triples
    );
}

#[test]
fn upstream_path_mode_ranks_high_coupling_nodes_exactly() {
    // Highest fan-out (transitive dependencies): pkg.a (3) > pkg.b (2) >
    // pkg.c (1). pkg.d depends on nothing internal and must not rank.
    let f = chain_repo();
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["upstream", "--path", ".", "--json"]);
    r.ok();
    let v = r.json();
    assert_eq!(v["mode"], "upstream");
    let rows = v["results"].as_array().unwrap();

    let triples: Vec<(String, i64, i64)> = rows
        .iter()
        .map(|x| {
            (
                x["node_id"].as_str().unwrap().to_string(),
                x["rank"].as_i64().unwrap(),
                x["transitive_dependencies"].as_i64().unwrap(),
            )
        })
        .collect();

    assert_eq!(
        triples,
        vec![
            ("module::pkg.a".to_string(), 1, 3),
            ("module::pkg.b".to_string(), 2, 2),
            ("module::pkg.c".to_string(), 3, 1),
        ],
        "coupling ranking must be exactly pkg.a > pkg.b > pkg.c by \
         transitive dependencies; got {:?}",
        triples
    );
    assert!(
        !triples.iter().any(|(id, _, _)| id == "module::pkg.d"),
        "pkg.d has no internal dependencies and must not appear: {:?}",
        triples
    );
}

#[test]
fn downstream_path_mode_respects_limit() {
    let f = chain_repo();
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["downstream", "--path", ".", "--limit", "1", "--json"]);
    r.ok();
    let v = r.json();
    let rows = v["results"].as_array().unwrap();
    assert_eq!(rows.len(), 1, "--limit 1 must cap results: {:?}", rows);
    // The single survivor must be the top-ranked one (d_fn), not an
    // arbitrary node.
    assert_eq!(
        rows[0]["node_id"].as_str(),
        Some("pkg/d.py::d_fn"),
        "limit must keep the highest-centrality node: {:?}",
        rows[0]
    );
}
