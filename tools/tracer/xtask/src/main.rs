//! cargo xtask — workspace automation for the tracer crate.
//!
//! `sync-dist` is the tracer crate's plugin-payload producer, the way
//! `cargo publish` is a crate's release path: run from inside `tools/tracer/`,
//! it produces the generated mirror the Claude Code plugin ships.
//!
//! The plugin marketplace copies only `packages/claude/`, so the launcher's
//! build-from-source fallback needs the tracer crate source physically inside
//! that payload — it cannot reach the canonical `tools/tracer/`. So the
//! mirror at `packages/claude/bin/tracer-dist/crate/` is a GENERATED artifact,
//! never hand-edited: this xtask is its single writer. Edit the tracer at
//! `tools/tracer/` and re-run `cargo xtask sync-dist` (setup.sh does, after
//! the tracer release build + binary install).
//!
//!   cargo xtask sync-dist            regenerate the mirror (idempotent)
//!   cargo xtask sync-dist --check    exit non-zero if the mirror has drifted
//!                                    from the tracer source (the drift guard)
//!   cargo xtask build-bin            build every shipped prebuilt from the
//!                                    mirror and stamp it with the mirror's hash
//!   cargo xtask build-bin --check    exit non-zero if a prebuilt is missing or
//!                                    older than the mirror it ships beside
//!
//! `build-bin` is the second half of the same payload. The launcher execs a
//! committed prebuilt before it considers anything else, and setup.sh cannot
//! run on Linux (it opens with xcode-select and brew bundle), so a Linux host
//! gets whatever binary is in the tree and nothing else. Until this task
//! existed nothing produced them: linux-x86_64 was committed once on 17 May and
//! never rebuilt. `scripts/tracer.py` now calls both tasks from `sync.py`, so
//! the payload is rebuilt at the commit that changes it rather than reported on.
//!
//! Both Linux prebuilts cross-compile on the host through cargo-zigbuild. zig
//! carries the Linux linker, libc, and C compiler the eleven tree-sitter
//! grammars need, which is the whole reason a container was ever involved. That
//! keeps a daemon out of the commit path and moves the glibc floor from a base
//! image tag onto the target triple, where it is chosen on purpose.
//!
//! The crux: `tools/tracer/` is a cargo workspace with this xtask as a member
//! when developed in-repo, but the mirror must build standalone with NO
//! workspace and NO xtask present (a leaked `[workspace]` table referencing a
//! missing `xtask` member fails `cargo build` for plugin users). So the
//! mirrored manifest is the tracer package manifest with its `[workspace]`
//! table stripped, while the canonical workspace lock is copied byte-identically
//! and validated against that standalone manifest.

use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitCode, ExitStatus};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tracer_cli_tests::{median, p95};

/// The glibc version every Linux prebuilt links against, appended to the target
/// triple so zig links the stubs for exactly that release. 2.17 is RHEL 7 era,
/// below any distribution still in service, and pinning it here is what makes
/// the floor a decision rather than a side effect of whichever machine built the
/// binary. The produced binary needs no symbol above it.
const GLIBC: &str = "2.17";

const BENCH_CHILD_DEADLINE: Duration = Duration::from_secs(60);

/// The prebuilts the plugin ships: directory under `tracer-dist/bin`, and the
/// Rust target that builds it. `None` means the host — nothing cross-compiles a
/// macOS binary, so mac-arm64 builds natively and `build-bin` needs an
/// Apple-silicon host.
const PREBUILTS: &[(&str, Option<&str>)] = &[
    ("mac-arm64", None),
    ("linux-x86_64", Some("x86_64-unknown-linux-gnu")),
    ("linux-arm64", Some("aarch64-unknown-linux-gnu")),
];

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let task = args.next();

    match task.as_deref() {
        Some("sync-dist") => sync_dist(args.next().as_deref() == Some("--check")),
        Some("build-bin") => build_bin(args.next().as_deref() == Some("--check")),
        Some("bench") => bench(args.collect()),
        Some(other) => {
            eprintln!("xtask: unknown task `{other}` (known: sync-dist, build-bin, bench)");
            ExitCode::from(2)
        }
        None => {
            eprintln!(
                "xtask: missing task (known: sync-dist [--check], build-bin [--check], bench)"
            );
            ExitCode::from(2)
        }
    }
}

struct BenchArgs {
    repo: PathBuf,
    baseline: PathBuf,
    candidate: PathBuf,
    samples: usize,
    json: bool,
    keep_worktrees: bool,
}

struct BenchWorktree {
    repo: PathBuf,
    path: PathBuf,
    keep: bool,
}

impl Drop for BenchWorktree {
    fn drop(&mut self) {
        if self.keep {
            return;
        }
        let _ = Command::new("git")
            .arg("-C")
            .arg(&self.repo)
            .args(["worktree", "remove", "--force"])
            .arg(&self.path)
            .output();
        let _ = fs::remove_dir_all(&self.path);
    }
}

enum Session {
    None,
    FirstTouch,
    Repeat,
}

enum BenchCommand {
    Trace { args: Vec<String>, session: Session },
    Edit { args: Vec<String> },
    Concurrent { files: Vec<String> },
}

struct BenchSample {
    elapsed_us: u64,
    child_median_us: Option<u64>,
    retries: usize,
}

struct SustainedSample {
    elapsed_us: u64,
    retries: usize,
}

struct BenchWorkload {
    name: String,
    command: BenchCommand,
}

fn bench(args: Vec<String>) -> ExitCode {
    let mut input = match parse_bench_args(args) {
        Ok(input) => input,
        Err(error) => return bench_usage_error(&error),
    };
    if !is_worktree(&input.repo) {
        return bench_usage_error(&format!(
            "--repo is not a git worktree: {}",
            input.repo.display()
        ));
    }
    for (name, binary) in [
        ("--baseline", &input.baseline),
        ("--candidate", &input.candidate),
    ] {
        if !is_executable(binary) {
            return bench_usage_error(&format!("{name} is not executable: {}", binary.display()));
        }
    }
    input.baseline = match fs::canonicalize(&input.baseline) {
        Ok(binary) => binary,
        Err(error) => {
            eprintln!("xtask bench: resolve --baseline: {error}");
            return ExitCode::from(1);
        }
    };
    input.candidate = match fs::canonicalize(&input.candidate) {
        Ok(binary) => binary,
        Err(error) => {
            eprintln!("xtask bench: resolve --candidate: {error}");
            return ExitCode::from(1);
        }
    };

    match run_bench(&input) {
        Ok((rows, sustained, failed)) => {
            print_bench(&rows, &sustained, input.json);
            if failed.is_empty() {
                ExitCode::SUCCESS
            } else {
                eprintln!("xtask bench: above 1.03: {}", failed.join(", "));
                ExitCode::from(1)
            }
        }
        Err(error) => {
            eprintln!("xtask bench: {error}");
            ExitCode::from(1)
        }
    }
}

fn parse_bench_args(args: Vec<String>) -> Result<BenchArgs, String> {
    let mut repo = None;
    let mut baseline = None;
    let mut candidate = None;
    let mut samples = 60;
    let mut json = false;
    let mut keep_worktrees = false;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--repo" => repo = Some(PathBuf::from(next_bench_value(&mut args, "--repo")?)),
            "--baseline" => {
                baseline = Some(PathBuf::from(next_bench_value(&mut args, "--baseline")?))
            }
            "--candidate" => {
                candidate = Some(PathBuf::from(next_bench_value(&mut args, "--candidate")?))
            }
            "--samples" => {
                samples = next_bench_value(&mut args, "--samples")?
                    .parse()
                    .map_err(|_| "--samples must be an integer".to_string())?;
            }
            "--json" => json = true,
            "--keep-worktrees" => keep_worktrees = true,
            _ => return Err(format!("unknown bench argument `{arg}`")),
        }
    }
    if !(1..=480).contains(&samples) {
        return Err("--samples must be between 1 and 480".into());
    }
    Ok(BenchArgs {
        repo: repo.ok_or_else(|| "missing --repo".to_string())?,
        baseline: baseline.ok_or_else(|| "missing --baseline".to_string())?,
        candidate: candidate.ok_or_else(|| "missing --candidate".to_string())?,
        samples,
        json,
        keep_worktrees,
    })
}

fn next_bench_value(args: &mut std::vec::IntoIter<String>, flag: &str) -> Result<String, String> {
    args.next().ok_or_else(|| format!("{flag} needs a value"))
}

fn bench_usage_error(error: &str) -> ExitCode {
    eprintln!("xtask bench: {error}");
    ExitCode::from(2)
}

fn is_worktree(repo: &Path) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "--is-inside-work-tree"])
        .output()
        .map(|output| {
            output.status.success() && String::from_utf8_lossy(&output.stdout).trim() == "true"
        })
        .unwrap_or(false)
}

fn is_executable(binary: &Path) -> bool {
    fs::metadata(binary)
        .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

fn run_bench(
    input: &BenchArgs,
) -> Result<(Vec<serde_json::Value>, serde_json::Value, Vec<String>), String> {
    prune_bench_worktrees(&input.repo)?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("read clock: {error}"))?
        .as_nanos();
    let scratch = std::env::temp_dir().join(format!("tracer-bench-{}-{stamp}", std::process::id()));
    fs::create_dir_all(&scratch)
        .map_err(|error| format!("create {}: {error}", scratch.display()))?;
    let result = (|| {
        let baseline_tree =
            add_bench_worktree(&input.repo, scratch.join("baseline"), input.keep_worktrees)?;
        let candidate_tree =
            add_bench_worktree(&input.repo, scratch.join("candidate"), input.keep_worktrees)?;
        let files = discover_bench_files(&baseline_tree.path)?;
        let edits = discover_bench_edits(&baseline_tree.path, &files)?;
        let workloads = bench_workloads(&files)?;

        apply_bench_edits(&baseline_tree.path, &edits)?;
        apply_bench_edits(&candidate_tree.path, &edits)?;
        warm_worktree(&input.baseline, &baseline_tree.path, &workloads, "baseline")?;
        warm_worktree(
            &input.candidate,
            &candidate_tree.path,
            &workloads,
            "candidate",
        )?;
        let mut rows = Vec::new();
        let mut failed = Vec::new();
        for workload in &workloads {
            let mut samples = input.samples;
            loop {
                let row = measure_workload(
                    workload,
                    samples,
                    &input.baseline,
                    &baseline_tree.path,
                    &input.candidate,
                    &candidate_tree.path,
                )?;
                let lower = row["interval"][0].as_f64().unwrap();
                let upper = row["interval"][1].as_f64().unwrap();
                if lower <= 1.03 && upper > 1.03 && samples < 480 {
                    samples = (samples * 2).min(480);
                    continue;
                }
                if upper > 1.03 {
                    failed.push(workload.name.clone());
                }
                rows.push(row);
                break;
            }
        }
        let mut baseline_rounds = Vec::with_capacity(10);
        let mut candidate_rounds = Vec::with_capacity(10);
        for round in 0..10 {
            if round % 2 == 0 {
                baseline_rounds.push(sustained_context(
                    &input.baseline,
                    &baseline_tree.path,
                    &files.php,
                    "baseline",
                    30,
                )?);
                candidate_rounds.push(sustained_context(
                    &input.candidate,
                    &candidate_tree.path,
                    &files.php,
                    "candidate",
                    30,
                )?);
            } else {
                candidate_rounds.push(sustained_context(
                    &input.candidate,
                    &candidate_tree.path,
                    &files.php,
                    "candidate",
                    30,
                )?);
                baseline_rounds.push(sustained_context(
                    &input.baseline,
                    &baseline_tree.path,
                    &files.php,
                    "baseline",
                    30,
                )?);
            }
        }
        let baseline_sustained = sustained_summary(&baseline_rounds);
        let candidate_sustained = sustained_summary(&candidate_rounds);
        let baseline_samples = baseline_sustained["samples_us"]
            .as_array()
            .unwrap()
            .iter()
            .map(|sample| sample.as_u64().unwrap())
            .collect::<Vec<_>>();
        let candidate_samples = candidate_sustained["samples_us"]
            .as_array()
            .unwrap()
            .iter()
            .map(|sample| sample.as_u64().unwrap())
            .collect::<Vec<_>>();
        let sustained_interval = bootstrap_interval(&baseline_samples, &candidate_samples);
        let sustained = serde_json::json!({
            "name": "sustained context php (10 interleaved rounds at 30/s)",
            "baseline": baseline_sustained,
            "candidate": candidate_sustained,
            "interval": sustained_interval,
        });
        let baseline_complete = sustained["baseline"]["completed"].as_u64().unwrap() == 300;
        let candidate_complete = sustained["candidate"]["completed"].as_u64().unwrap() == 300;
        let baseline_backlog = sustained["baseline"]["max_backlog"].as_u64().unwrap() <= 30;
        let candidate_backlog = sustained["candidate"]["max_backlog"].as_u64().unwrap() <= 30;
        if !baseline_complete
            || !candidate_complete
            || !baseline_backlog
            || !candidate_backlog
            || sustained["interval"][1].as_f64().unwrap() > 1.03
        {
            failed.push("sustained context php".into());
        }
        if input.keep_worktrees {
            eprintln!(
                "xtask bench: kept worktrees at {} and {}",
                baseline_tree.path.display(),
                candidate_tree.path.display()
            );
        }
        Ok((rows, sustained, failed))
    })();
    if !input.keep_worktrees {
        let _ = fs::remove_dir_all(scratch);
    }
    result
}

fn prune_bench_worktrees(repo: &Path) -> Result<(), String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["worktree", "prune"])
        .output()
        .map_err(|error| format!("prune stale bench worktrees: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "prune stale bench worktrees: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

fn add_bench_worktree(repo: &Path, path: PathBuf, keep: bool) -> Result<BenchWorktree, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["worktree", "add", "--detach"])
        .arg(&path)
        .arg("HEAD")
        .output()
        .map_err(|error| format!("add worktree {}: {error}", path.display()))?;
    if !output.status.success() {
        return Err(format!(
            "add worktree {}: {}",
            path.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(BenchWorktree {
        repo: repo.to_path_buf(),
        path,
        keep,
    })
}

struct BenchFiles {
    php: String,
    tsx: String,
    php_dir: String,
    class: String,
    concurrent: Vec<String>,
    php_files: Vec<String>,
}

struct BenchEdits {
    attribute_file: String,
    attribute_line: usize,
    removed_file: String,
    removed_start: usize,
    removed_end: usize,
    deleted_file: String,
    renamed_from: String,
    renamed_to: String,
}

fn discover_bench_files(root: &Path) -> Result<BenchFiles, String> {
    let tracked = tracked_files(root)?;
    let mut directories = BTreeMap::new();
    for path in tracked.iter().filter(|path| is_source_file(path)) {
        let mut components = path.components();
        let Some(directory) = components.next() else {
            continue;
        };
        if components.next().is_some() {
            *directories
                .entry(PathBuf::from(directory.as_os_str()))
                .or_insert(0_usize) += 1;
        }
    }
    let mut directories: Vec<_> = directories.into_iter().collect();
    directories.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    let source_dir = directories
        .first()
        .map(|(path, _)| path)
        .ok_or_else(|| "repo has no top-level source directory".to_string())?;
    let php_path = tracked
        .iter()
        .filter(|path| path.starts_with(source_dir))
        .find(|path| is_extension(path, "php") && count_lines(&root.join(path)).unwrap_or(0) >= 200)
        .cloned()
        .ok_or_else(|| {
            format!(
                "no PHP file with at least 200 lines under {}",
                source_dir.display()
            )
        })?;
    let tsx_path = tracked
        .iter()
        .find(|path| is_extension(path, "tsx"))
        .cloned()
        .ok_or_else(|| "repo has no TSX file".to_string())?;
    let source = fs::read_to_string(root.join(&php_path))
        .map_err(|error| format!("read {}: {error}", php_path.display()))?;
    let class = php_class(&source).ok_or_else(|| format!("no class in {}", php_path.display()))?;
    let php_dir = php_path
        .parent()
        .ok_or_else(|| format!("{} has no parent", php_path.display()))?;
    let concurrent: Vec<_> = tracked
        .iter()
        .filter(|path| path.starts_with(php_dir) && is_extension(path, "php"))
        .cloned()
        .collect();
    if concurrent.len() < 8 {
        return Err(format!(
            "{} has fewer than eight PHP files",
            php_dir.display()
        ));
    }
    let php_files: Vec<_> = concurrent
        .iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect();
    Ok(BenchFiles {
        php: php_path.to_string_lossy().into_owned(),
        tsx: tsx_path.to_string_lossy().into_owned(),
        php_dir: php_dir.to_string_lossy().into_owned(),
        class,
        concurrent: concurrent
            .into_iter()
            .take(8)
            .map(|path| path.to_string_lossy().into_owned())
            .collect(),
        php_files,
    })
}

fn discover_bench_edits(root: &Path, files: &BenchFiles) -> Result<BenchEdits, String> {
    let mut attribute = None;
    let mut methods = Vec::new();
    for file in &files.php_files {
        let source =
            fs::read_to_string(root.join(file)).map_err(|error| format!("read {file}: {error}"))?;
        let lines: Vec<_> = source.lines().collect();
        for (line, value) in lines.iter().enumerate() {
            if value.trim_start().starts_with("#[")
                && lines
                    .iter()
                    .skip(line + 1)
                    .take(12)
                    .any(|next| next.contains("function "))
            {
                attribute = Some((file.clone(), line));
                break;
            }
        }
        for start in lines
            .iter()
            .enumerate()
            .filter_map(|(line, value)| value.contains("function ").then_some(line))
        {
            if let Some(end) = method_end(&lines, start) {
                methods.push((file.clone(), start, end));
            }
        }
        if attribute.is_some() && !methods.is_empty() {
            break;
        }
    }
    let (attribute_file, attribute_line) = attribute
        .ok_or_else(|| format!("no PHP attribute above a method under {}", files.php_dir))?;
    let (removed_file, removed_start, removed_end) = methods
        .into_iter()
        .find(|(file, start, end)| {
            file != &attribute_file || *end < attribute_line || *start > attribute_line
        })
        .ok_or_else(|| format!("no removable PHP method under {}", files.php_dir))?;
    let remaining: Vec<_> = files
        .php_files
        .iter()
        .filter(|file| {
            **file != attribute_file && **file != removed_file && !files.concurrent.contains(*file)
        })
        .collect();
    if remaining.len() < 2 {
        return Err(format!(
            "{} has fewer than two PHP files for delete and rename",
            files.php_dir
        ));
    }
    let deleted_file = (*remaining[0]).clone();
    let renamed_from = (*remaining[1]).clone();
    let renamed_path = Path::new(&renamed_from);
    let stem = renamed_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| format!("{} has no UTF-8 file stem", renamed_path.display()))?;
    let renamed_to = renamed_path
        .with_file_name(format!("{stem}BenchRenamed.php"))
        .to_string_lossy()
        .into_owned();
    Ok(BenchEdits {
        attribute_file,
        attribute_line,
        removed_file,
        removed_start,
        removed_end,
        deleted_file,
        renamed_from,
        renamed_to,
    })
}

fn method_end(lines: &[&str], start: usize) -> Option<usize> {
    let mut depth = 0_usize;
    let mut opened = false;
    for (line, value) in lines.iter().enumerate().skip(start) {
        for character in value.chars() {
            if character == '{' {
                opened = true;
                depth += 1;
            } else if character == '}' && opened {
                depth -= 1;
                if depth == 0 {
                    return Some(line);
                }
            }
        }
    }
    None
}

fn apply_bench_edits(root: &Path, edits: &BenchEdits) -> Result<(), String> {
    for (file, attribute_line, removed) in [
        (
            &edits.attribute_file,
            Some(edits.attribute_line),
            (edits.removed_file == edits.attribute_file)
                .then_some((edits.removed_start, edits.removed_end)),
        ),
        (
            &edits.removed_file,
            None,
            (edits.removed_file != edits.attribute_file)
                .then_some((edits.removed_start, edits.removed_end)),
        ),
    ] {
        let path = root.join(file);
        let source = fs::read_to_string(&path)
            .map_err(|error| format!("read {}: {error}", path.display()))?;
        let ends_with_newline = source.ends_with('\n');
        let mut lines: Vec<_> = source.lines().map(str::to_owned).collect();
        if let Some(line) = attribute_line {
            lines[line].push(' ');
        }
        if let Some((start, end)) = removed {
            lines.drain(start..=end);
        }
        let mut edited = lines.join("\n");
        if ends_with_newline {
            edited.push('\n');
        }
        fs::write(&path, edited).map_err(|error| format!("write {}: {error}", path.display()))?;
    }
    fs::remove_file(root.join(&edits.deleted_file))
        .map_err(|error| format!("delete {}: {error}", edits.deleted_file))?;
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["mv", &edits.renamed_from, &edits.renamed_to])
        .output()
        .map_err(|error| format!("rename {}: {error}", edits.renamed_from))?;
    if !output.status.success() {
        return Err(format!(
            "rename {}: {}",
            edits.renamed_from,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}

fn tracked_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["ls-files", "-z"])
        .output()
        .map_err(|error| format!("list tracked files under {}: {error}", root.display()))?;
    if !output.status.success() {
        return Err(format!(
            "list tracked files under {}: {}",
            root.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let mut files: Vec<PathBuf> = output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| PathBuf::from(String::from_utf8_lossy(path).into_owned()))
        .collect();
    files.sort();
    Ok(files)
}

fn is_source_file(path: &Path) -> bool {
    ["php", "tsx", "ts", "jsx", "js", "py", "rs"]
        .iter()
        .any(|extension| is_extension(path, extension))
}

fn is_extension(path: &Path, extension: &str) -> bool {
    path.extension().and_then(|value| value.to_str()) == Some(extension)
}

fn count_lines(path: &Path) -> Result<usize, std::io::Error> {
    Ok(fs::read_to_string(path)?.lines().count())
}

fn php_class(source: &str) -> Option<String> {
    source.lines().find_map(|line| {
        let (_, rest) = line.split_once("class ")?;
        let name: String = rest
            .chars()
            .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
            .collect();
        (!name.is_empty()).then_some(name)
    })
}

fn bench_workloads(files: &BenchFiles) -> Result<Vec<BenchWorkload>, String> {
    let command = |name: String, args: Vec<String>, session| BenchWorkload {
        name,
        command: BenchCommand::Trace { args, session },
    };
    Ok(vec![
        command(
            "context php --no-record".into(),
            vec!["context".into(), files.php.clone(), "--no-record".into()],
            Session::None,
        ),
        command(
            "context tsx --no-record".into(),
            vec!["context".into(), files.tsx.clone(), "--no-record".into()],
            Session::None,
        ),
        command(
            "context php first touch".into(),
            vec!["context".into(), files.php.clone()],
            Session::FirstTouch,
        ),
        command(
            "context php repeat".into(),
            vec!["context".into(), files.php.clone()],
            Session::Repeat,
        ),
        command(
            "read php".into(),
            vec!["read".into(), files.php.clone()],
            Session::None,
        ),
        command(
            "grep class".into(),
            vec![
                "grep".into(),
                files.class.clone(),
                files.php_dir.clone(),
                "-t".into(),
                "php".into(),
            ],
            Session::None,
        ),
        command(
            "pattern php".into(),
            vec![
                "pattern".into(),
                "$A->$B()".into(),
                "-t".into(),
                "php".into(),
                files.php_dir.clone(),
            ],
            Session::None,
        ),
        command(
            "info php".into(),
            vec!["info".into(), files.php.clone()],
            Session::None,
        ),
        command(
            "callers class".into(),
            vec!["callers".into(), files.class.clone()],
            Session::None,
        ),
        command(
            "usages path".into(),
            vec!["usages".into(), "--path".into(), files.php_dir.clone()],
            Session::None,
        ),
        command(
            "usages class".into(),
            vec!["usages".into(), files.class.clone()],
            Session::None,
        ),
        command(
            "defines class".into(),
            vec!["defines".into(), files.class.clone()],
            Session::None,
        ),
        command(
            "structure php".into(),
            vec!["structure".into(), files.php.clone()],
            Session::None,
        ),
        command(
            "blame php --json".into(),
            vec!["blame".into(), files.php.clone(), "--json".into()],
            Session::None,
        ),
        command("status".into(), vec!["status".into()], Session::None),
        command("diff".into(), vec!["diff".into()], Session::None),
        command(
            "diff --symbols".into(),
            vec!["diff".into(), "--symbols".into()],
            Session::None,
        ),
        command("stats".into(), vec!["stats".into()], Session::None),
        command("context".into(), vec!["context".into()], Session::None),
        BenchWorkload {
            name: "edit context php --no-record".into(),
            command: BenchCommand::Edit {
                args: vec!["context".into(), files.php.clone(), "--no-record".into()],
            },
        },
        BenchWorkload {
            name: "concurrent context php".into(),
            command: BenchCommand::Concurrent {
                files: files.concurrent.clone(),
            },
        },
    ])
}

fn warm_worktree(
    binary: &Path,
    root: &Path,
    workloads: &[BenchWorkload],
    side: &str,
) -> Result<(), String> {
    retry_sample(|| {
        bench_sample(
            binary,
            root,
            &["cache".into(), "build".into(), ".".into()],
            &[],
            "cache build",
            side,
        )
    })?;
    for workload in workloads {
        for sample in 0..3 {
            run_workload(workload, binary, root, sample, side)?;
        }
    }
    Ok(())
}

fn measure_workload(
    workload: &BenchWorkload,
    samples: usize,
    baseline: &Path,
    baseline_root: &Path,
    candidate: &Path,
    candidate_root: &Path,
) -> Result<serde_json::Value, String> {
    let mut baseline_samples = Vec::with_capacity(samples);
    let mut candidate_samples = Vec::with_capacity(samples);
    for sample in 0..samples {
        baseline_samples.push(run_workload(
            workload,
            baseline,
            baseline_root,
            sample,
            "baseline",
        )?);
        candidate_samples.push(run_workload(
            workload,
            candidate,
            candidate_root,
            sample,
            "candidate",
        )?);
    }
    let baseline_elapsed: Vec<_> = baseline_samples
        .iter()
        .map(|sample| sample.elapsed_us)
        .collect();
    let candidate_elapsed: Vec<_> = candidate_samples
        .iter()
        .map(|sample| sample.elapsed_us)
        .collect();
    let baseline_retries: usize = baseline_samples.iter().map(|sample| sample.retries).sum();
    let candidate_retries: usize = candidate_samples.iter().map(|sample| sample.retries).sum();
    let baseline_median = median(&baseline_elapsed);
    let candidate_median = median(&candidate_elapsed);
    let ratio = candidate_median as f64 / baseline_median as f64;
    let interval = bootstrap_interval(&baseline_elapsed, &candidate_elapsed);
    Ok(serde_json::json!({
        "name": workload.name,
        "samples": samples,
        "baseline_median_us": baseline_median,
        "candidate_median_us": candidate_median,
        "ratio": ratio,
        "interval": interval,
        "baseline_samples_us": baseline_elapsed,
        "candidate_samples_us": candidate_elapsed,
        "baseline_retries": baseline_retries,
        "candidate_retries": candidate_retries,
        "baseline_child_median_us": baseline_samples.iter().filter_map(|sample| sample.child_median_us).collect::<Vec<_>>(),
        "candidate_child_median_us": candidate_samples.iter().filter_map(|sample| sample.child_median_us).collect::<Vec<_>>(),
    }))
}

fn run_workload(
    workload: &BenchWorkload,
    binary: &Path,
    root: &Path,
    sample: usize,
    side: &str,
) -> Result<BenchSample, String> {
    let (mut sample, retries) =
        retry_sample(|| run_workload_once(workload, binary, root, sample, side))?;
    sample.retries = retries;
    Ok(sample)
}

fn run_workload_once(
    workload: &BenchWorkload,
    binary: &Path,
    root: &Path,
    sample: usize,
    side: &str,
) -> Result<BenchSample, String> {
    match &workload.command {
        BenchCommand::Trace { args, session } => {
            let session = match session {
                Session::None => None,
                Session::FirstTouch => Some(format!("bench-first-{side}-{sample}")),
                Session::Repeat => Some("bench-repeat".to_string()),
            };
            let envs = session
                .as_ref()
                .map(|value| vec![("AGENT_SESSION_ID", value.as_str())])
                .unwrap_or_default();
            Ok(BenchSample {
                elapsed_us: bench_sample(binary, root, args, &envs, &workload.name, side)?,
                child_median_us: None,
                retries: 0,
            })
        }
        BenchCommand::Edit { args } => {
            let path = root.join(&args[1]);
            let original = fs::read_to_string(&path)
                .map_err(|error| format!("read {}: {error}", path.display()))?;
            let edited = if original.ends_with('\n') {
                format!("{original}\n")
            } else {
                format!("{original}\n\n")
            };
            fs::write(&path, edited)
                .map_err(|error| format!("write {}: {error}", path.display()))?;
            let result =
                bench_sample(binary, root, args, &[], &workload.name, side).map(|elapsed_us| {
                    BenchSample {
                        elapsed_us,
                        child_median_us: None,
                        retries: 0,
                    }
                });
            fs::write(&path, original)
                .map_err(|error| format!("restore {}: {error}", path.display()))?;
            result
        }
        BenchCommand::Concurrent { files } => {
            let (elapsed_us, child_median_us) = concurrent_context(binary, root, files, side)?;
            Ok(BenchSample {
                elapsed_us,
                child_median_us: Some(child_median_us),
                retries: 0,
            })
        }
    }
}

fn bench_sample(
    binary: &Path,
    root: &Path,
    args: &[String],
    envs: &[(&str, &str)],
    row: &str,
    side: &str,
) -> Result<u64, String> {
    let mut command = Command::new(binary);
    command.args(args).current_dir(root).env("HOME", root);
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
    command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let started = Instant::now();
    let mut child = command
        .spawn()
        .map_err(|error| format!("spawn {row} {side}: {error}"))?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| format!("capture {row} {side} stdout"))?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or_else(|| format!("capture {row} {side} stderr"))?;
    let stdout_reader = std::thread::spawn(move || {
        let mut output = Vec::new();
        stdout.read_to_end(&mut output).map(|_| output)
    });
    let stderr_reader = std::thread::spawn(move || {
        let mut output = Vec::new();
        stderr.read_to_end(&mut output).map(|_| output)
    });
    let status = wait_for_bench_child(&mut child, started, row, side);
    let stdout = stdout_reader
        .join()
        .map_err(|_| format!("read {row} {side} stdout: reader panicked"))?
        .map_err(|error| format!("read {row} {side} stdout: {error}"))?;
    let stderr = stderr_reader
        .join()
        .map_err(|_| format!("read {row} {side} stderr: reader panicked"))?
        .map_err(|error| format!("read {row} {side} stderr: {error}"))?;
    let status = status?;
    if !status.success() || stdout.is_empty() {
        return Err(format!(
            "{row} {side} failed: {}",
            String::from_utf8_lossy(&stderr).trim()
        ));
    }
    Ok(started.elapsed().as_micros() as u64)
}

fn retry_sample<T>(mut sample: impl FnMut() -> Result<T, String>) -> Result<(T, usize), String> {
    for retries in 0..=3 {
        match sample() {
            Ok(value) => return Ok((value, retries)),
            Err(error)
                if error.ends_with("exceeded the 60-second child deadline") && retries < 3 =>
            {
                continue
            }
            Err(error) if error.ends_with("exceeded the 60-second child deadline") => {
                return Err(format!("{error} after {retries} retries"));
            }
            Err(error) => return Err(error),
        }
    }
    unreachable!("the retry loop returns on every attempt")
}

fn wait_for_bench_child(
    child: &mut Child,
    started: Instant,
    row: &str,
    side: &str,
) -> Result<ExitStatus, String> {
    loop {
        match child
            .try_wait()
            .map_err(|error| format!("wait for {row} {side}: {error}"))?
        {
            Some(status) => return Ok(status),
            None if started.elapsed() >= BENCH_CHILD_DEADLINE => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "{row} {side} exceeded the {}-second child deadline",
                    BENCH_CHILD_DEADLINE.as_secs()
                ));
            }
            None => std::thread::sleep(Duration::from_millis(10)),
        }
    }
}

fn concurrent_context(
    binary: &Path,
    root: &Path,
    files: &[String],
    side: &str,
) -> Result<(u64, u64), String> {
    let mut children = Vec::with_capacity(files.len());
    for file in files {
        let mut command = Command::new(binary);
        command
            .args(["context", file])
            .current_dir(root)
            .env("HOME", root)
            .env_remove("AGENT_SESSION_ID")
            .env_remove("CODEX_THREAD_ID")
            .env_remove("CLAUDE_CODE_SESSION_ID")
            .env_remove("TRACER_AGENT_ID")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let started = Instant::now();
        let mut child = command
            .spawn()
            .map_err(|error| format!("spawn concurrent context php {side} {file}: {error}"))?;
        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| format!("capture concurrent context stdout for {file}"))?;
        let mut stderr = child
            .stderr
            .take()
            .ok_or_else(|| format!("capture concurrent context stderr for {file}"))?;
        let stdout_reader = std::thread::spawn(move || {
            let mut output = Vec::new();
            stdout.read_to_end(&mut output).map(|_| output)
        });
        let stderr_reader = std::thread::spawn(move || {
            let mut output = Vec::new();
            stderr.read_to_end(&mut output).map(|_| output)
        });
        let file = file.clone();
        let side = side.to_string();
        children.push(std::thread::spawn(move || {
            let status = wait_for_bench_child(&mut child, started, "concurrent context php", &side);
            let elapsed_us = started.elapsed().as_micros() as u64;
            let stdout = stdout_reader
                .join()
                .map_err(|_| {
                    format!("read concurrent context php {side} stdout for {file}: reader panicked")
                })?
                .map_err(|error| {
                    format!("read concurrent context php {side} stdout for {file}: {error}")
                })?;
            let stderr = stderr_reader
                .join()
                .map_err(|_| {
                    format!("read concurrent context php {side} stderr for {file}: reader panicked")
                })?
                .map_err(|error| {
                    format!("read concurrent context php {side} stderr for {file}: {error}")
                })?;
            let status = status?;
            if !status.success() || stdout.is_empty() {
                return Err(format!(
                    "concurrent context php {side} {file} failed: {}",
                    String::from_utf8_lossy(&stderr).trim()
                ));
            }
            Ok(elapsed_us)
        }));
    }
    let mut elapsed = Vec::with_capacity(children.len());
    for child in children {
        elapsed.push(
            child
                .join()
                .map_err(|_| "wait for concurrent context: worker panicked".to_string())??,
        );
    }
    Ok((*elapsed.iter().max().unwrap(), median(&elapsed)))
}

fn sustained_context(
    binary: &Path,
    root: &Path,
    file: &str,
    side: &str,
    requests: u64,
) -> Result<serde_json::Value, String> {
    let start = Instant::now();
    let (sender, receiver) = std::sync::mpsc::channel();
    let mut completed = 0_u64;
    let mut backlog = 0_u64;
    let mut max_backlog = 0_u64;
    let mut samples = Vec::with_capacity(requests as usize);
    for request in 0..requests {
        let arrival = start + Duration::from_nanos(request * 1_000_000_000 / 30);
        if let Some(wait) = arrival.checked_duration_since(Instant::now()) {
            std::thread::sleep(wait);
        }
        while let Ok(result) = receiver.try_recv() {
            backlog -= 1;
            completed += 1;
            samples.push(result?);
        }
        let sender = sender.clone();
        let binary = binary.to_path_buf();
        let root = root.to_path_buf();
        let file = file.to_string();
        let side = side.to_string();
        std::thread::spawn(move || {
            let result = retry_sample(|| {
                let mut command = Command::new(&binary);
                command
                    .args(["context", &file])
                    .current_dir(&root)
                    .env("HOME", &root)
                    .env("AGENT_SESSION_ID", "bench-sustained")
                    .env(
                        "TRACER_AGENT_ID",
                        format!("bench-sustained-{}", request % 30),
                    )
                    .env_remove("CODEX_THREAD_ID")
                    .env_remove("CLAUDE_CODE_SESSION_ID")
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null());
                let spawned = Instant::now();
                let mut child = command
                    .spawn()
                    .map_err(|error| format!("spawn sustained context php {side}: {error}"))?;
                wait_for_bench_child(&mut child, spawned, "sustained context php", &side).and_then(
                    |status| {
                        status
                            .success()
                            .then_some(spawned.elapsed().as_micros() as u64)
                            .ok_or_else(|| {
                                format!("sustained context php {side} exited unsuccessfully")
                            })
                    },
                )
            })
            .map(|(elapsed_us, retries)| SustainedSample {
                elapsed_us,
                retries,
            });
            let _ = sender.send(result);
        });
        backlog += 1;
        max_backlog = max_backlog.max(backlog);
    }
    drop(sender);
    while completed < requests {
        let result = receiver
            .recv()
            .map_err(|error| format!("collect sustained context: {error}"))?;
        completed += 1;
        samples.push(result?);
    }
    Ok(serde_json::json!({
        "completed": completed,
        "max_backlog": max_backlog,
        "samples_us": samples.iter().map(|sample| sample.elapsed_us).collect::<Vec<_>>(),
        "retries": samples.iter().map(|sample| sample.retries).sum::<usize>(),
    }))
}

fn sustained_summary(rounds: &[serde_json::Value]) -> serde_json::Value {
    let mut completed = 0_u64;
    let mut max_backlog = 0_u64;
    let mut retries = 0_u64;
    let mut elapsed = Vec::with_capacity(300);
    for round in rounds {
        completed += round["completed"].as_u64().unwrap();
        max_backlog = max_backlog.max(round["max_backlog"].as_u64().unwrap());
        retries += round["retries"].as_u64().unwrap();
        elapsed.extend(
            round["samples_us"]
                .as_array()
                .unwrap()
                .iter()
                .map(|sample| sample.as_u64().unwrap()),
        );
    }
    serde_json::json!({
        "completed": completed,
        "median_us": median(&elapsed),
        "p95_us": p95(&elapsed),
        "max_backlog": max_backlog,
        "retries": retries,
        "samples_us": elapsed,
    })
}

fn bootstrap_interval(baseline: &[u64], candidate: &[u64]) -> [f64; 2] {
    let mut random = 0x9e37_79b9_7f4a_7c15_u64;
    let mut ratios = Vec::with_capacity(2_000);
    for _ in 0..2_000 {
        let mut baseline_resample = Vec::with_capacity(baseline.len());
        let mut candidate_resample = Vec::with_capacity(candidate.len());
        for _ in 0..baseline.len() {
            random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
            baseline_resample.push(baseline[(random as usize) % baseline.len()]);
            random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
            candidate_resample.push(candidate[(random as usize) % candidate.len()]);
        }
        ratios.push(median(&candidate_resample) as f64 / median(&baseline_resample) as f64);
    }
    ratios.sort_by(f64::total_cmp);
    [ratios[49], ratios[1_949]]
}

fn print_bench(rows: &[serde_json::Value], sustained: &serde_json::Value, json: bool) {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "workloads": rows,
                "sustained": sustained,
            }))
            .unwrap()
        );
        return;
    }
    println!("workload                                      baseline  candidate    ratio     95% interval  child median      retries");
    for row in rows {
        let baseline_child = row["baseline_child_median_us"]
            .as_array()
            .filter(|samples| !samples.is_empty())
            .map(|samples| {
                median(
                    &samples
                        .iter()
                        .map(|sample| sample.as_u64().unwrap())
                        .collect::<Vec<_>>(),
                )
            });
        let candidate_child = row["candidate_child_median_us"]
            .as_array()
            .filter(|samples| !samples.is_empty())
            .map(|samples| {
                median(
                    &samples
                        .iter()
                        .map(|sample| sample.as_u64().unwrap())
                        .collect::<Vec<_>>(),
                )
            });
        let child_median = match (baseline_child, candidate_child) {
            (Some(baseline), Some(candidate)) => {
                format!(
                    "{:>7.2}/{:>7.2}ms",
                    baseline as f64 / 1_000.0,
                    candidate as f64 / 1_000.0
                )
            }
            _ => "             -".to_string(),
        };
        let retries = format!(
            "{:>4}/{:>4}",
            row["baseline_retries"].as_u64().unwrap(),
            row["candidate_retries"].as_u64().unwrap(),
        );
        println!(
            "{:<45} {:>8.2}ms {:>8.2}ms {:>8.3}  [{:.3}, {:.3}]  {}  {}",
            row["name"].as_str().unwrap(),
            row["baseline_median_us"].as_u64().unwrap() as f64 / 1_000.0,
            row["candidate_median_us"].as_u64().unwrap() as f64 / 1_000.0,
            row["ratio"].as_f64().unwrap(),
            row["interval"][0].as_f64().unwrap(),
            row["interval"][1].as_f64().unwrap(),
            child_median,
            retries,
        );
    }
    for side in ["baseline", "candidate"] {
        println!(
            "{:<45} completed={:>3} median={:>7.2}ms p95={:>7.2}ms backlog={} retries={}",
            format!("{} {side}", sustained["name"].as_str().unwrap()),
            sustained[side]["completed"].as_u64().unwrap(),
            sustained[side]["median_us"].as_u64().unwrap() as f64 / 1_000.0,
            sustained[side]["p95_us"].as_u64().unwrap() as f64 / 1_000.0,
            sustained[side]["max_backlog"].as_u64().unwrap(),
            sustained[side]["retries"].as_u64().unwrap(),
        );
    }
    println!(
        "{:<45} [{:.3}, {:.3}]",
        format!("{} 95% interval", sustained["name"].as_str().unwrap()),
        sustained["interval"][0].as_f64().unwrap(),
        sustained["interval"][1].as_f64().unwrap(),
    );
}

/// Canonical tracer crate root: the workspace member's parent.
/// `CARGO_MANIFEST_DIR` is `<repo>/tools/tracer/xtask`; the tracer crate is one
/// level up. This is invariant regardless of the caller's working directory.
fn tracer_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .expect("xtask crate always has a parent (the tracer crate root)")
        .to_path_buf()
}

/// The plugin payload root, derived from the tracer root:
/// `<repo>/tools/tracer` -> `<repo>/packages/claude/bin/tracer-dist`.
fn dist_dir(tracer_root: &Path) -> PathBuf {
    let repo = tracer_root
        .parent() // tools/
        .and_then(|p| p.parent()) // <repo>/
        .expect("tracer root is always <repo>/tools/tracer");
    repo.join("packages/claude/bin/tracer-dist")
}

/// The build-from-source mirror inside the payload — also the source every
/// prebuilt is compiled from, so one tree defines both fallback paths.
fn mirror_dir(tracer_root: &Path) -> PathBuf {
    dist_dir(tracer_root).join("crate")
}

fn sync_dist(check: bool) -> ExitCode {
    let src_root = tracer_root();
    let mirror = mirror_dir(&src_root);

    let manifest = match build_standalone_manifest(&src_root) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("xtask sync-dist: {e}");
            return ExitCode::from(1);
        }
    };
    let lock = match build_standalone_lock(&src_root, &manifest) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("xtask sync-dist: {e}");
            return ExitCode::from(1);
        }
    };

    if check {
        if mirror_matches(&src_root, &mirror, &manifest, &lock) {
            ExitCode::SUCCESS
        } else {
            eprintln!("xtask sync-dist: DRIFT — packages/claude/bin/tracer-dist/crate is");
            eprintln!("out of sync with tools/tracer. crate/ is generated; do not hand-edit.");
            eprintln!("Run: cargo xtask sync-dist   (from tools/tracer)");
            ExitCode::from(1)
        }
    } else {
        match regenerate(&src_root, &mirror, &manifest, &lock) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("xtask sync-dist: {e}");
                ExitCode::from(1)
            }
        }
    }
}

fn build_bin(check: bool) -> ExitCode {
    let src_root = tracer_root();
    let mirror = mirror_dir(&src_root);
    let bin = dist_dir(&src_root).join("bin");

    let stamp = match crate_stamp(&mirror) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("xtask build-bin: {e}");
            return ExitCode::from(1);
        }
    };
    if check {
        let stale = stale_prebuilts(&bin, &stamp);
        if stale.is_empty() {
            return ExitCode::SUCCESS;
        }
        eprintln!("xtask build-bin: STALE — the shipped prebuilts do not match");
        eprintln!("packages/claude/bin/tracer-dist/crate:");
        for reason in &stale {
            eprintln!("  {reason}");
        }
        eprintln!("Run: cargo xtask build-bin   (from tools/tracer)");
        return ExitCode::from(1);
    }

    match produce(&mirror, &bin, &stamp) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("xtask build-bin: {e}");
            ExitCode::from(1)
        }
    }
}

/// One line per prebuilt that is missing, plus one for a stamp that disagrees
/// with the mirror. Empty means every shipped binary was built from the crate
/// sitting beside it.
fn stale_prebuilts(bin: &Path, stamp: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (dir, _) in PREBUILTS {
        if !bin.join(dir).join("trace").is_file() {
            out.push(format!("{dir}/trace is missing"));
        }
    }
    match fs::read_to_string(bin.join("source.sha256")) {
        Ok(recorded) if recorded.trim() == stamp => {}
        Ok(_) => out.push("source.sha256 records a different crate".to_string()),
        Err(_) => out.push("source.sha256 is missing".to_string()),
    }
    out
}

/// Build every prebuilt, then record the crate they came from. The stamp is
/// written last so an interrupted run leaves the previous stamp in place and
/// the next `--check` still reports stale.
fn produce(mirror: &Path, bin: &Path, stamp: &str) -> Result<(), String> {
    require_cross_toolchain()?;
    for (dir, target) in PREBUILTS {
        let target_dir = mirror.join("target").join(format!("dist-{dir}"));
        let produced = match target {
            None => {
                host_build(mirror, &target_dir)?;
                target_dir.join("release/trace")
            }
            Some(t) => {
                linux_build(mirror, &target_dir, t)?;
                // cargo lays the output under the bare triple; the `.2.17` the
                // build was asked for is a linker instruction, not a directory.
                target_dir.join(t).join("release/trace")
            }
        };
        let dest = bin.join(dir);
        fs::create_dir_all(&dest).map_err(|e| format!("create {}: {e}", dest.display()))?;
        fs::copy(&produced, dest.join("trace"))
            .map_err(|e| format!("copy {} -> {}: {e}", produced.display(), dest.display()))?;
        eprintln!("xtask build-bin: {dir}");
    }
    fs::write(bin.join("source.sha256"), format!("{stamp}\n"))
        .map_err(|e| format!("write source.sha256: {e}"))
}

/// zig supplies the Linux linker, libc, and C compiler the eleven tree-sitter
/// grammars need; cargo-zigbuild puts them behind a cargo subcommand. Checked
/// before the first build so a missing toolchain costs no compile time and says
/// what to install, the shape `trace doctor` uses for a missing binary.
fn require_cross_toolchain() -> Result<(), String> {
    let zig = Command::new("zig")
        .arg("version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok();
    // Probed through cargo, not PATH: `cargo install` puts cargo-zigbuild in
    // $CARGO_HOME/bin, which cargo searches for `cargo-*` subcommands but which
    // is not on an interactive PATH here.
    let zigbuild = rustup_cargo()
        .args(["zigbuild", "--help"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if zig && zigbuild {
        return Ok(());
    }
    let mut missing = String::new();
    if !zig {
        missing.push_str("  ✗ zig\n    install: brew install zig\n");
    }
    if !zigbuild {
        missing.push_str("  ✗ cargo-zigbuild\n    install: cargo install cargo-zigbuild\n");
    }
    Err(format!(
        "the Linux prebuilts cross-compile with zig.\n\nMissing:\n{missing}\nThen \
         re-run `cargo xtask build-bin`."
    ))
}

/// mac-arm64, compiled for the host. Guarded on the host triple because this
/// build produces a binary for whatever machine runs it, and a non-Apple-silicon
/// host would silently ship the wrong architecture under the mac-arm64 name.
fn host_build(mirror: &Path, target_dir: &Path) -> Result<(), String> {
    if !(std::env::consts::OS == "macos" && std::env::consts::ARCH == "aarch64") {
        return Err(format!(
            "mac-arm64 needs an Apple-silicon host; this is {}-{}",
            std::env::consts::OS,
            std::env::consts::ARCH
        ));
    }
    let status = rustup_cargo()
        .args(["build", "--release", "--locked", "--manifest-path"])
        .arg(mirror.join("Cargo.toml"))
        .arg("--target-dir")
        .arg(target_dir)
        .status()
        .map_err(|e| format!("run cargo build for mac-arm64: {e}"))?;
    if !status.success() {
        return Err("cargo build failed for mac-arm64".into());
    }
    Ok(())
}

/// One Linux prebuilt, cross-compiled on the host. The glibc version rides on
/// the target triple, so the floor is chosen here rather than inherited from
/// whatever libc the building machine happens to carry.
fn linux_build(mirror: &Path, target_dir: &Path, target: &str) -> Result<(), String> {
    let status = rustup_cargo()
        .args(["zigbuild", "--release", "--locked", "--manifest-path"])
        .arg(mirror.join("Cargo.toml"))
        .arg("--target")
        .arg(format!("{target}.{GLIBC}"))
        .arg("--target-dir")
        .arg(target_dir)
        .status()
        .map_err(|e| format!("run cargo zigbuild for {target}: {e}"))?;
    if !status.success() {
        return Err(format!("cargo zigbuild failed for {target}"));
    }
    Ok(())
}

/// cargo from the rustup toolchain, never whatever `cargo` PATH resolves to.
/// The Brewfile installs both `rust` and `rustup`, and Homebrew's `rust` wins on
/// PATH while shipping only the host target — a plain `cargo zigbuild` fails
/// with `can't find crate for core`. Going through `rustup run` picks the
/// toolchain that holds the Linux targets whatever the caller's PATH looks like,
/// and builds all three prebuilts with one compiler.
fn rustup_cargo() -> Command {
    let mut cmd = Command::new("rustup");
    cmd.args(["run", "stable", "cargo"]);
    cmd
}

/// sha256 over the standalone crate: for every file in sorted relative-path
/// order, the path bytes, a zero byte, the file bytes, a zero byte. `target/`
/// is excluded — it is build output, not input. `scripts/tracer.py` recomputes
/// this exact rule, so the pre-commit path checks the stamp with no cargo, no
/// Rust toolchain, and no network round trip.
fn crate_stamp(mirror: &Path) -> Result<String, String> {
    let mut rels = vec!["Cargo.lock".to_string(), "Cargo.toml".to_string()];
    for rel in list_files(&mirror.join("src"))? {
        rels.push(format!("src/{}", rel.display()));
    }
    rels.sort();

    let mut hasher = Sha256::new();
    for rel in &rels {
        let bytes = fs::read(mirror.join(rel)).map_err(|e| format!("read {rel}: {e}"))?;
        hasher.update(rel.as_bytes());
        hasher.update([0u8]);
        hasher.update(&bytes);
        hasher.update([0u8]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    static CARGO_ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn stamps_and_manifest_have_the_pinned_consumer_contract() {
        let root = std::env::temp_dir().join(format!("tracer-xtask-stamp-{}", std::process::id()));
        let source = root.join("source");
        let mirror = root.join("mirror");
        fs::create_dir_all(source.join("src")).unwrap();
        fs::create_dir_all(mirror.join("src")).unwrap();
        fs::write(
            source.join("Cargo.toml"),
            "[workspace]\nmembers = [\"xtask\"]\n\n[package]\nname = \"tracer\"\n",
        )
        .unwrap();
        fs::write(source.join("Cargo.lock"), "workspace lock\n").unwrap();
        fs::write(source.join("src/main.rs"), "fn main() {}\n").unwrap();
        fs::write(mirror.join("Cargo.toml"), "[package]\nname = \"tracer\"\n").unwrap();
        fs::write(mirror.join("Cargo.lock"), "workspace lock\n").unwrap();
        fs::write(mirror.join("src/main.rs"), "fn main() {}\n").unwrap();

        assert_eq!(
            build_standalone_manifest(&source).unwrap(),
            fs::read_to_string(mirror.join("Cargo.toml")).unwrap()
        );
        assert_eq!(
            crate_stamp(&mirror).unwrap(),
            "8c3e50358c46b79f00f11255434efec52a62b056153500da0a87a7ff5e581ccd"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn standalone_lock_keeps_canonical_bytes() {
        let _cargo_env = CARGO_ENV.lock().unwrap();
        let source = tracer_root();
        let manifest = build_standalone_manifest(&source).unwrap();
        let canonical = fs::read_to_string(source.join("Cargo.lock")).unwrap();
        let standalone = build_standalone_lock(&source, &manifest).unwrap();

        assert_eq!(standalone, canonical);
    }

    #[test]
    fn local_git_lock_keeps_the_pinned_commit() {
        let _cargo_env = CARGO_ENV.lock().unwrap();
        let root =
            std::env::temp_dir().join(format!("tracer-xtask-git-fixture-{}", std::process::id()));
        let cargo_home = root.join("cargo-home");
        fs::create_dir_all(&cargo_home).unwrap();
        let previous_cargo_home = std::env::var_os("CARGO_HOME");
        std::env::set_var("CARGO_HOME", &cargo_home);
        let dependency = root.join("dependency");
        fs::create_dir_all(dependency.join("src")).unwrap();
        fs::write(
            dependency.join("Cargo.toml"),
            "[package]\nname = \"probe-dependency\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        fs::write(
            dependency.join("src/lib.rs"),
            "pub fn version() -> u8 { 1 }\n",
        )
        .unwrap();
        git(&dependency, &["init"]);
        git(&dependency, &["add", "."]);
        git(
            &dependency,
            &[
                "-c",
                "user.name=test",
                "-c",
                "user.email=test@example.com",
                "commit",
                "-m",
                "old",
            ],
        );
        let old = git_output(&dependency, &["rev-parse", "HEAD"]);

        let source = root.join("workspace");
        fs::create_dir_all(source.join("src")).unwrap();
        fs::create_dir_all(source.join("xtask/src")).unwrap();
        fs::write(source.join("src/main.rs"), "fn main() {}\n").unwrap();
        fs::write(source.join("xtask/src/lib.rs"), "\n").unwrap();
        fs::write(
            source.join("xtask/Cargo.toml"),
            "[package]\nname = \"xtask\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        let dependency_url = format!("file://{}", dependency.display());
        let manifest = format!("[workspace]\nmembers = [\"xtask\"]\n\n[package]\nname = \"probe-consumer\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\nprobe-dependency = {{ git = \"{dependency_url}\" }}\n");
        fs::write(source.join("Cargo.toml"), &manifest).unwrap();
        cargo_generate(&source);

        fs::write(
            dependency.join("Cargo.toml"),
            "[package]\nname = \"probe-dependency\"\nversion = \"0.2.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        fs::write(
            dependency.join("src/lib.rs"),
            "pub fn version() -> u8 { 2 }\n",
        )
        .unwrap();
        git(&dependency, &["add", "."]);
        git(
            &dependency,
            &[
                "-c",
                "user.name=test",
                "-c",
                "user.email=test@example.com",
                "commit",
                "-m",
                "new",
            ],
        );
        let new = git_output(&dependency, &["rev-parse", "HEAD"]);

        let standalone_manifest = build_standalone_manifest(&source).unwrap();
        let lock = build_standalone_lock(&source, &standalone_manifest).unwrap();
        let consumer = root.join("consumer");
        fs::create_dir_all(&consumer).unwrap();
        fs::write(consumer.join("Cargo.toml"), &standalone_manifest).unwrap();
        fs::write(consumer.join("Cargo.lock"), lock).unwrap();
        copy_tree(&source.join("src"), &consumer.join("src")).unwrap();
        let pinned = cargo_tree(&consumer);
        assert_eq!(
            pinned,
            format!(
                "probe-consumer v0.1.0 ({})\n└── probe-dependency v0.1.0 ({dependency_url}#{})\n",
                consumer.display(),
                &old[..8]
            )
        );

        let inconsistent_manifest = standalone_manifest.replace(
            &format!("git = \"{dependency_url}\""),
            &format!("git = \"{dependency_url}\", version = \"0.2\""),
        );
        assert!(build_standalone_lock(&source, &inconsistent_manifest).is_err());

        let fresh = root.join("fresh");
        fs::create_dir_all(&fresh).unwrap();
        fs::write(fresh.join("Cargo.toml"), &standalone_manifest).unwrap();
        copy_tree(&source.join("src"), &fresh.join("src")).unwrap();
        cargo_generate(&fresh);
        assert_eq!(
            fs::read(fresh.join("Cargo.toml")).unwrap(),
            fs::read(consumer.join("Cargo.toml")).unwrap()
        );
        let fresh_tree = cargo_tree(&fresh);
        assert_eq!(
            fresh_tree,
            format!(
                "probe-consumer v0.1.0 ({})\n└── probe-dependency v0.2.0 ({dependency_url}#{})\n",
                fresh.display(),
                &new[..8]
            )
        );
        assert_eq!(
            fs::read_to_string(source.join("Cargo.toml")).unwrap(),
            manifest
        );
        match previous_cargo_home {
            Some(value) => std::env::set_var("CARGO_HOME", value),
            None => std::env::remove_var("CARGO_HOME"),
        }
        let _ = fs::remove_dir_all(root);
    }

    fn cargo_generate(root: &Path) {
        let status = Command::new(cargo())
            .args(["generate-lockfile", "--manifest-path"])
            .arg(root.join("Cargo.toml"))
            .status()
            .unwrap();
        assert!(status.success());
    }

    fn git(root: &Path, args: &[&str]) {
        assert!(Command::new("git")
            .args(args)
            .current_dir(root)
            .status()
            .unwrap()
            .success());
    }

    fn git_output(root: &Path, args: &[&str]) -> String {
        String::from_utf8(
            Command::new("git")
                .args(args)
                .current_dir(root)
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap()
        .trim()
        .to_string()
    }

    fn cargo_tree(root: &Path) -> String {
        let mut command = Command::new(cargo());
        command.args(["tree", "--offline", "--locked"]);
        let output = command
            .arg("--manifest-path")
            .arg(root.join("Cargo.toml"))
            .output()
            .unwrap();
        assert!(output.status.success());
        String::from_utf8(output.stdout).unwrap()
    }
}

/// The tracer manifest with its `[workspace]` table removed. The mirror is
/// consumed in isolation by plugin users; a `[workspace]` table referencing a
/// missing `xtask` member breaks their `cargo build`. Everything else — the
/// `[package]`, `[[bin]]`, `[dependencies]`, `[profile.release]` — is verbatim.
fn build_standalone_manifest(src_root: &Path) -> Result<String, String> {
    let raw = fs::read_to_string(src_root.join("Cargo.toml"))
        .map_err(|e| format!("read tools/tracer/Cargo.toml: {e}"))?;

    let mut out = String::with_capacity(raw.len());
    let mut in_workspace = false;
    for line in raw.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('[') {
            // Entering a new top-level table or array-of-tables.
            in_workspace = trimmed == "[workspace]";
            if in_workspace {
                continue;
            }
        }
        if in_workspace {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    // Collapse the blank line the stripped table leaves at the top so the
    // mirrored manifest starts cleanly at `[package]` and the output is stable.
    Ok(format!("{}\n", out.trim_start_matches('\n').trim_end()))
}

/// The canonical `Cargo.lock`, verified against the standalone manifest without
/// resolving newer registry versions. Cargo allows unrelated workspace package
/// rows in a lockfile; they are not part of the standalone consumer's graph.
fn build_standalone_lock(src_root: &Path, manifest: &str) -> Result<String, String> {
    static SCRATCH_SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let sequence = SCRATCH_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let scratch = std::env::temp_dir().join(format!(
        "tracer-xtask-lock-{}-{sequence}",
        std::process::id()
    ));
    fs::create_dir_all(scratch.join("src")).map_err(|e| format!("create scratch dir: {e}"))?;

    fs::write(scratch.join("Cargo.toml"), manifest)
        .map_err(|e| format!("write scratch manifest: {e}"))?;
    fs::copy(src_root.join("Cargo.lock"), scratch.join("Cargo.lock"))
        .map_err(|e| format!("copy tools/tracer/Cargo.lock: {e}"))?;
    copy_tree(&src_root.join("src"), &scratch.join("src"))?;

    let status = Command::new(cargo())
        .args(["tree", "--locked", "--offline", "--manifest-path"])
        .arg(scratch.join("Cargo.toml"))
        .stdout(std::process::Stdio::null())
        .status()
        .map_err(|e| format!("run cargo tree: {e}"))?;
    if !status.success() {
        let _ = fs::remove_dir_all(&scratch);
        return Err("cargo rejected the standalone Cargo.lock".into());
    }

    let lock = fs::read_to_string(scratch.join("Cargo.lock"))
        .map_err(|e| format!("read generated Cargo.lock: {e}"))?;
    let _ = fs::remove_dir_all(&scratch);
    Ok(lock)
}

/// Regenerate the mirror: src tree, standalone manifest, standalone lock.
/// Idempotent — a second run leaves the mirror byte-identical.
fn regenerate(src_root: &Path, mirror: &Path, manifest: &str, lock: &str) -> Result<(), String> {
    fs::create_dir_all(mirror).map_err(|e| format!("create mirror dir: {e}"))?;

    let mirror_src = mirror.join("src");
    let _ = fs::remove_dir_all(&mirror_src);
    fs::create_dir_all(&mirror_src).map_err(|e| format!("create mirror src: {e}"))?;
    copy_tree(&src_root.join("src"), &mirror_src)?;

    fs::write(mirror.join("Cargo.toml"), manifest)
        .map_err(|e| format!("write mirror Cargo.toml: {e}"))?;
    fs::write(mirror.join("Cargo.lock"), lock)
        .map_err(|e| format!("write mirror Cargo.lock: {e}"))?;
    Ok(())
}

/// True when the on-disk mirror already equals what `regenerate` would write —
/// the drift guard's core comparison (src tree + manifest + lock).
fn mirror_matches(src_root: &Path, mirror: &Path, manifest: &str, lock: &str) -> bool {
    fs::read_to_string(mirror.join("Cargo.toml"))
        .ok()
        .as_deref()
        == Some(manifest)
        && fs::read_to_string(mirror.join("Cargo.lock"))
            .ok()
            .as_deref()
            == Some(lock)
        && trees_equal(&src_root.join("src"), &mirror.join("src"))
}

/// Recursive byte-exact directory copy (mirrors the shell `cp -R src`).
fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|e| format!("create {}: {e}", to.display()))?;
    for entry in fs::read_dir(from).map_err(|e| format!("read {}: {e}", from.display()))? {
        let entry = entry.map_err(|e| format!("dir entry under {}: {e}", from.display()))?;
        let src = entry.path();
        let dst = to.join(entry.file_name());
        let ty = entry
            .file_type()
            .map_err(|e| format!("file type of {}: {e}", src.display()))?;
        if ty.is_dir() {
            copy_tree(&src, &dst)?;
        } else {
            fs::copy(&src, &dst)
                .map_err(|e| format!("copy {} -> {}: {e}", src.display(), dst.display()))?;
        }
    }
    Ok(())
}

/// True when two directory trees are byte-identical (same set of files, same
/// contents). Drives the `--check` drift comparison for the src tree.
fn trees_equal(a: &Path, b: &Path) -> bool {
    let mut a_entries = match list_files(a) {
        Ok(v) => v,
        Err(_) => return false,
    };
    let mut b_entries = match list_files(b) {
        Ok(v) => v,
        Err(_) => return false,
    };
    a_entries.sort();
    b_entries.sort();
    if a_entries != b_entries {
        return false;
    }
    for rel in &a_entries {
        match (fs::read(a.join(rel)), fs::read(b.join(rel))) {
            (Ok(x), Ok(y)) if x == y => {}
            _ => return false,
        }
    }
    true
}

/// Relative paths of every file under `root`, recursively.
fn list_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    fn walk(base: &Path, dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
        for entry in fs::read_dir(dir).map_err(|e| format!("read {}: {e}", dir.display()))? {
            let entry = entry.map_err(|e| format!("dir entry: {e}"))?;
            let path = entry.path();
            let ty = entry.file_type().map_err(|e| format!("file type: {e}"))?;
            if ty.is_dir() {
                walk(base, &path, out)?;
            } else {
                out.push(
                    path.strip_prefix(base)
                        .map_err(|e| format!("strip prefix: {e}"))?
                        .to_path_buf(),
                );
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(root, root, &mut out)?;
    Ok(out)
}

/// The cargo to invoke for sub-builds — honor `CARGO` (set by the parent
/// `cargo xtask` run) so the same toolchain is used end to end.
fn cargo() -> String {
    std::env::var("CARGO").unwrap_or_else(|_| "cargo".into())
}
