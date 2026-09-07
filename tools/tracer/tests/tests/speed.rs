//! Speed assertions. Every command gets a wall-clock budget; cache-backed
//! commands additionally get a cold-vs-warm pair so a lost cache is caught.
//!
//! Thresholds are intentionally loose — they are regression tripwires, not
//! micro-benchmarks. On this machine the
//! observed times are ~0.1–0.3s; the budgets below leave generous headroom
//! for a loaded CI box while still failing on an order-of-magnitude
//! regression or a cache that silently stopped working. Tune `SLOW`/`FAST`
//! up if a constrained runner flakes — the cold-vs-warm *ratio* check
//! survives threshold tuning.

use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tracer_cli_tests::{standard_repo, trace_bin, Fixture};

/// A repo big enough that re-extracting every file is unmistakably slower
/// than serving a warm cache. The dead-cache signal needs this: on
/// `standard_repo()` (five trivial files) a no-op cache is invisible
/// because cold and warm are both ~instant. Hundreds of branchy files make
/// "cache silently stopped working" measurable as warm ≫ should-be.
fn large_repo() -> Fixture {
    let f = Fixture::new();
    for d in 0..12 {
        for i in 0..30 {
            f.write(
                &format!("pkg{d:02}/mod{i:02}.py"),
                "def f(a, b, c):\n\
                 \x20   if a and b:\n\
                 \x20       return 1\n\
                 \x20   for i in range(c):\n\
                 \x20       if i % 2 == 0 or i == c:\n\
                 \x20           return i\n\
                 \x20   return 0\n",
            );
        }
    }
    f.commit("large multi-package repo (360 branchy files)");
    f
}

const SLOW: Duration = Duration::from_secs(10); // cold / heavy commands
const FAST: Duration = Duration::from_secs(5); // warm / light commands

#[test]
fn doctor_is_fast() {
    let f = standard_repo();
    f.trace(&["doctor"]).ok().within(FAST);
}

#[test]
fn read_warm_is_fast() {
    let f = standard_repo();
    f.trace(&["cache", "build", "."]).ok();
    f.trace(&["read", "src/app.py"]).ok().within(FAST);
}

#[test]
fn info_cold_then_warm() {
    let f = standard_repo();
    f.trace(&["cache", "clear", "--all"]).ok();
    let cold = f.trace(&["info", "src/app.py"]);
    cold.ok().within(SLOW);
    let warm = f.trace(&["info", "src/app.py"]);
    warm.ok().within(FAST);
}

#[test]
fn cache_build_cold_within_budget() {
    let f = standard_repo();
    f.trace(&["cache", "clear", "--all"]).ok();
    f.trace(&["cache", "build", "."]).ok().within(SLOW);
}

#[test]
fn cache_build_warm_is_fast() {
    let f = standard_repo();
    f.trace(&["cache", "build", "."]).ok();
    f.trace(&["cache", "build", "."]).ok().within(FAST);
}

#[test]
fn architecture_query_warm_is_fast() {
    let f = standard_repo();
    f.trace(&["cache", "build", "."]).ok();
    f.trace(&["callers", "helper"]).ok().within(FAST);
    f.trace(&["usages", "--path", ".", "--json"])
        .ok()
        .within(FAST);
}

#[test]
fn search_commands_within_budget() {
    let f = standard_repo();
    f.trace(&["grep", "helper", "--path", "."])
        .ok()
        .within(SLOW);
    f.trace(&["find", "**/*.py", "."]).ok().within(SLOW);
    f.trace(&["find", "*.py", "."]).ok().within(SLOW);
}

#[test]
fn git_commands_within_budget() {
    let f = standard_repo();
    f.trace(&["history", "src/app.py"]).ok().within(SLOW);
    f.trace(&["blame", "src/app.py"]).ok().within(SLOW);
    f.trace(&["status"]).ok().within(SLOW);
}

#[test]
fn primer_within_budget() {
    let f = standard_repo();
    f.trace(&["cache", "clear", "--all"]).ok();
    f.trace(&["context"]).ok().within(SLOW);
}

/// Dead-cache detector. Sized so a non-functioning cache is *measurable*:
/// a whole-repo `info` over 360 branchy files re-extracts every one of
/// them when cold. With a working cache the warm pass reuses all 360
/// entries and is dramatically faster; a cache that silently stopped
/// working re-extracts on every run, so warm stays as slow as cold and
/// this fails. The ratio is the signal that survives absolute-threshold
/// tuning — not a micro-benchmark.
#[test]
fn warm_whole_repo_is_far_faster_than_cold_on_a_large_repo() {
    let f = large_repo();
    f.trace(&["cache", "clear", "--all"]).ok();

    let cold = f.trace(&["info", ".", "--json"]);
    cold.ok();
    // Sanity: the fixture really is large (a dead cache on five files would
    // not move the needle, so the fixture size is itself the test).
    // large_repo() writes exactly 12 packages × 30 modules = 360 .py
    // files; `info .` analyzes every one. Pinning the exact count both
    // documents the fixture size and fails loudly if the generator or the
    // directory walk ever drops files.
    assert_eq!(
        cold.json()["counts"]["files"].as_i64().unwrap(),
        360,
        "large_repo() must yield exactly 360 analyzed files: {} files",
        cold.json()["counts"]["files"]
    );

    let warm = f.trace(&["info", ".", "--json"]);
    warm.ok();

    // Loose tripwire: a working warm pass is far under cold. If the cache
    // is dead, warm ≈ cold (full re-extraction) and this fails. The 60%
    // ceiling is generous headroom — a healthy warm pass on this fixture
    // is a small fraction of cold, not 60% of it.
    let ceiling = cold.elapsed.mul_f64(0.60).max(Duration::from_millis(400));
    assert!(
        warm.elapsed <= ceiling,
        "warm whole-repo info ({:?}) not far below cold ({:?}) on a \
         360-file repo — the cache is not serving warm reads",
        warm.elapsed,
        cold.elapsed
    );
}

/// Opt-in, artifact-producing measurements for the performance plan. This is
/// ignored so the ordinary speed tripwires stay cheap:
///
/// TRACE_BENCHMARK=1 TRACE_BENCHMARK_OUTPUT=/absolute/path.json \
/// TRACE_BIN=/absolute/path/to/trace cargo test --test speed \
/// representative_workloads -- --ignored --exact --nocapture
#[test]
#[ignore]
fn representative_workloads() {
    if std::env::var("TRACE_BENCHMARK").as_deref() != Ok("1") {
        panic!("set TRACE_BENCHMARK=1 to run the destructive-in-scratch benchmark");
    }
    let output = PathBuf::from(
        std::env::var("TRACE_BENCHMARK_OUTPUT")
            .expect("TRACE_BENCHMARK_OUTPUT must name the result artifact"),
    );
    let binary =
        fs::canonicalize(trace_bin()).expect("TRACE_BIN must be an absolute existing binary");
    let next_source = Path::new("/Users/jordan/Developer/references/next.js");
    assert!(
        next_source.join(".git").exists(),
        "Next.js reference checkout is missing"
    );

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let scratch = std::env::temp_dir().join(format!("trace-benchmark-{stamp}"));
    fs::create_dir_all(&scratch).unwrap();
    let next = scratch.join("next.js");
    let tracer = scratch.join("tracer");
    clone_shared(
        next_source,
        &next,
        "02a73c59bbef2b56380fd5e600e64a3f0d4506f9",
    );
    clone_shared(
        Path::new("/Users/jordan/dotfiles"),
        &tracer,
        "0dd1cbb73291194eecf9b9199c291dea6758bc1c",
    );
    let fixture = benchmark_fixture();
    let mutations = measure_mutations(&binary, &fixture.root);
    let concurrent = measure_concurrent_agents(&binary, &fixture.root);
    let hook = measure_hook(&binary, &fixture.root);

    let mut repositories = serde_json::Map::new();
    for (name, root, read_file, symbol, scope, language) in [
        (
            "nextjs",
            next.as_path(),
            "packages/next/src/server/next-server.ts",
            "NextServer",
            "packages/next/src/server",
            "typescript",
        ),
        (
            "tracer",
            tracer.as_path(),
            "tools/tracer/src/main.rs",
            "main",
            "tools/tracer/src",
            "rust",
        ),
        (
            "multilingual",
            fixture.root.as_path(),
            "src/python.py",
            "shared_token",
            "src",
            "python",
        ),
    ] {
        let workloads = workload_args(read_file, symbol, scope, language);
        let cold = measure_set(&binary, root, &workloads, true);
        measured_run(
            &binary,
            root,
            &["cache".into(), "build".into(), ".".into()],
            &[],
        );
        let warm = measure_set(&binary, root, &workloads, false);
        let subprocess_counts = measure_subprocess_counts(&binary, root, &workloads, &scratch);
        repositories.insert(
            name.into(),
            serde_json::json!({
                "revision": git_output(root, &["rev-parse", "HEAD"]),
                "input": root,
                "cold_tracer_cache": cold,
                "warm_tracer_cache": warm,
                "subprocess_counts": subprocess_counts,
            }),
        );
    }

    let identity = serde_json::json!({
        "absolute_path": binary,
        "sha256": command_output("shasum", &["-a", "256", binary.to_str().unwrap()])
            .split_whitespace().next().unwrap(),
        "source_revision": git_output(Path::new("/Users/jordan/dotfiles"), &["rev-parse", "HEAD"]),
    });
    let document = serde_json::json!({
        "binary": identity,
        "sample_count": {"cold": 3, "warm": 5, "state": 5},
        "measurement": {
            "wall_time": "Instant around complete process",
            "peak_rss": "/usr/bin/time -l maximum resident set size",
            "output_bytes": "stdout plus stderr excluding time diagnostics",
            "subprocess_count": "separate PATH observer runs, excluded from latency and RSS samples",
        },
        "repositories": repositories,
        "mutations": mutations,
        "concurrent_independent_agents": concurrent,
        "twenty_file_hook_event": hook,
    });
    fs::write(&output, serde_json::to_vec_pretty(&document).unwrap()).unwrap();
    fs::remove_dir_all(&scratch).unwrap();
    println!("wrote {}", output.display());
}

fn clone_shared(source: &Path, destination: &Path, revision: &str) {
    let status = Command::new("git")
        .args(["clone", "--quiet", "--shared"])
        .arg(source)
        .arg(destination)
        .status()
        .unwrap();
    assert!(status.success(), "failed to clone {}", source.display());
    let checkout = Command::new("git")
        .args(["checkout", "--quiet", "--detach", revision])
        .current_dir(destination)
        .status()
        .unwrap();
    assert!(
        checkout.success(),
        "failed to pin {} at {revision}",
        source.display()
    );
}

fn benchmark_fixture() -> Fixture {
    let fixture = Fixture::new();
    for i in 0..20 {
        fixture.write(
            &format!("src/module_{i:02}.py"),
            &format!("def shared_token_{i}(value):\n    return value if value else {i}\n"),
        );
    }
    fixture.write(
        "src/python.py",
        "def shared_token(value):\n    return value if value else 0\n",
    );
    fixture.write(
        "src/typescript.ts",
        "export function typedToken(value: number) { return value > 0 ? value : 0; }\n",
    );
    fixture.write(
        "src/rust.rs",
        "pub fn rust_token(value: i32) -> i32 { if value > 0 { value } else { 0 } }\n",
    );
    fixture.write(
        "src/php.php",
        "<?php function php_token($value) { return $value ?: 0; }\n",
    );
    fixture.commit("benchmark multilingual fixture");
    fixture.write(
        "docs/revision-marker.md",
        "second revision for deployment-ref movement\n",
    );
    fixture.commit("benchmark deployment revision");
    fixture
}

fn workload_args(
    read_file: &str,
    symbol: &str,
    scope: &str,
    language: &str,
) -> Vec<(&'static str, Vec<String>)> {
    vec![
        (
            "read_30_lines",
            vec!["read", read_file, "--lines", "1:30", "--json"],
        ),
        ("info", vec!["info", read_file, "--json"]),
        ("structure", vec!["structure", read_file, "--json"]),
        (
            "grep_bare_name",
            vec!["grep", symbol, "--path", scope, "--json"],
        ),
        (
            "grep_equivalent_regex",
            vec!["grep", &format!("{symbol}.*"), "--path", scope, "--json"],
        ),
        (
            "grep_broad",
            vec!["grep", "function|class|def", "--path", scope, "--json"],
        ),
        (
            "pattern",
            vec![
                "pattern", "$F($A)", "-l", language, "--path", scope, "--json",
            ],
        ),
        ("scoped_reach", vec!["usages", "--path", scope, "--json"]),
        ("primer", vec!["context"]),
    ]
    .into_iter()
    .map(|(name, args)| (name, args.into_iter().map(String::from).collect()))
    .collect()
}

fn measure_set(
    binary: &Path,
    root: &Path,
    workloads: &[(&str, Vec<String>)],
    clear_each_sample: bool,
) -> serde_json::Value {
    let mut results = serde_json::Map::new();
    for (name, args) in workloads {
        if !clear_each_sample {
            measured_run(binary, root, args, &[]);
        }
        let mut samples = Vec::new();
        let sample_count = if clear_each_sample { 3 } else { 5 };
        for _ in 0..sample_count {
            if clear_each_sample {
                let _ = fs::remove_dir_all(root.join(".tracer-cache"));
            }
            samples.push(measured_run(binary, root, args, &[]));
        }
        let mut summary = summarize(samples);
        summary["arguments"] = serde_json::json!(args);
        results.insert((*name).into(), summary);
    }
    serde_json::Value::Object(results)
}

fn measured_run(
    binary: &Path,
    root: &Path,
    args: &[String],
    envs: &[(&str, &str)],
) -> serde_json::Value {
    let start = Instant::now();
    let mut command = Command::new("/usr/bin/time");
    command
        .arg("-l")
        .arg(binary)
        .args(args)
        .current_dir(root)
        .env("HOME", root);
    for key in [
        "AGENT_SESSION_ID",
        "CODEX_THREAD_ID",
        "CLAUDE_CODE_SESSION_ID",
        "TRACER_AGENT_ID",
    ] {
        command.env_remove(key);
    }
    for (key, value) in envs {
        command.env(key, value);
    }
    let output = command.output().unwrap();
    let elapsed_us = start.elapsed().as_micros() as u64;
    let stderr = String::from_utf8_lossy(&output.stderr);
    let rss = stderr.lines().find_map(|line| {
        line.trim()
            .strip_suffix("  maximum resident set size")?
            .trim()
            .parse::<u64>()
            .ok()
    });
    assert!(
        output.status.success(),
        "benchmark command failed: {:?}\n{}",
        args,
        stderr
    );
    assert!(
        !output.stdout.is_empty(),
        "benchmark command returned empty output: {:?}",
        args
    );
    if !matches!(args.first().map(String::as_str), Some("context" | "cache")) {
        let document: serde_json::Value =
            serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
                panic!("benchmark output was not structured JSON for {args:?}: {error}")
            });
        for slot in ["query", "context", "results", "counts"] {
            assert!(
                document.get(slot).is_some(),
                "benchmark output lacks {slot} for {args:?}"
            );
        }
    }
    let stdout_sha256 = hex::encode(Sha256::digest(&output.stdout));
    serde_json::json!({
        "elapsed_us": elapsed_us,
        "peak_rss_bytes": rss,
        "output_bytes": output.stdout.len(),
        "status": output.status.code(),
        "stdout_sha256": stdout_sha256,
        "stdout": String::from_utf8_lossy(&output.stdout),
    })
}

fn summarize(mut samples: Vec<serde_json::Value>) -> serde_json::Value {
    let mut elapsed: Vec<u64> = samples
        .iter()
        .map(|v| v["elapsed_us"].as_u64().unwrap())
        .collect();
    elapsed.sort_unstable();
    let median = elapsed[elapsed.len() / 2];
    let p95 = elapsed[((elapsed.len() as f64 * 0.95).ceil() as usize).saturating_sub(1)];
    let peak_rss = samples
        .iter()
        .filter_map(|v| v["peak_rss_bytes"].as_u64())
        .max();
    let output_bytes = samples
        .iter()
        .map(|v| v["output_bytes"].as_u64().unwrap())
        .max()
        .unwrap();
    let representative_stdout = samples[0]["stdout"].take();
    for sample in &mut samples {
        sample.as_object_mut().unwrap().remove("stdout");
    }
    serde_json::json!({
        "median_us": median,
        "p95_us": p95,
        "range_us": [elapsed[0], elapsed[elapsed.len() - 1]],
        "peak_rss_bytes": peak_rss,
        "output_bytes": output_bytes,
        "representative_stdout": representative_stdout,
        "raw": samples.drain(..).collect::<Vec<_>>(),
    })
}

fn measure_subprocess_counts(
    binary: &Path,
    root: &Path,
    workloads: &[(&str, Vec<String>)],
    scratch: &Path,
) -> serde_json::Value {
    use std::os::unix::fs::PermissionsExt;
    let observer = scratch.join(format!(
        "observer-{}",
        root.file_name().unwrap().to_string_lossy()
    ));
    fs::create_dir_all(&observer).unwrap();
    let log = observer.join("calls.log");
    for name in ["git", "scc", "rg", "sg", "ast-grep", "ctags"] {
        let lookup = Command::new("which").arg(name).output().unwrap();
        if !lookup.status.success() {
            continue;
        }
        let actual = String::from_utf8(lookup.stdout).unwrap().trim().to_string();
        let script = format!(
            "#!/bin/sh\nprintf '%s\\n' '{name}' >> '{}'\nexec '{}' \"$@\"\n",
            log.display(),
            actual
        );
        let wrapper = observer.join(name);
        fs::write(&wrapper, script).unwrap();
        fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let path = format!("{}:{}", observer.display(), std::env::var("PATH").unwrap());
    let mut result = serde_json::Map::new();
    for (name, args) in workloads {
        fs::write(&log, "").unwrap();
        measured_run(binary, root, args, &[("PATH", &path)]);
        let mut counts = BTreeMap::new();
        for called in fs::read_to_string(&log).unwrap().lines() {
            *counts.entry(called.to_string()).or_insert(0_u64) += 1;
        }
        result.insert((*name).to_string(), serde_json::json!({"arguments": args, "backends": counts, "total": counts.values().sum::<u64>()}));
    }
    serde_json::Value::Object(result)
}

fn measure_mutations(binary: &Path, root: &Path) -> serde_json::Value {
    let whole = vec!["info".into(), ".".into(), "--json".into()];
    let file = vec!["info".into(), "src/python.py".into(), "--json".into()];
    let original_python = "def shared_token(value):\n    return value if value else 0\n";
    let original_module = "def shared_token_0(value):\n    return value if value else 0\n";
    let mut states = BTreeMap::new();
    states.insert(
        "content_edit",
        summarize(
            (0..5)
                .map(|_| {
                    fs::write(root.join("src/python.py"), original_python).unwrap();
                    measured_run(binary, root, &file, &[]);
                    fs::write(
                        root.join("src/python.py"),
                        "def shared_token(value):\n    return value + 1\n",
                    )
                    .unwrap();
                    measured_run(binary, root, &file, &[])
                })
                .collect(),
        ),
    );
    fs::write(root.join("src/python.py"), original_python).unwrap();
    states.insert(
        "add",
        summarize(
            (0..5)
                .map(|_| {
                    let _ = fs::remove_file(root.join("src/added.py"));
                    measured_run(binary, root, &whole, &[]);
                    fs::write(
                        root.join("src/added.py"),
                        "def added_token():\n    return 1\n",
                    )
                    .unwrap();
                    let result = measured_run(binary, root, &whole, &[]);
                    fs::remove_file(root.join("src/added.py")).unwrap();
                    result
                })
                .collect(),
        ),
    );
    states.insert(
        "delete",
        summarize(
            (0..5)
                .map(|_| {
                    fs::write(root.join("src/module_00.py"), original_module).unwrap();
                    measured_run(binary, root, &whole, &[]);
                    fs::remove_file(root.join("src/module_00.py")).unwrap();
                    measured_run(binary, root, &whole, &[])
                })
                .collect(),
        ),
    );
    states.insert(
        "rename",
        summarize(
            (0..5)
                .map(|_| {
                    let _ = fs::remove_file(root.join("src/renamed.py"));
                    fs::write(root.join("src/module_00.py"), original_module).unwrap();
                    measured_run(binary, root, &whole, &[]);
                    fs::rename(root.join("src/module_00.py"), root.join("src/renamed.py")).unwrap();
                    let result = measured_run(binary, root, &whole, &[]);
                    fs::rename(root.join("src/renamed.py"), root.join("src/module_00.py")).unwrap();
                    result
                })
                .collect(),
        ),
    );
    let head = git_output(root, &["rev-parse", "HEAD"]);
    let parent = git_output(root, &["rev-parse", "HEAD~1"]);
    states.insert(
        "deployment_ref_only",
        summarize(
            (0..5)
                .map(|_| {
                    update_ref(root, "refs/remotes/origin/production", &parent);
                    measured_run(binary, root, &file, &[]);
                    update_ref(root, "refs/remotes/origin/production", &head);
                    assert_eq!(git_output(root, &["rev-parse", "HEAD"]), head);
                    assert_eq!(
                        git_output(root, &["rev-parse", "refs/remotes/origin/production"]),
                        head
                    );
                    measured_run(binary, root, &file, &[])
                })
                .collect(),
        ),
    );
    states.insert("deployment_ref", serde_json::json!({"head": head, "moved_from": parent, "moved_ref": "refs/remotes/origin/production"}));
    serde_json::to_value(states).unwrap()
}

fn update_ref(root: &Path, name: &str, value: &str) {
    let status = Command::new("git")
        .args(["update-ref", name, value])
        .current_dir(root)
        .status()
        .unwrap();
    assert!(status.success(), "failed to move {name} to {value}");
}

fn measure_concurrent_agents(binary: &Path, root: &Path) -> serde_json::Value {
    let mut samples = Vec::new();
    for index in 0..5 {
        let start = Instant::now();
        let first = Command::new("/usr/bin/time")
            .arg("-l")
            .arg(binary)
            .args(["context", "src/python.py"])
            .current_dir(root)
            .env_remove("AGENT_SESSION_ID")
            .env_remove("CODEX_THREAD_ID")
            .env_remove("CLAUDE_CODE_SESSION_ID")
            .env_remove("TRACER_AGENT_ID")
            .env("HOME", root)
            .env("AGENT_SESSION_ID", format!("bench-{index}"))
            .env("TRACER_AGENT_ID", "first")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let second = Command::new("/usr/bin/time")
            .arg("-l")
            .arg(binary)
            .args(["context", "src/typescript.ts"])
            .current_dir(root)
            .env_remove("AGENT_SESSION_ID")
            .env_remove("CODEX_THREAD_ID")
            .env_remove("CLAUDE_CODE_SESSION_ID")
            .env_remove("TRACER_AGENT_ID")
            .env("HOME", root)
            .env("AGENT_SESSION_ID", format!("bench-{index}"))
            .env("TRACER_AGENT_ID", "second")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let first_output = first.wait_with_output().unwrap();
        let second_output = second.wait_with_output().unwrap();
        assert!(first_output.status.success() && second_output.status.success());
        assert!(!first_output.stdout.is_empty() && !second_output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&first_output.stdout).contains("[git:"));
        assert!(String::from_utf8_lossy(&second_output.stdout).contains("[git:"));
        let peak_rss = [
            first_output.stderr.as_slice(),
            second_output.stderr.as_slice(),
        ]
        .into_iter()
        .filter_map(|bytes| {
            String::from_utf8_lossy(bytes).lines().find_map(|line| {
                line.trim()
                    .strip_suffix("  maximum resident set size")?
                    .trim()
                    .parse::<u64>()
                    .ok()
            })
        })
        .max();
        let combined = [
            first_output.stdout.as_slice(),
            second_output.stdout.as_slice(),
        ]
        .concat();
        samples.push(serde_json::json!({
            "elapsed_us": start.elapsed().as_micros() as u64,
            "peak_rss_bytes": peak_rss,
            "output_bytes": combined.len(),
            "status": 0,
            "stdout_sha256": hex::encode(Sha256::digest(&combined)),
            "stdout": String::from_utf8_lossy(&combined),
        }));
    }
    summarize(samples)
}

fn measure_hook(binary: &Path, _root: &Path) -> serde_json::Value {
    let fixture = Fixture::new();
    for index in 0..20 {
        fixture.write(
            &format!("hook/file_{index:02}.py"),
            &format!("hook_token = {index}\n"),
        );
    }
    fixture.commit("exact twenty-file hook fixture");
    let support = fixture.root.join("benchmark-support");
    fs::create_dir_all(&support).unwrap();
    let archive = std::env::var("TRACE_BENCHMARK_HOOK_ARCHIVE").unwrap_or_else(|_| {
        "/Users/jordan/dotfiles/docs/agents/012-tracer-performance/artifacts/enrich-on-read-baseline.tar.gz".to_string()
    });
    let unpacked = support.join("hook");
    fs::create_dir_all(&unpacked).unwrap();
    let unpack = Command::new("tar")
        .args(["-xzf", &archive, "-C"])
        .arg(&unpacked)
        .status()
        .unwrap();
    assert!(unpack.success());
    let hook = unpacked.join("enrich_on_read.py");
    let bin_dir = support.join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    std::os::unix::fs::symlink(binary, bin_dir.join("trace")).unwrap();
    assert_eq!(fs::canonicalize(bin_dir.join("trace")).unwrap(), binary);
    let path = format!("{}:{}", bin_dir.display(), std::env::var("PATH").unwrap());
    let event = serde_json::json!({
        "tool_name": "Grep",
        "session_id": "benchmark-hook",
        "agent_id": "baseline",
        "cwd": fixture.root,
        "tool_input": {"pattern": "hook_token", "path": fixture.root},
    })
    .to_string();
    let cold = summarize(
        (0..5)
            .map(|_| {
                let _ = fs::remove_dir_all(fixture.root.join(".tracer-cache"));
                hook_run(&hook, &fixture.root, &path, &event)
            })
            .collect(),
    );
    hook_run(&hook, &fixture.root, &path, &event);
    let warm = summarize(
        (0..5)
            .map(|_| hook_run(&hook, &fixture.root, &path, &event))
            .collect(),
    );
    let implementation_sha256 = command_output("shasum", &["-a", "256", &archive])
        .split_whitespace()
        .next()
        .unwrap()
        .to_string();
    serde_json::json!({
        "implementation_archive": archive,
        "implementation_sha256": implementation_sha256,
        "fixture_revision": git_output(&fixture.root, &["rev-parse", "HEAD"]),
        "binary_link": bin_dir.join("trace"),
        "cold_first_session": cold,
        "warm_repeated_session": warm,
    })
}

fn hook_run(hook: &Path, root: &Path, path: &str, event: &str) -> serde_json::Value {
    let start = Instant::now();
    let mut child = Command::new("/usr/bin/time")
        .args(["-l", "python3"])
        .arg(hook)
        .current_dir(root)
        .env("PATH", path)
        .env_remove("AGENT_SESSION_ID")
        .env_remove("CODEX_THREAD_ID")
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .env_remove("TRACER_AGENT_ID")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    use std::io::Write;
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(event.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success() && !output.stdout.is_empty(),
        "hook failed: {stderr}"
    );
    let envelope: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("hook feedback JSON");
    let context = envelope
        .pointer("/hookSpecificOutput/additionalContext")
        .or_else(|| envelope.pointer("/additionalContext"))
        .and_then(|value| value.as_str())
        .expect("hook additionalContext");
    let full_blocks = context
        .lines()
        .filter(|line| {
            line.starts_with(&format!("{}/hook/file_", root.display())) && line.ends_with(".py")
        })
        .count();
    assert_eq!(
        full_blocks, 20,
        "expected twenty actual file-context blocks: {context}"
    );
    let rss = stderr.lines().find_map(|line| {
        line.trim()
            .strip_suffix("  maximum resident set size")?
            .trim()
            .parse::<u64>()
            .ok()
    });
    serde_json::json!({
        "elapsed_us": start.elapsed().as_micros() as u64,
        "peak_rss_bytes": rss,
        "output_bytes": output.stdout.len(),
        "status": output.status.code(),
        "stdout_sha256": hex::encode(Sha256::digest(&output.stdout)),
        "stdout": String::from_utf8_lossy(&output.stdout),
    })
}

fn git_output(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

fn command_output(command: &str, args: &[&str]) -> String {
    let output = Command::new(command).args(args).output().unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap()
}
