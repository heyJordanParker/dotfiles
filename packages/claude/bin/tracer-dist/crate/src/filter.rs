//! `--filter` — an in-process jq program over a command's JSON value.
//!
//! The agent must never pipe `trace` output into `jq` (or any text
//! trimmer); the `guard_trace.py` hook enforces that. This module is the
//! in-binary replacement: `jaq` (a pure-Rust jq) runs the program over the
//! command's JSON value, keeping `trace` a single static binary with no
//! external `jq` dependency. Output goes back through `jsonfmt` so the
//! filtered bytes share the one stable wire format.

use anyhow::{bail, Result};
use jaq_core::load::{Arena, File, Loader};
use jaq_core::{data, unwrap_valr, Ctx, Vars};
use jaq_json::{read, Val};
use serde_json::Value;

/// Run `program` (jq syntax) over `value`, returning every produced value
/// in stream order. A parse/compile/runtime error fails loud with the jq
/// diagnostic — never a partial or silent result.
pub fn apply(value: &Value, program: &str) -> Result<Vec<Value>> {
    let json = serde_json::to_string(value)?;
    let input = match read::parse_single(&json.as_bytes()) {
        Ok(v) => v,
        Err(e) => bail!("--filter: could not read command output as JSON: {e:?}"),
    };

    let file = File {
        code: program,
        path: (),
    };
    let defs = jaq_core::defs()
        .chain(jaq_std::defs())
        .chain(jaq_json::defs());
    let funs = jaq_core::funs()
        .chain(jaq_std::funs())
        .chain(jaq_json::funs());

    let loader = Loader::new(defs);
    let arena = Arena::default();
    let modules = match loader.load(&arena, file) {
        Ok(m) => m,
        Err(e) => bail!("--filter: invalid jq program: {e:?}"),
    };
    let filter = match jaq_core::Compiler::default().with_funs(funs).compile(modules) {
        Ok(f) => f,
        Err(e) => bail!("--filter: invalid jq program: {e:?}"),
    };

    let ctx = Ctx::<data::JustLut<Val>>::new(&filter.lut, Vars::new([]));
    let mut out = Vec::new();
    for result in filter.id.run((ctx, input)).map(unwrap_valr) {
        match result {
            Ok(v) => {
                let rendered = v.to_string();
                let parsed: Value = serde_json::from_str(&rendered).map_err(|e| {
                    anyhow::anyhow!("--filter: result was not valid JSON: {e}")
                })?;
                out.push(parsed);
            }
            Err(e) => bail!("--filter: jq runtime error: {e:?}"),
        }
    }
    Ok(out)
}
