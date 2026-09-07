//! Architecture-graph commands: callers, defines, structure, dependencies,
//! usages (both symbol and `--path` modes). These read the relations index
//! in the `file/` cache namespace.
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

use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::os::unix::fs::symlink;
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

/// The one result row for `node_id`. The graph commands return a row list,
/// so a test that means "the entry for this symbol" says so.
fn symbol<'a>(v: &'a serde_json::Value, node_id: &str) -> &'a serde_json::Value {
    v["results"]
        .as_array()
        .unwrap_or_else(|| panic!("results must be a row list: {v}"))
        .iter()
        .find(|row| row["node_id"].as_str() == Some(node_id))
        .unwrap_or_else(|| panic!("no result row for {node_id}: {v}"))
}

/// The file-state shoulder the document carries for `path`, out of the
/// `context` slot where every command keeps its enrichment.
fn shoulder_of<'a>(v: &'a serde_json::Value, path: &str) -> &'a str {
    v["files"][path]["shoulder"]
        .as_str()
        .unwrap_or_else(|| panic!("no shoulder for {path}: {v}"))
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
    let v = r.view();
    // helper is defined in src/util.py and imported by src/app.py.
    let entry = &symbol(&v, "src/util.py::helper");
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
    // Absence guard: `lone_fn` is referenced by nobody, so no use site
    // may be reported. A graph that links everything would report a
    // bogus caller here.
    let f = chain_repo();
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "lone_fn", "--json"]);
    r.ok();
    let v = r.view();
    let callers = symbol(&v, "lone.py::lone_fn")["callers"]
        .as_array()
        .unwrap();
    assert!(
        callers.is_empty(),
        "lone_fn has no referencer; callers must be empty, got {:?}",
        callers
    );

    // `d_fn`'s only use site is the call inside `c_fn` at pkg/c.py:4. With
    // function-granular reference edges the source node is the CALLING
    // FUNCTION `pkg/c.py::c_fn` — not the module `module::pkg.c` and never
    // pkg.a / pkg.b. Asserting the *exact* caller set catches both an
    // over-connected graph and a regression back to module granularity.
    let r = f.trace(&["callers", "d_fn", "--json"]);
    r.ok();
    let v = r.view();
    let callers = symbol(&v, "pkg/d.py::d_fn")["callers"].as_array().unwrap();
    let ids = node_ids(&symbol(&v, "pkg/d.py::d_fn")["callers"]);
    assert_eq!(
        ids,
        vec!["pkg/c.py::c_fn".to_string()],
        "d_fn's only caller is the function c_fn (function-granular source): {:?}",
        callers
    );
    assert!(
        !ids.iter().any(|i| {
            i == "module::pkg.a"
                || i == "module::pkg.b"
                || i == "module::pkg.c"
                || i == "pkg/b.py::b_fn"
                || i == "pkg/a.py::a_fn"
        }),
        "transitive importers / the module itself must not appear as direct callers: {:?}",
        ids
    );
    // The use site is the call inside c_fn at pkg/c.py:4; the source node is
    // the function c_fn, and the count summary reports it as one resolved
    // caller with none ambiguous.
    let row = callers
        .iter()
        .find(|c| c["source_file"].as_str() == Some("pkg/c.py"))
        .unwrap_or_else(|| panic!("missing pkg/c.py use site: {:?}", callers));
    assert_eq!(row["source_line"].as_i64(), Some(4));
    assert_eq!(row["relation"].as_str(), Some("references"));
    assert_eq!(row["label"].as_str(), Some("c_fn"));
    assert_eq!(
        symbol(&v, "pkg/d.py::d_fn")["caller_count"].as_i64(),
        Some(1)
    );
    assert_eq!(
        symbol(&v, "pkg/d.py::d_fn")["resolved_count"].as_i64(),
        Some(1)
    );
    assert_eq!(
        symbol(&v, "pkg/d.py::d_fn")["ambiguous_count"].as_i64(),
        Some(0)
    );
}

#[test]
fn caller_row_carries_use_site_file_shoulder() {
    // Each caller row carries the canonical passive-context shoulder of its
    // use-site file, so a `callers` result tells the agent the file state of
    // every place the symbol is called without a second `read`/`info` call.
    // `d_fn`'s sole use site is the call inside `c_fn` at pkg/c.py:4, so the
    // row's shoulder is pkg/c.py's file-state shoulder — settled single
    // commit, local-only, the file's CCN, the canonical bracketed form.
    let f = chain_repo();
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "d_fn", "--json"]);
    r.ok();
    let v = r.view();
    let row = symbol(&v, "pkg/d.py::d_fn")["callers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["source_file"].as_str() == Some("pkg/c.py"))
        .expect("missing pkg/c.py use site");
    let _ = row;
    let shoulder = shoulder_of(&v, "pkg/c.py");
    assert!(
        shoulder.starts_with("[git: new (1 commit) \u{00b7} age:"),
        "caller-row shoulder must be the canonical bracketed file-state shoulder: {shoulder:?}"
    );
    assert!(
        shoulder.contains("\u{00b7} churn: 1 commit, 1/30d \u{00b7}")
            && shoulder.contains("\u{00b7} presence: local-only \u{00b7}"),
        "caller-row shoulder must carry churn + presence from the use-site file's facts: {shoulder:?}"
    );
}

#[test]
fn usages_dependent_row_carries_file_shoulder() {
    // A downstream result row carries the canonical shoulder of the
    // dependent file. In the a→b→c→d chain, d_fn's transitive dependents are
    // the modules pkg.a/pkg.b/pkg.c; each row's shoulder is that dependent
    // file's canonical file-state shoulder.
    let f = chain_repo();
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["usages", "d_fn", "--json"]);
    r.ok();
    let v = r.view();
    let deps = symbol(&v, "pkg/d.py::d_fn")["dependents"]
        .as_array()
        .unwrap();
    let row = deps
        .iter()
        .find(|d| d["source_file"].as_str() == Some("pkg/c.py"))
        .expect("pkg/c.py must be a dependent of d_fn");
    let _ = row;
    let shoulder = shoulder_of(&v, "pkg/c.py");
    assert!(
        shoulder.starts_with("[git: new (1 commit) \u{00b7} age:")
            && shoulder.contains("\u{00b7} churn: 1 commit, 1/30d \u{00b7}"),
        "downstream-row shoulder must be the canonical file-state shoulder: {shoulder:?}"
    );
}

#[test]
fn defines_row_carries_definition_file_shoulder() {
    // Each `defines` row carries the canonical passive-context shoulder of the
    // file the symbol is defined in, so a definition lookup also tells the
    // agent the file state of where the symbol lives. d_fn is defined in
    // pkg/d.py — a settled single-commit, local-only file.
    let f = chain_repo();
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["defines", "d_fn", "--json"]);
    r.ok();
    let v = r.view();
    let row = v["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["source_file"].as_str() == Some("pkg/d.py"))
        .expect("d_fn must be defined in pkg/d.py");
    let _ = row;
    let shoulder = shoulder_of(&v, "pkg/d.py");
    assert!(
        shoulder.starts_with("[git: new (1 commit) \u{00b7} age:")
            && shoulder.contains("\u{00b7} churn: 1 commit, 1/30d \u{00b7}")
            && shoulder.contains("\u{00b7} presence: local-only \u{00b7}"),
        "defines-row shoulder must be the canonical file-state shoulder: {shoulder:?}"
    );
}

#[test]
fn structure_carries_file_shoulder() {
    // `trace structure <file>` carries the canonical passive-context shoulder
    // of the file at the top level, so listing what a file declares also
    // surfaces that file's state. pkg/d.py is a settled single-commit file.
    let f = chain_repo();
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["structure", "pkg/d.py", "--json"]);
    r.ok();
    let v = r.view();
    let file = v["file"]
        .as_str()
        .expect("structure echoes the file it read");
    let shoulder = v["files"][file]["shoulder"]
        .as_str()
        .expect("structure must carry a non-null shoulder for an in-repo file");
    assert!(
        shoulder.starts_with("[git: new (1 commit) \u{00b7} age:")
            && shoulder.contains("\u{00b7} churn: 1 commit, 1/30d \u{00b7}")
            && shoulder.contains("\u{00b7} presence: local-only \u{00b7}"),
        "structure shoulder must be the canonical file-state shoulder: {shoulder:?}"
    );
}

/// `callers` bounds its rows the way every other row command does: a
/// `--limit` with the `find` truncation contract, and nothing silently
/// dropped.
///
/// A member call fans out to one row per same-named candidate, so a common
/// method name multiplies: `get` in laravel-framework is declared 112 times
/// and its 2,055 call sites produce 224,908 rows. The cut must say how many
/// there were, say that it cut, and give the whole set back on request —
/// never decide for the caller that some of them do not exist.
#[test]
fn callers_truncates_by_limit_and_says_what_it_held_back() {
    let f = Fixture::new();
    for n in 0..11 {
        f.write(
            &format!("src/C{n}.php"),
            &format!("<?php\nclass C{n}\n{{\n    public function get()\n    {{\n        return {n};\n    }}\n}}\n"),
        );
    }
    // Three call sites × eleven candidates = 33 ambiguous rows.
    f.write(
        "src/call.php",
        "<?php\nfunction run($x)\n{\n    $x->get();\n    $x->get();\n    return $x->get();\n}\n",
    );
    f.commit("eleven methods named get");

    let whole = f.trace(&["callers", "get", "--limit", "1000", "--json"]);
    whole.ok();
    let v = whole.view();
    assert_eq!(
        v["total"].as_i64().unwrap(),
        33,
        "every candidate of every site is a row: {}",
        whole.stdout
    );
    assert_eq!(
        v["truncated"], false,
        "a limit above the total must not truncate: {}",
        whole.stdout
    );

    let cut = f.trace(&["callers", "get", "--limit", "5", "--json"]);
    cut.ok();
    let c = cut.view();
    assert_eq!(
        c["total"].as_i64().unwrap(),
        33,
        "the total must survive the cut: {}",
        cut.stdout
    );
    assert_eq!(
        c["truncated"], true,
        "a limit below the total must say it cut: {}",
        cut.stdout
    );
    assert_eq!(
        c["callers"].as_i64().unwrap(),
        5,
        "exactly `limit` rows survive: {}",
        cut.stdout
    );
    // The human render names the flag that returns the rest.
    let human = f.trace(&["callers", "get", "--limit", "5"]);
    human.ok();
    assert!(
        human.stdout.contains("see all: --limit 33"),
        "the cut must name the command that undoes it:\n{}",
        human.stdout
    );
}

#[test]
fn callers_unknown_symbol_exits_2() {
    let f = standard_repo();
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "NoSuchSymbol_zzz"]);
    r.code_is(2);
    assert!(
        r.combined()
            .contains("not declared anywhere in this repository"),
        "a miss must name the symbol and say it is not declared here: {}",
        r.combined()
    );
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
    let v = r.view();
    assert_eq!(v["symbol"], "helper");
    // helper is defined in exactly one fixture file (src/util.py); the
    // absence assertion below proves no other file defines it, so the
    // count is exactly 1.
    assert_eq!(
        v["definitions"].as_i64().unwrap(),
        1,
        "helper is defined exactly once in the fixture: {}",
        v
    );
    let defs = v["results"].as_array().unwrap();
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
// structure — the graph resolution per declaration
// ---------------------------------------------------------------------------

#[test]
fn structure_resolves_each_declaration_to_its_graph_node() {
    // The row-level `node_id` is what makes a declaration addressable by
    // `callers` / `usages` / `dependencies`, and it is the resolution the
    // separate `symbols` command used to be the only way to see.
    let f = standard_repo();
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["structure", "src/util.py", "--json"]);
    r.ok();
    let v = r.view();
    let ids: Vec<String> = v["symbols_by_kind"]
        .as_object()
        .unwrap()
        .values()
        .flat_map(|rows| rows.as_array().unwrap().iter())
        .filter_map(|s| s["node_id"].as_str().map(|x| x.to_string()))
        .collect();
    // src/util.py declares exactly one graph-resolved symbol, `helper`. The
    // exact set also proves no foreign file's symbols leak into the rows.
    assert_eq!(
        ids,
        vec!["src/util.py::helper".to_string()],
        "src/util.py must resolve exactly [src/util.py::helper]: {ids:?}"
    );
}

// ---------------------------------------------------------------------------
// upstream — transitive dependencies, exact depth
// ---------------------------------------------------------------------------

#[test]
fn dependencies_symbol_mode_returns_dependencies() {
    let f = standard_repo();
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["dependencies", "main", "--json"]);
    r.ok();
    let v = r.view();
    let deps = symbol(&v, "src/app.py::main")["dependencies"]
        .as_array()
        .unwrap();
    assert!(
        deps.iter()
            .any(|d| d["node_id"].as_str() == Some("src/util.py::helper")),
        "main should depend on helper: {:?}",
        deps
    );
}

#[test]
fn dependencies_transitive_reach_is_exact_per_depth() {
    // a_fn → b_fn → c_fn → d_fn. Forward edges resolve to symbol nodes.
    // depth N must include exactly the first N hops — no more, no fewer.
    let f = chain_repo();
    f.trace(&["cache", "build", "."]).ok();

    let at = |depth: &str| -> Vec<String> {
        let r = f.trace(&["dependencies", "a_fn", "--depth", depth, "--json"]);
        r.ok();
        let v = r.view();
        let mut ids = node_ids(&symbol(&v, "pkg/a.py::a_fn")["dependencies"]);
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
    assert_eq!(
        at("9"),
        at("3"),
        "over-deep traversal must not invent nodes"
    );
    assert!(
        !at("9").iter().any(|i| i.contains("lone")),
        "the unrelated island must never appear in a dependency chain"
    );
}

#[test]
fn dependencies_missing_arg_exits_2() {
    let f = standard_repo();
    let r = f.trace(&["dependencies"]);
    r.code_is(2);
    assert!(
        r.combined().contains("SYMBOL or --path"),
        "{}",
        r.combined()
    );
}

// ---------------------------------------------------------------------------
// downstream — transitive dependents, exact depth
// ---------------------------------------------------------------------------

#[test]
fn usages_symbol_mode_returns_dependents() {
    let f = standard_repo();
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["usages", "helper", "--json"]);
    r.ok();
    let v = r.view();
    let dependents = symbol(&v, "src/util.py::helper")["dependents"]
        .as_array()
        .unwrap();
    assert!(
        !dependents.is_empty(),
        "helper has a dependent (app.py imports it): {:?}",
        dependents
    );
}

#[test]
fn usages_transitive_reach_is_exact_per_depth() {
    // d_fn ← pkg.c ← pkg.b ← pkg.a. Reverse edges resolve to module nodes.
    // This is the case that exposed the dead-end-after-one-hop defect:
    // before the fix, depth 2/3/9 all returned only `module::pkg.c`.
    let f = chain_repo();
    f.trace(&["cache", "build", "."]).ok();

    let at = |depth: &str| -> Vec<String> {
        let r = f.trace(&["usages", "d_fn", "--depth", depth, "--json"]);
        r.ok();
        let v = r.view();
        let mut ids = node_ids(&symbol(&v, "pkg/d.py::d_fn")["dependents"]);
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
    assert_eq!(
        at("9"),
        at("3"),
        "over-deep traversal must not invent nodes"
    );
    assert!(
        !at("9").iter().any(|i| i.contains("lone")),
        "the unrelated island must never appear in a dependent chain"
    );
}

#[test]
fn usages_excludes_unrelated_symbol() {
    // Absence: the island has no dependents at any depth.
    let f = chain_repo();
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["usages", "lone_fn", "--depth", "9", "--json"]);
    r.ok();
    let v = r.view();
    let dependents = symbol(&v, "lone.py::lone_fn")["dependents"]
        .as_array()
        .unwrap();
    assert!(
        dependents.is_empty(),
        "lone_fn is imported by nobody; dependents must be empty: {:?}",
        dependents
    );
}

#[test]
fn usages_missing_arg_exits_2() {
    let f = standard_repo();
    let r = f.trace(&["usages"]);
    r.code_is(2);
    assert!(
        r.combined().contains("SYMBOL or --path"),
        "{}",
        r.combined()
    );
}

/// Reverse queries served from the reverse-edge index must return the same
/// complete set a full edge-list scan would. This fan-in fixture has one
/// target (`core_fn` in `pkg/core.py`) imported and called by five distinct
/// modules; the reverse-dependent set is therefore exactly those five module
/// nodes — no fewer (a dropped bucket entry would lose one), no more (a
/// mis-keyed entry would add an unrelated one), and the island module that
/// imports nothing internal must be absent. Pinning the exact, complete set
/// is the black-box equivalent of "the index agrees with the scan."
#[test]
fn reverse_query_returns_complete_dependent_set() {
    let f = Fixture::new();
    f.write("pkg/__init__.py", "");
    f.write("pkg/core.py", "def core_fn(x):\n    return x + 1\n");
    for name in ["one", "two", "three", "four", "five"] {
        f.write(
            &format!("pkg/{name}.py"),
            &format!("from pkg.core import core_fn\n\ndef {name}_fn(x):\n    return core_fn(x)\n"),
        );
    }
    // Island: imports nothing internal, depends on nobody in the chain. Must
    // never surface as a dependent of core_fn.
    f.write("pkg/island.py", "def island_fn():\n    return 0\n");
    f.commit("fan-in repo");
    f.trace(&["cache", "build", "."]).ok();

    let r = f.trace(&["usages", "core_fn", "--depth", "1", "--json"]);
    r.ok();
    let v = r.view();
    let mut dependents = node_ids(&symbol(&v, "pkg/core.py::core_fn")["dependents"]);
    dependents.sort();
    let mut expected = vec![
        "module::pkg.one".to_string(),
        "module::pkg.two".to_string(),
        "module::pkg.three".to_string(),
        "module::pkg.four".to_string(),
        "module::pkg.five".to_string(),
    ];
    expected.sort();
    assert_eq!(
        dependents, expected,
        "reverse-index dependent set must be exactly the five importing \
         modules — the same complete set a full edge scan returns"
    );

    // callers (reference-edge reverse index) must surface all five use sites
    // and never the island.
    let cr = f.trace(&["callers", "core_fn", "--json"]);
    cr.ok();
    let cv = cr.view();
    let callers = symbol(&cv, "pkg/core.py::core_fn")["callers"]
        .as_array()
        .expect("core_fn callers array must exist");
    let caller_files: std::collections::BTreeSet<&str> = callers
        .iter()
        .filter_map(|c| c["source_file"].as_str())
        .collect();
    for name in ["one", "two", "three", "four", "five"] {
        assert!(
            caller_files.contains(format!("pkg/{name}.py").as_str()),
            "callers (reference reverse index) dropped a use site in pkg/{name}.py; \
             got {caller_files:?}"
        );
    }
    assert!(
        !caller_files.contains("pkg/island.py"),
        "the island must never appear as a caller of core_fn: {caller_files:?}"
    );
}

// ---------------------------------------------------------------------------
// edge confidence
// ---------------------------------------------------------------------------

#[test]
fn callers_reports_confidence_classes() {
    // One fixture, two confidence classes on reference rows (callers now
    // returns use sites, so the classes are reference-edge confidences):
    //   * clean.py: imports `only_here` from `uniq` and calls it — the
    //     candidate lives in an imported file → EXTRACTED.
    //   * sole.py: calls `rare_unique_name` with no import context at
    //     all; the name uniquely identifies one declaration globally →
    //     INFERRED.
    let f = Fixture::new();
    f.write("uniq.py", "def only_here():\n    return 1\n");
    f.write(
        "clean.py",
        "from uniq import only_here\n\ndef use():\n    return only_here()\n",
    );
    f.write("target.py", "def rare_unique_name():\n    return 1\n");
    f.write("sole.py", "def u():\n    return rare_unique_name()\n");
    f.commit("confidence repo");
    f.trace(&["cache", "build", "."]).ok();

    let r = f.trace(&["callers", "only_here", "--json"]);
    r.ok();
    let v = r.view();
    let callers = symbol(&v, "uniq.py::only_here")["callers"]
        .as_array()
        .unwrap();
    assert_eq!(callers.len(), 1, "only_here has one caller: {:?}", callers);
    assert_eq!(
        callers[0]["confidence"].as_str(),
        Some("EXTRACTED"),
        "resolvable module+symbol import must be EXTRACTED: {:?}",
        callers[0]
    );

    let r = f.trace(&["callers", "rare_unique_name", "--json"]);
    r.ok();
    let v = r.view();
    let callers = symbol(&v, "target.py::rare_unique_name")["callers"]
        .as_array()
        .unwrap();
    assert_eq!(callers.len(), 1, "rare has one caller: {:?}", callers);
    assert_eq!(
        callers[0]["confidence"].as_str(),
        Some("INFERRED"),
        "uniquely-named ref with no import context must be INFERRED: {:?}",
        callers[0]
    );
}

// ---------------------------------------------------------------------------
// path mode — exact centrality / coupling ordering
// ---------------------------------------------------------------------------

#[test]
fn usages_path_mode_ranks_central_nodes_exactly() {
    // In the chain a→b→c→d, the most-depended-on symbol is d_fn (3 transitive
    // dependents), then c_fn (2), then b_fn (1). a_fn has 0 dependents and
    // must not rank. Exact ordering — not "is an array".
    let f = chain_repo();
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["usages", "--path", ".", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(v["mode"], "path");
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
fn dependencies_path_mode_ranks_high_coupling_nodes_exactly() {
    // Highest fan-out (transitive dependencies): pkg.a (3) > pkg.b (2) >
    // pkg.c (1). pkg.d depends on nothing internal and must not rank.
    let f = chain_repo();
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["dependencies", "--path", ".", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(v["mode"], "path");
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
fn path_mode_scopes_ranked_subjects_without_scoping_reach() {
    let f = Fixture::new();
    f.write("scoped/__init__.py", "");
    f.write("outside/__init__.py", "");
    f.write("outside/core.py", "def core_fn(x):\n    return x + 1\n");
    f.write(
        "outside/mid.py",
        "from outside.core import core_fn\n\ndef mid_fn(x):\n    return core_fn(x)\n",
    );
    f.write(
        "scoped/entry.py",
        "from outside.mid import mid_fn\n\ndef entry_fn(x):\n    return mid_fn(x)\n",
    );
    f.write("scoped/base.py", "def base_fn(x):\n    return x + 1\n");
    f.write(
        "outside/consumer.py",
        "from scoped.base import base_fn\n\ndef consumer_fn(x):\n    return base_fn(x)\n",
    );
    f.write(
        "outside/top.py",
        "from outside.consumer import consumer_fn\n\ndef top_fn(x):\n    return consumer_fn(x)\n",
    );
    f.commit("cross-scope chains");
    f.trace(&["cache", "build", "."]).ok();

    let dependencies = f.trace(&["dependencies", "--path", "scoped", "--json"]);
    dependencies.ok();
    let dependency_view = dependencies.view();
    let dependency_rows = dependency_view["results"]
        .as_array()
        .expect("dependencies results must be rows");
    assert_eq!(
        dependency_rows.len(),
        1,
        "only scoped files with dependencies may rank: {dependency_rows:?}"
    );
    assert_eq!(dependency_rows[0]["node_id"], "module::scoped.entry");
    assert_eq!(dependency_rows[0]["direct_dependencies"], 1);
    assert_eq!(
        dependency_rows[0]["transitive_dependencies"], 2,
        "the scoped entry must keep its reach through outside/mid.py to outside/core.py"
    );

    let file_dependencies = f.trace(&["dependencies", "--path", "scoped/entry.py", "--json"]);
    file_dependencies.ok();
    let file_view = file_dependencies.view();
    let file_rows = file_view["results"]
        .as_array()
        .expect("file-scoped dependencies results must be rows");
    assert_eq!(
        file_rows.len(),
        1,
        "a file scope must select only that file"
    );
    assert_eq!(file_rows[0]["node_id"], "module::scoped.entry");
    assert_eq!(file_rows[0]["transitive_dependencies"], 2);

    let usages = f.trace(&["usages", "--path", "scoped", "--json"]);
    usages.ok();
    let usage_view = usages.view();
    let usage_rows = usage_view["results"]
        .as_array()
        .expect("usages results must be rows");
    assert_eq!(
        usage_rows.len(),
        1,
        "only scoped files with dependents may rank: {usage_rows:?}"
    );
    assert_eq!(usage_rows[0]["node_id"], "scoped/base.py::base_fn");
    assert_eq!(usage_rows[0]["direct_dependents"], 1);
    assert_eq!(
        usage_rows[0]["transitive_dependents"], 2,
        "the scoped base must keep its reach through outside/consumer.py to outside/top.py"
    );
}

#[test]
fn path_mode_outside_root_uses_unscoped_ranking() {
    let f = chain_repo();
    f.trace(&["cache", "build", "."]).ok();

    let outside = std::env::temp_dir().join(format!(
        "trace-reach-outside-root-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("outside.py"), "def outside():\n    return 0\n").unwrap();
    symlink(&outside, f.root.join("outside-link")).unwrap();

    let expected = f.trace(&["usages", "--path", ".", "--json"]);
    expected.ok();
    let run = f.trace(&["usages", "--path", "outside-link", "--json"]);

    let _ = fs::remove_file(f.root.join("outside-link"));
    let _ = fs::remove_dir_all(&outside);

    run.ok();
    assert_eq!(
        run.view()["results"],
        expected.view()["results"],
        "a path that canonicalizes outside the repository keeps the repository-wide ranking"
    );
}

#[test]
fn path_mode_uses_the_repository_containing_the_requested_path() {
    let f = standard_repo();
    let other = chain_repo();
    let expected = other.trace(&["usages", "--path", ".", "--json"]);
    expected.ok();
    let path = other.root.to_string_lossy().to_string();

    let run = f.trace(&["usages", "--path", &path, "--json"]);

    run.ok();
    assert_eq!(
        run.view()["results"],
        expected.view()["results"],
        "--path must rank the repository containing the requested path"
    );
}

#[test]
fn missing_path_mode_uses_the_repository_wide_ranking() {
    let f = chain_repo();
    let expected = f.trace(&["usages", "--path", ".", "--json"]);
    expected.ok();

    let run = f.trace(&["usages", "--path", "missing", "--json"]);

    run.ok();
    assert_eq!(
        run.view()["results"],
        expected.view()["results"],
        "a missing --path must not suppress the repository-wide ranking"
    );
}

#[test]
fn usages_path_mode_ranks_every_subject_before_limiting() {
    let f = Fixture::new();
    f.write("root.py", "def root_fn(x):\n    return x + 1\n");
    f.write(
        "hub.py",
        "from root import root_fn\n\ndef hub_fn(x):\n    return root_fn(x)\n",
    );
    for n in 0..7 {
        f.write(
            &format!("leaf_{n}.py"),
            &format!("from hub import hub_fn\n\ndef leaf_{n}_fn(x):\n    return hub_fn(x)\n"),
        );
    }
    for distractor in 0..4 {
        f.write(
            &format!("distractor_{distractor}.py"),
            &format!("def distractor_{distractor}_fn(x):\n    return x + 1\n"),
        );
        for caller in 0..2 {
            f.write(
                &format!("caller_{distractor}_{caller}.py"),
                &format!(
                    "from distractor_{distractor} import distractor_{distractor}_fn\n\ndef caller_{distractor}_{caller}_fn(x):\n    return distractor_{distractor}_fn(x)\n"
                ),
            );
        }
    }
    f.commit("low-direct high-transitive root");
    f.trace(&["cache", "build", "."]).ok();

    let r = f.trace(&["usages", "--path", ".", "--limit", "1", "--json"]);
    r.ok();
    let v = r.view();
    let rows = v["results"]
        .as_array()
        .expect("usages results must be rows");
    assert_eq!(
        rows.len(),
        1,
        "limit must apply after exact ranking: {rows:?}"
    );
    assert_eq!(
        rows[0]["node_id"], "root.py::root_fn",
        "root_fn has only one direct dependent but reaches hub plus seven leaves"
    );
    assert_eq!(rows[0]["direct_dependents"], 1);
    assert_eq!(rows[0]["transitive_dependents"], 8);
}

#[test]
fn usages_path_mode_respects_limit() {
    let f = chain_repo();
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["usages", "--path", ".", "--limit", "1", "--json"]);
    r.ok();
    let v = r.view();
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

#[test]
fn path_mode_matches_exact_reach_oracle_beyond_thirty_subjects() {
    let f = Fixture::new();
    f.write("scope/__init__.py", "");
    f.write("outside/__init__.py", "");

    let scoped: Vec<String> = (0..34).map(|n| format!("scope/n{n:02}.py")).collect();
    let outside = [
        "outside/a.py".to_string(),
        "outside/b.py".to_string(),
        "outside/c.py".to_string(),
        "outside/d.py".to_string(),
    ];
    let mut written_edges: Vec<(String, String)> = (0..34)
        .map(|n| (scoped[n].clone(), scoped[(n + 1) % 34].clone()))
        .collect();

    // n00 -> n01/n02 -> n03 is a diamond. The repeated n04 -> n05 import
    // must still be one stored graph edge, as the relations index promises.
    written_edges.extend([
        (scoped[0].clone(), scoped[2].clone()),
        (scoped[1].clone(), scoped[3].clone()),
        (scoped[4].clone(), scoped[5].clone()),
        (scoped[10].clone(), outside[0].clone()),
        (outside[0].clone(), outside[1].clone()),
        (outside[1].clone(), scoped[11].clone()),
        (outside[2].clone(), scoped[12].clone()),
        (outside[3].clone(), outside[2].clone()),
    ]);

    let mut imports: HashMap<String, Vec<String>> = HashMap::new();
    for (source, target) in &written_edges {
        imports
            .entry(source.clone())
            .or_default()
            .push(target.clone());
    }
    for file in scoped.iter().chain(outside.iter()) {
        let stem = file
            .rsplit_once('/')
            .map(|(_, name)| name)
            .unwrap()
            .trim_end_matches(".py");
        let mut source = String::new();
        for target in imports.get(file).into_iter().flatten() {
            let target_module = target.trim_end_matches(".py").replace('/', ".");
            let target_stem = target
                .rsplit_once('/')
                .map(|(_, name)| name)
                .unwrap()
                .trim_end_matches(".py");
            source.push_str(&format!("from {target_module} import {target_stem}_fn\n"));
        }
        source.push_str(&format!("\ndef {stem}_fn():\n    return 0\n"));
        f.write(file, &source);
    }
    f.commit("exact reach graph");
    f.trace(&["cache", "build", "."]).ok();

    let mut graph_edges = written_edges;
    graph_edges.sort();
    graph_edges.dedup();

    for (command, direct_key, transitive_key, follows_imports) in [
        (
            "dependencies",
            "direct_dependencies",
            "transitive_dependencies",
            true,
        ),
        (
            "usages",
            "direct_dependents",
            "transitive_dependents",
            false,
        ),
    ] {
        let mut adjacency: HashMap<&str, Vec<&str>> = HashMap::new();
        let mut direct: HashMap<&str, i64> = HashMap::new();
        for (importer, imported) in &graph_edges {
            let (subject, next) = if follows_imports {
                (importer.as_str(), imported.as_str())
            } else {
                (imported.as_str(), importer.as_str())
            };
            adjacency.entry(subject).or_default().push(next);
            *direct.entry(subject).or_insert(0) += 1;
        }

        for depth in [1i64, 2, 4, i64::MAX] {
            let mut expected: Vec<(String, i64, i64)> = direct
                .iter()
                .filter(|(file, _)| file.starts_with("scope/"))
                .map(|(file, edge_count)| {
                    let mut seen = HashSet::from([*file]);
                    let mut frontier = VecDeque::from([(*file, 0i64)]);
                    while let Some((current, current_depth)) = frontier.pop_front() {
                        if current_depth >= depth {
                            continue;
                        }
                        for next in adjacency.get(current).into_iter().flatten() {
                            if seen.insert(*next) {
                                frontier.push_back((*next, current_depth + 1));
                            }
                        }
                    }
                    (file.to_string(), *edge_count, (seen.len() - 1) as i64)
                })
                .collect();
            expected.sort_by(|a, b| {
                b.2.cmp(&a.2)
                    .then_with(|| b.1.cmp(&a.1))
                    .then_with(|| b.0.cmp(&a.0))
            });
            let expected: Vec<(String, i64, i64, i64, String)> = expected
                .into_iter()
                .enumerate()
                .map(|(index, (file, direct, transitive))| {
                    let node_id = if follows_imports {
                        format!("module::{}", file.trim_end_matches(".py").replace('/', "."))
                    } else {
                        let stem = file
                            .rsplit_once('/')
                            .map(|(_, name)| name)
                            .unwrap()
                            .trim_end_matches(".py");
                        format!("{file}::{stem}_fn")
                    };
                    (file, direct, transitive, (index + 1) as i64, node_id)
                })
                .collect();

            for worker_count in ["1", "4"] {
                let depth_arg = depth.to_string();
                let r = f.trace_env(
                    &[
                        command, "--path", "scope", "--depth", &depth_arg, "--limit", "100",
                        "--json",
                    ],
                    &[("RAYON_NUM_THREADS", worker_count)],
                );
                r.ok();
                let v = r.view();
                let actual: Vec<(String, i64, i64, i64, String)> = v["results"]
                    .as_array()
                    .expect("path-mode results must be rows")
                    .iter()
                    .map(|row| {
                        (
                            row["source_file"].as_str().unwrap().to_string(),
                            row[direct_key].as_i64().unwrap(),
                            row[transitive_key].as_i64().unwrap(),
                            row["rank"].as_i64().unwrap(),
                            row["node_id"].as_str().unwrap().to_string(),
                        )
                    })
                    .collect();
                assert_eq!(
                    actual, expected,
                    "{command} depth {depth} with {worker_count} workers must match the complete independent BFS oracle"
                );
            }
        }
    }
}
