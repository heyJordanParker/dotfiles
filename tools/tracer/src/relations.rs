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
//! entries. Each row records the file it came from, so a changed file's symbol
//! rows are filtered out and re-added from its fresh extraction. When a file
//! or declaration change can alter resolution, importer rows are re-derived
//! from the unchanged files' cached imports — source is never re-extracted,
//! and there is no global fingerprint.

use crate::extraction::{Declaration, ExtractionResult, RefShape};
use crate::file_facts::{self, FileFacts};
use crate::{cache, extraction, memo};
use rayon::prelude::*;
use serde::Deserialize;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

struct ImportResolution<'a> {
    invalid_bindings: &'a [String],
    targets: Vec<(String, &'static str, Option<String>)>,
}

pub const CONFIDENCE_EXTRACTED: &str = "EXTRACTED";
pub const CONFIDENCE_INFERRED: &str = "INFERRED";
pub const CONFIDENCE_AMBIGUOUS: &str = "AMBIGUOUS";

/// The stored index: the two inversions plus the provenance that makes an
/// incremental update possible.
///
/// `built_from` is `relative path -> content-hash key`, the exact set the
/// maps were computed from. Comparing it against the current per-file hashes
/// names the files whose rows are stale, which is the whole update: drop
/// those paths from every list, then add what their fresh extraction says.
#[derive(Debug, Default)]
pub struct Relations {
    files: Vec<Arc<str>>,
    file_table_hash: String,
    symbols: OnceLock<HashMap<String, (Vec<u32>, Vec<u32>)>>,
    importers: HashMap<Arc<str>, Vec<Importer>>,
    built_from: BTreeMap<Arc<str>, Built>,
    repo_root: PathBuf,
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
struct StoredEdges {
    files: Vec<String>,
    built: Vec<(String, Option<String>)>,
    importers: HashMap<String, Vec<(u32, u64, Option<String>)>>,
}

#[derive(Deserialize)]
struct StoredSymbols {
    table: String,
    symbols: HashMap<String, (Vec<u32>, Vec<u32>)>,
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
    pub confidence: &'static str,
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
            .get_or_init(|| self.load_symbols())
            .get(&name.to_lowercase())
            .into_iter()
            .flat_map(|(defined_in, _)| defined_in)
            .filter_map(|index| self.files.get(*index as usize))
            .map(|path| &**path)
    }

    /// Files with a use site naming `name`. A candidate set for the
    /// resolver, not an answer: a use site is a match by name only until
    /// `shape_matches` has seen it.
    pub fn used_in(&self, name: &str) -> impl Iterator<Item = &str> {
        self.symbols
            .get_or_init(|| self.load_symbols())
            .get(&name.to_lowercase())
            .into_iter()
            .flat_map(|(_, used_in)| used_in)
            .filter_map(|index| self.files.get(*index as usize))
            .map(|path| &**path)
    }

    /// Files whose imports resolve to `file` — the import inversion.
    pub fn importers_of(&self, file: &str) -> &[Importer] {
        self.importers
            .get(file)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
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
    fn intern(&mut self, path: &str) -> (Arc<str>, u32) {
        if let Some((index, existing)) = self
            .files
            .iter()
            .enumerate()
            .find(|(_, existing)| &***existing == path)
        {
            return (Arc::clone(existing), index as u32);
        }
        let path: Arc<str> = Arc::from(path);
        self.files.push(Arc::clone(&path));
        (path, (self.files.len() - 1) as u32)
    }

    /// Every indexed file paired with its language, in sorted order — the
    /// complete input `ModulePaths` requires.
    pub fn listing(&self) -> Vec<(String, Option<String>)> {
        self.built_from
            .iter()
            .map(|(path, built)| (path.to_string(), built.language.clone()))
            .collect()
    }

    /// The indexed language for one file.
    pub fn language(&self, path: &str) -> Option<&str> {
        self.built_from
            .get(path)
            .and_then(|built| built.language.as_deref())
    }

    /// How many distinct names the index carries.
    pub fn name_count(&self) -> usize {
        self.symbols.get_or_init(|| self.load_symbols()).len()
    }

    /// Whether the index knows this name at all — the search signpost's
    /// whole question, answered without loading a file.
    pub fn knows(&self, name: &str) -> bool {
        self.symbols
            .get_or_init(|| self.load_symbols())
            .contains_key(&name.to_lowercase())
    }

    fn symbols_mut(&mut self) -> &mut HashMap<String, (Vec<u32>, Vec<u32>)> {
        self.symbols.get_or_init(|| self.load_symbols());
        self.symbols.get_mut().expect("symbols cell initialized")
    }

    /// Remove every trace of `file`, so its fresh contribution can be added
    /// without duplicating rows. A name left with no files at all is dropped
    /// rather than kept as an empty row.
    fn forget(&mut self, file: &str) {
        let index = self
            .files
            .iter()
            .position(|candidate| &**candidate == file)
            .map(|index| index as u32);
        self.symbols_mut().retain(|_, (defined_in, used_in)| {
            defined_in.retain(|candidate| Some(*candidate) != index);
            used_in.retain(|candidate| Some(*candidate) != index);
            !(defined_in.is_empty() && used_in.is_empty())
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
        let (path, index) = self.intern(&facts.path);
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
            let row = self
                .symbols_mut()
                .entry(declaration.name.to_lowercase())
                .or_default();
            if !row.0.contains(&index) {
                row.0.push(index);
            }
        }
        for reference in &extraction.references {
            let row = self
                .symbols_mut()
                .entry(reference.name.to_lowercase())
                .or_default();
            if !row.1.contains(&index) {
                row.1.push(index);
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
        let (importer, _) = self.intern(path);
        for (target, confidence, symbol) in self.resolve_imports(path, imports, language, modules) {
            let (target, _) = self.intern(&target);
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
        importer_path: &str,
        imports: &[extraction::Import],
        language: Option<&str>,
        modules: &ModulePaths,
    ) -> Vec<(String, &'static str, Option<String>)> {
        let mut out: Vec<(String, &'static str, Option<String>)> = Vec::new();
        for import in imports {
            let resolution = self.resolve_import(importer_path, import, language, modules);
            for (target, confidence, symbol) in resolution.targets {
                match out.iter_mut().find(|(file, _, _)| file == &target) {
                    Some(existing) => {
                        if existing.2.is_none() && symbol.is_some() {
                            existing.1 = confidence;
                            existing.2 = symbol;
                        }
                    }
                    None => out.push((target, confidence, symbol)),
                }
            }
        }
        out
    }

    fn resolve_import<'a>(
        &self,
        importer_path: &str,
        import: &'a extraction::Import,
        language: Option<&str>,
        modules: &ModulePaths,
    ) -> ImportResolution<'a> {
        // `from module import symbol` names the symbol's own file when
        // one exists, and the module otherwise — Python's package form,
        // and the reason a from-imported file must not read as having no
        // importers. Other languages' named imports still name the module
        // written after `from`, not a child path made from the symbol.
        let combined = if language == Some("python") {
            import
                .symbol
                .as_ref()
                .map(|symbol| format!("{}.{}", import.module, symbol))
        } else {
            None
        };
        let combined_resolved = combined
            .and_then(|module| modules.resolve(&module, importer_path, language))
            .unwrap_or_default();
        let module_resolved = modules.resolve(&import.module, importer_path, language);
        let Some(module_resolved) = module_resolved else {
            return ImportResolution {
                invalid_bindings: if import.locals.is_empty() {
                    import.symbol.as_slice()
                } else {
                    &import.locals
                },
                targets: Vec::new(),
            };
        };
        let mut resolved = if combined_resolved.is_empty() {
            module_resolved
        } else {
            combined_resolved
        };
        // A module path that resolved is a clean resolution. When it did
        // not, an imported symbol the map knows still names its file:
        // one declaring file is INFERRED, several are AMBIGUOUS.
        let confidence = if resolved.len() == 1 {
            CONFIDENCE_EXTRACTED
        } else if resolved.len() > 1 {
            CONFIDENCE_AMBIGUOUS
        } else {
            let Some(symbol) = import.symbol.as_ref() else {
                return ImportResolution {
                    invalid_bindings: &[],
                    targets: Vec::new(),
                };
            };
            resolved = self
                .defined_in(symbol)
                .filter(|file| {
                    self.built_from
                        .get(*file)
                        .and_then(|built| built.language.as_deref())
                        == language
                })
                .map(str::to_string)
                .collect();
            match resolved.len() {
                1 => CONFIDENCE_INFERRED,
                n if n > 1 => CONFIDENCE_AMBIGUOUS,
                _ => {
                    return ImportResolution {
                        invalid_bindings: &[],
                        targets: Vec::new(),
                    }
                }
            }
        };
        let mut targets = Vec::new();
        for target in resolved {
            // The named symbol only stands as the depended-on thing when
            // the target file actually declares it; otherwise the module
            // is what the import reaches.
            let symbol = import
                .symbol
                .as_ref()
                .filter(|s| self.defined_in(s).any(|file| file == target))
                .cloned();
            // One edge per target file. A language that emits both a
            // module import and a named import for the same statement
            // would otherwise double its direct-edge count. The named
            // symbol wins because it says what is actually depended on.
            targets.push((target, confidence, symbol));
        }
        ImportResolution {
            invalid_bindings: &[],
            targets,
        }
    }

    /// The stored forms, with every path written once in the edges entry.
    ///
    /// A path appears in the index once per name that mentions it. Spelling
    /// each one in full made laravel-framework's entry 7.0 MB, and parsing it
    /// cost 81 MB of resident memory before a single query ran. One path
    /// table plus integer references carries the identical index in 1.1 MB.
    fn to_json(&self) -> (Value, Value) {
        // `built_from` holds every indexed file, and no row can name a file
        // outside it: rows are absorbed per file and `forget` drops both.
        let paths: Vec<&str> = self.built_from.keys().map(|p| &**p).collect();
        let slot: HashMap<&str, usize> = paths.iter().enumerate().map(|(i, p)| (*p, i)).collect();
        let at = |p: &str| slot.get(p).copied();

        let built: Vec<Value> = self
            .built_from
            .values()
            .map(|b| json!([b.key, b.language]))
            .collect();

        let old_paths: Vec<&str> = self.files.iter().map(|path| &**path).collect();
        let mut symbols =
            Map::with_capacity(self.symbols.get_or_init(|| self.load_symbols()).len());
        for (name, (defined_in, used_in)) in self.symbols.get().expect("symbols initialized") {
            let remap = |indexes: &[u32]| {
                indexes
                    .iter()
                    .filter_map(|index| old_paths.get(*index as usize).and_then(|path| at(path)))
                    .collect::<Vec<_>>()
            };
            symbols.insert(name.clone(), json!([remap(defined_in), remap(used_in)]));
        }

        let mut importers = Map::with_capacity(self.importers.len());
        for (target, list) in &self.importers {
            let Some(t) = at(target) else { continue };
            let rows: Vec<Value> = list
                .iter()
                .filter_map(|i| {
                    Some(json!([
                        at(&i.file)?,
                        confidence_code(i.confidence),
                        i.symbol
                    ]))
                })
                .collect();
            importers.insert(t.to_string(), Value::Array(rows));
        }

        (
            json!({"files": paths, "built": built, "importers": Value::Object(importers)}),
            json!({"table": self.file_table_hash, "symbols": Value::Object(symbols)}),
        )
    }

    fn from_bytes(bytes: &[u8], repo_root: &Path) -> Option<Self> {
        let stored: StoredEdges = serde_json::from_slice(bytes).ok()?;
        // One handle per path, made once here. Every row below clones a
        // handle rather than the path's bytes, which is the whole reason the
        // index is cheap to hold.
        let files: Vec<Arc<str>> = stored.files.into_iter().map(Arc::<str>::from).collect();
        let file_table_hash = table_hash(files.iter().map(|path| &**path));
        let mut out = Relations {
            files,
            file_table_hash,
            repo_root: repo_root.to_path_buf(),
            ..Default::default()
        };
        for (i, (key, language)) in stored.built.into_iter().enumerate() {
            let file = Arc::clone(out.files.get(i)?);
            out.built_from.insert(file, Built { key, language });
        }
        for (target, list) in stored.importers {
            let Some(target) = target
                .parse::<u32>()
                .ok()
                .and_then(|index| out.files.get(index as usize).cloned())
            else {
                continue;
            };
            let rows: Vec<Importer> = list
                .into_iter()
                .filter_map(|(file, confidence, symbol)| {
                    Some(Importer {
                        file: out.files.get(file as usize).cloned()?,
                        confidence: confidence_name(confidence),
                        symbol,
                    })
                })
                .collect();
            out.importers.insert(target, rows);
        }
        Some(out)
    }

    fn load_symbols(&self) -> HashMap<String, (Vec<u32>, Vec<u32>)> {
        let table = &self.file_table_hash;
        if let Some(symbols) =
            cache::load_bytes(cache::NAMESPACE_FILE, &symbols_key(), &self.repo_root)
                .as_deref()
                .and_then(|bytes| serde_json::from_slice::<StoredSymbols>(bytes).ok())
                .filter(|stored| stored.table == *table)
                .map(|stored| stored.symbols)
        {
            return symbols;
        }
        let symbols = self.rebuild_symbols();
        let document = json!({"table": table, "symbols": symbols});
        let _ = cache::save(
            cache::NAMESPACE_FILE,
            &symbols_key(),
            &document,
            &self.repo_root,
        );
        symbols
    }

    fn rebuild_symbols(&self) -> HashMap<String, (Vec<u32>, Vec<u32>)> {
        let mut symbols: HashMap<String, (Vec<u32>, Vec<u32>)> = HashMap::new();
        for (base, chunk) in self.files.chunks(file_facts::RESOLVE_CHUNK).enumerate() {
            let paths: Vec<PathBuf> = chunk
                .iter()
                .map(|path| self.repo_root.join(&**path))
                .collect();
            let facts = file_facts::get_batch(&paths, &self.repo_root);
            for (offset, path) in chunk.iter().enumerate() {
                let index = base * file_facts::RESOLVE_CHUNK + offset;
                let Some(facts) = facts.get(&**path) else {
                    continue;
                };
                let Some(extraction) = &facts.extraction else {
                    continue;
                };
                for declaration in &extraction.declarations {
                    let row = symbols.entry(declaration.name.to_lowercase()).or_default();
                    if !row.0.contains(&(index as u32)) {
                        row.0.push(index as u32);
                    }
                }
                for reference in &extraction.references {
                    let row = symbols.entry(reference.name.to_lowercase()).or_default();
                    if !row.1.contains(&(index as u32)) {
                        row.1.push(index as u32);
                    }
                }
            }
        }
        symbols
    }
}

/// The three confidence classes, stored as their ordinal. Written out in full
/// they were the second-largest thing in the index after the paths.
pub(crate) fn confidence_code(confidence: &str) -> u64 {
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
    /// Normalized module path, relative file, and language in discovery order.
    entries: Vec<(String, String, Option<String>)>,
    /// Exact module path to positions in `entries`. Multiple positions retain
    /// honest same-language ambiguity without making exact imports scan the
    /// repository.
    exact: HashMap<String, Vec<usize>>,
}

impl ModulePaths {
    fn new(files: &[(String, Option<String>)]) -> Self {
        let mut entries = Vec::with_capacity(files.len());
        let mut exact: HashMap<String, Vec<usize>> = HashMap::with_capacity(files.len());
        for (path, language) in files {
            let mut module = file_to_module(path, language.as_deref());
            if language.as_deref() == Some("php") {
                module = module.to_lowercase();
            }
            let position = entries.len();
            exact.entry(module.clone()).or_default().push(position);
            entries.push((module, path.clone(), language.clone()));
        }
        Self { entries, exact }
    }

    /// The compatible-language files an import may name. Explicit relative
    /// imports are first made repository-relative from the importing file;
    /// suffix fallback retains every honest candidate rather than choosing
    /// whichever file happened to be discovered first.
    fn resolve(
        &self,
        module_path: &str,
        importer_path: &str,
        language: Option<&str>,
    ) -> Option<Vec<String>> {
        let mut wanted = module_path.to_string();
        if module_path.starts_with("./") || module_path.starts_with("../") {
            let mut path = Path::new(importer_path)
                .parent()
                .unwrap_or_else(|| Path::new(""))
                .to_path_buf();
            for segment in module_path.split('/') {
                match segment {
                    "" | "." => {}
                    ".." => {
                        if !path.pop() {
                            return None;
                        }
                    }
                    part => path.push(part),
                }
            }
            wanted = file_to_module(&path.to_string_lossy(), language);
        }

        let compared = if language == Some("php") {
            wanted.replace('\\', "/").to_lowercase()
        } else {
            wanted.clone()
        };
        let compatible = |entry_language: &Option<String>| entry_language.as_deref() == language;
        if let Some(positions) = self.exact.get(&compared) {
            let mut exact: Vec<String> = positions
                .iter()
                .filter_map(|position| self.entries.get(*position))
                .filter(|(_, _, entry_language)| compatible(entry_language))
                .map(|(_, file, _)| file.clone())
                .collect();
            if !exact.is_empty() {
                exact.sort();
                exact.dedup();
                return Some(exact);
            }
        }

        let mut suffixes: Vec<String> = self
            .entries
            .iter()
            .filter(|(module, _, entry_language)| {
                if !compatible(entry_language) {
                    return false;
                }
                module.strip_suffix(&compared).is_some_and(|prefix| {
                    prefix.is_empty() || prefix.ends_with('.') || prefix.ends_with('/')
                })
            })
            .map(|(_, file, _)| file.clone())
            .collect();
        suffixes.sort();
        suffixes.dedup();
        Some(suffixes)
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

/// Stable key: the index is mutable and updated in place, so it never
/// carries a fingerprint. The schema version rides along because the rows
/// describe extraction output, which a schema bump can reshape.
fn edges_key() -> String {
    format!("relations_edges_v1__schema{}", cache::SCHEMA_VERSION)
}

fn symbols_key() -> String {
    format!("relations_symbols_v1__schema{}", cache::SCHEMA_VERSION)
}

fn table_hash<'a>(paths: impl IntoIterator<Item = &'a str>) -> String {
    let mut hasher = Sha256::new();
    for path in paths {
        hasher.update(path.as_bytes());
        hasher.update(b"\0");
    }
    hex::encode(hasher.finalize())
}

static MEMO: memo::Memo<Relations> = OnceLock::new();

/// The two inversions for a repository, current as of this call.
///
/// Loads the stored index, compares its `built_from` against the per-file
/// content hashes on disk, and re-absorbs declarations for files that moved.
/// When files or declarations move, imports are re-resolved from cached
/// per-file extraction so unchanged importers follow the new declarations. A
/// first call on a cold cache absorbs everything, which is the same work the
/// graph build did minus reference resolution.
pub fn get(repo_root: &Path) -> Arc<Relations> {
    memo::get_or_build(&MEMO, repo_root, || load_and_update(repo_root))
}

fn load_and_update(repo_root: &Path) -> Relations {
    let Some(files) = discover_files(repo_root) else {
        return cache::load_bytes(cache::NAMESPACE_FILE, &edges_key(), repo_root)
            .as_deref()
            .and_then(|bytes| Relations::from_bytes(bytes, repo_root))
            .unwrap_or_else(|| Relations {
                repo_root: repo_root.to_path_buf(),
                ..Default::default()
            });
    };
    let hashes = file_facts::file_hashes_for(&files, repo_root);
    let mut relations = cache::load_bytes(cache::NAMESPACE_FILE, &edges_key(), repo_root)
        .as_deref()
        .and_then(|bytes| Relations::from_bytes(bytes, repo_root))
        .unwrap_or_else(|| Relations {
            repo_root: repo_root.to_path_buf(),
            ..Default::default()
        });

    let gone: Vec<String> = relations
        .built_from
        .keys()
        .filter(|path| !hashes.contains_key(&***path))
        .map(|path| path.to_string())
        .collect();
    let moved: Vec<String> = hashes
        .iter()
        .filter(|(path, key)| relations.built_from.get(path.as_str()).map(|b| &b.key) != Some(*key))
        .map(|(path, _)| path.clone())
        .collect();
    if gone.is_empty() && moved.is_empty() {
        return relations;
    }
    for path in &gone {
        relations.forget(path);
    }

    // Adding or removing a file moves module-path resolution for every file,
    // so the whole index is rebuilt. A content-only change re-absorbs the
    // files that moved. If their declarations moved too, every import is
    // re-resolved from cached extraction because symbol fallback depends on
    // declarations in files the importer never opened.
    let structural = !gone.is_empty()
        || moved
            .iter()
            .any(|path| !relations.built_from.contains_key(path.as_str()));
    let moved_files: HashSet<&str> = moved.iter().map(String::as_str).collect();
    if !structural {
        relations.symbols.get_or_init(|| relations.load_symbols());
    }
    let declarations_before: BTreeSet<(String, String)> = if structural {
        BTreeSet::new()
    } else {
        relations
            .symbols
            .get()
            .expect("symbols loaded before incremental update")
            .iter()
            .flat_map(|(name, (defined_in, _))| {
                defined_in
                    .iter()
                    .filter_map(|index| relations.files.get(*index as usize))
                    .filter(|file| moved_files.contains::<str>(file.as_ref()))
                    .map(|file| (name.clone(), file.to_string()))
            })
            .collect()
    };
    let touched: Vec<String> = if structural {
        hashes.keys().cloned().collect()
    } else {
        moved.clone()
    };

    if structural {
        relations = Relations {
            repo_root: repo_root.to_path_buf(),
            ..Default::default()
        };
        relations
            .symbols
            .set(HashMap::new())
            .expect("new symbols cell is empty");
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
    let mut imports_by_file: BTreeMap<String, (Option<String>, Vec<extraction::Import>)> =
        BTreeMap::new();
    for chunk in touched.chunks(file_facts::RESOLVE_CHUNK) {
        let needed: Vec<PathBuf> = chunk.iter().map(|rel| repo_root.join(rel)).collect();
        let mut facts = file_facts::get_batch(&needed, repo_root);
        for path in chunk {
            let (Some(f), Some(key)) = (facts.remove(path), hashes.get(path)) else {
                continue;
            };
            relations.absorb_symbols(&f, key);
            if let Some(extraction) = f.extraction {
                if !extraction.imports.is_empty() {
                    imports_by_file.insert(f.path, (f.language, extraction.imports));
                }
            }
        }
    }

    let declarations_after: BTreeSet<(String, String)> = if structural {
        BTreeSet::new()
    } else {
        relations
            .symbols
            .get()
            .expect("symbols loaded before incremental update")
            .iter()
            .flat_map(|(name, (defined_in, _))| {
                defined_in
                    .iter()
                    .filter_map(|index| relations.files.get(*index as usize))
                    .filter(|file| moved_files.contains::<str>(file.as_ref()))
                    .map(|file| (name.clone(), file.to_string()))
            })
            .collect()
    };
    let rebuild_importers = structural || declarations_before != declarations_after;
    if rebuild_importers {
        relations.importers.clear();
        let already_loaded: HashSet<&str> = touched.iter().map(String::as_str).collect();
        let untouched: Vec<String> = hashes
            .keys()
            .filter(|path| !already_loaded.contains(path.as_str()))
            .cloned()
            .collect();
        for chunk in untouched.chunks(file_facts::RESOLVE_CHUNK) {
            let needed: Vec<PathBuf> = chunk.iter().map(|rel| repo_root.join(rel)).collect();
            let mut facts = file_facts::get_batch(&needed, repo_root);
            for path in chunk {
                let Some(f) = facts.remove(path) else {
                    continue;
                };
                if let Some(extraction) = f.extraction {
                    if !extraction.imports.is_empty() {
                        imports_by_file.insert(f.path, (f.language, extraction.imports));
                    }
                }
            }
        }
    }

    // The module map is a pure function of the file list and each file's
    // language, both of which the index carries for every indexed file.
    let modules = ModulePaths::new(&relations.listing());
    for (path, (language, imports)) in &imports_by_file {
        relations.absorb_imports(path, language.as_deref(), imports, &modules);
    }

    relations.file_table_hash = table_hash(relations.files.iter().map(|path| &**path));
    let (edges, symbols) = relations.to_json();
    let edges_key = edges_key();
    let symbols_key = symbols_key();
    let _ = cache::save(cache::NAMESPACE_FILE, &symbols_key, &symbols, repo_root);
    let _ = cache::save(cache::NAMESPACE_FILE, &edges_key, &edges, repo_root);
    // A schema bump rotates the key, so the prior version's index would sit
    // in the namespace forever without this sweep.
    cache::evict_prefixed(
        cache::NAMESPACE_FILE,
        "relations_edges_v1_",
        &edges_key,
        repo_root,
    );
    cache::evict_prefixed(
        cache::NAMESPACE_FILE,
        "relations_symbols_v1_",
        &symbols_key,
        repo_root,
    );
    cache::evict_prefixed(cache::NAMESPACE_FILE, "relations_v", "", repo_root);
    relations
}

/// Files to index: the shared enumeration, filtered to supported extensions,
/// symlinks excluded. Same set the graph build walked.
pub fn discover_files(repo_root: &Path) -> Option<Vec<PathBuf>> {
    let exts = extraction::supported_extensions();
    Some(
        crate::repo_files::tracked_paths(repo_root, None)?
            .into_iter()
            .filter(|f| {
                f.extension()
                    .and_then(|e| e.to_str())
                    .map(|e| exts.contains(&e.to_lowercase().as_str()))
                    .unwrap_or(false)
                    && !f.is_symlink()
            })
            .collect(),
    )
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
    let mut out = Vec::new();
    for chunk in files.chunks(file_facts::RESOLVE_CHUNK) {
        let needed: Vec<PathBuf> = chunk.iter().map(|path| repo_root.join(*path)).collect();
        let facts = file_facts::get_batch(&needed, repo_root);
        for path in chunk {
            let Some(f) = facts.get(*path) else { continue };
            let Some(extraction) = &f.extraction else {
                continue;
            };
            for declaration in extraction
                .declarations
                .iter()
                .filter(|declaration| declaration.name.eq_ignore_ascii_case(name))
            {
                out.push(Candidate {
                    file: path.to_string(),
                    language: f.language.clone(),
                    declaration: declaration.clone(),
                });
            }
        }
    }
    out
}

/// Which way an import walk runs.
#[derive(Clone, Copy)]
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
pub fn reachable(file: &str, max_depth: i64, reach: Reach, repo_root: &Path) -> Vec<Reached> {
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
    reachable_from(&relations, &forward, file, max_depth, reach)
}

fn reachable_from(
    relations: &Relations,
    forward: &HashMap<&str, Vec<(&str, Option<&String>)>>,
    file: &str,
    max_depth: i64,
    reach: Reach,
) -> Vec<Reached> {
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
                .map(|v| v.iter().map(|(f, s)| (f.to_string(), s.cloned())).collect())
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
/// everything else reaches through that hub, so every scoped subject's
/// transitive set is measured before the limit. `usages --path`,
/// `dependencies --path` and the primer's Spine are the same question
/// asked three ways and read this one function.
pub fn ranked_by_reach(
    max_depth: i64,
    limit: usize,
    reach: Reach,
    scope: Option<&str>,
    repo_root: &Path,
) -> Vec<Ranked> {
    let relations = get(repo_root);
    let mut identity: HashMap<&str, usize> = HashMap::new();
    let mut files: Vec<&str> = Vec::new();
    for (target, importer) in relations.import_edges() {
        for file in [target, &*importer.file] {
            if !identity.contains_key(file) {
                identity.insert(file, files.len());
                files.push(file);
            }
        }
    }

    let mut adjacency: Vec<Vec<usize>> = vec![Vec::new(); files.len()];
    let mut direct = vec![0i64; files.len()];
    // The ranked file is the TARGET there, and the edge's symbol lives in the
    // target, so the row can name what is actually depended on rather than
    // the file that holds it.
    let mut named: Vec<Option<&String>> = vec![None; files.len()];
    for (target, importer) in relations.import_edges() {
        let target = identity[target];
        let importer_file = identity[&*importer.file];
        let (node, next) = match reach {
            Reach::Importers => (target, importer_file),
            Reach::Imports => (importer_file, target),
        };
        adjacency[node].push(next);
        direct[node] += 1;
        if matches!(reach, Reach::Importers) {
            if let Some(symbol) = importer.symbol.as_ref() {
                named[target].get_or_insert(symbol);
            }
        }
    }
    let scope = scope.unwrap_or("").trim_end_matches('/');
    let prefix = if scope.is_empty() {
        None
    } else {
        Some(format!("{scope}/"))
    };
    let mut subjects: Vec<usize> = files
        .iter()
        .enumerate()
        .filter(|(id, file)| {
            direct[*id] > 0
                && (scope.is_empty()
                    || **file == scope
                    || prefix
                        .as_deref()
                        .map(|prefix| file.starts_with(prefix))
                        .unwrap_or(false))
        })
        .map(|(id, _)| id)
        .collect();
    subjects.sort_by_key(|id| files[*id]);

    let mut ranked: Vec<Ranked> = subjects
        .into_par_iter()
        .map_init(
            || {
                (
                    vec![0usize; files.len()],
                    0usize,
                    std::collections::VecDeque::new(),
                )
            },
            |(visited, visit, frontier), file| {
                *visit += 1;
                visited[file] = *visit;
                frontier.clear();
                frontier.push_back((file, 0i64));
                let mut transitive = 0i64;
                while let Some((current, depth)) = frontier.pop_front() {
                    if depth >= max_depth {
                        continue;
                    }
                    for &next in &adjacency[current] {
                        if visited[next] == *visit {
                            continue;
                        }
                        visited[next] = *visit;
                        transitive += 1;
                        frontier.push_back((next, depth + 1));
                    }
                }
                Ranked {
                    file: files[file].to_string(),
                    symbol: named[file].cloned(),
                    direct: direct[file],
                    transitive,
                }
            },
        )
        .collect();
    ranked
        .sort_by(|a, b| (b.transitive, b.direct, &b.file).cmp(&(a.transitive, a.direct, &a.file)));
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
/// Resolution stays structural: candidates are restricted to the same
/// language as the use site, a free call resolves only to non-method
/// declarations, a static use resolves to the named class or its method, and
/// a member call resolves to methods. Member calls remain ambiguous when the
/// receiver type is unknown; free and static calls also remain ambiguous when
/// an ambiguous import legitimately names multiple candidates. Resolution
/// runs only over the files the index names for this name.
pub fn use_sites(name: &str, repo_root: &Path) -> Vec<UseSite> {
    let relations = get(repo_root);
    let mut wanted: Vec<&str> = relations.used_in(name).collect();
    if wanted.is_empty() {
        return Vec::new();
    }
    let defining: Vec<&str> = relations.defined_in(name).collect();

    // The declaration's own language is the file's, which `Declaration` does
    // not carry — it is a per-file fact, so it rides beside the row rather
    // than being copied onto every declaration in the index.
    let mut candidates = Vec::new();
    for chunk in defining.chunks(file_facts::RESOLVE_CHUNK) {
        let declaring: Vec<PathBuf> = chunk.iter().map(|path| repo_root.join(*path)).collect();
        let facts = file_facts::get_batch(&declaring, repo_root);
        for path in chunk {
            let Some(f) = facts.get(*path) else {
                continue;
            };
            candidates.extend(
                f.extraction
                    .iter()
                    .flat_map(|extraction| extraction.declarations.iter())
                    .filter(|declaration| declaration.name.eq_ignore_ascii_case(name))
                    .map(|declaration| Candidate {
                        file: path.to_string(),
                        language: f.language.clone(),
                        declaration: declaration.clone(),
                    }),
            );
        }
    }
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
                let Some(f) = facts.get(*path) else {
                    return out;
                };
                let Some(extraction) = &f.extraction else {
                    return out;
                };
                let import_resolutions: Vec<ImportResolution<'_>> = extraction
                    .imports
                    .iter()
                    .map(|import| {
                        relations.resolve_import(path, import, f.language.as_deref(), &modules)
                    })
                    .collect();
                let invalid_bindings: Vec<&str> = import_resolutions
                    .iter()
                    .flat_map(|resolution| resolution.invalid_bindings.iter().map(String::as_str))
                    .collect();
                let imported: HashMap<String, &'static str> = import_resolutions
                    .iter()
                    .flat_map(|resolution| resolution.targets.iter())
                    .fold(HashMap::new(), |mut imported, (target, confidence, _)| {
                        let confidence = confidence_name(confidence_code(&confidence));
                        imported
                            .entry(target.clone())
                            .and_modify(|current| {
                                if confidence_code(confidence) < confidence_code(current) {
                                    *current = confidence;
                                }
                            })
                            .or_insert(confidence);
                        imported
                    });
                for reference in extraction
                    .references
                    .iter()
                    .filter(|r| r.name.eq_ignore_ascii_case(name))
                {
                    if invalid_bindings
                        .iter()
                        .any(|binding| reference_uses_binding(reference, binding))
                    {
                        continue;
                    }
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

fn reference_uses_binding(reference: &extraction::Reference, binding: &str) -> bool {
    match reference.shape {
        RefShape::Free => reference.name.eq_ignore_ascii_case(binding),
        RefShape::Member | RefShape::Static => reference
            .receiver
            .as_deref()
            .is_some_and(|receiver| receiver.eq_ignore_ascii_case(binding)),
    }
}

#[allow(clippy::too_many_arguments)]
fn resolve_one(
    candidates: &[Candidate],
    path: &str,
    facts: &FileFacts,
    extraction: &ExtractionResult,
    reference: &extraction::Reference,
    imported: &HashMap<String, &'static str>,
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
        let confidence = if candidate.file == path {
            CONFIDENCE_EXTRACTED
        } else {
            imported
                .get(&candidate.file)
                .copied()
                .unwrap_or(CONFIDENCE_INFERRED)
        };
        out.push(site(candidate, confidence));
        return;
    }
    // Import context narrows a multi-candidate match. One imported candidate
    // keeps the import edge's confidence; several are all legitimate targets
    // and therefore remain ambiguous.
    let in_imports: Vec<(&Candidate, &str)> = matching
        .iter()
        .filter_map(|candidate| {
            imported
                .get(&candidate.file)
                .map(|confidence| (*candidate, *confidence))
        })
        .collect();
    if in_imports.len() == 1 {
        out.push(site(in_imports[0].0, in_imports[0].1));
        return;
    }
    if in_imports.len() > 1 {
        for (candidate, _) in in_imports {
            out.push(site(candidate, CONFIDENCE_AMBIGUOUS));
        }
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
        RefShape::Free => !is_method && (!is_type || constructs_by_call(referrer_language)),
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
