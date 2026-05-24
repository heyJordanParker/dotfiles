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
//!
//! The crux: `tools/tracer/` is a cargo workspace with this xtask as a member
//! when developed in-repo, but the mirror must build standalone with NO
//! workspace and NO xtask present (a leaked `[workspace]` table referencing a
//! missing `xtask` member fails `cargo build` for plugin users). So the
//! mirrored manifest is the tracer package manifest with its `[workspace]`
//! table stripped, and the mirrored lock is resolved from that stripped
//! manifest — not the workspace lock, which carries xtask and its deps.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let task = args.next();
    let check = args.next().as_deref() == Some("--check");

    match task.as_deref() {
        Some("sync-dist") => sync_dist(check),
        Some(other) => {
            eprintln!("xtask: unknown task `{other}` (known: sync-dist)");
            ExitCode::from(2)
        }
        None => {
            eprintln!("xtask: missing task (known: sync-dist [--check])");
            ExitCode::from(2)
        }
    }
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

/// The plugin payload mirror, derived from the tracer root:
/// `<repo>/tools/tracer` -> `<repo>/packages/claude/bin/tracer-dist/crate`.
fn mirror_dir(tracer_root: &Path) -> PathBuf {
    let repo = tracer_root
        .parent() // tools/
        .and_then(|p| p.parent()) // <repo>/
        .expect("tracer root is always <repo>/tools/tracer");
    repo.join("packages/claude/bin/tracer-dist/crate")
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
            eprintln!(
                "xtask sync-dist: DRIFT — packages/claude/bin/tracer-dist/crate is"
            );
            eprintln!(
                "out of sync with tools/tracer. crate/ is generated; do not hand-edit."
            );
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

/// A `Cargo.lock` that resolves exactly the standalone (workspace-stripped)
/// manifest's dependency graph — the lock plugin users build against. Produced
/// by resolving the stripped manifest in an isolated temp tree so the
/// workspace lock (which carries `xtask` and its deps) never leaks in.
fn build_standalone_lock(src_root: &Path, manifest: &str) -> Result<String, String> {
    let scratch = std::env::temp_dir().join(format!(
        "tracer-xtask-lock-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    fs::create_dir_all(scratch.join("src"))
        .map_err(|e| format!("create scratch dir: {e}"))?;

    fs::write(scratch.join("Cargo.toml"), manifest)
        .map_err(|e| format!("write scratch manifest: {e}"))?;
    copy_tree(&src_root.join("src"), &scratch.join("src"))?;

    let status = Command::new(cargo())
        .args(["generate-lockfile", "--manifest-path"])
        .arg(scratch.join("Cargo.toml"))
        .status()
        .map_err(|e| format!("run cargo generate-lockfile: {e}"))?;
    if !status.success() {
        return Err("cargo generate-lockfile failed for the standalone manifest".into());
    }

    let lock = fs::read_to_string(scratch.join("Cargo.lock"))
        .map_err(|e| format!("read generated Cargo.lock: {e}"))?;
    let _ = fs::remove_dir_all(&scratch);
    Ok(lock)
}

/// Regenerate the mirror: src tree, standalone manifest, standalone lock.
/// Idempotent — a second run leaves the mirror byte-identical.
fn regenerate(
    src_root: &Path,
    mirror: &Path,
    manifest: &str,
    lock: &str,
) -> Result<(), String> {
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
fn mirror_matches(
    src_root: &Path,
    mirror: &Path,
    manifest: &str,
    lock: &str,
) -> bool {
    fs::read_to_string(mirror.join("Cargo.toml")).ok().as_deref() == Some(manifest)
        && fs::read_to_string(mirror.join("Cargo.lock")).ok().as_deref() == Some(lock)
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
            let ty = entry
                .file_type()
                .map_err(|e| format!("file type: {e}"))?;
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
