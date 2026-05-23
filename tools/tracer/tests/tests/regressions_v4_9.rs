//! Regression coverage for the six contradictions surfaced by end-to-end
//! probes of the v4.8 release. Each test reproduces one contradiction
//! against a real-CLI fixture and pins the corrected behaviour.

use std::fs;
use tracer_cli_tests::Fixture;

// ---------------------------------------------------------------------------
// Contradiction 1 — stale architecture cache after a schema-shape change
// ---------------------------------------------------------------------------
//
// Simulates "binary upgrade against a repo with a pre-existing
// `.tracer-cache/`": we plant cache entries whose per-file hashes use an
// older SCHEMA_VERSION token, plus a matching mtime index. The new binary
// must NOT honour those stale per-file hashes (which would otherwise
// produce a stale architecture fingerprint hit) — the first query has to
// rebuild from current schema and serve the new graph.

#[test]
fn architecture_cache_rebuilds_after_schema_shape_change() {
    let f = Fixture::new();
    f.write(
        "pkg/a.py",
        "from pkg.b import b_fn\n\ndef a_fn(x):\n    return b_fn(x)\n",
    );
    f.write("pkg/b.py", "def b_fn(x):\n    return x + 1\n");
    f.write("pkg/__init__.py", "");
    f.commit("init");

    // Plant a stale on-disk cache shaped like an older schema. The mtime
    // index keys per-file entries by mtime+size; the new binary's mtime
    // index key must rotate with the schema so this stale index is
    // unreachable on upgrade.
    let cache_root = f.root.join(".tracer-cache");
    let file_ns = cache_root.join("file");
    let arch_ns = cache_root.join("architecture");
    fs::create_dir_all(&file_ns).unwrap();
    fs::create_dir_all(&arch_ns).unwrap();

    // Stale per-file entry — body shaped like a real FileFacts JSON but
    // missing `references`, mimicking an older extraction shape.
    let stale_file_key = "stalehash000000000000000000000000000000000000000000000000000000";
    let stale_file_body = serde_json::json!({
        "path": "pkg/a.py",
        "language": "python",
        "cyclomatic_complexity_total": 0,
        "function_count": 0,
        "max_function_cyclomatic_complexity": 0,
        "loc": 4,
        "extraction": {
            "language": "python",
            "imports": [],
            "exports": [],
            "declarations": [],
            // No `references` key — the older schema didn't have one.
        }
    });
    fs::write(
        file_ns.join(format!("{stale_file_key}.json")),
        serde_json::to_string(&stale_file_body).unwrap(),
    )
    .unwrap();

    // Stale mtime index — under the OLD schema's key name. The bug it
    // pins: any mtime-index key shape that omits SCHEMA_VERSION returns
    // stale per-file hashes after a schema bump, which keep the
    // architecture fingerprint stable and serve the stale graph.
    let md = fs::metadata(f.root.join("pkg/a.py")).unwrap();
    use std::time::UNIX_EPOCH;
    let mtime_ns = md
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_nanos() as i64)
        .unwrap_or(0);
    let size = md.len() as i64;
    let stale_idx = serde_json::json!({
        "pkg/a.py": {
            "mtime_ns": mtime_ns,
            "size": size,
            "key": stale_file_key,
        },
    });
    // Old key shape: `mtime_index_v1__{backend}` — what the v4.8 binary
    // wrote. The new binary must not read from that location.
    fs::write(
        file_ns.join("mtime_index_v1__ast.json"),
        serde_json::to_string(&stale_idx).unwrap(),
    )
    .unwrap();

    // Stale architecture entry — empty graph at a fabricated fingerprint.
    // If the bug is present, the rebuilt architecture cache would also
    // be empty (stale per-file hash → same fingerprint → same lookup);
    // the recovery here is that on a real schema bump the per-file
    // hashes change, the fingerprint changes, and a fresh graph builds.
    let stale_graph = serde_json::json!({
        "nodes": {},
        "edges": [],
        "symbol_index": {},
        "module_index": {},
        "file_to_module_id": {},
    });
    fs::write(
        arch_ns.join("stalefp00000000000000000000000000000000000000000000000000000000.json"),
        serde_json::to_string(&stale_graph).unwrap(),
    )
    .unwrap();

    // First query after the "upgrade". No manual cache clear.
    let r = f.trace(&["callers", "b_fn", "--json"]);
    r.ok();
    let v = r.json();
    let callers = v["pkg/b.py::b_fn"]["callers"]
        .as_array()
        .expect("b_fn callers array must exist after schema-bump rebuild");
    assert!(
        !callers.is_empty(),
        "b_fn must have at least one caller after a schema-shape upgrade — \
         stale per-file hashes are keeping the architecture cache stale; got {:?}",
        v
    );
}

// ---------------------------------------------------------------------------
// Contradiction 2 — PHP `callers <ClassName>` zero on real Laravel idioms
// ---------------------------------------------------------------------------

#[test]
fn php_callers_captures_class_use_idioms() {
    // Each idiom-bearing file is a distinct use site of `User`.
    let f = Fixture::new();
    f.write(
        "app/Models/User.php",
        "<?php\nnamespace App\\Models;\nclass User {\n    public function id() { return 1; }\n}\n",
    );
    f.write(
        "app/Http/Controllers/StaticClassUser.php",
        "<?php\nnamespace App\\Http\\Controllers;\nuse App\\Models\\User;\nclass StaticClassUser {\n    public function handle() { return User::class; }\n}\n",
    );
    f.write(
        "app/Http/Controllers/TypeHintUser.php",
        "<?php\nnamespace App\\Http\\Controllers;\nuse App\\Models\\User;\nclass TypeHintUser {\n    public function show(User $u) { return $u->id(); }\n}\n",
    );
    f.write(
        "app/Http/Controllers/InstanceofUser.php",
        "<?php\nnamespace App\\Http\\Controllers;\nuse App\\Models\\User;\nclass InstanceofUser {\n    public function check($x) { return $x instanceof User; }\n}\n",
    );
    f.write(
        "app/Http/Controllers/CtorInjectUser.php",
        "<?php\nnamespace App\\Http\\Controllers;\nuse App\\Models\\User;\nclass CtorInjectUser {\n    public function __construct(User $user) {}\n}\n",
    );
    f.commit("php class use idioms");
    f.trace(&["cache", "build", "."]).ok();

    let r = f.trace(&["callers", "User", "--json"]);
    r.ok();
    let v = r.json();
    let callers = v["app/Models/User.php::User"]["callers"]
        .as_array()
        .expect("User callers array must exist");

    let files: Vec<&str> = callers
        .iter()
        .filter_map(|c| c["source_file"].as_str())
        .collect();
    for required in [
        "app/Http/Controllers/StaticClassUser.php",
        "app/Http/Controllers/TypeHintUser.php",
        "app/Http/Controllers/InstanceofUser.php",
        "app/Http/Controllers/CtorInjectUser.php",
    ] {
        assert!(
            files.contains(&required),
            "expected a use site in {required}; got files={:?}",
            files
        );
    }
}

#[test]
fn php_callers_fallback_to_module_importers_when_no_references() {
    // A class with no in-method use sites but plenty of importers must
    // not vanish from `callers`. The old behaviour (before the symbol
    // index existed) surfaced importer modules; the symbol index alone
    // returns zero rows; the fix preserves the importer fallback.
    let f = Fixture::new();
    f.write(
        "app/Models/Lonely.php",
        "<?php\nnamespace App\\Models;\nclass Lonely {}\n",
    );
    f.write(
        "app/Importers/A.php",
        "<?php\nnamespace App\\Importers;\nuse App\\Models\\Lonely;\nclass A {}\n",
    );
    f.write(
        "app/Importers/B.php",
        "<?php\nnamespace App\\Importers;\nuse App\\Models\\Lonely;\nclass B {}\n",
    );
    f.commit("php importer-only");
    f.trace(&["cache", "build", "."]).ok();

    let r = f.trace(&["callers", "Lonely", "--json"]);
    r.ok();
    let v = r.json();
    let entry = &v["app/Models/Lonely.php::Lonely"];
    let callers = entry["callers"]
        .as_array()
        .expect("Lonely callers must exist");
    let files: Vec<&str> = callers
        .iter()
        .filter_map(|c| c["source_file"].as_str())
        .collect();
    assert!(
        files.contains(&"app/Importers/A.php")
            && files.contains(&"app/Importers/B.php"),
        "module-importer fallback must surface importing files when the \
         symbol has zero references; got {:?}",
        files
    );
}

// ---------------------------------------------------------------------------
// Contradiction 3 — TS module-name caller rows mislabelled INFERRED
// ---------------------------------------------------------------------------

#[test]
fn typescript_module_caller_is_extracted_when_import_resolves() {
    // A module-name query whose only caller is an importing module whose
    // import string resolves cleanly to the target → EXTRACTED, not
    // INFERRED.
    let f = Fixture::new();
    f.write("src/helpers.ts", "export const a = 1;\n");
    f.write(
        "src/app.ts",
        "import { a } from './helpers';\nexport const x = a + 1;\n",
    );
    f.commit("ts clean import");
    f.trace(&["cache", "build", "."]).ok();

    let r = f.trace(&["callers", "helpers", "--json"]);
    r.ok();
    let v = r.json();
    let entry = &v["module::src/helpers"];
    let callers = entry["callers"]
        .as_array()
        .expect("helpers callers array must exist");
    let importer = callers
        .iter()
        .find(|c| c["node_id"].as_str() == Some("module::src/app"))
        .expect("src/app must be among helpers' callers");
    assert_eq!(
        importer["confidence"].as_str(),
        Some("EXTRACTED"),
        "a cleanly-resolved import must be EXTRACTED, not INFERRED: {:?}",
        importer
    );
}

// ---------------------------------------------------------------------------
// Contradiction 4 — `downstream --path P` empty where `upstream --path P` works
// ---------------------------------------------------------------------------

#[test]
fn downstream_path_mode_returns_results_when_upstream_does() {
    // A path inside the repo on which `upstream --path` returns results
    // must give `downstream --path` matching coverage — not the empty
    // "(no nodes in the architecture graph — cache may be empty)" verdict.
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
    f.write("pkg/c.py", "def c_fn(x):\n    return x + 1\n");
    f.commit("path-mode parity");
    f.trace(&["cache", "build", "."]).ok();

    // Resolve "pkg/a.py" via a single-file path. `a.py` imports a sibling
    // (pkg.b), and the architecture graph carries that internal edge.
    // The bug: path_mode set repo_root to the absolutized arg, so when the
    // arg is a file the git-ls-files fallback returned just that file —
    // and edges whose target lived outside that file disappeared on the
    // downstream side while the upstream side still found the importer.
    let pkg_file = f.path("pkg/a.py");

    let up = f.trace(&["upstream", "--path", &pkg_file, "--json"]);
    up.ok();
    let up_rows = up.json()["results"]
        .as_array()
        .expect("upstream rows must be an array")
        .len();

    let dn = f.trace(&["downstream", "--path", &pkg_file, "--json"]);
    dn.ok();
    let dn_value = dn.json();
    let dn_rows = dn_value["results"]
        .as_array()
        .expect("downstream rows must be an array")
        .len();

    assert!(up_rows > 0, "upstream must return rows on pkg/a.py: {:?}", up.stdout);
    assert!(
        dn_rows > 0,
        "downstream --path must agree with upstream --path on the same \
         path/cache; upstream returned {up_rows} rows but downstream \
         returned {dn_rows}. value={:?}",
        dn_value
    );
}

// ---------------------------------------------------------------------------
// Contradiction 5 — self-recursive calls reported as use sites of self
// ---------------------------------------------------------------------------

#[test]
fn self_recursive_call_is_not_a_caller_of_itself() {
    // A function whose only "use site" is its own recursive call must
    // not show up as its own caller. The contract pinned here: self
    // references do not appear in the caller list.
    let f = Fixture::new();
    f.write(
        "rec.py",
        "def fact(n):\n    if n <= 1:\n        return 1\n    return n * fact(n - 1)\n",
    );
    f.commit("recursion");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "fact", "--json"]);
    r.ok();
    let v = r.json();
    let callers = v["rec.py::fact"]["callers"]
        .as_array()
        .expect("fact callers must exist");
    assert!(
        callers.is_empty(),
        "self-recursive calls must not be reported as use sites of self; \
         got {:?}",
        callers
    );
}

// ---------------------------------------------------------------------------
// Contradiction 6 — `trace structure <ts-file>` reports Symbols: 0
// ---------------------------------------------------------------------------

#[test]
fn structure_reports_nonzero_symbols_for_tsx_file_with_declarations() {
    // universal-ctags returns zero entries on .tsx files, so structure
    // historically reported `Symbols: 0` even when the architecture graph
    // had every declaration indexed. The fix joins in graph-known
    // declarations when ctags is silent — Header/Footer/Widget must all
    // surface.
    let f = Fixture::new();
    f.write(
        "src/comp.tsx",
        "const Header = () => <div>x</div>;\nfunction Footer() { return <span/>; }\nexport class Widget {\n  show() { return 1; }\n}\nexport { Header, Footer };\n",
    );
    f.commit("tsx decls");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["structure", "src/comp.tsx", "--json"]);
    r.ok();
    let v = r.json();
    let count = v["symbol_count"].as_i64().unwrap_or(0);
    assert!(
        count > 0,
        "structure must report a non-zero symbol_count for a TSX file with \
         declarations; got {} value={:?}",
        count,
        v
    );

    // The human-text branch must agree.
    let r = f.trace(&["structure", "src/comp.tsx"]);
    r.ok();
    assert!(
        r.stdout.contains("Symbols: ") && !r.stdout.contains("Symbols: 0"),
        "human output must not say 'Symbols: 0' for a populated TSX file; \
         got\n{}",
        r.stdout
    );
}

