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
//!     resolves to a single non-method symbol or to nothing unless ambiguous
//!     import evidence names every candidate; a static / `new` / type-hint
//!     use resolves to the named class exactly; a cross-language call resolves
//!     to nothing. A member call whose receiver type the site does not name
//!     remains ambiguous and narrows to methods of that name, never to a free
//!     function or a wrong-language symbol.
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
    v["results"]
        .as_array()
        .unwrap_or_else(|| panic!("results must be a row list: {v}"))
        .iter()
        .find(|row| row["node_id"].as_str() == Some(def_node_id))
        .unwrap_or_else(|| panic!("no result row for {def_node_id}: {v}"))["callers"]
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

/// The one result row for `node_id`.
fn symbol<'a>(v: &'a serde_json::Value, node_id: &str) -> &'a serde_json::Value {
    v["results"]
        .as_array()
        .unwrap_or_else(|| panic!("results must be a row list: {v}"))
        .iter()
        .find(|row| row["node_id"].as_str() == Some(node_id))
        .unwrap_or_else(|| panic!("no result row for {node_id}: {v}"))
}

fn def_files(v: &serde_json::Value) -> Vec<(String, i64)> {
    v["results"]
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

#[test]
fn structure_json_carries_cached_php_and_typescript_headers() {
    let f = Fixture::new();
    f.write(
        "sample.php",
        r#"<?php
enum Suit: string {
    case Hearts = 'H';
}

class Sample {
    public int $a = 1, $b = 2;
    public string $name {
        &get { return ''; }
        set(string $value) { $this->name = $value; }
    }
    private const VERSION = 3;
    public function __construct(private readonly Level $level) {}
    /** @internal since 2.0 */
    public function old(): void {}
    #[Route('/samples', methods: ['POST'])]
    #[IsGranted('ROLE_ADMIN')]
    public static function create(array $attributes = []): static { return new static; }
}
function blockOnNextLine(): void
{
}
"#,
    );
    f.write(
        "sample.ts",
        r#"@Injectable()
export default class App extends React.Component<Props> {
    private count = 0;
    static readonly MAX = 3;
}
export interface Props {
    onSave(): void;
}
export type Status = 'idle' | 'saving';
function save(value: string): void;
function save(value: number): void;
function save(value: string | number): void {}
export const load = async ({ id }: Params): Promise<Item> => { return get(id); };
const labels = { save: 'Save', cancel: 'Cancel' };
abstract class Worker {
    abstract run(): void;
}
function outer() {
    const nested = () => 1;
}
const form = useForm({
  defaultValues: { email: "", password: "", remember: false } as LoginValues,
  onSubmit: async ({ value }) => { return value; },
});
const [a, setA] = useState(0);
"#,
    );
    f.commit("inventory headers");
    for (path, name, line, header_line, end_line, kind, container, parent, header) in [
        ("sample.php", "Suit", 2, 2, 4, "enum", None, None, "enum Suit: string { … }"),
        ("sample.php", "Hearts", 3, 3, 3, "property", Some("Suit"), Some(0), "case Hearts = 'H';"),
        ("sample.php", "Sample", 6, 6, 19, "class", None, None, "class Sample { … }"),
        ("sample.php", "$a", 7, 7, 7, "property", Some("Sample"), Some(2), "public int $a = 1;"),
        ("sample.php", "$b", 7, 7, 7, "property", Some("Sample"), Some(2), "public int $b = 2;"),
        ("sample.php", "$name", 8, 8, 11, "property", Some("Sample"), Some(2), "public string $name { &get { … } set(string $value) { … } }"),
        ("sample.php", "VERSION", 12, 12, 12, "constant", Some("Sample"), Some(2), "private const VERSION = 3;"),
        ("sample.php", "$level", 13, 13, 13, "property", Some("Sample"), Some(2), "private readonly Level $level"),
        ("sample.php", "old", 15, 14, 15, "function", Some("Sample"), Some(2), "/** @internal since 2.0 */\npublic function old(): void { … }"),
        ("sample.php", "create", 18, 16, 18, "function", Some("Sample"), Some(2), "#[Route('/samples', methods: ['POST'])]\n#[IsGranted('ROLE_ADMIN')]\npublic static function create(array $attributes = []): static { … }"),
        ("sample.php", "blockOnNextLine", 20, 20, 22, "function", None, None, "function blockOnNextLine(): void { … }"),
        ("sample.ts", "App", 2, 1, 5, "class", None, None, "@Injectable()\nexport default class App extends React.Component<Props> { … }"),
        ("sample.ts", "count", 3, 3, 3, "property", None, Some(0), "private count = 0;"),
        ("sample.ts", "MAX", 4, 4, 4, "property", None, Some(0), "static readonly MAX = 3;"),
        ("sample.ts", "Props", 6, 6, 8, "interface", None, None, "export interface Props { … }"),
        ("sample.ts", "onSave", 7, 7, 7, "function", None, Some(3), "onSave(): void;"),
        ("sample.ts", "Status", 9, 9, 9, "type", None, None, "export type Status = 'idle' | 'saving';"),
        ("sample.ts", "save", 10, 10, 10, "function", None, None, "function save(value: string): void;"),
        ("sample.ts", "save", 11, 11, 11, "function", None, None, "function save(value: number): void;"),
        ("sample.ts", "save", 12, 12, 12, "function", None, None, "function save(value: string | number): void { … }"),
        ("sample.ts", "load", 13, 13, 13, "constant", None, None, "export const load = async ({ id }: Params): Promise<Item> => { … };"),
        ("sample.ts", "labels", 14, 14, 14, "constant", None, None, "const labels = { save: 'Save', cancel: 'Cancel' };"),
        ("sample.ts", "Worker", 15, 15, 17, "class", None, None, "abstract class Worker { … }"),
        ("sample.ts", "run", 16, 16, 16, "function", None, Some(11), "abstract run(): void;"),
        ("sample.ts", "outer", 18, 18, 20, "function", None, None, "function outer() { … }"),
        ("sample.ts", "nested", 19, 19, 19, "constant", None, Some(13), "const nested = () => …;"),
        ("sample.ts", "form", 21, 21, 24, "constant", None, None, "const form = useForm({\n  defaultValues: { email: \"\", password: \"\", remember: false } as LoginValues,\n  onSubmit: async ({ value }) => { … },\n});"),
        ("sample.ts", "a", 25, 25, 25, "constant", None, None, "const [a, setA] = useState(0);"),
    ] {
        let result = if path == "deploy.sh" {
            f.trace_env(
                &["structure", path, "--json"],
                &[("TRACE_TIMING", "1")],
            )
        } else {
            f.trace(&["structure", path, "--json"])
        };
        result.ok();
        if path == "deploy.sh" {
            assert!(result.stderr.contains("timing ctags "), "{}", result.stderr);
        }
        let view = result.view();
        let rows = view["symbols_by_kind"].as_object().unwrap();
        let row = rows
            .values()
            .flat_map(|rows| rows.as_array().into_iter().flatten())
            .find(|row| row["name"] == name && row["line"] == line)
            .unwrap_or_else(|| panic!("missing {path}:{line} {name}: {view:#}"));
        assert_eq!(row["header_line"], header_line, "{path}:{line} {name}");
        assert_eq!(row["line"], line, "{path}:{line} {name}");
        assert_eq!(row["end_line"], end_line, "{path}:{line} {name}");
        assert_eq!(row["kind"], kind, "{path}:{line} {name}");
        assert_eq!(row["container"].as_str(), container, "{path}:{line} {name}");
        assert_eq!(row["parent"].as_u64(), parent.map(|parent| parent as u64), "{path}:{line} {name}");
        assert_eq!(row["header"], header, "{path}:{line} {name}");
    }
    let result = f.trace(&["structure", "sample.ts", "--json"]);
    result.ok();
    let rows = result.view()["symbols_by_kind"]
        .as_object()
        .unwrap()
        .clone();
    let destructured = rows
        .values()
        .flat_map(|rows| rows.as_array().into_iter().flatten())
        .filter(|row| row["header"] == "const [a, setA] = useState(0);")
        .count();
    assert_eq!(destructured, 2, "{rows:#?}");
}

// ---------------------------------------------------------------------------
// Python — every declaration kind, every confidence

/// A file that appears after its importer resolves that importer's existing
/// import: the update re-resolves every importer whose rows name the new
/// file's stem, from the stored import rows, without re-reading the tree.
#[test]
fn an_added_file_resolves_the_importer_already_naming_it() {
    let f = Fixture::new();
    f.write("pkg/__init__.py", "");
    f.write(
        "pkg/late.py",
        "from thing import go\n\ndef run():\n    return go()\n",
    );
    f.commit("importer first");
    f.trace(&["cache", "build", "."]).ok();

    f.write("pkg/thing.py", "def go():\n    return 1\n");
    f.commit("dependency later");
    let r = f.trace(&["usages", "go", "--json"]);
    r.ok();
    let v = r.view();

    let dependents = symbol(&v, "pkg/thing.py::go")["dependents"]
        .as_array()
        .unwrap_or_else(|| panic!("dependents must be a row list: {v}"));
    assert!(
        dependents
            .iter()
            .any(|d| d["source_file"].as_str() == Some("pkg/late.py")),
        "pkg/late.py imports the added file by suffix and must depend on it: {v}"
    );
}
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
    let v = r.view();
    assert_eq!(v["definitions"].as_i64().unwrap(), 1);
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
    let v = r.view();
    assert_eq!(v["definitions"].as_i64().unwrap(), 1);
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
    let v = r.view();
    assert_eq!(v["definitions"].as_i64().unwrap(), 1);
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
    let v = r.view();
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
    let v = r.view();
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
    f.write(
        "job.py",
        "class Job:\n    def run(self):\n        return 1\n",
    );
    f.write(
        "task.py",
        "class Task:\n    def run(self):\n        return 2\n",
    );
    f.write("free.py", "def run():\n    return 0\n");
    f.write("caller.py", "def go(obj):\n    return obj.run()\n");
    f.commit("py member collision");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "run", "--json"]);
    r.ok();
    let v = r.view();
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
fn python_member_call_on_a_value_resolves_only_through_an_import() {
    let f = Fixture::new();
    f.write(
        "store.py",
        concat!(
            "class Store:\n",
            "    def save(self):\n",
            "        return 1\n",
            "\n",
            "    def flush(self):\n",
            "        return self.save()\n",
            "\n",
            "\n",
            "def run(cache):\n",
            "    return cache.save()\n",
        ),
    );
    f.write(
        "keeper.py",
        "from store import Store\n\n\ndef keep(store):\n    return store.save()\n",
    );
    f.write("other.py", "def drop(box):\n    return box.save()\n");
    f.commit("py value receivers");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "save", "--json"]);
    r.ok();
    let mut rows = caller_rows(&r.view(), "store.py::save");
    rows.sort();
    assert_eq!(
        rows,
        vec![
            ("keeper.py".to_string(), 5, "EXTRACTED".to_string()),
            ("other.py".to_string(), 2, "AMBIGUOUS".to_string()),
            ("store.py".to_string(), 6, "EXTRACTED".to_string()),
            ("store.py".to_string(), 10, "AMBIGUOUS".to_string()),
        ]
    );
}

#[test]
fn python_super_and_cls_calls_keep_the_calling_file_confidence() {
    let f = Fixture::new();
    f.write(
        "model.py",
        concat!(
            "class Base:\n",
            "    @classmethod\n",
            "    def create(cls):\n",
            "        return cls()\n",
            "\n",
            "    @classmethod\n",
            "    def load(cls):\n",
            "        return cls.create()\n",
            "\n",
            "\n",
            "class Child(Base):\n",
            "    @classmethod\n",
            "    def build(cls):\n",
            "        return super().create()\n",
        ),
    );
    f.commit("py self receivers");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "create", "--json"]);
    r.ok();
    let mut rows = caller_rows(&r.view(), "model.py::create");
    rows.sort();
    assert_eq!(
        rows,
        vec![
            ("model.py".to_string(), 8, "EXTRACTED".to_string()),
            ("model.py".to_string(), 14, "EXTRACTED".to_string()),
        ]
    );
}

#[test]
fn python_inferred_reference_resolves_without_target_module() {
    // The caller does not import `lone_unique_name` from anywhere; the
    // symbol has a single uniquely-named declaration. The reference
    // resolves by name alone — INFERRED.
    let f = Fixture::new();
    f.write("tgt.py", "def lone_unique_name():\n    return 1\n");
    f.write("caller.py", "def use():\n    return lone_unique_name()\n");
    f.commit("py inferred");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "lone_unique_name", "--json"]);
    r.ok();
    let v = r.view();
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
    let v = r.view();
    assert_eq!(v["definitions"].as_i64().unwrap(), 1);
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
    let v = r.view();
    assert_eq!(v["definitions"].as_i64().unwrap(), 1);
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
    let v = r.view();
    assert_eq!(v["definitions"].as_i64().unwrap(), 1);
    let defs = def_files(&v);
    assert_eq!(defs, vec![("nest.ts".to_string(), 2)]);
}

#[test]
fn ts_callers_returns_use_sites() {
    let f = Fixture::new();
    f.write(
        "util.ts",
        "export function helper(x: number): number { return x; }\n",
    );
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
    let v = r.view();
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
    f.write(
        "a.ts",
        "export function compute(x: number): number { return x; }\n",
    );
    f.write(
        "b.ts",
        "export function compute(x: number): number { return x + 1; }\n",
    );
    f.write(
        "caller.ts",
        "export function go(): number { return compute(1); }\n",
    );
    f.commit("ts free collision");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "compute", "--json"]);
    r.ok();
    let v = r.view();
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
fn free_call_resolves_to_its_own_file_before_an_imported_namesake() {
    let f = Fixture::new();
    f.write(
        "comments.ts",
        "export function CommentsSection(): string { return formatDate(new Date()); }\nfunction formatDate(d: Date): string { return 'comments'; }\n",
    );
    f.write(
        "editor.ts",
        concat!(
            "import { CommentsSection } from './comments';\n",
            "\n",
            "export function OrderEditor(): string { return CommentsSection() + formatDate(new Date()); }\n",
            "function formatDate(d: Date): string { return 'editor'; }\n",
        ),
    );
    f.commit("a local function beside an imported file's namesake");
    let r = f.trace(&["callers", "formatDate", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(references_of(&v, "editor.ts::formatDate", 4), extracted("editor.ts", &[3]), "{v}");
    assert_eq!(references_of(&v, "comments.ts::formatDate", 2), extracted("comments.ts", &[1]), "{v}");
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
    f.write(
        "helpers.ts",
        "export function save(): number { return 0; }\n",
    );
    f.write(
        "caller.ts",
        "export function go(obj: any): number { return obj.save(); }\n",
    );
    f.commit("ts member collision");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "save", "--json"]);
    r.ok();
    let v = r.view();
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
fn ts_member_call_on_a_value_resolves_only_through_an_import() {
    let f = Fixture::new();
    f.write(
        "api.ts",
        concat!(
            "export class TenantApi {\n",
            "  delete(): void {}\n",
            "  purge(): void { this.delete(); }\n",
            "}\n",
            "\n",
            "export function drop(ids: Set<number>): void { ids.delete(1); }\n",
        ),
    );
    f.write(
        "caller.ts",
        "import { TenantApi } from './api';\n\nexport function remove(api: TenantApi): void { api.delete(); }\n",
    );
    f.write(
        "other.ts",
        "export function clear(names: Set<string>): void { names.delete('x'); }\n",
    );
    f.commit("ts value receivers");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "delete", "--json"]);
    r.ok();
    let mut rows = caller_rows(&r.view(), "api.ts::delete");
    rows.sort();
    assert_eq!(
        rows,
        vec![
            ("api.ts".to_string(), 3, "EXTRACTED".to_string()),
            ("api.ts".to_string(), 6, "AMBIGUOUS".to_string()),
            ("caller.ts".to_string(), 3, "EXTRACTED".to_string()),
            ("other.ts".to_string(), 1, "AMBIGUOUS".to_string()),
        ]
    );
}

#[test]
fn ts_super_call_keeps_the_calling_file_confidence() {
    let f = Fixture::new();
    f.write(
        "page.ts",
        concat!(
            "export class Base {\n",
            "  render(): void {}\n",
            "}\n",
            "\n",
            "export class Page extends Base {\n",
            "  paint(): void { super.render(); }\n",
            "}\n",
        ),
    );
    f.commit("ts super receiver");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "render", "--json"]);
    r.ok();
    assert_eq!(
        caller_rows(&r.view(), "page.ts::render"),
        vec![("page.ts".to_string(), 6, "EXTRACTED".to_string())]
    );
}

#[test]
fn ts_class_method_call_keeps_the_calling_file_confidence() {
    let f = Fixture::new();
    f.write(
        "cart.ts",
        concat!(
            "export class Cart {\n",
            "  static make(): Cart { return new Cart(); }\n",
            "}\n",
            "\n",
            "export function open(): Cart { return Cart.make(); }\n",
        ),
    );
    f.commit("ts class receiver");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "make", "--json"]);
    r.ok();
    assert_eq!(
        caller_rows(&r.view(), "cart.ts::make"),
        vec![("cart.ts".to_string(), 5, "EXTRACTED".to_string())]
    );
}

#[test]
fn a_line_that_calls_one_target_twice_is_one_row() {
    let f = Fixture::new();
    f.write(
        "util.ts",
        "export function helper(x: number): number { return x; }\n",
    );
    f.write(
        "app.ts",
        "import { helper } from './util';\n\nexport function twice(): number { return helper(helper(1)); }\n",
    );
    f.commit("one line, two calls");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "helper", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(
        caller_rows(&v, "util.ts::helper"),
        vec![("app.ts".to_string(), 3, "EXTRACTED".to_string())]
    );
    assert_eq!(v["callers"].as_i64(), Some(1));
    assert_eq!(v["total"].as_i64(), Some(1));
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
    let v = r.view();
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
    let v = r.view();
    assert_eq!(v["definitions"].as_i64().unwrap(), 1);
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
    let v = r.view();
    assert_eq!(v["definitions"].as_i64().unwrap(), 1);
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
    let v = r.view();
    assert_eq!(v["definitions"].as_i64().unwrap(), 1);
    let defs = def_files(&v);
    assert_eq!(defs, vec![("nest.php".to_string(), 3)]);
}

#[test]
fn php_callers_returns_use_sites() {
    let f = Fixture::new();
    f.write("util.php", "<?php\nfunction helper($x) { return $x; }\n");
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
    let v = r.view();
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
    f.write(
        "caller.php",
        "<?php\nfunction go() { return compute(1); }\n",
    );
    f.commit("php free collision");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "compute", "--json"]);
    r.ok();
    let v = r.view();
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
    let v = r.view();
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
fn php_member_call_with_a_stated_receiver_resolves_to_that_class() {
    let f = Fixture::new();
    f.write(
        "first.php",
        "<?php\nclass First {\n  public function handle() { return 1; }\n}\n",
    );
    f.write(
        "second.php",
        "<?php\nclass Second {\n  public function handle() { return 2; }\n}\n",
    );
    f.write(
        "caller.php",
        concat!(
            "<?php\n",
            "class Caller {\n",
            "  public function __construct(private readonly First $promoted) {}\n",
            "  private First $declared;\n",
            "  public function viaParameter(First $first) { return $first->handle(); }\n",
            "  public function viaPromoted() { return $this->promoted->handle(); }\n",
            "  public function viaDeclared() { return $this->declared->handle(); }\n",
            "  public function viaContainer() { return app(First::class)->handle(); }\n",
            "  public function viaNew() { return (new First())->handle(); }\n",
            "  public function viaLocal() { $first = new First(); return $first->handle(); }\n",
            "  public function viaExternal() { $method = new ReflectionMethod('a', 'b'); return $method->handle(); }\n",
            "}\n",
        ),
    );
    f.commit("php stated receivers");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "handle", "--json"]);
    r.ok();
    let v = r.view();
    let rows_first = caller_rows(&v, "first.php::handle");
    let rows_second = caller_rows(&v, "second.php::handle");
    for line in 5..=10 {
        assert!(
            rows_first
                .iter()
                .any(|(f, l, c)| f == "caller.php" && *l == line && c != "AMBIGUOUS"),
            "First::handle must record a resolved call at caller.php:{line} — got {:?}",
            rows_first
        );
    }
    assert!(
        !rows_second.iter().any(|(f, _, _)| f == "caller.php"),
        "Second::handle must receive no call whose receiver is stated as another class — got {:?}",
        rows_second
    );
    assert!(
        !rows_first.iter().any(|(f, l, _)| f == "caller.php" && *l == 11),
        "a receiver stated as an external class resolves to no project method — got {:?}",
        rows_first
    );
}

#[test]
fn php_this_self_and_static_resolve_to_the_enclosing_class() {
    let f = Fixture::new();
    f.write("composer.json", r#"{"autoload":{"psr-4":{"App\\":"src/"}}}"#);
    f.write(
        "src/Models/Order.php",
        concat!(
            "<?php\n",
            "namespace App\\Models;\n",
            "class Order {\n",
            "    public function items() { return []; }\n",
            "    public static function label() { return 'order'; }\n",
            "}\n",
        ),
    );
    f.write(
        "src/Billing/Subscription.php",
        concat!(
            "<?php\n",
            "namespace App\\Billing;\n",
            "use App\\Models\\Order;\n",
            "class Subscription {\n",
            "    public function items() { return []; }\n",
            "    public static function label() { return 'subscription'; }\n",
            "    public function renew(Order $order) { return $this->items(); }\n",
            "    public function title() { return self::label() . static::label(); }\n",
            "}\n",
        ),
    );
    f.commit("own-class receivers beside an imported namesake");
    let subscription = "src/Billing/Subscription.php";
    let r = f.trace(&["callers", "items", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(references_of(&v, &format!("{subscription}::items"), 5), extracted(subscription, &[7]), "{v}");
    assert_eq!(references_of(&v, "src/Models/Order.php::items", 4), vec![], "{v}");
    let r = f.trace(&["callers", "label", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(references_of(&v, &format!("{subscription}::label"), 6), extracted(subscription, &[8]), "{v}");
    assert_eq!(references_of(&v, "src/Models/Order.php::label", 5), vec![], "{v}");
}

#[test]
fn php_this_call_to_an_inherited_method_resolves_to_the_parent() {
    let f = Fixture::new();
    f.write("composer.json", r#"{"autoload":{"psr-4":{"App\\":"src/"}}}"#);
    f.write(
        "src/Core/DatabaseEntity.php",
        "<?php\nnamespace App\\Core;\nabstract class DatabaseEntity {\n    public function save() { return true; }\n}\n",
    );
    f.write(
        "src/Mail/Draft.php",
        "<?php\nnamespace App\\Mail;\nclass Draft {\n    public function save() { return false; }\n}\n",
    );
    f.write(
        "src/Models/Order.php",
        concat!(
            "<?php\n",
            "namespace App\\Models;\n",
            "use App\\Core\\DatabaseEntity;\n",
            "use Illuminate\\Contracts\\Order as OrderContract;\n",
            "class Order extends DatabaseEntity implements OrderContract {\n",
            "    public function checkout() { return $this->save(); }\n",
            "}\n",
        ),
    );
    f.commit("an inherited method called through $this");
    let r = f.trace(&["callers", "save", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(
        references_of(&v, "src/Core/DatabaseEntity.php::save", 4),
        extracted("src/Models/Order.php", &[6]),
        "{v}"
    );
    assert_eq!(references_of(&v, "src/Mail/Draft.php::save", 4), vec![], "{v}");
}

#[test]
fn php_member_call_on_a_value_resolves_only_through_an_import() {
    let f = Fixture::new();
    f.write("composer.json", r#"{"autoload":{"psr-4":{"App\\":"src/"}}}"#);
    f.write(
        "src/Http/Response.php",
        concat!(
            "<?php\n",
            "namespace App\\Http;\n",
            "class Response {\n",
            "    public function json() { return []; }\n",
            "    public function body() { return $this->json(); }\n",
            "    public function merge($other) { return $other->json(); }\n",
            "}\n",
        ),
    );
    f.write(
        "src/Pay/PayPal.php",
        concat!(
            "<?php\n",
            "namespace App\\Pay;\n",
            "class PayPal {\n",
            "    public function charge($response) { return $response->json(); }\n",
            "}\n",
        ),
    );
    f.write(
        "src/Api/Client.php",
        concat!(
            "<?php\n",
            "namespace App\\Api;\n",
            "use App\\Http\\Response;\n",
            "class Client {\n",
            "    public function read($response) { return $response->json(); }\n",
            "    public function parse(Response $response) { return $response->json(); }\n",
            "}\n",
        ),
    );
    f.commit("php value receivers");
    let r = f.trace(&["callers", "json", "--json"]);
    r.ok();
    let mut rows = caller_rows(&r.view(), "src/Http/Response.php::json");
    rows.sort();
    assert_eq!(
        rows,
        vec![
            ("src/Api/Client.php".to_string(), 5, "EXTRACTED".to_string()),
            ("src/Api/Client.php".to_string(), 6, "EXTRACTED".to_string()),
            ("src/Http/Response.php".to_string(), 5, "EXTRACTED".to_string()),
            ("src/Http/Response.php".to_string(), 6, "AMBIGUOUS".to_string()),
            ("src/Pay/PayPal.php".to_string(), 4, "AMBIGUOUS".to_string()),
        ]
    );
}

/// `Response` and `Payload` each declare `json`, so a `json` call whose
/// receiver has no stated class is `AMBIGUOUS` against both.
fn two_json_classes(f: &Fixture) {
    f.write("composer.json", r#"{"autoload":{"psr-4":{"App\\":"src/"}}}"#);
    f.write(
        "src/Http/Response.php",
        "<?php\nnamespace App\\Http;\nclass Response {\n    public function json() { return []; }\n}\n",
    );
    f.write(
        "src/Http/Payload.php",
        "<?php\nnamespace App\\Http;\nclass Payload {\n    public function json() { return []; }\n}\n",
    );
}

fn sorted_caller_rows(f: &Fixture, def_node_id: &str) -> Vec<(String, i64, String)> {
    let r = f.trace(&["callers", "json", "--json"]);
    r.ok();
    let mut rows = caller_rows(&r.view(), def_node_id);
    rows.sort();
    rows
}

fn row(file: &str, line: i64, confidence: &str) -> (String, i64, String) {
    (file.to_string(), line, confidence.to_string())
}

#[test]
fn php_local_assigned_from_a_call_takes_its_declared_return_type() {
    let f = Fixture::new();
    two_json_classes(&f);
    f.write(
        "src/Http/Client.php",
        concat!(
            "<?php\n",
            "namespace App\\Http;\n",
            "class Client {\n",
            "    public function post(string $path, ?array $body = null): Response { return new Response(); }\n",
            "    public static function open(): Response { return new Response(); }\n",
            "    public function raw() { return new Response(); }\n",
            "    public function viaThis() { $response = $this->post('/a'); return $response->json(); }\n",
            "    public function viaItself() { $node = $node->post('/b'); return $node->json(); }\n",
            "}\n",
        ),
    );
    f.write(
        "src/Flows/Funnels.php",
        concat!(
            "<?php\n",
            "namespace App\\Flows;\n",
            "use App\\Http\\Client;\n",
            "class Funnels {\n",
            "    public function viaParameter(Client $client) { $created = $client->post('/b', ['x' => 1]); return $created->json(); }\n",
            "    public function viaStatic() { $opened = Client::open(); return $opened->json(); }\n",
            "    public function viaUntyped(Client $client) { $raw = $client->raw(); return $raw->json(); }\n",
            "}\n",
        ),
    );
    f.write(
        "src/Flows/Reports.php",
        concat!(
            "<?php\n",
            "namespace App\\Flows;\n",
            "use App\\Http\\Client;\n",
            "use App\\Http\\Payload;\n",
            "use App\\Http\\Response;\n",
            "class Reports {\n",
            "    public function read(Client $client) { $read = $client->post('/c'); return $read->json(); }\n",
            "}\n",
        ),
    );
    f.commit("locals assigned from calls with declared return types");
    assert_eq!(
        sorted_caller_rows(&f, "src/Http/Response.php::json"),
        vec![
            row("src/Flows/Funnels.php", 5, "INFERRED"),
            row("src/Flows/Funnels.php", 6, "INFERRED"),
            row("src/Flows/Funnels.php", 7, "AMBIGUOUS"),
            row("src/Flows/Reports.php", 7, "EXTRACTED"),
            row("src/Http/Client.php", 7, "INFERRED"),
            row("src/Http/Client.php", 8, "AMBIGUOUS"),
        ]
    );
    assert_eq!(
        sorted_caller_rows(&f, "src/Http/Payload.php::json"),
        vec![row("src/Flows/Funnels.php", 7, "AMBIGUOUS"), row("src/Http/Client.php", 8, "AMBIGUOUS")]
    );
}

#[test]
fn php_chained_call_takes_the_declared_return_type() {
    let f = Fixture::new();
    two_json_classes(&f);
    f.write(
        "src/Http/Client.php",
        concat!(
            "<?php\n",
            "namespace App\\Http;\n",
            "class Client {\n",
            "    public function post(string $path): Response { return new Response(); }\n",
            "    public static function make(): static { return new static(); }\n",
            "    public function fresh(): self { return $this; }\n",
            "    public function viaThis() { return $this->post('/a')->json(); }\n",
            "    public function viaSelf() { return $this->fresh()->post('/a')->json(); }\n",
            "}\n",
        ),
    );
    f.write(
        "src/Flows/Funnels.php",
        concat!(
            "<?php\n",
            "namespace App\\Flows;\n",
            "use App\\Http\\Client;\n",
            "class Funnels {\n",
            "    public function viaParameter(Client $client) { return $client->post('/b')?->json(); }\n",
            "    public function viaStatic() { return Client::make()->post('/c')->json(); }\n",
            "}\n",
        ),
    );
    f.commit("chained calls with declared return types");
    assert_eq!(
        sorted_caller_rows(&f, "src/Http/Response.php::json"),
        vec![
            row("src/Flows/Funnels.php", 5, "INFERRED"),
            row("src/Flows/Funnels.php", 6, "INFERRED"),
            row("src/Http/Client.php", 7, "INFERRED"),
            row("src/Http/Client.php", 8, "INFERRED"),
        ]
    );
    assert_eq!(sorted_caller_rows(&f, "src/Http/Payload.php::json"), vec![]);
}

#[test]
fn php_return_type_states_a_receiver_only_when_it_names_one_class_of_the_repository() {
    let f = Fixture::new();
    two_json_classes(&f);
    f.write(
        "src/Http/Client.php",
        concat!(
            "<?php\n",
            "namespace App\\Http;\n",
            "class Client {\n",
            "    public function maybe(): ?Response { return null; }\n",
            "    public function either(): Response|null { return null; }\n",
            "    public function both(): Response|Payload { return new Payload(); }\n",
            "    public function viaNullable() { return $this->maybe()->json(); }\n",
            "    public function viaNullUnion() { return $this->either()->json(); }\n",
            "    public function viaClassUnion() { return $this->both()->json(); }\n",
            "}\n",
        ),
    );
    f.write(
        "src/Pay/PayPal.php",
        concat!(
            "<?php\n",
            "namespace App\\Pay;\n",
            "use Illuminate\\Http\\Client\\Response;\n",
            "class PayPal {\n",
            "    private function token(): Response { return new Response(); }\n",
            "    public function charge() { $response = $this->token(); return $response->json(); }\n",
            "}\n",
        ),
    );
    f.commit("return types naming no single class of the repository");
    assert_eq!(
        sorted_caller_rows(&f, "src/Http/Response.php::json"),
        vec![
            row("src/Http/Client.php", 7, "INFERRED"),
            row("src/Http/Client.php", 8, "INFERRED"),
            row("src/Http/Client.php", 9, "AMBIGUOUS"),
            row("src/Pay/PayPal.php", 6, "AMBIGUOUS"),
        ]
    );
    assert_eq!(
        sorted_caller_rows(&f, "src/Http/Payload.php::json"),
        vec![row("src/Http/Client.php", 9, "AMBIGUOUS"), row("src/Pay/PayPal.php", 6, "AMBIGUOUS")]
    );
}

#[test]
fn php_returned_class_that_inherits_the_method_resolves_to_the_ancestor_that_declares_it() {
    let f = Fixture::new();
    f.write("composer.json", r#"{"autoload":{"psr-4":{"App\\":"src/"}}}"#);
    f.write(
        "src/Core/Entity.php",
        "<?php\nnamespace App\\Core;\nabstract class Entity {\n    public function save() { return true; }\n}\n",
    );
    f.write(
        "src/Mail/Draft.php",
        "<?php\nnamespace App\\Mail;\nclass Draft {\n    public function save() { return false; }\n}\n",
    );
    f.write(
        "src/Models/Account.php",
        concat!(
            "<?php\n",
            "namespace App\\Models;\n",
            "use App\\Core\\Entity;\n",
            "class Account extends Entity {\n",
            "    public static function find(): static { return new static(); }\n",
            "}\n",
        ),
    );
    f.write(
        "src/Http/Controller.php",
        concat!(
            "<?php\n",
            "namespace App\\Http;\n",
            "use App\\Models\\Account;\n",
            "class Controller {\n",
            "    public function update() { $account = Account::find(); return $account->save(); }\n",
            "}\n",
        ),
    );
    f.commit("a returned class whose method is inherited");
    let r = f.trace(&["callers", "save", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(caller_rows(&v, "src/Core/Entity.php::save"), vec![row("src/Http/Controller.php", 5, "INFERRED")]);
    assert_eq!(caller_rows(&v, "src/Mail/Draft.php::save"), Vec::<(String, i64, String)>::new());
}

#[test]
fn php_typed_receiver_reaches_the_nearest_declaration_through_traits_and_parents() {
    let f = Fixture::new();
    f.write("composer.json", r#"{"autoload":{"psr-4":{"App\\":"src/"}}}"#);
    f.write(
        "src/Core/Entity.php",
        concat!(
            "<?php\n",
            "namespace App\\Core;\n",
            "abstract class Entity {\n",
            "    public function save() { return true; }\n",
            "    public function label() { return 'entity'; }\n",
            "    public function greet() { return 'entity'; }\n",
            "}\n",
        ),
    );
    f.write(
        "src/Core/Greets.php",
        "<?php\nnamespace App\\Core;\ntrait Greets {\n    public function greet() { return 'hi'; }\n}\n",
    );
    f.write(
        "src/Mail/Draft.php",
        concat!(
            "<?php\n",
            "namespace App\\Mail;\n",
            "class Draft {\n",
            "    public function save() { return false; }\n",
            "    public function greet() { return 'draft'; }\n",
            "    public function label() { return 'draft'; }\n",
            "}\n",
        ),
    );
    f.write(
        "src/Models/Account.php",
        concat!(
            "<?php\n",
            "namespace App\\Models;\n",
            "use App\\Core\\Entity;\n",
            "use App\\Core\\Greets;\n",
            "class Account extends Entity {\n",
            "    use Greets;\n",
            "    public function label() { return 'account'; }\n",
            "}\n",
        ),
    );
    f.write(
        "src/Http/Controller.php",
        concat!(
            "<?php\n",
            "namespace App\\Http;\n",
            "use App\\Models\\Account;\n",
            "class Controller {\n",
            "    public function update(Account $account) {\n",
            "        $account->save();\n",
            "        $account->greet();\n",
            "        return $account->label();\n",
            "    }\n",
            "}\n",
        ),
    );
    f.commit("an inherited method, a trait method, and an own method");
    let saves = f.trace(&["callers", "save", "--json"]);
    saves.ok();
    let v = saves.view();
    assert_eq!(caller_rows(&v, "src/Core/Entity.php::save"), vec![row("src/Http/Controller.php", 6, "INFERRED")]);
    assert_eq!(caller_rows(&v, "src/Mail/Draft.php::save"), Vec::<(String, i64, String)>::new());
    let greets = f.trace(&["callers", "greet", "--json"]);
    greets.ok();
    let v = greets.view();
    assert_eq!(caller_rows(&v, "src/Core/Greets.php::greet"), vec![row("src/Http/Controller.php", 7, "INFERRED")]);
    assert!(!caller_rows(&v, "src/Core/Entity.php::greet").iter().any(|(file, _, _)| file == "src/Http/Controller.php"));
    let labels = f.trace(&["callers", "label", "--json"]);
    labels.ok();
    let v = labels.view();
    assert_eq!(caller_rows(&v, "src/Models/Account.php::label"), vec![row("src/Http/Controller.php", 8, "EXTRACTED")]);
    assert!(!caller_rows(&v, "src/Core/Entity.php::label").iter().any(|(file, _, _)| file == "src/Http/Controller.php"));
}

#[test]
fn php_call_on_a_base_type_reaches_every_override_as_ambiguous() {
    let f = Fixture::new();
    f.write("composer.json", r#"{"autoload":{"psr-4":{"App\\":"src/"}}}"#);
    f.write(
        "src/View/Element.php",
        "<?php\nnamespace App\\View;\nabstract class Element {\n    abstract public function render(): string;\n}\n",
    );
    f.write(
        "src/View/Button.php",
        "<?php\nnamespace App\\View;\nclass Button extends Element {\n    public function render(): string { return 'b'; }\n}\n",
    );
    f.write(
        "src/View/Renders.php",
        "<?php\nnamespace App\\View;\ninterface Renders {\n    public function render(): string;\n}\n",
    );
    f.write(
        "src/View/Card.php",
        "<?php\nnamespace App\\View;\nclass Card implements Renders {\n    public function render(): string { return 'c'; }\n}\n",
    );
    f.write(
        "src/Mail/Template.php",
        "<?php\nnamespace App\\Mail;\nclass Template {\n    public function render(): string { return 't'; }\n}\n",
    );
    f.write(
        "src/Http/Page.php",
        concat!(
            "<?php\n",
            "namespace App\\Http;\n",
            "use App\\View\\Element;\n",
            "use App\\View\\Renders;\n",
            "class Page {\n",
            "    public function show(Element $element, Renders $card) {\n",
            "        $element->render();\n",
            "        return $card->render();\n",
            "    }\n",
            "}\n",
        ),
    );
    f.commit("calls on an abstract class and on an interface");
    let r = f.trace(&["callers", "render", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(caller_rows(&v, "src/View/Element.php::render"), vec![row("src/Http/Page.php", 7, "EXTRACTED")]);
    assert_eq!(caller_rows(&v, "src/View/Button.php::render"), vec![row("src/Http/Page.php", 7, "AMBIGUOUS")]);
    assert_eq!(caller_rows(&v, "src/View/Renders.php::render"), vec![row("src/Http/Page.php", 8, "EXTRACTED")]);
    assert_eq!(caller_rows(&v, "src/View/Card.php::render"), vec![row("src/Http/Page.php", 8, "AMBIGUOUS")]);
    assert_eq!(caller_rows(&v, "src/Mail/Template.php::render"), Vec::<(String, i64, String)>::new());
}

#[test]
fn php_lineage_follows_the_class_the_calling_file_names_never_a_namesake() {
    let f = Fixture::new();
    f.write("composer.json", r#"{"autoload":{"psr-4":{"App\\":"src/"}}}"#);
    f.write(
        "src/Core/Entity.php",
        "<?php\nnamespace App\\Core;\nabstract class Entity {\n    public function delete() { return true; }\n}\n",
    );
    f.write(
        "src/Mail/Audience.php",
        "<?php\nnamespace App\\Mail;\nclass Audience {\n    public function delete() { return false; }\n}\n",
    );
    f.write(
        "src/Tokens/Token.php",
        "<?php\nnamespace App\\Tokens;\nuse App\\Core\\Entity;\nfinal class Token extends Entity {}\n",
    );
    f.write(
        "src/Access/Token.php",
        "<?php\nnamespace App\\Access;\nuse App\\Mail\\Audience;\nfinal class Token extends Audience {}\n",
    );
    f.write(
        "src/Tokens/TokenService.php",
        concat!(
            "<?php\n",
            "namespace App\\Tokens;\n",
            "class TokenService {\n",
            "    public function revoke(Token $token) {\n",
            "        return $token->delete();\n",
            "    }\n",
            "}\n",
        ),
    );
    f.commit("two classes named Token with different parents");
    let r = f.trace(&["callers", "delete", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(caller_rows(&v, "src/Core/Entity.php::delete"), vec![row("src/Tokens/TokenService.php", 5, "INFERRED")]);
    assert_eq!(caller_rows(&v, "src/Mail/Audience.php::delete"), vec![row("src/Access/Token.php", 1, "EXTRACTED")]);
}

#[test]
fn pest_closure_this_is_the_test_case_its_pest_file_binds() {
    let f = Fixture::new();
    f.write("composer.json", r#"{"autoload-dev":{"psr-4":{"Tests\\":"tests/"}}}"#);
    f.write(
        "tests/Pest.php",
        "<?php\nuse Tests\\TestCase;\npest()\n    ->extends(TestCase::class)\n    ->afterEach(function () {})\n    ->in('Feature');\n",
    );
    f.write(
        "tests/TestCase.php",
        "<?php\nnamespace Tests;\nclass TestCase {\n    public function post(string $uri): Response { return new Response(); }\n}\n",
    );
    f.write(
        "tests/Response.php",
        "<?php\nnamespace Tests;\nclass Response {\n    public function json(): array { return []; }\n}\n",
    );
    f.write("tests/Other.php", "<?php\nnamespace Tests;\nclass Other {\n    public function json(): array { return []; }\n}\n");
    f.write(
        "tests/Feature/LoginTest.php",
        "<?php\nit('logs in', function () {\n    $this->post('/login')->json();\n    $response = $this->post('/logout');\n    $response->json();\n});\n",
    );
    f.write("tests/Unit/MathTest.php", "<?php\nit('adds', function () {\n    $this->post('/add')->json();\n});\n");
    f.commit("pest tests bound to a test case in one directory");
    let r = f.trace(&["callers", "json", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(
        caller_rows(&v, "tests/Response.php::json"),
        vec![
            row("tests/Feature/LoginTest.php", 3, "INFERRED"),
            row("tests/Feature/LoginTest.php", 5, "INFERRED"),
            row("tests/Unit/MathTest.php", 3, "AMBIGUOUS"),
        ]
    );
    assert_eq!(caller_rows(&v, "tests/Other.php::json"), vec![row("tests/Unit/MathTest.php", 3, "AMBIGUOUS")]);
}

#[test]
fn private_method_takes_no_caller_from_another_file() {
    let f = Fixture::new();
    f.write(
        "type.php",
        concat!(
            "<?php\n",
            "class Type {\n",
            "    private static function where($x) { return $x; }\n",
            "    public function scope() { return self::where(1); }\n",
            "}\n",
        ),
    );
    f.write("query.php", "<?php\nfunction build($query) { return $query->where('a', 1); }\n");
    f.write("greets.php", "<?php\ntrait Greets {\n    private function greet() { return 'hi'; }\n}\n");
    f.write(
        "person.php",
        "<?php\nclass Person {\n    use Greets;\n    public function hello() { return $this->greet(); }\n}\n",
    );
    f.commit("private methods of a class and of a trait");
    let r = f.trace(&["callers", "where", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(references_of(&v, "type.php::where", 3), extracted("type.php", &[4]), "{v}");
    let r = f.trace(&["callers", "greet", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(
        references_of(&v, "greets.php::greet", 3),
        vec![("person.php".to_string(), 4, "INFERRED".to_string())],
        "{v}"
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
    let v = r.view();
    let rows = caller_rows(&v, "tgt.php::lonePhpUniqueName");
    assert!(
        rows.iter()
            .any(|(f, l, c)| f == "caller.php" && *l == 2 && c == "INFERRED"),
        "php INFERRED ref expected — got {:?}",
        rows
    );
}

#[test]
fn php_attribute_application_is_a_caller_of_the_attribute_class() {
    // `#[Encrypted]` names the attribute class at the site. A file in the
    // same namespace applies it with no `use` line, so the import graph
    // never sees it; the reference does, on the class, the method, and the
    // property that carry it.
    let f = Fixture::new();
    f.write(
        "Encrypted.php",
        "<?php\nnamespace App;\n#[\\Attribute]\nfinal class Encrypted {}\n",
    );
    f.write(
        "Ledger.php",
        concat!(
            "<?php\n",
            "namespace App;\n",
            "#[Encrypted(reason: 'whole record')]\n",
            "final class Ledger {\n",
            "  #[Encrypted]\n",
            "  public string $token = '';\n",
            "  #[Encrypted]\n",
            "  public function post(): void {}\n",
            "}\n",
        ),
    );
    f.commit("php attribute without import");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "Encrypted", "--json"]);
    r.ok();
    let v = r.view();
    let rows = caller_rows(&v, "Encrypted.php::Encrypted");
    for line in [3, 5, 7] {
        assert!(
            rows.iter()
                .any(|(f, l, c)| f == "Ledger.php" && *l == line && c == "INFERRED"),
            "Ledger.php:{line} applies #[Encrypted] and must be a caller — got {:?}",
            rows
        );
    }
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
    let v = r.view();
    let rows_php = caller_rows(&v, "controller.php::process");
    assert!(
        !rows_php.iter().any(|(f, _, _)| f == "account.ts"),
        "a TypeScript process() must NOT resolve onto the PHP process method — got {:?}",
        rows_php
    );
}

#[test]
fn relative_imports_resolve_from_each_importing_directory() {
    let f = Fixture::new();
    for package in ["one", "two"] {
        f.write(
            &format!("packages/{package}/helpers.ts"),
            "export function parentHelper(): number { return 1; }\n",
        );
        f.write(
            &format!("packages/{package}/feature/helpers.ts"),
            "export function localHelper(): number { return 2; }\n",
        );
        f.write(
            &format!("packages/{package}/feature/app.ts"),
            concat!(
                "import { localHelper } from './helpers';\n",
                "import { parentHelper } from '../helpers';\n",
                "export function useHelpers(): number {\n",
                "  return localHelper() + parentHelper();\n",
                "}\n",
            ),
        );
    }
    f.commit("relative imports in two packages");
    f.trace(&["cache", "build", "."]).ok();

    for (name, target) in [
        ("localHelper", "feature/helpers.ts"),
        ("parentHelper", "helpers.ts"),
    ] {
        let r = f.trace(&["callers", name, "--json"]);
        r.ok();
        let v = r.view();
        for package in ["one", "two"] {
            let definition = format!("packages/{package}/{target}::{name}");
            let rows = caller_rows(&v, &definition);
            let expected = format!("packages/{package}/feature/app.ts");
            assert!(
                rows.iter().any(|(file, _, confidence)| {
                    file == &expected && confidence == "EXTRACTED"
                }),
                "{definition} did not resolve from its own importer directory: {rows:?}"
            );
            assert!(
                rows.iter().all(|(file, _, _)| file == &expected),
                "{definition} received a caller from the other package: {rows:?}"
            );
        }
    }
}

#[test]
fn ambiguous_suffix_imports_keep_every_compatible_candidate() {
    let f = Fixture::new();
    f.write(
        "left/helpers.ts",
        "export function sharedHelper(): number { return 1; }\n",
    );
    f.write(
        "right/helpers.ts",
        "export function sharedHelper(): number { return 2; }\n",
    );
    f.write(
        "app/main.ts",
        "import { sharedHelper } from 'helpers';\nexport const value = sharedHelper();\n",
    );
    f.commit("ambiguous suffix import");
    f.trace(&["cache", "build", "."]).ok();

    let r = f.trace(&["callers", "sharedHelper", "--json"]);
    r.ok();
    let v = r.view();
    for definition in [
        "left/helpers.ts::sharedHelper",
        "right/helpers.ts::sharedHelper",
    ] {
        let rows = caller_rows(&v, definition);
        assert!(
            rows.iter().any(|(file, _, confidence)| {
                file == "app/main.ts" && confidence == "AMBIGUOUS"
            }),
            "ambiguous import omitted {definition}: {rows:?}"
        );
    }
}

#[test]
fn ambiguous_imports_preserve_each_reference_site_candidate() {
    let f = Fixture::new();
    f.write(
        "left/helpers.ts",
        "export function sharedHelper(): number { return 1; }\n",
    );
    f.write(
        "right/helpers.ts",
        "export function sharedHelper(): number { return 2; }\n",
    );
    f.write(
        "app/main.ts",
        "import { sharedHelper } from 'helpers';\nexport function useHelper(): number {\n  return sharedHelper();\n}\n",
    );
    f.commit("ambiguous reference site");
    f.trace(&["cache", "build", "."]).ok();

    let r = f.trace(&["callers", "sharedHelper", "--json"]);
    r.ok();
    let v = r.view();
    for definition in [
        "left/helpers.ts::sharedHelper",
        "right/helpers.ts::sharedHelper",
    ] {
        let rows = caller_rows(&v, definition);
        assert!(
            rows.iter().any(|(file, line, confidence)| {
                file == "app/main.ts" && *line == 3 && confidence == "AMBIGUOUS"
            }),
            "ambiguous import omitted the actual call for {definition}: {rows:?}"
        );
    }
}

#[test]
fn a_name_declared_beyond_one_resolve_chunk_keeps_every_candidate() {
    let f = Fixture::new();
    for index in 0..513 {
        f.write(
            &format!("declarations/{index:03}.ts"),
            &format!("export class Type{index} {{\n  save(): number {{ return {index}; }}\n}}\n"),
        );
    }
    f.write(
        "caller.ts",
        "export function persist(value: any): number {\n  return value.save();\n}\n",
    );
    f.commit("common declaration beyond one resolve chunk");
    f.trace(&["cache", "build", "."]).ok();

    let definitions = f.trace(&["defines", "save", "--json"]);
    definitions.ok();
    let definitions = definitions.view();
    assert_eq!(definitions["definitions"].as_i64(), Some(513));
    let files = def_files(&definitions);
    assert_eq!(files.len(), 513);
    assert_eq!(files.first(), Some(&("declarations/000.ts".to_string(), 2)));
    assert_eq!(files.last(), Some(&("declarations/512.ts".to_string(), 2)));

    let callers = f.trace(&["callers", "save", "--limit", "600", "--json"]);
    callers.ok();
    let callers = callers.view();
    assert_eq!(callers["symbols"].as_i64(), Some(513));
    assert_eq!(callers["callers"].as_i64(), Some(513));
    assert_eq!(callers["total"].as_i64(), Some(513));
    assert_eq!(callers["truncated"].as_bool(), Some(false));
    for index in 0..513 {
        let node_id = format!("declarations/{index:03}.ts::save");
        assert_eq!(
            caller_rows(&callers, &node_id),
            vec![("caller.ts".to_string(), 2, "AMBIGUOUS".to_string())],
            "candidate {node_id} lost or changed its use site"
        );
    }
}

#[test]
fn typescript_named_import_resolves_the_declared_module() {
    let f = Fixture::new();
    f.write(
        "helpers.ts",
        "export function sharedHelper(): number { return 1; }\n",
    );
    f.write(
        "helpers/sharedHelper.ts",
        "export function sharedHelper(): number { return 2; }\n",
    );
    f.write(
        "app.ts",
        "import { sharedHelper } from './helpers';\nexport function useHelper(): number {\n  return sharedHelper();\n}\n",
    );
    f.commit("module and symbol path collision");
    f.trace(&["cache", "build", "."]).ok();

    let r = f.trace(&["callers", "sharedHelper", "--json"]);
    r.ok();
    let v = r.view();
    let module_rows = caller_rows(&v, "helpers.ts::sharedHelper");
    assert!(
        module_rows.iter().any(|(file, line, confidence)| {
            file == "app.ts" && *line == 3 && confidence == "EXTRACTED"
        }),
        "the declared from-module did not receive the call: {module_rows:?}"
    );
    let symbol_path_rows = caller_rows(&v, "helpers/sharedHelper.ts::sharedHelper");
    assert!(
        symbol_path_rows.iter().all(|(file, _, _)| file != "app.ts"),
        "the named symbol path stole the declared module import: {symbol_path_rows:?}"
    );
}

#[test]
fn relative_imports_cannot_escape_the_repository_root() {
    let f = Fixture::new();
    f.write(
        "helper.ts",
        "export function escapedHelper(): number { return 1; }\n",
    );
    f.write(
        "app.ts",
        "import { escapedHelper } from '../helper';\nexport const value = escapedHelper();\n",
    );
    f.commit("root escape import");
    f.trace(&["cache", "build", "."]).ok();

    let r = f.trace(&["callers", "escapedHelper", "--json"]);
    r.ok();
    let rows = caller_rows(&r.view(), "helper.ts::escapedHelper");
    assert!(
        rows.iter().all(|(file, _, _)| file != "app.ts"),
        "an import above the repository root clamped onto helper.ts: {rows:?}"
    );
}

#[test]
fn invalid_typescript_imports_suppress_only_their_local_bindings() {
    let f = Fixture::new();
    f.write(
        "local.ts",
        "export function local(): number { return 1; }\n",
    );
    f.write(
        "remote.ts",
        "export function remote(): number { return 2; }\n",
    );
    f.write(
        "fallback.ts",
        "export function fallback(): number { return 3; }\n",
    );
    f.write(
        "worker.ts",
        "export class Worker { execute(): number { return 4; } }\n",
    );
    f.write(
        "app/main.ts",
        "import { remote as local } from '../../outside';\nimport fallback from '../../outside';\nimport * as library from '../../outside';\nexport function useImports(): number {\n  local();\n  remote();\n  fallback();\n  return library.execute();\n}\n",
    );
    f.commit("invalid TypeScript bindings");
    f.trace(&["cache", "build", "."]).ok();

    let local = f.trace(&["callers", "local", "--json"]);
    local.ok();
    assert_eq!(caller_rows(&local.view(), "local.ts::local"), Vec::new());

    let remote = f.trace(&["callers", "remote", "--json"]);
    remote.ok();
    assert_eq!(
        caller_rows(&remote.view(), "remote.ts::remote"),
        vec![("app/main.ts".to_string(), 6, "INFERRED".to_string())]
    );

    let fallback = f.trace(&["callers", "fallback", "--json"]);
    fallback.ok();
    assert_eq!(
        caller_rows(&fallback.view(), "fallback.ts::fallback"),
        Vec::new()
    );

    let execute = f.trace(&["callers", "execute", "--json"]);
    execute.ok();
    assert_eq!(
        caller_rows(&execute.view(), "worker.ts::execute"),
        Vec::new()
    );
}

#[test]
fn invalid_named_binding_does_not_suppress_an_unrelated_member_property() {
    let f = Fixture::new();
    f.write(
        "worker.ts",
        "export class Worker { local(): number { return 1; } }\n",
    );
    f.write(
        "app/main.ts",
        "import { remote as local } from '../../outside';\nexport function use(object: any): number {\n  return object.local();\n}\n",
    );
    f.commit("member property matches invalid binding");
    f.trace(&["cache", "build", "."]).ok();

    let callers = f.trace(&["callers", "local", "--json"]);
    callers.ok();
    assert_eq!(
        caller_rows(&callers.view(), "worker.ts::local"),
        vec![("app/main.ts".to_string(), 3, "AMBIGUOUS".to_string())]
    );
}

#[test]
fn unrelated_same_file_method_does_not_exempt_an_invalid_namespace_binding() {
    let f = Fixture::new();
    f.write(
        "worker.ts",
        "export class Worker { execute(): number { return 1; } }\n",
    );
    f.write(
        "app/main.ts",
        "import * as library from '../../outside';\nclass Marker { library(): number { return 0; } }\nexport function use(): number {\n  return library.execute();\n}\n",
    );
    f.commit("method name matches invalid namespace binding");
    f.trace(&["cache", "build", "."]).ok();

    let callers = f.trace(&["callers", "execute", "--json"]);
    callers.ok();
    assert_eq!(
        caller_rows(&callers.view(), "worker.ts::execute"),
        Vec::new()
    );
}

#[test]
fn invalid_binding_suppresses_only_a_call_of_its_exact_name() {
    let f = Fixture::new();
    f.write(
        "local.ts",
        "export function local(): number { return 1; }\n",
    );
    f.write(
        "app/main.ts",
        "import { Local } from '../../outside';\nexport function use(): number {\n  return local();\n}\n",
    );
    f.commit("invalid binding differs in case from the call");
    f.trace(&["cache", "build", "."]).ok();

    let callers = f.trace(&["callers", "local", "--json"]);
    callers.ok();
    assert_eq!(
        caller_rows(&callers.view(), "local.ts::local"),
        vec![("app/main.ts".to_string(), 3, "INFERRED".to_string())]
    );
}

#[test]
fn import_symbol_fallback_never_crosses_languages() {
    let f = Fixture::new();
    f.write("helpers.py", "def shared_helper():\n    return 1\n");
    f.write(
        "app.ts",
        "import { shared_helper } from 'missing';\nexport const value = shared_helper();\n",
    );
    f.commit("cross-language import homonym");
    f.trace(&["cache", "build", "."]).ok();

    let r = f.trace(&["callers", "shared_helper", "--json"]);
    r.ok();
    let rows = caller_rows(&r.view(), "helpers.py::shared_helper");
    assert!(
        rows.iter().all(|(file, _, _)| file != "app.ts"),
        "a TypeScript import resolved to a Python homonym: {rows:?}"
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
    let v = r.view();
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
    let v = r.view();
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
    let r = f.trace(&["usages", "--path", ".", "--json"]);
    r.ok();
    let v = r.view();
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
    let v = r.view();
    let entry = &symbol(&v, "util.py::helper");
    let ids: Vec<String> = entry["callers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["node_id"].as_str().unwrap_or("").to_string())
        .collect();
    assert!(
        ids.contains(&"app.py::first".to_string()) && ids.contains(&"app.py::second".to_string()),
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
    // A caller row carries the calling declaration's source header. Here
    // `first` calls `helper`, so `helper`'s caller row exposes `first`.
    let f = Fixture::new();
    f.write(
        "util.ts",
        "export function helper(x: number): number { return x; }\n",
    );
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
    let v = r.view();
    let row = symbol(&v, "util.ts::helper")["callers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["label"].as_str() == Some("first"))
        .cloned()
        .unwrap_or_else(|| panic!("missing `first` caller row: {}", v));
    assert!(
        row["declaration"]["header"] == "export function first(a: string): number { … }",
        "caller row must carry the calling declaration header, got {row}"
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
    let v = r.view();
    // Across both definition entries, every row is AMBIGUOUS and the
    // per-symbol counts agree. The ordering invariant: within any entry, no
    // resolved row appears after an ambiguous one.
    for key in ["user.ts::save", "post.ts::save"] {
        let entry = symbol(&v, key);
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

#[test]
fn constructor_callers_count_and_limit_every_row_they_return() {
    let f = Fixture::new();
    f.write(
        "src/Thing.php",
        "<?php\nnamespace App;\n\nclass Thing\n{\n    public function __construct()\n    {\n    }\n}\n",
    );
    f.write("src/a.php", "<?php\nuse App\\Thing;\n\nfunction a() { return new Thing(); }\n");
    f.write("src/b.php", "<?php\nuse App\\Thing;\n\nfunction b() { return new Thing(); }\n");
    f.commit("one constructor, two importers");
    f.trace(&["cache", "build", "."]).ok();

    let whole = f.trace(&["callers", "__construct", "--json"]);
    whole.ok();
    let v = whole.view();
    assert_eq!(caller_rows(&v, "src/Thing.php::__construct").len(), 2, "{}", whole.stdout);
    assert_eq!(v["callers"].as_i64(), Some(2), "{}", whole.stdout);
    assert_eq!(v["total"].as_i64(), Some(2), "{}", whole.stdout);
    assert_eq!(v["truncated"], false, "{}", whole.stdout);

    let cut = f.trace(&["callers", "__construct", "--limit", "1", "--json"]);
    cut.ok();
    let c = cut.view();
    assert_eq!(caller_rows(&c, "src/Thing.php::__construct").len(), 1, "{}", cut.stdout);
    assert_eq!(c["callers"].as_i64(), Some(1), "{}", cut.stdout);
    assert_eq!(c["total"].as_i64(), Some(2), "{}", cut.stdout);
    assert_eq!(c["truncated"], true, "{}", cut.stdout);
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
    assert_eq!(r.view()["definitions"].as_i64().unwrap(), 1);
    // Method on the impl — found by the full declaration index.
    let r = f.trace(&["defines", "add_item", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(v["definitions"].as_i64().unwrap(), 1);
    assert_eq!(def_files(&v), vec![("lib.rs".to_string(), 4)]);
}

#[test]
fn rust_callers_resolve_to_calling_function() {
    // `helper` is defined in util.rs and called by `first`/`second` in
    // app.rs. The caller rows are the calling FUNCTIONS at their use-site
    // lines — the cross-file, function-granular Rust contract.
    let f = Fixture::new();
    f.write(
        "util.rs",
        "pub fn helper(x: i32) -> i32 {\n    x + 1\n}\n\npub struct Reader;\n\nimpl Reader {\n    pub fn open() {}\n}\n",
    );
    f.write(
        "app.rs",
        concat!(
            "use crate::util::{helper, Reader};\n",
            "pub fn first() -> i32 {\n",
            "    helper(1)\n",
            "}\n",
            "pub fn second() -> i32 {\n",
            "    helper(2)\n",
            "}\n",
            "pub fn associated() {\n",
            "    Reader::open();\n",
            "}\n",
            "pub fn outer() {\n",
            "    fn nested() {\n",
            "        helper(3);\n",
            "    }\n",
            "    nested();\n",
            "}\n",
        ),
    );
    f.commit("rust callers");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "helper", "--json"]);
    r.ok();
    let v = r.view();
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
    let ids: Vec<String> = symbol(&v, "util.rs::helper")["callers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["node_id"].as_str().unwrap_or("").to_string())
        .collect();
    assert!(
        ids.contains(&"app.rs::first".to_string()) && ids.contains(&"app.rs::second".to_string()),
        "Rust caller sources must be the calling functions: {:?}",
        ids
    );
    assert!(
        ids.contains(&"app.rs::nested".to_string()),
        "nested free call must resolve to its nested function: {:?}",
        ids
    );

    let r = f.trace(&["callers", "open", "--json"]);
    r.ok();
    let v = r.view();
    let rows = caller_rows(&v, "util.rs::open");
    assert!(
        rows.iter()
            .any(|(file, line, _)| file == "app.rs" && *line == 9),
        "Reader::open() at app.rs:9 must resolve: {:?}",
        rows
    );
    let ids: Vec<String> = symbol(&v, "util.rs::open")["callers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|caller| caller["node_id"].as_str().unwrap_or("").to_string())
        .collect();
    assert!(
        ids.contains(&"app.rs::associated".to_string()),
        "associated call source must be associated: {:?}",
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
    let v = r.view();
    let ids: Vec<String> = symbol(&v, "model.rs::Cart")["callers"]
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
fn a_call_from_another_function_in_the_same_file_is_a_caller_and_recursion_is_not() {
    let f = Fixture::new();
    f.write(
        "jobs.py",
        "def countdown(n):\n    return countdown(n - 1)\n\ndef run():\n    return countdown(3)\n",
    );
    f.commit("same-file call");
    let r = f.trace(&["callers", "countdown", "--json"]);
    r.ok();
    assert_eq!(
        caller_rows(&r.view(), "jobs.py::countdown"),
        vec![("jobs.py".to_string(), 5, "EXTRACTED".to_string())]
    );
}

#[test]
fn a_method_that_constructs_its_own_class_is_a_caller_of_it() {
    let f = Fixture::new();
    f.write(
        "node.py",
        "class Node:\n    def clone(self):\n        return Node()\n\ndef build():\n    return Node()\n",
    );
    f.commit("self construction");
    let r = f.trace(&["callers", "Node", "--json"]);
    r.ok();
    assert_eq!(
        caller_rows(&r.view(), "node.py::Node"),
        vec![
            ("node.py".to_string(), 3, "EXTRACTED".to_string()),
            ("node.py".to_string(), 6, "EXTRACTED".to_string()),
        ]
    );
}

fn twin_rust_crates(f: &Fixture) {
    f.write("alpha/Cargo.toml", "[package]\nname = \"alpha\"\nversion = \"0.1.0\"\n");
    f.write("alpha/src/main.rs", "mod read;\nmod summary;\nfn main() {}\n");
    f.write("alpha/src/summary.rs", "pub fn front_matter() -> String {\n    String::new()\n}\n");
    f.write("beta/Cargo.toml", "[package]\nname = \"beta\"\nversion = \"0.1.0\"\n");
    f.write("beta/src/lib.rs", "pub mod summary;\n");
    f.write("beta/src/summary.rs", "pub fn front_matter() -> String {\n    String::new()\n}\n");
}

#[test]
fn rust_free_call_resolves_to_the_declaration_its_crate_imports() {
    let f = Fixture::new();
    twin_rust_crates(&f);
    f.write(
        "alpha/src/read.rs",
        "use crate::summary::front_matter;\npub fn render() -> String {\n    front_matter()\n}\n",
    );
    f.commit("twin crates, free call");
    let r = f.trace(&["callers", "front_matter", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(
        caller_rows(&v, "alpha/src/summary.rs::front_matter"),
        vec![("alpha/src/read.rs".to_string(), 3, "EXTRACTED".to_string())],
        "{v}"
    );
    let beta = caller_rows(&v, "beta/src/summary.rs::front_matter");
    assert!(!beta.iter().any(|(file, _, _)| file.starts_with("alpha/")), "{beta:?}");
}

#[test]
fn rust_module_path_call_resolves_inside_its_own_crate() {
    let f = Fixture::new();
    twin_rust_crates(&f);
    f.write(
        "alpha/src/read.rs",
        concat!(
            "use crate::summary;\n",
            "pub fn render() -> String {\n",
            "    summary::front_matter()\n",
            "}\n",
            "pub fn direct() -> String {\n",
            "    crate::summary::front_matter()\n",
            "}\n",
        ),
    );
    f.commit("twin crates, path calls");
    let r = f.trace(&["callers", "front_matter", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(
        caller_rows(&v, "alpha/src/summary.rs::front_matter"),
        vec![
            ("alpha/src/read.rs".to_string(), 3, "EXTRACTED".to_string()),
            ("alpha/src/read.rs".to_string(), 6, "EXTRACTED".to_string()),
        ],
        "{v}"
    );
    let beta = caller_rows(&v, "beta/src/summary.rs::front_matter");
    assert!(!beta.iter().any(|(file, _, _)| file.starts_with("alpha/")), "{beta:?}");
}

#[test]
fn rust_call_inside_macro_arguments_resolves() {
    let f = Fixture::new();
    twin_rust_crates(&f);
    f.write(
        "alpha/src/read.rs",
        concat!(
            "use crate::summary;\n",
            "pub fn render() -> String {\n",
            "    format!(\n",
            "        \"{}\",\n",
            "        summary::front_matter()\n",
            "    )\n",
            "}\n",
            "pub fn check() {\n",
            "    assert_eq!(vec![summary::front_matter()].len(), 1);\n",
            "}\n",
        ),
    );
    f.commit("calls inside macros");
    let r = f.trace(&["callers", "front_matter", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(
        caller_rows(&v, "alpha/src/summary.rs::front_matter"),
        vec![
            ("alpha/src/read.rs".to_string(), 5, "EXTRACTED".to_string()),
            ("alpha/src/read.rs".to_string(), 9, "EXTRACTED".to_string()),
        ],
        "{v}"
    );
}

#[test]
fn rust_member_call_on_an_unstated_receiver_is_ambiguous() {
    let f = Fixture::new();
    f.write(
        "editor.rs",
        "pub struct LineEditor {\n    buffer: String,\n}\nimpl LineEditor {\n    pub fn insert(&mut self, c: char) {\n        self.buffer.push(c);\n    }\n}\n",
    );
    f.write(
        "summary.rs",
        concat!(
            "use serde_json::Map;\n",
            "pub fn headline() -> Map<String, String> {\n",
            "    let mut map = Map::new();\n",
            "    map.insert(\"a\".into(), \"b\".into());\n",
            "    map\n",
            "}\n",
        ),
    );
    f.commit("external member call");
    let session = [("AGENT_SESSION_ID", "rust-member-call")];
    let read = f.trace_env(&["read", "summary.rs", "--lines", "3:5", "--json"], &session);
    read.ok();
    let calls = &read.json()["context"]["files"]["summary.rs"]["calls"];
    assert!(
        !calls.as_array().into_iter().flatten().any(|call| call["method"] == "LineEditor::insert"),
        "map.insert must not resolve onto the one in-repo insert: {calls}"
    );
    let r = f.trace(&["callers", "insert", "--json"]);
    r.ok();
    assert_eq!(
        caller_rows(&r.view(), "editor.rs::insert"),
        vec![("summary.rs".to_string(), 4, "AMBIGUOUS".to_string())]
    );
}

#[test]
fn rust_self_method_call_resolves_to_its_own_impl() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write("src/main.rs", "mod bar;\nmod foo;\nfn main() {}\n");
    f.write(
        "src/foo.rs",
        "pub struct Foo;\nimpl Foo {\n    pub fn helper(&self) {}\n    pub fn run(&self) {\n        self.helper();\n        Self::build();\n    }\n    pub fn build() {}\n}\n",
    );
    f.write(
        "src/bar.rs",
        "pub struct Bar;\nimpl Bar {\n    pub fn helper(&self) {}\n    pub fn build() {}\n}\n",
    );
    f.commit("self calls");
    for (name, line) in [("helper", 5), ("build", 6)] {
        let r = f.trace(&["callers", name, "--json"]);
        r.ok();
        let v = r.view();
        assert_eq!(
            caller_rows(&v, &format!("src/foo.rs::{name}")),
            vec![("src/foo.rs".to_string(), line, "EXTRACTED".to_string())],
            "{v}"
        );
        let bar = caller_rows(&v, &format!("src/bar.rs::{name}"));
        assert!(!bar.iter().any(|(file, _, _)| file == "src/foo.rs"), "{bar:?}");
    }
}

#[test]
fn rust_member_call_on_a_typed_parameter_resolves_to_that_type() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write("src/main.rs", "mod journal;\nmod ledger;\nmod refund;\nfn main() {}\n");
    f.write(
        "src/ledger.rs",
        "pub struct Ledger;\nimpl Ledger {\n    pub fn record(&self, cents: i64) -> i64 {\n        cents\n    }\n}\n",
    );
    f.write(
        "src/journal.rs",
        "pub struct Journal;\nimpl Journal {\n    pub fn record(&self, cents: i64) -> i64 {\n        cents\n    }\n}\n",
    );
    f.write(
        "src/refund.rs",
        concat!(
            "use crate::ledger::Ledger;\n",
            "pub fn back(ledger: &Ledger, cents: i64) -> i64 {\n",
            "    ledger.record(-cents)\n",
            "}\n",
            "pub fn any<T: Copy>(book: &T, cents: i64) -> i64 {\n",
            "    book.record(cents)\n",
            "}\n",
        ),
    );
    f.commit("typed parameter");
    let r = f.trace(&["callers", "record", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(
        caller_rows(&v, "src/ledger.rs::record"),
        vec![
            ("src/refund.rs".to_string(), 3, "EXTRACTED".to_string()),
            ("src/refund.rs".to_string(), 6, "AMBIGUOUS".to_string()),
        ],
        "{v}"
    );
    assert_eq!(
        caller_rows(&v, "src/journal.rs::record"),
        vec![("src/refund.rs".to_string(), 6, "AMBIGUOUS".to_string())],
        "{v}"
    );
}

#[test]
fn rust_use_forms_resolve_inside_the_crate() {
    let f = Fixture::new();
    for root in ["app", "other"] {
        f.write(
            &format!("{root}/Cargo.toml"),
            &format!("[package]\nname = \"{root}\"\nversion = \"0.1.0\"\n"),
        );
        f.write(
            &format!("{root}/src/tools.rs"),
            "pub fn measure() -> i32 {\n    1\n}\npub fn parse<T>() -> i32 {\n    2\n}\n",
        );
        f.write(
            &format!("{root}/src/shapes.rs"),
            "pub fn area() -> i32 {\n    3\n}\npub fn edge() -> i32 {\n    4\n}\n",
        );
        f.write(&format!("{root}/src/units.rs"), "pub fn scale() -> i32 {\n    5\n}\n");
    }
    f.write("app/src/main.rs", "mod paint;\nmod shapes;\nmod tools;\nmod units;\nfn main() {}\n");
    f.write("other/src/lib.rs", "pub mod shapes;\npub mod tools;\npub mod units;\n");
    f.write(
        "app/src/paint.rs",
        concat!(
            "use crate::tools::*;\n",
            "use crate::shapes::{self, edge as border};\n",
            "pub fn draw() -> i32 {\n",
            "    measure()\n",
            "        + border()\n",
            "        + shapes::area()\n",
            "        + crate::tools::parse::<u8>()\n",
            "}\n",
            "mod tests {\n",
            "    use super::super::units::scale;\n",
            "    fn probe() -> i32 {\n",
            "        scale()\n",
            "    }\n",
            "}\n",
        ),
    );
    f.commit("use forms");
    for (target, lines) in [
        ("app/src/tools.rs::measure", vec![4]),
        ("app/src/units.rs::scale", vec![12]),
        ("app/src/shapes.rs::edge", vec![5]),
        ("app/src/shapes.rs::area", vec![6]),
        ("app/src/tools.rs::parse", vec![7]),
    ] {
        let name = target.rsplit("::").next().unwrap();
        let r = f.trace(&["callers", name, "--json"]);
        r.ok();
        let v = r.view();
        assert_eq!(
            caller_rows(&v, target),
            lines
                .iter()
                .map(|line| ("app/src/paint.rs".to_string(), *line, "EXTRACTED".to_string()))
                .collect::<Vec<_>>(),
            "{target}: {v}"
        );
        let twin = target.replacen("app/", "other/", 1);
        let other = caller_rows(&v, &twin);
        assert!(!other.iter().any(|(file, _, _)| file.starts_with("app/")), "{twin}: {other:?}");
    }
}

#[test]
fn rust_path_naming_another_crate_resolves_into_that_crate() {
    let f = Fixture::new();
    f.write(
        "core/Cargo.toml",
        "[package]\nname = \"ledger-core\"\nversion = \"0.1.0\"\n\n[lib]\npath = \"lib/core.rs\"\n",
    );
    f.write("core/lib/core.rs", "pub mod books;\npub fn open() -> i32 {\n    1\n}\n");
    f.write("core/lib/books.rs", "pub fn balance() -> i32 {\n    2\n}\n");
    f.write("other/Cargo.toml", "[package]\nname = \"other\"\nversion = \"0.1.0\"\n");
    f.write("other/src/lib.rs", "pub mod books;\npub fn open() -> i32 {\n    3\n}\n");
    f.write("other/src/books.rs", "pub fn balance() -> i32 {\n    4\n}\n");
    f.write(
        "app/Cargo.toml",
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\nledger-core = { path = \"../core\" }\n",
    );
    f.write(
        "app/src/main.rs",
        "use ledger_core::books;\nfn main() {\n    ledger_core::open();\n    books::balance();\n}\n",
    );
    f.commit("crates by name");
    for (target, line) in [("core/lib/core.rs::open", 3), ("core/lib/books.rs::balance", 4)] {
        let r = f.trace(&["callers", target.rsplit("::").next().unwrap(), "--json"]);
        r.ok();
        assert_eq!(
            caller_rows(&r.view(), target),
            vec![("app/src/main.rs".to_string(), line, "EXTRACTED".to_string())],
            "{target}"
        );
    }
}

#[test]
fn rust_use_of_an_external_crate_records_no_import_edge() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write(
        "src/main.rs",
        "mod json;\nmod store;\nuse crate::store::Store;\nuse serde_json::Map;\nfn main() {}\n",
    );
    f.write("src/json.rs", "pub struct Map;\n");
    f.write("src/store.rs", "pub struct Store;\n");
    f.commit("external use");
    assert_eq!(
        dependencies_of(&f, "src/main.rs"),
        vec![("src/store".to_string(), "EXTRACTED".to_string())]
    );
}

fn dependencies_of(f: &Fixture, file: &str) -> Vec<(String, String)> {
    let run = f.trace(&["info", file, "--json"]);
    run.ok();
    let mut dependencies: Vec<(String, String)> = run.json()["context"]["files"][file]["dependencies"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|row| {
            (
                row["module"].as_str().unwrap_or("").to_string(),
                row["confidence"].as_str().unwrap_or("").to_string(),
            )
        })
        .collect();
    dependencies.sort();
    dependencies
}

#[test]
fn rust_module_file_added_or_moved_reresolves_warmed_imports() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write(
        "src/main.rs",
        "mod store;\nmod tools;\nuse crate::store::Shelf;\nuse crate::tools::measure;\nfn main() {}\n",
    );
    f.write("src/tools.rs", "pub fn measure() {}\n");
    f.commit("store declared without a file");
    let tools = ("src/tools".to_string(), "EXTRACTED".to_string());
    assert_eq!(dependencies_of(&f, "src/main.rs"), vec![tools.clone()]);

    f.write("src/store/mod.rs", "pub struct Store;\n");
    f.commit("store as a directory");
    assert_eq!(
        dependencies_of(&f, "src/main.rs"),
        vec![("src/store/mod".to_string(), "EXTRACTED".to_string()), tools.clone()]
    );

    f.write("src/store.rs", "pub struct Store;\n");
    f.git(&["rm", "--quiet", "src/store/mod.rs"]);
    f.commit("store as a file");
    assert_eq!(
        dependencies_of(&f, "src/main.rs"),
        vec![("src/store".to_string(), "EXTRACTED".to_string()), tools]
    );
}

#[test]
fn rust_inline_test_module_reaches_its_own_file_without_importing_it() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write("src/main.rs", "mod shapes;\nfn main() {}\n");
    f.write(
        "src/shapes.rs",
        "pub fn area() -> i32 {\n    3\n}\n#[cfg(test)]\nmod tests {\n    use super::*;\n    fn check() -> i32 {\n        area()\n    }\n}\n",
    );
    f.commit("inline tests");
    assert_eq!(dependencies_of(&f, "src/shapes.rs"), vec![]);
    let r = f.trace(&["callers", "area", "--json"]);
    r.ok();
    assert_eq!(
        caller_rows(&r.view(), "src/shapes.rs::area"),
        vec![("src/shapes.rs".to_string(), 8, "EXTRACTED".to_string())]
    );
}

#[test]
fn rust_bare_call_reaches_an_inline_module_only_from_inside_it() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write("src/main.rs", "mod cli;\nmod engine;\nfn main() {}\n");
    f.write("src/engine.rs", "pub fn start() {}\n");
    f.write(
        "src/cli.rs",
        concat!(
            "use crate::engine::start;\n",
            "pub fn run() {\n",
            "    start();\n",
            "}\n",
            "#[cfg(test)]\n",
            "mod tests {\n",
            "    #[test]\n",
            "    fn start() {}\n",
            "    fn probe() {\n",
            "        start();\n",
            "    }\n",
            "}\n",
        ),
    );
    f.commit("an imported function beside a test of its name in an inline module");
    let r = f.trace(&["callers", "start", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(caller_rows(&v, "src/cli.rs::start"), extracted("src/cli.rs", &[10]), "{v}");
    assert_eq!(caller_rows(&v, "src/engine.rs::start"), extracted("src/cli.rs", &[3]), "{v}");
}

#[test]
fn rust_reexport_passes_an_inline_module_of_the_file_it_reaches() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write("src/main.rs", "mod cli;\nmod engine;\nmod other;\nfn main() {\n    cli::run();\n}\n");
    f.write("src/cli.rs", "use crate::engine::start;\npub fn run() {\n    start();\n}\n");
    f.write(
        "src/engine.rs",
        "pub use crate::other::start;\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn start() {}\n}\n",
    );
    f.write("src/other.rs", "pub fn start() {}\n");
    f.commit("a re-export beside a test of its name in an inline module");
    let r = f.trace(&["callers", "start", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(references_of(&v, "src/other.rs::start", 1), extracted("src/cli.rs", &[3]), "{v}");
    assert_eq!(references_of(&v, "src/engine.rs::start", 5), vec![], "{v}");
}

#[test]
fn rust_path_through_a_reexport_passes_an_inline_module_of_the_file_it_names() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write("src/main.rs", "fn main() {\n    engine::start();\n}\nmod engine;\nmod real;\n");
    f.write(
        "src/engine.rs",
        concat!(
            "pub use crate::real::start;\n",
            "#[cfg(test)]\n",
            "mod tests {\n",
            "    use super::*;\n",
            "    #[test]\n",
            "    fn start() {}\n",
            "}\n",
        ),
    );
    f.write("src/real.rs", "pub fn start() {}\n");
    f.commit("a path through a re-export beside a test of its name");
    let r = f.trace(&["callers", "start", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(references_of(&v, "src/real.rs::start", 1), extracted("src/main.rs", &[2]), "{v}");
    assert_eq!(references_of(&v, "src/engine.rs::start", 6), vec![], "{v}");
}

#[test]
fn rust_use_into_an_inline_module_reaches_only_that_module() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write("src/main.rs", "mod cli;\nmod engine;\nfn main() {\n    cli::run();\n}\n");
    f.write("src/cli.rs", "use crate::engine::imp::start;\npub fn run() {\n    start();\n}\n");
    f.write("src/engine.rs", "pub mod imp {\n    pub fn start() {}\n}\npub fn start() {}\n");
    f.commit("a use into an inline module beside a top-level namesake");
    let r = f.trace(&["callers", "start", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(references_of(&v, "src/engine.rs::start", 2), extracted("src/cli.rs", &[3]), "{v}");
    assert_eq!(references_of(&v, "src/engine.rs::start", 4), vec![], "{v}");
}

#[test]
fn rust_path_into_an_inline_module_of_another_file_reaches_only_that_module() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write("src/main.rs", "mod nested;\nfn main() {\n    nested::imp::start();\n}\n");
    f.write("src/nested.rs", "pub mod imp {\n    pub fn start() {}\n}\npub fn start() {}\n");
    f.commit("a path into another file's inline module beside a top-level namesake");
    let r = f.trace(&["callers", "start", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(references_of(&v, "src/nested.rs::start", 2), extracted("src/main.rs", &[3]), "{v}");
    assert_eq!(references_of(&v, "src/nested.rs::start", 4), vec![], "{v}");
}

#[test]
fn rust_type_path_never_follows_a_use_inside_a_function_body() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write("src/main.rs", "mod engine;\nmod other;\nfn main() {\n    engine::Thing::new();\n}\n");
    f.write("src/engine.rs", "pub fn helper() {\n    use crate::other::Thing;\n    Thing::new();\n}\n");
    f.write("src/other.rs", "pub struct Thing;\nimpl Thing {\n    pub fn new() -> Self {\n        Thing\n    }\n}\n");
    f.commit("a type named only by a use inside a function body");
    let r = f.trace(&["callers", "new", "--json"]);
    r.ok();
    let v = r.view();
    let mut expected = extracted("src/engine.rs", &[3]);
    expected.push(("src/main.rs".to_string(), 4, "INFERRED".to_string()));
    assert_eq!(caller_rows(&v, "src/other.rs::new"), expected, "{v}");
}

#[test]
fn rust_type_path_into_an_inline_module_never_follows_a_use_inside_a_function_body() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write("src/main.rs", "mod engine;\nmod other;\nfn main() {\n    engine::imp::Thing::new();\n}\n");
    f.write(
        "src/engine.rs",
        "pub mod imp {\n    pub fn helper() {\n        use crate::other::Thing;\n        Thing::new();\n    }\n}\n",
    );
    f.write("src/other.rs", "pub struct Thing;\nimpl Thing {\n    pub fn new() -> Self {\n        Thing\n    }\n}\n");
    f.commit("a type named only by a use inside a function body of an inline module");
    let r = f.trace(&["callers", "new", "--json"]);
    r.ok();
    let v = r.view();
    let mut expected = extracted("src/engine.rs", &[4]);
    expected.push(("src/main.rs".to_string(), 4, "INFERRED".to_string()));
    assert_eq!(caller_rows(&v, "src/other.rs::new"), expected, "{v}");
}

#[test]
fn rust_only_a_bare_name_reads_the_use_items_of_the_calling_function() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write(
        "src/main.rs",
        concat!(
            "mod a;\n",
            "mod b;\n",
            "mod c;\n",
            "pub(crate) use crate::c::bar;\n",
            "\n",
            "fn main() {\n",
            "    use crate::b::bar;\n",
            "    a::bar();\n",
            "    bar();\n",
            "    through_a();\n",
            "    from_root();\n",
            "}\n",
            "\n",
            "fn from_root() {\n",
            "    use crate::b::bar;\n",
            "    crate::bar();\n",
            "}\n",
            "\n",
            "fn through_a() {\n",
            "    use crate::a::bar;\n",
            "    bar();\n",
            "}\n",
        ),
    );
    f.write("src/a.rs", "pub(crate) use crate::bar;\n");
    f.write("src/b.rs", "pub fn bar() {}\n");
    f.write("src/c.rs", "pub fn bar() {}\n");
    f.commit("a crate path and re-exports back into the calling module beside a function's own use");
    let r = f.trace(&["callers", "bar", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(references_of(&v, "src/c.rs::bar", 1), extracted("src/main.rs", &[8, 16, 21]), "{v}");
    assert_eq!(references_of(&v, "src/b.rs::bar", 1), extracted("src/main.rs", &[9]), "{v}");
}

#[test]
fn rust_use_inside_a_function_body_hides_a_same_named_item_of_its_module() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write("src/main.rs", "mod b;\nfn bar() {}\nfn main() {\n    use crate::b::bar;\n    bar();\n}\n");
    f.write("src/b.rs", "pub fn bar() {}\n");
    f.commit("a function's own use beside a same-named item of its module");
    let r = f.trace(&["callers", "bar", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(references_of(&v, "src/b.rs::bar", 1), extracted("src/main.rs", &[5]), "{v}");
    assert_eq!(references_of(&v, "src/main.rs::bar", 2), vec![], "{v}");
}

#[test]
fn rust_glob_inside_a_function_body_shares_the_start_with_a_same_named_item_of_its_module() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write("src/main.rs", "mod b;\nfn bar() {}\nfn main() {\n    use crate::b::*;\n    bar();\n}\n");
    f.write("src/b.rs", "pub fn bar() {}\n");
    f.commit("a function's own glob beside a same-named item of its module");
    let r = f.trace(&["callers", "bar", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(references_of(&v, "src/b.rs::bar", 1), ambiguous("src/main.rs", &[5]), "{v}");
    assert_eq!(references_of(&v, "src/main.rs::bar", 2), ambiguous("src/main.rs", &[5]), "{v}");
}

#[test]
fn rust_glob_inside_a_function_body_that_brings_no_namesake_keeps_the_item_of_its_module() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write("src/main.rs", "mod b;\nfn bar() {}\nfn main() {\n    use crate::b::*;\n    bar();\n}\n");
    f.write("src/b.rs", "pub fn other() {}\n");
    f.commit("a function's own glob into a module with no namesake");
    let r = f.trace(&["callers", "bar", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(references_of(&v, "src/main.rs::bar", 2), extracted("src/main.rs", &[5]), "{v}");
}

#[test]
fn rust_use_of_a_module_inside_a_function_body_hides_no_function_of_its_module() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write(
        "src/main.rs",
        "mod util;\nfn helpers() {}\nfn run() {\n    use crate::util::helpers;\n    helpers();\n}\nfn main() {\n    run();\n}\n",
    );
    f.write("src/util.rs", "pub mod helpers;\n");
    f.write("src/util/helpers.rs", "pub fn helpers() {}\n");
    f.commit("a function's own use of a module beside a same-named function of its module");
    let r = f.trace(&["callers", "helpers", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(references_of(&v, "src/main.rs::helpers", 2), extracted("src/main.rs", &[5]), "{v}");
    assert_eq!(references_of(&v, "src/util/helpers.rs::helpers", 1), vec![], "{v}");
}

#[test]
fn rust_crate_path_to_an_external_use_never_reads_the_use_items_of_the_calling_function() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write(
        "src/main.rs",
        "mod other;\nuse std::process::exit;\nfn main() {\n    use crate::other::exit;\n    crate::exit(0);\n}\n",
    );
    f.write("src/other.rs", "pub fn exit(_: i32) {}\n");
    f.commit("a crate path to a standard library use beside a function's own use");
    let r = f.trace(&["callers", "exit", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(references_of(&v, "src/other.rs::exit", 1), vec![], "{v}");
}

#[test]
fn rust_type_path_into_an_inline_module_of_the_calling_file_never_follows_a_use_inside_a_function_body() {
    let f = Fixture::new();
    f.write(
        "Cargo.toml",
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\next = \"1\"\n",
    );
    f.write(
        "src/main.rs",
        concat!(
            "mod other;\n",
            "mod inner {\n",
            "    pub use ext::Thing;\n",
            "}\n",
            "fn main() {\n",
            "    inner::Thing::new();\n",
            "}\n",
            "fn build() {\n",
            "    use crate::other::Thing;\n",
            "    Thing::new();\n",
            "}\n",
        ),
    );
    f.write("src/other.rs", "pub struct Thing;\nimpl Thing {\n    pub fn new() -> Self {\n        Thing\n    }\n}\n");
    f.commit("a type an inline module re-exports from a dependency beside a function's own use");
    let r = f.trace(&["callers", "new", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(caller_rows(&v, "src/other.rs::new"), extracted("src/main.rs", &[10]), "{v}");
}

#[test]
fn rust_glob_over_a_type_follows_no_use_of_its_file() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write(
        "src/main.rs",
        "mod engine;\nuse std::cmp::*;\nuse engine::Kind::*;\nfn main() {\n    let _ = A;\n    max(1, 2);\n}\n",
    );
    f.write(
        "src/engine.rs",
        concat!(
            "pub enum Kind { A }\n",
            "fn max(a: i32, b: i32) -> i32 { if a > b { a } else { b } }\n",
            "#[cfg(test)]\n",
            "mod tests {\n",
            "    use super::*;\n",
            "}\n",
        ),
    );
    f.commit("a glob over an enum beside a glob over the standard library");
    let r = f.trace(&["callers", "max", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(references_of(&v, "src/engine.rs::max", 2), vec![], "{v}");
}

#[test]
fn rust_glob_into_an_inline_module_follows_no_use_of_a_sibling_module() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write(
        "src/main.rs",
        "mod engine;\nuse std::cmp::*;\nuse engine::imp::*;\nfn main() {\n    go();\n    max(1, 2);\n}\n",
    );
    f.write(
        "src/engine.rs",
        concat!(
            "pub mod imp { pub fn go() {} }\n",
            "fn max(a: i32, b: i32) -> i32 { if a > b { a } else { b } }\n",
            "#[cfg(test)]\n",
            "mod tests {\n",
            "    use super::*;\n",
            "}\n",
        ),
    );
    f.commit("a glob into an inline module beside a test module that globs its parent");
    let r = f.trace(&["callers", "max", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(references_of(&v, "src/engine.rs::max", 2), vec![], "{v}");
    let r = f.trace(&["callers", "go", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(references_of(&v, "src/engine.rs::go", 1), extracted("src/main.rs", &[5]), "{v}");
}

#[test]
fn rust_use_in_an_inline_module_reads_no_glob_of_a_sibling_module() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write("src/main.rs", "mod engine;\nmod other;\nfn main() {\n    engine::imp::go();\n}\n");
    f.write(
        "src/engine.rs",
        concat!(
            "pub mod imp {\n",
            "    pub use tools::go;\n",
            "}\n",
            "mod side {\n",
            "    use crate::other::*;\n",
            "}\n",
        ),
    );
    f.write("src/other.rs", "pub mod tools;\n");
    f.write("src/other/tools.rs", "pub fn go() {}\n");
    f.commit("a use whose first segment only a sibling module's glob brings in");
    let r = f.trace(&["callers", "go", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(references_of(&v, "src/other/tools.rs::go", 1), vec![], "{v}");
}

#[test]
fn rust_path_into_an_inline_module_of_the_calling_file_follows_its_use_items() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write(
        "src/main.rs",
        concat!(
            "mod engine;\n",
            "mod other;\n",
            "mod relay {\n",
            "    pub use crate::other::Gadget;\n",
            "}\n",
            "fn main() {\n",
            "    crate::relay::Gadget::build();\n",
            "    relay::Gadget::build();\n",
            "    engine::Thing::new();\n",
            "    engine::sub::Thing::new();\n",
            "    engine::relay::Gadget::build();\n",
            "}\n",
        ),
    );
    f.write(
        "src/engine.rs",
        concat!(
            "mod inner {\n",
            "    pub use crate::other::Thing;\n",
            "}\n",
            "pub use inner::*;\n",
            "pub mod relay {\n",
            "    pub use crate::other::Gadget;\n",
            "}\n",
            "pub mod sub {\n",
            "    pub use super::inner::Thing;\n",
            "}\n",
        ),
    );
    f.write(
        "src/other.rs",
        concat!(
            "pub struct Thing;\n",
            "impl Thing {\n",
            "    pub fn new() -> Self {\n",
            "        Thing\n",
            "    }\n",
            "}\n",
            "pub struct Gadget;\n",
            "impl Gadget {\n",
            "    pub fn build() -> Self {\n",
            "        Gadget\n",
            "    }\n",
            "}\n",
        ),
    );
    f.commit("re-exports inside inline modules of the calling file and of another file");
    let r = f.trace(&["callers", "build", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(caller_rows(&v, "src/other.rs::build"), extracted("src/main.rs", &[7, 8, 11]), "{v}");
    let r = f.trace(&["callers", "new", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(caller_rows(&v, "src/other.rs::new"), extracted("src/main.rs", &[9, 10]), "{v}");
}

#[test]
fn rust_path_into_an_inline_module_that_only_reexports_reaches_the_declaring_file() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write("src/main.rs", "mod other;\nmod relay;\nfn main() {\n    relay::imp::go();\n}\n");
    f.write("src/relay.rs", "pub mod imp {\n    pub use crate::other::go;\n}\n");
    f.write("src/other.rs", "pub fn go() {}\n");
    f.commit("a path into an inline module that re-exports its target");
    let r = f.trace(&["callers", "go", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(references_of(&v, "src/other.rs::go", 1), extracted("src/main.rs", &[4]), "{v}");
}

#[test]
fn rust_name_rule_reaches_an_inline_module_only_through_inline_modules_its_file_declares() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write("src/main.rs", "mod engine;\nfn main() {\n    engine::missing::imp::go();\n}\n");
    f.write(
        "src/engine.rs",
        "use nowhere::*;\nmod missing;\npub mod imp {\n    pub fn go() {}\n}\n",
    );
    f.commit("a path through a module with no file beside a namesake inline module");
    let r = f.trace(&["callers", "go", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(references_of(&v, "src/engine.rs::go", 4), vec![], "{v}");
}

#[test]
fn rust_name_rule_reaches_only_the_top_level_of_a_file_its_path_names() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write("src/main.rs", "mod engine;\nfn main() {\n    engine::go();\n}\n");
    f.write("src/engine.rs", "use nowhere::*;\n#[cfg(test)]\nmod tests {\n    pub fn go() {}\n}\n");
    f.commit("a path to a file whose only namesake sits in its test module");
    let r = f.trace(&["callers", "go", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(references_of(&v, "src/engine.rs::go", 4), vec![], "{v}");
}

/// The reference rows of the declaration `node_id` names at `line`.
fn references_of(v: &serde_json::Value, node_id: &str, line: i64) -> Vec<(String, i64, String)> {
    v["results"]
        .as_array()
        .unwrap_or_else(|| panic!("results must be a row list: {v}"))
        .iter()
        .find(|row| row["node_id"] == node_id && row["source_line"] == line)
        .unwrap_or_else(|| panic!("no result row for {node_id} at line {line}: {v}"))["callers"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|caller| caller["relation"] == "references")
        .map(|caller| {
            (
                caller["source_file"].as_str().unwrap_or("").to_string(),
                caller["source_line"].as_i64().unwrap_or(0),
                caller["confidence"].as_str().unwrap_or("").to_string(),
            )
        })
        .collect()
}

fn extracted(file: &str, lines: &[i64]) -> Vec<(String, i64, String)> {
    lines
        .iter()
        .map(|line| (file.to_string(), *line, "EXTRACTED".to_string()))
        .collect()
}

#[test]
fn rust_call_through_a_reexport_or_a_glob_reaches_its_declaring_file() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write(
        "src/lib.rs",
        "pub mod glob;\npub mod thing;\npub mod util;\npub use thing::helper;\npub use thing::Thing;\n",
    );
    let thing = "pub struct Thing;\nimpl Thing {\n    pub fn build() -> Thing {\n        Thing\n    }\n}\npub fn helper() {}\n";
    f.write("src/thing.rs", thing);
    f.write(
        "src/util.rs",
        "use crate::helper;\nuse crate::Thing;\npub fn make() -> Thing {\n    helper();\n    Thing::build()\n}\n",
    );
    f.write(
        "src/glob.rs",
        "use crate::thing::*;\npub fn globbed() -> Thing {\n    helper();\n    Thing::build()\n}\n",
    );
    f.write("other/Cargo.toml", "[package]\nname = \"other\"\nversion = \"0.1.0\"\n");
    f.write("other/src/lib.rs", thing);
    f.commit("re-exports and globs");
    for (name, glob_line, util_line) in [("build", 4, 5), ("helper", 3, 4)] {
        let r = f.trace(&["callers", name, "--json"]);
        r.ok();
        let v = r.view();
        let mut expected = extracted("src/glob.rs", &[glob_line]);
        expected.extend(extracted("src/util.rs", &[util_line]));
        assert_eq!(caller_rows(&v, &format!("src/thing.rs::{name}")), expected, "{v}");
        let twin = caller_rows(&v, &format!("other/src/lib.rs::{name}"));
        assert!(!twin.iter().any(|(file, _, _)| file.starts_with("src/")), "{twin:?}");
    }
}

#[test]
fn rust_same_named_methods_in_one_file_take_the_caller_that_holds_the_call() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write("src/lib.rs", "pub mod homonyms;\n");
    f.write(
        "src/homonyms.rs",
        concat!(
            "pub struct B;\n",
            "impl B {\n",
            "    pub fn new() -> B {\n",
            "        B\n",
            "    }\n",
            "}\n",
            "pub struct A;\n",
            "impl A {\n",
            "    pub fn new() -> A {\n",
            "        let _b = B::new();\n",
            "        A\n",
            "    }\n",
            "}\n",
        ),
    );
    f.commit("same-named methods");
    let r = f.trace(&["callers", "new", "--json"]);
    r.ok();
    let v = r.view();
    let callers_of = |line: i64| -> Vec<(i64, i64)> {
        v["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["node_id"] == "src/homonyms.rs::new" && row["source_line"] == line)
            .unwrap_or_else(|| panic!("no new at line {line}: {v}"))["callers"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|caller| caller["relation"] == "references")
            .map(|caller| {
                (
                    caller["source_line"].as_i64().unwrap(),
                    caller["declaration"]["line"].as_i64().unwrap_or(0),
                )
            })
            .collect()
    };
    assert_eq!(callers_of(3), vec![(10, 9)], "B::new is called from A::new: {v}");
    assert_eq!(callers_of(9), vec![], "A::new has no caller: {v}");
}

#[test]
fn rust_shadowed_parameter_states_no_receiver_type() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write("src/lib.rs", "pub mod shadow;\n");
    f.write(
        "src/shadow.rs",
        concat!(
            "pub struct Raw;\n",
            "pub struct Ready;\n",
            "impl Raw {\n",
            "    pub fn finish(self) -> Ready {\n",
            "        Ready\n",
            "    }\n",
            "    pub fn apply(&self) {}\n",
            "}\n",
            "impl Ready {\n",
            "    pub fn apply(&self) {}\n",
            "}\n",
            "pub fn plain(options: Raw) {\n",
            "    options.apply();\n",
            "}\n",
            "pub fn rebound(options: Raw) {\n",
            "    let options = options.finish();\n",
            "    options.apply();\n",
            "}\n",
            "pub fn looped(options: Raw, all: Vec<Ready>) {\n",
            "    for options in all {\n",
            "        options.apply();\n",
            "    }\n",
            "}\n",
            "pub fn unwrapped(options: Raw, next: Option<Ready>) {\n",
            "    if let Some(options) = next {\n",
            "        options.apply();\n",
            "    }\n",
            "}\n",
            "pub fn drained(options: Raw, mut all: Vec<Ready>) {\n",
            "    while let Some(options) = all.pop() {\n",
            "        options.apply();\n",
            "    }\n",
            "}\n",
            "pub fn matched(options: Raw, next: Option<Ready>) {\n",
            "    match next {\n",
            "        Some(options) => options.apply(),\n",
            "        None => {}\n",
            "    }\n",
            "}\n",
        ),
    );
    f.commit("shadowed parameters");
    let shadowed = |file: &str| -> Vec<(String, i64, String)> {
        [17, 21, 26, 31, 36]
            .iter()
            .map(|line| (file.to_string(), *line, "AMBIGUOUS".to_string()))
            .collect()
    };
    let r = f.trace(&["callers", "apply", "--json"]);
    r.ok();
    let v = r.view();
    let mut raw = extracted("src/shadow.rs", &[13]);
    raw.extend(shadowed("src/shadow.rs"));
    assert_eq!(caller_rows(&v, "src/shadow.rs::apply"), raw, "{v}");
    let ready: Vec<(String, i64, String)> = v["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["source_line"] == 10)
        .unwrap()["callers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|caller| {
            (
                caller["source_file"].as_str().unwrap().to_string(),
                caller["source_line"].as_i64().unwrap(),
                caller["confidence"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert_eq!(ready, shadowed("src/shadow.rs"), "{v}");
    let r = f.trace(&["callers", "finish", "--json"]);
    r.ok();
    assert_eq!(caller_rows(&r.view(), "src/shadow.rs::finish"), extracted("src/shadow.rs", &[16]));
}

#[test]
fn rust_calls_inside_macro_arguments_resolve_as_they_do_outside_one() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write("src/lib.rs", "pub mod ledger;\npub mod other;\npub fn helper() -> u8 {\n    0\n}\n");
    f.write(
        "src/other.rs",
        "pub struct Other;\nimpl Other {\n    pub fn total(&self) -> u8 {\n        0\n    }\n    pub fn build() -> Other {\n        Other\n    }\n}\n",
    );
    f.write(
        "src/ledger.rs",
        concat!(
            "use crate::other::Other;\n",
            "pub struct Ledger;\n",
            "impl Ledger {\n",
            "    pub fn total(&self) -> u8 {\n",
            "        3\n",
            "    }\n",
            "    pub fn build() -> Ledger {\n",
            "        Ledger\n",
            "    }\n",
            "    pub fn check(&self) {\n",
            "        assert_eq!(self.total(), 3);\n",
            "        self.total();\n",
            "        format!(\"{}\", Self::build());\n",
            "        Self::build();\n",
            "    }\n",
            "}\n",
            "pub fn helper() -> u8 {\n",
            "    1\n",
            "}\n",
            "mod tests {\n",
            "    fn probe() {\n",
            "        assert_eq!(super::helper(), 1);\n",
            "        super::helper();\n",
            "    }\n",
            "}\n",
        ),
    );
    f.commit("calls inside macro arguments");
    for (name, lines, elsewhere) in [
        ("total", [11, 12], "src/other.rs::total"),
        ("build", [13, 14], "src/other.rs::build"),
        ("helper", [22, 23], "src/lib.rs::helper"),
    ] {
        let r = f.trace(&["callers", name, "--json"]);
        r.ok();
        let v = r.view();
        assert_eq!(
            caller_rows(&v, &format!("src/ledger.rs::{name}")),
            extracted("src/ledger.rs", &lines),
            "{name}: {v}"
        );
        let other = caller_rows(&v, elsewhere);
        assert!(
            !other.iter().any(|(file, line, _)| file == "src/ledger.rs" && lines.contains(line)),
            "{elsewhere}: {other:?}"
        );
    }
}

#[test]
fn rust_binary_targets_resolve_crate_paths_inside_their_own_crate() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write("src/lib.rs", "pub mod helpers;\n");
    f.write("src/helpers.rs", "pub fn run() {}\n");
    let main = "mod helpers;\nfn main() {\n    crate::helpers::run();\n}\n";
    f.write("src/bin/tool.rs", main);
    f.write("src/bin/helpers.rs", "pub fn run() {}\n");
    f.write("src/bin/report/main.rs", main);
    f.write("src/bin/report/helpers.rs", "pub fn run() {}\n");
    f.commit("binary targets");
    let r = f.trace(&["callers", "run", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(caller_rows(&v, "src/bin/helpers.rs::run"), extracted("src/bin/tool.rs", &[3]), "{v}");
    assert_eq!(
        caller_rows(&v, "src/bin/report/helpers.rs::run"),
        extracted("src/bin/report/main.rs", &[3]),
        "{v}"
    );
    let library = caller_rows(&v, "src/helpers.rs::run");
    assert!(!library.iter().any(|(file, _, _)| file.starts_with("src/bin/")), "{library:?}");
}

#[test]
fn a_name_line_inside_a_cargo_multiline_string_keeps_the_package_name() {
    let f = Fixture::new();
    f.write(
        "core/Cargo.toml",
        "[package]\nname = \"ledger-core\"\nversion = \"0.1.0\"\ndescription = \"\"\"\nname = \"x\"\n\"\"\"\n",
    );
    f.write("core/src/lib.rs", "pub fn open() -> i32 {\n    1\n}\n");
    f.write(
        "app/Cargo.toml",
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\nledger-core = { path = \"../core\" }\n",
    );
    f.write("app/src/main.rs", "fn main() {\n    ledger_core::open();\n}\n");
    f.commit("multi-line string in a manifest");
    let r = f.trace(&["callers", "open", "--json"]);
    r.ok();
    assert_eq!(
        caller_rows(&r.view(), "core/src/lib.rs::open"),
        extracted("app/src/main.rs", &[2])
    );
}

#[test]
fn a_cargo_version_bump_reresolves_no_import() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write("src/main.rs", "mod tools;\nuse crate::tools::measure;\nfn main() {\n    measure();\n}\n");
    f.write("src/tools.rs", "pub fn measure() {}\n");
    f.commit("one crate");
    f.trace(&["callers", "measure"]).ok();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.2.0\"\n");
    for _ in 0..2 {
        let bumped = f.trace_env(&["callers", "measure", "--json"], &[("TRACE_TIMING", "1")]);
        bumped.ok();
        assert!(!bumped.stderr.contains("decode relations_imports"), "{}", bumped.stderr);
        assert_eq!(
            caller_rows(&bumped.view(), "src/tools.rs::measure"),
            extracted("src/main.rs", &[4])
        );
    }
}

#[test]
fn a_cargo_dependency_added_makes_its_paths_external_and_reresolves_no_import() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write("src/main.rs", "mod map;\nfn main() {\n    serde_json::Map::new();\n}\n");
    f.write("src/map.rs", "pub struct Map;\nimpl Map {\n    pub fn new() -> Map {\n        Map\n    }\n}\n");
    f.commit("a path through an unlisted crate");
    let unlisted = f.trace(&["callers", "new", "--json"]);
    unlisted.ok();
    assert_eq!(
        caller_rows(&unlisted.view(), "src/map.rs::new"),
        vec![("src/main.rs".to_string(), 3, "INFERRED".to_string())]
    );
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\nserde_json = \"1\"\n");
    let updated = f.trace_env(&["callers", "main", "--json"], &[("TRACE_TIMING", "1")]);
    updated.ok();
    assert!(!updated.stderr.contains("decode relations_imports"), "{}", updated.stderr);
    let listed = f.trace(&["callers", "new", "--json"]);
    listed.ok();
    assert_eq!(caller_rows(&listed.view(), "src/map.rs::new"), vec![]);
}

fn ambiguous(file: &str, lines: &[i64]) -> Vec<(String, i64, String)> {
    lines
        .iter()
        .map(|line| (file.to_string(), *line, "AMBIGUOUS".to_string()))
        .collect()
}

#[test]
fn rust_method_the_stated_receiver_type_does_not_declare_stays_ambiguous() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write(
        "src/shape.rs",
        "pub trait Shape {\n    fn describe(&self) -> String {\n        String::new()\n    }\n}\n",
    );
    f.write(
        "src/lib.rs",
        concat!(
            "pub mod shape;\n",
            "use crate::shape::Shape;\n",
            "use std::ops::Deref;\n",
            "pub struct Circle;\n",
            "impl Shape for Circle {}\n",
            "impl Circle {\n",
            "    pub fn report(&self) -> String {\n",
            "        self.describe()\n",
            "    }\n",
            "    pub fn radius(&self) -> f64 {\n",
            "        2.0\n",
            "    }\n",
            "}\n",
            "pub struct Wrapper(Circle);\n",
            "impl Deref for Wrapper {\n",
            "    type Target = Circle;\n",
            "    fn deref(&self) -> &Circle {\n",
            "        &self.0\n",
            "    }\n",
            "}\n",
            "pub fn measure(wrapper: Wrapper) -> f64 {\n",
            "    wrapper.radius()\n",
            "}\n",
        ),
    );
    f.commit("trait default method and deref");
    for (target, line) in [("src/shape.rs::describe", 8), ("src/lib.rs::radius", 22)] {
        let r = f.trace(&["callers", target.rsplit("::").next().unwrap(), "--json"]);
        r.ok();
        let v = r.view();
        assert_eq!(caller_rows(&v, target), ambiguous("src/lib.rs", &[line]), "{target}: {v}");
    }
}

#[test]
fn rust_type_path_reaches_an_impl_in_another_module_of_its_crate() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write("src/lib.rs", "pub mod thing;\npub fn make() -> thing::Thing {\n    thing::Thing::assemble()\n}\n");
    f.write("src/thing.rs", "mod build;\npub struct Thing;\n");
    f.write(
        "src/thing/build.rs",
        "use super::Thing;\nimpl Thing {\n    pub fn assemble() -> Thing {\n        Thing\n    }\n}\n",
    );
    f.commit("impl in a child module");
    let r = f.trace(&["callers", "assemble", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(caller_rows(&v, "src/thing/build.rs::assemble"), extracted("src/lib.rs", &[3]), "{v}");
}

#[test]
fn rust_free_call_through_a_reexport_passes_a_same_named_method() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write(
        "src/lib.rs",
        concat!(
            "pub mod caller;\n",
            "pub mod helpers;\n",
            "pub use helpers::build;\n",
            "pub struct Foo;\n",
            "impl Foo {\n",
            "    pub fn build() -> Foo {\n",
            "        Foo\n",
            "    }\n",
            "}\n",
        ),
    );
    f.write("src/helpers.rs", "pub fn build() -> u8 {\n    1\n}\n");
    f.write("src/caller.rs", "use crate::build;\npub fn run() -> u8 {\n    build()\n}\n");
    f.commit("re-export beside a method");
    let r = f.trace(&["callers", "build", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(caller_rows(&v, "src/helpers.rs::build"), extracted("src/caller.rs", &[3]), "{v}");
    let method = caller_rows(&v, "src/lib.rs::build");
    assert!(!method.iter().any(|(file, line, _)| file == "src/caller.rs" && *line == 3), "{method:?}");
}

#[test]
fn rust_local_free_call_ignores_a_namesake_in_a_file_imported_for_another_name() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write("src/a.rs", "pub struct X;\npub fn build() {}\n");
    f.write("src/main.rs", "mod a;\nuse crate::a::X;\nfn build() {}\nfn main() {\n    build();\n}\n");
    f.commit("local free call");
    let r = f.trace(&["callers", "build", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(caller_rows(&v, "src/main.rs::build"), extracted("src/main.rs", &[5]), "{v}");
    let namesake = caller_rows(&v, "src/a.rs::build");
    assert!(!namesake.iter().any(|(_, line, _)| *line == 5), "{namesake:?}");
}

#[test]
fn rust_renamed_reexport_reaches_the_type_it_renames() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write("src/lib.rs", "pub mod inner;\npub mod user;\npub use inner::Thing as Renamed;\n");
    f.write("src/inner.rs", "pub struct Thing;\nimpl Thing {\n    pub fn build() -> Thing {\n        Thing\n    }\n}\n");
    f.write("src/user.rs", "use crate::Renamed;\npub fn via() -> Renamed {\n    Renamed::build()\n}\n");
    f.commit("renamed re-export");
    let r = f.trace(&["callers", "build", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(caller_rows(&v, "src/inner.rs::build"), extracted("src/user.rs", &[3]), "{v}");
}

fn aliased_twin_crates(f: &Fixture) {
    let library = "pub struct Error;\nimpl Error {\n    pub fn new() -> Error {\n        Error\n    }\n}\npub fn run() {}\n";
    for name in ["alpha", "beta"] {
        f.write(&format!("{name}/Cargo.toml"), &format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\n"));
        f.write(&format!("{name}/src/lib.rs"), library);
    }
    f.write(
        "app/Cargo.toml",
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\nalpha = { path = \"../alpha\" }\nbeta = { path = \"../beta\" }\n",
    );
    f.write(
        "app/src/main.rs",
        concat!(
            "use alpha::{run, Error};\n",
            "use beta::{run as beta_run, Error as BetaError};\n",
            "fn main() {\n",
            "    Error::new();\n",
            "    BetaError::new();\n",
            "    run();\n",
            "    beta_run();\n",
            "}\n",
        ),
    );
    f.commit("aliased twin crates");
}

#[test]
fn rust_aliased_type_path_resolves_into_the_crate_its_use_names() {
    let f = Fixture::new();
    aliased_twin_crates(&f);
    let r = f.trace(&["callers", "new", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(caller_rows(&v, "alpha/src/lib.rs::new"), extracted("app/src/main.rs", &[4]), "{v}");
    assert_eq!(caller_rows(&v, "beta/src/lib.rs::new"), extracted("app/src/main.rs", &[5]), "{v}");
}

#[test]
fn rust_aliased_free_call_resolves_to_the_function_it_renames() {
    let f = Fixture::new();
    aliased_twin_crates(&f);
    let r = f.trace(&["callers", "run", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(caller_rows(&v, "alpha/src/lib.rs::run"), extracted("app/src/main.rs", &[6]), "{v}");
    assert_eq!(caller_rows(&v, "beta/src/lib.rs::run"), extracted("app/src/main.rs", &[7]), "{v}");
}

#[test]
fn rust_type_path_without_a_manifest_keeps_its_caller() {
    let f = Fixture::new();
    f.write("src/lib.rs", "pub mod a;\npub fn make() {\n    a::Thing::assemble();\n}\n");
    f.write("src/a.rs", "pub struct Thing;\nimpl Thing {\n    pub fn assemble() -> Thing {\n        Thing\n    }\n}\n");
    f.commit("type path, no manifest");
    let r = f.trace(&["callers", "assemble", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(
        caller_rows(&v, "src/a.rs::assemble"),
        vec![("src/lib.rs".to_string(), 3, "INFERRED".to_string())],
        "{v}"
    );
}

#[test]
fn rust_qualified_external_path_never_resolves_into_the_repository() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write("src/lib.rs", "pub mod aliased;\npub mod error;\npub mod qualified;\n");
    f.write(
        "src/error.rs",
        concat!(
            "use std::io;\n",
            "pub struct Error;\n",
            "impl Error {\n",
            "    pub fn new() -> Self {\n",
            "        Error\n",
            "    }\n",
            "    pub fn wrap() -> io::Error {\n",
            "        io::Error::new(io::ErrorKind::Other, \"x\")\n",
            "    }\n",
            "}\n",
        ),
    );
    f.write(
        "src/qualified.rs",
        concat!(
            "use crate::error::Error;\n",
            "pub fn open() -> std::io::Error {\n",
            "    std::io::Error::new(std::io::ErrorKind::Other, \"x\")\n",
            "}\n",
            "pub fn local() -> Error {\n",
            "    Error::new()\n",
            "}\n",
        ),
    );
    f.write(
        "src/aliased.rs",
        concat!(
            "use crate::error::Error;\n",
            "use std::io::Error as IoError;\n",
            "pub fn open() -> IoError {\n",
            "    IoError::new(std::io::ErrorKind::Other, \"x\")\n",
            "}\n",
            "pub fn local() -> Error {\n",
            "    Error::new()\n",
            "}\n",
        ),
    );
    f.commit("external paths beside a local Error");
    let r = f.trace(&["callers", "Error::new", "--json"]);
    r.ok();
    let v = r.view();
    let mut expected = extracted("src/aliased.rs", &[7]);
    expected.extend(extracted("src/qualified.rs", &[6]));
    assert_eq!(caller_rows(&v, "src/error.rs::new"), expected, "{v}");
}

#[test]
fn rust_path_through_a_module_a_glob_brings_in_keeps_its_caller() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write("src/lib.rs", "pub mod model;\npub mod a;\n");
    f.write("src/model.rs", "pub struct Thing;\nimpl Thing {\n    pub fn assemble() -> Thing {\n        Thing\n    }\n}\n");
    f.write("src/a.rs", "use crate::model;\nmod b;\n");
    f.write(
        "src/a/b.rs",
        "use super::*;\npub fn make() -> model::Thing {\n    model::Thing::assemble()\n}\n",
    );
    f.commit("module brought in by the parent's use, through a glob");
    let r = f.trace(&["callers", "assemble", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(caller_rows(&v, "src/model.rs::assemble"), extracted("src/a/b.rs", &[3]), "{v}");
}

#[test]
fn rust_path_into_an_inline_module_through_a_use_or_a_glob_keeps_its_caller() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write(
        "src/lib.rs",
        concat!(
            "mod outer {\n",
            "    pub mod inner {\n",
            "        pub fn deep() -> u8 {\n",
            "            2\n",
            "        }\n",
            "    }\n",
            "}\n",
            "use outer::inner;\n",
            "use outer::inner as renamed;\n",
            "pub fn via_renamed_module() -> u8 {\n",
            "    renamed::deep()\n",
            "}\n",
            "pub fn via_module_import() -> u8 {\n",
            "    inner::deep()\n",
            "}\n",
            "pub fn via_full_path() -> u8 {\n",
            "    outer::inner::deep()\n",
            "}\n",
            "mod helpers {\n",
            "    pub fn shared() -> u8 {\n",
            "        3\n",
            "    }\n",
            "}\n",
            "#[cfg(test)]\n",
            "mod tests {\n",
            "    use super::*;\n",
            "    #[test]\n",
            "    fn through_glob() {\n",
            "        assert_eq!(helpers::shared(), 3);\n",
            "    }\n",
            "    #[test]\n",
            "    fn through_glob_plain() {\n",
            "        let value = helpers::shared();\n",
            "        let _ = value;\n",
            "    }\n",
            "}\n",
        ),
    );
    f.commit("inline modules reached through a use, an alias, and a glob");
    for (name, lines) in [("deep", vec![11, 14, 17]), ("shared", vec![29, 33])] {
        let r = f.trace(&["callers", name, "--json"]);
        r.ok();
        let v = r.view();
        assert_eq!(caller_rows(&v, &format!("src/lib.rs::{name}")), extracted("src/lib.rs", &lines), "{v}");
    }
}

#[test]
fn rust_module_path_reaches_only_the_module_it_names() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write("src/other.rs", "pub fn deep() -> u8 {\n    1\n}\n");
    f.write(
        "src/lib.rs",
        concat!(
            "pub mod other;\n",
            "mod inner {\n",
            "    pub fn deep() -> u8 {\n",
            "        2\n",
            "    }\n",
            "}\n",
            "#[cfg(test)]\n",
            "mod tests {\n",
            "    use super::*;\n",
            "    use crate::other;\n",
            "    fn through_glob() -> u8 {\n",
            "        inner::deep()\n",
            "    }\n",
            "}\n",
        ),
    );
    f.commit("an inline module's function beside an imported module's function of its name");
    let r = f.trace(&["callers", "deep", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(caller_rows(&v, "src/lib.rs::deep"), extracted("src/lib.rs", &[12]), "{v}");
    let other = caller_rows(&v, "src/other.rs::deep");
    assert!(!other.iter().any(|(file, line, _)| file == "src/lib.rs" && *line == 12), "{v}");
}

#[test]
fn rust_module_path_the_crate_names_never_reaches_a_same_stem_file() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write("src/main.rs", "mod bound;\nmod cli;\nmod engine;\nmod net;\nfn main() {\n    cli::run();\n}\n");
    f.write("src/net.rs", "pub mod engine;\n");
    f.write("src/net/engine.rs", "pub fn start() {}\npub struct Config;\n");
    f.write("src/engine.rs", "#[path = \"imp_unix.rs\"]\nmod imp;\npub use imp::start;\n");
    f.write("src/imp_unix.rs", "pub fn start() {}\n");
    f.write(
        "src/cli.rs",
        "use crate::net::engine::Config;\npub fn run() {\n    let _ = Config;\n    crate::engine::start();\n}\n",
    );
    f.write(
        "src/bound.rs",
        "use crate::engine;\nuse crate::net::engine::Config;\npub fn go() {\n    let _ = Config;\n    engine::start();\n}\n",
    );
    f.commit("a module path into a #[path] module beside an imported file of the module's stem");
    let r = f.trace(&["callers", "start", "--json"]);
    r.ok();
    let v = r.view();
    let namesake = caller_rows(&v, "src/net/engine.rs::start");
    for (file, line) in [("src/cli.rs", 4), ("src/bound.rs", 5)] {
        assert!(!namesake.iter().any(|caller| caller.0 == file && caller.1 == line), "{file}:{line} {v}");
    }
    assert!(namesake.contains(&("src/cli.rs".to_string(), 1, "EXTRACTED".to_string())), "{v}");
}

#[test]
fn rust_crate_path_never_reaches_a_same_stem_file_of_that_crate() {
    let f = Fixture::new();
    f.write(
        "Cargo.toml",
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\nmylib = { path = \"mylib\" }\n",
    );
    f.write("mylib/Cargo.toml", "[package]\nname = \"mylib\"\nversion = \"0.1.0\"\n");
    f.write("mylib/src/lib.rs", "pub mod engine;\npub mod net;\n");
    f.write("mylib/src/net.rs", "pub mod engine;\n");
    f.write("mylib/src/net/engine.rs", "pub fn start() {}\npub struct Config;\n");
    f.write("mylib/src/engine.rs", "#[path = \"imp_unix.rs\"]\nmod imp;\npub use imp::start;\n");
    f.write("mylib/src/imp_unix.rs", "pub fn start() {}\n");
    f.write("mylib/src/bin/mylib.rs", "fn main() {}\n");
    f.write(
        "src/lib.rs",
        "use mylib::net::engine::Config;\npub fn run() {\n    let _ = Config;\n    mylib::engine::start();\n}\n",
    );
    f.commit("a crate path into a #[path] module beside an imported file of the module's stem");
    let r = f.trace(&["callers", "start", "--json"]);
    r.ok();
    let v = r.view();
    let namesake = caller_rows(&v, "mylib/src/net/engine.rs::start");
    assert!(!namesake.iter().any(|caller| caller.0 == "src/lib.rs" && caller.1 == 4), "{v}");
    assert!(namesake.contains(&("src/lib.rs".to_string(), 1, "EXTRACTED".to_string())), "{v}");
}

#[test]
fn rust_type_path_outside_the_repository_never_resolves_to_a_free_function() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write(
        "src/lib.rs",
        concat!(
            "pub mod build;\n",
            "use std::collections::*;\n",
            "pub fn vector() -> Vec<u8> {\n",
            "    Vec::new()\n",
            "}\n",
            "pub fn map() -> HashMap<u8, u8> {\n",
            "    HashMap::new()\n",
            "}\n",
            "pub fn number() -> u8 {\n",
            "    str::parse::<u8>(\"1\").unwrap()\n",
            "}\n",
        ),
    );
    f.write("src/build.rs", "pub fn new() -> u8 {\n    1\n}\npub fn parse() -> u8 {\n    2\n}\n");
    f.commit("prelude and glob-imported type paths beside lone free functions");
    for name in ["new", "parse"] {
        let r = f.trace(&["callers", name, "--json"]);
        r.ok();
        let v = r.view();
        assert_eq!(caller_rows(&v, &format!("src/build.rs::{name}")), vec![], "{v}");
    }
}

#[test]
fn rust_type_path_into_the_repository_never_resolves_to_a_free_function() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write("src/main.rs", "mod app;\nmod config;\nmod loader;\nfn main() {\n    app::run();\n}\n");
    f.write("src/loader.rs", "pub trait Loader: Sized + Default {\n    fn load() -> Self {\n        Self::default()\n    }\n}\n");
    f.write(
        "src/config.rs",
        concat!(
            "use crate::loader::Loader;\n",
            "use Kind::*;\n",
            "#[derive(Default)]\n",
            "pub struct Config;\n",
            "pub enum Kind {\n",
            "    File,\n",
            "    Env,\n",
            "}\n",
            "impl Loader for Config {}\n",
            "pub fn load(kind: Kind) -> Config {\n",
            "    match kind {\n",
            "        File | Env => Config,\n",
            "    }\n",
            "}\n",
        ),
    );
    f.write(
        "src/app.rs",
        concat!(
            "use crate::config;\n",
            "use crate::loader::Loader;\n",
            "pub fn run() {\n",
            "    let _ = config::Config::load();\n",
            "    let _ = config::load(config::Kind::File);\n",
            "}\n",
        ),
    );
    f.commit("a type path to a trait method beside a free function of its name in the type's file");
    let r = f.trace(&["callers", "load", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(caller_rows(&v, "src/config.rs::load"), extracted("src/app.rs", &[5]), "{v}");
}

#[test]
fn rust_path_through_a_cargo_dependency_is_external_and_any_other_falls_back() {
    let f = Fixture::new();
    f.write(
        "Cargo.toml",
        concat!(
            "[package]\n",
            "name = \"app\"\n",
            "version = \"0.1.0\"\n",
            "\n",
            "[dependencies]\n",
            "serde_json = \"1\"\n",
            "json = { package = \"serde_json\", version = \"1\" }\n",
            "\n",
            "[dev-dependencies]\n",
            "json-fixtures = \"1\"\n",
            "\n",
            "[build-dependencies]\n",
            "json-codegen = \"1\"\n",
        ),
    );
    f.write("src/lib.rs", "pub mod external;\npub mod map;\n");
    f.write("src/map.rs", "pub struct Map;\nimpl Map {\n    pub fn new() -> Map {\n        Map\n    }\n}\n");
    f.write(
        "src/external.rs",
        concat!(
            "use serde_json::Map;\n",
            "pub fn bound() -> Map<String, u8> {\n",
            "    Map::new()\n",
            "}\n",
            "pub fn written() {\n",
            "    serde_json::Map::new();\n",
            "}\n",
            "pub fn renamed() {\n",
            "    json::Map::new();\n",
            "}\n",
            "pub fn tested() {\n",
            "    json_fixtures::Map::new();\n",
            "}\n",
            "pub fn built() {\n",
            "    json_codegen::Map::new();\n",
            "}\n",
            "pub fn local() -> crate::map::Map {\n",
            "    crate::map::Map::new()\n",
            "}\n",
            "pub fn unlisted() {\n",
            "    unlisted::Map::new();\n",
            "}\n",
        ),
    );
    f.write("tool/Cargo.toml", "[package]\nname = \"tool\"\nversion = \"0.1.0\"\n");
    f.write("tool/src/main.rs", "fn main() {\n    serde_json::Map::new();\n}\n");
    f.commit("dependency paths beside a local Map");
    let r = f.trace(&["callers", "new", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(
        caller_rows(&v, "src/map.rs::new"),
        vec![
            ("src/external.rs".to_string(), 18, "EXTRACTED".to_string()),
            ("src/external.rs".to_string(), 21, "INFERRED".to_string()),
            ("tool/src/main.rs".to_string(), 2, "INFERRED".to_string()),
        ],
        "{v}"
    );
}

fn crate_error(f: &Fixture) {
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write(
        "src/error.rs",
        "pub struct Error;\nimpl Error {\n    pub fn build() -> Self {\n        Error\n    }\n}\n",
    );
}

#[test]
fn rust_use_in_a_test_module_binds_only_inside_it() {
    let f = Fixture::new();
    crate_error(&f);
    f.write(
        "src/lib.rs",
        concat!(
            "pub mod error;\n",
            "use std::io::Error;\n",
            "use std::io::Error as Failure;\n",
            "pub fn io() -> Error {\n",
            "    Error::other(\"x\")\n",
            "}\n",
            "#[cfg(test)]\n",
            "mod tests {\n",
            "    use crate::error::Error;\n",
            "    use crate::error::Error as Failure;\n",
            "    fn builds() {\n",
            "        let _ = Error::build();\n",
            "    }\n",
            "    fn fails() {\n",
            "        let _ = Failure::build();\n",
            "    }\n",
            "}\n",
        ),
    );
    f.commit("a test module binds the crate's Error over the file's io::Error");
    let r = f.trace(&["callers", "build", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(caller_rows(&v, "src/error.rs::build"), extracted("src/lib.rs", &[12, 15]), "{v}");
}

#[test]
fn rust_use_in_a_test_module_leaves_the_outer_module_bound() {
    let f = Fixture::new();
    crate_error(&f);
    f.write(
        "src/lib.rs",
        concat!(
            "pub mod error;\n",
            "use crate::error::Error;\n",
            "pub fn make() -> Error {\n",
            "    Error::build()\n",
            "}\n",
            "#[cfg(test)]\n",
            "mod tests {\n",
            "    use std::io::Error;\n",
            "    fn io() -> Error {\n",
            "        Error::other(\"x\")\n",
            "    }\n",
            "}\n",
        ),
    );
    f.commit("a test module binds io::Error over the file's crate Error");
    let r = f.trace(&["callers", "build", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(caller_rows(&v, "src/error.rs::build"), extracted("src/lib.rs", &[4]), "{v}");
}

#[test]
fn rust_renamed_path_dependency_reaches_its_library() {
    for (workspace, dependency) in [
        ("[workspace]\nmembers = [\"app\", \"core\"]\n", "engine = { path = \"../core\", package = \"my-core\" }\n"),
        (
            "[workspace]\nmembers = [\"app\", \"core\"]\n\n[workspace.dependencies]\nengine = { path = \"core\", package = \"my-core\" }\n",
            "engine = { workspace = true }\n",
        ),
    ] {
        let f = Fixture::new();
        f.write("Cargo.toml", workspace);
        f.write("core/Cargo.toml", "[package]\nname = \"my-core\"\nversion = \"0.1.0\"\n");
        f.write("core/src/lib.rs", "pub fn start() {}\npub fn stop() {}\n");
        f.write(
            "app/Cargo.toml",
            &format!("[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\n{dependency}"),
        );
        f.write("app/src/main.rs", "use engine::stop;\nfn main() {\n    engine::start();\n    stop();\n}\n");
        f.commit("a path dependency renamed by its key");
        for (name, line) in [("start", 3), ("stop", 4)] {
            let r = f.trace(&["callers", name, "--json"]);
            r.ok();
            let v = r.view();
            assert_eq!(
                caller_rows(&v, &format!("core/src/lib.rs::{name}")),
                extracted("app/src/main.rs", &[line]),
                "{dependency}{v}"
            );
        }
    }
}

#[test]
fn rust_path_dependency_is_named_by_its_library() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[workspace]\nmembers = [\"apps/app\", \"apps/engine\", \"kernel\"]\n");
    f.write("kernel/Cargo.toml", "[package]\nname = \"kernel\"\nversion = \"0.1.0\"\n\n[lib]\nname = \"engine\"\n");
    f.write("kernel/src/lib.rs", "pub fn start() {}\n");
    f.write("apps/engine/Cargo.toml", "[package]\nname = \"engine\"\nversion = \"0.1.0\"\n");
    f.write("apps/engine/src/lib.rs", "pub fn start() {}\n");
    f.write(
        "apps/app/Cargo.toml",
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\nkernel = { path = \"../../kernel\" }\n",
    );
    f.write("apps/app/src/main.rs", "fn main() {\n    engine::start();\n}\n");
    f.commit("a path dependency whose library has its own name, beside a nearer library of that name");
    let r = f.trace(&["callers", "start", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(caller_rows(&v, "kernel/src/lib.rs::start"), extracted("apps/app/src/main.rs", &[2]), "{v}");
    assert_eq!(caller_rows(&v, "apps/engine/src/lib.rs::start"), vec![], "{v}");
}

#[test]
fn rust_patched_dependency_reaches_its_library() {
    let f = Fixture::new();
    f.write(
        "Cargo.toml",
        "[workspace]\nmembers = [\"app\", \"engine\"]\n\n[patch.crates-io]\nengine = { path = \"engine\" }\n",
    );
    f.write("engine/Cargo.toml", "[package]\nname = \"engine\"\nversion = \"0.1.0\"\n");
    f.write("engine/src/lib.rs", "pub fn start() {}\n\npub fn stop() {}\n");
    f.write("app/Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\nengine = \"0.1\"\n");
    f.write("app/src/main.rs", "use engine::stop;\n\nfn main() {\n    engine::start();\n    stop();\n}\n");
    f.commit("a crates.io dependency patched to a crate of the repository");
    for (name, line) in [("start", 4), ("stop", 5)] {
        let r = f.trace(&["callers", name, "--json"]);
        r.ok();
        let v = r.view();
        assert_eq!(caller_rows(&v, &format!("engine/src/lib.rs::{name}")), extracted("app/src/main.rs", &[line]), "{v}");
    }
}

#[test]
fn rust_patch_matches_its_source_the_way_cargo_compares_it() {
    for (patch, dependency, confidence) in [
        ("[patch.\"https://github.com/acme/engine\"]", "{ git = \"https://github.com/acme/engine.git\" }", "EXTRACTED"),
        ("[patch.\"https://github.com/acme/engine\"]", "{ git = \"https://GitHub.com/acme/engine.git\" }", "EXTRACTED"),
        ("[patch.\"https://github.com/Acme/Engine/\"]", "{ git = \"http://github.com/acme/engine\" }", "EXTRACTED"),
        ("[patch.\"https://gitlab.com/acme/engine\"]", "{ git = \"HTTPS://GitLab.com/acme/engine\" }", "EXTRACTED"),
        ("[patch.\"https://github.com/rust-lang/crates.io-index\"]", "\"0.1\"", "EXTRACTED"),
        ("[patch.crates-io]", "{ version = \"0.1\", registry = \"crates-io\" }", "EXTRACTED"),
        ("[patch.\"https://gitlab.com/Acme/engine\"]", "{ git = \"https://gitlab.com/acme/engine\" }", "INFERRED"),
    ] {
        let f = Fixture::new();
        f.write(
            "Cargo.toml",
            &format!("[workspace]\nmembers = [\"app\", \"engine\"]\n\n{patch}\nengine = {{ path = \"engine\" }}\n"),
        );
        f.write("engine/Cargo.toml", "[package]\nname = \"engine\"\nversion = \"0.1.0\"\n");
        f.write("engine/src/lib.rs", "pub fn start() {}\n\npub fn stop() {}\n");
        f.write(
            "app/Cargo.toml",
            &format!("[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\nengine = {dependency}\n"),
        );
        f.write("app/src/main.rs", "use engine::stop;\n\nfn main() {\n    engine::start();\n    stop();\n}\n");
        f.commit("a dependency patched under its source spelled another way");
        let r = f.trace(&["callers", "stop", "--json"]);
        r.ok();
        let v = r.view();
        assert_eq!(
            caller_rows(&v, "engine/src/lib.rs::stop"),
            vec![("app/src/main.rs".to_string(), 5, confidence.to_string())],
            "{patch} {dependency}{v}"
        );
    }
}

#[test]
fn rust_renamed_patch_reaches_its_library() {
    let f = Fixture::new();
    f.write(
        "Cargo.toml",
        "[workspace]\nmembers = [\"app\", \"engine\"]\n\n[patch.crates-io]\nengine-local = { path = \"engine\", package = \"engine\" }\n",
    );
    f.write("engine/Cargo.toml", "[package]\nname = \"engine\"\nversion = \"0.1.0\"\n");
    f.write("engine/src/lib.rs", "pub fn start() {}\n");
    f.write("app/Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\nengine = \"0.1\"\n");
    f.write("app/src/main.rs", "fn main() {\n    engine::start();\n}\n");
    f.commit("a crates.io dependency patched by an entry that names its package");
    let r = f.trace(&["callers", "start", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(caller_rows(&v, "engine/src/lib.rs::start"), extracted("app/src/main.rs", &[2]), "{v}");
}

#[test]
fn rust_bare_name_reaches_only_the_crates_its_package_names() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[workspace]\nmembers = [\"crates/config\", \"crates/svc\"]\n");
    f.write("crates/config/Cargo.toml", "[package]\nname = \"config\"\nversion = \"0.1.0\"\n");
    f.write("crates/config/src/lib.rs", "pub fn load() {}\n");
    f.write("crates/svc/Cargo.toml", "[package]\nname = \"svc\"\nversion = \"0.1.0\"\n");
    f.write("crates/svc/src/lib.rs", "mod config;\nmod handlers;\n");
    f.write("crates/svc/src/config.rs", "pub fn load() {}\n");
    f.write("crates/svc/src/handlers.rs", "use crate::config;\n\nmod run;\n");
    f.write(
        "crates/svc/src/handlers/run.rs",
        concat!(
            "use super::*;\n",
            "\n",
            "pub fn start() {\n",
            "    config::load();\n",
            "}\n",
            "\n",
            "mod chained {\n",
            "    use crate::config;\n",
            "    use config::load;\n",
            "\n",
            "    fn boot() {\n",
            "        load();\n",
            "    }\n",
            "}\n",
        ),
    );
    f.commit("a crate's own config module beside a workspace library it does not depend on");
    let r = f.trace(&["callers", "load", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(caller_rows(&v, "crates/config/src/lib.rs::load"), vec![], "{v}");
    assert_eq!(
        caller_rows(&v, "crates/svc/src/config.rs::load"),
        extracted("crates/svc/src/handlers/run.rs", &[4, 12]),
        "{v}"
    );
}

#[test]
fn rust_registry_dependency_named_for_a_package_of_the_repository_reaches_it_inferred() {
    let f = Fixture::new();
    f.write("json/Cargo.toml", "[package]\nname = \"json\"\nversion = \"0.1.0\"\n");
    f.write(
        "json/src/lib.rs",
        "pub struct Value;\nimpl Value {\n    pub fn from(number: u8) -> Value {\n        let _ = number;\n        Value\n    }\n}\n",
    );
    f.write("app/Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\njson = \"0.12\"\n");
    f.write(
        "app/src/main.rs",
        "use json::Value;\nfn main() {\n    let _ = json::Value::from(1);\n    let _ = Value::from(2);\n}\n",
    );
    f.commit("a crates.io json beside the repository's json");
    let r = f.trace(&["callers", "from", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(
        caller_rows(&v, "json/src/lib.rs::from"),
        vec![
            ("app/src/main.rs".to_string(), 3, "INFERRED".to_string()),
            ("app/src/main.rs".to_string(), 4, "INFERRED".to_string()),
        ],
        "{v}"
    );
}

#[test]
fn rust_dependency_named_for_a_package_of_the_repository_is_never_proven_external() {
    for workspace in [Some("[workspace]\nmembers = [\"app\", \"engine\"]\n"), None] {
        let f = Fixture::new();
        if let Some(workspace) = workspace {
            f.write("Cargo.toml", workspace);
        }
        f.write(".cargo/config.toml", "[patch.crates-io]\nengine = { path = \"engine\" }\n");
        f.write("engine/Cargo.toml", "[package]\nname = \"engine\"\nversion = \"0.1.0\"\n");
        f.write("engine/src/lib.rs", "pub fn start() {}\n\npub fn stop() {}\n");
        f.write("app/Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\nengine = \"0.1\"\n");
        f.write("app/src/main.rs", "use engine::stop;\n\nfn main() {\n    engine::start();\n    stop();\n}\n");
        f.commit("a crates.io dependency patched in .cargo/config.toml to a package of the repository");
        for (name, line) in [("start", 4), ("stop", 5)] {
            let r = f.trace(&["callers", name, "--json"]);
            r.ok();
            let v = r.view();
            assert_eq!(
                caller_rows(&v, &format!("engine/src/lib.rs::{name}")),
                vec![("app/src/main.rs".to_string(), line, "INFERRED".to_string())],
                "{workspace:?}{v}"
            );
        }
    }
}

#[test]
fn rust_path_into_a_crate_its_package_leaves_out_never_reaches_an_imported_module() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[workspace]\nmembers = [\"app\", \"engine\"]\n");
    f.write(".cargo/config.toml", "[patch.crates-io]\nengine = { path = \"engine\" }\n");
    f.write("engine/Cargo.toml", "[package]\nname = \"engine\"\nversion = \"0.1.0\"\n");
    f.write("engine/src/lib.rs", "pub fn start() {}\n");
    f.write("app/Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\nengine = \"0.1\"\n");
    f.write("app/src/main.rs", "mod cli;\nmod helpers;\n\nfn main() {\n    cli::run();\n}\n");
    f.write(
        "app/src/cli.rs",
        concat!(
            "use crate::helpers;\n",
            "use engine::start;\n",
            "\n",
            "pub fn run() {\n",
            "    println!(\"{}\", helpers::banner());\n",
            "    engine::start();\n",
            "    start();\n",
            "}\n",
        ),
    );
    f.write("app/src/helpers.rs", "pub fn start() {}\n\npub fn banner() -> &'static str {\n    \"app\"\n}\n");
    f.commit("calls into a left-out crate beside an imported module with a function of its name");
    let r = f.trace(&["callers", "start", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(
        caller_rows(&v, "engine/src/lib.rs::start"),
        vec![
            ("app/src/cli.rs".to_string(), 6, "INFERRED".to_string()),
            ("app/src/cli.rs".to_string(), 7, "INFERRED".to_string()),
        ],
        "{v}"
    );
    assert_eq!(
        caller_rows(&v, "app/src/helpers.rs::start"),
        extracted("app/src/cli.rs", &[1]),
        "{v}"
    );
}

#[test]
fn rust_use_through_a_left_out_name_records_an_inferred_import_edge() {
    let f = Fixture::new();
    f.write(".cargo/config.toml", "[patch.crates-io]\nengine = { path = \"engine\" }\n");
    f.write("engine/Cargo.toml", "[package]\nname = \"engine\"\nversion = \"0.1.0\"\n");
    f.write("engine/src/lib.rs", "pub fn start() {}\n\npub fn stop() {}\n");
    f.write("app/Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\nengine = \"0.1\"\n");
    f.write("app/src/main.rs", "use engine::stop;\n\nfn main() {\n    engine::start();\n    stop();\n}\n");
    f.commit("a use of a crate vendored outside its package's manifest");
    assert_eq!(
        dependencies_of(&f, "app/src/main.rs"),
        vec![("engine/src/lib".to_string(), "INFERRED".to_string())]
    );
    let dependencies = f.trace(&["dependencies", "--path", "app/src/main.rs", "--json"]);
    dependencies.ok();
    assert_eq!(dependencies.view()["results"][0]["direct_dependencies"], 1, "{}", dependencies.view());
    let usages = f.trace(&["usages", "--path", "engine/src/lib.rs", "--json"]);
    usages.ok();
    assert_eq!(usages.view()["results"][0]["declaration"]["name"], "stop", "{}", usages.view());
}

#[test]
fn rust_dependency_left_out_after_the_index_is_warm_reresolves_its_imports() {
    let f = Fixture::new();
    f.write(".cargo/config.toml", "[patch.crates-io]\nengine = { path = \"engine\" }\n");
    f.write("engine/Cargo.toml", "[package]\nname = \"engine\"\nversion = \"0.1.0\"\n");
    f.write("engine/src/lib.rs", "pub fn stop() {}\n");
    f.write("app/Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write("app/src/main.rs", "use engine::stop;\n\nfn main() {\n    stop();\n}\n");
    f.commit("a use of a crate its package does not depend on yet");
    assert_eq!(dependencies_of(&f, "app/src/main.rs"), vec![]);

    f.write("app/Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\nengine = \"0.1\"\n");
    f.commit("the package depends on the vendored crate");
    assert_eq!(
        dependencies_of(&f, "app/src/main.rs"),
        vec![("engine/src/lib".to_string(), "INFERRED".to_string())]
    );
}

#[test]
fn rust_left_out_name_is_its_library_name_unless_the_entry_renames_it() {
    for (dependency, written) in [
        ("md-5 = \"0.10\"", "md5"),
        ("hash = { version = \"0.10\", package = \"md-5\" }", "hash"),
    ] {
        let f = Fixture::new();
        f.write(".cargo/config.toml", "[patch.crates-io]\nmd-5 = { path = \"md5\" }\n");
        f.write("md5/Cargo.toml", "[package]\nname = \"md-5\"\nversion = \"0.10.99\"\n\n[lib]\nname = \"md5\"\n");
        f.write("md5/src/lib.rs", "pub fn compute() {}\n");
        f.write(
            "app/Cargo.toml",
            &format!("[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\n{dependency}\n"),
        );
        f.write("app/src/main.rs", &format!("fn main() {{\n    {written}::compute();\n}}\n"));
        f.commit("a left-out package whose library has its own name");
        let r = f.trace(&["callers", "compute", "--json"]);
        r.ok();
        let v = r.view();
        assert_eq!(
            caller_rows(&v, "md5/src/lib.rs::compute"),
            vec![("app/src/main.rs".to_string(), 2, "INFERRED".to_string())],
            "{dependency}{v}"
        );
    }
}

#[test]
fn rust_path_from_the_crate_root_takes_no_confidence_from_an_unrelated_import() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write("src/main.rs", "mod cli;\nmod engine;\nmod helpers;\n\nfn main() {\n    cli::run();\n}\n");
    f.write("src/engine.rs", "#[path = \"imp_unix.rs\"]\nmod imp;\n\npub use imp::start;\n");
    f.write("src/imp_unix.rs", "pub fn start() {}\n");
    f.write("src/helpers.rs", "pub fn start() {}\n\npub fn banner() {}\n");
    f.write(
        "src/cli.rs",
        "use crate::helpers;\n\npub fn run() {\n    helpers::banner();\n    crate::engine::start();\n}\n",
    );
    f.commit("a re-export through a #[path] module beside an imported module with a function of its name");
    let r = f.trace(&["callers", "start", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(caller_rows(&v, "src/helpers.rs::start"), extracted("src/cli.rs", &[1]), "{v}");
}

#[test]
fn rust_path_from_the_crate_root_takes_no_confidence_from_the_calling_file() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write("src/lib.rs", "mod engine;\n\npub fn start() {}\n\npub fn run() {\n    crate::engine::start();\n}\n");
    f.write("src/engine.rs", "#[path = \"imp_unix.rs\"]\nmod imp;\n\npub use imp::start;\n");
    f.write("src/imp_unix.rs", "pub use std::process::abort as start;\n");
    f.commit("a re-export through a #[path] module beside a function of its name in the calling file");
    let r = f.trace(&["callers", "start", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(caller_rows(&v, "src/lib.rs::start"), vec![], "{v}");
}

#[test]
fn rust_target_dependency_is_external() {
    let f = Fixture::new();
    f.write(
        "Cargo.toml",
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n\n[target.'cfg(unix)'.dependencies]\nnix = \"0.29\"\n",
    );
    f.write("src/lib.rs", "pub mod error;\npub fn errno() {\n    let _ = nix::Error::last();\n}\n");
    f.write("src/error.rs", "pub struct Error;\nimpl Error {\n    pub fn last() -> Self {\n        Error\n    }\n}\n");
    f.commit("a dependency of one target");
    let r = f.trace(&["callers", "last", "--json"]);
    r.ok();
    let v = r.view();
    let callers = caller_rows(&v, "src/error.rs::last");
    assert!(!callers.iter().any(|(file, line, _)| file == "src/lib.rs" && *line == 3), "{v}");
}

#[test]
fn rust_use_through_a_module_another_use_binds_keeps_its_caller() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write("src/lib.rs", "pub mod model;\npub mod service;\n");
    f.write("src/model.rs", "pub fn build() -> u8 {\n    1\n}\n");
    f.write("src/service.rs", "use crate::model;\nuse model::build;\npub fn make() -> u8 {\n    build()\n}\n");
    f.commit("a use through a module another use binds");
    let r = f.trace(&["callers", "build", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(caller_rows(&v, "src/model.rs::build"), extracted("src/service.rs", &[4]), "{v}");
}

#[test]
fn rust_path_through_an_inline_module_stays_in_its_own_file() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write(
        "src/lib.rs",
        concat!(
            "mod inner {\n",
            "    pub struct Thing;\n",
            "    impl Thing {\n",
            "        pub fn make() -> Thing {\n",
            "            Thing\n",
            "        }\n",
            "    }\n",
            "}\n",
            "pub fn outer() {\n",
            "    inner::Thing::make();\n",
            "}\n",
        ),
    );
    f.commit("inline module path");
    let r = f.trace(&["callers", "make", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(caller_rows(&v, "src/lib.rs::make"), extracted("src/lib.rs", &[10]), "{v}");
}

#[test]
fn rust_use_of_an_inline_module_resolves_in_its_own_file() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write(
        "src/lib.rs",
        concat!(
            "mod shapes {\n",
            "    pub struct Thing;\n",
            "    impl Thing {\n",
            "        pub fn assemble() -> Thing {\n",
            "            Thing\n",
            "        }\n",
            "    }\n",
            "    pub fn helper() -> u8 {\n",
            "        1\n",
            "    }\n",
            "}\n",
            "use shapes::helper;\n",
            "use shapes::Thing;\n",
            "pub fn through_use() -> Thing {\n",
            "    Thing::assemble()\n",
            "}\n",
            "pub fn free_through_use() -> u8 {\n",
            "    helper()\n",
            "}\n",
            "pub fn free_through_path() -> u8 {\n",
            "    shapes::helper()\n",
            "}\n",
            "mod tests {\n",
            "    use super::shapes::helper;\n",
            "    fn probe() -> u8 {\n",
            "        helper()\n",
            "    }\n",
            "}\n",
        ),
    );
    f.commit("use of an inline module");
    for (name, lines) in [("helper", vec![18, 21, 26]), ("assemble", vec![15])] {
        let r = f.trace(&["callers", name, "--json"]);
        r.ok();
        let v = r.view();
        assert_eq!(caller_rows(&v, &format!("src/lib.rs::{name}")), extracted("src/lib.rs", &lines), "{v}");
    }
}

#[test]
fn rust_path_through_a_globbed_module_reaches_its_file() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write("src/lib.rs", "pub mod foo;\n");
    f.write("src/foo.rs", "pub mod bar;\npub mod baz;\n");
    f.write(
        "src/foo/baz.rs",
        "pub struct Thing;\nimpl Thing {\n    pub fn assemble() -> Thing {\n        Thing\n    }\n}\npub fn helper() {}\n",
    );
    f.write(
        "src/foo/bar.rs",
        "use super::*;\nuse baz::helper;\npub fn make() -> baz::Thing {\n    helper();\n    baz::Thing::assemble()\n}\n",
    );
    f.commit("module reached through a glob");
    for (name, line) in [("helper", 4), ("assemble", 5)] {
        let r = f.trace(&["callers", name, "--json"]);
        r.ok();
        let v = r.view();
        assert_eq!(
            caller_rows(&v, &format!("src/foo/baz.rs::{name}")),
            extracted("src/foo/bar.rs", &[line]),
            "{v}"
        );
    }
    assert_eq!(
        dependencies_of(&f, "src/foo/bar.rs"),
        vec![
            ("src/foo".to_string(), "EXTRACTED".to_string()),
            ("src/foo/baz".to_string(), "EXTRACTED".to_string()),
        ]
    );
}

#[test]
fn rust_call_resolves_only_to_a_declaration_of_its_exact_name() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write("src/output.rs", "pub const BUDGET: usize = 3;\npub fn budget() -> usize {\n    BUDGET\n}\n");
    f.write("src/main.rs", "mod output;\nuse crate::output::budget;\nfn main() {\n    budget();\n}\n");
    f.commit("function beside its constant");
    let r = f.trace(&["callers", "budget", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(caller_rows(&v, "src/output.rs::budget"), extracted("src/main.rs", &[4]), "{v}");
}

#[test]
fn rust_defines_finds_only_a_declaration_of_its_exact_name() {
    let f = Fixture::new();
    f.write("output.rs", "pub const BUDGET: usize = 3;\npub fn budget() -> usize {\n    BUDGET\n}\n");
    f.commit("function beside its constant");
    let r = f.trace(&["defines", "budget", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(def_files(&v), vec![("output.rs".to_string(), 2)], "{v}");
}

#[test]
fn rust_qualifier_names_only_the_module_or_type_of_its_exact_name() {
    let f = Fixture::new();
    f.write("cart.rs", "pub fn add_item() {}\n");
    f.write("lib.rs", "struct Cart;\nimpl Cart {\n    fn add_item(&self) {}\n}\n");
    f.commit("module and type that differ in case");
    let r = f.trace(&["defines", "cart::add_item", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(def_files(&v), vec![("cart.rs".to_string(), 1)], "{v}");
}

#[test]
fn rust_binding_out_of_force_keeps_the_parameter_type() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write(
        "src/lib.rs",
        concat!(
            "pub struct Raw;\n",
            "pub struct Ready;\n",
            "impl Raw {\n",
            "    pub fn apply(&self) {}\n",
            "}\n",
            "impl Ready {\n",
            "    pub fn apply(&self) {}\n",
            "}\n",
            "pub fn finished(options: Raw, next: Option<Ready>, all: Vec<Ready>) {\n",
            "    {\n",
            "        let options = 1;\n",
            "        let _ = options;\n",
            "    }\n",
            "    options.apply();\n",
            "    if let Some(options) = next {\n",
            "        let _ = options;\n",
            "    }\n",
            "    options.apply();\n",
            "    for options in all {\n",
            "        let _ = options;\n",
            "    }\n",
            "    options.apply();\n",
            "    let check = |options: Ready| options.apply();\n",
            "    options.apply();\n",
            "}\n",
        ),
    );
    f.commit("bindings out of force");
    let r = f.trace(&["callers", "apply", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(caller_rows(&v, "src/lib.rs::apply"), extracted("src/lib.rs", &[14, 18, 22, 24]), "{v}");
    let ready: Vec<(String, i64, String)> = v["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["source_line"] == 7)
        .unwrap_or_else(|| panic!("no Ready::apply row: {v}"))["callers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|caller| {
            (
                caller["source_file"].as_str().unwrap().to_string(),
                caller["source_line"].as_i64().unwrap(),
                caller["confidence"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert_eq!(ready, extracted("src/lib.rs", &[23]), "{v}");
}

#[test]
fn rust_type_the_crate_scope_cannot_reach_falls_to_the_name_rule_at_most_inferred() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write("src/lib.rs", "pub mod generated {\n    include!(\"../assets/widget.rs\");\n}\npub mod user;\n");
    f.write(
        "assets/widget.rs",
        "pub struct Widget;\nimpl Widget {\n    pub fn build() -> Widget {\n        Widget\n    }\n}\n",
    );
    f.write("src/user.rs", "use crate::generated::Widget;\npub fn make() -> Widget {\n    Widget::build()\n}\n");
    f.commit("a type only an include! declares");
    let r = f.trace(&["callers", "build", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(
        references_of(&v, "assets/widget.rs::build", 3),
        vec![("src/user.rs".to_string(), 3, "INFERRED".to_string())],
        "{v}"
    );
}

#[test]
fn rust_path_attribute_module_reads_super_from_the_file_that_declares_it() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write("src/lib.rs", "pub mod exec;\n");
    f.write("src/exec/mod.rs", "pub mod buffer;\n");
    f.write(
        "src/exec/buffer.rs",
        concat!(
            "pub struct HeadTailBuffer {\n",
            "    max_bytes: usize,\n",
            "}\n",
            "impl HeadTailBuffer {\n",
            "    pub fn new(max_bytes: usize) -> Self {\n",
            "        HeadTailBuffer { max_bytes }\n",
            "    }\n",
            "}\n",
            "#[cfg(test)]\n",
            "#[path = \"buffer_tests.rs\"]\n",
            "mod tests;\n",
        ),
    );
    f.write(
        "src/exec/buffer_tests.rs",
        "use super::HeadTailBuffer;\n#[test]\nfn keeps_the_limit() {\n    let buffer = HeadTailBuffer::new(10);\n    assert_eq!(buffer.max_bytes, 10);\n}\n",
    );
    f.commit("tests in a file a path attribute names");
    let r = f.trace(&["callers", "new", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(
        references_of(&v, "src/exec/buffer.rs::new", 5),
        extracted("src/exec/buffer_tests.rs", &[4]),
        "{v}"
    );
}

#[test]
fn rust_path_through_a_module_a_use_binds_reaches_the_module_it_names() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write("src/lib.rs", "mod engine;\nmod picker;\npub(crate) use engine::config;\n");
    f.write("src/engine/mod.rs", "pub mod config;\n");
    f.write(
        "src/engine/config.rs",
        "pub struct ConfigEditsBuilder;\nimpl ConfigEditsBuilder {\n    pub fn new() -> Self {\n        ConfigEditsBuilder\n    }\n}\n",
    );
    f.write(
        "src/picker.rs",
        "use crate::config::ConfigEditsBuilder;\npub fn persist() {\n    let _ = ConfigEditsBuilder::new();\n}\n",
    );
    f.commit("a module a use re-exports");
    let r = f.trace(&["callers", "new", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(references_of(&v, "src/engine/config.rs::new", 3), extracted("src/picker.rs", &[3]), "{v}");
}

#[test]
fn rust_method_matches_the_type_its_impl_names_read_through_the_same_alias() {
    let f = Fixture::new();
    f.write("generated/Cargo.toml", "[package]\nname = \"generated\"\nversion = \"0.1.0\"\n");
    f.write("generated/src/lib.rs", "pub mod plugin_api;\n");
    f.write(
        "generated/src/plugin_api/mod.rs",
        concat!(
            "pub mod event;\n",
            "pub mod generated_api {\n",
            "    include!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/assets/generated.rs\"));\n",
            "}\n",
        ),
    );
    f.write(
        "generated/src/plugin_api/event.rs",
        concat!(
            "pub use super::generated_api::api::event::{\n",
            "    PluginInfo as ProtobufPluginInfo, TabHistory as ProtobufTabHistory,\n",
            "};\n",
            "impl From<u32> for ProtobufPluginInfo {\n",
            "    fn from(id: u32) -> ProtobufPluginInfo {\n",
            "        ProtobufPluginInfo { id }\n",
            "    }\n",
            "}\n",
            "impl From<u32> for ProtobufTabHistory {\n",
            "    fn from(id: u32) -> ProtobufTabHistory {\n",
            "        ProtobufTabHistory { id }\n",
            "    }\n",
            "}\n",
            "pub fn make() -> ProtobufPluginInfo {\n",
            "    ProtobufPluginInfo::from(1)\n",
            "}\n",
        ),
    );
    f.write(
        "generated/assets/generated.rs",
        concat!(
            "pub mod api {\n",
            "    pub mod event {\n",
            "        pub struct PluginInfo {\n",
            "            pub id: u32,\n",
            "        }\n",
            "        pub struct TabHistory {\n",
            "            pub id: u32,\n",
            "        }\n",
            "    }\n",
            "}\n",
        ),
    );
    f.write("plain/Cargo.toml", "[package]\nname = \"plain\"\nversion = \"0.1.0\"\n");
    f.write("plain/src/lib.rs", "mod conv;\nmod gen;\n");
    f.write("plain/src/gen.rs", "pub struct PluginInfo {\n    pub id: u32,\n}\n");
    f.write(
        "plain/src/conv.rs",
        concat!(
            "use crate::gen::PluginInfo;\n",
            "impl From<u32> for PluginInfo {\n",
            "    fn from(id: u32) -> PluginInfo {\n",
            "        PluginInfo { id }\n",
            "    }\n",
            "}\n",
            "impl PluginInfo {\n",
            "    fn zero() -> Self {\n",
            "        Self::from(0)\n",
            "    }\n",
            "}\n",
            "pub fn make() -> PluginInfo {\n",
            "    PluginInfo::from(1)\n",
            "}\n",
        ),
    );
    f.commit("impls of aliased and plain types");
    let r = f.trace(&["callers", "from", "--json"]);
    r.ok();
    let v = r.view();
    let event = "generated/src/plugin_api/event.rs";
    assert_eq!(references_of(&v, &format!("{event}::from"), 5), extracted(event, &[15]), "{v}");
    assert_eq!(references_of(&v, &format!("{event}::from"), 10), Vec::new(), "{v}");
    assert_eq!(references_of(&v, "plain/src/conv.rs::from", 3), extracted("plain/src/conv.rs", &[9, 13]), "{v}");
}

#[test]
fn rust_every_use_that_binds_a_name_counts() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write(
        "src/lib.rs",
        concat!(
            "#[cfg(not(windows))]\n",
            "mod unix;\n",
            "#[cfg(windows)]\n",
            "mod windows;\n",
            "#[cfg(not(windows))]\n",
            "use crate::unix::UnixBackend as Backend;\n",
            "#[cfg(windows)]\n",
            "use crate::windows::WindowsBackend as Backend;\n",
            "pub fn make() {\n",
            "    let _ = Backend::new();\n",
            "}\n",
        ),
    );
    for (file, name) in [("src/unix.rs", "UnixBackend"), ("src/windows.rs", "WindowsBackend")] {
        f.write(file, &format!("pub struct {name};\nimpl {name} {{\n    pub fn new() -> Self {{\n        {name}\n    }}\n}}\n"));
    }
    f.commit("cfg alternatives that bind one alias");
    let r = f.trace(&["callers", "new", "--json"]);
    r.ok();
    let v = r.view();
    for file in ["src/unix.rs", "src/windows.rs"] {
        assert_eq!(references_of(&v, &format!("{file}::new"), 3), ambiguous("src/lib.rs", &[10]), "{v}");
    }
}

#[test]
fn rust_type_alias_is_a_hop_the_walk_follows() {
    let f = Fixture::new();
    f.write("plugin/Cargo.toml", "[package]\nname = \"plugin\"\nversion = \"0.1.0\"\n");
    f.write(
        "plugin/src/lib.rs",
        concat!(
            "pub struct LoadOutcome {\n",
            "    pub count: u32,\n",
            "}\n",
            "impl Default for LoadOutcome {\n",
            "    fn default() -> Self {\n",
            "        LoadOutcome { count: 0 }\n",
            "    }\n",
            "}\n",
        ),
    );
    f.write(
        "host/Cargo.toml",
        "[package]\nname = \"host\"\nversion = \"0.1.0\"\n\n[dependencies]\nplugin = { path = \"../plugin\" }\n",
    );
    f.write("host/src/lib.rs", "pub mod outcome;\npub mod user;\n");
    f.write("host/src/outcome.rs", "pub type LoadOutcome = plugin::LoadOutcome;\n");
    f.write(
        "host/src/user.rs",
        "use crate::outcome::LoadOutcome;\npub fn fresh() -> LoadOutcome {\n    LoadOutcome::default()\n}\n",
    );
    f.commit("a type alias into another crate");
    let r = f.trace(&["callers", "default", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(references_of(&v, "plugin/src/lib.rs::default", 5), extracted("host/src/user.rs", &[3]), "{v}");
}

#[test]
fn rust_impl_for_a_type_outside_the_repository_is_ambiguous() {
    let f = Fixture::new();
    f.write(
        "Cargo.toml",
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\nurl = \"2\"\n",
    );
    f.write(
        "src/lib.rs",
        concat!(
            "pub mod location;\n",
            "pub mod thread;\n",
            "use crate::location::Location;\n",
            "use crate::thread::ThreadSource;\n",
            "use url::Url;\n",
            "pub fn link(location: &Location) -> Url {\n",
            "    Url::from(location)\n",
            "}\n",
            "pub fn name(_: ThreadSource) -> String {\n",
            "    String::from(\"x\")\n",
            "}\n",
        ),
    );
    f.write(
        "src/location.rs",
        concat!(
            "use url::Url;\n",
            "pub struct Location;\n",
            "impl From<&Location> for Url {\n",
            "    fn from(_: &Location) -> Url {\n",
            "        Url::parse(\"https://example.com\").unwrap()\n",
            "    }\n",
            "}\n",
        ),
    );
    f.write(
        "src/thread.rs",
        "pub struct ThreadSource;\nimpl From<ThreadSource> for String {\n    fn from(_: ThreadSource) -> String {\n        String::new()\n    }\n}\n",
    );
    f.commit("impls for a dependency's type and a prelude type");
    let r = f.trace(&["callers", "from", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(references_of(&v, "src/location.rs::from", 4), ambiguous("src/lib.rs", &[7]), "{v}");
    assert_eq!(references_of(&v, "src/thread.rs::from", 3), ambiguous("src/lib.rs", &[10]), "{v}");
}

#[test]
fn rust_glob_outside_the_repository_proves_no_reference_external() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[workspace]\nmembers = [\"tui\", \"client\", \"corelib\"]\n");
    f.write(
        "tui/Cargo.toml",
        "[package]\nname = \"tui\"\nversion = \"0.1.0\"\n\n[dependencies]\nclient = { path = \"../client\" }\next = \"1\"\n",
    );
    f.write("tui/src/lib.rs", "pub(crate) use client::legacy;\nuse ext::prelude::*;\nmod user;\n");
    f.write("tui/src/user.rs", "use crate::legacy::config::Builder;\npub fn go() {\n    Builder::new();\n}\n");
    f.write(
        "client/Cargo.toml",
        "[package]\nname = \"client\"\nversion = \"0.1.0\"\n\n[dependencies]\ncorelib = { path = \"../corelib\" }\n",
    );
    f.write("client/src/lib.rs", "pub mod legacy {\n    pub use corelib::config;\n}\n");
    f.write("corelib/Cargo.toml", "[package]\nname = \"corelib\"\nversion = \"0.1.0\"\n");
    f.write("corelib/src/lib.rs", "pub mod config;\n");
    f.write("corelib/src/config.rs", "pub struct Builder;\nimpl Builder {\n    pub fn new() -> Self {\n        Builder\n    }\n}\n");
    f.commit("a re-exported module beside a glob over a dependency");
    let r = f.trace(&["callers", "new", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(
        references_of(&v, "corelib/src/config.rs::new", 3),
        vec![("tui/src/user.rs".to_string(), 3, "INFERRED".to_string())],
        "{v}"
    );
}

#[test]
fn rust_reexport_chain_across_crates_reaches_its_declaring_file() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[workspace]\nmembers = [\"tui\", \"client\", \"corelib\"]\n");
    f.write(
        "tui/Cargo.toml",
        "[package]\nname = \"tui\"\nversion = \"0.1.0\"\n\n[dependencies]\nclient = { path = \"../client\" }\n",
    );
    f.write("tui/src/lib.rs", "pub(crate) use client::legacy_core;\nmod picker;\n");
    f.write(
        "tui/src/picker.rs",
        "use crate::legacy_core::config::edit::ConfigEditsBuilder;\npub fn persist() {\n    ConfigEditsBuilder::new();\n}\n",
    );
    f.write(
        "client/Cargo.toml",
        "[package]\nname = \"client\"\nversion = \"0.1.0\"\n\n[dependencies]\ncorelib = { path = \"../corelib\" }\n",
    );
    f.write(
        "client/src/lib.rs",
        concat!(
            "pub mod legacy_core {\n",
            "    pub mod config {\n",
            "        pub use corelib::config::*;\n",
            "\n",
            "        pub mod edit {\n",
            "            pub use corelib::config::edit::*;\n",
            "        }\n",
            "    }\n",
            "}\n",
        ),
    );
    f.write("corelib/Cargo.toml", "[package]\nname = \"corelib\"\nversion = \"0.1.0\"\n");
    f.write("corelib/src/lib.rs", "pub mod config;\n");
    f.write("corelib/src/config/mod.rs", "pub mod edit;\npub struct Config;\n");
    f.write(
        "corelib/src/config/edit.rs",
        "pub struct ConfigEditsBuilder;\nimpl ConfigEditsBuilder {\n    pub fn new() -> Self {\n        ConfigEditsBuilder\n    }\n}\n",
    );
    f.commit("a module re-exported from another crate, then an inline module, then a glob");
    let r = f.trace(&["callers", "new", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(
        references_of(&v, "corelib/src/config/edit.rs::new", 3),
        vec![("tui/src/picker.rs".to_string(), 3, "EXTRACTED".to_string())],
        "{v}"
    );
}

#[test]
fn rust_member_call_on_a_parameter_of_a_type_outside_the_repository_stays_ambiguous() {
    let f = Fixture::new();
    f.write(
        "Cargo.toml",
        "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\naxum = \"0.7\"\n",
    );
    f.write(
        "src/lib.rs",
        concat!(
            "pub mod screen;\n",
            "use axum::middleware::Next;\n",
            "pub async fn middleware(next: Next) {\n",
            "    next.run();\n",
            "}\n",
        ),
    );
    f.write("src/screen.rs", "pub struct Screen;\nimpl Screen {\n    pub fn run(&self) {}\n}\n");
    f.commit("a member call on a parameter of a dependency's type");
    let r = f.trace(&["callers", "run", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(references_of(&v, "src/screen.rs::run", 3), ambiguous("src/lib.rs", &[4]), "{v}");
}

#[test]
fn rust_type_path_reaches_an_impl_in_another_crate() {
    let f = Fixture::new();
    f.write("status/Cargo.toml", "[package]\nname = \"status\"\nversion = \"0.1.0\"\n");
    f.write("status/src/lib.rs", "pub struct Status;\n");
    f.write(
        "mcp/Cargo.toml",
        "[package]\nname = \"mcp\"\nversion = \"0.1.0\"\n\n[dependencies]\nstatus = { path = \"../status\" }\n",
    );
    f.write(
        "mcp/src/lib.rs",
        "use status::Status;\npub struct Auth;\nimpl From<Auth> for Status {\n    fn from(_: Auth) -> Status {\n        Status\n    }\n}\n",
    );
    f.write(
        "app/Cargo.toml",
        concat!(
            "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\n",
            "mcp = { path = \"../mcp\" }\nstatus = { path = \"../status\" }\n",
        ),
    );
    f.write(
        "app/src/main.rs",
        "use mcp::Auth;\nuse status::Status;\nfn main() {\n    let _ = Status::from(Auth);\n}\n",
    );
    f.commit("an impl in a crate other than its type's");
    let r = f.trace(&["callers", "from", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(references_of(&v, "mcp/src/lib.rs::from", 4), extracted("app/src/main.rs", &[4]), "{v}");
}

#[test]
fn rust_type_path_resolves_once_per_file() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write(
        "src/lib.rs",
        concat!(
            "pub mod thing;\n",
            "use crate::thing::Thing;\n",
            "pub fn first() -> Thing {\n",
            "    Thing::new()\n",
            "}\n",
            "pub fn second() -> Thing {\n",
            "    Thing::new()\n",
            "}\n",
        ),
    );
    f.write("src/thing.rs", "pub struct Thing;\nimpl Thing {\n    pub fn new() -> Thing {\n        Thing\n    }\n}\n");
    f.commit("one type path written twice in one file");
    let r = f.trace_env(&["callers", "new", "--json"], &[("TRACE_TIMING", "1")]);
    r.ok();
    let phases = |name: &str| r.stderr.lines().filter(|line| line.starts_with(&format!("timing {name} "))).count();
    assert_eq!((phases("resolve_type"), phases("own_types")), (1, 1), "{}", r.stderr);
    assert_eq!(references_of(&r.view(), "src/thing.rs::new", 3), extracted("src/lib.rs", &[4, 7]));
}

#[test]
fn callers_names_every_calling_declaration_of_one_file_from_one_resolution() {
    let f = Fixture::new();
    f.write("src/api.ts", "export function buildUrl(path: string): string {\n  return path;\n}\n");
    f.write(
        "src/client.ts",
        concat!(
            "import { buildUrl } from \"./api\";\n",
            "export function users(): string {\n",
            "  return buildUrl(\"users\");\n",
            "}\n",
            "export function posts(): string {\n",
            "  return buildUrl(\"posts\");\n",
            "}\n",
        ),
    );
    f.commit("two calling functions in one file");
    let r = f.trace_env(&["callers", "buildUrl", "--json"], &[("TRACE_TIMING", "1")]);
    r.ok();
    let resolutions = r.stderr.lines().filter(|line| line.starts_with("timing use_sites ")).count();
    assert_eq!(resolutions, 1, "{}", r.stderr);
    let rows: Vec<(String, i64, String)> = r.view()["results"][0]["callers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|caller| {
            (
                caller["source_file"].as_str().unwrap().to_string(),
                caller["source_line"].as_i64().unwrap(),
                caller["declaration"]["name"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect();
    assert_eq!(
        rows,
        vec![
            ("src/client.ts".to_string(), 3, "users".to_string()),
            ("src/client.ts".to_string(), 6, "posts".to_string()),
        ]
    );
}

#[test]
fn adding_a_rust_crate_root_reabsorbs_no_file_of_another_language() {
    let f = Fixture::new();
    f.write("Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.1.0\"\n");
    f.write("src/main.rs", "mod tools;\nuse crate::tools::measure;\nfn main() {\n    measure();\n}\n");
    f.write("src/tools.rs", "pub fn measure() {}\n");
    f.write("notes.py", "def jot():\n    pass\n");
    f.commit("rust beside python");
    f.trace(&["callers", "measure"]).ok();
    let timing = [("TRACE_TIMING", "1")];
    let structure = f.trace_env(&["structure", "notes.py"], &timing);
    structure.ok();
    let decoded = |stderr: &str| -> Vec<String> {
        stderr
            .lines()
            .filter_map(|line| line.strip_prefix("timing decode "))
            .filter_map(|rest| rest.split_whitespace().next())
            .filter(|key| key.len() == 64 && key.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .map(str::to_string)
            .collect()
    };
    let notes = decoded(&structure.stderr);
    assert_eq!(notes.len(), 1, "{}", structure.stderr);
    f.write("src/bin/x.rs", "fn main() {}\n");
    let added = f.trace_env(&["callers", "measure", "--json"], &timing);
    added.ok();
    assert!(!decoded(&added.stderr).contains(&notes[0]), "{}", added.stderr);
    assert_eq!(caller_rows(&added.view(), "src/tools.rs::measure"), extracted("src/main.rs", &[4]));
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
    assert_eq!(r.view()["definitions"].as_i64().unwrap(), 1);
    let r = f.trace(&["defines", "AddItem", "--json"]);
    r.ok();
    assert_eq!(r.view()["definitions"].as_i64().unwrap(), 1);
    let r = f.trace(&["defines", "Cart", "--json"]);
    r.ok();
    assert_eq!(r.view()["definitions"].as_i64().unwrap(), 1);
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
    let v = r.view();
    // `util.Helper(1)` is a package-qualified call → Free shape (Go's
    // dominant cross-file edge); it resolves to the free function `Helper`,
    // sourced from the calling function `First` at the use site.
    let ids: Vec<String> = symbol(&v, "util.go::Helper")["callers"]
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
    assert_eq!(r.view()["definitions"].as_i64().unwrap(), 1);
    let r = f.trace(&["defines", "add_item", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(v["definitions"].as_i64().unwrap(), 1);
    assert_eq!(def_files(&v), vec![("cart.rb".to_string(), 2)]);
}

#[test]
fn ruby_member_call_collision_is_ambiguous() {
    // Two classes with a same-named method `run`, an unqualified receiver
    // call `obj.run` — the member-call ambiguity, the only residual one.
    let f = Fixture::new();
    f.write("job.rb", "class Job\n  def run\n    1\n  end\nend\n");
    f.write("task.rb", "class Task\n  def run\n    2\n  end\nend\n");
    f.write("caller.rb", "def go(obj)\n  obj.run\nend\n");
    f.commit("ruby member collision");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "run", "--json"]);
    r.ok();
    let v = r.view();
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
fn ruby_member_call_on_a_value_resolves_only_through_an_import() {
    let f = Fixture::new();
    f.write(
        "cart.rb",
        concat!(
            "class Cart\n",
            "  def add\n",
            "  end\n",
            "\n",
            "  def fill\n",
            "    self.add\n",
            "  end\n",
            "\n",
            "  def self.build\n",
            "    new\n",
            "  end\n",
            "end\n",
        ),
    );
    f.write(
        "shop.rb",
        "def put(list)\n  list.add(1)\nend\n\ndef open\n  Cart.build\nend\n",
    );
    f.commit("ruby value receivers");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "add", "--json"]);
    r.ok();
    let mut rows = caller_rows(&r.view(), "cart.rb::add");
    rows.sort();
    assert_eq!(
        rows,
        vec![
            ("cart.rb".to_string(), 6, "EXTRACTED".to_string()),
            ("shop.rb".to_string(), 2, "AMBIGUOUS".to_string()),
        ]
    );
    let r = f.trace(&["callers", "build", "--json"]);
    r.ok();
    assert_eq!(
        caller_rows(&r.view(), "cart.rb::build"),
        vec![("shop.rb".to_string(), 6, "INFERRED".to_string())]
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
    assert_eq!(r.view()["definitions"].as_i64().unwrap(), 1);
    let r = f.trace(&["defines", "addItem", "--json"]);
    r.ok();
    let v = r.view();
    assert_eq!(v["definitions"].as_i64().unwrap(), 1);
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
    let v = r.view();
    let rows = caller_rows(&v, "Cart.java::Cart");
    assert!(
        rows.iter().any(|(file, _, _)| file == "Factory.java"),
        "new Cart() must resolve to the Cart type from Factory.java: {:?}",
        rows
    );
}

#[test]
fn java_member_call_on_a_value_resolves_only_through_an_import() {
    let f = Fixture::new();
    f.write(
        "Cart.java",
        concat!(
            "public class Cart {\n",
            "  void add() {}\n",
            "  void fill() { this.add(); }\n",
            "  void refill() { add(); }\n",
            "  static Cart make() { return new Cart(); }\n",
            "}\n",
        ),
    );
    f.write(
        "Shop.java",
        concat!(
            "public class Shop {\n",
            "  void put(java.util.List<Integer> list) { list.add(1); }\n",
            "  Cart open() { return Cart.make(); }\n",
            "}\n",
        ),
    );
    f.commit("java value receivers");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "add", "--json"]);
    r.ok();
    let mut rows = caller_rows(&r.view(), "Cart.java::add");
    rows.sort();
    assert_eq!(
        rows,
        vec![
            ("Cart.java".to_string(), 3, "EXTRACTED".to_string()),
            ("Cart.java".to_string(), 4, "EXTRACTED".to_string()),
            ("Shop.java".to_string(), 2, "AMBIGUOUS".to_string()),
        ]
    );
    let r = f.trace(&["callers", "make", "--json"]);
    r.ok();
    assert_eq!(
        caller_rows(&r.view(), "Cart.java::make"),
        vec![("Shop.java".to_string(), 3, "INFERRED".to_string())]
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
    let v = r.view();
    assert_eq!(v["definitions"].as_i64().unwrap(), 1);
    assert_eq!(def_files(&v), vec![("lib.c".to_string(), 2)]);
    let r = f.trace(&["defines", "Point", "--json"]);
    r.ok();
    assert_eq!(r.view()["definitions"].as_i64().unwrap(), 1);
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
    f.write("caller.c", "int go(void) {\n    return compute(1);\n}\n");
    f.commit("c free collision");
    f.trace(&["cache", "build", "."]).ok();
    let r = f.trace(&["callers", "compute", "--json"]);
    r.ok();
    let v = r.view();
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
    let v = r.view();
    let ids: Vec<String> = symbol(&v, "util.c::lone_unique_c")["callers"]
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
    let v = r.view();
    let rows_go = caller_rows(&v, "handler.go::process");
    assert!(
        !rows_go.iter().any(|(file, _, _)| file == "lib.rs"),
        "a Rust process() must never resolve onto the Go process function: {:?}",
        rows_go
    );
}

#[test]
fn php_psr4_roots_resolve_imports_and_exclude_external_names() {
    let f = Fixture::new();
    f.write(
        "composer.json",
        r#"{"autoload":{"psr-4":{"App\\":"src/"}},"autoload-dev":{"psr-4":{"Tests\\Lib\\":"tests/lib/"}}}"#,
    );
    f.write(
        "src/Billing/Cart.php",
        "<?php\nnamespace App\\Billing; class Cart {}\n",
    );
    f.write(
        "src/Billing/Order.php",
        "<?php\nnamespace App\\Billing; class Order {}\n",
    );
    f.write(
        "tests/lib/Case.php",
        "<?php\nnamespace Tests\\Lib; class Case {}\n",
    );
    f.write("src/Cache.php", "<?php\nnamespace App; class Cache {}\n");
    f.write(
        "app.php",
        "<?php\nuse App\\Billing\\{Cart, Order};\nuse App\\Billing\\Cart as C;\nuse Tests\\Lib\\Case;\nuse Illuminate\\Support\\Facades\\Cache;\nnew Cart(); new Order(); new Case(); new C();\n",
    );
    f.commit("PHP PSR-4 roots");
    f.trace(&["cache", "build", "."]).ok();

    let info = f.trace(&["info", "app.php", "--json"]);
    info.ok();
    let document = info.json();
    let dependencies = &document["context"]["files"].as_object().unwrap().values().next().unwrap()["dependencies"];
    let resolved: Vec<(&str, &str)> = dependencies
        .as_array()
        .unwrap_or_else(|| panic!("dependencies missing: {dependencies}"))
        .iter()
        .map(|dependency| {
            (
                dependency["module"].as_str().unwrap_or(""),
                dependency["confidence"].as_str().unwrap_or(""),
            )
        })
        .collect();
    assert!(
        resolved.contains(&("src/Billing/Cart", "EXTRACTED")),
        "{resolved:?}"
    );
    assert!(
        resolved.contains(&("src/Billing/Order", "EXTRACTED")),
        "{resolved:?}"
    );
    assert!(
        resolved.contains(&("tests/lib/Case", "EXTRACTED")),
        "{resolved:?}"
    );
    assert!(
        resolved.iter().all(|(module, _)| *module != "src/Cache"),
        "{resolved:?}"
    );

    let structure = f.trace(&["structure", "app.php", "--json"]);
    structure.ok();
    let structure = structure.view();
    let imports = &structure["imports"];
    assert!(
        imports
            .as_array()
            .is_some_and(|imports| imports.iter().any(|import| {
                import["module"].as_str() == Some("App\\Billing")
                    && import["symbol"].as_str() == Some("Cart")
            })),
        "alias import must retain Cart: {imports}"
    );
    assert!(
        imports.as_array().is_some_and(|imports| imports
            .iter()
            .all(|import| { import["symbol"].as_str() != Some("C") })),
        "alias must not replace the imported symbol: {imports}"
    );
}

#[test]
fn composer_change_reresolves_warmed_php_imports() {
    let f = Fixture::new();
    f.write(
        "composer.json",
        r#"{"autoload":{"psr-4":{"App\\":"src/"}}}"#,
    );
    f.write(
        "src/Billing/Cart.php",
        "<?php\nnamespace App\\Billing; class Cart {}\n",
    );
    f.write(
        "lib/Billing/Cart.php",
        "<?php\nnamespace App\\Billing; class Cart {}\n",
    );
    f.write("app.php", "<?php\nuse App\\Billing\\Cart;\nnew Cart();\n");
    f.commit("first Composer root");

    let dependency = |fixture: &Fixture| {
        let run = fixture.trace(&["info", "app.php", "--json"]);
        run.ok();
        let document = run.json();
        document["context"]["files"].as_object().unwrap().values().next().unwrap()["dependencies"].clone()
    };
    assert!(dependency(&f)
        .as_array()
        .is_some_and(|dependencies| dependencies
            .iter()
            .any(|dependency| { dependency["module"].as_str() == Some("src/Billing/Cart") })));

    f.write(
        "composer.json",
        r#"{"autoload":{"psr-4":{"App\\":"lib/"}}}"#,
    );
    f.commit("second Composer root");
    let dependencies = dependency(&f);
    assert!(
        dependencies
            .as_array()
            .is_some_and(|dependencies| dependencies.iter().any(|dependency| {
                dependency["module"].as_str() == Some("lib/Billing/Cart")
                    && dependency["confidence"].as_str() == Some("EXTRACTED")
            })),
        "Composer root change did not re-resolve warmed importer: {dependencies}"
    );
}

#[test]
fn typescript_paths_resolve_files_barrels_and_dynamic_imports() {
    let f = Fixture::new();
    f.write(
        "tsconfig.json",
        r#"{"compilerOptions":{"baseUrl":".","paths":{"@/*":["./web/*"],"@shell":["./web/shell/Frame.tsx"]}}}"#,
    );
    f.write("web/lib/a.ts", "export const A = 1;\n");
    f.write("web/lib/x.ts", "export const x = 1;\n");
    f.write("web/pages/p.ts", "export const p = 1;\n");
    f.write("web/shell/Frame.tsx", "export const Frame = () => null;\n");
    f.write(
        "web/form-user.ts",
        "import { form } from '@/components/form'; void form;\n",
    );
    f.write(
        "web/main.ts",
        "export { A } from '@/lib/a';\nimport('@/pages/p');\nimport { x } from '@/lib/x';\nimport { Frame } from '@shell';\nvoid x; void Frame;\n",
    );
    f.commit("TypeScript paths");
    f.trace(&["cache", "build", "."]).ok();

    let info = f.trace(&["info", "web/main.ts", "--json"]);
    info.ok();
    let document = info.json();
    let dependencies = &document["context"]["files"].as_object().unwrap().values().next().unwrap()["dependencies"];
    let resolved: Vec<(&str, &str)> = dependencies
        .as_array()
        .unwrap_or_else(|| panic!("dependencies missing: {dependencies}"))
        .iter()
        .map(|dependency| {
            (
                dependency["module"].as_str().unwrap_or(""),
                dependency["confidence"].as_str().unwrap_or(""),
            )
        })
        .collect();
    for module in ["web/lib/a", "web/lib/x", "web/pages/p", "web/shell/Frame"] {
        assert!(
            resolved.contains(&(module, "EXTRACTED")),
            "{module} missing from {resolved:?}"
        );
    }

    f.write("web/components/form/index.ts", "export const form = 1;\n");
    f.commit("add TypeScript barrel");
    let barrel = f.trace(&["info", "web/form-user.ts", "--json"]);
    barrel.ok();
    let document = barrel.json();
    let dependencies = &document["context"]["files"].as_object().unwrap().values().next().unwrap()["dependencies"];
    assert!(
        dependencies
            .as_array()
            .unwrap_or_else(|| panic!("dependencies missing: {dependencies}"))
            .iter()
            .any(|dependency| {
                dependency["module"].as_str() == Some("web/components/form/index")
                    && dependency["confidence"].as_str() == Some("EXTRACTED")
            }),
        "{dependencies}"
    );
}

#[test]
fn exact_typescript_import_replaces_an_ambiguous_alias_target() {
    let f = Fixture::new();
    f.write(
        "tsconfig.json",
        r#"{"compilerOptions":{"paths":{"@/*":["first/*","second/*"]}}}"#,
    );
    f.write("first/target.ts", "export const target = 1;\n");
    f.write("second/target.ts", "export const target = 2;\n");
    f.write(
        "entry.ts",
        "import { target as ambiguous } from '@/target';\nimport { target } from './first/target';\nvoid ambiguous; void target;\n",
    );
    f.commit("ambiguous alias then exact TypeScript import");

    let info = f.trace(&["info", "entry.ts", "--json"]);
    info.ok();
    let document = info.json();
    let dependencies = &document["context"]["files"].as_object().unwrap().values().next().unwrap()["dependencies"];
    assert!(
        dependencies
            .as_array()
            .is_some_and(|dependencies| dependencies.iter().any(|dependency| {
                dependency["module"].as_str() == Some("first/target")
                    && dependency["confidence"].as_str() == Some("EXTRACTED")
            })),
        "the exact import must replace ambiguity for first/target: {dependencies}"
    );
}

#[test]
fn properties_define_without_callers_and_abstract_classes_resolve_statically() {
    let f = Fixture::new();
    f.write("types.ts", "abstract class A {}\ninterface Props { a: string }\ninterface Ctx { save: () => void }\ninterface Repo { find(): T }\ntype P = { onClick: () => void }\ninterface Marker {}\ninterface Box extends A, B {}\ntype U = A | B\n");
    f.write("use.ts", "new A();\n");
    f.write("model.php", "<?php\nclass Model {\n    #[Field(label: 'x')] public string $current;\n    public function current() {}\n}\n");
    f.commit("declaration kinds");

    let r = f.trace(&["callers", "A", "--json"]);
    r.ok();
    assert!(caller_rows(&r.view(), "types.ts::A")
        .iter()
        .any(|(file, _, _)| file == "use.ts"));

    let r = f.trace(&["defines", "$current", "--json"]);
    r.ok();
    assert_eq!(def_files(&r.view()), vec![("model.php".to_string(), 3)]);
    let r = f.trace(&["defines", "current", "--json"]);
    r.ok();
    assert_eq!(def_files(&r.view()), vec![("model.php".to_string(), 4)]);
    let r = f.trace(&["callers", "$current"]);
    r.code_is(2);
    assert!(r
        .combined()
        .contains("a property has readers, not callers; tracer does not extract reads"));
}

#[test]
fn php_declaration_annotations_belong_to_their_declaration() {
    let f = Fixture::new();
    f.write(
        "model.php",
        "<?php\n/** @internal since 2.0 */\n#[Entity]\nclass Model {\n    #[Action] public function save() {}\n    #[Field(label: 'Owner\\'s inbox', options: App\\Options::class)] public string $inbox;\n}\n",
    );
    f.commit("PHP declaration annotations");

    let r = f.trace(&["structure", "model.php", "--json"]);
    r.ok();
    let view = r.view();
    let model = view["symbols_by_kind"]["class"]
        .as_array()
        .unwrap()
        .iter()
        .find(|symbol| symbol["name"] == "Model")
        .unwrap();
    assert_eq!(
        model["annotations"],
        serde_json::json!(["@internal", "Entity"]),
        "{model}"
    );
    assert!(
        model["annotations"]
            .as_array()
            .is_some_and(|annotations| annotations.iter().all(|annotation| annotation != "Action")),
        "{model}"
    );
    let inbox = view["symbols_by_kind"]["property"]
        .as_array()
        .unwrap()
        .iter()
        .find(|symbol| symbol["name"] == "$inbox")
        .unwrap();
    assert_eq!(inbox["annotations"][0], "Field", "{inbox}");
}

#[test]
fn python_protocols_and_decorators_have_declaration_metadata() {
    let f = Fixture::new();
    f.write(
        "types.py",
        "from abc import ABC, ABCMeta\nfrom typing import Callable, Protocol\n\n@injectable()\nclass Service:\n    pass\n\nclass Shape(Protocol):\n    name: str\n\nclass Contract(Protocol):\n    save: Callable[[], None]\n\nclass AbstractBase(ABC):\n    pass\n\nclass Meta(metaclass=ABCMeta):\n    pass\n",
    );
    f.commit("Python declaration metadata");

    let r = f.trace(&["structure", "types.py", "--json"]);
    r.ok();
    let view = r.view();
    let classes = view["symbols_by_kind"]["class"].as_array().unwrap();
    let service = classes
        .iter()
        .find(|symbol| symbol["name"] == "Service")
        .unwrap();
    assert_eq!(
        service["annotations"],
        serde_json::json!(["injectable"]),
        "{service}"
    );
}

#[test]
fn typescript_member_decorators_stay_with_their_member() {
    let f = Fixture::new();
    f.write(
        "members.ts",
        "class Members {\n    @A()\n    first() {}\n    second() {}\n    @B()\n    third() {}\n}\n",
    );
    f.commit("TypeScript member decorators");

    let r = f.trace(&["structure", "members.ts", "--json"]);
    r.ok();
    let view = r.view();
    let functions = view["symbols_by_kind"]["function"].as_array().unwrap();
    let first = functions
        .iter()
        .find(|symbol| symbol["name"] == "first")
        .unwrap();
    let second = functions
        .iter()
        .find(|symbol| symbol["name"] == "second")
        .unwrap();
    let third = functions
        .iter()
        .find(|symbol| symbol["name"] == "third")
        .unwrap();
    assert_eq!(first["annotations"], serde_json::json!(["A"]), "{first}");
    assert_eq!(second["annotations"], serde_json::json!([]), "{second}");
    assert_eq!(third["annotations"], serde_json::json!(["B"]), "{third}");
}

#[test]
fn structure_carries_headers_for_remaining_languages_and_ctags() {
    let f = Fixture::new();
    f.write("sample.py", "from dataclasses import dataclass\n\n@dataclass(frozen=True)\nclass Point:\n    x: int = 1\n\n    async def fetch(self, url: str) -> bytes:\n        return url.encode()\n");
    f.write("Point.java", "import java.io.IOException;\nimport java.util.List;\n\npublic record Point(int x, int y) {\n    private static final int MAX = 3, MIN = 0;\n\n    public <T> List<T> load(String id) throws IOException {\n        return null;\n    }\n}\n");
    f.write("reader.go", "package storage\n\ntype (\n    Reader struct {\n        data []byte\n    }\n)\n\ntype Writer interface {\n    Write(p []byte) (int, error)\n}\n\nconst (\n    Busy = iota\n    Idle\n)\n\nfunc (r *Reader) Read(p []byte) (int, error) {\n    return 0, nil\n}\n");
    f.write("reader.rs", "#[derive(Debug)]\npub struct Reader<T> {\n    value: T,\n}\n\nimpl<T> Reader<T>\nwhere\n    T: Clone,\n{\n    pub fn read(&self, value: T) -> T {\n        value\n    }\n}\n\nimpl std::fmt::Display for Reader<()> {\n    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {\n        Ok(())\n    }\n}\n\nenum Result<T> {\n    Some(T),\n}\n\npub const MAX: usize = 3;\n");
    f.write("reader.rb", "class Reader\n  attr_reader :name\n\n  private\n\n  def read(value)\n    value\n  end\n\n  def self.open(path)\n    new\n  end\n\n  def size = @data.size\nend\n");
    f.write("reader.c", "int (*handler)(int, char *);\ntypedef struct {\n    int x;\n} Point;\n\nstatic int count(const char *s) {\n    return 0;\n}\n\nenum Color {\n    RED,\n};\n\nstruct Node {\n    struct Node *next;\n};\n");
    f.write("deploy.sh", "deploy() {\n  cat >&2 <<EOF\ndeploy\nEOF\n}\n");
    f.write(
        "declarations.ts",
        "export function* walkPersistedElements(state: number): Generator<number> {\n    yield state;\n}\n\nexport const first = 1, second = 2;\nexport const handlers = { onClick: (event: Event): void => console.log(event) };\n",
    );
    f.write(
        "package.json",
        "{\"scripts\": {\"build\": \"cargo build\"}}\n",
    );
    f.write("README.md", "# Tracer\n\n## Cache\n");
    f.commit("remaining declaration headers");

    for (path, expected_headers) in [
        (
            "sample.py",
            vec![
                "@dataclass(frozen=True)\nclass Point: …",
                "x: int = 1",
                "async def fetch(self, url: str) -> bytes: …",
            ],
        ),
        (
            "Point.java",
            vec![
                "public record Point(int x, int y) { … }",
                "private static final int MAX = 3;",
                "public <T> List<T> load(String id) throws IOException { … }",
            ],
        ),
        (
            "reader.go",
            vec![
                "package storage",
                "type Reader struct { … }",
                "data []byte",
                "Write(p []byte) (int, error)",
                "type Writer interface { … }",
                "const Busy = iota",
                "const Idle",
                "func (r *Reader) Read(p []byte) (int, error) { … }",
            ],
        ),
        (
            "declarations.ts",
            vec![
                "export function* walkPersistedElements(state: number): Generator<number> { … }",
                "export const first = 1;",
                "export const second = 2;",
                "export const handlers = { onClick: (event: Event): void => … };",
            ],
        ),
        (
            "reader.rs",
            vec![
                "#[derive(Debug)]\npub struct Reader<T> { … }",
                "impl<T> Reader<T>\nwhere\n    T: Clone, { … }",
                "pub fn read(&self, value: T) -> T { … }",
                "impl std::fmt::Display for Reader<()> { … }",
                "Some(T)",
                "pub const MAX: usize = 3;",
            ],
        ),
        (
            "reader.rb",
            vec![
                "class Reader { … }",
                "attr_reader :name",
                "private",
                "def read(value) { … }",
                "def self.open(path) { … }",
                "def size = …",
            ],
        ),
        (
            "reader.c",
            vec![
                "int (*handler)(int, char *);",
                "typedef struct { … } Point;",
                "static int count(const char *s) { … }",
                "enum Color { … }",
                "RED,",
                "struct Node *next;",
            ],
        ),
        ("deploy.sh", vec!["deploy()"]),
    ] {
        let result = f.trace(&["structure", path, "--json"]);
        result.ok();
        let view = result.view();
        let records = view["symbols_by_kind"]
            .as_object()
            .unwrap()
            .values()
            .flat_map(|rows| rows.as_array().into_iter().flatten())
            .collect::<Vec<_>>();
        for expected_header in expected_headers {
            assert!(
                records
                    .iter()
                    .any(|candidate| candidate["header"] == expected_header),
                "missing {path} row `{expected_header}`: {records:#?}"
            );
        }
        if path == "deploy.sh" {
            assert!(
                records
                    .iter()
                    .all(|candidate| candidate["kind"] != "heredoc"),
                "heredocs are shell syntax, not declarations: {records:#?}"
            );
        }
    }

    let generator = f.trace(&["defines", "walkPersistedElements"]);
    generator.ok();
    assert!(
        generator
            .stdout
            .contains("export function* walkPersistedElements(state: number): Generator<number> { … }"),
        "generator definition did not retain its header: {}",
        generator.stdout
    );

    let package = f.trace(&["structure", "package.json", "--json"]);
    package.ok();
    assert_eq!(
        package.view()["symbols"],
        0,
        "JSON keys are not declarations: {}",
        package.stdout
    );
    let readme = f.trace(&["structure", "README.md", "--json"]);
    readme.ok();
    assert!(
        readme.view()["symbols"].as_i64().unwrap_or_default() > 0,
        "Markdown headings remain declarations: {}",
        readme.stdout
    );

    f.trace(&["context", "sample.py"]).ok();
    for args in [["context", "sample.py"], ["read", "deploy.sh"]] {
        let warm = f.trace_env(&args, &[("TRACE_TIMING", "1")]);
        warm.ok();
        assert!(
            !warm.stderr.contains("timing ctags"),
            "warm {args:?} spawned ctags:\n{}",
            warm.stderr
        );
    }
}

#[test]
fn structure_nests_python_declarations_under_the_nearest_definition() {
    let f = Fixture::new();
    f.write(
        "nested.py",
        "def outer(value: int) -> int:\n    def inner(offset: int) -> int:\n        return value + offset\n    class Result:\n        def get(self) -> int:\n            return inner(1)\n    return Result().get()\n",
    );
    f.commit("Python nested declarations");

    let result = f.trace(&["structure", "nested.py"]);
    result.ok();
    assert!(
        result.stdout.contains("L2      def inner(offset: int) -> int: …")
            && result.stdout.contains("L4      class Result: …")
            && result.stdout.contains("L5        def get(self) -> int: …"),
        "nested Python declarations did not retain their lexical parents:\n{}",
        result.stdout
    );
}

#[test]
fn rust_attribute_stack_keeps_the_first_header_line() {
    let f = Fixture::new();
    f.write(
        "reader.rs",
        "#[derive(Debug)]\n#[repr(C)]\npub struct Reader {\n    value: u8,\n}\n",
    );
    f.commit("stacked Rust attributes");

    let structure = f.trace(&["structure", "reader.rs", "--json"]);
    structure.ok();
    let view = structure.view();
    let reader = view["symbols_by_kind"]["class"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["name"] == "Reader")
        .unwrap();
    assert_eq!(reader["header_line"], 1, "{}", structure.stdout);
    assert_eq!(
        reader["header"],
        "#[derive(Debug)]\n#[repr(C)]\npub struct Reader { … }",
        "{}",
        structure.stdout
    );

    let read = f.trace(&["read", "reader.rs", "--lines", "1:1"]);
    read.ok();
    assert!(
        read.stdout
            .contains("L3    #[derive(Debug)]\n      #[repr(C)]\n      pub struct Reader { … }"),
        "{}",
        read.stdout
    );
}

#[test]
fn rust_comment_after_an_attribute_keeps_the_attribute_in_the_header() {
    let f = Fixture::new();
    f.write(
        "cart.rs",
        "struct Cart;\nimpl Cart {\n    #[must_use] // note\n    pub fn total(&self) -> u32 {\n        0\n    }\n}\n",
    );
    f.commit("comment after a Rust attribute");

    let structure = f.trace(&["structure", "cart.rs", "--json"]);
    structure.ok();
    let view = structure.view();
    let total = view["symbols_by_kind"]["function"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["name"] == "total")
        .unwrap();
    assert_eq!(total["header_line"], 3, "{}", structure.stdout);
    assert_eq!(
        total["header"],
        "#[must_use] pub fn total(&self) -> u32 { … }",
        "{}",
        structure.stdout
    );
}

#[test]
fn go_interface_header_uses_the_ast_body_boundary() {
    let f = Fixture::new();
    f.write(
        "writer.go",
        "package storage\ntype Writer interface /* { */ {\n    Write(p []byte) (int, error)\n}\n",
    );
    f.commit("Go interface comment brace");

    let result = f.trace(&["structure", "writer.go", "--json"]);
    result.ok();
    let view = result.view();
    let writer = view["symbols_by_kind"]["interface"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["name"] == "Writer")
        .unwrap();
    assert_eq!(
        writer["header"],
        "type Writer interface /* { */ { … }",
        "{}",
        result.stdout
    );
}

#[test]
fn same_line_declarations_keep_source_order() {
    let f = Fixture::new();
    f.write("generators.ts", "export const first = 1, second = 2;\n");
    f.commit("same-line TypeScript declarations");

    let result = f.trace(&["structure", "generators.ts", "--json"]);
    result.ok();
    let view = result.view();
    let constants = view["symbols_by_kind"]["constant"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["name"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(constants, vec!["first", "second"], "{}", result.stdout);
}

#[test]
fn rust_where_clause_keeps_its_trailing_comma() {
    let f = Fixture::new();
    f.write("reader.rs", "impl<T> Reader<T>\nwhere\n    T: Clone,\n{\n}\n");
    f.commit("Rust where comma");
    let result = f.trace(&["structure", "reader.rs", "--json"]);
    result.ok();
    let view = result.view();
    assert_eq!(view["symbols_by_kind"]["impl"][0]["header"], "impl<T> Reader<T>\nwhere\n    T: Clone, { … }");
}

#[test]
fn ctags_rows_keep_their_body_extent_and_qualified_scope() {
    let f = Fixture::new();
    f.write(
        "headings.md",
        "# Overview\n\n## Runtime\n\n### Storage\n\n## Usage\n",
    );
    f.write(
        "functions.sh",
        "normalize_path() {\n    local path=$1\n    printf '%s\\n' \"$path\"\n}\n\nother() {\n    true\n}\n",
    );
    f.commit("ctags extent and scope");

    let headings = f.trace(&["structure", "headings.md"]);
    headings.ok();
    assert!(
        headings.stdout.contains("L1    # Overview")
            && headings.stdout.contains("L3      ## Runtime")
            && headings.stdout.contains("L5        ### Storage"),
        "Markdown headings did not retain their parent chain:\n{}",
        headings.stdout
    );

    let context = f.trace(&["context", "functions.sh", "--offset", "2", "--limit", "1"]);
    context.ok();
    assert!(
        context.stdout.contains("L1    normalize_path()"),
        "the shell function did not own its body window:\n{}",
        context.stdout
    );
}
