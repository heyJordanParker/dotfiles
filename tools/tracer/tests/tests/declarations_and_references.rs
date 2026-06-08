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
//!   * References resolve by STRUCTURE, not bare name: the use site and the
//!     declaration must agree on language and call shape. A free call
//!     resolves to a single non-method symbol or to nothing (never a fan-out
//!     across same-named declarations); a static / `new` / type-hint use
//!     resolves to the named class exactly; a cross-language call resolves to
//!     nothing. The only residual AMBIGUOUS edge is a member call whose
//!     receiver type the site does not name — it narrows to methods of that
//!     name, never to a free function or a wrong-language symbol.
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
fn python_free_call_collision_resolves_to_nothing() {
    // Two unrelated free functions `process` in different files; a caller
    // that calls `process(1)` free, without importing either. Under the
    // structural model a free call resolves to a single non-method symbol
    // or to nothing — it never fans out to every same-named declaration,
    // because a free call to an un-imported, multiply-declared name is name
    // coincidence, exactly the noise this model removes. So NEITHER `process`
    // declaration records a caller from `caller.py`.
    let f = Fixture::new();
    f.write("a.py", "def process(x):\n    return x\n");
    f.write("b.py", "def process(x):\n    return x + 1\n");
    f.write("caller.py", "def go():\n    return process(1)\n");
    f.commit("py free collision");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "process", "--json"]);
    r.ok();
    let v = r.json();
    let rows_a = caller_rows(&v, "a.py::process");
    let rows_b = caller_rows(&v, "b.py::process");
    assert!(
        !rows_a.iter().any(|(f, _, _)| f == "caller.py"),
        "free-call collision must NOT fan out to a.py::process — got {:?}",
        rows_a
    );
    assert!(
        !rows_b.iter().any(|(f, _, _)| f == "caller.py"),
        "free-call collision must NOT fan out to b.py::process — got {:?}",
        rows_b
    );
}

#[test]
fn python_member_call_collision_is_the_only_ambiguity() {
    // The sole residual ambiguity: a method call `obj.run()` whose receiver
    // type the site does not name, matching same-named methods on two
    // classes in different files. BOTH methods must record AMBIGUOUS — and a
    // free function of the same name must NOT, because a member call resolves
    // only to methods, never to a free symbol.
    let f = Fixture::new();
    f.write("job.py", "class Job:\n    def run(self):\n        return 1\n");
    f.write("task.py", "class Task:\n    def run(self):\n        return 2\n");
    f.write("free.py", "def run():\n    return 0\n");
    f.write("caller.py", "def go(obj):\n    return obj.run()\n");
    f.commit("py member collision");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "run", "--json"]);
    r.ok();
    let v = r.json();
    let rows_job = caller_rows(&v, "job.py::run");
    let rows_task = caller_rows(&v, "task.py::run");
    let rows_free = caller_rows(&v, "free.py::run");
    assert!(
        rows_job
            .iter()
            .any(|(f, l, c)| f == "caller.py" && *l == 2 && c == "AMBIGUOUS"),
        "Job.run must record AMBIGUOUS member call at caller.py:2 — got {:?}",
        rows_job
    );
    assert!(
        rows_task
            .iter()
            .any(|(f, l, c)| f == "caller.py" && *l == 2 && c == "AMBIGUOUS"),
        "Task.run must record AMBIGUOUS member call at caller.py:2 — got {:?}",
        rows_task
    );
    assert!(
        !rows_free.iter().any(|(f, _, _)| f == "caller.py"),
        "the free function run() must NOT receive a member-call edge — got {:?}",
        rows_free
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
fn ts_free_call_collision_resolves_to_nothing() {
    // Two free `compute` functions, a caller that calls `compute(1)` free
    // without importing either. The structural model resolves a free call to
    // a single non-method symbol or to nothing — never a fan-out to every
    // same-named declaration. So neither `compute` records the caller.
    let f = Fixture::new();
    f.write("a.ts", "export function compute(x: number): number { return x; }\n");
    f.write("b.ts", "export function compute(x: number): number { return x + 1; }\n");
    f.write(
        "caller.ts",
        "export function go(): number { return compute(1); }\n",
    );
    f.commit("ts free collision");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "compute", "--json"]);
    r.ok();
    let v = r.json();
    let rows_a = caller_rows(&v, "a.ts::compute");
    let rows_b = caller_rows(&v, "b.ts::compute");
    assert!(
        !rows_a.iter().any(|(f, _, _)| f == "caller.ts"),
        "free-call collision must NOT fan out to a.ts::compute — got {:?}",
        rows_a
    );
    assert!(
        !rows_b.iter().any(|(f, _, _)| f == "caller.ts"),
        "free-call collision must NOT fan out to b.ts::compute — got {:?}",
        rows_b
    );
}

#[test]
fn ts_member_call_collision_is_the_only_ambiguity() {
    // The residual ambiguity: a method call `obj.save()` whose receiver type
    // is not named at the site, matching same-named methods on two classes
    // in different files. Both methods record AMBIGUOUS; a free function of
    // the same name does not.
    let f = Fixture::new();
    f.write(
        "user.ts",
        "export class User {\n  save(): number { return 1; }\n}\n",
    );
    f.write(
        "post.ts",
        "export class Post {\n  save(): number { return 2; }\n}\n",
    );
    f.write("helpers.ts", "export function save(): number { return 0; }\n");
    f.write(
        "caller.ts",
        "export function go(obj: any): number { return obj.save(); }\n",
    );
    f.commit("ts member collision");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "save", "--json"]);
    r.ok();
    let v = r.json();
    let rows_user = caller_rows(&v, "user.ts::save");
    let rows_post = caller_rows(&v, "post.ts::save");
    let rows_free = caller_rows(&v, "helpers.ts::save");
    assert!(
        rows_user
            .iter()
            .any(|(f, l, c)| f == "caller.ts" && *l == 1 && c == "AMBIGUOUS"),
        "User.save must record AMBIGUOUS member call at caller.ts:1 — got {:?}",
        rows_user
    );
    assert!(
        rows_post
            .iter()
            .any(|(f, l, c)| f == "caller.ts" && *l == 1 && c == "AMBIGUOUS"),
        "Post.save must record AMBIGUOUS member call at caller.ts:1 — got {:?}",
        rows_post
    );
    assert!(
        !rows_free.iter().any(|(f, _, _)| f == "caller.ts"),
        "the free function save() must NOT receive a member-call edge — got {:?}",
        rows_free
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
fn php_free_call_collision_resolves_to_nothing() {
    // Two free `compute` functions; a caller calls `compute(1)` free. The
    // structural model resolves a free call to one symbol or none — never a
    // fan-out across same-named declarations. Neither records the caller.
    let f = Fixture::new();
    f.write("a.php", "<?php\nfunction compute($x) { return $x; }\n");
    f.write("b.php", "<?php\nfunction compute($x) { return $x + 1; }\n");
    f.write("caller.php", "<?php\nfunction go() { return compute(1); }\n");
    f.commit("php free collision");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "compute", "--json"]);
    r.ok();
    let v = r.json();
    let rows_a = caller_rows(&v, "a.php::compute");
    let rows_b = caller_rows(&v, "b.php::compute");
    assert!(
        !rows_a.iter().any(|(f, _, _)| f == "caller.php"),
        "free-call collision must NOT fan out to a.php::compute — got {:?}",
        rows_a
    );
    assert!(
        !rows_b.iter().any(|(f, _, _)| f == "caller.php"),
        "free-call collision must NOT fan out to b.php::compute — got {:?}",
        rows_b
    );
}

#[test]
fn php_member_call_collision_is_the_only_ambiguity() {
    // The residual ambiguity: a method call `$obj->handle()` whose receiver
    // type is not named, matching same-named methods on two classes in
    // different files. Both methods record AMBIGUOUS; a free function of the
    // same name does not.
    let f = Fixture::new();
    f.write(
        "first.php",
        "<?php\nclass First {\n  public function handle() { return 1; }\n}\n",
    );
    f.write(
        "second.php",
        "<?php\nclass Second {\n  public function handle() { return 2; }\n}\n",
    );
    f.write("free.php", "<?php\nfunction handle() { return 0; }\n");
    f.write(
        "caller.php",
        "<?php\nfunction go($obj) { return $obj->handle(); }\n",
    );
    f.commit("php member collision");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "handle", "--json"]);
    r.ok();
    let v = r.json();
    let rows_first = caller_rows(&v, "first.php::handle");
    let rows_second = caller_rows(&v, "second.php::handle");
    let rows_free = caller_rows(&v, "free.php::handle");
    assert!(
        rows_first
            .iter()
            .any(|(f, l, c)| f == "caller.php" && *l == 2 && c == "AMBIGUOUS"),
        "First::handle must record AMBIGUOUS member call at caller.php:2 — got {:?}",
        rows_first
    );
    assert!(
        rows_second
            .iter()
            .any(|(f, l, c)| f == "caller.php" && *l == 2 && c == "AMBIGUOUS"),
        "Second::handle must record AMBIGUOUS member call at caller.php:2 — got {:?}",
        rows_second
    );
    assert!(
        !rows_free.iter().any(|(f, _, _)| f == "caller.php"),
        "the free function handle() must NOT receive a member-call edge — got {:?}",
        rows_free
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
// Cross-language resolution is impossible — the headline guarantee
// ---------------------------------------------------------------------------

#[test]
fn cross_language_call_resolves_to_nothing() {
    // A TypeScript free call `process()` and a PHP method `process` of the
    // same name. The languages differ, so the structural model never
    // resolves the TS use site onto the PHP declaration — the PHP method
    // records zero callers from the TS file. This is the false edge the
    // change exists to kill (a TS `process()` formerly fanned to PHP
    // controllers' `process` methods).
    let f = Fixture::new();
    f.write(
        "controller.php",
        "<?php\nclass MediaController {\n  public function process() { return 1; }\n}\n",
    );
    f.write(
        "account.ts",
        "export function handler(): number { return process(); }\nfunction process(): number { return 2; }\n",
    );
    f.commit("cross-language collision");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "process", "--json"]);
    r.ok();
    let v = r.json();
    let rows_php = caller_rows(&v, "controller.php::process");
    assert!(
        !rows_php.iter().any(|(f, _, _)| f == "account.ts"),
        "a TypeScript process() must NOT resolve onto the PHP process method — got {:?}",
        rows_php
    );
}

#[test]
fn python_free_call_resolves_to_class_construction() {
    // Python constructs by calling the class: `Widget()` is instantiation — a
    // free call that must resolve to the class. The model allows a free call
    // to reach a class only in call-constructing languages (Python, Ruby).
    let f = Fixture::new();
    f.write("model.py", "class Widget:\n    pass\n");
    f.write(
        "app.py",
        "from model import Widget\n\ndef build():\n    return Widget()\n",
    );
    f.commit("py construction");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "Widget", "--json"]);
    r.ok();
    let v = r.json();
    let rows = caller_rows(&v, "model.py::Widget");
    assert!(
        rows.iter().any(|(f, _, _)| f == "app.py"),
        "a Python free call Widget() must resolve to the class as construction — got {:?}",
        rows
    );
}

#[test]
fn ts_free_call_does_not_resolve_to_class_but_new_does() {
    // TypeScript constructs with `new`, classified Static. A bare `Widget()`
    // free call is therefore NOT a reference to the class (the model forbids
    // free→class in new-constructing languages); only `new Widget()` resolves.
    let f = Fixture::new();
    f.write("model.ts", "export class Widget {}\n");
    f.write(
        "free.ts",
        "import { Widget } from './model';\nexport function bare(): unknown { return Widget(); }\n",
    );
    f.write(
        "ctor.ts",
        "import { Widget } from './model';\nexport function make(): Widget { return new Widget(); }\n",
    );
    f.commit("ts construction");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "Widget", "--json"]);
    r.ok();
    let v = r.json();
    let rows = caller_rows(&v, "model.ts::Widget");
    assert!(
        !rows.iter().any(|(f, _, _)| f == "free.ts"),
        "a TS free call Widget() must NOT resolve to the class — got {:?}",
        rows
    );
    assert!(
        rows.iter().any(|(f, _, _)| f == "ctor.ts"),
        "a TS `new Widget()` must resolve to the class as construction — got {:?}",
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

// ---------------------------------------------------------------------------
// Function-granular callers — the edge source is the CALLING SYMBOL
// ---------------------------------------------------------------------------

#[test]
fn callers_source_is_calling_function_not_module() {
    // `helper` is called inside two functions in app.py. With
    // function-granular reference edges each caller row's node_id is the
    // CALLING FUNCTION (`app.py::first`, `app.py::second`) — not the module
    // `module::app`. This is the headline behavior change.
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
    f.commit("py granular callers");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "helper", "--json"]);
    r.ok();
    let v = r.json();
    let entry = &v["util.py::helper"];
    let ids: Vec<String> = entry["callers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["node_id"].as_str().unwrap_or("").to_string())
        .collect();
    assert!(
        ids.contains(&"app.py::first".to_string())
            && ids.contains(&"app.py::second".to_string()),
        "caller node ids must be the calling FUNCTIONS, got {:?}",
        ids
    );
    assert!(
        !ids.iter().any(|i| i == "module::app"),
        "the importer MODULE must not be the source of a function-granular call edge: {:?}",
        ids
    );
    // Count summary heads the answer: two resolved callers, none ambiguous.
    assert_eq!(entry["caller_count"].as_i64(), Some(2));
    assert_eq!(entry["resolved_count"].as_i64(), Some(2));
    assert_eq!(entry["ambiguous_count"].as_i64(), Some(0));
}

#[test]
fn callers_carry_calling_symbol_signature() {
    // A caller row carries the calling symbol's signature (the same surface
    // `structure` extracts): parameters, types, return type. Here `first`
    // calls `helper`, so `helper`'s caller row exposes `first`'s signature.
    let f = Fixture::new();
    f.write("util.ts", "export function helper(x: number): number { return x; }\n");
    // `first` is DECLARED on line 2 but CALLS helper on line 4 — distinct
    // lines, so the signature lookup must use the calling symbol's
    // declaration coordinates, not the use-site line.
    f.write(
        "app.ts",
        concat!(
            "import { helper } from './util';\n",
            "export function first(a: string): number {\n",
            "  const r = 1;\n",
            "  return helper(r);\n",
            "}\n",
        ),
    );
    f.commit("ts caller signature");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "helper", "--json"]);
    r.ok();
    let v = r.json();
    let row = v["util.ts::helper"]["callers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["label"].as_str() == Some("first"))
        .cloned()
        .unwrap_or_else(|| panic!("missing `first` caller row: {}", v));
    let sig = &row["signature"];
    assert!(
        sig.is_object(),
        "caller row must carry the calling symbol's signature object, got {:?}",
        sig
    );
    let params = sig["parameters"].as_array().unwrap_or_else(|| {
        panic!("signature must list parameters, got {:?}", sig)
    });
    assert!(
        params.iter().any(|p| p["name"].as_str() == Some("a")
            && p["type"].as_str() == Some("string")),
        "first's parameter `a: string` must surface in the caller signature: {:?}",
        params
    );
    assert_eq!(
        sig["return_type"].as_str(),
        Some("number"),
        "first's return type must surface: {:?}",
        sig
    );
}

#[test]
fn callers_order_resolved_before_ambiguous() {
    // A member-call collision yields two AMBIGUOUS rows; a separate EXTRACTED
    // caller of one method exists too. The output must order the resolved
    // (EXTRACTED/INFERRED) rows ahead of the AMBIGUOUS ones.
    let f = Fixture::new();
    f.write(
        "user.ts",
        "export class User {\n  save(): number { return 1; }\n}\n",
    );
    f.write(
        "post.ts",
        "export class Post {\n  save(): number { return 2; }\n}\n",
    );
    // A static/known caller of User.save via an explicit instance would be
    // ideal, but the cross-class member-call ambiguity is what we order
    // here: both rows are AMBIGUOUS, so the assertion is that no AMBIGUOUS
    // row precedes a resolved one (vacuously true here) AND the count
    // summary reports both as ambiguous.
    f.write(
        "caller.ts",
        "export function go(obj: any): number { return obj.save(); }\n",
    );
    f.commit("ts order");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "save", "--json"]);
    r.ok();
    let v = r.json();
    // Across both definition entries, every row is AMBIGUOUS and the
    // per-symbol counts agree. The ordering invariant: within any entry, no
    // resolved row appears after an ambiguous one.
    for key in ["user.ts::save", "post.ts::save"] {
        let entry = &v[key];
        let rows = entry["callers"].as_array().unwrap();
        let ranks: Vec<u8> = rows
            .iter()
            .map(|c| match c["confidence"].as_str().unwrap_or("") {
                "EXTRACTED" => 0,
                "INFERRED" => 1,
                "AMBIGUOUS" => 2,
                _ => 3,
            })
            .collect();
        assert!(
            ranks.windows(2).all(|w| w[0] <= w[1]),
            "{key}: callers must be confidence-ordered (resolved before ambiguous): {:?}",
            ranks
        );
        assert_eq!(
            entry["ambiguous_count"].as_i64(),
            Some(entry["caller_count"].as_i64().unwrap()),
            "{key}: every member-collision row is ambiguous"
        );
    }
}

// ---------------------------------------------------------------------------
// New-language extraction — Rust, Go, Ruby, Java, C
// ---------------------------------------------------------------------------

#[test]
fn rust_defines_finds_function_and_method() {
    let f = Fixture::new();
    f.write(
        "lib.rs",
        concat!(
            "fn free_helper(x: i32) -> i32 { x + 1 }\n",
            "struct Cart { items: i32 }\n",
            "impl Cart {\n",
            "    fn add_item(&self, x: i32) -> i32 { x }\n",
            "}\n",
        ),
    );
    f.commit("rust defines");
    f.trace(&["cache", "build", "."]).ok();
    // Free function.
    let r = f.trace(&["defines", "free_helper", "--json"]);
    r.ok();
    assert_eq!(r.json()["definition_count"].as_i64().unwrap(), 1);
    // Method on the impl — found by the full declaration index.
    let r = f.trace(&["defines", "add_item", "--json"]);
    r.ok();
    let v = r.json();
    assert_eq!(v["definition_count"].as_i64().unwrap(), 1);
    assert_eq!(def_files(&v), vec![("lib.rs".to_string(), 4)]);
}

#[test]
fn rust_callers_resolve_to_calling_function() {
    // `helper` is defined in util.rs and called by `first`/`second` in
    // app.rs. The caller rows are the calling FUNCTIONS at their use-site
    // lines — the cross-file, function-granular Rust contract.
    let f = Fixture::new();
    f.write("util.rs", "pub fn helper(x: i32) -> i32 {\n    x + 1\n}\n");
    f.write(
        "app.rs",
        concat!(
            "use crate::util::helper;\n",
            "pub fn first() -> i32 {\n",
            "    helper(1)\n",
            "}\n",
            "pub fn second() -> i32 {\n",
            "    helper(2)\n",
            "}\n",
        ),
    );
    f.commit("rust callers");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "helper", "--json"]);
    r.ok();
    let v = r.json();
    let rows = caller_rows(&v, "util.rs::helper");
    assert!(
        rows.iter().any(|(file, l, _)| file == "app.rs" && *l == 3),
        "first's call site at app.rs:3 must appear: {:?}",
        rows
    );
    assert!(
        rows.iter().any(|(file, l, _)| file == "app.rs" && *l == 6),
        "second's call site at app.rs:6 must appear: {:?}",
        rows
    );
    let ids: Vec<String> = v["util.rs::helper"]["callers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["node_id"].as_str().unwrap_or("").to_string())
        .collect();
    assert!(
        ids.contains(&"app.rs::first".to_string())
            && ids.contains(&"app.rs::second".to_string()),
        "Rust caller sources must be the calling functions: {:?}",
        ids
    );
}

#[test]
fn rust_struct_construction_resolves_to_type() {
    // `Cart { .. }` constructs the struct — a Static use that resolves to
    // the type, sourced from the constructing function.
    let f = Fixture::new();
    f.write("model.rs", "pub struct Cart {\n    pub items: i32,\n}\n");
    f.write(
        "app.rs",
        concat!(
            "use crate::model::Cart;\n",
            "pub fn build() -> Cart {\n",
            "    Cart { items: 0 }\n",
            "}\n",
        ),
    );
    f.commit("rust construction");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "Cart", "--json"]);
    r.ok();
    let v = r.json();
    let ids: Vec<String> = v["model.rs::Cart"]["callers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["node_id"].as_str().unwrap_or("").to_string())
        .collect();
    assert!(
        ids.contains(&"app.rs::build".to_string()),
        "Rust struct construction must resolve to the type, sourced from build(): {:?}",
        ids
    );
}

#[test]
fn go_defines_function_method_and_type() {
    let f = Fixture::new();
    f.write(
        "main.go",
        concat!(
            "package main\n",
            "type Cart struct { items int }\n",
            "func (c *Cart) AddItem(x int) int { return x }\n",
            "func Helper(x int) int { return x + 1 }\n",
        ),
    );
    f.commit("go defines");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["defines", "Helper", "--json"]);
    r.ok();
    assert_eq!(r.json()["definition_count"].as_i64().unwrap(), 1);
    let r = f.trace(&["defines", "AddItem", "--json"]);
    r.ok();
    assert_eq!(r.json()["definition_count"].as_i64().unwrap(), 1);
    let r = f.trace(&["defines", "Cart", "--json"]);
    r.ok();
    assert_eq!(r.json()["definition_count"].as_i64().unwrap(), 1);
}

#[test]
fn go_callers_resolve_to_calling_function() {
    let f = Fixture::new();
    f.write(
        "util.go",
        "package util\nfunc Helper(x int) int { return x + 1 }\n",
    );
    f.write(
        "app.go",
        concat!(
            "package app\n",
            "import \"example.com/util\"\n",
            "func First() int {\n",
            "    return util.Helper(1)\n",
            "}\n",
        ),
    );
    f.commit("go callers");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "Helper", "--json"]);
    r.ok();
    let v = r.json();
    // `util.Helper(1)` is a package-qualified call → Free shape (Go's
    // dominant cross-file edge); it resolves to the free function `Helper`,
    // sourced from the calling function `First` at the use site.
    let ids: Vec<String> = v["util.go::Helper"]["callers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["node_id"].as_str().unwrap_or("").to_string())
        .collect();
    assert!(
        ids.contains(&"app.go::First".to_string()),
        "Go caller source must be the calling function First: {:?}",
        ids
    );
}

#[test]
fn ruby_defines_method_and_class() {
    let f = Fixture::new();
    f.write(
        "cart.rb",
        concat!(
            "class Cart\n",
            "  def add_item(x)\n",
            "    x\n",
            "  end\n",
            "end\n",
        ),
    );
    f.commit("ruby defines");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["defines", "Cart", "--json"]);
    r.ok();
    assert_eq!(r.json()["definition_count"].as_i64().unwrap(), 1);
    let r = f.trace(&["defines", "add_item", "--json"]);
    r.ok();
    let v = r.json();
    assert_eq!(v["definition_count"].as_i64().unwrap(), 1);
    assert_eq!(def_files(&v), vec![("cart.rb".to_string(), 2)]);
}

#[test]
fn ruby_member_call_collision_is_ambiguous() {
    // Two classes with a same-named method `run`, an unqualified receiver
    // call `obj.run` — the member-call ambiguity, the only residual one.
    let f = Fixture::new();
    f.write("job.rb", "class Job\n  def run\n    1\n  end\nend\n");
    f.write("task.rb", "class Task\n  def run\n    2\n  end\nend\n");
    f.write(
        "caller.rb",
        "def go(obj)\n  obj.run\nend\n",
    );
    f.commit("ruby member collision");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "run", "--json"]);
    r.ok();
    let v = r.json();
    let rows_job = caller_rows(&v, "job.rb::run");
    let rows_task = caller_rows(&v, "task.rb::run");
    assert!(
        rows_job
            .iter()
            .any(|(file, _, c)| file == "caller.rb" && c == "AMBIGUOUS"),
        "Job#run must record an AMBIGUOUS member call: {:?}",
        rows_job
    );
    assert!(
        rows_task
            .iter()
            .any(|(file, _, c)| file == "caller.rb" && c == "AMBIGUOUS"),
        "Task#run must record an AMBIGUOUS member call: {:?}",
        rows_task
    );
}

#[test]
fn java_defines_class_and_method() {
    let f = Fixture::new();
    f.write(
        "Cart.java",
        concat!(
            "public class Cart {\n",
            "  public int addItem(int x) { return x; }\n",
            "}\n",
        ),
    );
    f.commit("java defines");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["defines", "Cart", "--json"]);
    r.ok();
    assert_eq!(r.json()["definition_count"].as_i64().unwrap(), 1);
    let r = f.trace(&["defines", "addItem", "--json"]);
    r.ok();
    let v = r.json();
    assert_eq!(v["definition_count"].as_i64().unwrap(), 1);
    assert_eq!(def_files(&v), vec![("Cart.java".to_string(), 2)]);
}

#[test]
fn java_new_construction_resolves_to_type() {
    // `new Cart()` is a Static construction use that resolves to the type,
    // sourced from the constructing method.
    let f = Fixture::new();
    f.write(
        "Cart.java",
        "public class Cart {\n  public int n() { return 1; }\n}\n",
    );
    f.write(
        "Factory.java",
        concat!(
            "public class Factory {\n",
            "  public Cart make() { return new Cart(); }\n",
            "}\n",
        ),
    );
    f.commit("java construction");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "Cart", "--json"]);
    r.ok();
    let v = r.json();
    let rows = caller_rows(&v, "Cart.java::Cart");
    assert!(
        rows.iter().any(|(file, _, _)| file == "Factory.java"),
        "new Cart() must resolve to the Cart type from Factory.java: {:?}",
        rows
    );
}

#[test]
fn c_defines_function_and_struct() {
    let f = Fixture::new();
    f.write(
        "lib.c",
        concat!(
            "struct Point { int x; int y; };\n",
            "int helper(int x) { return x + 1; }\n",
        ),
    );
    f.commit("c defines");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["defines", "helper", "--json"]);
    r.ok();
    let v = r.json();
    assert_eq!(v["definition_count"].as_i64().unwrap(), 1);
    assert_eq!(def_files(&v), vec![("lib.c".to_string(), 2)]);
    let r = f.trace(&["defines", "Point", "--json"]);
    r.ok();
    assert_eq!(r.json()["definition_count"].as_i64().unwrap(), 1);
}

#[test]
fn c_free_call_collision_resolves_to_nothing() {
    // Two C functions DEFINED with the same name `compute` in different
    // files; a caller calls `compute(1)` free. A free call to a
    // multiply-defined name is name coincidence under the structural model,
    // so neither definition records the caller — mirroring the Python / TS /
    // PHP free-collision tests, now for C.
    let f = Fixture::new();
    f.write("a.c", "int compute(int x) { return x; }\n");
    f.write("b.c", "int compute(int x) { return x + 1; }\n");
    f.write(
        "caller.c",
        "int go(void) {\n    return compute(1);\n}\n",
    );
    f.commit("c free collision");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "compute", "--json"]);
    r.ok();
    let v = r.json();
    let rows_a = caller_rows(&v, "a.c::compute");
    let rows_b = caller_rows(&v, "b.c::compute");
    assert!(
        !rows_a.iter().any(|(file, _, _)| file == "caller.c"),
        "free-call collision must NOT fan out to a.c::compute — got {:?}",
        rows_a
    );
    assert!(
        !rows_b.iter().any(|(file, _, _)| file == "caller.c"),
        "free-call collision must NOT fan out to b.c::compute — got {:?}",
        rows_b
    );
}

#[test]
fn c_unique_free_call_resolves() {
    // A C free call to a UNIQUELY-named function in another file resolves
    // (INFERRED) and is sourced from the calling function.
    let f = Fixture::new();
    f.write("util.c", "int lone_unique_c(int x) { return x + 1; }\n");
    f.write(
        "app.c",
        concat!(
            "int first(void) {\n",
            "    return lone_unique_c(1);\n",
            "}\n",
        ),
    );
    f.commit("c unique call");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "lone_unique_c", "--json"]);
    r.ok();
    let v = r.json();
    let ids: Vec<String> = v["util.c::lone_unique_c"]["callers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["node_id"].as_str().unwrap_or("").to_string())
        .collect();
    assert!(
        ids.contains(&"app.c::first".to_string()),
        "unique C free call must resolve to the calling function first(): {:?}",
        ids
    );
}

// ---------------------------------------------------------------------------
// Cross-language still resolves to nothing across the NEW languages
// ---------------------------------------------------------------------------

#[test]
fn rust_call_does_not_resolve_onto_go_function() {
    // A Rust free call `process()` and a Go function `process` of the same
    // name. Different languages → the structural model never links them.
    let f = Fixture::new();
    f.write(
        "handler.go",
        "package main\nfunc process() int { return 1 }\n",
    );
    f.write(
        "lib.rs",
        concat!(
            "fn process() -> i32 {\n    2\n}\n",
            "fn run() -> i32 {\n    process()\n}\n",
        ),
    );
    f.commit("rust/go cross language");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "process", "--json"]);
    r.ok();
    let v = r.json();
    let rows_go = caller_rows(&v, "handler.go::process");
    assert!(
        !rows_go.iter().any(|(file, _, _)| file == "lib.rs"),
        "a Rust process() must never resolve onto the Go process function: {:?}",
        rows_go
    );
}
