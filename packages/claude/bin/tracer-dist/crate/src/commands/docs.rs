//! `trace docs <path>` — the deduped project-docs set for a path.
//!
//! Single-purpose surface over the shared `nested_memory` walk-up and its
//! existing per-session dedupe: the same `Claude.md` / `CLAUDE.md` /
//! `.claude` / rules ancestor walk `trace read` injects, returned on its
//! own. Going through the same session-dedupe state file means a doc
//! surfaced by `trace docs` is not re-emitted by a later `trace read` in
//! the same session, and the reverse.

use super::nested_memory;
use crate::cache;
use anyhow::Result;
use serde_json::{json, Value};
use std::path::Path;

pub fn run(path: &Path, directory_mode: bool, as_json: bool) -> Result<Value> {
    let target_raw = path;
    let target = target_raw
        .canonicalize()
        .unwrap_or_else(|_| cache::absolutize(target_raw));
    if !target.exists() {
        eprintln!("Error: path not found: {}", target_raw.display());
        std::process::exit(2);
    }
    let repo_root = cache::repo_root_for(&target);
    let scope_dir = directory_mode || target.is_dir();

    let mut session_dedupe = nested_memory::load_session_dedupe();
    let memories =
        nested_memory::load_for_file(&target, &repo_root, &mut session_dedupe, scope_dir);
    nested_memory::save_session_dedupe(&session_dedupe);

    let display = cache::relative_to_root(&target, &repo_root);

    let out = json!({
        "path": display,
        "directory_scoped": scope_dir,
        "docs": memories.iter().map(|m| json!({
            "path": m.relative_path,
            "kind": m.kind,
            "size": m.size,
            "large": m.large,
            "content": m.content,
        })).collect::<Vec<_>>(),
        "doc_count": memories.len(),
    });

    if as_json {
        return Ok(out);
    }

    let mut header = format!("# docs for {display}");
    if scope_dir {
        header += " (directory-scoped)";
    }
    println!("{header}");
    if memories.is_empty() {
        println!("  (no project docs for this path in this session)");
        return Ok(out);
    }
    let block = nested_memory::render(&memories);
    if !block.is_empty() {
        println!("{block}");
    }
    Ok(out)
}
