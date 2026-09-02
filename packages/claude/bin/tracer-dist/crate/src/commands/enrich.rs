//! Per-match file enrichment shared by `grep` and `pattern`.
//! file_complexity, git_context, and the `nearest_doc` walk (in
//! `crate::digest`).
//!
//! Enrichment is per FILE, so it is stored once per file and referenced by
//! every match in it. An `EnrichedMatch` is a borrowed match plus a shared
//! handle to its file's enrichment; its `Serialize` writes the same seven
//! keys, in the same order, that the emitted document has always carried, so
//! every match still arrives with its full context. Storing it per match
//! instead cost 168 MB on a 59,644-match search where the enrichment itself
//! is 2,365 files' worth.
//!
//! Files resolve through `file_facts::get_batch`, which `file_facts.rs` names
//! the only correct path for a multi-file command: the git map, the scc map,
//! and the mtime index are hoisted once instead of being re-read per file.
//! The batch is chunked and projected to the fields the enrichment renders,
//! so the whole extraction set for every matched file is never resident.
//!
//! Also the shared `file_shoulders` join used by the relations commands
//! (`callers`, `usages`, `defines`, `structure`) to attach the canonical
//! passive-context shoulder to each result's `source_file` — a render-time
//! join of `file/` facts onto the rows those commands resolve, batched and
//! deduped by path so a file appearing in many rows is resolved once.

use crate::{cache, digest, file_facts, passive_context};
use serde::ser::{SerializeMap, Serializer};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crate::file_facts::RESOLVE_CHUNK;

pub struct Match {
    pub file: String,
    pub line: i64,
    pub snippet: String,
}

/// One file's enrichment, held once however many matches the file has.
pub struct FileEnrichment {
    file_complexity: Value,
    nearest_doc: Value,
    git: Value,
    shoulder: Value,
}

impl FileEnrichment {
    fn nearest_doc_str(&self) -> Option<&str> {
        self.nearest_doc.as_str()
    }
    fn shoulder_str(&self) -> Option<&str> {
        self.shoulder.as_str()
    }
}

impl Serialize for FileEnrichment {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(4))?;
        map.serialize_entry("file_complexity", &self.file_complexity)?;
        map.serialize_entry("nearest_doc", &self.nearest_doc)?;
        map.serialize_entry("git", &self.git)?;
        map.serialize_entry("shoulder", &self.shoulder)?;
        map.end()
    }
}

/// A match plus a shared handle to its file's enrichment.
///
/// The row serializes as the match alone. Its file's enrichment lives in the
/// document's `context` slot, keyed by path: a `--filter` that projects rows
/// cannot take the context with it, and a file matched a thousand times
/// carries its enrichment once instead of a thousand times.
pub struct EnrichedMatch<'a> {
    m: &'a Match,
    file: Rc<FileEnrichment>,
}

impl Serialize for EnrichedMatch<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(3))?;
        map.serialize_entry("file", &self.m.file)?;
        map.serialize_entry("line", &self.m.line)?;
        map.serialize_entry("snippet", &self.m.snippet)?;
        map.end()
    }
}

/// The `context` slot of a search document: every matched file's enrichment,
/// keyed by the same path the rows carry, plus the repo-wide complexity
/// figures that calibrate read depth.
pub struct SearchContext<'a> {
    pub files: &'a BTreeMap<String, Rc<FileEnrichment>>,
    pub repo: &'a Value,
    /// The graph command that answers the question whole, when the searched
    /// word is a name the graph knows. See `signpost`.
    pub signpost: Option<String>,
}

/// The per-file map, written through the shared handles rather than cloned:
/// `Rc` is only `Serialize` under serde's `rc` feature, and the handle is
/// what keeps one file's enrichment single-copy across all its matches.
struct FileMap<'a>(&'a BTreeMap<String, Rc<FileEnrichment>>);

impl Serialize for FileMap<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(self.0.len()))?;
        for (path, enrichment) in self.0 {
            map.serialize_entry(path, &**enrichment)?;
        }
        map.end()
    }
}

impl Serialize for SearchContext<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(3))?;
        map.serialize_entry("files", &FileMap(self.files))?;
        map.serialize_entry("repo", self.repo)?;
        map.serialize_entry("signpost", &self.signpost)?;
        map.end()
    }
}

/// The one document both searches emit: same four slots, same key order.
pub struct SearchDocument<'a> {
    pub query: Value,
    pub context: SearchContext<'a>,
    pub results: &'a [EnrichedMatch<'a>],
    /// An empty result over a base that contains nested checkouts is a scope
    /// fact, not an absence fact — named so the next call is scoped inside.
    pub nested_repos: &'a [String],
}

impl Serialize for SearchDocument<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(Some(4))?;
        map.serialize_entry("query", &self.query)?;
        map.serialize_entry("context", &self.context)?;
        map.serialize_entry("results", self.results)?;
        map.serialize_entry(
            "counts",
            &json!({
                "matches": self.results.len(),
                "files": self.context.files.len(),
                "nested_repos": self.nested_repos,
            }),
        )?;
        map.end()
    }
}

/// Per-file complexity scalars from cached facts.
fn file_complexity(facts: Option<&file_facts::FileFacts>) -> Value {
    match facts {
        None => json!({
            "ccn_total": 0,
            "ccn_max_function": 0,
            "loc": 0,
            "rank": "unknown",
        }),
        Some(f) => json!({
            "ccn_total": f.cyclomatic_complexity_total,
            "ccn_max_function": f.cyclomatic_complexity_max,
            "loc": f.loc,
            "rank": f.rank,
        }),
    }
}

/// Git context for a file: last commit, author, 30-day commit count.
fn git_context(facts: Option<&file_facts::FileFacts>) -> Value {
    match facts {
        None => json!({
            "last_modified": Value::Null,
            "last_author": Value::Null,
            "commits_30d": 0,
        }),
        Some(f) => json!({
            "last_modified": f.last_modified,
            "last_author": f.last_author,
            "commits_30d": f.commits_30d,
        }),
    }
}

/// Enrich matches with per-file complexity, nearest doc, and git context.
/// `repo_root` is resolved once by the caller for the search path — every
/// match lives under it, so per-file root resolution is correct without
/// paying a `git rev-parse` per match.
///
/// Matches are sorted by `(file, line, snippet)` before enrichment so the
/// emitted order is byte-identical across repeated identical invocations.
/// The underlying search backends (`rg --json`, `sg run --json`) walk files
/// in parallel and emit per-file blocks in nondeterministic order; without
/// this sort the same query returns the same matches in a different order
/// each run, which breaks output diffing and caching for any consumer.
pub fn enrich<'a>(
    matches: &'a [Match],
    repo_root: &Path,
) -> (Vec<EnrichedMatch<'a>>, BTreeMap<String, Rc<FileEnrichment>>) {
    let mut ordered: Vec<&Match> = matches.iter().collect();
    ordered.sort_by(|a, b| {
        (&a.file, a.line, &a.snippet).cmp(&(&b.file, b.line, &b.snippet))
    });

    let mut unique: Vec<&str> = ordered.iter().map(|m| m.file.as_str()).collect();
    unique.dedup();
    let abs: Vec<PathBuf> = unique
        .iter()
        .map(|f| cache::absolutize(Path::new(f)))
        .collect();

    // Resolve in chunks, projecting each file's facts to what the enrichment
    // renders and dropping the facts with the chunk.
    let mut by_file: BTreeMap<String, Rc<FileEnrichment>> = BTreeMap::new();
    for (names, paths) in unique
        .chunks(RESOLVE_CHUNK)
        .zip(abs.chunks(RESOLVE_CHUNK))
    {
        let facts_map = file_facts::get_batch(paths, repo_root);
        for (name, path) in names.iter().zip(paths.iter()) {
            let facts = facts_map.get(&cache::relative_to_root(path, repo_root));
            by_file.insert(
                name.to_string(),
                Rc::new(FileEnrichment {
                    file_complexity: file_complexity(facts),
                    nearest_doc: json!(digest::nearest_doc(path)),
                    git: git_context(facts),
                    shoulder: json!(facts.map(|f| passive_context::render(f, None))),
                }),
            );
        }
    }

    let enriched = ordered
        .into_iter()
        .map(|m| EnrichedMatch {
            m,
            file: Rc::clone(&by_file[m.file.as_str()]),
        })
        .collect();
    (enriched, by_file)
}

/// Shared human renderer for grep/struct: grouped-by-file with a per-file
/// shoulder, then a one-line summary.
pub fn render_human(
    enriched: &[EnrichedMatch],
    files: &BTreeMap<String, Rc<FileEnrichment>>,
    repo_ctx: &Value,
    signpost: Option<&str>,
) {
    if enriched.is_empty() {
        println!("(no matches)");
        if let Some(line) = signpost {
            println!("{line}");
        }
        return;
    }
    let mut current_file: Option<&str> = None;
    for m in enriched {
        if current_file != Some(m.m.file.as_str()) {
            let file = &m.m.file;
            let doc = m.file.nearest_doc_str().unwrap_or("(no doc)");
            println!();
            match m.file.shoulder_str() {
                Some(s) => println!("{file}  {s}  [doc={doc}]"),
                None => println!("{file}  [doc={doc}]"),
            }
            current_file = Some(m.m.file.as_str());
        }
        println!("  L{:<5} {}", m.m.line, m.m.snippet);
    }
    println!();
    println!(
        "matches={} files={} repo_p95={}",
        enriched.len(),
        files.len(),
        repo_ctx["complexity_p95"].as_i64().unwrap_or(0),
    );
    if let Some(line) = signpost {
        println!("{line}");
    }
}

/// The one line that turns a partial search into the command that answers
/// the question whole.
///
/// A structural pattern matches one call shape, so `dispatch($$$A)` finds the
/// plain calls and silently misses `$x->dispatch()` and `X::dispatch()`; a
/// text search for the same word finds all three plus every comment and
/// docblock. Both are partial, and the structural one is partial in silence.
/// When the searched word is a name the graph resolved, this names
/// `trace callers <name>`, which answers from the graph with every call shape
/// already resolved. `None` when the word is not a name the graph knows, so
/// an ordinary text search says nothing extra.
pub fn signpost(word: Option<&str>, repo_root: &Path) -> Option<String> {
    let word = word?;
    // The index answers this without loading a file: how many places declare
    // the word, and how many mention it.
    let index = crate::relations::get(repo_root);
    if !index.knows(word) {
        return None;
    }
    let callers = index.used_in(word).count() as i64;
    if callers == 0 {
        return None;
    }
    // The kind lives in the declaring file's extraction, not the index, so
    // it costs the declaring files only — one or two, never the repo.
    let declarations = crate::relations::declarations(word, repo_root);
    let definitions = declarations.len() as i64;
    let kind = declarations
        .first()
        .map(|c| c.declaration.kind.clone())
        .unwrap_or_else(|| "symbol".to_string());
    Some(format!(
        "{word} is a {kind} \u{00b7} {callers} callers \u{00b7} {definitions} definitions \u{2192} trace callers {word}"
    ))
}

/// The searched word when the search term is a bare name, so a signpost is
/// even possible: a regex or a phrase names no symbol.
pub fn searched_name(term: &str) -> Option<&str> {
    let bare = term
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_')
        && term.len() > 1
        && !term.chars().next().is_some_and(|c| c.is_numeric());
    bare.then_some(term)
}

/// Canonical passive-context shoulder per `source_file`, batched and deduped.
/// Maps each unique repo-relative source file in `rel_files` to its shoulder
/// string. Files with no resolvable facts (external nodes, deleted files) are
/// absent from the map, so a caller looks up by path and renders nothing when
/// the entry is missing. The relations commands carry a `source_file` per
/// result row; this lets each result carry the same file-state shoulder the
/// per-file commands emit, without recomputing facts per row.
pub fn file_shoulders(
    rel_files: &[String],
    repo_root: &Path,
) -> HashMap<String, String> {
    let mut unique: Vec<String> = rel_files.to_vec();
    unique.sort();
    unique.dedup();
    let abs: Vec<PathBuf> = unique.iter().map(|r| repo_root.join(r)).collect();
    let facts_map = file_facts::get_batch(&abs, repo_root);
    let mut out = HashMap::with_capacity(unique.len());
    for rel in unique {
        if let Some(f) = facts_map.get(&rel) {
            out.insert(rel, passive_context::render(f, None));
        }
    }
    out
}

/// The same join, rendered as the document's `context.files` slot: one entry
/// per file, `{"shoulder": …}`, which is the shape every command's per-file
/// context carries.
pub fn shoulder_context(shoulders: &HashMap<String, String>) -> Value {
    let mut out = serde_json::Map::with_capacity(shoulders.len());
    for (path, shoulder) in shoulders {
        out.insert(path.clone(), json!({"shoulder": shoulder}));
    }
    Value::Object(out)
}
