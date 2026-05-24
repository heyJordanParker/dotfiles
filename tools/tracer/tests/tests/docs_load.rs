//! `trace docs load <path>` — the hook-facing alias forwarding to path-mode
//! plus path-mode itself. Black-box assertions on the unified response shape,
//! log emission, and source/triggering metadata round-trip.
//!
//! Pins the contract `inject-docs.sh` depends on:
//!   - response includes `docs` (new) and may include `already_loaded`
//!     (skipped with per-entry source) so the hook can surface both slices
//!     to the agent
//!   - emissions land in the log atomically with the response,
//!     under the supplied `--source` value
//!   - `--triggering-tool` and `--triggering-command` flags land on the
//!     log event so downstream auditing has the original
//!     tool + command
//!   - second invocation surfaces nothing new (already_loaded carries the
//!     full ancestor set; `docs` is empty; `doc_count` is 0)
//!   - the `load` sub-verb and path-mode share one response shape

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use tracer_cli_tests::Fixture;

#[allow(non_upper_case_globals)] // project naming rule bans ALL_CAPS for our own identifiers
static load_seq: AtomicU64 = AtomicU64::new(0);

fn fresh_session_id(tag: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let seq = load_seq.fetch_add(1, Ordering::SeqCst);
    format!("trace-docs-load-{tag}-{nanos}-{seq}")
}

fn log_dir(repo_root: &Path, session_id: &str, agent_id: &str) -> PathBuf {
    repo_root
        .join(".tracer-cache")
        .join("sessions")
        .join(session_id)
        .join(agent_id)
}

fn read_events_jsonl(repo_root: &Path, session_id: &str, agent_id: &str) -> Vec<serde_json::Value> {
    let path = log_dir(repo_root, session_id, agent_id).join("events.jsonl");
    let Ok(text) = std::fs::read_to_string(&path) else {
        return vec![];
    };
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("events.jsonl line not valid JSON"))
        .collect()
}

fn docs_repo() -> Fixture {
    let f = Fixture::new();
    f.write("Claude.md", "# Root rules\n\nProject root.\n");
    f.write("sub/Claude.md", "# Sub rules\n\nThis dir has rules.\n");
    f.write(
        "sub/util.py",
        "def helper(v):\n    if v > 0:\n        return v + 1\n    return 0\n",
    );
    f.commit("init docs repo");
    f
}

// --- response shape -------------------------------------------------------

#[test]
fn first_load_surfaces_full_chain_with_no_already_loaded_key() {
    let f = docs_repo();
    let sid = fresh_session_id("first");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    let r = f.trace_env(
        &["docs", "load", "sub/util.py", "--source", "trace_inject_hook", "--json"],
        &env,
    );
    r.ok();
    let v = r.json();

    let docs = v["docs"].as_array().expect("docs must be an array");
    assert_eq!(
        docs.len(),
        2,
        "first load must surface both Claude.md ancestors (root + sub): {v}"
    );
    assert_eq!(v["doc_count"].as_i64().unwrap(), 2);
    assert!(
        v.get("already_loaded").is_none(),
        "first load against an empty log must omit `already_loaded` entirely: {v}"
    );
    assert_eq!(v["source"].as_str().unwrap(), "trace_inject_hook");

    // Every docs entry carries the doc content + identity fields the hook
    // needs to render to the agent.
    for entry in docs {
        for key in ["path", "kind", "size", "large", "content"] {
            assert!(
                entry.get(key).is_some(),
                "docs entry missing `{key}`: {entry}"
            );
        }
    }
}

#[test]
fn second_load_returns_full_chain_in_already_loaded_with_empty_docs() {
    let f = docs_repo();
    let sid = fresh_session_id("second");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    // Prime: first call surfaces the chain.
    f.trace_env(
        &["docs", "load", "sub/util.py", "--source", "trace_inject_hook", "--json"],
        &env,
    )
    .ok();

    // Second call against the same session: the same chain is already in
    // the log, so `docs` is empty and `already_loaded` carries
    // the full set.
    let r = f.trace_env(
        &["docs", "load", "sub/util.py", "--source", "trace_inject_hook", "--json"],
        &env,
    );
    r.ok();
    let v = r.json();

    let docs = v["docs"].as_array().expect("docs must be an array");
    let already_loaded = v["already_loaded"]
        .as_array()
        .expect("already_loaded must be present when non-empty");

    assert!(
        docs.is_empty(),
        "second load must surface nothing new — chain is in the log: {v}"
    );
    assert_eq!(v["doc_count"].as_i64().unwrap(), 0);
    assert_eq!(
        already_loaded.len(),
        2,
        "second load must report both ancestors as already_loaded: {v}"
    );

    // Each already_loaded entry carries a per-entry source — the value
    // recorded when the doc was originally surfaced. First call used
    // `trace_inject_hook`, so both entries report that source.
    for entry in already_loaded {
        for key in ["path", "kind", "size", "large", "source"] {
            assert!(
                entry.get(key).is_some(),
                "already_loaded entry missing `{key}`: {entry}"
            );
        }
        // Per-entry content is excluded — the agent already has it in context;
        // the hook only needs identity + source to render the skipped slice.
        assert!(
            entry.get("content").is_none(),
            "already_loaded entry must omit content (it's already in context): {entry}"
        );
        assert_eq!(
            entry["source"].as_str().unwrap(),
            "trace_inject_hook",
            "already_loaded entry must report the original load source: {entry}"
        );
    }
}

// --- log emission -------------------------------------------

#[test]
fn emissions_are_recorded_under_the_supplied_source() {
    let f = docs_repo();
    let sid = fresh_session_id("source");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    f.trace_env(
        &["docs", "load", "sub/util.py", "--source", "trace_inject_hook", "--json"],
        &env,
    )
    .ok();

    let events = read_events_jsonl(&f.root, &sid, "root");
    assert_eq!(
        events.len(),
        2,
        "expected one event per surfaced doc, got: {events:?}"
    );
    for ev in &events {
        assert_eq!(
            ev["source"].as_str().unwrap(),
            "trace_inject_hook",
            "log event must record the --source value verbatim: {ev}"
        );
        assert_eq!(ev["kind"].as_str().unwrap(), "doc_injection", "{ev}");
        assert!(
            ev["content_hash"].as_str().unwrap().starts_with("sha256:"),
            "{ev}"
        );
    }
}

#[test]
fn triggering_tool_and_command_land_on_the_log_event() {
    let f = docs_repo();
    let sid = fresh_session_id("trigger");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    f.trace_env(
        &[
            "docs", "load", "sub/util.py",
            "--source", "trace_inject_hook",
            "--triggering-tool", "Bash",
            "--triggering-command", "trace info sub/util.py",
            "--json",
        ],
        &env,
    )
    .ok();

    let events = read_events_jsonl(&f.root, &sid, "root");
    assert!(
        !events.is_empty(),
        "log must have at least one event after load"
    );
    for ev in &events {
        assert_eq!(
            ev["triggering_tool"].as_str().unwrap(),
            "Bash",
            "log event must record --triggering-tool: {ev}"
        );
        assert_eq!(
            ev["triggering_command"].as_str().unwrap(),
            "trace info sub/util.py",
            "log event must record --triggering-command: {ev}"
        );
    }
}

#[test]
fn omitted_triggering_flags_record_no_triggering_metadata() {
    let f = docs_repo();
    let sid = fresh_session_id("no-trigger");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    f.trace_env(
        &["docs", "load", "sub/util.py", "--source", "trace_inject_hook", "--json"],
        &env,
    )
    .ok();

    let events = read_events_jsonl(&f.root, &sid, "root");
    for ev in &events {
        // The schema field is `Option<String>` on the log;
        // absent flag → null.
        assert!(
            ev["triggering_tool"].is_null(),
            "absent --triggering-tool must serialize as null: {ev}"
        );
        assert!(
            ev["triggering_command"].is_null(),
            "absent --triggering-command must serialize as null: {ev}"
        );
    }
}

// --- cross-source dedupe --------------------------------------------------

#[test]
fn docs_loaded_by_path_mode_appear_in_already_loaded_with_path_mode_source() {
    // Prior path-mode call seeds the log under source
    // `trace_docs`. A follow-up `load` call must observe those entries in
    // already_loaded with `source: "trace_docs"`, proving the per-entry
    // source attribution tracks the original load surface, not the current
    // one.
    let f = docs_repo();
    let sid = fresh_session_id("cross-source");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    // Seed via path-mode → source `trace_docs` on the events.
    f.trace_env(&["docs", "sub/util.py", "--json"], &env).ok();

    let r = f.trace_env(
        &["docs", "load", "sub/util.py", "--source", "trace_inject_hook", "--json"],
        &env,
    );
    r.ok();
    let v = r.json();

    let already_loaded = v["already_loaded"]
        .as_array()
        .expect("already_loaded must be present");
    assert_eq!(
        already_loaded.len(),
        2,
        "path-mode seeded both ancestors; load must see them as already_loaded: {v}"
    );
    for entry in already_loaded {
        assert_eq!(
            entry["source"].as_str().unwrap(),
            "trace_docs",
            "per-entry source must reflect the ORIGINAL load surface (`trace_docs`), \
             not the current call's `--source`: {entry}"
        );
    }
}

// --- path-mode shape parity -----------------------------------------------

#[test]
fn path_mode_returns_the_same_shape_as_load_alias() {
    // `trace docs <path>` and `trace docs load <path>` are one implementation
    // behind two CLI verbs. The response shape must match on every key
    // visible to a hook reader: `docs`, `doc_count`, optional
    // `already_loaded`. The two verbs only differ in default `--source`
    // (`trace_docs` vs `trace_docs_load`).
    let f = docs_repo();
    let sid_path = fresh_session_id("shape-path");
    let env_path = [("CLAUDE_CODE_SESSION_ID", sid_path.as_str())];
    let sid_load = fresh_session_id("shape-load");
    let env_load = [("CLAUDE_CODE_SESSION_ID", sid_load.as_str())];

    let r_path = f.trace_env(&["docs", "sub/util.py", "--json"], &env_path);
    r_path.ok();
    let v_path = r_path.json();

    let r_load = f.trace_env(&["docs", "load", "sub/util.py", "--json"], &env_load);
    r_load.ok();
    let v_load = r_load.json();

    assert!(v_path["docs"].is_array(), "path-mode must carry the `docs` key: {v_path}");
    assert!(v_load["docs"].is_array(), "load alias must carry the `docs` key: {v_load}");
    assert_eq!(v_path["doc_count"].as_i64().unwrap(), 2);
    assert_eq!(v_load["doc_count"].as_i64().unwrap(), 2);
    assert!(
        v_path.get("already_loaded").is_none(),
        "fresh path-mode call must omit already_loaded: {v_path}"
    );
    assert!(
        v_load.get("already_loaded").is_none(),
        "fresh load alias call must omit already_loaded: {v_load}"
    );

    // Default sources differ.
    assert_eq!(v_path["source"].as_str().unwrap(), "trace_docs");
    assert_eq!(v_load["source"].as_str().unwrap(), "trace_docs_load");
}

// --- graph flag regression ------------------------------------------------

#[test]
fn graph_flag_returns_graph_document() {
    // `--graph` is the flag form on `trace docs`. Sanity-check it returns
    // the graph document shape (node_count + graph keys present).
    let f = docs_repo();
    let r = f.trace(&["docs", "--graph", "--json"]);
    r.ok();
    let v = r.json();
    assert!(v["graph"].is_object(), "graph mode must return a graph object: {v}");
    assert!(
        v["node_count"].as_i64().unwrap() >= 2,
        "graph must surface at least the two Claude.md nodes: {v}"
    );
}

#[test]
fn graph_sub_verb_no_longer_accepted() {
    // `graph` was briefly a sub-verb and was reverted to the `--graph` flag.
    // Calling the old sub-verb form must fail with exit 2 — clap has no
    // `graph` subcommand so the token falls through to the positional
    // `<path>`, which fails path validation with "path not found: graph".
    // Either path proves the sub-verb is gone — not silently absorbed.
    let f = docs_repo();
    let r = f.trace(&["docs", "graph", "--json"]);
    r.code_is(2);
    let out = r.combined();
    assert!(
        out.contains("unexpected argument")
            || out.contains("unrecognized subcommand")
            || out.contains("Usage")
            || out.contains("path not found: graph"),
        "expected a clap usage error or path-validation error rejecting `graph`, got: {out}"
    );
}

// --- default source -------------------------------------------------------

#[test]
fn default_source_when_flag_omitted() {
    // `--source` has a default so a hook can omit it during local debugging.
    // The contract: the supplied default lands on the log.
    let f = docs_repo();
    let sid = fresh_session_id("default-source");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    let r = f.trace_env(&["docs", "load", "sub/util.py", "--json"], &env);
    r.ok();
    let v = r.json();
    let default_source = v["source"].as_str().unwrap();
    assert_eq!(
        default_source, "trace_docs_load",
        "load alias must default --source to `trace_docs_load`: {default_source}"
    );

    let events = read_events_jsonl(&f.root, &sid, "root");
    for ev in &events {
        assert_eq!(
            ev["source"].as_str().unwrap(),
            default_source,
            "log event source must match the response default: {ev}"
        );
    }
}

// --- standalone (no session id) -------------------------------------------

#[test]
fn no_session_id_means_load_still_returns_shape_and_writes_no_log() {
    // Standalone tracer use: no CLAUDE_CODE_SESSION_ID → the
    // log is a no-op, but `load` still returns the response
    // shape so a calling hook gets a structured "nothing was new, nothing
    // was loaded" answer instead of an error.
    let f = docs_repo();
    let r = std::process::Command::new(tracer_cli_tests::trace_bin())
        .args(["docs", "load", "sub/util.py", "--source", "trace_inject_hook", "--json"])
        .current_dir(&f.root)
        .env_remove("CLAUDE_CODE_SESSION_ID")
        .env_remove("CLAUDE_SESSION_ID")
        .env_remove("TRACER_SESSION_ID")
        .output()
        .expect("spawn trace");
    assert!(
        r.status.success(),
        "standalone trace docs load must succeed:\n{}",
        String::from_utf8_lossy(&r.stderr)
    );
    let stdout = String::from_utf8_lossy(&r.stdout);
    let v: serde_json::Value =
        serde_json::from_str(&stdout).expect("standalone load must still return JSON");
    // Without a session id the log no-ops, so EVERY doc
    // surfaces as new on each call; nothing is "already loaded".
    assert_eq!(v["doc_count"].as_i64().unwrap(), 2);
    assert!(
        v.get("already_loaded").is_none(),
        "no session id ⇒ no priors ⇒ already_loaded omitted: {v}"
    );
}

// --- AGENTS.md harness convention recognition -----------------------------

#[test]
fn agents_md_surfaces_in_per_file_doc_walk_with_agents_md_kind() {
    // The per-file walker (nested_memory) must recognize AGENTS.md and its
    // dual-casing peer Agents.md across every ancestor, tagging each with
    // `kind: agents_md`. Two ancestors carry one each, both casings:
    // root holds AGENTS.md; sub/ holds Agents.md.
    let f = Fixture::new();
    f.write("AGENTS.md", "# Cross-harness root\n\nProject root rules.\n");
    f.write("sub/Agents.md", "# Cross-harness sub\n\nDir-scoped rules.\n");
    f.write("sub/util.py", "def helper():\n    return 1\n");
    f.commit("agents.md per-file walk fixture");

    let sid = fresh_session_id("agents-walk");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    let r = f.trace_env(&["docs", "load", "sub/util.py", "--json"], &env);
    r.ok();
    let v = r.json();
    let docs = v["docs"].as_array().expect("docs must be an array");
    assert_eq!(
        docs.len(),
        2,
        "both AGENTS.md ancestors must surface (root + sub): {v}"
    );
    for d in docs {
        assert_eq!(
            d["kind"].as_str().unwrap(),
            "agents_md",
            "AGENTS.md ancestors must carry kind `agents_md`, not collapse into `claude_md`: {d}"
        );
    }
}

#[test]
fn agents_local_md_surfaces_with_agents_local_md_kind() {
    // The personal-overrides peer (`AGENTS.local.md`) is the gitignored
    // sibling of `AGENTS.md`, analogous to `CLAUDE.local.md`. It must
    // surface from the per-file walk with kind `agents_local_md`.
    let f = Fixture::new();
    // Untracked on purpose — `.local.md` files are gitignored by
    // convention. The walker doesn't care about VCS tracking; it cares
    // about on-disk presence.
    f.write("AGENTS.md", "# Cross-harness\n\nProject root.\n");
    f.write("AGENTS.local.md", "# Personal overrides\n\nLocal only.\n");
    f.write("util.py", "def helper():\n    return 1\n");
    f.commit("agents + agents.local fixture");

    let sid = fresh_session_id("agents-local");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    let r = f.trace_env(&["docs", "load", "util.py", "--json"], &env);
    r.ok();
    let v = r.json();
    let kinds: std::collections::BTreeSet<String> = v["docs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["kind"].as_str().unwrap().to_string())
        .collect();
    assert!(
        kinds.contains("agents_md"),
        "AGENTS.md must surface with kind `agents_md`: {v}"
    );
    assert!(
        kinds.contains("agents_local_md"),
        "AGENTS.local.md must surface with kind `agents_local_md`: {v}"
    );
}

#[test]
fn claude_md_and_agents_md_coexist_in_per_file_walk() {
    // Multi-harness repos carry both files. The walker must surface both,
    // each with its own kind — no collapse, no preference.
    let f = Fixture::new();
    f.write("CLAUDE.md", "# Claude\n\n");
    f.write("AGENTS.md", "# Agents\n\n");
    f.write("util.py", "def helper():\n    return 1\n");
    f.commit("both conventions side-by-side");

    let sid = fresh_session_id("both-conventions");
    let env = [("CLAUDE_CODE_SESSION_ID", sid.as_str())];

    let r = f.trace_env(&["docs", "load", "util.py", "--json"], &env);
    r.ok();
    let v = r.json();
    let by_path: std::collections::BTreeMap<String, String> = v["docs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            (
                d["path"].as_str().unwrap().to_string(),
                d["kind"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert_eq!(
        by_path.get("CLAUDE.md").map(String::as_str),
        Some("claude_md"),
        "CLAUDE.md retains its kind: {v}"
    );
    assert_eq!(
        by_path.get("AGENTS.md").map(String::as_str),
        Some("agents_md"),
        "AGENTS.md gets its own kind: {v}"
    );
}
