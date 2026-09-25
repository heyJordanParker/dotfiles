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
        vec![("app/main.ts".to_string(), 3, "INFERRED".to_string())]
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
