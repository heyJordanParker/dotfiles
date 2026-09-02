//! The two inversions, and the on-demand reference resolver.
//!
//! Per-file entries (`file_facts::FileFacts::extraction`) already hold every
//! declaration, import and reference, content-addressed and invalidated per
//! file. They are the source of truth. Anything answerable from the file in
//! hand therefore needs no stored structure: `file_to_module` is a pure
//! function of a path, so which file a module path names is derivable from
//! the file list alone, and a file's own imports and references are in its
//! own entry.
//!
//! What cannot be derived is the inversion — who references this name, and
//! who imports this file — because computing it means reading every file.
//! Exactly two relations invert, so exactly two maps are stored and nothing
//! else:
//!
//!   symbols    name -> { defined_in: [file], used_in: [file] }
//!   importers  file -> [importing file]
//!
//! Neither carries payload. Line, kind, container and language stay in the
//! per-file entry, because the files holding them are exactly the files an
//! answer already loads.
//!
//! This replaces a materialized graph whose reference edges were 98% of its
//! mass (733,105 of 747,199 on laravel-framework, 261,747 of 262,425 on
//! WordPress) and which cost 194 MB and 0.90 s to decode for a question whose
//! answer was 382 rows drawn from 82 files. Cost is now proportional to the
//! answer instead of to the repository.
//!
//! Storage follows `file_facts`'s `mtime_index_v2__` precedent exactly: one
//! mutable index in the `file/` namespace over immutable content-addressed
//! entries. Each row records the file it came from, so a changed file's rows
//! are filtered out and re-added from its fresh extraction — the repository
//! is never re-derived, and there is no global fingerprint.

use crate::extraction::{Declaration, ExtractionResult, RefShape};
use crate::file_facts::{self, FileFacts};
use crate::{cache, extraction, memo};
use rayon::prelude::*;
use serde::Deserialize;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

pub const CONFIDENCE_EXTRACTED: &str = "EXTRACTED";
pub const CONFIDENCE_INFERRED: &str = "INFERRED";
pub const CONFIDENCE_AMBIGUOUS: &str = "AMBIGUOUS";

/// The two file lists for one symbol name. Both are repo-relative paths in
/// discovery order, so a query's row order is stable across runs.
///
/// A path is held by shared handle, never by value. It appears once per name
/// that mentions it, and spelling each occurrence as its own `String` turned
/// next.js's 8.2 MB index into 172 MB of resident memory before a query ran.
/// The handles all point at the one copy `Relations::intern` keeps.
#[derive(Debug, Default, Clone)]
pub struct SymbolRow {
    pub defined_in: Vec<Arc<str>>,
    pub used_in: Vec<Arc<str>>,
}

/// The stored index: the two inversions plus the provenance that makes an
/// incremental update possible.
///
/// `built_from` is `relative path -> content-hash key`, the exact set the
/// maps were computed from. Comparing it against the current per-file hashes
/// names the files whose rows are stale, which is the whole update: drop
/// those paths from every list, then add what their fresh extraction says.
#[derive(Debug, Default, Clone)]
pub struct Relations {
    symbols: HashMap<String, SymbolRow>,
    importers: HashMap<Arc<str>, Vec<Importer>>,
    built_from: BTreeMap<Arc<str>, Built>,
}

/// What the index knows about a file without opening it: the content key its
/// rows were absorbed from, and its language. The language is stored because
/// every module path is spelled from it, and reading it back out of 3,030
/// per-file entries cost 0.12s on laravel-framework — on `callers`, `usages`,
/// `dependencies`, and the primer alike.
#[derive(Debug, Clone)]
struct Built {
    key: String,
    language: Option<String>,
}

/// The index exactly as it sits on disk, so reading it allocates the rows
/// and nothing else. `to_json` writes this shape.
#[derive(Deserialize)]
struct Stored {
    files: Vec<String>,
    built: Vec<(String, Option<String>)>,
    symbols: HashMap<String, (Vec<u32>, Vec<u32>)>,
    importers: HashMap<String, Vec<(u32, u64, Option<String>)>>,
}

/// One file that imports another, and how surely the import resolved.
///
/// Confidence is the one thing here that is not derivable from the importing
/// file alone: an import whose module path does not resolve but whose symbol
/// does is INFERRED, and deciding that needs the whole symbol map. So it is
/// an inversion fact and it is stored, unlike kind, line and language, which
/// the importing file already carries.
#[derive(Debug, Clone)]
pub struct Importer {
    pub file: Arc<str>,
    pub confidence: String,
    /// The symbol the import named, when it named one. `from util import
    /// helper` depends on `helper`, not merely on `util`, and a row that
    /// says the module loses which part of it is actually depended on.
    /// It is an edge fact, so it rides on the edge.
    pub symbol: Option<String>,
}

impl Relations {
    /// Files declaring `name`, matched case-insensitively the way the
    /// extractors index it.
    pub fn defined_in(&self, name: &str) -> impl Iterator<Item = &str> {
        self.symbols
            .get(&name.to_lowercase())
            .map(|r| r.defined_in.as_slice())
            .unwrap_or(&[])
            .iter()
            .map(|p| &**p)
    }

    /// Files with a use site naming `name`. A candidate set for the
    /// resolver, not an answer: a use site is a match by name only until
    /// `shape_matches` has seen it.
    pub fn used_in(&self, name: &str) -> impl Iterator<Item = &str> {
        self.symbols
            .get(&name.to_lowercase())
            .map(|r| r.used_in.as_slice())
            .unwrap_or(&[])
            .iter()
            .map(|p| &**p)
    }

    /// Files whose imports resolve to `file` — the import inversion.
    pub fn importers_of(&self, file: &str) -> &[Importer] {
        self.importers.get(file).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Every file that imports something, with what it imports. The ranking
    /// queries — the primer Spine, `usages --path`, `dependencies --path` —
    /// walk the whole import graph, and it is 14,094 edges on
    /// laravel-framework and 678 on WordPress, so they walk it directly.
    pub fn import_edges(&self) -> impl Iterator<Item = (&str, &Importer)> {
        self.importers
            .iter()
            .flat_map(|(target, list)| list.iter().map(move |i| (&**target, i)))
    }

    /// How many internal files this file imports. The stored map is the
    /// inversion, so this counts the edges whose importer is this file.
    fn imports_count(&self, file: &str) -> usize {
        self.import_edges()
            .filter(|(_, importer)| &*importer.file == file)
            .count()
    }

    /// The two numbers the shoulder shows for a file: who reaches it, and
    /// what it reaches. One owner — `read`, `context`, `status` and `diff`
    /// each held a copy of this lookup against the graph.
    pub fn module_counts(&self, file: &str) -> Option<Value> {
        if !self.built_from.contains_key(file) {
            return None;
        }
        Some(json!({
            "callers": self.importers_of(file).len(),
            "depended_on_by_modules": self.imports_count(file),
        }))
    }

    /// Every file the index covers, in sorted order. The module-path
    /// resolution both the index build and the resolver use is a pure
    /// function of this list.
    pub fn files(&self) -> impl Iterator<Item = &str> {
        self.built_from.keys().map(|p| &**p)
    }

    /// The one shared handle for a path. Every list that names a file holds a
    /// clone of this handle, so the path's bytes exist once per index.
    fn intern(&mut self, path: &str) -> Arc<str> {
        match self.built_from.get_key_value(path) {
            Some((existing, _)) => Arc::clone(existing),
            None => Arc::from(path),
        }
    }

    /// Every indexed file paired with its language, in sorted order — what
    /// `ModulePaths` is built from, and what a row needs to spell a module
    /// path. Free: the index carries it.
    pub fn listing(&self) -> Vec<(String, Option<String>)> {
        self.built_from
            .iter()
            .map(|(path, built)| (path.to_string(), built.language.clone()))
            .collect()
    }

    /// How many distinct names the index carries.
    pub fn name_count(&self) -> usize {
        self.symbols.len()
    }

    /// Whether the index knows this name at all — the search signpost's
    /// whole question, answered without loading a file.
    pub fn knows(&self, name: &str) -> bool {
        self.symbols.contains_key(&name.to_lowercase())
    }

    fn row_mut(&mut self, name: &str) -> &mut SymbolRow {
        self.symbols.entry(name.to_lowercase()).or_default()
    }

    /// Remove every trace of `file`, so its fresh contribution can be added
    /// without duplicating rows. A name left with no files at all is dropped
    /// rather than kept as an empty row.
    fn forget(&mut self, file: &str) {
        self.symbols.retain(|_, row| {
            row.defined_in.retain(|f| &**f != file);
            row.used_in.retain(|f| &**f != file);
            !(row.defined_in.is_empty() && row.used_in.is_empty())
        });
        for list in self.importers.values_mut() {
            list.retain(|i| &*i.file != file);
        }
        self.importers.retain(|_, list| !list.is_empty());
        self.built_from.remove(file);
    }

    /// Pass one: this file's declarations and use sites into the symbol map.
    /// Every file's declarations must land before any file's imports are
    /// resolved, because an import that misses on module path falls back to
    /// the symbol map — so the two passes cannot be merged.
    fn absorb_symbols(&mut self, facts: &FileFacts, key: &str) {
        let path = self.intern(&facts.path);
        self.built_from.insert(
            Arc::clone(&path),
            Built {
                key: key.to_string(),
                language: facts.language.clone(),
            },
        );
        let extraction = match &facts.extraction {
            Some(e) => e,
            None => return,
        };
        for declaration in &extraction.declarations {
            let row = self.row_mut(&declaration.name);
            if !row.defined_in.contains(&path) {
                row.defined_in.push(Arc::clone(&path));
            }
        }
        for reference in &extraction.references {
            let row = self.row_mut(&reference.name);
            if !row.used_in.contains(&path) {
                row.used_in.push(Arc::clone(&path));
            }
        }
    }

    /// Pass two: this file's resolved imports into the import inversion.
    fn absorb_imports(
        &mut self,
        path: &str,
        language: Option<&str>,
        imports: &[extraction::Import],
        modules: &ModulePaths,
    ) {
        let importer = self.intern(path);
        for (target, confidence, symbol) in
            self.resolve_imports(imports, language, modules)
        {
            let target = self.intern(&target);
            let list = self.importers.entry(target).or_default();
            if !list.iter().any(|i| i.file == importer) {
                list.push(Importer {
                    file: Arc::clone(&importer),
                    confidence,
                    symbol,
                });
            }
        }
    }

    /// The internal files one file's imports resolve to, each with its
    /// confidence. An import naming a package outside the repository
    /// resolves to nothing and is dropped: the inversion answers which file
    /// in this repository imports this file, and an external package is not
    /// one.
    fn resolve_imports(
        &self,
        imports: &[extraction::Import],
        language: Option<&str>,
        modules: &ModulePaths,
    ) -> Vec<(String, String, Option<String>)> {
        let mut out: Vec<(String, String, Option<String>)> = Vec::new();
        for import in imports {
            // `from module import symbol` names the symbol's own file when
            // one exists, and the module otherwise — the dominant Python and
            // TypeScript form, and the reason a from-imported file must not
            // read as having no importers.
            let combined = import.symbol.as_ref().map(|symbol| {
                if language == Some("python") {
                    format!("{}.{}", import.module, symbol)
                } else {
                    format!("{}/{}", import.module, symbol)
                }
            });
            let resolved = combined
                .and_then(|c| modules.resolve(&c, language))
                .or_else(|| modules.resolve(&import.module, language));
            // A module path that resolved is a clean resolution. When it did
            // not, an imported symbol the map knows still names its file:
            // one declaring file is INFERRED, several are AMBIGUOUS.
            let (target, confidence) = match resolved {
                Some(file) => (file, CONFIDENCE_EXTRACTED.to_string()),
                None => {
                    let Some(symbol) = import.symbol.as_ref() else { continue };
                    let declaring: Vec<&str> = self.defined_in(symbol).collect();
                    match declaring.len() {
                        1 => (declaring[0].to_string(), CONFIDENCE_INFERRED.to_string()),
                        n if n > 1 => {
                            (declaring[0].to_string(), CONFIDENCE_AMBIGUOUS.to_string())
                        }
                        _ => continue,
                    }
                }
            };
            // The named symbol only stands as the depended-on thing when the
            // target file actually declares it; otherwise the module is what
            // the import reaches.
            let symbol = import
                .symbol
                .as_ref()
                .filter(|s| self.defined_in(s).any(|f| f == target))
                .cloned();
            // One edge per target file. A language that emits both a module
            // import and a named import for the same statement would
            // otherwise put the importer in the inversion twice and double
            // its direct-edge count. The named symbol wins the slot, because
            // it says what is actually depended on.
            match out.iter_mut().find(|(f, _, _)| f == &target) {
                Some(existing) => {
                    if existing.2.is_none() && symbol.is_some() {
                        existing.1 = confidence;
                        existing.2 = symbol;
                    }
                }
                None => out.push((target, confidence, symbol)),
            }
        }
        out
    }

    /// The stored form, with every path written once.
    ///
    /// A path appears in the index once per name that mentions it. Spelling
    /// each one in full made laravel-framework's entry 7.0 MB, and parsing it
    /// cost 81 MB of resident memory before a single query ran. One path
    /// table plus integer references carries the identical index in 1.1 MB.
    fn to_json(&self) -> Value {
        // `built_from` holds every indexed file, and no row can name a file
        // outside it: rows are absorbed per file and `forget` drops both.
        let paths: Vec<&str> = self.built_from.keys().map(|p| &**p).collect();
        let slot: HashMap<&str, usize> = paths
            .iter()
            .enumerate()
            .map(|(i, p)| (*p, i))
            .collect();
        let at = |p: &str| slot.get(p).copied();

        let built: Vec<Value> = self
            .built_from
            .values()
            .map(|b| json!([b.key, b.language]))
            .collect();

        let mut symbols = Map::with_capacity(self.symbols.len());
        for (name, row) in &self.symbols {
            let d: Vec<usize> = row.defined_in.iter().filter_map(|p| at(p)).collect();
            let u: Vec<usize> = row.used_in.iter().filter_map(|p| at(p)).collect();
            symbols.insert(name.clone(), json!([d, u]));
        }

        let mut importers = Map::with_capacity(self.importers.len());
        for (target, list) in &self.importers {
            let Some(t) = at(target) else { continue };
            let rows: Vec<Value> = list
                .iter()
                .filter_map(|i| {
                    Some(json!([at(&i.file)?, confidence_code(&i.confidence), i.symbol]))
                })
                .collect();
            importers.insert(t.to_string(), Value::Array(rows));
        }

        json!({
            "files": paths,
            "built": built,
            "symbols": Value::Object(symbols),
            "importers": Value::Object(importers),
        })
    }

    fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let stored: Stored = serde_json::from_slice(bytes).ok()?;
        // One handle per path, made once here. Every row below clones a
        // handle rather than the path's bytes, which is the whole reason the
        // index is cheap to hold.
        let files: Vec<Arc<str>> =
            stored.files.into_iter().map(Arc::<str>::from).collect();
        let path_at = |i: u32| files.get(i as usize).map(Arc::clone);
        let paths = |list: Vec<u32>| -> Vec<Arc<str>> {
            list.into_iter().filter_map(path_at).collect()
        };

        let mut out = Relations::default();
        for (i, (key, language)) in stored.built.into_iter().enumerate() {
            let file = Arc::clone(files.get(i)?);
            out.built_from.insert(file, Built { key, language });
        }
        for (name, (defined_in, used_in)) in stored.symbols {
            out.symbols.insert(
                name,
                SymbolRow {
                    defined_in: paths(defined_in),
                    used_in: paths(used_in),
                },
            );
        }
        for (target, list) in stored.importers {
            let Some(target) = target.parse::<u32>().ok().and_then(path_at) else {
                continue;
            };
            let rows: Vec<Importer> = list
                .into_iter()
                .filter_map(|(file, confidence, symbol)| {
                    Some(Importer {
                        file: path_at(file)?,
                        confidence: confidence_name(confidence).to_string(),
                        symbol,
                    })
                })
                .collect();
            out.importers.insert(target, rows);
        }
        Some(out)
    }
}

/// The three confidence classes, stored as their ordinal. Written out in full
/// they were the second-largest thing in the index after the paths.
fn confidence_code(confidence: &str) -> u64 {
    match confidence {
        CONFIDENCE_EXTRACTED => 0,
        CONFIDENCE_INFERRED => 1,
        _ => 2,
    }
}

fn confidence_name(code: u64) -> &'static str {
    match code {
        0 => CONFIDENCE_EXTRACTED,
        1 => CONFIDENCE_INFERRED,
        _ => CONFIDENCE_AMBIGUOUS,
    }
}

/// Module path -> file, derived from the file list and nothing else.
///
/// `file_to_module` is a pure function of a path and its language, so this
/// map is rebuilt from the paths on every call rather than stored. It is what
/// turns a written import (`Illuminate\Support\Str`, `./helpers`) into the
/// repo-relative file it names.
struct ModulePaths {
    /// module path -> relative file, in discovery order.
    exact: HashMap<String, String>,
    ordered: Vec<(String, String)>,
    lowered: Vec<(String, String)>,
}

impl ModulePaths {
    fn new(files: &[(String, Option<String>)]) -> Self {
        let mut exact = HashMap::with_capacity(files.len());
        let mut ordered = Vec::with_capacity(files.len());
        let mut lowered = Vec::with_capacity(files.len());
        for (path, language) in files {
            let module = file_to_module(path, language.as_deref());
            if !exact.contains_key(&module) {
                exact.insert(module.clone(), path.clone());
                lowered.push((module.to_lowercase(), path.clone()));
                ordered.push((module, path.clone()));
            }
        }
        Self { exact, ordered, lowered }
    }

    /// The file an imported module path names: exact hit, else the
    /// language-aware suffix match the graph build used.
    fn resolve(&self, module_path: &str, language: Option<&str>) -> Option<String> {
        if let Some(file) = self.exact.get(module_path) {
            return Some(file.clone());
        }
        if language == Some("php") {
            let slashed = module_path.replace('\\', "/").to_lowercase();
            return self
                .lowered
                .iter()
                .find(|(module, _)| module.ends_with(&slashed))
                .map(|(_, file)| file.clone());
        }
        // `./helpers` and `../helpers` name the same module as `src/helpers`;
        // without stripping the prefix the suffix match never sees past it.
        let normalized = strip_relative_prefix(module_path);
        let dot_suffix = format!(".{normalized}");
        let slash_suffix = format!("/{normalized}");
        self.ordered
            .iter()
            .find(|(module, _)| {
                module.ends_with(&normalized)
                    || module.ends_with(&dot_suffix)
                    || module.ends_with(&slash_suffix)
            })
            .map(|(_, file)| file.clone())
    }

}

/// The addressable id for one declaration, as every command has always
/// emitted it. It is formatted from the file and the name at render time
/// rather than stored: both are already in hand wherever a row is built, so
/// keeping it as a field would be a third copy of two facts.
pub fn symbol_id(file: &str, name: &str) -> String {
    format!("{file}::{name}")
}

/// The addressable id for a file's module, on the same terms.
pub fn module_id(file: &str, language: Option<&str>) -> String {
    format!("module::{}", file_to_module(file, language))
}

/// Module path for a file: extension stripped, separators turned into `.`
/// for Python and `/` for every other language.
pub fn file_to_module(relative_path: &str, language: Option<&str>) -> String {
    let path = Path::new(relative_path);
    let stem = match path.extension() {
        Some(_) => path.with_extension("").to_string_lossy().to_string(),
        None => relative_path.to_string(),
    };
    if language == Some("python") {
        stem.replace(std::path::MAIN_SEPARATOR, ".")
    } else {
        stem.replace(std::path::MAIN_SEPARATOR, "/")
    }
}

/// Drop `./` and any leading `../` segments from a relative import path.
fn strip_relative_prefix(module_path: &str) -> String {
    let mut s = module_path;
    if let Some(rest) = s.strip_prefix("./") {
        s = rest;
    }
    while let Some(rest) = s.strip_prefix("../") {
        s = rest;
    }
    s.to_string()
}

/// Stable key: the index is mutable and updated in place, so it never
/// carries a fingerprint. The schema version rides along because the rows
/// describe extraction output, which a schema bump can reshape.
fn index_key() -> String {
    format!("relations_v2__schema{}", cache::SCHEMA_VERSION)
}

static MEMO: memo::Memo<Relations> = OnceLock::new();

/// The two inversions for a repository, current as of this call.
///
/// Loads the stored index, compares its `built_from` against the per-file
/// content hashes on disk, and updates only the files that moved: dropped
/// paths are forgotten, added and changed paths are absorbed from their fresh
/// extraction. A first call on a cold cache absorbs everything, which is the
/// same work the graph build did minus the resolution pass.
pub fn get(repo_root: &Path) -> Arc<Relations> {
    memo::get_or_build(&MEMO, repo_root, || load_and_update(repo_root))
}

fn load_and_update(repo_root: &Path) -> Relations {
    let files = discover_files(repo_root);
    let hashes = file_facts::file_hashes_for(&files, repo_root);
    let mut relations = cache::load_bytes(cache::NAMESPACE_FILE, &index_key(), repo_root)
        .as_deref()
        .and_then(Relations::from_bytes)
        .unwrap_or_default();

    let gone: Vec<String> = relations
        .built_from
        .keys()
        .filter(|path| !hashes.contains_key(&***path))
        .map(|path| path.to_string())
        .collect();
    let moved: Vec<String> = hashes
        .iter()
        .filter(|(path, key)| {
            relations.built_from.get(path.as_str()).map(|b| &b.key) != Some(*key)
        })
        .map(|(path, _)| path.clone())
        .collect();
    if gone.is_empty() && moved.is_empty() {
        return relations;
    }
    for path in &gone {
        relations.forget(path);
    }

    // Adding or removing a file moves module-path resolution for every file,
    // so the whole index is rebuilt. A content-only change leaves every path
    // in place, and then only the files that moved are re-absorbed — and
    // only their entries are read. Reading all 3,030 entries for a one-line
    // edit was 0.3s on laravel-framework, on every command.
    let structural = !gone.is_empty()
        || moved
            .iter()
            .any(|path| !relations.built_from.contains_key(path.as_str()));
    let touched: Vec<String> = if structural {
        hashes.keys().cloned().collect()
    } else {
        moved.clone()
    };

    if structural {
        relations = Relations::default();
    } else {
        for path in &moved {
            relations.forget(path);
        }
    }

    // Declarations first, across every touched file, then imports: an import
    // that misses on module path falls back to the symbol map, so no file's
    // imports can resolve until every file's declarations have landed.
    //
    // The passes are split by what they need, not by reading each file twice.
    // Pass one walks the files a chunk at a time and keeps only each one's
    // imports — a handful of rows against the hundreds of references in the
    // same extraction — so the chunk's facts are dropped before the next
    // chunk is read. Holding every file's facts at once instead cost 848 MB
    // on next.js (22,702 files); the whole import set there is 36,957 rows.
    let mut imports_by_file: Vec<(String, Option<String>, Vec<extraction::Import>)> =
        Vec::with_capacity(touched.len());
    for chunk in touched.chunks(file_facts::RESOLVE_CHUNK) {
        let needed: Vec<PathBuf> = chunk.iter().map(|rel| repo_root.join(rel)).collect();
        let facts = file_facts::get_batch(&needed, repo_root);
        for path in chunk {
            let (Some(f), Some(key)) = (facts.get(path), hashes.get(path)) else {
                continue;
            };
            relations.absorb_symbols(f, key);
            if let Some(e) = &f.extraction {
                if !e.imports.is_empty() {
                    imports_by_file.push((
                        f.path.clone(),
                        f.language.clone(),
                        e.imports.clone(),
                    ));
                }
            }
        }
    }

    // The module map is a pure function of the file list and each file's
    // language, both of which the index now carries for every touched file.
    let modules = ModulePaths::new(&relations.listing());
    for (path, language, imports) in &imports_by_file {
        relations.absorb_imports(path, language.as_deref(), imports, &modules);
    }

    let key = index_key();
    let _ = cache::save(cache::NAMESPACE_FILE, &key, &relations.to_json(), repo_root);
    // A schema bump rotates the key, so the prior version's index would sit
    // in the namespace forever without this sweep.
    cache::evict_prefixed(cache::NAMESPACE_FILE, "relations_", &key, repo_root);
    relations
}

/// Files to index: the shared enumeration, filtered to supported extensions,
/// symlinks excluded. Same set the graph build walked.
pub fn discover_files(repo_root: &Path) -> Vec<PathBuf> {
    let exts = extraction::supported_extensions();
    crate::repo_files::tracked_paths(repo_root, None)
        .unwrap_or_default()
        .into_iter()
        .filter(|f| {
            f.extension()
                .and_then(|e| e.to_str())
                .map(|e| exts.contains(&e.to_lowercase().as_str()))
                .unwrap_or(false)
                && !f.is_symlink()
        })
        .collect()
}

/// One declaration a name could resolve to, with the language of the file
/// that declares it. Language is a per-file fact, so it sits beside the
/// declaration rather than being copied into every one.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub file: String,
    pub language: Option<String>,
    pub declaration: Declaration,
}

/// One resolved use site: the file and line where the call is written, the
/// declaration it resolves to, and how sure the resolution is.
#[derive(Debug, Clone)]
pub struct UseSite {
    pub file: String,
    pub line: i64,
    /// The declaration the use site sits inside, when it sits inside one.
    /// This is the calling function a `callers` row names, and it is resolved
    /// from the referencing file's own declarations — the file is already
    /// loaded, so it costs nothing. `None` for a use site at module top
    /// level, whose row is the file itself.
    pub caller: Option<Declaration>,
    pub target_file: String,
    pub target: Declaration,
    pub confidence: &'static str,
}

/// Every declaration of `name`, loaded from the files the index names.
///
/// The index carries no payload, so kind, line, container and language come
/// from the declaring files' own entries — and those are exactly the files
/// the answer is about, so the load is the answer, not overhead. A name the
/// index does not know falls back to files whose own name matches, which is
/// how a module is addressable by its last path segment.
pub fn declarations(name: &str, repo_root: &Path) -> Vec<Candidate> {
    let relations = get(repo_root);
    let files: Vec<&str> = relations.defined_in(name).collect();
    if files.is_empty() {
        return Vec::new();
    }
    let needed: Vec<PathBuf> = files.iter().map(|p| repo_root.join(p)).collect();
    let facts = file_facts::get_batch(&needed, repo_root);
    let mut out = Vec::new();
    for path in files {
        let Some(f) = facts.get(path) else { continue };
        let Some(extraction) = &f.extraction else { continue };
        for declaration in extraction
            .declarations
            .iter()
            .filter(|d| d.name.eq_ignore_ascii_case(name))
        {
            out.push(Candidate {
                file: path.to_string(),
                language: f.language.clone(),
                declaration: declaration.clone(),
            });
        }
    }
    out
}

/// Which way an import walk runs.
pub enum Reach {
    /// Who depends on this file.
    Importers,
    /// What this file depends on.
    Imports,
}

/// One file reached by an import walk, how many hops away it is, and the
/// symbol the reaching import named when it named one.
pub struct Reached {
    pub file: String,
    pub depth: i64,
    pub symbol: Option<String>,
}

/// Files reachable from `file` along import edges, up to `max_depth`.
///
/// Only the importer direction is stored; the forward direction is that map
/// read the other way, inverted once in memory. 14,094 edges on
/// laravel-framework and 678 on WordPress, so inverting costs nothing and
/// storing both would be a second copy of one fact.
pub fn reachable(
    file: &str,
    max_depth: i64,
    reach: Reach,
    repo_root: &Path,
) -> Vec<Reached> {
    let relations = get(repo_root);
    let forward: HashMap<&str, Vec<(&str, Option<&String>)>> = match reach {
        Reach::Importers => HashMap::new(),
        Reach::Imports => {
            let mut map: HashMap<&str, Vec<(&str, Option<&String>)>> = HashMap::new();
            for (target, importer) in relations.import_edges() {
                map.entry(&importer.file)
                    .or_default()
                    .push((target, importer.symbol.as_ref()));
            }
            map
        }
    };
    let mut seen: HashSet<String> = HashSet::from([file.to_string()]);
    let mut frontier = std::collections::VecDeque::from([(file.to_string(), 0i64)]);
    let mut out = Vec::new();
    while let Some((current, depth)) = frontier.pop_front() {
        if depth >= max_depth {
            continue;
        }
        let next: Vec<(String, Option<String>)> = match reach {
            Reach::Importers => relations
                .importers_of(&current)
                .iter()
                .map(|i| (i.file.to_string(), i.symbol.clone()))
                .collect(),
            Reach::Imports => forward
                .get(current.as_str())
                .map(|v| {
                    v.iter()
                        .map(|(f, s)| (f.to_string(), s.cloned()))
                        .collect()
                })
                .unwrap_or_default(),
        };
        for (step, symbol) in next {
            if !seen.insert(step.clone()) {
                continue;
            }
            out.push(Reached {
                file: step.clone(),
                depth: depth + 1,
                symbol,
            });
            frontier.push_back((step, depth + 1));
        }
    }
    out
}

/// The files with the widest reach: `(file, direct, transitive)`, ranked by
/// transitive reach then direct edges, ties broken by path so repeated runs
/// rank identically.
///
/// Ranking by raw edge count buries a file that one hub imports and
/// everything else reaches through that hub, so the transitive set ranks;
/// the walk is the cost, so it runs only over the top `limit * 3` by direct
/// count. `usages --path`, `dependencies --path` and the primer's Spine are
/// the same question asked three ways and read this one function.
pub fn ranked_by_reach(
    max_depth: i64,
    limit: usize,
    reach: Reach,
    repo_root: &Path,
) -> Vec<Ranked> {
    let relations = get(repo_root);
    let mut direct: HashMap<&str, i64> = HashMap::new();
    // The most-imported symbol of a file, for the importer ranking. The
    // ranked file is the TARGET there, and the edge's symbol lives in the
    // target, so the row can name what is actually depended on rather than
    // the file that holds it.
    let mut named: HashMap<&str, &String> = HashMap::new();
    for (target, importer) in relations.import_edges() {
        let node = match reach {
            Reach::Importers => target,
            Reach::Imports => &importer.file,
        };
        *direct.entry(node).or_insert(0) += 1;
        if matches!(reach, Reach::Importers) {
            if let Some(symbol) = importer.symbol.as_ref() {
                named.entry(target).or_insert(symbol);
            }
        }
    }
    let mut by_direct: Vec<&str> = direct.keys().copied().collect();
    by_direct.sort_by(|a, b| direct[b].cmp(&direct[a]).then(a.cmp(b)));
    by_direct.truncate(limit.saturating_mul(3));

    let mut ranked: Vec<Ranked> = by_direct
        .into_iter()
        .map(|file| {
            let walked = match reach {
                Reach::Importers => {
                    reachable(file, max_depth, Reach::Importers, repo_root).len()
                }
                Reach::Imports => {
                    reachable(file, max_depth, Reach::Imports, repo_root).len()
                }
            };
            Ranked {
                file: file.to_string(),
                symbol: named.get(file).map(|s| (*s).clone()),
                direct: direct[file],
                transitive: walked as i64,
            }
        })
        .collect();
    ranked.sort_by(|a, b| {
        (b.transitive, b.direct, &b.file).cmp(&(a.transitive, a.direct, &a.file))
    });
    ranked.truncate(limit);
    ranked
}

/// One row of a reach ranking: the file, the symbol it is depended on
/// through when there is one, and its direct and transitive edge counts.
pub struct Ranked {
    pub file: String,
    pub symbol: Option<String>,
    pub direct: i64,
    pub transitive: i64,
}

/// Every indexed file's language, which a row needs to spell that file's
/// module path. The index carries it, so no per-file entry is opened.
pub fn languages(repo_root: &Path) -> HashMap<String, Option<String>> {
    get(repo_root).listing().into_iter().collect()
}

/// Files whose module path's last segment is `name` — the module fallback
/// that makes `trace callers app` answer for `src/app.py`. Derived from the
/// file list, which is why it needs no stored module index.
pub fn modules_named(name: &str, repo_root: &Path) -> Vec<String> {
    let relations = get(repo_root);
    let wanted = name.to_lowercase();
    relations
        .files()
        .filter(|path| {
            Path::new(path)
                .file_stem()
                .map(|s| s.to_string_lossy().to_lowercase() == wanted)
                .unwrap_or(false)
        })
        .map(str::to_string)
        .collect()
}

/// Every resolved use site of `name`, computed now from the files the index
/// names rather than read out of a stored edge list.
///
/// The resolution model is unchanged, structural as before: candidates are
/// restricted to the same language as the use site, a free call resolves only
/// to non-method declarations, a static use resolves to the named class or
/// its method, and a member call resolves to methods — the sole residual
/// ambiguous case. What changed is when it runs and over how much: the files
/// the index names for this one name, not every file in the repository.
pub fn use_sites(name: &str, repo_root: &Path) -> Vec<UseSite> {
    let relations = get(repo_root);
    let mut wanted: Vec<&str> = relations.used_in(name).collect();
    if wanted.is_empty() {
        return Vec::new();
    }
    let defining: Vec<&str> = relations.defined_in(name).collect();
    let declaring: Vec<PathBuf> = defining.iter().map(|p| repo_root.join(p)).collect();
    let declaring_facts = file_facts::get_batch(&declaring, repo_root);

    // The declaration's own language is the file's, which `Declaration` does
    // not carry — it is a per-file fact, so it rides beside the row rather
    // than being copied onto every declaration in the index.
    let candidates: Vec<Candidate> = defining
        .iter()
        .filter_map(|path| declaring_facts.get(*path).map(|f| (*path, f)))
        .flat_map(|(path, f)| {
            f.extraction
                .iter()
                .flat_map(|e| e.declarations.iter())
                .filter(|d| d.name.eq_ignore_ascii_case(name))
                .map(|d| Candidate {
                    file: path.to_string(),
                    language: f.language.clone(),
                    declaration: d.clone(),
                })
                .collect::<Vec<_>>()
        })
        .collect();
    if candidates.is_empty() {
        return Vec::new();
    }

    let modules = ModulePaths::new(&relations.listing());

    // Each mentioning file resolves against the same candidate set and
    // nothing else, so the files run in parallel. On laravel-framework
    // `Collection` is named in over a thousand files, and resolving them one
    // after another was the whole cost of the command.
    //
    // A chunk at a time, like every other whole-repository walk: a name used
    // in thousands of files held every one of their extractions at once, and
    // a use site is finished the moment its file is resolved.
    wanted.sort();
    let mut sites = Vec::new();
    for chunk in wanted.chunks(file_facts::RESOLVE_CHUNK) {
        let needed: Vec<PathBuf> = chunk.iter().map(|p| repo_root.join(*p)).collect();
        let facts = file_facts::get_batch(&needed, repo_root);
        let resolved: Vec<UseSite> = chunk
            .par_iter()
            .map(|path| {
                let mut out = Vec::new();
                let Some(f) = facts.get(*path) else { return out };
                let Some(extraction) = &f.extraction else { return out };
                let imported: HashSet<String> = relations
                    .resolve_imports(&extraction.imports, f.language.as_deref(), &modules)
                    .into_iter()
                    .map(|(target, _, _)| target)
                    .collect();
                for reference in extraction
                    .references
                    .iter()
                    .filter(|r| r.name.eq_ignore_ascii_case(name))
                {
                    resolve_one(
                        &candidates,
                        path,
                        f,
                        extraction,
                        reference,
                        &imported,
                        &mut out,
                    );
                }
                out
            })
            .flatten()
            .collect();
        sites.extend(resolved);
    }
    sites
}

#[allow(clippy::too_many_arguments)]
fn resolve_one(
    candidates: &[Candidate],
    path: &str,
    facts: &FileFacts,
    extraction: &ExtractionResult,
    reference: &extraction::Reference,
    imported: &HashSet<String>,
    out: &mut Vec<UseSite>,
) {
    let matching: Vec<&Candidate> = candidates
        .iter()
        .filter(|c| {
            shape_matches(
                c,
                facts.language.as_deref(),
                reference.shape,
                reference.receiver.as_deref(),
            )
        })
        .collect();
    if matching.is_empty() {
        return;
    }
    // The calling function, resolved in the file that is already open.
    let caller = reference.enclosing.as_ref().and_then(|name| {
        extraction
            .declarations
            .iter()
            .find(|d| d.name.eq_ignore_ascii_case(name))
            .cloned()
    });
    let site = |candidate: &Candidate, confidence| UseSite {
        file: path.to_string(),
        line: reference.line,
        caller: caller.clone(),
        target_file: candidate.file.clone(),
        target: candidate.declaration.clone(),
        confidence,
    };
    if matching.len() == 1 {
        let candidate = matching[0];
        // A function's own recursive call adds no cross-symbol caller.
        if candidate.file == path && sole_declaration(extraction, &reference.name) {
            return;
        }
        let confidence = if imported.contains(&candidate.file) || candidate.file == path {
            CONFIDENCE_EXTRACTED
        } else {
            CONFIDENCE_INFERRED
        };
        out.push(site(candidate, confidence));
        return;
    }
    // Import context narrows a multi-candidate match: a single candidate in
    // an imported (or the same) file is the resolved one.
    let in_imports: Vec<&&Candidate> = matching
        .iter()
        .filter(|c| imported.contains(&c.file))
        .collect();
    if in_imports.len() == 1 {
        out.push(site(in_imports[0], CONFIDENCE_EXTRACTED));
        return;
    }
    // Only a member call may stay ambiguous: its receiver type is not named,
    // so several same-language methods genuinely could be the target. A free
    // or static call names its target exactly, so an unresolved one is name
    // coincidence, and fanning out would reintroduce the noise this model
    // removes.
    if reference.shape != RefShape::Member {
        return;
    }
    for candidate in matching {
        out.push(site(candidate, CONFIDENCE_AMBIGUOUS));
    }
}

/// A reference is a self-reference when the file declares exactly one symbol
/// with this name, which is the self-recursion case without dropping the
/// rarer one where two homonyms share a file.
fn sole_declaration(extraction: &ExtractionResult, name: &str) -> bool {
    extraction
        .declarations
        .iter()
        .filter(|d| d.name.eq_ignore_ascii_case(name))
        .count()
        == 1
}

/// Does a candidate declaration structurally match a use site of this shape,
/// in this language, naming this receiver?
fn shape_matches(
    candidate: &Candidate,
    referrer_language: Option<&str>,
    shape: RefShape,
    receiver: Option<&str>,
) -> bool {
    // Same-language only — the rule that removes every cross-language edge.
    if candidate.language.as_deref() != referrer_language {
        return false;
    }
    let declaration = &candidate.declaration;
    let is_method = declaration.container.is_some();
    let is_type = matches!(
        declaration.kind.as_str(),
        "class" | "interface" | "trait" | "enum" | "struct"
    );
    match shape {
        // A free call resolves to a non-method declaration. A type is a valid
        // target only where calling the class constructs an instance (Python,
        // Ruby); elsewhere construction is a `new` expression, already
        // classified Static.
        RefShape::Free => {
            !is_method && (!is_type || constructs_by_call(referrer_language))
        }
        // A static use names the class at the site, so it resolves exactly:
        // the named type itself, or the method of that name whose container
        // is the named type.
        RefShape::Static => match receiver {
            Some(named) => {
                (is_type && declaration.name.eq_ignore_ascii_case(named))
                    || declaration
                        .container
                        .as_deref()
                        .map(|c| c.eq_ignore_ascii_case(named))
                        .unwrap_or(false)
            }
            None => is_type || is_method,
        },
        // A member call names a value receiver, not a type, so it resolves to
        // methods of that name.
        RefShape::Member => is_method,
    }
}

/// Languages where calling a class constructs an instance, so a free call is
/// a valid reference to the class.
fn constructs_by_call(language: Option<&str>) -> bool {
    matches!(language, Some("python") | Some("ruby"))
}
