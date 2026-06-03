//! Single place that prints a command's JSON value to stdout.
//!
//! The review envelope is this tool's contract: the model's review text plus
//! run metadata, always emitted as formatted JSON. `doctor` and `download` are
//! human-output commands and print their own text directly.

use anyhow::Result;
use serde_json::Value;

/// Print a value as formatted (pretty) JSON — the review envelope's only
/// output form.
pub fn emit(value: &Value) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
