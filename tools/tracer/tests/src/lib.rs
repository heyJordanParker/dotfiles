//! Shared black-box harness for the `trace` CLI test suite.
//!
//! Every integration test drives the CLI as a subprocess via [`trace`]. The
//! binary under test is resolved from the `TRACE_BIN` environment variable,
//! defaulting to `trace` on `PATH`, so the suite can run against any built
//! `trace` binary without changing the assertions.
//!
//! Nothing here imports or links tracer internals. The only contract is the
//! CLI's observable surface: exit code, stdout, stderr, the `--json`
//! document shape, and wall-clock latency.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Resolve the `trace` binary under test.
///
/// `TRACE_BIN` overrides the default (`trace` on PATH), so the suite can be
/// pointed at a specific build, e.g.
/// `TRACE_BIN=/path/to/target/release/trace`.
pub fn trace_bin() -> String {
    std::env::var("TRACE_BIN").unwrap_or_else(|_| "trace".to_string())
}

/// Outcome of one CLI invocation.
pub struct Run {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
    /// Wall-clock duration of the subprocess, start to exit.
    pub elapsed: Duration,
}

impl Run {
    /// Assert the process exited 0; panic with full diagnostics otherwise.
    pub fn ok(&self) -> &Self {
        assert_eq!(
            self.code, 0,
            "expected exit 0, got {}\n--- stdout ---\n{}\n--- stderr ---\n{}",
            self.code, self.stdout, self.stderr
        );
        self
    }

    /// Assert a specific non-zero exit code.
    pub fn code_is(&self, want: i32) -> &Self {
        assert_eq!(
            self.code, want,
            "expected exit {}, got {}\n--- stdout ---\n{}\n--- stderr ---\n{}",
            want, self.code, self.stdout, self.stderr
        );
        self
    }

    /// Combined stdout+stderr — error text lands on either stream across
    /// commands (click prints usage to stdout, explicit errors to stderr).
    pub fn combined(&self) -> String {
        format!("{}{}", self.stdout, self.stderr)
    }

    /// Assert stdout parses as JSON; return the value.
    pub fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.stdout).unwrap_or_else(|e| {
            panic!(
                "stdout was not valid JSON ({e})\n--- stdout ---\n{}\n--- stderr ---\n{}",
                self.stdout, self.stderr
            )
        })
    }

    /// Assert the run finished within `budget`; panic with the measured time
    /// otherwise. Thresholds are deliberately generous (see suite README) so
    /// they catch gross regressions without flaking on a loaded CI box.
    pub fn within(&self, budget: Duration) -> &Self {
        assert!(
            self.elapsed <= budget,
            "command took {:?}, budget was {:?}\n--- stdout ---\n{}",
            self.elapsed,
            budget,
            self.stdout
        );
        self
    }
}

/// Invoke the `trace` binary with `args`, working dir `cwd`, returning the
/// captured outcome. Inherits the parent environment so the external binaries
/// (`scc`, `rg`, `ctags`, `ast-grep`, `git`) resolve on PATH.
pub fn trace<I, S>(cwd: &Path, args: I) -> Run
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    trace_env(cwd, args, &[])
}

/// Same as [`trace`] but with extra environment variables — used for the
/// `TRACER_CCN_BACKEND` toggle coverage.
pub fn trace_env<I, S>(cwd: &Path, args: I, env: &[(&str, &str)]) -> Run
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut cmd = Command::new(trace_bin());
    cmd.current_dir(cwd);
    for a in args {
        cmd.arg(a);
    }
    for (k, v) in env {
        cmd.env(k, v);
    }
    let start = Instant::now();
    let out = cmd
        .output()
        .unwrap_or_else(|e| panic!("failed to spawn `{}`: {e}", trace_bin()));
    let elapsed = start.elapsed();
    Run {
        code: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        elapsed,
    }
}

#[allow(non_upper_case_globals)] // project naming rule bans ALL_CAPS for our own identifiers
static fixture_seq: AtomicU64 = AtomicU64::new(0);

/// A throwaway git repository on disk, deleted on drop.
///
/// Commits are created with an explicit, hermetic environment (fixed
/// author/committer, isolated `HOME`) so the suite never depends on the
/// developer's git config — the same pattern the prior pytest `test_glob.py`
/// used. `git commit` is run via a direct subprocess (not a shell), which is
/// also what keeps the suite independent of any commit-gating shell hooks.
pub struct Fixture {
    pub root: PathBuf,
}

impl Fixture {
    /// Create an empty initialized git repo in a unique temp directory.
    pub fn new() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let seq = fixture_seq.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!("trace-fixture-{}-{}", nanos, seq));
        fs::create_dir_all(&root).unwrap();
        let f = Fixture { root };
        f.git(&["init", "--quiet"]);
        f.git(&["config", "user.email", "test@example.com"]);
        f.git(&["config", "user.name", "Tracer Test"]);
        f
    }

    /// Run a git command in the fixture with a hermetic environment.
    pub fn git(&self, args: &[&str]) {
        let status = Command::new("git")
            .args(args)
            .current_dir(&self.root)
            .env_clear()
            .env("PATH", "/opt/homebrew/bin:/usr/bin:/bin:/usr/local/bin")
            .env("HOME", &self.root)
            .env("GIT_AUTHOR_NAME", "Tracer Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Tracer Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .unwrap_or_else(|e| panic!("git {:?} failed to spawn: {e}", args));
        assert!(
            status.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&status.stderr)
        );
    }

    /// Write `contents` to `rel` (creating parent dirs). Returns the abs path.
    pub fn write(&self, rel: &str, contents: &str) -> PathBuf {
        let p = self.root.join(rel);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&p, contents).unwrap();
        p
    }

    /// Write raw bytes (used for the binary-file edge case).
    pub fn write_bytes(&self, rel: &str, bytes: &[u8]) -> PathBuf {
        let p = self.root.join(rel);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&p, bytes).unwrap();
        p
    }

    /// `git add -A && git commit -m msg`.
    pub fn commit(&self, msg: &str) {
        self.git(&["add", "-A"]);
        self.git(&["commit", "--quiet", "-m", msg]);
    }

    /// `git worktree add <abs_path> -b <branch>`. Returns the linked
    /// worktree's absolute path. The linked worktree is cleaned up when
    /// the parent fixture drops (its root is removed recursively); git's
    /// `worktrees` admin dir is also gone with the main repo.
    pub fn add_worktree(&self, sub: &str, branch: &str) -> PathBuf {
        let abs = self.root.join(sub);
        self.git(&[
            "worktree",
            "add",
            "-b",
            branch,
            abs.to_str().expect("worktree path is UTF-8"),
        ]);
        abs
    }

    /// Path to a fixture-relative file as a string (for passing to `trace`).
    pub fn path(&self, rel: &str) -> String {
        self.root.join(rel).to_string_lossy().into_owned()
    }

    /// Run `trace` with the fixture root as the working directory.
    pub fn trace(&self, args: &[&str]) -> Run {
        trace(&self.root, args)
    }

    /// Run `trace` with extra env (backend toggle).
    pub fn trace_env(&self, args: &[&str], env: &[(&str, &str)]) -> Run {
        trace_env(&self.root, args, env)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

impl Default for Fixture {
    fn default() -> Self {
        Self::new()
    }
}

/// A representative multi-language repo used by most correctness tests.
///
/// Layout (committed):
/// ```text
/// src/app.py        imports src.util.helper; branchy main()
/// src/util.py       helper()
/// src/front.tsx     exported component, imports a constant
/// lib/widget.php    a PHP class + function
/// docs/readme.md    non-source file
/// pyproject.toml    package-config marker for the primer
/// ```
pub fn standard_repo() -> Fixture {
    let f = Fixture::new();
    f.write(
        "src/app.py",
        concat!(
            "import os\n",
            "from src.util import helper\n",
            "\n\n",
            "def main(x):\n",
            "    \"\"\"Entry point with real branching.\"\"\"\n",
            "    if x:\n",
            "        return helper(x)\n",
            "    for i in range(10):\n",
            "        if i % 2 == 0:\n",
            "            print(i)\n",
            "    return None\n",
        ),
    );
    f.write(
        "src/util.py",
        "def helper(v):\n    if v > 0:\n        return v + 1\n    return 0\n",
    );
    f.write(
        "src/front.tsx",
        concat!(
            "import { CONST } from './consts';\n",
            "export const Widget = (p: {n: number}) => {\n",
            "  return p.n > CONST ? <div/> : <span/>;\n",
            "};\n",
        ),
    );
    f.write("src/consts.ts", "export const CONST = 5;\n");
    f.write(
        "lib/widget.php",
        concat!(
            "<?php\n",
            "function render($x) {\n",
            "  if ($x) { return 1; }\n",
            "  return 0;\n",
            "}\n",
            "class Widget {\n",
            "  public function show() { return render(1); }\n",
            "}\n",
        ),
    );
    f.write("docs/readme.md", "# Project\n\nDocs only.\n");
    f.write(
        "pyproject.toml",
        "[project]\nname = \"fixture\"\nversion = \"0.1.0\"\n",
    );
    f.commit("init standard repo");
    f
}

/// Normalize the one genuinely non-deterministic axis in a passive-context
/// shoulder so the rest can be pinned by exact equality.
///
/// exempt-(a): the age token is `today - last_modified` bucketed by
/// `passive_context::age`. A hermetic fixture commits at test run time, so
/// the bucket is `today` — except when the commit lands at 23:59:59 UTC and
/// the read happens after the next UTC midnight, where it flips to `1d`.
/// That UTC-boundary flip is the documented non-deterministic axis: the
/// tightest stable invariant is "the age token is one of the freshly-
/// committed buckets". This replaces the live token with the literal
/// `<AGE>` in the ` · age: <tok>` form the canonical shoulder uses (both
/// the full `render` and the dense `render_compact` variant carry the
/// `age:` field), and panics if it is not a fresh-commit bucket, so a
/// wrong age still fails loudly. The age token may itself be a
/// `created→modified` range; the normalizer matches the single fresh-commit
/// token a hermetic same-run commit always produces.
pub fn normalize_age(shoulder: &str) -> String {
    // Fresh-commit buckets only: `today` (same UTC day) or `1d` (one UTC
    // midnight crossed mid-run). Anything else is a real bug, not the axis.
    for tok in ["today", "1d"] {
        let verbose = format!(" · age: {tok}");
        if shoulder.contains(&verbose) {
            return shoulder.replace(&verbose, " · age: <AGE>");
        }
    }
    panic!(
        "shoulder age token is not a fresh-commit bucket (today/1d) — \
         hermetic fixture age regressed: {shoulder:?}"
    );
}

/// Parse the human `cache stats` table into `namespace -> entry_count`.
/// Lines look like: `  file            5 entries     2.6 KB`.
pub fn parse_stats_table(stdout: &str) -> BTreeMap<String, u64> {
    let mut out = BTreeMap::new();
    for line in stdout.lines() {
        let toks: Vec<&str> = line.split_whitespace().collect();
        if toks.len() >= 3 && toks[2] == "entries" {
            if let Ok(n) = toks[1].parse::<u64>() {
                out.insert(toks[0].to_string(), n);
            }
        }
    }
    out
}
