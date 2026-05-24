//! Context-primer drift detector — set-comparison surface for the
//! deterministic `context_prime` prediction vs. the observed set that
//! Claude Code actually injected at SessionStart.
//!
//! The detector is the architectural anchor that lets the tracer trust the
//! primer over time: if a Claude Code release silently changes auto-load
//! rules, or a user override modifies the inventory, the prediction in
//! `context_prime::run` and reality diverge. This module computes that
//! divergence and reconciles the session log's view to observed reality
//! so subsequent emissions (Read enrichment, `trace docs`, …) operate on
//! what Claude Code actually loaded — never on a stale prediction.
//!
//! Wired into `context_prime::run`: after the predicted set is recorded via
//! `session_log::record_emission`, the observed set is read from
//! `--observed-from <path>` (`-` for stdin) and compared. On drift, the
//! diff is appended to the log as an `EventKind::ContextPrimeDrift` event
//! (source `"context_prime_drift"`) and the view's `emitted` map is rewritten
//! from {predicted paths} to {observed paths + their content hashes}. No
//! observed input means no drift detection — standalone tracer use stays
//! valid, matching the no-op contract the log itself honors when the
//! session id is absent.
//!
//! Input contract (the only Claude-Code-facing surface a hook owns):
//!
//! ```json
//! {
//!   "paths": [
//!     {"path": "/abs/CLAUDE.md",           "content_hash": "sha256:…", "size": 1234},
//!     {"path": "/abs/sub/Claude.md",       "content_hash": "sha256:…", "size":  456}
//!   ]
//! }
//! ```
//!
//! `size` is optional. `content_hash` must be a `sha256:<hex>` string so
//! the reconciled view's hashes stay schema-compatible with the rest of
//! the log.

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::io::Read;

/// One observed doc the harness actually loaded. Mirrors the subset of
/// `nested_memory::LoadedMemory` the log persists.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObservedDoc {
    pub path: String,
    pub content_hash: String,
    #[serde(default)]
    pub size: usize,
}

/// The full observed set parsed from `--observed-from`. A flat list keeps
/// the input contract obvious to a bash hook author.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct Observed {
    pub paths: Vec<ObservedDoc>,
}

/// Drift between the predicted set (already in the log view) and the
/// observed set. `missing` = predicted but not observed; `extra` = observed
/// but not predicted. Empty `missing` + empty `extra` means no drift.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Report {
    pub predicted: Vec<String>,
    pub observed: Vec<String>,
    pub missing: Vec<String>,
    pub extra: Vec<String>,
}

/// Read the observed set from `source`. `source == "-"` reads stdin; any
/// other value is treated as a path. Returns `None` when the input is
/// empty or whitespace-only — the hook's "I had nothing to report"
/// signal — so drift detection is skipped entirely. Returns `Err` only
/// for unreadable sources or malformed JSON; the caller surfaces that as
/// a hard CLI error so a misconfigured hook fails loudly.
pub fn read_observed(source: &str) -> Result<Option<Observed>> {
    let raw = if source == "-" {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .context("failed to read --observed-from stdin")?;
        buf
    } else {
        fs::read_to_string(source)
            .with_context(|| format!("failed to read --observed-from path: {source}"))?
    };
    parse_observed(&raw)
}

/// Parse the observed-set JSON contract. Returns `Ok(None)` for empty
/// input (no observation provided — skip detection). Returns `Err` for
/// malformed JSON or a schema violation. Exposed for tests; production
/// callers use `read_observed`.
pub fn parse_observed(raw: &str) -> Result<Option<Observed>> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let value: Value = serde_json::from_str(trimmed)
        .context("--observed-from input is not valid JSON")?;
    let obj = value
        .as_object()
        .ok_or_else(|| anyhow!("--observed-from input must be a JSON object with a `paths` array"))?;
    let paths_value = obj
        .get("paths")
        .ok_or_else(|| anyhow!("--observed-from input is missing the `paths` array"))?;
    let docs: Vec<ObservedDoc> = serde_json::from_value(paths_value.clone())
        .context("--observed-from `paths` must be an array of {path, content_hash, size?} objects")?;
    for doc in &docs {
        if doc.path.is_empty() {
            return Err(anyhow!("--observed-from contains an entry with an empty path"));
        }
        if !doc.content_hash.starts_with("sha256:") {
            return Err(anyhow!(
                "--observed-from entry for `{}` has content_hash `{}` — must start with `sha256:`",
                doc.path,
                doc.content_hash
            ));
        }
    }
    Ok(Some(Observed { paths: docs }))
}

/// Compute the drift between `predicted` (paths the context primer just
/// recorded into the log view) and `observed` (paths Claude Code
/// actually injected). Returns `None` when the sets agree — caller skips
/// event emission entirely on no-drift, so the log stays append-only
/// with one event per real divergence.
pub fn detect(predicted: &BTreeSet<String>, observed: &Observed) -> Option<Report> {
    let observed_paths: BTreeSet<String> =
        observed.paths.iter().map(|d| d.path.clone()).collect();
    let missing: Vec<String> = predicted.difference(&observed_paths).cloned().collect();
    let extra: Vec<String> = observed_paths.difference(predicted).cloned().collect();
    if missing.is_empty() && extra.is_empty() {
        return None;
    }
    Some(Report {
        predicted: predicted.iter().cloned().collect(),
        observed: observed_paths.into_iter().collect(),
        missing,
        extra,
    })
}
