//! Exact per-language cyclomatic-complexity and extraction coverage.
//!
//! Every assertion here is an exact equality against a hand-computed
//! McCabe value, not a lower bound. The convention pinned: CCN starts at
//! 1; +1 per branch point (if/elif/else-if, each loop, each case/when
//! arm, each catch/except, ternary, and each short-circuit boolean
//! operator); `else`/`default` are not counted; nested functions are
//! their own scope and do not inflate the enclosing function.
//!
//! Each language fixture mixes a guarded branch with a short-circuit
//! operator, a loop, a second guarded branch with a short-circuit
//! operator, and a nested function — so a wrong decision-node set, a
//! missed short-circuit count, or a broken nested-function boundary
//! changes the number and fails the test.
//!
//! Hand computation for the shared `outer` shape (all languages except
//! Ruby and C, which differ structurally and are computed inline):
//!   base 1
//!   + `if A && B`            → if(1) + short-circuit(1) = 2
//!   + loop over items        → 1
//!   + `if X || Y`            → if(1) + short-circuit(1) = 2
//!   = 6   (the nested function's own branches are NOT counted here)
//! nested `inner`: base 1 + `if N` (1) = 2
//! file total = 6 + 2 = 8, max = 6, function_count = 2

use serde_json::Value;
use tracer_cli_tests::Fixture;

/// Drive `trace info <file> --json` and return the parsed document.
fn info(f: &Fixture, rel: &str) -> Value {
    let r = f.trace(&["info", rel, "--json"]);
    r.ok();
    r.json()
}

/// Assert the exact file-level complexity scalars.
fn assert_totals(v: &Value, total: i64, max: i64, count: i64) {
    assert_eq!(
        v["cyclomatic_complexity_total"].as_i64().unwrap(),
        total,
        "cyclomatic_complexity_total mismatch: {v:#}"
    );
    assert_eq!(
        v["cyclomatic_complexity_max"].as_i64().unwrap(),
        max,
        "cyclomatic_complexity_max mismatch: {v:#}"
    );
    assert_eq!(
        v["function_count"].as_i64().unwrap(),
        count,
        "function_count mismatch: {v:#}"
    );
}

/// Assert one function's exact CCN by name.
fn assert_fn(v: &Value, name: &str, ccn: i64) {
    let funcs = v["functions"].as_array().unwrap();
    let found = funcs
        .iter()
        .find(|f| f["name"].as_str() == Some(name))
        .unwrap_or_else(|| panic!("function `{name}` not in {v:#}"));
    assert_eq!(
        found["cyclomatic_complexity"].as_i64().unwrap(),
        ccn,
        "function `{name}` CCN mismatch: {found:#}"
    );
}

#[test]
fn python_exact_complexity() {
    let f = Fixture::new();
    f.write(
        "m.py",
        concat!(
            "def outer(a, b, items):\n",
            "    if a and b:\n",
            "        return 1\n",
            "    for x in items:\n",
            "        if x > 0 or x < -10:\n",
            "            print(x)\n",
            "    def inner(n):\n",
            "        if n:\n",
            "            return n\n",
            "        return 0\n",
            "    return inner(a)\n",
        ),
    );
    f.commit("py");
    let v = info(&f, "m.py");
    assert_fn(&v, "outer", 6);
    assert_fn(&v, "outer.inner", 2);
    assert_totals(&v, 8, 6, 2);
}

#[test]
fn typescript_exact_complexity() {
    let f = Fixture::new();
    f.write(
        "m.ts",
        concat!(
            "function outer(a: number, b: number, items: number[]): number {\n",
            "  if (a && b) {\n",
            "    return 1;\n",
            "  }\n",
            "  for (const x of items) {\n",
            "    if (x > 0 || x < -10) {\n",
            "      console.log(x);\n",
            "    }\n",
            "  }\n",
            "  const inner = (n: number): number => {\n",
            "    if (n) {\n",
            "      return n;\n",
            "    }\n",
            "    return 0;\n",
            "  };\n",
            "  return inner(a);\n",
            "}\n",
        ),
    );
    f.commit("ts");
    let v = info(&f, "m.ts");
    assert_fn(&v, "outer", 6);
    assert_fn(&v, "outer.inner", 2);
    assert_totals(&v, 8, 6, 2);
}

#[test]
fn php_exact_complexity() {
    let f = Fixture::new();
    f.write(
        "m.php",
        concat!(
            "<?php\n",
            "function outer($a, $b, $items) {\n",
            "  if ($a && $b) {\n",
            "    return 1;\n",
            "  }\n",
            "  foreach ($items as $x) {\n",
            "    if ($x > 0 || $x < -10) {\n",
            "      echo $x;\n",
            "    }\n",
            "  }\n",
            "  $inner = function($n) {\n",
            "    if ($n) {\n",
            "      return $n;\n",
            "    }\n",
            "    return 0;\n",
            "  };\n",
            "  return $inner($a);\n",
            "}\n",
        ),
    );
    f.commit("php");
    let v = info(&f, "m.php");
    assert_fn(&v, "outer", 6);
    // PHP names the closure `outer.<anonymous>`.
    assert_fn(&v, "outer.<anonymous>", 2);
    assert_totals(&v, 8, 6, 2);
}

#[test]
fn bash_exact_complexity() {
    // Regression guard: in bash `[ … ] && [ … ]` is a `list` node, not a
    // `binary_expression`, so a parent-kind-gated short-circuit scan
    // misses both `&&`/`||`. `outer` must be 6, not 4.
    let f = Fixture::new();
    f.write(
        "m.sh",
        concat!(
            "#!/bin/bash\n",
            "outer() {\n",
            "  local a=$1\n",
            "  local b=$2\n",
            "  if [ \"$a\" -gt 0 ] && [ \"$b\" -gt 0 ]; then\n",
            "    echo one\n",
            "  fi\n",
            "  for x in $3; do\n",
            "    if [ \"$x\" -gt 0 ] || [ \"$x\" -lt -10 ]; then\n",
            "      echo \"$x\"\n",
            "    fi\n",
            "  done\n",
            "  inner() {\n",
            "    if [ \"$1\" -gt 0 ]; then\n",
            "      return 1\n",
            "    fi\n",
            "    return 0\n",
            "  }\n",
            "  inner \"$a\"\n",
            "}\n",
        ),
    );
    f.commit("bash");
    let v = info(&f, "m.sh");
    assert_fn(&v, "outer", 6);
    assert_fn(&v, "outer.inner", 2);
    assert_totals(&v, 8, 6, 2);
}

#[test]
fn lua_exact_complexity() {
    let f = Fixture::new();
    f.write(
        "m.lua",
        concat!(
            "local function outer(a, b, items)\n",
            "  if a and b then\n",
            "    return 1\n",
            "  end\n",
            "  for _, x in ipairs(items) do\n",
            "    if x > 0 or x < -10 then\n",
            "      print(x)\n",
            "    end\n",
            "  end\n",
            "  local function inner(n)\n",
            "    if n then\n",
            "      return n\n",
            "    end\n",
            "    return 0\n",
            "  end\n",
            "  return inner(a)\n",
            "end\n",
            "return outer\n",
        ),
    );
    f.commit("lua");
    let v = info(&f, "m.lua");
    assert_fn(&v, "outer", 6);
    assert_fn(&v, "outer.inner", 2);
    assert_totals(&v, 8, 6, 2);
}

#[test]
fn go_exact_complexity() {
    let f = Fixture::new();
    f.write(
        "m.go",
        concat!(
            "package main\n",
            "\n",
            "func outer(a int, b int, items []int) int {\n",
            "\tif a > 0 && b > 0 {\n",
            "\t\treturn 1\n",
            "\t}\n",
            "\tfor _, x := range items {\n",
            "\t\tif x > 0 || x < -10 {\n",
            "\t\t\tprintln(x)\n",
            "\t\t}\n",
            "\t}\n",
            "\tinner := func(n int) int {\n",
            "\t\tif n > 0 {\n",
            "\t\t\treturn n\n",
            "\t\t}\n",
            "\t\treturn 0\n",
            "\t}\n",
            "\treturn inner(a)\n",
            "}\n",
        ),
    );
    f.commit("go");
    let v = info(&f, "m.go");
    assert_fn(&v, "outer", 6);
    // Go names the func literal `outer.<func>`.
    assert_fn(&v, "outer.<func>", 2);
    assert_totals(&v, 8, 6, 2);
}

#[test]
fn rust_exact_complexity() {
    let f = Fixture::new();
    f.write(
        "m.rs",
        concat!(
            "fn outer(a: i64, b: i64, items: &[i64]) -> i64 {\n",
            "    if a > 0 && b > 0 {\n",
            "        return 1;\n",
            "    }\n",
            "    for x in items {\n",
            "        if *x > 0 || *x < -10 {\n",
            "            println!(\"{}\", x);\n",
            "        }\n",
            "    }\n",
            "    let inner = |n: i64| -> i64 {\n",
            "        if n > 0 {\n",
            "            return n;\n",
            "        }\n",
            "        0\n",
            "    };\n",
            "    inner(a)\n",
            "}\n",
        ),
    );
    f.commit("rust");
    let v = info(&f, "m.rs");
    assert_fn(&v, "outer", 6);
    // Rust names the closure `outer.<closure>`.
    assert_fn(&v, "outer.<closure>", 2);
    assert_totals(&v, 8, 6, 2);
}

#[test]
fn ruby_exact_complexity() {
    // Regression guard: tree-sitter-ruby emits the `if`/`for` keyword
    // token with the SAME kind string as the enclosing statement node;
    // counting both double-counts. The `do` blocks are their own
    // function scope (function_kinds), so their branches are NOT counted
    // into `outer`.
    //   outer  : base 1 + if(1) + `&&`(1)                = 3
    //   each-do: base 1 + if(1) + `||`(1)                = 3
    //   lambda : base 1 + if(1)                          = 2
    //   total = 8, max = 3, function_count = 3
    let f = Fixture::new();
    f.write(
        "m.rb",
        concat!(
            "def outer(a, b, items)\n",
            "  if a && b\n",
            "    return 1\n",
            "  end\n",
            "  items.each do |x|\n",
            "    if x > 0 || x < -10\n",
            "      puts x\n",
            "    end\n",
            "  end\n",
            "  inner = lambda do |n|\n",
            "    if n\n",
            "      return n\n",
            "    end\n",
            "    0\n",
            "  end\n",
            "  inner.call(a)\n",
            "end\n",
        ),
    );
    f.commit("ruby");
    let v = info(&f, "m.rb");
    assert_fn(&v, "outer", 3);
    assert_totals(&v, 8, 3, 3);
    // Both `do` blocks are named `outer.<block>`; assert the multiset of
    // block CCNs is exactly {2, 3}.
    let mut blocks: Vec<i64> = v["functions"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|x| x["name"].as_str() == Some("outer.<block>"))
        .map(|x| x["cyclomatic_complexity"].as_i64().unwrap())
        .collect();
    blocks.sort();
    assert_eq!(blocks, vec![2, 3], "ruby block CCNs: {v:#}");
}

#[test]
fn java_exact_complexity() {
    let f = Fixture::new();
    f.write(
        "M.java",
        concat!(
            "class M {\n",
            "  static int outer(int a, int b, int[] items) {\n",
            "    if (a > 0 && b > 0) {\n",
            "      return 1;\n",
            "    }\n",
            "    for (int x : items) {\n",
            "      if (x > 0 || x < -10) {\n",
            "        System.out.println(x);\n",
            "      }\n",
            "    }\n",
            "    java.util.function.IntUnaryOperator inner = (int n) -> {\n",
            "      if (n > 0) {\n",
            "        return n;\n",
            "      }\n",
            "      return 0;\n",
            "    };\n",
            "    return inner.applyAsInt(a);\n",
            "  }\n",
            "}\n",
        ),
    );
    f.commit("java");
    let v = info(&f, "M.java");
    // Java qualifies the method by its class and names the lambda.
    assert_fn(&v, "M::outer", 6);
    assert_fn(&v, "M::outer.<lambda>", 2);
    assert_totals(&v, 8, 6, 2);
}

#[test]
fn c_exact_complexity() {
    // C has no nested functions; the boundary test is that the
    // top-level `helper`'s `if` is NOT counted into `outer`.
    //   outer : base 1 + if(1) + `&&`(1) + for(1) + if(1) + `||`(1) = 6
    //   helper: base 1 + if(1)                                      = 2
    //   total = 8, max = 6, function_count = 2
    let f = Fixture::new();
    f.write(
        "m.c",
        concat!(
            "#include <stdio.h>\n",
            "\n",
            "static int helper(int n) {\n",
            "    if (n > 0) {\n",
            "        return n;\n",
            "    }\n",
            "    return 0;\n",
            "}\n",
            "\n",
            "int outer(int a, int b, int *items, int len) {\n",
            "    if (a > 0 && b > 0) {\n",
            "        return 1;\n",
            "    }\n",
            "    for (int i = 0; i < len; i++) {\n",
            "        if (items[i] > 0 || items[i] < -10) {\n",
            "            printf(\"%d\", items[i]);\n",
            "        }\n",
            "    }\n",
            "    return helper(a);\n",
            "}\n",
        ),
    );
    f.commit("c");
    let v = info(&f, "m.c");
    // tree-sitter-c exposes function names under a declarator, not a
    // `name` field, so the walker labels both `<anonymous>`; the CCN
    // values are exact and that is what this test pins. Assert the
    // multiset of per-function CCNs is exactly {2, 6}.
    let mut ccns: Vec<i64> = v["functions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["cyclomatic_complexity"].as_i64().unwrap())
        .collect();
    ccns.sort();
    assert_eq!(ccns, vec![2, 6], "c per-function CCNs: {v:#}");
    assert_totals(&v, 8, 6, 2);
}

// --- Exact import / export extraction -----------------------------------

/// Drive `trace structure <file> --json`.
fn structure(f: &Fixture, rel: &str) -> Value {
    let r = f.trace(&["structure", rel, "--json"]);
    r.ok();
    r.json()
}

/// `(module, symbol-or-empty, line)` triples for every import, in order.
fn import_triples(v: &Value) -> Vec<(String, String, i64)> {
    v["imports"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| {
            (
                i["module"].as_str().unwrap_or("").to_string(),
                i["symbol"].as_str().unwrap_or("").to_string(),
                i["line"].as_i64().unwrap(),
            )
        })
        .collect()
}

/// `(name, kind, line)` triples for every export, in order.
fn export_triples(v: &Value) -> Vec<(String, String, i64)> {
    v["exports"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            (
                e["name"].as_str().unwrap().to_string(),
                e["kind"].as_str().unwrap().to_string(),
                e["line"].as_i64().unwrap(),
            )
        })
        .collect()
}

#[test]
fn python_imports_exact_set() {
    let f = Fixture::new();
    f.write(
        "m.py",
        concat!(
            "import os\n",
            "import sys as system\n",
            "from collections import OrderedDict\n",
            "from typing import List, Optional\n",
            "\n",
            "\n",
            "def alpha(x):\n",
            "    return x\n",
            "\n",
            "\n",
            "class Beta:\n",
            "    pass\n",
        ),
    );
    f.commit("py imports");
    let v = structure(&f, "m.py");
    assert_eq!(
        import_triples(&v),
        vec![
            ("os".into(), "".into(), 1),
            ("sys".into(), "".into(), 2),
            ("collections".into(), "OrderedDict".into(), 3),
            ("typing".into(), "List".into(), 4),
            ("typing".into(), "Optional".into(), 4),
        ],
        "python import set: {v:#}"
    );
    assert_eq!(
        export_triples(&v),
        vec![
            ("alpha".into(), "function".into(), 7),
            ("Beta".into(), "class".into(), 11),
        ],
        "python export set: {v:#}"
    );
}

#[test]
fn typescript_imports_exact_set() {
    // Regression guard: `import { a, b } from 'm'` puts the module string
    // AFTER the specifiers; a byte-distance heuristic resolves the
    // symbols to `unknown`. They must resolve to `fs`.
    let f = Fixture::new();
    f.write(
        "m.ts",
        concat!(
            "import { readFile, writeFile } from 'fs';\n",
            "import path from 'path';\n",
            "\n",
            "export const RATE = 7;\n",
            "export function compute(a: number): number {\n",
            "  return a * RATE;\n",
            "}\n",
            "export class Engine {}\n",
            "export interface Spec { id: number; }\n",
            "export type Id = string;\n",
        ),
    );
    f.commit("ts imports");
    let v = structure(&f, "m.ts");
    assert_eq!(
        import_triples(&v),
        vec![
            ("fs".into(), "readFile".into(), 1),
            ("fs".into(), "writeFile".into(), 1),
            ("fs".into(), "".into(), 1),
            ("path".into(), "".into(), 2),
        ],
        "typescript import set: {v:#}"
    );
    assert_eq!(
        export_triples(&v),
        vec![
            ("RATE".into(), "constant".into(), 4),
            ("compute".into(), "function".into(), 5),
            ("Engine".into(), "class".into(), 8),
            ("Spec".into(), "interface".into(), 9),
            ("Id".into(), "type".into(), 10),
        ],
        "typescript export set: {v:#}"
    );
}

#[test]
fn php_imports_exact_set() {
    let f = Fixture::new();
    f.write(
        "m.php",
        concat!(
            "<?php\n",
            "use App\\Models\\User;\n",
            "use App\\Services\\Billing;\n",
            "\n",
            "interface Payable {}\n",
            "\n",
            "class Invoice implements Payable {\n",
            "}\n",
            "\n",
            "function total($x) {\n",
            "  return $x;\n",
            "}\n",
        ),
    );
    f.commit("php imports");
    let v = structure(&f, "m.php");
    assert_eq!(
        import_triples(&v),
        vec![
            ("App\\Models".into(), "User".into(), 2),
            ("App\\Services".into(), "Billing".into(), 3),
        ],
        "php import set: {v:#}"
    );
    assert_eq!(
        export_triples(&v),
        vec![
            ("Payable".into(), "interface".into(), 5),
            ("Invoice".into(), "class".into(), 7),
            ("total".into(), "function".into(), 10),
        ],
        "php export set: {v:#}"
    );
}
