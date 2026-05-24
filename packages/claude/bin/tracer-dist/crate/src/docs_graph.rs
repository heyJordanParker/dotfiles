//! Docs-graph builder: walks every recognized project-rules markdown file
//! in the repo, resolves `@include` directives, parses conditional
//! `paths:` frontmatter onto rule nodes. Two harness conventions are
//! recognized: Claude Code's `CLAUDE.md`/`Claude.md` (plus `.local.md`
//! peer) and OpenAI's `AGENTS.md`/`Agents.md` (plus `.local.md` peer)
//! — the cross-harness convention adopted by Codex, Cursor, Aider, Jules,
//! Amp et al. Both share the directory walk and the same `@include`/
//! `paths:` mechanics; each gets its own `kind` (`agents_md` /
//! `agents_local_md` mirror the existing `claude_md` / `local_md`).
//!
//! Contributes doc-file nodes and `includes` edges to the unified
//! architecture graph — no separate cache entry. The single cache entry
//! lives under `architecture/` (keyed by `cache::architecture_fingerprint`
//! over both per-file content hashes and the docs-side `git_head +
//! doc_mtime_aggregate`), so a doc-file edit invalidates the same entry a
//! code-file edit does.
//!
//! The walker, include resolver, and frontmatter parsers belong to
//! `commands::nested_memory`; this module only assembles the graph from
//! their output. There is no second implementation of any of those.

use crate::cache;
use crate::commands::nested_memory;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// One doc node in the graph. Kinds mirror `LoadedMemory.kind` values
/// emitted by `nested_memory` (claude_md / local_md / rules_unconditional /
/// rules_conditional / include) so consumers can filter by source category.
#[derive(Debug, Clone)]
pub struct DocNode {
    pub path: String,
    pub kind: String,
    pub size: usize,
    /// `paths:` frontmatter globs for conditional rules; None otherwise.
    pub paths_globs: Option<Vec<String>>,
}

/// `source` includes `target` via an `@include` directive (or is a rules
/// file whose conditional `paths:` matched).
#[derive(Debug, Clone)]
pub struct DocEdge {
    pub source: String,
    pub target: String,
    pub relation: String,
}

#[derive(Debug, Clone, Default)]
pub struct DocsGraph {
    pub head: String,
    pub mtime_aggregate: String,
    pub built_at_ms: u128,
    pub nodes: Vec<DocNode>,
    pub edges: Vec<DocEdge>,
}

impl DocsGraph {
    pub fn to_json(&self) -> Value {
        json!({
            "head": self.head,
            "mtime_aggregate": self.mtime_aggregate,
            "built_at_ms": self.built_at_ms,
            "nodes": self.nodes.iter().map(|n| json!({
                "path": n.path,
                "kind": n.kind,
                "size": n.size,
                "paths_globs": n.paths_globs,
            })).collect::<Vec<_>>(),
            "edges": self.edges.iter().map(|e| json!({
                "source": e.source,
                "target": e.target,
                "relation": e.relation,
            })).collect::<Vec<_>>(),
        })
    }
}

/// Docs-side inputs to the unified architecture fingerprint. Computed
/// alongside the graph itself so callers fingerprint over the same
/// snapshot they cache.
#[derive(Debug, Clone)]
pub struct DocsInputs {
    pub head: String,
    pub mtime_aggregate: String,
}

/// Build the docs graph for `repo_root`. Pure in-memory build — no cache
/// reads, no cache writes. Returns the graph plus the docs-side inputs
/// the unified architecture-graph fingerprint needs.
pub fn build(repo_root: &Path) -> (DocsGraph, DocsInputs) {
    let head = git_head(repo_root);
    let doc_files = discover_doc_files(repo_root);
    let mtime_aggregate = mtime_aggregate(&doc_files);
    let graph = assemble(repo_root, head.clone(), mtime_aggregate.clone(), &doc_files);
    let inputs = DocsInputs { head, mtime_aggregate };
    (graph, inputs)
}

fn git_head(repo_root: &Path) -> String {
    Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo_root)
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "no-head".to_string())
}

/// Every recognized rules-markdown file under repo_root, in canonical sort
/// order. The recognized set spans both harness conventions: Claude Code's
/// `CLAUDE.md` / `Claude.md` (plus `.local.md` peer) and OpenAI's
/// `AGENTS.md` / `Agents.md` (plus `.local.md` peer), plus any markdown
/// file under a `.claude/rules/` ancestor. Hidden dirs other than
/// `.claude` are pruned, plus the standard SKIP_DIRS set (`.git`,
/// `.tracer-cache`, `node_modules`, …).
fn discover_doc_files(repo_root: &Path) -> Vec<PathBuf> {
    let skip = crate::repo_files::skip_dirs();
    let mut out: Vec<PathBuf> = Vec::new();
    let walker = walkdir::WalkDir::new(repo_root).into_iter().filter_entry(|e| {
        let name = e.file_name().to_string_lossy();
        if e.file_type().is_dir() {
            // Allow `.claude` and the repo root through; otherwise prune
            // hidden dirs and the skip set.
            if name == ".claude" {
                return true;
            }
            !skip.contains(name.as_ref()) && !name.starts_with('.')
        } else {
            true
        }
    });
    for entry in walker.flatten() {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let name = entry.file_name().to_string_lossy();
        if is_rules_doc(&name) {
            out.push(path.to_path_buf());
            continue;
        }
        // `.claude/rules/**/*.md` — any markdown file anywhere under a
        // `.claude/rules` ancestor.
        if path.extension().and_then(|e| e.to_str()) == Some("md")
            && path.components().any(|c| c.as_os_str() == "rules")
            && path.components().any(|c| c.as_os_str() == ".claude")
        {
            out.push(path.to_path_buf());
        }
    }
    out.sort();
    out
}

/// True for any project-rules markdown filename recognized by either
/// harness convention. Claude Code: `CLAUDE.md` / `Claude.md` /
/// `CLAUDE.local.md` / `Claude.local.md`. OpenAI (Codex + the cross-harness
/// `AGENTS.md` ecosystem — Cursor, Aider, Jules, Amp et al.): `AGENTS.md`
/// / `Agents.md` / `AGENTS.local.md` / `Agents.local.md`.
fn is_rules_doc(name: &str) -> bool {
    matches!(
        name,
        "CLAUDE.md"
            | "Claude.md"
            | "CLAUDE.local.md"
            | "Claude.local.md"
            | "AGENTS.md"
            | "Agents.md"
            | "AGENTS.local.md"
            | "Agents.local.md"
    )
}

/// sha256 over (relpath\0mtime_ns\n) for every doc file, in sorted order.
/// `repo_root` resolution is intentionally not applied here — the input
/// list is already canonical-absolute, and the aggregate is over its
/// stable identity. A missing mtime contributes the literal "no-mtime".
fn mtime_aggregate(doc_files: &[PathBuf]) -> String {
    let mut h = Sha256::new();
    for p in doc_files {
        let mtime = fs::metadata(p)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| format!("{}", d.as_nanos()))
            .unwrap_or_else(|| "no-mtime".to_string());
        h.update(p.to_string_lossy().as_bytes());
        h.update(b"\0");
        h.update(mtime.as_bytes());
        h.update(b"\n");
    }
    format!("sha256:{}", hex::encode(h.finalize()))
}

fn assemble(
    repo_root: &Path,
    head: String,
    mtime_aggregate: String,
    doc_files: &[PathBuf],
) -> DocsGraph {
    let mut nodes: BTreeMap<String, DocNode> = BTreeMap::new();
    let mut edges: Vec<DocEdge> = Vec::new();
    let mut session_dedupe: BTreeSet<String> = BTreeSet::new();

    for path in doc_files {
        let mut pass_dedupe: BTreeSet<String> = BTreeSet::new();
        let kind = classify(path);
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        if content.trim().is_empty() {
            continue;
        }
        let paths_globs = if kind.starts_with("rules") {
            nested_memory::extract_paths_frontmatter(&content)
        } else {
            None
        };
        // Promote rules-unconditional to rules_conditional when frontmatter
        // is present — matches nested_memory's runtime classification.
        let kind = if kind == "rules_unconditional" && paths_globs.is_some() {
            "rules_conditional".to_string()
        } else {
            kind
        };
        let rel = relative_path(path, repo_root);
        let size = content.chars().count();
        nodes.insert(
            rel.clone(),
            DocNode {
                path: rel.clone(),
                kind,
                size,
                paths_globs,
            },
        );

        // Resolve @-includes through nested_memory's shared walker.
        let included = nested_memory::load_includes(
            path,
            &content,
            repo_root,
            &mut pass_dedupe,
            &mut session_dedupe,
            0,
        );
        for inc in included {
            let inc_rel = inc.relative_path.clone();
            edges.push(DocEdge {
                source: rel.clone(),
                target: inc_rel.clone(),
                relation: "includes".to_string(),
            });
            nodes.entry(inc_rel.clone()).or_insert(DocNode {
                path: inc_rel,
                kind: inc.kind,
                size: inc.size,
                paths_globs: None,
            });
        }
    }

    DocsGraph {
        head,
        mtime_aggregate,
        built_at_ms: now_ms(),
        nodes: nodes.into_values().collect(),
        edges,
    }
}

/// Classify a doc file by its on-disk path. Conditional-rules promotion
/// happens later in `assemble` once the file's frontmatter is parsed.
fn classify(path: &Path) -> String {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    match name.as_str() {
        "CLAUDE.local.md" | "Claude.local.md" => "local_md".to_string(),
        "AGENTS.local.md" | "Agents.local.md" => "agents_local_md".to_string(),
        "AGENTS.md" | "Agents.md" => "agents_md".to_string(),
        n if is_rules_doc(n) => "claude_md".to_string(),
        _ => "rules_unconditional".to_string(),
    }
}

fn relative_path(path: &Path, repo_root: &Path) -> String {
    cache::relative_to_root(path, repo_root)
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}
