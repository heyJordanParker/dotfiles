//! Full declaration index + reference index.
//!
//! These tests pin the new capabilities added to the architecture graph:
//!
//!   * `defines` finds ANY declaration — not just exported / module-level
//!     ones. Methods on classes, private (non-exported) functions, and
//!     nested definitions all resolve.
//!
//!   * `callers` returns USE SITES — file:line rows for every reference
//!     to a resolvable symbol — not just the importer modules of its
//!     module. Confidence labels EXTRACTED / INFERRED / AMBIGUOUS apply
//!     to each reference row.
//!
//!   * Same-named symbols in different scopes are independently resolvable;
//!     ambiguous references surface as AMBIGUOUS on every candidate.
//!
//!   * Existing import-edge behaviour for module-level queries continues
//!     unchanged — module → module dependents are preserved exactly.
//!
//! Languages covered: Python, TypeScript, PHP (the three with extractors).

use tracer_cli_tests::Fixture;

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn caller_rows(v: &serde_json::Value, def_node_id: &str) -> Vec<(String, i64, String)> {
    v[def_node_id]["callers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            (
                c["source_file"].as_str().unwrap_or("").to_string(),
                c["source_line"].as_i64().unwrap_or(0),
                c["confidence"].as_str().unwrap_or("").to_string(),
            )
        })
        .collect()
}

fn def_files(v: &serde_json::Value) -> Vec<(String, i64)> {
    v["definitions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            (
                d["source_file"].as_str().unwrap_or("").to_string(),
                d["source_line"].as_i64().unwrap_or(0),
            )
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Python — every declaration kind, every confidence
// ---------------------------------------------------------------------------

#[test]
fn python_defines_finds_non_exported_top_level() {
    // `_private_helper` is a top-level function but conventionally private
    // (no decorator, leading underscore). The old export-only index missed
    // it entirely; the full declaration index must find it.
    let f = Fixture::new();
    f.write(
        "mod.py",
        "def _private_helper(x):\n    return x + 1\n\ndef public(x):\n    return _private_helper(x)\n",
    );
    f.commit("py private");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["defines", "_private_helper", "--json"]);
    r.ok();
    let v = r.json();
    assert_eq!(v["definition_count"].as_i64().unwrap(), 1);
    let defs = def_files(&v);
    assert_eq!(defs, vec![("mod.py".to_string(), 1)]);
}

#[test]
fn python_defines_finds_method_on_class() {
    let f = Fixture::new();
    f.write(
        "shop.py",
        "class Cart:\n    def add_item(self, x):\n        return x\n\n    def remove_item(self, x):\n        return x\n",
    );
    f.commit("py method");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["defines", "add_item", "--json"]);
    r.ok();
    let v = r.json();
    assert_eq!(v["definition_count"].as_i64().unwrap(), 1);
    let defs = def_files(&v);
    assert_eq!(defs, vec![("shop.py".to_string(), 2)]);
}

#[test]
fn python_defines_finds_nested_function() {
    let f = Fixture::new();
    f.write(
        "nest.py",
        "def outer():\n    def inner_helper():\n        return 1\n    return inner_helper()\n",
    );
    f.commit("py nested");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["defines", "inner_helper", "--json"]);
    r.ok();
    let v = r.json();
    assert_eq!(v["definition_count"].as_i64().unwrap(), 1);
    let defs = def_files(&v);
    assert_eq!(defs, vec![("nest.py".to_string(), 2)]);
}

#[test]
fn python_callers_returns_use_sites_not_just_modules() {
    // Two distinct call sites in the same caller file: each must appear
    // as its own row with the right line.
    let f = Fixture::new();
    f.write("util.py", "def helper(x):\n    return x\n");
    f.write(
        "app.py",
        concat!(
            "from util import helper\n",
            "\n",
            "def first():\n",
            "    return helper(1)\n",
            "\n",
            "def second():\n",
            "    return helper(2)\n",
        ),
    );
    f.commit("py refs");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "helper", "--json"]);
    r.ok();
    let v = r.json();
    let mut rows = caller_rows(&v, "util.py::helper");
    rows.sort();
    // Two call sites in app.py at lines 4 and 7; both EXTRACTED (the import
    // resolves cleanly).
    assert!(
        rows.contains(&("app.py".to_string(), 4, "EXTRACTED".to_string())),
        "missing first call site: {:?}",
        rows
    );
    assert!(
        rows.contains(&("app.py".to_string(), 7, "EXTRACTED".to_string())),
        "missing second call site: {:?}",
        rows
    );
}

#[test]
fn python_callers_marks_ambiguous_on_collision() {
    // Two unrelated `process` declarations in different files. A caller
    // that references `process` without disambiguating context must
    // surface AMBIGUOUS on EVERY candidate definition.
    let f = Fixture::new();
    f.write("a.py", "def process(x):\n    return x\n");
    f.write("b.py", "def process(x):\n    return x + 1\n");
    f.write(
        "caller.py",
        "def go():\n    return process(1)\n",
    );
    f.commit("py ambiguous");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "process", "--json"]);
    r.ok();
    let v = r.json();
    // Both candidates must report the same call site, marked AMBIGUOUS.
    let rows_a = caller_rows(&v, "a.py::process");
    let rows_b = caller_rows(&v, "b.py::process");
    assert!(
        rows_a
            .iter()
            .any(|(f, l, c)| f == "caller.py" && *l == 2 && c == "AMBIGUOUS"),
        "a.py::process must record AMBIGUOUS reference from caller.py:2 — got {:?}",
        rows_a
    );
    assert!(
        rows_b
            .iter()
            .any(|(f, l, c)| f == "caller.py" && *l == 2 && c == "AMBIGUOUS"),
        "b.py::process must record AMBIGUOUS reference from caller.py:2 — got {:?}",
        rows_b
    );
}

#[test]
fn python_inferred_reference_resolves_without_target_module() {
    // The caller does not import `lone_unique_name` from anywhere; the
    // symbol has a single uniquely-named declaration. The reference
    // resolves by name alone — INFERRED.
    let f = Fixture::new();
    f.write(
        "tgt.py",
        "def lone_unique_name():\n    return 1\n",
    );
    f.write(
        "caller.py",
        "def use():\n    return lone_unique_name()\n",
    );
    f.commit("py inferred");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "lone_unique_name", "--json"]);
    r.ok();
    let v = r.json();
    let rows = caller_rows(&v, "tgt.py::lone_unique_name");
    assert!(
        rows.iter()
            .any(|(f, l, c)| f == "caller.py" && *l == 2 && c == "INFERRED"),
        "uniquely-named ref without import must be INFERRED at caller.py:2 — got {:?}",
        rows
    );
}

// ---------------------------------------------------------------------------
// TypeScript — every declaration kind, every confidence
// ---------------------------------------------------------------------------

#[test]
fn ts_defines_finds_non_exported_top_level() {
    let f = Fixture::new();
    f.write(
        "lib.ts",
        "function privateOnly(): number { return 1; }\nexport function pub(): number { return privateOnly(); }\n",
    );
    f.commit("ts private");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["defines", "privateOnly", "--json"]);
    r.ok();
    let v = r.json();
    assert_eq!(v["definition_count"].as_i64().unwrap(), 1);
    let defs = def_files(&v);
    assert_eq!(defs, vec![("lib.ts".to_string(), 1)]);
}

#[test]
fn ts_defines_finds_method_on_class() {
    let f = Fixture::new();
    f.write(
        "cart.ts",
        "export class Cart {\n  addItem(x: number): number { return x; }\n  removeItem(x: number): number { return x; }\n}\n",
    );
    f.commit("ts method");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["defines", "addItem", "--json"]);
    r.ok();
    let v = r.json();
    assert_eq!(v["definition_count"].as_i64().unwrap(), 1);
    let defs = def_files(&v);
    assert_eq!(defs, vec![("cart.ts".to_string(), 2)]);
}

#[test]
fn ts_defines_finds_nested_function() {
    let f = Fixture::new();
    f.write(
        "nest.ts",
        "export function outer(): number {\n  function innerOnly(): number { return 1; }\n  return innerOnly();\n}\n",
    );
    f.commit("ts nested");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["defines", "innerOnly", "--json"]);
    r.ok();
    let v = r.json();
    assert_eq!(v["definition_count"].as_i64().unwrap(), 1);
    let defs = def_files(&v);
    assert_eq!(defs, vec![("nest.ts".to_string(), 2)]);
}

#[test]
fn ts_callers_returns_use_sites() {
    let f = Fixture::new();
    f.write("util.ts", "export function helper(x: number): number { return x; }\n");
    f.write(
        "app.ts",
        concat!(
            "import { helper } from './util';\n",
            "\n",
            "export function first(): number { return helper(1); }\n",
            "\n",
            "export function second(): number { return helper(2); }\n",
        ),
    );
    f.commit("ts refs");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "helper", "--json"]);
    r.ok();
    let v = r.json();
    let rows = caller_rows(&v, "util.ts::helper");
    assert!(
        rows.iter()
            .any(|(f, l, c)| f == "app.ts" && *l == 3 && c == "EXTRACTED"),
        "missing first ts call site: {:?}",
        rows
    );
    assert!(
        rows.iter()
            .any(|(f, l, c)| f == "app.ts" && *l == 5 && c == "EXTRACTED"),
        "missing second ts call site: {:?}",
        rows
    );
}

#[test]
fn ts_callers_marks_ambiguous_on_collision() {
    let f = Fixture::new();
    f.write("a.ts", "export function compute(x: number): number { return x; }\n");
    f.write("b.ts", "export function compute(x: number): number { return x + 1; }\n");
    f.write(
        "caller.ts",
        "export function go(): number { return compute(1); }\n",
    );
    f.commit("ts ambiguous");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "compute", "--json"]);
    r.ok();
    let v = r.json();
    let rows_a = caller_rows(&v, "a.ts::compute");
    let rows_b = caller_rows(&v, "b.ts::compute");
    assert!(
        rows_a
            .iter()
            .any(|(f, l, c)| f == "caller.ts" && *l == 1 && c == "AMBIGUOUS"),
        "a.ts::compute must record AMBIGUOUS at caller.ts:1 — got {:?}",
        rows_a
    );
    assert!(
        rows_b
            .iter()
            .any(|(f, l, c)| f == "caller.ts" && *l == 1 && c == "AMBIGUOUS"),
        "b.ts::compute must record AMBIGUOUS at caller.ts:1 — got {:?}",
        rows_b
    );
}

#[test]
fn ts_inferred_reference_resolves_without_target_module() {
    let f = Fixture::new();
    f.write(
        "tgt.ts",
        "export function loneUniqueTsName(): number { return 1; }\n",
    );
    f.write(
        "caller.ts",
        "export function use(): number { return loneUniqueTsName(); }\n",
    );
    f.commit("ts inferred");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "loneUniqueTsName", "--json"]);
    r.ok();
    let v = r.json();
    let rows = caller_rows(&v, "tgt.ts::loneUniqueTsName");
    assert!(
        rows.iter()
            .any(|(f, l, c)| f == "caller.ts" && *l == 1 && c == "INFERRED"),
        "ts INFERRED ref expected — got {:?}",
        rows
    );
}

// ---------------------------------------------------------------------------
// PHP — every declaration kind, every confidence
// ---------------------------------------------------------------------------

#[test]
fn php_defines_finds_non_exported_function() {
    // PHP doesn't have a formal "export" — every top-level function is
    // public. Verify the declaration index still finds a function not
    // tied to any class.
    let f = Fixture::new();
    f.write(
        "lib.php",
        "<?php\nfunction internalHelper($x) { return $x; }\nfunction publicEntry($x) { return internalHelper($x); }\n",
    );
    f.commit("php private");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["defines", "internalHelper", "--json"]);
    r.ok();
    let v = r.json();
    assert_eq!(v["definition_count"].as_i64().unwrap(), 1);
    let defs = def_files(&v);
    assert_eq!(defs, vec![("lib.php".to_string(), 2)]);
}

#[test]
fn php_defines_finds_method_on_class() {
    let f = Fixture::new();
    f.write(
        "cart.php",
        "<?php\nclass Cart {\n  public function addItem($x) { return $x; }\n  public function removeItem($x) { return $x; }\n}\n",
    );
    f.commit("php method");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["defines", "addItem", "--json"]);
    r.ok();
    let v = r.json();
    assert_eq!(v["definition_count"].as_i64().unwrap(), 1);
    let defs = def_files(&v);
    assert_eq!(defs, vec![("cart.php".to_string(), 3)]);
}

#[test]
fn php_defines_finds_nested_function() {
    // PHP: a function declared inside another function. The grammar allows
    // it; the declaration index must surface the inner function.
    let f = Fixture::new();
    f.write(
        "nest.php",
        "<?php\nfunction outer() {\n  function innerOnly() { return 1; }\n  return innerOnly();\n}\n",
    );
    f.commit("php nested");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["defines", "innerOnly", "--json"]);
    r.ok();
    let v = r.json();
    assert_eq!(v["definition_count"].as_i64().unwrap(), 1);
    let defs = def_files(&v);
    assert_eq!(defs, vec![("nest.php".to_string(), 3)]);
}

#[test]
fn php_callers_returns_use_sites() {
    let f = Fixture::new();
    f.write(
        "util.php",
        "<?php\nfunction helper($x) { return $x; }\n",
    );
    f.write(
        "app.php",
        concat!(
            "<?php\n",
            "function first() { return helper(1); }\n",
            "function second() { return helper(2); }\n",
        ),
    );
    f.commit("php refs");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "helper", "--json"]);
    r.ok();
    let v = r.json();
    let rows = caller_rows(&v, "util.php::helper");
    // Two call sites at lines 2 and 3.
    assert!(
        rows.iter().any(|(f, l, _)| f == "app.php" && *l == 2),
        "missing first php call site: {:?}",
        rows
    );
    assert!(
        rows.iter().any(|(f, l, _)| f == "app.php" && *l == 3),
        "missing second php call site: {:?}",
        rows
    );
}

#[test]
fn php_callers_marks_ambiguous_on_collision() {
    let f = Fixture::new();
    f.write(
        "a.php",
        "<?php\nfunction compute($x) { return $x; }\n",
    );
    f.write(
        "b.php",
        "<?php\nfunction compute($x) { return $x + 1; }\n",
    );
    f.write(
        "caller.php",
        "<?php\nfunction go() { return compute(1); }\n",
    );
    f.commit("php ambiguous");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "compute", "--json"]);
    r.ok();
    let v = r.json();
    let rows_a = caller_rows(&v, "a.php::compute");
    let rows_b = caller_rows(&v, "b.php::compute");
    assert!(
        rows_a
            .iter()
            .any(|(f, l, c)| f == "caller.php" && *l == 2 && c == "AMBIGUOUS"),
        "a.php::compute must record AMBIGUOUS at caller.php:2 — got {:?}",
        rows_a
    );
    assert!(
        rows_b
            .iter()
            .any(|(f, l, c)| f == "caller.php" && *l == 2 && c == "AMBIGUOUS"),
        "b.php::compute must record AMBIGUOUS at caller.php:2 — got {:?}",
        rows_b
    );
}

#[test]
fn php_inferred_reference_resolves_without_target_module() {
    let f = Fixture::new();
    f.write(
        "tgt.php",
        "<?php\nfunction lonePhpUniqueName() { return 1; }\n",
    );
    f.write(
        "caller.php",
        "<?php\nfunction use_it() { return lonePhpUniqueName(); }\n",
    );
    f.commit("php inferred");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "lonePhpUniqueName", "--json"]);
    r.ok();
    let v = r.json();
    let rows = caller_rows(&v, "tgt.php::lonePhpUniqueName");
    assert!(
        rows.iter()
            .any(|(f, l, c)| f == "caller.php" && *l == 2 && c == "INFERRED"),
        "php INFERRED ref expected — got {:?}",
        rows
    );
}

// ---------------------------------------------------------------------------
// Import-edge regression — existing behavior must remain unchanged
// ---------------------------------------------------------------------------

#[test]
fn module_level_import_dependents_unchanged_by_reference_index() {
    // The four-module chain a → b → c → d. The pre-existing module-level
    // import graph is the contract this test pins: regardless of the new
    // reference edges, downstream at depth 3 from `pkg.d` must still
    // climb exactly to pkg.c, pkg.b, pkg.a — no more, no fewer.
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
    f.commit("chain repo for regression");
    f.trace(&["cache", "build", "."]).ok();

    // The path-mode centrality ranking is the most sensitive view of the
    // import graph; pinning it exactly catches any silent edge inflation
    // from the new reference index.
    let r = f.trace(&["downstream", "--path", ".", "--json"]);
    r.ok();
    let v = r.json();
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
        "module-level import centrality must be unchanged by reference index: {:?}",
        triples
    );
}
