//! Regression coverage for the six contradictions surfaced by end-to-end
//! probes of the v4.8 release. Each test reproduces one contradiction
//! against a real-CLI fixture and pins the corrected behaviour.

use std::fs;
use tracer_cli_tests::Fixture;

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

// ---------------------------------------------------------------------------
// Contradiction 1 — stale relations index after a schema-shape change
// ---------------------------------------------------------------------------
//
// Simulates "binary upgrade against a repo with a pre-existing
// `.tracer-cache/`": we plant cache entries whose per-file hashes use an
// older SCHEMA_VERSION token, plus a matching mtime index and a prior
// schema's relations index. The new binary must NOT honour those stale
// per-file hashes — the first query has to re-absorb under the current
// schema — and the superseded index must be swept, not left to accumulate.

#[test]
fn the_relations_index_rebuilds_after_schema_shape_change() {
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
    fs::create_dir_all(&file_ns).unwrap();

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

    // A prior schema's relations index, holding an answer that contradicts
    // the tree: it claims nothing declares b_fn. The current binary keys its
    // index by the current schema, so this one is unreachable, and the
    // eviction sweep must then delete it.
    let stale_index = file_ns.join("relations_v1__schema1.json");
    let stale_relations = serde_json::json!({
        "symbols": {},
        "importers": {},
        "built_from": {"pkg/a.py": stale_file_key, "pkg/b.py": stale_file_key},
    });
    fs::write(
        &stale_index,
        serde_json::to_string(&stale_relations).unwrap(),
    )
    .unwrap();

    // First query after the "upgrade". No manual cache clear.
    let r = f.trace(&["callers", "b_fn", "--json"]);
    r.ok();
    let v = r.view();
    let callers = symbol(&v, "pkg/b.py::b_fn")["callers"]
        .as_array()
        .expect("b_fn callers array must exist after schema-bump rebuild");
    assert!(
        !callers.is_empty(),
        "b_fn must have at least one caller after a schema-shape upgrade — \
         stale per-file hashes are keeping the relations index stale; got {:?}",
        v
    );

    // Eviction: the prior schema's index is gone, and the namespace holds
    // exactly two current relations-index entries.
    assert!(
        !stale_index.exists(),
        "the prior schema's relations index survived the rebuild — a schema \
         bump would leave one behind on every upgrade"
    );
    let indexes: Vec<_> = fs::read_dir(&file_ns)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("relations_"))
        })
        .collect();
    assert_eq!(
        indexes.len(),
        2,
        "the file namespace must hold exactly two relations-index entries after the \
         rebuild; got {indexes:?}"
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
    let v = r.view();
    let callers = symbol(&v, "app/Models/User.php::User")["callers"]
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
    let v = r.view();
    let entry = symbol(&v, "app/Models/Lonely.php::Lonely");
    let callers = entry["callers"]
        .as_array()
        .expect("Lonely callers must exist");
    let files: Vec<&str> = callers
        .iter()
        .filter_map(|c| c["source_file"].as_str())
        .collect();
    assert!(
        files.contains(&"app/Importers/A.php") && files.contains(&"app/Importers/B.php"),
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
    let v = r.view();
    let entry = symbol(&v, "module::src/helpers");
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
// Contradiction 4 — a file scope amputated cross-file edges
// ---------------------------------------------------------------------------

#[test]
fn path_mode_scopes_subjects_but_keeps_cross_file_reach() {
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
    f.commit("scoped path reach");
    f.trace(&["cache", "build", "."]).ok();

    let middle = f.path("pkg/b.py");
    for (command, direct_key, transitive_key, node_id) in [
        (
            "dependencies",
            "direct_dependencies",
            "transitive_dependencies",
            "module::pkg.b",
        ),
        (
            "usages",
            "direct_dependents",
            "transitive_dependents",
            "pkg/b.py::b_fn",
        ),
    ] {
        let run = f.trace(&[command, "--path", &middle, "--json"]);
        run.ok();
        let document = run.json();
        for slot in ["query", "context", "results", "counts"] {
            assert!(
                document.get(slot).is_some(),
                "{command} omitted {slot}: {document}"
            );
        }
        assert_eq!(document["query"]["path"], middle);
        assert_eq!(document["counts"]["nodes"], 1);
        let rows = document["results"]
            .as_array()
            .expect("results must be rows");
        assert_eq!(rows.len(), 1, "only pkg/b.py may be ranked: {rows:?}");
        assert_eq!(rows[0]["node_id"], node_id);
        assert_eq!(rows[0][direct_key], 1);
        assert_eq!(rows[0][transitive_key], 1);
        assert!(
            document["context"]["files"].get("pkg/b.py").is_some(),
            "the positive subject must carry its passive context: {document}"
        );
    }

    let entry = f.path("pkg/a.py");
    let dependencies = f.trace(&["dependencies", "--path", &entry, "--json"]);
    dependencies.ok();
    let dependency_document = dependencies.json();
    let dependency_rows = dependency_document["results"]
        .as_array()
        .expect("dependencies results must be rows");
    assert_eq!(dependency_rows.len(), 1, "only pkg/a.py may be ranked");
    assert_eq!(dependency_rows[0]["node_id"], "module::pkg.a");
    assert_eq!(dependency_rows[0]["direct_dependencies"], 1);
    assert_eq!(dependency_rows[0]["transitive_dependencies"], 2);
    assert!(dependency_document["context"]["files"]
        .get("pkg/a.py")
        .is_some());

    let usages = f.trace(&["usages", "--path", &entry, "--json"]);
    usages.ok();
    let usage_document = usages.json();
    assert_eq!(usage_document["results"], serde_json::json!([]));
    assert_eq!(usage_document["counts"]["nodes"], 0);

    let human = f.trace(&["usages", "--path", &entry]);
    human.ok();
    assert_eq!(
        human.stdout.trim(),
        "(no files with dependents found in this scope)"
    );
    assert!(!human.stdout.contains("cache build"));
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
    let v = r.view();
    let callers = symbol(&v, "rec.py::fact")["callers"]
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
    let v = r.view();
    let count = v["symbols"].as_i64().unwrap_or(0);
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
