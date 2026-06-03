//! Shared black-box harness for the `review-prompt` CLI test suite.
//!
//! Every integration test drives the CLI as a subprocess via [`review`]. The
//! binary under test is resolved from the `REVIEW_PROMPT_BIN` environment
//! variable, defaulting to the release build beside this crate
//! (`../target/release/review-prompt`), so the suite runs against the built
//! binary without requiring it on `PATH`.
//!
//! Nothing here imports or links the tool's internals. The only contract is
//! the CLI's observable surface: exit code, stdout, stderr, and the JSON
//! envelope shape.
//!
//! Two model states matter. Tests that actually run the model gate on
//! [`model_present`] and skip cleanly when the 5.3 GB file is absent. The
//! no-model error paths and every argument/usage behavior run unconditionally:
//! they point `PROMPT_REVIEWER_MODEL` at a path that does not exist (see
//! [`with_missing_model`]) so they exercise "no model present" without
//! disturbing the real cached model.

use std::ffi::OsStr;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// The environment variable the tool reads for the on-disk model path. The
/// path is the user-overridable lever; the suite uses it to point at a
/// nonexistent file for the no-model paths.
pub const MODEL_PATH_ENV: &str = "PROMPT_REVIEWER_MODEL";

/// Resolve the `review-prompt` binary under test.
///
/// `REVIEW_PROMPT_BIN` overrides the default, so the suite can be pointed at a
/// specific build. The default is the release binary beside this test crate,
/// `<crate>/../target/release/review-prompt`.
pub fn review_prompt_bin() -> String {
    if let Ok(explicit) = std::env::var("REVIEW_PROMPT_BIN") {
        return explicit;
    }
    // CARGO_MANIFEST_DIR is `tools/prompt-reviewer/tests`; the binary builds to
    // `tools/prompt-reviewer/target/release/review-prompt`.
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .expect("test crate has a parent (the package root)")
        .join("target")
        .join("release")
        .join("review-prompt")
        .to_string_lossy()
        .into_owned()
}

/// Outcome of one CLI invocation.
pub struct Run {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
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

    /// Assert the process exited non-zero; panic with diagnostics otherwise.
    pub fn failed(&self) -> &Self {
        assert_ne!(
            self.code, 0,
            "expected a non-zero exit\n--- stdout ---\n{}\n--- stderr ---\n{}",
            self.stdout, self.stderr
        );
        self
    }

    /// Combined stdout+stderr — error text lands on either stream (clap prints
    /// usage to stderr; anyhow errors print to stderr; commands print to
    /// stdout), so message assertions read both.
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
}

/// Invoke the binary with `args`, no stdin, inheriting the environment.
pub fn review<I, S>(args: I) -> Run
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    run(args, None, &[])
}

/// Invoke the binary with `args` and `stdin` piped in.
pub fn review_stdin<I, S>(args: I, stdin: &str) -> Run
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    run(args, Some(stdin), &[])
}

/// Invoke the binary with `args` and extra environment variables.
pub fn review_env<I, S>(args: I, env: &[(&str, &str)]) -> Run
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    run(args, None, env)
}

/// The one runner. Spawns the binary, optionally feeds `stdin`, applies extra
/// `env`, and captures the outcome.
pub fn run<I, S>(args: I, stdin: Option<&str>, env: &[(&str, &str)]) -> Run
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let bin = review_prompt_bin();
    let mut cmd = Command::new(&bin);
    for a in args {
        cmd.arg(a);
    }
    for (k, v) in env {
        cmd.env(k, v);
    }
    cmd.stdin(if stdin.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    });
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .unwrap_or_else(|e| panic!("failed to spawn `{bin}`: {e} (build it with `cargo build --release`)"));
    if let Some(text) = stdin {
        child
            .stdin
            .take()
            .expect("stdin was piped")
            .write_all(text.as_bytes())
            .expect("writing to child stdin");
    }
    let out = child.wait_with_output().expect("waiting for the child");
    Run {
        code: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

/// A temp path that is guaranteed not to exist, for pointing
/// `PROMPT_REVIEWER_MODEL` at a missing model. Unique per call so parallel
/// tests never collide.
pub fn missing_model_path() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, Ordering::SeqCst);
    std::env::temp_dir()
        .join(format!("prompt-reviewer-absent-model-{nanos}-{seq}.gguf"))
        .to_string_lossy()
        .into_owned()
}

/// The `PROMPT_REVIEWER_MODEL` env pair pointing at a guaranteed-absent file —
/// the lever for exercising "no model present" without touching the real
/// cached model.
pub fn with_missing_model() -> (String, String) {
    (MODEL_PATH_ENV.to_string(), missing_model_path())
}

/// Whether the real model is present, decided by the tool's own contract:
/// `doctor` exits 0 when the model is on disk and non-zero when it is not.
/// Model-gated tests call this and return early when it is false, so the suite
/// passes on a machine without the 5.3 GB file.
pub fn model_present() -> bool {
    review(["doctor"]).code == 0
}
