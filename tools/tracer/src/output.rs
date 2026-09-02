//! Central output for value-producing commands.
//!
//! This module owns two top-level steps:
//!
//! - `guard` runs *before* the command, so an invalid `--filter`/`--json`
//!   combination fails fast with no wasted work or stray human output.
//! - `Sink::emit` runs *after*, and is the single place that decides stdout:
//!   filtered jq results, the stable `jsonfmt` JSON, or nothing (the
//!   command already rendered its human text).
//!
//! A command reaches `Sink::emit` one of two ways, and they are the same
//! path, not two policies. `run_value` is for a command whose result is
//! naturally one `serde_json::Value`; `run_streamed` hands the command the
//! `Sink` so it can emit borrowed structures at the point they are alive,
//! without a `Value` tree in between. `run_value` is defined in terms of
//! `run_streamed`, so there is exactly one emit policy and one byte format
//! path for every command.
//!
//! Nothing is rendered to an intermediate `String`. `jsonfmt::write_pretty`
//! serializes straight into a buffered stdout, so a large result does not
//! exist a second time in memory just to be printed.
//!
//! `--filter` requires `--json` explicitly — it is never implied. Commands
//! with no JSON form (`doctor`, `cache build`, `cache clear`, `context`)
//! call `guard` with `as_json = false`, so `--filter` against them fails
//! with the same message.

use anyhow::{bail, Result};
use serde_json::Value;
use std::io::Write;

/// The one document shape. Every `--json` result carries the same four
/// slots, so an agent learns one shape instead of nine names for "the rows"
/// and ten for "how many", and a `--filter` that projects rows can never
/// reach the enrichment — it lives outside them.
///
///   query    what was asked
///   context  the enrichment, keyed so a row projection cannot take it
///   results  the rows
///   counts   how many
pub fn document(query: Value, context: Value, results: Value, counts: Value) -> Value {
    serde_json::json!({
        "query": query,
        "context": context,
        "results": results,
        "counts": counts,
    })
}

/// Validate the `--filter`/`--json` combination before the command runs.
/// `--filter` operates on JSON, so it requires `--json`.
pub fn guard(as_json: bool, filter: Option<&str>) -> Result<()> {
    if filter.is_some() && !as_json {
        bail!("--filter requires --json");
    }
    Ok(())
}

/// Top-level lifecycle for a command that emits its own document: validate
/// the `--filter`/`--json` combination, then run the command with the sink
/// it emits through. The guard runs first, so a misuse fails fast with no
/// stray output.
pub fn run_streamed(
    as_json: bool,
    filter: Option<&str>,
    command: impl FnOnce(&Sink) -> Result<()>,
) -> Result<()> {
    guard(as_json, filter)?;
    command(&Sink { as_json, filter })
}

/// The lifecycle for a command whose result is one `Value`. `Value` is
/// `Serialize` like any other document, so this is `run_streamed` with the
/// command's return value handed straight to the same sink — not a second
/// way to emit.
pub fn run_value(
    as_json: bool,
    filter: Option<&str>,
    command: impl FnOnce() -> Result<Value>,
) -> Result<()> {
    run_streamed(as_json, filter, |sink| sink.emit(&command()?))
}

/// Wrap the agent's jq program so its output arrives beside the document's
/// `context` slot instead of in place of it.
///
/// The whole point of tracer is that a repository fact never reaches an agent
/// stripped of its context, and `--filter '.results'` is the one flag that
/// could strip it: in the recorded transcripts, 598 of 632 `grep --filter`
/// expressions projected rows and dropped the enrichment with them. Composing
/// the program instead of post-processing its output keeps this to one jaq
/// parse of the document, which on a 60,000-match search is the run's
/// dominant cost.
fn keeping_context(program: &str) -> String {
    // A jq program is a stream, so its outputs are collected into one array.
    // A program that produced exactly one value is unwrapped again, so
    // `--filter '.results'` reads as the rows themselves rather than a list
    // holding the rows, while `--filter '.results[]'` still reads as a list.
    format!(
        "{{\"context\": .context, \"results\": ([{program}] | if length == 1 then .[0] else . end)}}"
    )
}

pub struct Sink<'a> {
    as_json: bool,
    filter: Option<&'a str>,
}

impl Sink<'_> {
    /// The one place stdout is decided. Nothing here holds the document a
    /// second time: it is serialized once, straight into the writer, whether
    /// that writer is stdout or the buffer jaq parses.
    pub fn emit<T: serde::Serialize + ?Sized>(&self, document: &T) -> Result<()> {
        if !self.as_json {
            return Ok(());
        }
        let stdout = std::io::stdout();
        let mut w = std::io::BufWriter::with_capacity(64 * 1024, stdout.lock());
        let Some(program) = self.filter else {
            crate::jsonfmt::write_pretty(&mut w, document)?;
            w.write_all(b"\n")?;
            return Ok(w.flush()?);
        };
        // jaq builds its own tree from these bytes. Handing `filter::apply` a
        // `serde_json::Value` instead would build a whole second tree of the
        // document that nothing reads — on a large result, the biggest
        // allocation in the run.
        let mut json = Vec::new();
        crate::jsonfmt::write_pretty(&mut json, document)?;
        let results = crate::filter::apply(&json, &keeping_context(program))?;
        drop(json);
        for result in results {
            crate::jsonfmt::write_pretty(&mut w, &result)?;
            w.write_all(b"\n")?;
        }
        Ok(w.flush()?)
    }
}
