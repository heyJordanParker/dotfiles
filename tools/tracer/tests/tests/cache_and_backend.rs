//! Cache lifecycle and the CCN backend.
//!
//! Covers: cold build, warm reuse, invalidation on content change, the
//! `file` namespace, `cache clear` (scoped, all), `cache stats` (human +
//! json), and the single AST CCN backend — CCN is AST-derived regardless of
//! the `TRACER_CCN_BACKEND` value, and the cache does not fork on that value.

use sha2::{Digest, Sha256};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use tracer_cli_tests::{parse_stats_table, standard_repo, Fixture};

/// The published on-disk cache key (tracer Claude.md + cache.rs module
/// header): sha256("v{SCHEMA}|ccn:ast\0" + file_bytes + "\0" + relpath),
/// hex-encoded. Reconstructed here from the documented formula — not from
/// tracer internals — so the schema-version-invalidation test can plant an
/// entry under one schema's key and prove it is unreachable under another.
fn file_cache_key(schema_version: u32, file_bytes: &[u8], relpath: &str) -> String {
    let mut h = Sha256::new();
    h.update(format!("v{schema_version}|ccn:ast\0").as_bytes());
    h.update(file_bytes);
    h.update(b"\0");
    h.update(relpath.as_bytes());
    hex::encode(h.finalize())
}

/// SCHEMA_VERSION as published in the tracer Claude.md / cache.rs. The
/// schema-bump test plants a poison entry at this version's key (proving
/// the cache IS consulted by this exact schema-versioned key) and at a
/// neighbor version's key (proving it is unreachable).
const PUBLISHED_SCHEMA_VERSION: u32 = 18;

#[test]
fn typescript_import_bindings_are_cached_without_changing_structure_output() {
    let f = Fixture::new();
    let source = "import './side-effect';\nimport ordinary from './ordinary';\nimport * as toolkit from './toolkit';\nimport primary, * as suite from './mixed';\nimport { remote as local, direct } from './named';\nrequire('./legacy');\n";
    f.write("app.ts", source);
    f.commit("TypeScript import forms");

    let structure = f.trace(&["structure", "app.ts", "--json"]);
    structure.ok();
    let structure = structure.view();
    let public_imports = structure["imports"].as_array().unwrap();
    assert_eq!(
        public_imports.len(),
        8,
        "existing import rows changed: {structure:#}"
    );
    assert!(
        public_imports
            .iter()
            .all(|import| import.get("locals").is_none()),
        "the internal binding leaked into structure output: {structure:#}"
    );

    let key = file_cache_key(PUBLISHED_SCHEMA_VERSION, source.as_bytes(), "app.ts");
    let cached: serde_json::Value = serde_json::from_slice(
        &fs::read(
            f.root
                .join(".tracer-cache/file")
                .join(format!("{key}.json")),
        )
        .unwrap(),
    )
    .unwrap();
    let imports = cached["extraction"]["imports"].as_array().unwrap();
    let binding = |module: &str, symbol: Option<&str>| {
        imports
            .iter()
            .find(|import| {
                import["module"].as_str() == Some(module) && import["symbol"].as_str() == symbol
            })
            .unwrap_or_else(|| panic!("missing {module} {symbol:?}: {imports:#?}"))
    };
    assert_eq!(
        binding("./side-effect", None)["locals"],
        serde_json::json!([])
    );
    assert_eq!(
        binding("./ordinary", None)["locals"],
        serde_json::json!(["ordinary"])
    );
    assert_eq!(
        binding("./toolkit", None)["locals"],
        serde_json::json!(["toolkit"])
    );
    let mixed: Vec<&serde_json::Value> = imports
        .iter()
        .filter(|import| import["module"].as_str() == Some("./mixed"))
        .collect();
    assert_eq!(
        mixed.len(),
        1,
        "mixed import fabricated module rows: {imports:#?}"
    );
    assert_eq!(mixed[0]["locals"], serde_json::json!(["primary", "suite"]));
    assert_eq!(
        binding("./named", Some("remote"))["locals"],
        serde_json::json!(["local"])
    );
    assert_eq!(
        binding("./named", Some("direct"))["locals"],
        serde_json::json!(["direct"])
    );
    assert_eq!(binding("./legacy", None)["locals"], serde_json::json!([]));
}

#[test]
fn structure_preserves_duplicate_typescript_import_rows() {
    let f = Fixture::new();
    f.write(
        "app.ts",
        "import { remote as left, remote as right } from './target';\nimport './same'; import './same';\n",
    );
    f.commit("duplicate TypeScript import rows");

    let structure = f.trace(&["structure", "app.ts", "--json"]);
    structure.ok();
    let structure = structure.view();
    let imports = structure["imports"].as_array().unwrap();
    assert_eq!(
        imports.len(),
        5,
        "legitimate import rows collapsed: {structure:#}"
    );
    assert_eq!(
        imports
            .iter()
            .filter(|import| import["module"] == "./target" && import["symbol"] == "remote")
            .count(),
        2,
        "aliased named rows collapsed: {structure:#}"
    );
    assert_eq!(
        imports
            .iter()
            .filter(|import| import["module"] == "./same" && import["symbol"].is_null())
            .count(),
        2,
        "same-line side-effect rows collapsed: {structure:#}"
    );
}

fn counting_scc(f: &Fixture) -> (String, std::path::PathBuf) {
    let real = Command::new("which")
        .arg("scc")
        .output()
        .expect("which scc")
        .stdout;
    let real = String::from_utf8(real).unwrap().trim().to_string();
    let bin = f.root.join("test-bin");
    fs::create_dir_all(&bin).unwrap();
    let count = f.root.join(".tracer-cache/scc-count");
    fs::create_dir_all(count.parent().unwrap()).unwrap();
    let wrapper = bin.join("scc");
    fs::write(
        &wrapper,
        format!(
            "#!/bin/sh\nprintf x >> '{}'\nexec '{}' \"$@\"\n",
            count.display(),
            real
        ),
    )
    .unwrap();
    fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    (path, count)
}

fn invocation_count(path: &Path) -> usize {
    fs::read(path).map(|bytes| bytes.len()).unwrap_or(0)
}

#[test]
fn repo_context_snapshot_tracks_filesystem_inputs_but_not_tracer_cache() {
    let f = Fixture::new();
    f.write(
        ".gitignore",
        ".tracer-cache/\ntest-bin/\nscc-count\nignored.md\n",
    );
    f.write("u.py", "value = 1\n");
    f.commit("seed");
    let (path, count) = counting_scc(&f);
    fs::create_dir_all(f.root.join(".tracer-cache/file")).unwrap();
    fs::write(
        f.root.join(".tracer-cache/file/repo_context_v3_old.json"),
        "{}",
    )
    .unwrap();

    let first = f.trace_env(&["info", "u.py", "--json"], &[("PATH", &path)]);
    first.ok();
    assert_eq!(first.view()["repo"]["total_files"], 1);
    assert_eq!(invocation_count(&count), 1);
    assert_eq!(
        entries_with_prefix(&f, "repo_context_v3_"),
        0,
        "superseded snapshot version survived publication"
    );

    f.trace_env(&["info", "u.py", "--json"], &[("PATH", &path)])
        .ok();
    assert_eq!(invocation_count(&count), 1, "unchanged inputs reran scc");

    fs::write(f.root.join(".tracer-cache/owned-state"), b"ignored").unwrap();
    f.trace_env(&["info", "u.py", "--json"], &[("PATH", &path)])
        .ok();
    assert_eq!(
        invocation_count(&count),
        1,
        "tracer state invalidated the snapshot"
    );

    f.write("ignored.md", "not an scc input\n");
    f.trace_env(&["info", "u.py", "--json"], &[("PATH", &path)])
        .ok();
    assert_eq!(
        invocation_count(&count),
        1,
        "a gitignored file invalidated the cached snapshot"
    );

    f.write("extra.py", "other = 2\n");
    let changed = f.trace_env(&["info", "u.py", "--json"], &[("PATH", &path)]);
    changed.ok();
    assert_eq!(changed.view()["repo"]["total_files"], 2);
    assert_eq!(
        invocation_count(&count),
        2,
        "an added source did not refresh scc"
    );
    assert_eq!(entries_with_prefix(&f, "repo_context_v5_"), 1);

    Command::new("touch")
        .args(["-t", "202001010000", "u.py"])
        .current_dir(&f.root)
        .status()
        .unwrap();
    f.trace_env(&["info", "u.py", "--json"], &[("PATH", &path)])
        .ok();
    let before_same_size = invocation_count(&count);
    f.write("u.py", "value = 2\n");
    Command::new("touch")
        .args(["-t", "202001010000", "u.py"])
        .current_dir(&f.root)
        .status()
        .unwrap();
    f.trace_env(&["info", "u.py", "--json"], &[("PATH", &path)])
        .ok();
    assert_eq!(
        invocation_count(&count),
        before_same_size + 1,
        "same-size restored-mtime edit did not refresh scc"
    );

    let already_modified = invocation_count(&count);
    f.write("u.py", "value = 3\n");
    f.trace_env(&["info", "u.py", "--json"], &[("PATH", &path)])
        .ok();
    assert_eq!(
        invocation_count(&count),
        already_modified + 1,
        "an edit to an already-modified tracked file did not refresh scc"
    );

    fs::remove_file(f.root.join("extra.py")).unwrap();
    let deleted = f.trace_env(&["info", "u.py", "--json"], &[("PATH", &path)]);
    deleted.ok();
    assert_eq!(deleted.view()["repo"]["total_files"], 1);

    f.write(".sccignore", "u.py\n");
    let ignored = f.trace_env(&["info", "u.py", "--json"], &[("PATH", &path)]);
    ignored.ok();
    assert_eq!(ignored.view()["repo"]["total_files"], 0);
}

#[test]
fn tracer_cache_ignores_its_own_entries_without_a_repository_rule() {
    let f = Fixture::new();
    f.write("u.py", "value = 1\n");
    f.commit("seed");

    f.trace(&["info", "u.py", "--json"]).ok();
    let status = Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=all"])
        .current_dir(&f.root)
        .output()
        .unwrap();
    assert!(status.status.success());
    assert!(
        !String::from_utf8_lossy(&status.stdout).contains(".tracer-cache"),
        "tracer cache entered git status: {}",
        String::from_utf8_lossy(&status.stdout)
    );

    let trace_status = f.trace(&["status", "--json"]);
    trace_status.ok();
    assert!(
        !trace_status.stdout.contains(".tracer-cache/"),
        "tracer cache entered trace status: {}",
        trace_status.stdout
    );
}

#[test]
fn warm_commands_observe_git_state_once_without_untracked_listing() {
    let f = Fixture::new();
    f.write("u.py", "def helper():\n    return 1\n");
    f.commit("seed");
    f.trace(&["info", "u.py", "--json"]).ok();

    for (name, args) in [
        ("info", vec!["info", "u.py", "--json"]),
        ("grep", vec!["grep", "helper", "--path", ".", "--json"]),
        ("regex", vec!["grep", "helper.*", "--path", ".", "--json"]),
    ] {
        let observer = f.root.join(format!(".tracer-cache/{name}-git-trace.json"));
        let run = f.trace_env(&args, &[("GIT_TRACE2_EVENT", observer.to_string_lossy().as_ref())]);
        run.ok();
        let events = fs::read_to_string(&observer).unwrap();
        let status = events
            .lines()
            .filter(|line| line.contains("\"argv\":[\"git\",\"status\""))
            .count();
        let cached = events
            .lines()
            .filter(|line| line.contains("\"argv\":[\"git\",\"ls-files\"") && line.contains("--cached"))
            .count();
        let others = events
            .lines()
            .filter(|line| line.contains("\"argv\":[\"git\",\"ls-files\"") && line.contains("--others"))
            .count();
        assert_eq!(status, 1, "{name} status calls: {events}");
        assert_eq!(cached, 1, "{name} cached listings: {events}");
        assert_eq!(others, 0, "{name} untracked listings: {events}");
    }
}

#[test]
fn a_failed_status_keeps_warm_repo_and_relations_caches() {
    let f = standard_repo();
    let (path, scc_count) = counting_scc(&f);
    let bin = f.root.join("test-bin");
    let real_git = String::from_utf8(
        Command::new("which")
            .arg("git")
            .output()
            .expect("which git")
            .stdout,
    )
    .expect("git path is utf-8")
    .trim()
    .to_string();
    let failed_once = f.root.join(".tracer-cache/status-failed-once");
    let git = bin.join("git");
    fs::write(
        &git,
        format!(
            "#!/bin/sh\nif [ \"$1\" = status ] && [ ! -e '{failed_once}' ]; then\n  touch '{failed_once}'\n  echo planted-status-failure >&2\n  exit 9\nfi\nexec '{real_git}' \"$@\"\n",
            failed_once = failed_once.display(),
        ),
    )
    .unwrap();
    fs::set_permissions(&git, fs::Permissions::from_mode(0o755)).unwrap();
    fs::write(&failed_once, "initial status succeeds").expect("status sentinel is writable");

    f.trace_env(&["cache", "build", "."], &[("PATH", &path)])
        .ok();
    let before_scc = invocation_count(&scc_count);
    let before_edges = fs::read(
        f.root
            .join(".tracer-cache/file")
            .read_dir()
            .expect("file cache exists")
            .flatten()
            .map(|entry| entry.path())
            .find(|path| {
                path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .starts_with("relations_edges_v1__")
            })
            .expect("relations edges entry exists"),
    )
    .expect("relations edges are readable");
    fs::remove_file(&failed_once).expect("remove initial status sentinel");

    let failed = f.trace_env(&["info", "src/util.py", "--json"], &[("PATH", &path)]);
    failed.ok();
    assert!(
        failed.stderr.contains("input scan"),
        "failed status was hidden: {}",
        failed.stderr
    );
    assert_eq!(invocation_count(&scc_count), before_scc, "status failure reran scc");
    assert_eq!(entries_with_prefix(&f, "repo_context_v5_"), 1);
    let after_edges = fs::read(
        f.root
            .join(".tracer-cache/file")
            .read_dir()
            .expect("file cache exists")
            .flatten()
            .map(|entry| entry.path())
            .find(|path| {
                path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .starts_with("relations_edges_v1__")
            })
            .expect("relations edges entry remains"),
    )
    .expect("relations edges remain readable");
    assert_eq!(after_edges, before_edges, "status failure rewrote relations edges");

    f.trace_env(&["info", "src/util.py", "--json"], &[("PATH", &path)])
        .ok();
    assert_eq!(invocation_count(&scc_count), before_scc, "healthy warm run reran scc");
}

#[test]
fn failed_repo_context_output_is_not_cached_and_the_next_call_recovers() {
    let f = Fixture::new();
    f.write(".gitignore", ".tracer-cache/\ntest-bin/\n");
    f.write("u.py", "value = 1\n");
    f.commit("seed");
    let (path, _) = counting_scc(&f);
    let wrapper = f.root.join("test-bin/scc");
    fs::write(&wrapper, "#!/bin/sh\nprintf 'not-json'\n").unwrap();
    fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o755)).unwrap();

    let failed = f.trace_env(&["info", "u.py", "--json"], &[("PATH", &path)]);
    failed.ok();
    assert_eq!(failed.view()["repo"]["total_files"], 0);
    assert!(
        failed.view()["file"].as_str().unwrap().ends_with("u.py"),
        "backend failure erased the requested file"
    );
    assert_eq!(
        failed.view()["loc"],
        0,
        "unavailable fallback must stay visible but uncached"
    );
    assert!(
        failed.stderr.contains("repo context unavailable"),
        "{}",
        failed.stderr
    );
    assert_eq!(entries_with_prefix(&f, "repo_context_v5_"), 0);

    fs::write(&wrapper, "#!/bin/sh\nprintf 'backend broke' >&2\nexit 9\n").unwrap();
    fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o755)).unwrap();
    let backend_failed = f.trace_env(&["info", "u.py", "--json"], &[("PATH", &path)]);
    backend_failed.ok();
    assert!(
        backend_failed.stderr.contains("backend broke"),
        "{}",
        backend_failed.stderr
    );
    assert_eq!(entries_with_prefix(&f, "repo_context_v5_"), 0);

    let real = Command::new("which").arg("scc").output().unwrap().stdout;
    let real = String::from_utf8(real).unwrap().trim().to_string();
    fs::write(&wrapper, format!("#!/bin/sh\nexec '{}' \"$@\"\n", real)).unwrap();
    fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o755)).unwrap();
    let recovered = f.trace_env(&["info", "u.py", "--json"], &[("PATH", &path)]);
    recovered.ok();
    assert_eq!(recovered.view()["repo"]["total_files"], 1);
    assert_eq!(
        recovered.view()["loc"],
        1,
        "the same-byte functionless file retained fabricated fallback LOC"
    );
    assert_eq!(entries_with_prefix(&f, "repo_context_v5_"), 1);
}

#[test]
fn functionless_files_remain_visible_outside_a_git_worktree() {
    let f = Fixture::new();
    f.write("plain.py", "value = 1\n");
    f.write(
        "module.ts",
        "import { value } from './plain';\nexport const other = value;\n",
    );
    fs::remove_dir_all(f.root.join(".git")).unwrap();

    let plain = f.trace(&["info", "plain.py", "--json"]);
    plain.ok();
    assert!(plain.view()["file"].as_str().unwrap().ends_with("plain.py"));
    assert_eq!(plain.view()["functions"], 0);

    let module = f.trace(&["structure", "module.ts", "--json"]);
    module.ok();
    let view = module.view();
    assert!(view["file"].as_str().unwrap().ends_with("module.ts"));
    assert_eq!(view["imports"][0]["module"], "./plain");
    assert!(
        view["exports"]
            .as_array()
            .is_some_and(|exports| !exports.is_empty()),
        "declarations/exports disappeared without Git or SCC: {view:#}"
    );
    assert!(
        !f.root.join(".tracer-cache").exists(),
        "outside-worktree facts were persisted"
    );
}

#[test]
fn replacing_the_backend_outside_the_repository_invalidates_its_snapshot() {
    let f = Fixture::new();
    f.write(".gitignore", ".tracer-cache/\n");
    f.write("u.py", "value = 1\n");
    f.commit("seed");
    let directory = std::env::temp_dir().join(format!("tracer-scc-backend-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let executable = directory.join("scc");
    let output = |count: i64| {
        format!(
            r#"#!/bin/sh
printf '%s' '[{{"Name":"Python","Count":1,"Code":1,"Complexity":{count},"Files":[{{"Location":"{}","Language":"Python","Code":1,"Complexity":{count}}}]}}]'
"#,
            f.root.join("u.py").display()
        )
    };
    fs::write(&executable, output(1)).unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!(
        "{}:{}",
        directory.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let warm = f.trace_env(&["info", "u.py", "--json"], &[("PATH", &path)]);
    warm.ok();
    assert_eq!(warm.view()["repo"]["total_files"], 1);
    assert_eq!(warm.view()["repo"]["complexity_p95"], 1);

    fs::write(&executable, output(2)).unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    let replaced = f.trace_env(&["info", "u.py", "--json"], &[("PATH", &path)]);
    replaced.ok();
    assert_eq!(
        replaced.view()["repo"]["complexity_p95"],
        2,
        "replacement backend output was hidden by a stale snapshot"
    );
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn an_ignored_unreadable_tree_cannot_erase_uncached_repo_context() {
    let f = Fixture::new();
    f.write(".gitignore", ".tracer-cache/\ntest-bin/\n");
    f.write(".sccignore", "build/\n");
    f.write("u.py", "value = 1\n");
    f.write("build/private.py", "value = 2\n");
    f.commit("seed");
    let (path, count) = counting_scc(&f);
    let unreadable = f.root.join("build");
    fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o000)).unwrap();
    let failed = f.trace_env(&["info", "u.py", "--json"], &[("PATH", &path)]);
    fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o755)).unwrap();
    failed.ok();
    assert!(
        !failed.stderr.contains("input scan failed"),
        "{}",
        failed.stderr
    );
    assert_eq!(failed.view()["loc"], 1);
    assert_eq!(failed.view()["repo"]["total_files"], 1);
    assert_eq!(invocation_count(&count), 1);
    assert_eq!(entries_with_prefix(&f, "repo_context_v5_"), 1);

    let recovered = f.trace_env(&["info", "u.py", "--json"], &[("PATH", &path)]);
    recovered.ok();
    assert_eq!(recovered.view()["repo"]["total_files"], 1);
    assert_eq!(invocation_count(&count), 2);
    assert_eq!(entries_with_prefix(&f, "repo_context_v5_"), 1);

    f.trace_env(&["info", "u.py", "--json"], &[("PATH", &path)])
        .ok();
    assert_eq!(
        invocation_count(&count),
        2,
        "stable snapshot was not reused"
    );
}

#[test]
fn a_repo_context_scan_error_cannot_publish_freshness() {
    let f = Fixture::new();
    f.write(".gitignore", ".tracer-cache/\ntest-bin/\n");
    f.write("u.py", "value = 1\n");
    f.write("unreadable/nested.py", "value = 2\n");
    f.commit("seed");
    let (path, count) = counting_scc(&f);
    let unreadable = f.root.join("unreadable");
    fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o000)).unwrap();
    let failed = f.trace_env(&["info", "u.py", "--json"], &[("PATH", &path)]);
    fs::set_permissions(&unreadable, fs::Permissions::from_mode(0o755)).unwrap();
    failed.ok();
    assert!(
        !failed.stderr.contains("input scan failed"),
        "{}",
        failed.stderr
    );
    assert_eq!(failed.view()["repo"]["total_files"], 1);
    assert_eq!(invocation_count(&count), 1);
    assert_eq!(entries_with_prefix(&f, "repo_context_v5_"), 1);

    let recovered = f.trace_env(&["info", "u.py", "--json"], &[("PATH", &path)]);
    recovered.ok();
    assert_eq!(recovered.view()["repo"]["total_files"], 2);
    assert_eq!(invocation_count(&count), 2);
    assert_eq!(entries_with_prefix(&f, "repo_context_v5_"), 1);
}

#[test]
fn repo_context_uses_the_same_executable_for_identity_and_execution() {
    let f = Fixture::new();
    f.write(".gitignore", ".tracer-cache/\ntest-bin/\nshadow-bin/\n");
    f.write("u.py", "value = 1\n");
    f.commit("seed");
    let (working_path, count) = counting_scc(&f);
    let shadow = f.root.join("shadow-bin");
    fs::create_dir_all(&shadow).unwrap();
    fs::write(shadow.join("scc"), "not executable").unwrap();
    let path = format!("{}:{}", shadow.display(), working_path);

    f.trace_env(&["info", "u.py", "--json"], &[("PATH", &path)])
        .ok();
    assert_eq!(
        invocation_count(&count),
        1,
        "the executable shadow was selected or execution used a different PATH resolution"
    );
}

#[test]
fn changing_inputs_during_scc_does_not_publish_a_snapshot() {
    let f = Fixture::new();
    f.write(".gitignore", ".tracer-cache/\ntest-bin/\n");
    f.write("u.py", "value = 1\n");
    f.commit("seed");
    let (path, _) = counting_scc(&f);
    let wrapper = f.root.join("test-bin/scc");
    let real = Command::new("which").arg("scc").output().unwrap().stdout;
    let real = String::from_utf8(real).unwrap().trim().to_string();
    fs::write(
        &wrapper,
        format!(
            "#!/bin/sh\nprintf 'value = 2\\n' > '{}'\nexec '{}' \"$@\"\n",
            f.root.join("u.py").display(),
            real
        ),
    )
    .unwrap();
    fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o755)).unwrap();

    let unstable = f.trace_env(&["info", "u.py", "--json"], &[("PATH", &path)]);
    unstable.ok();
    assert!(
        unstable
            .stderr
            .contains("inputs changed while scc was running"),
        "{}",
        unstable.stderr
    );
    assert_eq!(entries_with_prefix(&f, "repo_context_v5_"), 0);

    fs::write(&wrapper, format!("#!/bin/sh\nexec '{}' \"$@\"\n", real)).unwrap();
    fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o755)).unwrap();
    let stable = f.trace_env(&["info", "u.py", "--json"], &[("PATH", &path)]);
    stable.ok();
    assert_eq!(stable.view()["repo"]["total_files"], 1);
    assert_eq!(entries_with_prefix(&f, "repo_context_v5_"), 1);
}

#[test]
fn malformed_cached_repo_context_is_recomputed() {
    let f = Fixture::new();
    f.write(".gitignore", ".tracer-cache/\ntest-bin/\n");
    f.write("u.py", "value = 1\n");
    f.commit("seed");
    let (path, count) = counting_scc(&f);
    f.trace_env(&["info", "u.py", "--json"], &[("PATH", &path)])
        .ok();
    let entry = fs::read_dir(f.root.join(".tracer-cache/file"))
        .unwrap()
        .flatten()
        .find(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("repo_context_v5_")
        })
        .unwrap();
    fs::write(
        entry.path(),
        r#"{"summary":{},"per_file":{},"languages":[]}"#,
    )
    .unwrap();
    let recovered = f.trace_env(&["info", "u.py", "--json"], &[("PATH", &path)]);
    recovered.ok();
    assert_eq!(recovered.view()["repo"]["total_files"], 1);
    assert_eq!(
        invocation_count(&count),
        2,
        "malformed cached aggregates were accepted"
    );
}

#[test]
fn cache_build_populates_the_file_namespace() {
    let f = standard_repo();
    let r = f.trace(&["cache", "build", "."]);
    r.ok();
    assert!(r.stdout.contains("Relations:"), "{}", r.stdout);
    let stats = f.trace(&["cache", "stats", "--json"]);
    stats.ok();
    let v = stats.view();
    // `cache build .` over standard_repo() populates exactly 10 file/
    // entries for this fixed tree: six per-file entries, the mtime index,
    // the git-activity map, and the two relations-index entries.
    assert_eq!(
        v["file"]["entries"].as_i64().unwrap(),
        10,
        "file namespace must hold exactly 10 entries after build: {}",
        stats.stdout
    );
    assert_eq!(
        relations_entry_count(&f),
        2,
        "build must leave exactly two relations-index entries: {}",
        stats.stdout
    );
}

/// Count the two relations-index entries in the file namespace. Each entry is
/// keyed by schema alone and rewritten in place rather than rotated.
fn relations_entry_count(f: &Fixture) -> usize {
    entries_with_prefix(f, "relations_")
}

/// The relations index is two mutable entries per repo, and it always answers
/// from the current tree. Across a doc change, a code change, and a new
/// file — each with a HEAD move — the namespace must still hold exactly two
/// entries, and they must report the edit rather than the prior state.
#[test]
fn the_relations_index_stays_single_and_current_across_builds() {
    let f = standard_repo();
    f.write("Claude.md", "# project\n");
    f.commit("add doc");

    f.trace(&["cache", "build", "."]).ok();
    assert_eq!(
        relations_entry_count(&f),
        2,
        "first build must leave exactly two relations-index entries"
    );

    // A doc change + HEAD move: neither touches code relations.
    f.write("Claude.md", "# project updated\n");
    f.commit("doc change");
    f.trace(&["cache", "build", "."]).ok();
    assert_eq!(
        relations_entry_count(&f),
        2,
        "a doc change must not add an index"
    );

    // A code change + HEAD move: the renamed symbol must replace the old one.
    f.write("src/util.py", "def renamed_helper(v):\n    return v + 99\n");
    f.commit("code change");
    f.trace(&["cache", "build", "."]).ok();
    assert_eq!(
        relations_entry_count(&f),
        2,
        "a code change must not add an index"
    );
    let renamed = f.trace(&["defines", "renamed_helper", "--json"]);
    renamed.ok();
    assert_eq!(
        renamed.view()["definitions"].as_i64().unwrap(),
        1,
        "the index did not pick up the renamed declaration: {}",
        renamed.stdout
    );
    let gone = f.trace(&["defines", "helper", "--json"]);
    assert_eq!(
        gone.code, 2,
        "the index still serves the declaration that was renamed away: {}",
        gone.stdout
    );

    // A new file + HEAD move: its declaration must be reachable.
    f.write("src/extra.py", "def extra_fn():\n    return 1\n");
    f.commit("another head move");
    f.trace(&["cache", "build", "."]).ok();
    assert_eq!(
        relations_entry_count(&f),
        2,
        "a new file must not add an index"
    );
    let added = f.trace(&["defines", "extra_fn", "--json"]);
    added.ok();
    assert_eq!(
        added.view()["definitions"].as_i64().unwrap(),
        1,
        "the index did not pick up the new file's declaration: {}",
        added.stdout
    );
}

#[test]
fn cache_stats_human_table_matches_json() {
    let f = standard_repo();
    f.trace(&["cache", "build", "."]).ok();
    let human = f.trace(&["cache", "stats"]);
    human.ok();
    let table = parse_stats_table(&human.stdout);
    let json = f.trace(&["cache", "stats", "--json"]).view();
    assert_eq!(
        table.get("file").copied().unwrap_or(0) as i64,
        json["file"]["entries"].as_i64().unwrap(),
        "human/json file count disagree\nhuman:\n{}",
        human.stdout
    );
}

#[test]
fn cache_build_is_idempotent_and_warm_is_faster() {
    let f = standard_repo();
    let cold = f.trace(&["cache", "build", "."]);
    cold.ok();
    let warm = f.trace(&["cache", "build", "."]);
    warm.ok();
    // Warm rebuild must not be dramatically slower than cold; mostly this
    // asserts idempotence (no crash, still reports a graph).
    assert!(warm.stdout.contains("Relations:"), "{}", warm.stdout);
}

#[test]
fn cache_invalidates_on_content_change() {
    let f = standard_repo();
    f.trace(&["cache", "build", "."]).ok();
    let before = f.trace(&["info", "src/util.py", "--json"]).view();
    // standard_repo()'s helper(v): base 1 + if(1) = 2, exactly.
    assert_eq!(
        before["ccn_total"].as_i64().unwrap(),
        2,
        "baseline helper() CCN must be exactly 2: {}",
        before["ccn_total"]
    );

    // Add decision points; ccn must change on the next read (cache key is
    // keyed on content, so the stale entry is unreachable). The new
    // helper: base 1 + if(1) + if(1) + for(1) + if(1) = 5, exactly — a
    // stale cache hit would still report 2 and fail this equality.
    f.write(
        "src/util.py",
        "def helper(v):\n    if v > 0:\n        if v > 10:\n            return v\n    \
         for i in range(v):\n        if i:\n            pass\n    return 0\n",
    );
    let after = f.trace(&["info", "src/util.py", "--json"]).view();
    assert_eq!(
        after["ccn_total"].as_i64().unwrap(),
        5,
        "post-edit helper() CCN must be exactly 5 (cache served a stale \
         entry if this is 2): {}",
        after["ccn_total"]
    );
}

/// `cache clear` empties `file/` and nothing else. `file/` and `sessions/`
/// share one `.tracer-cache/`, so clearing derived facts must never take an
/// agent's session log with it — only `--all` does that.
#[test]
fn cache_clear_leaves_the_session_log() {
    // The outer process gives this test hostile session identity just as an
    // Agent harness does. Re-exec keeps that mutation out of this parallel
    // Rust test process; the inner run must isolate it before fixture values
    // are applied.
    if std::env::var_os("TRACER_CACHE_CLEAR_ISOLATION_CHILD").is_none() {
        let mut child = Command::new(std::env::current_exe().expect("test executable path"));
        child
            .arg("--exact")
            .arg("cache_clear_leaves_the_session_log")
            .env("TRACER_CACHE_CLEAR_ISOLATION_CHILD", "1");
        for key in [
            "AGENT_SESSION_ID",
            "CODEX_THREAD_ID",
            "CLAUDE_CODE_SESSION_ID",
            "TRACER_AGENT_ID",
        ] {
            child.env(key, format!("hostile-{key}"));
        }
        let output = child.output().expect("isolated cache-clear test spawns");
        assert!(
            output.status.success(),
            "cache-clear isolation test failed:\n--- stdout ---\n{}\n--- stderr ---\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        return;
    }

    let f = standard_repo();
    f.write("Claude.md", "# project\n");
    f.commit("add doc");
    f.trace(&["cache", "build", "."]).ok();
    let session = f.trace_env(
        &["docs", "src/util.py"],
        &[
            // This explicit fixture value must win over every hostile parent
            // identity. The harness preserves the caller's real PATH.
            ("CLAUDE_CODE_SESSION_ID", "cache-clear-scope"),
            ("TRACER_CCN_BACKEND", "scc"),
        ],
    );
    session.ok();
    let sessions_dir = f.root.join(".tracer-cache/sessions/cache-clear-scope/root");
    assert!(
        sessions_dir.is_dir(),
        "the fixture wrote no session log to protect"
    );

    let r = f.trace(&["cache", "clear"]);
    r.ok();
    let v = f.trace(&["cache", "stats", "--json"]).json();
    assert_eq!(
        v["results"]["file"]["entries"].as_i64().unwrap(),
        0,
        "file namespace not cleared: {v}"
    );
    assert!(
        sessions_dir.is_dir(),
        "a scoped clear of file/ removed the session log too"
    );
}

#[test]
fn cache_clear_all_removes_everything() {
    let f = standard_repo();
    f.trace(&["cache", "build", "."]).ok();
    f.trace_env(
        &["docs", "src/util.py"],
        &[("CLAUDE_CODE_SESSION_ID", "cache-clear-all")],
    )
    .ok();
    let r = f.trace(&["cache", "clear", "--all"]);
    r.ok();
    assert!(r.stdout.contains("Removed"), "{}", r.stdout);
    assert!(
        !f.root.join(".tracer-cache").exists(),
        "--all must remove the whole cache tree, sessions included"
    );
    let v = f.trace(&["cache", "stats", "--json"]).view();
    assert_eq!(v["file"]["entries"].as_i64().unwrap(), 0);
}

#[test]
fn ccn_backend_is_ast_and_cache_does_not_fork_on_env_value() {
    let f = standard_repo();

    // Default build.
    f.trace(&["cache", "build", "."]).ok();
    let default_entries = f.trace(&["cache", "stats", "--json"]).view()["file"]["entries"]
        .as_i64()
        .unwrap();
    let default_info = f.trace(&["info", "src/app.py", "--json"]).view();

    // Building the same tree with TRACER_CCN_BACKEND set must not add a
    // second set of entries — there is one backend, one cache identity.
    f.trace_env(&["cache", "build", "."], &[("TRACER_CCN_BACKEND", "ast")])
        .ok();
    let after_ast = f.trace(&["cache", "stats", "--json"]).view()["file"]["entries"]
        .as_i64()
        .unwrap();
    assert_eq!(
        after_ast, default_entries,
        "setting TRACER_CCN_BACKEND forked the cache \
         (default={default_entries}, after_ast={after_ast}) — there is one backend"
    );

    // FileFacts shape (the keys downstream commands depend on) is present
    // regardless of the env value.
    let ast_info = f
        .trace_env(
            &["info", "src/app.py", "--json"],
            &[("TRACER_CCN_BACKEND", "ast")],
        )
        .view();
    // The FileFacts keys are not merely present — they carry the exact,
    // hand-verifiable values for src/app.py (main(): if + for + if over
    // base 1 = CCN 4; one function; rank low; Python). Asserting the
    // values, and that they are identical under both env settings, proves
    // the env value neither forks the cache nor shifts the computation.
    let expected = serde_json::json!({
        "functions": 1,
        "ccn_total": 4,
        "ccn_max_function": 4,
        "rank": "low",
    });
    for (key, want) in expected.as_object().unwrap() {
        assert_eq!(
            &default_info[key], want,
            "default-env FileFacts `{key}` wrong for src/app.py"
        );
        assert_eq!(
            &ast_info[key], want,
            "ast-env FileFacts `{key}` wrong for src/app.py"
        );
    }
    // The language rides in the per-file context, keyed by the path the
    // query echoes, so a row projection cannot take it away.
    for info in [&default_info, &ast_info] {
        let file = info["file"].as_str().unwrap();
        assert_eq!(
            info["files"][file]["language"], "python",
            "FileFacts language wrong for src/app.py"
        );
    }
}

#[test]
fn ccn_is_ast_derived_regardless_of_backend_env_value() {
    let f = standard_repo();
    let default = f.trace(&["info", "src/app.py", "--json"]).view();
    let explicit_ast = f
        .trace_env(
            &["info", "src/app.py", "--json"],
            &[("TRACER_CCN_BACKEND", "ast")],
        )
        .view();
    let bogus = f
        .trace_env(
            &["info", "src/app.py", "--json"],
            &[("TRACER_CCN_BACKEND", "definitely-not-a-backend")],
        )
        .view();
    // One backend (AST): every env value yields the identical CCN.
    assert_eq!(
        default["ccn_total"], explicit_ast["ccn_total"],
        "explicit ast value changed CCN — backend is not value-independent"
    );
    assert_eq!(
        default["ccn_total"], bogus["ccn_total"],
        "unknown TRACER_CCN_BACKEND value changed CCN — backend is not value-independent"
    );
}

/// The stable byte format: `--json` emits every non-ASCII scalar as a
/// `\uXXXX` escape (astral chars as a UTF-16 surrogate pair) with `": "` /
/// `", "` separators. Asserted on the RAW stdout bytes — never parsed
/// through serde_json, which would normalize `é` back to `é` and
/// launder away exactly the bytes the guarantee is about.
#[test]
fn json_output_is_ascii_escaped_on_raw_bytes() {
    let f = Fixture::new();
    // café (U+00E9, in the BMP) and 🚀 (U+1F680, astral → surrogate pair).
    f.write("uni.py", "x = 1  # caf\u{00e9} \u{1f680} token_NONASCII\n");
    f.commit("non-ascii content");

    let r = f.trace(&["grep", "token_NONASCII", "--path", ".", "--json"]);
    r.ok();
    let raw = r.stdout.as_bytes();

    // 1. Every byte is ASCII — no raw UTF-8 multibyte leaked through.
    assert!(
        raw.iter().all(|b| b.is_ascii()),
        "non-ASCII byte in --json output; the stable format must escape all"
    );
    // 2. The exact escape sequences are present as literal backslash-u
    //    (a normalizing parser would have collapsed these to é / 🚀).
    assert!(
        r.stdout.contains("caf\\u00e9"),
        "BMP scalar é not \\u-escaped: {}",
        r.stdout
    );
    assert!(
        r.stdout.contains("\\ud83d\\ude80"),
        "astral scalar 🚀 not emitted as a UTF-16 surrogate pair: {}",
        r.stdout
    );
    // 3. The fixed separators (`": "` after a key, `", "` between items).
    assert!(
        r.stdout.contains("\"matches\": "),
        "key separator must be \": \": {}",
        r.stdout
    );
    // 4. The literal raw é byte (0xC3 0xA9) must NOT appear anywhere.
    assert!(
        !r.stdout.contains('\u{00e9}') && !r.stdout.contains('\u{1f680}'),
        "raw non-ASCII scalar present — format laundered: {}",
        r.stdout
    );

    // The on-disk cache entry shares the same byte format. The file-cache
    // entry for uni.py records its language; assert the entry bytes are
    // also pure ASCII (same serializer, same guarantee on disk).
    f.trace(&["cache", "build", "."]).ok();
    let entry_dir = f.root.join(".tracer-cache/file");
    let mut checked_an_entry = false;
    for e in fs::read_dir(&entry_dir).unwrap().flatten() {
        let p = e.path();
        if p.extension().and_then(|x| x.to_str()) != Some("json") {
            continue;
        }
        let bytes = fs::read(&p).unwrap();
        assert!(
            bytes.iter().all(|b| b.is_ascii()),
            "on-disk cache entry {p:?} contains a non-ASCII byte — \
             the on-disk format must use the same ASCII escaping as --json"
        );
        checked_an_entry = true;
    }
    assert!(checked_an_entry, "no file-cache entry written to assert on");
}

/// Schema-version invalidation, proven black-box against the *published*
/// key formula (no tracer internals linked). A cache entry whose key was
/// derived under a different SCHEMA_VERSION must be unreachable: the binary
/// must ignore it and recompute the true value.
///
/// Step A pins that the cache really is consulted by the current-schema
/// key (a poison planted there is served). Step B is the invalidation
/// proof: the same poison under a *neighbor* schema's key is NOT served.
#[test]
fn schema_version_bump_makes_prior_entries_unreachable() {
    let src = "def helper(v):\n    if v > 0:\n        return v + 1\n    return 0\n";

    // grep enrichment serves `file_complexity.ccn_total` straight from the
    // `file/` cache entry (`file_facts::get`), so a poisoned entry under a
    // given key is observable. (`info`'s top-level CCN is recomputed from
    // source and would mask the cache, so it is the wrong probe here.)
    let bytes = src.as_bytes();
    let poison_entry = serde_json::json!({
        "path": "u.py",
        "language": "Python",
        "loc": 4,
        "function_count": 1,
        "cyclomatic_complexity_total": 999,
        "cyclomatic_complexity_max": 999,
        "rank": "critical",
        "functions": [{
            "name": "helper",
            "start_line": 1,
            "nloc": 4,
            "cyclomatic_complexity": 999
        }],
        "mtime_ns": 0,
        "size_bytes": bytes.len() as i64,
        "extraction": serde_json::Value::Null
    });
    let grep_ccn = |fx: &Fixture| -> i64 {
        let v = fx
            .trace(&["grep", "helper", "--path", ".", "--json"])
            .view();
        let file = v["results"][0]["file"].as_str().expect("one match on u.py");
        v["files"][file]["file_complexity"]["ccn_total"]
            .as_i64()
            .unwrap()
    };

    // --- Step A: the current-schema key IS consulted. ---
    let f = Fixture::new();
    f.write("u.py", src);
    f.commit("seed");
    f.trace(&["cache", "build", "."]).ok();
    // helper(v): base 1 + if(1) = exactly 2.
    let true_ccn = grep_ccn(&f);
    assert_eq!(true_ccn, 2, "fixture sanity: helper() CCN is exactly 2");

    let cur_key = file_cache_key(PUBLISHED_SCHEMA_VERSION, bytes, "u.py");
    let entry_dir = f.root.join(".tracer-cache/file");
    let cur_path = entry_dir.join(format!("{cur_key}.json"));
    assert!(
        cur_path.exists(),
        "reconstructed current-schema key {cur_key} has no on-disk entry — \
         the published key formula is wrong or the cache was not warmed"
    );
    let mut malformed_entry = poison_entry.clone();
    malformed_entry
        .as_object_mut()
        .expect("cache fixture is an object")
        .remove("functions");
    fs::write(&cur_path, serde_json::to_string(&malformed_entry).unwrap()).unwrap();
    assert_eq!(
        grep_ccn(&f),
        true_ccn,
        "a current-schema entry missing retained function facts was fabricated as valid"
    );
    // Poison the current-schema entry and defeat the mtime fast-path so the
    // content-hash (schema-versioned) key path is what answers.
    fs::write(&cur_path, serde_json::to_string(&poison_entry).unwrap()).unwrap();
    fs::remove_file(entry_dir.join(format!(
        "mtime_index_v1__schema{}__ast.json",
        PUBLISHED_SCHEMA_VERSION
    )))
    .ok();
    assert_eq!(
        grep_ccn(&f),
        999,
        "the cache is NOT keyed/consulted by the current-schema key — \
         a poison planted at that key was not served, so the rest of this \
          test cannot prove schema invalidation"
    );
    let info = f.trace(&["info", "u.py", "--json"]).view();
    assert_eq!(
        info["ccn_total"], 999,
        "info reparsed instead of serving retained rows: {info:#}"
    );
    assert_eq!(
        info["results"][0]["cyclomatic_complexity"], 999,
        "info did not serve the retained sentinel row: {info:#}"
    );
    let structure = f.trace(&["structure", "u.py", "--json"]).view();
    assert_eq!(
        structure["symbols_by_kind"]["function"][0]["cyclomatic_complexity"], 999,
        "structure reparsed instead of serving the retained sentinel row: {structure:#}"
    );

    let mut inconsistent_empty = poison_entry.clone();
    inconsistent_empty["functions"] = serde_json::json!([]);
    fs::write(
        &cur_path,
        serde_json::to_string(&inconsistent_empty).unwrap(),
    )
    .unwrap();
    assert_eq!(
        grep_ccn(&f),
        true_ccn,
        "empty retained rows with a non-zero function count were accepted"
    );

    // --- Step B: a neighbor-schema key is unreachable. ---
    let g = Fixture::new();
    g.write("u.py", src);
    g.commit("seed");
    g.trace(&["cache", "build", "."]).ok();
    let g_entry_dir = g.root.join(".tracer-cache/file");
    // Remove the legitimate current-schema entry and the mtime index, then
    // plant the SAME poison under the *previous* schema version's key.
    fs::remove_file(g_entry_dir.join(format!("{cur_key}.json"))).ok();
    fs::remove_file(g_entry_dir.join(format!(
        "mtime_index_v1__schema{}__ast.json",
        PUBLISHED_SCHEMA_VERSION
    )))
    .ok();
    let old_key = file_cache_key(PUBLISHED_SCHEMA_VERSION - 1, bytes, "u.py");
    fs::write(
        g_entry_dir.join(format!("{old_key}.json")),
        serde_json::to_string(&poison_entry).unwrap(),
    )
    .unwrap();
    // The old-schema entry must be unreachable: the binary recomputes the
    // true CCN, never the 999 poison sitting under the prior schema's key.
    assert_eq!(
        grep_ccn(&g),
        true_ccn,
        "an entry keyed under SCHEMA_VERSION {} was served while the \
         binary runs SCHEMA_VERSION {} — schema-version bumps do NOT \
         invalidate prior entries",
        PUBLISHED_SCHEMA_VERSION - 1,
        PUBLISHED_SCHEMA_VERSION
    );
}

#[test]
fn info_and_structure_serve_the_same_retained_function_rows() {
    let f = Fixture::new();
    let src = "def helper(v):\n    if v > 0:\n        return v + 1\n    return 0\n";
    f.write("u.py", src);
    f.commit("seed");
    f.trace(&["cache", "build", "."]).ok();

    let entry_dir = f.root.join(".tracer-cache/file");
    let entry = fs::read_dir(&entry_dir)
        .unwrap()
        .flatten()
        .find(|entry| {
            fs::read(entry.path())
                .ok()
                .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
                .and_then(|value| value["path"].as_str().map(|path| path == "u.py"))
                .unwrap_or(false)
        })
        .expect("content-derived u.py cache entry");
    let mut cached: serde_json::Value =
        serde_json::from_slice(&fs::read(entry.path()).unwrap()).unwrap();
    if cached.get("functions").is_some() {
        let retained_bytes = serde_json::to_vec(&cached).unwrap().len();
        let mut without_rows = cached.clone();
        without_rows
            .as_object_mut()
            .expect("content cache entry is an object")
            .remove("functions");
        let prior_bytes = serde_json::to_vec(&without_rows).unwrap().len();
        println!(
            "owned u.py cache entry: {retained_bytes} bytes with rows, {prior_bytes} bytes without rows, +{} bytes",
            retained_bytes - prior_bytes
        );
    }
    cached["function_count"] = serde_json::json!(1);
    cached["cyclomatic_complexity_total"] = serde_json::json!(41);
    cached["cyclomatic_complexity_max"] = serde_json::json!(41);
    cached["rank"] = serde_json::json!("high");
    cached["functions"] = serde_json::json!([{
        "name": "cached_helper",
        "start_line": 1,
        "nloc": 4,
        "cyclomatic_complexity": 41
    }]);
    fs::write(entry.path(), serde_json::to_vec(&cached).unwrap()).unwrap();

    let info = f.trace(&["info", "u.py", "--json"]).view();
    assert_eq!(info["ccn_total"], 41, "info reparsed source: {info:#}");
    assert_eq!(
        info["results"][0]["name"], "cached_helper",
        "info lost cached row: {info:#}"
    );
    let structure = f.trace(&["structure", "u.py", "--json"]).view();
    assert_eq!(
        structure["symbols_by_kind"]["function"][0]["cyclomatic_complexity"], 41,
        "structure reparsed source: {structure:#}"
    );
}

/// The canonical passive-context shoulder for one file.
fn shoulder(f: &Fixture, rel: &str) -> String {
    let v = f.trace(&["info", rel, "--json"]).view();
    let file = v["file"].as_str().expect("info echoes the file it read");
    v["files"][file]["shoulder"]
        .as_str()
        .unwrap_or_else(|| panic!("no shoulder for {rel}: {v}"))
        .to_string()
}

/// Cache entries in the `file` namespace whose key starts with `prefix`.
fn entries_with_prefix(f: &Fixture, prefix: &str) -> usize {
    fs::read_dir(f.root.join(".tracer-cache/file"))
        .map(|rd| {
            rd.flatten()
                .filter(|e| {
                    e.file_name()
                        .to_str()
                        .map(|n| n.starts_with(prefix) && n.ends_with(".json"))
                        .unwrap_or(false)
                })
                .count()
        })
        .unwrap_or(0)
}

/// A commit moves HEAD and empties `git status`, but leaves the file's bytes,
/// size, and mtime untouched — so both the content-hash entry and its
/// mtime-index row still match and the entry is served as-is. While that
/// entry carried the git fields, the shoulder kept rendering the pre-commit
/// `modified (N commits)` until the bytes changed, and on a clean tree the
/// status overlay returned early and could never clear it.
#[test]
fn a_commit_refreshes_the_shoulder_with_no_content_change() {
    let f = Fixture::new();
    // Keep the cache out of git, so committing does not stage it and the tree
    // is genuinely clean afterwards — the state the defect needed.
    f.write(".gitignore", ".tracer-cache/\n");
    f.write("u.py", "def helper(v):\n    return v\n");
    f.commit("one");

    f.write(
        "u.py",
        "def helper(v):\n    if v:\n        return v\n    return 0\n",
    );
    let dirty = shoulder(&f, "u.py");
    assert!(
        dirty.contains("git: modified"),
        "an edited file should read as modified: {dirty}"
    );

    f.commit("two");
    let clean = shoulder(&f, "u.py");
    assert!(
        !clean.contains("modified"),
        "a committed file still reads as uncommitted: {clean}"
    );
    assert!(
        clean.contains("git: 2 commits"),
        "the commit count did not move with HEAD: {clean}"
    );
}

/// The bulk git map is keyed by HEAD and the 30-day cutoff date, so every
/// commit and every new day writes a fresh entry. Without the eviction the
/// superseded ones stay forever: 64 of them at ~800 KB each had accumulated
/// in the dotfiles repo.
#[test]
fn superseded_git_activity_entries_are_evicted() {
    let f = Fixture::new();
    f.write(".gitignore", ".tracer-cache/\n");
    f.write("u.py", "def helper(v):\n    return v\n");
    f.commit("one");
    shoulder(&f, "u.py");

    f.write(
        "u.py",
        "def helper(v):\n    if v:\n        return v\n    return 0\n",
    );
    f.commit("two");
    shoulder(&f, "u.py");

    assert_eq!(
        entries_with_prefix(&f, "git_activity_v2__"),
        1,
        "a superseded git-activity entry survived the commit"
    );
}

#[test]
fn repository_build_discovers_untracked_files_once() {
    let f = Fixture::new();
    f.write(".gitignore", ".tracer-cache/\n");
    f.write("tracked.py", "VALUE = 1\n");
    f.commit("seed");
    f.write("nested/untracked.py", "OTHER = 2\n");
    let trace_events = f.root.join(".tracer-cache/git-trace.json");
    fs::create_dir_all(trace_events.parent().unwrap()).unwrap();
    let trace_events_text = trace_events.to_string_lossy().to_string();

    f.trace_env(
        &["cache", "build", "."],
        &[("GIT_TRACE2_EVENT", trace_events_text.as_str())],
    )
    .ok();

    let events = fs::read_to_string(&trace_events).expect("git trace2 event log");
    let status_walks = events
        .lines()
        .filter(|line| {
            line.contains("\"argv\":[\"git\",\"status\"") && line.contains("--untracked-files=all")
        })
        .count();
    let duplicate_walks = events
        .lines()
        .filter(|line| line.contains("\"argv\":[\"git\",\"ls-files\"") && line.contains("--others"))
        .count();
    assert_eq!(
        status_walks, 1,
        "expected one shared untracked status walk; events:\n{events}"
    );
    assert_eq!(
        duplicate_walks, 0,
        "ls-files performed a second untracked discovery walk; events:\n{events}"
    );
}

#[test]
fn prior_presence_cache_representation_is_unreachable() {
    let f = Fixture::new();
    f.write(".gitignore", ".tracer-cache/\n");
    f.write("deployed.py", "VALUE = 1\n");
    f.commit("seed");
    f.git(&["update-ref", "refs/remotes/origin/master", "HEAD"]);
    let tip = Command::new("git")
        .args(["rev-parse", "origin/master"])
        .current_dir(&f.root)
        .output()
        .unwrap();
    let tip = String::from_utf8(tip.stdout).unwrap();
    let mut hasher = Sha256::new();
    hasher.update(b"main\0origin/master\0");
    hasher.update(tip.trim().as_bytes());
    hasher.update(b"\n");
    let old_key = format!("git_presence__{}", hex::encode(hasher.finalize()));
    let file_cache = f.root.join(".tracer-cache/file");
    fs::create_dir_all(&file_cache).unwrap();
    fs::write(file_cache.join(format!("{old_key}.json")), "{}").unwrap();

    let shoulder = shoulder(&f, "deployed.py");
    assert!(
        shoulder.contains("presence: main"),
        "previous newline-based presence cache was served: {shoulder}"
    );
    assert_eq!(
        entries_with_prefix(&f, "git_presence__"),
        0,
        "previous presence cache was not evicted"
    );
    assert_eq!(
        entries_with_prefix(&f, "git_presence_v2__"),
        1,
        "versioned presence cache was not written"
    );
}

#[test]
fn prior_git_activity_cache_representation_is_unreachable() {
    let f = Fixture::new();
    f.write(".gitignore", ".tracer-cache/\n");
    f.write("local.py", "VALUE = 1\n");
    f.commit("seed");
    let clean = shoulder(&f, "local.py");
    assert!(
        clean.contains("presence: local-only"),
        "fixture unexpectedly deployed: {clean}"
    );

    let file_cache = f.root.join(".tracer-cache/file");
    let current = fs::read_dir(&file_cache)
        .unwrap()
        .flatten()
        .find(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("git_activity_v2__")
        })
        .expect("versioned git activity cache");
    let old_name =
        current
            .file_name()
            .to_string_lossy()
            .replacen("git_activity_v2__", "git_activity__", 1);
    let mut payload: serde_json::Value =
        serde_json::from_slice(&fs::read(current.path()).unwrap()).unwrap();
    payload["local.py"]["present_in"] = serde_json::json!(["main"]);
    fs::write(
        file_cache.join(old_name),
        serde_json::to_vec(&payload).unwrap(),
    )
    .unwrap();
    fs::remove_file(current.path()).unwrap();

    let refreshed = shoulder(&f, "local.py");
    assert!(
        refreshed.contains("presence: local-only"),
        "prior combined history/presence cache was served: {refreshed}"
    );
    assert_eq!(
        entries_with_prefix(&f, "git_activity__"),
        0,
        "previous git activity cache was not evicted"
    );
    assert_eq!(
        entries_with_prefix(&f, "git_activity_v2__"),
        1,
        "versioned git activity cache was not written"
    );
}
