//! `trace context prime --reason …` — the context-primer command.
//!
//! Black-box: spawn `trace` with an isolated `HOME` so the user-global
//! CLAUDE.md lands inside the fixture; the session log
//! directory lives at the fixture root, which IS the repo root
//! (`Fixture::new` runs `git init`). Assertions go through the documented
//! on-disk shape (`events.jsonl` + `view.json`) and the command's `--json`
//! output.
//!
//! The primer models repo docs only: user-global CLAUDE.md + project-root
//! CLAUDE.md chain (and their `@include` graphs). The Claude memory system
//! (`$HOME/.claude/projects/<slug>/memory/MEMORY.md`) is harness-internal
//! state managed by Claude Code itself and is out of tracer's scope.
//!
//! Cases pinned (per the context-primer feature's definition of done):
//!   * empty case               — no docs at all, no events appended, success
//!   * single CLAUDE.md         — project-root CLAUDE.md only, one event
//!   * nested @-imports         — recursive include graph fully captured
//!   * recursion depth cap      — runaway @include cycle is bounded
//!   * memory file ignored      — a populated MEMORY.md never enters the primer
//!
//! All five run against the same hermetic fixture pattern; each owns a
//! unique session id so the log directory is isolated even
//! when tests share the same `HOME`.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use tracer_cli_tests::Fixture;

#[allow(non_upper_case_globals)] // project naming rule bans ALL_CAPS for our own identifiers
static context_prime_seq: AtomicU64 = AtomicU64::new(0);

fn fresh_session_id(tag: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let seq = context_prime_seq.fetch_add(1, Ordering::SeqCst);
    format!("trace-context-prime-test-{tag}-{nanos}-{seq}")
}

fn log_dir(home: &PathBuf, session_id: &str) -> PathBuf {
    home.join(".tracer-cache")
        .join("sessions")
        .join(session_id)
        .join("root")
}

fn read_events(home: &PathBuf, session_id: &str) -> Vec<serde_json::Value> {
    let path = log_dir(home, session_id).join("events.jsonl");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return vec![];
    };
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("events.jsonl line is valid JSON"))
        .collect()
}

/// Build a fresh isolated fixture + the env every test needs:
/// `HOME` rerouted to the fixture, fresh session id, deterministic log.
/// Returns `(fixture, home, session_id)`.
fn isolated(tag: &str) -> (Fixture, PathBuf, String) {
    let f = Fixture::new();
    let home = f.root.clone();
    // Pre-create the user-global .claude dir; tests opt into writing
    // CLAUDE.md / MEMORY.md per case.
    std::fs::create_dir_all(home.join(".claude")).unwrap();
    let sid = fresh_session_id(tag);
    (f, home, sid)
}

fn env_pairs<'a>(home: &'a str, sid: &'a str) -> Vec<(&'a str, &'a str)> {
    vec![("HOME", home), ("CLAUDE_CODE_SESSION_ID", sid)]
}

// --- empty case ------------------------------------------------------------

#[test]
fn empty_repo_records_no_events_and_succeeds() {
    let (f, home, sid) = isolated("empty");
    // No files in the fixture beyond the fixture-init scaffold — nothing
    // for the context primer to find (no CLAUDE.md, no MEMORY.md, no
    // user-global CLAUDE.md since the fixture's HOME starts empty).

    let home_str = home.to_string_lossy().into_owned();
    let run = f.trace_env(
        &["context", "prime", "--reason", "session_start", "--json"],
        &env_pairs(&home_str, &sid),
    );
    run.ok();

    let v = run.json();
    assert_eq!(v["reason"], "session_start");
    assert_eq!(v["source"], "context_prime_session_start");
    assert_eq!(
        v["mirrored_count"], 0,
        "no docs exist, mirrored_count must be 0: {v}"
    );
    let events = read_events(&home, &sid);
    assert!(
        events.is_empty(),
        "no docs surfaced means no events appended, got {events:?}"
    );
}

// --- single CLAUDE.md ------------------------------------------------------

#[test]
fn single_project_claude_md_records_one_event() {
    let (f, home, sid) = isolated("single");
    f.write("CLAUDE.md", "# Project rules\n\nSingle root file.\n");
    f.commit("add root claude.md");

    let home_str = home.to_string_lossy().into_owned();
    let run = f.trace_env(
        &["context", "prime", "--reason", "session_start", "--json"],
        &env_pairs(&home_str, &sid),
    );
    run.ok();

    let v = run.json();
    assert_eq!(
        v["mirrored_count"], 1,
        "one CLAUDE.md should yield one mirrored doc: {v}"
    );
    let mirrored = v["mirrored"].as_array().unwrap();
    assert_eq!(mirrored[0]["kind"], "claude_md");
    assert_eq!(mirrored[0]["path"], "CLAUDE.md");

    let events = read_events(&home, &sid);
    assert_eq!(events.len(), 1, "one event per mirrored doc, got {events:?}");
    let ev = &events[0];
    assert_eq!(ev["kind"], "doc_injection");
    assert_eq!(ev["source"], "context_prime_session_start");
    assert!(
        ev["content_hash"].as_str().unwrap().starts_with("sha256:"),
        "content_hash must be sha256-prefixed: {ev}"
    );
}

// --- nested @-imports ------------------------------------------------------

#[test]
fn nested_at_imports_are_walked_recursively() {
    let (f, home, sid) = isolated("nested");
    // @-imports use the `@include <path>` directive that the shared
    // `nested_memory::load_includes` walker matches (the locked reuse
    // target for include walking).
    f.write("CLAUDE.md", "# Root\n\n@include docs/one.md\n");
    f.write("docs/one.md", "# One\n\n@include two.md\n");
    f.write("docs/two.md", "# Two\n\nLeaf rules.\n");
    f.commit("add nested includes");

    let home_str = home.to_string_lossy().into_owned();
    let run = f.trace_env(
        &["context", "prime", "--reason", "session_start", "--json"],
        &env_pairs(&home_str, &sid),
    );
    run.ok();

    let v = run.json();
    // Root CLAUDE.md + one.md + two.md = 3 docs.
    assert_eq!(
        v["mirrored_count"], 3,
        "include graph must be walked transitively, got {v}"
    );
    let surfaced: BTreeSet<String> = v["mirrored"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["path"].as_str().unwrap().to_string())
        .collect();
    assert!(
        surfaced.contains("CLAUDE.md")
            && surfaced.contains("docs/one.md")
            && surfaced.contains("docs/two.md"),
        "expected root + both included files, got {surfaced:?}"
    );

    let events = read_events(&home, &sid);
    assert_eq!(events.len(), 3, "one event per surfaced doc: {events:?}");
    for ev in &events {
        assert_eq!(ev["source"], "context_prime_session_start");
    }
}

// --- recursion depth cap ---------------------------------------------------

#[test]
fn cyclic_includes_terminate_at_depth_cap() {
    let (f, home, sid) = isolated("cycle");
    // a → b → c → a: a true cycle. The cap (MAX_INCLUDE_DEPTH = 5) plus
    // path dedupe guarantees termination — without either, the walk loops
    // forever. The contract: it returns, the unique file set is exactly
    // {root, a, b, c}, and no event is recorded twice for the same path.
    f.write("CLAUDE.md", "# Root\n\n@include a.md\n");
    f.write("a.md", "# A\n\n@include b.md\n");
    f.write("b.md", "# B\n\n@include c.md\n");
    f.write("c.md", "# C\n\n@include a.md\n");
    f.commit("add cyclic includes");

    let home_str = home.to_string_lossy().into_owned();
    let run = f.trace_env(
        &["context", "prime", "--reason", "session_start", "--json"],
        &env_pairs(&home_str, &sid),
    );
    // The real assertion: the command terminates. Anything past here is
    // bonus correctness on the deduped set.
    run.ok();

    let v = run.json();
    assert_eq!(
        v["mirrored_count"], 4,
        "cycle must dedupe to the 4 distinct files, got {v}"
    );
    let surfaced: BTreeSet<String> = v["mirrored"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["path"].as_str().unwrap().to_string())
        .collect();
    for expected in ["CLAUDE.md", "a.md", "b.md", "c.md"] {
        assert!(
            surfaced.contains(expected),
            "missing {expected} from cycle walk: {surfaced:?}"
        );
    }

    // Each path appears exactly once in the log — no duplicate events
    // from re-traversal.
    let events = read_events(&home, &sid);
    let event_paths: Vec<String> = events
        .iter()
        .map(|e| e["visible_as"].as_str().unwrap().to_string())
        .collect();
    let unique: BTreeSet<&String> = event_paths.iter().collect();
    assert_eq!(
        event_paths.len(),
        unique.len(),
        "cycle must not double-record any path: {event_paths:?}"
    );
}

// --- memory file ignored ---------------------------------------------------

#[test]
fn project_memory_file_is_not_mirrored() {
    let (f, home, sid) = isolated("memory-ignored");
    // The Claude memory system lives outside tracer's scope. Even when a
    // populated MEMORY.md exists at the canonical
    // `$HOME/.claude/projects/<slug>/memory/MEMORY.md` path, the context
    // primer must surface only the repo's CLAUDE.md and leave the memory
    // file untouched. This pins the scope cut: tracer models repo docs,
    // never harness-internal state.
    f.write("CLAUDE.md", "# Project\n\nRoot rules only.\n");
    f.commit("add root claude.md only");

    // Build the canonical memory path Claude Code would use for this cwd
    // and write a non-trivial MEMORY.md there. If tracer ever reactivates
    // the memory leg, this file would land in `mirrored` and the test
    // would fail.
    let cwd_abs = std::fs::canonicalize(&f.root).unwrap();
    let slug = cwd_abs.to_string_lossy().replace('/', "-");
    let memory_dir = home
        .join(".claude")
        .join("projects")
        .join(&slug)
        .join("memory");
    std::fs::create_dir_all(&memory_dir).unwrap();
    std::fs::write(
        memory_dir.join("MEMORY.md"),
        "# Memory\n\nShould never appear in the context primer.\n",
    )
    .unwrap();

    let home_str = home.to_string_lossy().into_owned();
    let run = f.trace_env(
        &["context", "prime", "--reason", "post_compact", "--json"],
        &env_pairs(&home_str, &sid),
    );
    run.ok();

    let v = run.json();
    assert_eq!(v["reason"], "post_compact");
    assert_eq!(v["source"], "context_prime_post_compact");
    // Exactly one mirrored doc — the CLAUDE.md. MEMORY.md is present on
    // disk at the canonical Claude Code path but out of tracer's scope,
    // so the primer skips it entirely.
    assert_eq!(
        v["mirrored_count"], 1,
        "MEMORY.md must not enter the context primer: {v}"
    );
    assert_eq!(v["mirrored"][0]["path"], "CLAUDE.md");

    let mirrored_paths: BTreeSet<String> = v["mirrored"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["path"].as_str().unwrap().to_string())
        .collect();
    assert!(
        !mirrored_paths.iter().any(|p| p.contains("MEMORY.md")),
        "no MEMORY.md path may appear in mirrored set: {mirrored_paths:?}"
    );

    let events = read_events(&home, &sid);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["source"], "context_prime_post_compact");
    let event_paths: Vec<String> = events
        .iter()
        .map(|e| e["visible_as"].as_str().unwrap().to_string())
        .collect();
    assert!(
        !event_paths.iter().any(|p| p.contains("MEMORY.md")),
        "no MEMORY.md path may be recorded in the session log: {event_paths:?}"
    );
}

// --- AGENTS.md harness convention -----------------------------------------

#[test]
fn agents_md_in_project_root_is_mirrored_with_agents_kind() {
    // `AGENTS.md` is the OpenAI / cross-harness rules-file convention
    // (Codex, Cursor, Aider, Jules, Amp et al.). The context primer must
    // surface it from the project-root chain with `kind: agents_md`,
    // independent of any CLAUDE.md presence.
    let (f, home, sid) = isolated("agents-only");
    f.write("AGENTS.md", "# Cross-harness rules\n\nProject root.\n");
    f.commit("add agents.md only");

    let home_str = home.to_string_lossy().into_owned();
    let run = f.trace_env(
        &["context", "prime", "--reason", "session_start", "--json"],
        &env_pairs(&home_str, &sid),
    );
    run.ok();

    let v = run.json();
    assert_eq!(
        v["mirrored_count"], 1,
        "AGENTS.md must surface like CLAUDE.md does: {v}"
    );
    let m = &v["mirrored"][0];
    assert_eq!(m["path"], "AGENTS.md");
    assert_eq!(
        m["kind"], "agents_md",
        "AGENTS.md must carry its own kind, not be collapsed into claude_md: {v}"
    );

    let events = read_events(&home, &sid);
    assert_eq!(events.len(), 1, "one event per surfaced doc: {events:?}");
    assert_eq!(events[0]["source"], "context_prime_session_start");
}

#[test]
fn agents_md_and_claude_md_both_surface_with_distinct_kinds() {
    // Repo has both conventions side-by-side — a real multi-harness setup.
    // Both must be mirrored, each tagged with its own kind so consumers
    // can attribute by harness origin.
    let (f, home, sid) = isolated("both");
    f.write("CLAUDE.md", "# Claude rules\n\nClaude Code.\n");
    f.write("AGENTS.md", "# Agents rules\n\nCross-harness.\n");
    f.commit("add both rules files");

    let home_str = home.to_string_lossy().into_owned();
    let run = f.trace_env(
        &["context", "prime", "--reason", "session_start", "--json"],
        &env_pairs(&home_str, &sid),
    );
    run.ok();

    let v = run.json();
    assert_eq!(
        v["mirrored_count"], 2,
        "both rules files must surface: {v}"
    );

    let by_path: std::collections::BTreeMap<String, String> = v["mirrored"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| {
            (
                m["path"].as_str().unwrap().to_string(),
                m["kind"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert_eq!(by_path.get("CLAUDE.md").map(String::as_str), Some("claude_md"));
    assert_eq!(by_path.get("AGENTS.md").map(String::as_str), Some("agents_md"));
}

#[test]
fn user_global_agents_md_is_mirrored_when_claude_md_absent() {
    // The user-global rules dir (`$HOME/.claude/`) is probed for both
    // conventions; CLAUDE.md wins when present, AGENTS.md is recognized
    // as the fallback peer. With only AGENTS.md present at the user-global
    // location, the context primer must surface it with `kind: agents_md`.
    let (f, home, sid) = isolated("user-global-agents");
    std::fs::write(
        home.join(".claude").join("AGENTS.md"),
        "# Global agents rules\n\nUser-global.\n",
    )
    .unwrap();
    // No project-root rules file — the only doc the mirror should see is
    // the user-global AGENTS.md.
    f.commit("empty project (no rules file)");

    let home_str = home.to_string_lossy().into_owned();
    let run = f.trace_env(
        &["context", "prime", "--reason", "session_start", "--json"],
        &env_pairs(&home_str, &sid),
    );
    run.ok();

    let v = run.json();
    assert_eq!(
        v["mirrored_count"], 1,
        "user-global AGENTS.md must surface in the absence of CLAUDE.md: {v}"
    );
    let m = &v["mirrored"][0];
    assert_eq!(m["kind"], "agents_md");
    assert!(
        m["path"].as_str().unwrap().ends_with("AGENTS.md"),
        "user-global AGENTS.md path expected: {m}"
    );
}

#[test]
fn lowercase_agents_md_casing_is_also_recognized() {
    // `Agents.md` (mixed casing) is the other casing in the wild — the
    // dotfiles repo's `packages/codex/Agents.md` symlink uses it. Both
    // casings must be probed; this case exists to fail loudly if the
    // mixed casing is dropped from the candidate set.
    let (f, home, sid) = isolated("agents-mixed-casing");
    f.write("Agents.md", "# Mixed-case rules\n\nProject root.\n");
    f.commit("add Agents.md (mixed casing)");

    let home_str = home.to_string_lossy().into_owned();
    let run = f.trace_env(
        &["context", "prime", "--reason", "session_start", "--json"],
        &env_pairs(&home_str, &sid),
    );
    run.ok();

    let v = run.json();
    assert_eq!(
        v["mirrored_count"], 1,
        "Agents.md (mixed casing) must surface: {v}"
    );
    assert_eq!(v["mirrored"][0]["kind"], "agents_md");
    // On case-insensitive filesystems (macOS default) the canonical path
    // collapses to whichever physical form exists; either AGENTS.md or
    // Agents.md is fine.
    let path = v["mirrored"][0]["path"].as_str().unwrap().to_lowercase();
    assert!(
        path.ends_with("agents.md"),
        "mixed-case Agents.md must surface under an agents.md path: {}",
        v["mirrored"][0]["path"]
    );
}
