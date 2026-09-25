//! `trace docs prime [FILE...]` — record docs the harness put in the agent's
//! context into the session log, so later tracer emissions (Read enrichment,
//! `trace docs`, …) skip what the agent already has.
//!
//! With files, those files are recorded as they are: Claude Code's
//! `InstructionsLoaded` hook names every doc Claude Code loads, the moment it
//! loads it. With none, the chain the harness loads at session start is
//! computed (below) and recorded under `--reason` — codex, which reports no
//! per-file load, primes this way.
//!
//! The primer's auto-load set is computed deterministically from documented
//! harness rules — never parsed from transcripts or coupled to harness
//! internals:
//!
//!   1. User-global CLAUDE.md — `$CLAUDE_CONFIG_DIR/CLAUDE.md`, else
//!      `$HOME/.claude/CLAUDE.md`.
//!   2. Project-root CLAUDE.md chain — every CLAUDE.md / Claude.md ancestor
//!      from repo_root down to cwd, both casings considered.
//!
//! The Claude memory system (`$HOME/.claude/projects/<slug>/memory/MEMORY.md`)
//! is harness-internal state managed by Claude Code itself and intentionally
//! out of tracer's scope — the primer models repo docs only.
//!
//! Each top-level doc's recursive `@include` graph is walked by
//! `nested_memory::load_includes` (the locked reuse target — no duplicate
//! include logic lives here). The boundary passed to `load_includes` is the
//! containment root each doc family belongs to: the repo root for the in-repo
//! chain, and `$HOME/.claude/` for the user-global doc, so @-imports stay
//! anchored to their own tree.
//!
//! Emission runs through `session_log::record_emission` with
//! `source = "context_prime_session_start" | "context_prime_post_compact"`
//! — the recorded source names outlive the command's own name, because a
//! session log written by one binary is read by the next, and renaming the
//! stored vocabulary would make every in-flight session re-inject its docs —
//! sharing the flock'd append + materialize the log already owns. No-op when
//! the session id is absent — standalone tracer use stays valid.

use super::{nested_memory, session_log};
use crate::cache;
use anyhow::Result;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy)]
pub enum Reason {
    SessionStart,
    PostCompact,
}

impl Reason {
    fn source(&self) -> &'static str {
        match self {
            Reason::SessionStart => "context_prime_session_start",
            Reason::PostCompact => "context_prime_post_compact",
        }
    }
    fn label(&self) -> &'static str {
        match self {
            Reason::SessionStart => "session_start",
            Reason::PostCompact => "post_compact",
        }
    }
}

pub fn parse_reason(s: &str) -> Result<Reason> {
    match s {
        "session_start" => Ok(Reason::SessionStart),
        "post_compact" => Ok(Reason::PostCompact),
        other => Err(anyhow::anyhow!(
            "unknown --reason `{other}` (expected session_start | post_compact)"
        )),
    }
}

/// The source a doc the harness reported loading is recorded under — Claude
/// Code's `InstructionsLoaded` event names each file it loads, includes too,
/// so the named file is recorded alone, with no include walk.
const HARNESS_LOADED: &str = "instructions_loaded";

pub fn run(reason: Reason, files: &[PathBuf], as_json: bool) -> Result<Value> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let repo_root = cache::worktree_root_for(&cwd).unwrap_or_else(|| cache::display_root(&cwd));

    let mut pass: BTreeSet<String> = BTreeSet::new();
    let mut session: BTreeSet<String> = session_log::loaded_paths();
    let mut memories: Vec<nested_memory::LoadedMemory> = Vec::new();

    if !files.is_empty() {
        for file in files {
            if let Some(memory) =
                nested_memory::try_load(file, &repo_root, "harness", &mut pass, &mut session)
            {
                memories.push(memory);
            }
        }
        session_log::record_emission(&memories, HARNESS_LOADED);
        return Ok(report(reason, HARNESS_LOADED, &cwd, &repo_root, &memories, as_json));
    }

    // 1. User-global rules file. Claude Code's CLAUDE.md is recognized;
    //    so is OpenAI's AGENTS.md when present at the same well-known
    //    location (`$CLAUDE_CONFIG_DIR` / `$HOME/.claude/`). Each carries
    //    its own kind tag so downstream consumers can attribute by harness.
    if let Some((global, kind)) = user_global_rules_doc() {
        let boundary = global
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| global.clone());
        load_one(
            &global,
            &boundary,
            kind,
            &mut pass,
            &mut session,
            &mut memories,
        );
    }

    // 2. Project-root rules chain — repo_root down to cwd. Both harness
    //    conventions are probed at every ancestor: CLAUDE.md/Claude.md
    //    (kind `claude_md`) and AGENTS.md/Agents.md (kind `agents_md`),
    //    plus the matching `.local.md` peers. Casing duplicates collapse
    //    in `try_load` via canonical-path dedupe on case-insensitive FSes.
    for dir in claude_md_chain(&cwd, &repo_root) {
        for (candidate, kind) in [
            (dir.join("CLAUDE.md"), "claude_md"),
            (dir.join("Claude.md"), "claude_md"),
            (dir.join("AGENTS.md"), "agents_md"),
            (dir.join("Agents.md"), "agents_md"),
        ] {
            load_one(
                &candidate,
                &repo_root,
                kind,
                &mut pass,
                &mut session,
                &mut memories,
            );
        }
    }

    session_log::record_emission(&memories, reason.source());
    Ok(report(reason, reason.source(), &cwd, &repo_root, &memories, as_json))
}

fn report(
    reason: Reason,
    source: &str,
    cwd: &Path,
    repo_root: &Path,
    memories: &[nested_memory::LoadedMemory],
    as_json: bool,
) -> Value {
    let context = json!({
        "cwd": cwd.to_string_lossy(),
        "repo_root": repo_root.to_string_lossy(),
    });
    let out = crate::output::document(
        json!({"reason": reason.label(), "source": source}),
        context,
        json!(memories
            .iter()
            .map(|m| json!({
                "path": m.relative_path,
                "kind": m.kind,
                "size": m.size,
                "large": m.large,
            }))
            .collect::<Vec<_>>()),
        json!({"mirrored": memories.len()}),
    );

    if as_json {
        return out;
    }

    println!(
        "docs prime · {} · mirrored {} doc(s) into session log",
        reason.label(),
        memories.len()
    );
    for m in memories {
        let marker = if m.large { " [LARGE]" } else { "" };
        println!(
            "  {} · {}{} ({} chars)",
            m.relative_path, m.kind, marker, m.size
        );
    }
    out
}

/// Try to load one top-level doc plus its `@include` graph, appending to
/// `memories`. Silently skips missing / empty / outside-boundary / already-
/// surfaced files — every gate already lives in `try_load` and `load_includes`.
fn load_one(
    path: &Path,
    boundary: &Path,
    kind: &str,
    pass: &mut BTreeSet<String>,
    session: &mut BTreeSet<String>,
    memories: &mut Vec<nested_memory::LoadedMemory>,
) {
    let Some(mem) = nested_memory::try_load(path, boundary, kind, pass, session) else {
        return;
    };
    let includes = nested_memory::load_includes(
        Path::new(&mem.path),
        &mem.content,
        boundary,
        pass,
        session,
        0,
    );
    memories.push(mem);
    memories.extend(includes);
}

/// Resolve the user-global rules doc Claude Code's harness loads at session
/// start. Honors `$CLAUDE_CONFIG_DIR`, defaults to `$HOME/.claude/`. Probes
/// both harness conventions in the same directory: Claude Code's
/// `CLAUDE.md`/`Claude.md` (kind `claude_md`) and OpenAI's `AGENTS.md`/
/// `Agents.md` (kind `agents_md`). Returns None when no candidate exists.
/// First match wins; precedence is Claude Code first, AGENTS.md second
/// — matching the longstanding Claude-default behavior with AGENTS.md as
/// a fallback peer.
fn user_global_rules_doc() -> Option<(PathBuf, &'static str)> {
    let dir = match std::env::var_os("CLAUDE_CONFIG_DIR") {
        Some(d) => PathBuf::from(d),
        None => nested_memory::home_dir()?.join(".claude"),
    };
    for (name, kind) in [
        ("CLAUDE.md", "claude_md"),
        ("Claude.md", "claude_md"),
        ("AGENTS.md", "agents_md"),
        ("Agents.md", "agents_md"),
    ] {
        let p = dir.join(name);
        if p.is_file() {
            return Some((p, kind));
        }
    }
    None
}

/// Directories that contribute a project-root CLAUDE.md to the primer chain:
/// from repo_root down to cwd inclusive. Returns an empty vec when cwd is
/// outside repo_root (no repo) — outside any worktree, the read-path
/// `unwrap_or_else(display_root)` falls back to cwd, so the chain is just
/// `[cwd]`.
fn claude_md_chain(cwd: &Path, repo_root: &Path) -> Vec<PathBuf> {
    let cwd = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
    let repo_root = repo_root
        .canonicalize()
        .unwrap_or_else(|_| repo_root.to_path_buf());
    if cwd.strip_prefix(&repo_root).is_err() {
        return vec![cwd];
    }
    let mut chain: Vec<PathBuf> = Vec::new();
    let mut current = cwd.clone();
    loop {
        chain.push(current.clone());
        if current == repo_root {
            break;
        }
        match current.parent() {
            Some(p) => current = p.to_path_buf(),
            None => break,
        }
        if current.strip_prefix(&repo_root).is_err() {
            break;
        }
    }
    chain.reverse();
    chain
}
