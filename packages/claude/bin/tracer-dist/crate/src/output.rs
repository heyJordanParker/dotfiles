//! Central output for value-producing commands.
//!
//! Every command that has a JSON form returns its `serde_json::Value` to
//! the top level instead of printing it. This module owns two top-level
//! steps:
//!
//! - `guard` runs *before* the command, so an invalid `--filter`/`--json`
//!   combination fails fast with no wasted work or stray human output.
//! - `emit` runs *after*, and is the single place that decides stdout:
//!   filtered jq results, the stable `jsonfmt` JSON, or nothing (the
//!   command already rendered its human text).
//!
//! `--filter` requires `--json` explicitly — it is never implied. Commands
//! with no JSON form (`doctor`, `cache build`, `cache clear`, `context`)
//! call `guard` with `as_json = false`, so `--filter` against them fails
//! with the same message.

use anyhow::{bail, Result};
use serde_json::Value;

/// Validate the `--filter`/`--json` combination before the command runs.
/// `--filter` operates on JSON, so it requires `--json`.
pub fn guard(as_json: bool, filter: Option<&str>) -> Result<()> {
    if filter.is_some() && !as_json {
        bail!("--filter requires --json");
    }
    Ok(())
}

/// Top-level lifecycle for a value-producing command: validate the
/// `--filter`/`--json` combination, run the command, then emit. The guard
/// runs before `command`, so a misuse fails fast with no stray output.
pub fn run_value(
    as_json: bool,
    filter: Option<&str>,
    command: impl FnOnce() -> Result<Value>,
) -> Result<()> {
    guard(as_json, filter)?;
    let value = command()?;
    emit(&value, as_json, filter)
}

/// Decide stdout for a value-producing command. Assumes `guard` already
/// passed: if a filter is present, `as_json` is true.
pub fn emit(value: &Value, as_json: bool, filter: Option<&str>) -> Result<()> {
    if let Some(program) = filter {
        for result in crate::filter::apply(value, program)? {
            println!("{}", crate::jsonfmt::to_pretty(&result));
        }
        return Ok(());
    }
    if as_json {
        println!("{}", crate::jsonfmt::to_pretty(value));
    }
    Ok(())
}
