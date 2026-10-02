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
use std::collections::HashSet;
use std::io::Write;
use std::sync::OnceLock;

/// `--budget`'s default: the longest Bash result Claude Code shows whole. It
/// saves anything longer to a file and shows the Agent a 2,000-character
/// preview.
pub const DEFAULT_BUDGET: usize = 30_000;

/// The length Claude Code measures text by: JavaScript's, in UTF-16 units.
pub fn width(text: &str) -> usize {
    text.encode_utf16().count()
}

/// The longest prefix of `text` at most `room` wide, cut at a character.
pub fn clip(text: &str, room: usize) -> &str {
    let mut used = 0;
    for (at, c) in text.char_indices() {
        used += c.len_utf16();
        if used > room {
            return &text[..at];
        }
    }
    text
}

static BUDGET: OnceLock<Option<usize>> = OnceLock::new();

/// Set the characters text output fits in, once, from `--budget`; 0 means
/// unbounded.
pub fn set_budget(chars: usize) {
    let _ = BUDGET.set((chars > 0).then_some(chars));
}

thread_local! {
    static SHARE: std::cell::Cell<Option<Option<usize>>> = const { std::cell::Cell::new(None) };
}

/// The characters text output fits in; `None` when unbounded.
pub fn budget() -> Option<usize> {
    SHARE
        .with(std::cell::Cell::get)
        .unwrap_or_else(|| BUDGET.get().copied().unwrap_or(Some(DEFAULT_BUDGET)))
}

pub fn within<T>(share: Option<usize>, run: impl FnOnce() -> T) -> T {
    let before = SHARE.with(|cell| cell.replace(Some(share)));
    let out = run();
    SHARE.with(|cell| cell.set(before));
    out
}

/// One file's entry in a listing of many, its texts from most to least
/// detail; the last is the least any entry is ever cut to.
pub struct Entry {
    /// Higher ranks keep their detail longest.
    pub rank: i64,
    pub levels: Vec<String>,
}

/// Each entry at the most detail the budget allows beside `fixed` other
/// characters. Detail is cut, never coverage: the lowest-ranked entry is cut
/// down one level at a time to its last, then the next lowest, so the
/// highest-ranked entries keep their detail longest and every entry keeps at
/// least its last level. Returns each entry's text in input order, and how
/// many were shortened.
pub fn fit(entries: &[Entry], fixed: usize) -> (Vec<&str>, usize) {
    let mut level = vec![0usize; entries.len()];
    if let Some(budget) = budget() {
        let mut size = fixed + entries.iter().map(|entry| width(&entry.levels[0]) + 1).sum::<usize>();
        let mut by_rank: Vec<usize> = (0..entries.len()).collect();
        by_rank.sort_by_key(|&index| entries[index].rank);
        'entries: for index in by_rank {
            let levels = &entries[index].levels;
            while level[index] + 1 < levels.len() {
                if size <= budget {
                    break 'entries;
                }
                size = size - width(&levels[level[index]]) + width(&levels[level[index] + 1]);
                level[index] += 1;
            }
        }
    }
    let shortened = level.iter().filter(|&&at| at > 0).count();
    let texts = entries
        .iter()
        .zip(&level)
        .map(|(entry, &at)| entry.levels[at].as_str())
        .collect();
    (texts, shortened)
}

/// `fit` for a listing with one entry per path, in the same order; a path
/// ending in `/` is a directory. When even every bare path overruns the
/// budget, the names go one line per directory, `dir/: a.php, b.php`, and past
/// that each directory with its counts, so the listing still says where every
/// entry is.
pub fn fit_listing(entries: &[Entry], paths: &[&str], fixed: usize) -> (Vec<String>, usize) {
    let (texts, shortened) = fit(entries, fixed);
    let size = |lines: &[String]| fixed + lines.iter().map(|line| width(line) + 1).sum::<usize>();
    let texts: Vec<String> = texts.into_iter().map(str::to_string).collect();
    let Some(budget) = budget().filter(|&budget| size(&texts) > budget) else {
        return (texts, shortened);
    };
    let mut directories: Vec<(&str, Vec<&str>)> = Vec::new();
    let mut at: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for path in paths {
        let (directory, name) = match path.trim_end_matches('/').rfind('/') {
            Some(slash) => (&path[..slash], &path[slash + 1..]),
            None => (".", *path),
        };
        let index = *at.entry(directory).or_insert_with(|| {
            directories.push((directory, Vec::new()));
            directories.len() - 1
        });
        directories[index].1.push(name);
    }
    let names: Vec<String> = directories
        .iter()
        .map(|(directory, names)| format!("{directory}/: {}", names.join(", ")))
        .collect();
    if size(&names) <= budget {
        return (names, entries.len());
    }
    let counts = directories
        .iter()
        .map(|(directory, names)| {
            let folders = names.iter().filter(|name| name.ends_with('/')).count();
            let kinds: Vec<String> = [(folders, "directory", "directories"), (names.len() - folders, "file", "files")]
                .into_iter()
                .filter(|(n, _, _)| *n > 0)
                .map(|(n, one, many)| counted(n, one, many))
                .collect();
            format!("{directory}/: {}", kinds.join(", "))
        })
        .collect();
    (counts, entries.len())
}

/// `n` and its noun, singular for one.
pub fn counted(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// The last line of an output the budget shortened: what was cut, and the
/// command that returns it whole.
pub fn shortened_line(shortened: usize, of: usize, unit: &str) -> String {
    let budget = budget().unwrap_or(0);
    format!("[{shortened} of {of} {unit} shortened to fit --budget {budget} — whole: {} --budget 0]", this_command())
}

pub fn closing_room(of: usize, unit: &str) -> usize {
    width(&shortened_line(of, of, unit)) + 1
}

/// This invocation as a shell command, without its own `--budget`.
fn this_command() -> String {
    let mut words = vec!["trace".to_string()];
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        if argument == "--budget" {
            arguments.next();
            continue;
        }
        if argument.starts_with("--budget=") {
            continue;
        }
        let plain = !argument.is_empty()
            && argument
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "-_./:=@,+%".contains(c));
        words.push(if plain {
            argument
        } else {
            format!("'{}'", argument.replace('\'', r"'\''"))
        });
    }
    words.join(" ")
}

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

/// Keep the enrichment of only the files the filtered result names, and of
/// the directories that hold them: `.counts` carries no file's context,
/// `.results[0]` carries its own file's. This runs on the parsed result, not
/// inside the jq program: collecting every string of a 111,062-row result in
/// jq added 1.4 CPU-seconds.
fn narrow_context(output: &mut Value) {
    for (slot, holders) in [("/context/files", false), ("/context/directories", true)] {
        let Some(keys) = output.pointer(slot).and_then(Value::as_object) else {
            continue;
        };
        let mut unnamed: HashSet<String> = keys.keys().cloned().collect();
        if let Some(results) = output.get("results") {
            forget_named(results, &mut unnamed, holders);
        }
        if let Some(keys) = output.pointer_mut(slot).and_then(Value::as_object_mut) {
            keys.retain(|path, _| !unnamed.contains(path));
        }
    }
}

fn forget_named(value: &Value, unnamed: &mut HashSet<String>, holders: bool) {
    if unnamed.is_empty() {
        return;
    }
    match value {
        Value::String(text) => {
            unnamed.remove(text);
            if holders {
                for (slash, _) in text.match_indices('/') {
                    unnamed.remove(&text[..=slash]);
                }
                if !text.contains('/') {
                    unnamed.remove("./");
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|item| forget_named(item, unnamed, holders)),
        Value::Object(map) => map.values().for_each(|item| forget_named(item, unnamed, holders)),
        _ => {}
    }
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
        for mut result in results {
            narrow_context(&mut result);
            crate::jsonfmt::write_pretty(&mut w, &result)?;
            w.write_all(b"\n")?;
        }
        Ok(w.flush()?)
    }
}
