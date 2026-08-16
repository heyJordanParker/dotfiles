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
//! table stripped, and the mirrored lock is resolved from that stripped
//! manifest — not the workspace lock, which carries xtask and its deps.

use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

/// The glibc version every Linux prebuilt links against, appended to the target
/// triple so zig links the stubs for exactly that release. 2.17 is RHEL 7 era,
/// below any distribution still in service, and pinning it here is what makes
/// the floor a decision rather than a side effect of whichever machine built the
/// binary. The produced binary needs no symbol above it.
const GLIBC: &str = "2.17";

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
    let check = args.next().as_deref() == Some("--check");

    match task.as_deref() {
        Some("sync-dist") => sync_dist(check),
        Some("build-bin") => build_bin(check),
        Some(other) => {
            eprintln!("xtask: unknown task `{other}` (known: sync-dist, build-bin)");
            ExitCode::from(2)
        }
        None => {
            eprintln!("xtask: missing task (known: sync-dist [--check], build-bin [--check])");
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

/// sha256 over the mirror crate: for every file in sorted relative-path order,
/// the path bytes, a zero byte, the file bytes, a zero byte. `target/` is
/// excluded — it is build output, not input. `scripts/tracer.py` recomputes
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
