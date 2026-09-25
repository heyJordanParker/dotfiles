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
//! Also the shared `facts_by_file` join used by the relations commands
//! (`callers`, `usages`, `defines`, `structure`) to attach each result's
//! `source_file` facts — a render-time join of `file/` facts onto the rows
//! those commands resolve, batched and deduped by path so a file appearing in
//! many rows is resolved once.

use crate::summary::Facts;
use crate::{cache, digest, file_facts, surface};
use serde::ser::{SerializeMap, Serializer};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::file_facts::RESOLVE_CHUNK;
use crate::output::counted;

#[derive(Default)]
pub struct Match {
    pub file: String,
    pub line: i64,
    /// The matched line; every line of a match that spans several (`-U`).
    pub snippet: String,
    /// `-C`: the lines just before and just after the match.
    pub before: Vec<String>,
    pub after: Vec<String>,
}

/// One file's enrichment, held once however many matches the file has: its
/// facts, and the nearest doc that governs it.
pub struct FileEnrichment {
    facts: Option<Facts>,
    nearest_doc: Option<String>,
    in_repository: bool,
}

impl FileEnrichment {
    /// The file's line in a list: its headline facts, how many matches it
    /// holds, and its nearest doc.
    fn headline(&self, matches: usize) -> String {
        let mut map = self.facts.as_ref().map(Facts::headline_map).unwrap_or_default();
        map.insert("matches".into(), matches.into());
        if let Some(doc) = &self.nearest_doc {
            map.insert("nearest_doc".into(), doc.clone().into());
        }
        crate::yamlfmt::flow(&Value::Object(map), false)
    }

    fn imported_by(&self) -> i64 {
        self.facts.as_ref().and_then(|facts| facts.imported_by).unwrap_or(0) as i64
    }
}

impl Serialize for FileEnrichment {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut facts = self.facts.as_ref().map(Facts::to_map).unwrap_or_default();
        // Outside any git repository there is no git state to describe.
        if !self.in_repository {
            facts.remove("git");
        }
        let mut map = s.serialize_map(Some(facts.len() + 1))?;
        for (key, value) in &facts {
            map.serialize_entry(key, value)?;
        }
        map.serialize_entry("nearest_doc", &self.nearest_doc)?;
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
    file: Arc<FileEnrichment>,
    declaration: Option<surface::Row>,
    type_row: Option<surface::Row>,
}

impl Serialize for EnrichedMatch<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(None)?;
        map.serialize_entry("file", &self.m.file)?;
        map.serialize_entry("line", &self.m.line)?;
        map.serialize_entry("snippet", &self.m.snippet)?;
        if !self.m.before.is_empty() || !self.m.after.is_empty() {
            map.serialize_entry("before", &self.m.before)?;
            map.serialize_entry("after", &self.m.after)?;
        }
        map.serialize_entry("declaration", &self.declaration)?;
        map.serialize_entry("type", &self.type_row)?;
        map.end()
    }
}

/// The `context` slot of a search document: every matched file's enrichment,
/// keyed by the same path the rows carry, plus the repo-wide complexity
/// figures that calibrate read depth.
pub struct SearchContext<'a> {
    pub files: &'a BTreeMap<String, Arc<FileEnrichment>>,
    pub repo: &'a Value,
    /// The graph command that answers the question whole, when the searched
    /// word is a name the graph knows. See `signpost`.
    pub signpost: Option<String>,
}

/// The per-file map, written through the shared handles rather than cloned:
/// the handle keeps one file's enrichment single-copy across all its matches
/// while allowing the completed map to cross a Rayon worker boundary.
struct FileMap<'a>(&'a BTreeMap<String, Arc<FileEnrichment>>);

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

/// Enrich matches with each file's facts and nearest doc.
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
///
/// Also returns each matched file's declaration rows, which the human render
/// groups the matches under.
pub fn enrich<'a>(
    matches: &'a [Match],
    repo_root: &Path,
    relations: Option<&crate::relations::Relations>,
    at: Option<&str>,
) -> (
    Vec<EnrichedMatch<'a>>,
    BTreeMap<String, Arc<FileEnrichment>>,
    BTreeMap<String, Vec<surface::Row>>,
) {
    let mut ordered: Vec<&Match> = matches.iter().collect();
    ordered.sort_by(|a, b| (&a.file, a.line, &a.snippet).cmp(&(&b.file, b.line, &b.snippet)));

    let mut unique: Vec<&str> = ordered.iter().map(|m| m.file.as_str()).collect();
    unique.dedup();
    let abs: Vec<PathBuf> = unique
        .iter()
        .map(|f| cache::absolutize(Path::new(f)))
        .collect();
    // Resolve in chunks, projecting each file's facts to what the enrichment
    // renders and dropping the facts with the chunk.
    let mut by_file: BTreeMap<String, Arc<FileEnrichment>> = BTreeMap::new();
    let mut surfaces: BTreeMap<String, Vec<surface::Row>> = BTreeMap::new();
    let in_repository = cache::worktree_root_for(repo_root).is_some();
    for (names, paths) in unique.chunks(RESOLVE_CHUNK).zip(abs.chunks(RESOLVE_CHUNK)) {
        let facts_map = file_facts::get_batch(paths, repo_root);
        for (name, path) in names.iter().zip(paths.iter()) {
            let (_, key) = file_facts::resolve_under_root(path, repo_root);
            let facts = facts_map.get(&key);
            let surface = match at {
                None => facts
                    .map(|facts| surface::rows(facts, None))
                    .unwrap_or_default(),
                Some(revision) => surface::rows_at(repo_root, revision, name.trim_start_matches("./")),
            };
            surfaces.insert(name.to_string(), surface);
            let graph = relations.and_then(|relations| {
                relations.module_counts(&key).or_else(|| {
                    path.canonicalize().ok().and_then(|canonical| {
                        relations.module_counts(&cache::relative_to_root(&canonical, repo_root))
                    })
                })
            });
            by_file.insert(
                name.to_string(),
                Arc::new(FileEnrichment {
                    facts: facts.map(|facts| Facts::of(facts, graph.as_ref())),
                    nearest_doc: digest::nearest_doc(path, repo_root),
                    in_repository,
                }),
            );
        }
    }

    let enriched = ordered
        .into_iter()
        .map(|m| {
            let (declaration, type_row) = surfaces
                .get(m.file.as_str())
                .map(|rows| surface::enclosing(rows, m.line))
                .map(|(declaration, type_row)| (declaration.cloned(), type_row.cloned()))
                .unwrap_or((None, None));
            EnrichedMatch {
                m,
                file: Arc::clone(&by_file[m.file.as_str()]),
                declaration,
                type_row,
            }
        })
        .collect();
    (enriched, by_file, surfaces)
}

/// Shared human renderer for `grep` and `pattern`, in `git grep
/// --show-function`'s shape: each file's header once, then every declaration
/// that encloses a match once, its match lines (`L<n>:`) and `-C` context
/// lines (`L<n>-`) under it. `files_only` (`-l`, `-c`) stops at each file's
/// header, which carries its match count.
pub fn render_human(
    enriched: &[EnrichedMatch],
    files: &BTreeMap<String, Arc<FileEnrichment>>,
    surfaces: &BTreeMap<String, Vec<surface::Row>>,
    signpost: Option<&str>,
    files_only: bool,
) {
    if enriched.is_empty() {
        println!("(no matches)");
        if let Some(line) = signpost {
            println!("{line}");
        }
        return;
    }
    // One entry per file, whole down to its path: the budget shortens the
    // files the fewest others import first, and names every file. A file's
    // block of lines is set off by a blank line; a one-line file is not.
    let mut entries: Vec<crate::output::Entry> = Vec::new();
    let mut paths: Vec<&str> = Vec::new();
    for group in enriched.chunk_by(|a, b| a.m.file == b.m.file) {
        let file = group[0].m.file.as_str();
        let header = format!("{file}  {}", group[0].file.headline(group.len()));
        let levels = if files_only {
            vec![header, file.to_string()]
        } else {
            let rows = surfaces.get(file).map_or(&[][..], Vec::as_slice);
            vec![format!("\n{header}{}", grouped_lines(group, rows, file)), header, file.to_string()]
        };
        entries.push(crate::output::Entry {
            rank: group[0].file.imported_by(),
            levels,
        });
        paths.push(file);
    }
    let footer = format!(
        "\n{} in {}",
        counted(enriched.len(), "match", "matches"),
        counted(files.len(), "file", "files"),
    );
    let fixed = footer.len() + 1 + signpost.map_or(0, |line| line.len() + 1) + crate::output::closing_room(entries.len(), "files");
    let (texts, shortened) = crate::output::fit_listing(&entries, &paths, fixed);
    for text in texts {
        println!("{text}");
    }
    println!("{footer}");
    if let Some(line) = signpost {
        println!("{line}");
    }
    if shortened > 0 {
        println!("{}", crate::output::shortened_line(shortened, entries.len(), "files"));
    }
}

/// One file's lines under the declarations that enclose them: each such
/// declaration once, a match on its own line standing in for it, every
/// matched line marked `:` and every context line `-`, as grep marks them.
fn grouped_lines(group: &[EnrichedMatch], rows: &[surface::Row], file: &str) -> String {
    // Source line → marker and text; a match outranks context on a line.
    let mut lines: BTreeMap<i64, (char, &str)> = BTreeMap::new();
    for found in group {
        for (text, line) in found.m.snippet.split('\n').zip(found.m.line..) {
            lines.insert(line, (':', text.trim_end()));
        }
    }
    for found in group {
        let after = found.m.line + found.m.snippet.split('\n').count() as i64;
        let before = found.m.line - found.m.before.len() as i64;
        let context = found.m.before.iter().zip(before..).chain(found.m.after.iter().zip(after..));
        for (text, line) in context {
            lines.entry(line).or_insert(('-', text.trim_end()));
        }
    }
    let mut shown: Vec<usize> = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        let encloses = group
            .iter()
            .any(|found| row.header_line <= found.m.line && found.m.line <= row.end_line);
        let repeat = shown
            .last()
            .is_some_and(|&last| rows[last].line == row.line && rows[last].header == row.header);
        if encloses && !repeat {
            shown.push(index);
        }
    }
    // (line, order, depth, marker, text): a declaration sorts before the
    // lines that share its line number, and after its parent.
    let mut items: Vec<(i64, usize, usize, char, String)> = Vec::new();
    for (order, &index) in shown.iter().enumerate() {
        let row = &rows[index];
        if !matches!(lines.get(&row.line), Some((':', _))) {
            lines.remove(&row.line);
            items.push((row.line, order, surface::depth(index, rows), ' ', surface::inline(row, file)));
        }
    }
    let mut source: Vec<(i64, Option<usize>, usize, char, &str)> = Vec::new();
    let mut least: HashMap<Option<usize>, usize> = HashMap::new();
    for (line, (marker, text)) in lines {
        let enclosing: Vec<usize> = shown
            .iter()
            .copied()
            .filter(|&index| rows[index].line < line && line <= rows[index].end_line)
            .collect();
        let owner = enclosing.last().copied();
        let code = text.trim_start_matches([' ', '\t']);
        if !code.is_empty() {
            let indent = least.entry(owner).or_insert(usize::MAX);
            *indent = (*indent).min(text.len() - code.len());
        }
        source.push((line, owner, enclosing.len(), marker, text));
    }
    for (line, owner, depth, marker, text) in source {
        let cut = least.get(&owner).copied().unwrap_or(0).min(text.len());
        items.push((line, usize::MAX, depth, marker, text[cut..].to_string()));
    }
    items.sort_by_key(|(line, order, ..)| (*line, *order));
    items
        .into_iter()
        .map(|(line, _, depth, marker, text)| {
            format!("\n  {:<6}{}{text}", format!("L{line}{marker}"), "  ".repeat(depth))
        })
        .collect()
}

pub struct Section {
    pub heading: String,
    pub files: Vec<(String, Vec<String>, usize)>,
}

pub fn render_sections(sections: &[Section], facts: &HashMap<String, Facts>, one: &str, many: &str) {
    let entries: Vec<Vec<crate::output::Entry>> = sections
        .iter()
        .map(|section| {
            section
                .files
                .iter()
                .map(|(file, rows, count)| {
                    let head = format!("    {file}");
                    let headline = facts.get(file).map_or(String::new(), |facts| format!("  {}", facts.headline()));
                    crate::output::Entry {
                        rank: facts.get(file).and_then(|facts| facts.imported_by).unwrap_or(0) as i64,
                        levels: vec![
                            format!("{head}{headline}\n{}", rows.join("\n")),
                            format!("{head}  {}", counted(*count, one, many)),
                        ],
                    }
                })
                .collect()
        })
        .collect();
    let whole: usize = sections
        .iter()
        .zip(&entries)
        .map(|(section, entries)| section.heading.len() + 1 + entries.iter().map(|entry| entry.levels[0].len() + 1).sum::<usize>())
        .sum();
    let closing = crate::output::closing_room(entries.iter().map(Vec::len).sum(), "files");
    let share = crate::output::budget()
        .filter(|&budget| whole + closing > budget)
        .map(|budget| budget.saturating_sub(closing) / sections.len().max(1));
    let mut shortened = 0;
    let mut files = 0;
    for (section, entries) in sections.iter().zip(&entries) {
        println!("{}", section.heading);
        let paths: Vec<&str> = section.files.iter().map(|(file, _, _)| file.as_str()).collect();
        let fixed = match (crate::output::budget(), share) {
            (Some(budget), Some(share)) => budget - share + section.heading.len() + 1,
            _ => section.heading.len() + 1,
        };
        let (texts, cut) = crate::output::fit_listing(entries, &paths, fixed);
        for text in texts {
            println!("{text}");
        }
        shortened += cut;
        files += entries.len();
    }
    if shortened > 0 {
        println!("{}", crate::output::shortened_line(shortened, files, "files"));
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
    // The index answers this without resolving call sites: how many places
    // declare the word, and how many files mention it.
    let index = crate::relations::get(repo_root);
    if !index.knows(word) {
        return None;
    }
    let mentioning_files = index.used_in(word).count();
    if mentioning_files == 0 {
        return None;
    }
    // The kinds live in the declaring files' extraction, not the index, so
    // they cost the declaring files only — one or two, never the repo.
    let declarations = crate::relations::declarations(word, repo_root);
    let mut kinds: Vec<&str> = Vec::new();
    for candidate in &declarations {
        if !kinds.contains(&candidate.declaration.kind.as_str()) {
            kinds.push(&candidate.declaration.kind);
        }
    }
    let kinds = if kinds.is_empty() {
        String::new()
    } else {
        format!(" ({})", kinds.join(", "))
    };
    Some(format!(
        "{word}: {}{kinds} \u{00b7} mentioned in {} \u{2192} trace callers {word}",
        counted(declarations.len(), "definition", "definitions"),
        counted(mentioning_files, "file", "files"),
    ))
}

/// The searched word when the search term is a bare name, so a signpost is
/// even possible: a regex or a phrase names no symbol.
pub fn searched_name(term: &str) -> Option<&str> {
    let bare = term.chars().all(|c| c.is_alphanumeric() || c == '_')
        && term.len() > 1
        && !term.chars().next().is_some_and(|c| c.is_numeric());
    bare.then_some(term)
}

pub fn facts_of(loaded: &HashMap<String, file_facts::FileFacts>, repo_root: &Path) -> HashMap<String, Facts> {
    let relations = crate::relations::get(repo_root);
    loaded
        .iter()
        .map(|(rel, facts)| (rel.clone(), Facts::of(facts, relations.module_counts(rel).as_ref())))
        .collect()
}

/// The same join as the document's `context.files` slot: each file's facts
/// under the keys its front matter uses.
pub fn facts_context(facts: &HashMap<String, Facts>) -> Value {
    let mut out = serde_json::Map::with_capacity(facts.len());
    for (path, facts) in facts {
        out.insert(path.clone(), Value::Object(facts.to_map()));
    }
    Value::Object(out)
}
