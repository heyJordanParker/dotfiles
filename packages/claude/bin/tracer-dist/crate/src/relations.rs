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
//! rows are filtered out and re-added from its fresh extraction. The index
//! also keeps each file's import rows, so when a file or declaration change
//! can alter resolution the importer inversion is re-derived in memory from
//! those rows: an added file used to force every entry to be read back, which
//! eight concurrent calls repeated eight times. Nothing is re-extracted, and
//! there is no global fingerprint.

use crate::extraction::{Declaration, ExtractionResult, RefShape};
use crate::file_facts::{self, FileFacts};
use crate::{cache, extraction, memo};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
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
    names: Vec<String>,
    file_table_hash: String,
    symbols: OnceLock<HashMap<String, (Vec<u32>, Vec<u32>)>>,
    importers: HashMap<Arc<str>, Vec<Importer>>,
    /// Each file's import rows as extracted, the source the importer
    /// inversion is derived from. Stored in their own entry and loaded only
    /// for an update, the way the symbol map is loaded only for a symbol
    /// query: a warm call never needs them. Files with no imports are absent.
    imports: HashMap<Arc<str>, Vec<extraction::Import>>,
    built_from: BTreeMap<Arc<str>, Built>,
    roots: Roots,
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
    annotations: Vec<(u32, u32)>,
}

/// One directory's place in the import graph, counted over its direct files.
/// An `AMBIGUOUS` edge and an edge between two files of the same directory
/// count for neither side.
#[derive(Clone, Default, Deserialize, Serialize)]
pub struct DirectoryMetrics {
    pub files: u32,
    /// Distinct files outside the directory that import one of its files.
    pub imported_by: u32,
    /// Distinct files outside the directory that its files import.
    pub imports: u32,
    /// The directories those imported files live in.
    pub imported_directories: BTreeSet<String>,
    pub annotations: BTreeMap<String, usize>,
}

/// The graph around one file, as its facts show it.
pub struct ModuleCounts {
    /// Files that import this one.
    pub imported_by: usize,
    /// Files this one imports.
    pub imports: u32,
}

#[derive(Default, Deserialize, Serialize)]
pub struct DirectoryIndex {
    pub directories: BTreeMap<String, DirectoryMetrics>,
    /// Files each file imports, keyed by path; a file importing nothing is absent.
    pub imports_per_file: BTreeMap<String, u32>,
}

/// The index exactly as it sits on disk, so reading it allocates the rows
/// and nothing else. `stored_forms` writes this shape.
#[derive(Deserialize)]
struct StoredEdges {
    files: Vec<String>,
    #[serde(default)]
    names: Vec<String>,
    built: Vec<(String, Option<String>, Vec<(u32, u32)>)>,
    importers: HashMap<String, Vec<(u32, u64, Option<String>)>>,
    #[serde(default)]
    roots: Roots,
}

#[derive(Deserialize)]
struct StoredSymbols {
    table: String,
    symbols: HashMap<String, (Vec<u32>, Vec<u32>)>,
}

#[derive(Deserialize)]
struct StoredImports {
    table: String,
    imports: Vec<Vec<extraction::Import>>,
}

/// The same three shapes on the way out, borrowing the rows they write so
/// a save serializes the index once, straight to bytes: building a JSON
/// value tree first cost 86ms per update on 37,000 import rows. Sorted maps
/// keep the bytes stable across writes of an unchanged index.
#[derive(Serialize)]
struct EdgesEntry<'a> {
    files: Vec<&'a str>,
    names: &'a [String],
    built: Vec<(&'a str, Option<&'a str>, &'a [(u32, u32)])>,
    importers: BTreeMap<String, Vec<(u32, u64, Option<&'a str>)>>,
    roots: &'a Roots,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
struct Roots {
    php: BTreeMap<String, String>,
    javascript_paths: BTreeMap<String, Vec<String>>,
    javascript_base_url: Option<String>,
    manifests: BTreeMap<String, String>,
}

impl Roots {
    fn read(repo_root: &Path) -> Self {
        let mut roots = Self::default();
        for name in ["composer.json", "tsconfig.json", "jsconfig.json"] {
            let path = repo_root.join(name);
            let Ok(bytes) = std::fs::read(&path) else {
                continue;
            };
            roots
                .manifests
                .insert(name.to_string(), hex::encode(Sha256::digest(&bytes)));
            let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
                continue;
            };
            if name == "composer.json" {
                for section in ["autoload", "autoload-dev"] {
                    let Some(psr4) = value[section]["psr-4"].as_object() else {
                        continue;
                    };
                    for (prefix, target) in psr4 {
                        let Some(target) = target.as_str() else {
                            continue;
                        };
                        roots
                            .php
                            .insert(prefix.clone(), target.trim_end_matches('/').to_string());
                    }
                }
            } else {
                let options = &value["compilerOptions"];
                if let Some(base_url) = options["baseUrl"].as_str() {
                    roots.javascript_base_url = normalized_path(PathBuf::new(), base_url)
                        .map(|path| path.to_string_lossy().to_string());
                }
                if let Some(paths) = options["paths"].as_object() {
                    for (alias, targets) in paths {
                        let targets: Vec<String> = targets
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(Value::as_str)
                            .filter_map(|target| normalized_path(PathBuf::new(), target))
                            .map(|path| path.to_string_lossy().to_string())
                            .collect();
                        if !targets.is_empty() {
                            roots.javascript_paths.insert(alias.clone(), targets);
                        }
                    }
                }
            }
        }
        roots
    }

    fn has_mapping(&self, language: Option<&str>) -> bool {
        match language {
            Some("php") => !self.php.is_empty(),
            Some("typescript") => {
                !self.javascript_paths.is_empty() || self.javascript_base_url.is_some()
            }
            _ => false,
        }
    }
}

fn normalized_path(mut base: PathBuf, path: &str) -> Option<PathBuf> {
    for segment in Path::new(path).components() {
        match segment {
            std::path::Component::Normal(segment) => base.push(segment),
            std::path::Component::ParentDir if !base.pop() => return None,
            _ => {}
        }
    }
    Some(base)
}

#[derive(Serialize)]
struct SymbolsEntry<'a> {
    table: &'a str,
    symbols: BTreeMap<&'a str, (Vec<u32>, Vec<u32>)>,
}

#[derive(Serialize)]
struct ImportsEntry<'a> {
    table: &'a str,
    imports: Vec<&'a [extraction::Import]>,
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

    /// The importers of `file` whose import resolved to it: an `AMBIGUOUS`
    /// row names several candidate files, so it is evidence for none of them.
    /// Every count of a file's importers reads this, the way the directory
    /// counts skip the same rows.
    pub fn resolved_importers_of(&self, file: &str) -> impl Iterator<Item = &Importer> {
        self.importers_of(file)
            .iter()
            .filter(|importer| importer.confidence != CONFIDENCE_AMBIGUOUS)
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

    pub fn directory_metrics(&self) -> Arc<DirectoryIndex> {
        memo::get_or_build(&DIRECTORY_METRICS_MEMO, &self.repo_root, || {
            self.load_or_build_directory_metrics()
        })
    }

    pub fn directory_metrics_for(&self, directory: &str) -> Option<DirectoryMetrics> {
        self.directory_metrics().directories.get(directory).cloned()
    }

    fn load_or_build_directory_metrics(&self) -> DirectoryIndex {
        if let Some(index) = load_directory_metrics(&self.repo_root) {
            return index;
        }
        let _lock = cache::maintain(&self.repo_root);
        if let Some(index) = load_directory_metrics(&self.repo_root) {
            return index;
        }
        let index = self.build_directory_metrics();
        save_directory_metrics(&index, &self.repo_root);
        index
    }

    fn build_directory_metrics(&self) -> DirectoryIndex {
        let files: Vec<&Arc<str>> = self.built_from.keys().collect();
        let slots: HashMap<&str, usize> = files
            .iter()
            .enumerate()
            .map(|(index, file)| (&***file, index))
            .collect();
        let mut directory_ids = Vec::with_capacity(files.len());
        let mut directories = Vec::new();
        let mut directory_slots = HashMap::new();
        let mut metrics = Vec::new();
        for file in &files {
            let directory = directory_of(file);
            let directory_id = match directory_slots.get(directory.as_str()) {
                Some(id) => *id,
                None => {
                    let id = directories.len();
                    directory_slots.insert(directory.clone(), id);
                    directories.push(directory);
                    metrics.push(DirectoryMetrics::default());
                    id
                }
            };
            directory_ids.push(directory_id);
        }
        for (index, (_, built)) in self.built_from.iter().enumerate() {
            let metrics = &mut metrics[directory_ids[index]];
            metrics.files += 1;
            for &(name, count) in &built.annotations {
                let Some(name) = self.names.get(name as usize) else {
                    continue;
                };
                *metrics.annotations.entry(name.clone()).or_default() += count as usize;
            }
        }

        let mut imports_per_file: BTreeMap<String, u32> = BTreeMap::new();
        let mut imported_by = vec![Vec::new(); directories.len()];
        let mut imports = vec![Vec::new(); directories.len()];
        let mut imported_directories = vec![Vec::new(); directories.len()];
        for (target, importer) in self.import_edges() {
            if importer.confidence == CONFIDENCE_AMBIGUOUS {
                continue;
            }
            let (Some(&target), Some(&importer)) = (slots.get(target), slots.get(&*importer.file))
            else {
                continue;
            };
            *imports_per_file.entry(files[importer].to_string()).or_default() += 1;
            let target_directory = directory_ids[target];
            let importer_directory = directory_ids[importer];
            if target_directory != importer_directory {
                imported_by[target_directory].push(importer as u32);
                imports[importer_directory].push(target as u32);
                imported_directories[importer_directory].push(target_directory as u32);
            }
        }
        for index in 0..metrics.len() {
            for list in [&mut imported_by[index], &mut imports[index], &mut imported_directories[index]] {
                list.sort_unstable();
                list.dedup();
            }
            metrics[index].imported_by = imported_by[index].len() as u32;
            metrics[index].imports = imports[index].len() as u32;
            metrics[index].imported_directories = imported_directories[index]
                .iter()
                .filter_map(|directory| directories.get(*directory as usize))
                .cloned()
                .collect();
        }
        DirectoryIndex {
            directories: directories.into_iter().zip(metrics).collect(),
            imports_per_file,
        }
    }

    /// The graph around a file that its facts show. One owner — every command
    /// that prints a file's facts reads it here.
    pub fn module_counts(&self, file: &str) -> Option<ModuleCounts> {
        // Only the tree-sitter extractors read imports: a Markdown file's
        // "imported by 0" would read as a fact about code.
        self.built_from.get(file)?;
        let extension = Path::new(file).extension()?.to_str()?.to_lowercase();
        if !extraction::supported_extensions().contains(&extension.as_str()) {
            return None;
        }
        Some(ModuleCounts {
            imported_by: self.resolved_importers_of(file).count(),
            imports: self
                .directory_metrics()
                .imports_per_file
                .get(file)
                .copied()
                .unwrap_or(0),
        })
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

    fn intern_name(&mut self, name: &str) -> u32 {
        if let Some(index) = self.names.iter().position(|candidate| candidate == name) {
            return index as u32;
        }
        self.names.push(name.to_string());
        (self.names.len() - 1) as u32
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
        self.imports.remove(file);
        self.built_from.remove(file);
    }

    /// Pass one: this file's declarations and use sites into the symbol map.
    /// Every file's declarations must land before any file's imports are
    /// resolved, because an import that misses on module path falls back to
    /// the symbol map — so the two passes cannot be merged.
    fn absorb_symbols(&mut self, facts: &FileFacts, key: &str) {
        let (path, index) = self.intern(&facts.path);
        let annotations = facts
            .extraction
            .as_ref()
            .map(annotation_counts)
            .unwrap_or_default()
            .into_iter()
            .map(|(name, count)| (self.intern_name(&name), count))
            .collect();
        self.built_from.insert(
            Arc::clone(&path),
            Built {
                key: key.to_string(),
                language: facts.language.clone(),
                annotations,
            },
        );
        let extraction = match &facts.extraction {
            Some(e) => e,
            None => return,
        };
        if !extraction.imports.is_empty() {
            self.imports
                .insert(Arc::clone(&path), extraction.imports.clone());
        }
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
                        if confidence_code(confidence) < confidence_code(existing.1) {
                            existing.1 = confidence;
                            existing.2 = symbol;
                        } else if existing.2.is_none() && symbol.is_some() {
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
        let php_combined = (language == Some("php"))
            .then(|| {
                import
                    .symbol
                    .as_ref()
                    .map(|symbol| format!("{}\\{}", import.module, symbol))
            })
            .flatten()
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
        let mut resolved = if !combined_resolved.is_empty() {
            combined_resolved
        } else if !php_combined.is_empty() {
            php_combined
        } else {
            module_resolved
        };
        // A module path that resolved is a clean resolution. When it did
        // not, an imported symbol the map knows still names its file:
        // one declaring file is INFERRED, several are AMBIGUOUS.
        let confidence = if resolved.len() == 1 {
            CONFIDENCE_EXTRACTED
        } else if resolved.len() > 1 {
            CONFIDENCE_AMBIGUOUS
        } else {
            if modules.roots.has_mapping(language) {
                return ImportResolution {
                    invalid_bindings: if import.locals.is_empty() {
                        import.symbol.as_slice()
                    } else {
                        &import.locals
                    },
                    targets: Vec::new(),
                };
            }
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
    fn stored_forms(&self) -> (EdgesEntry<'_>, SymbolsEntry<'_>, ImportsEntry<'_>) {
        // `built_from` holds every indexed file, and no row can name a file
        // outside it: rows are absorbed per file and `forget` drops both.
        let paths: Vec<&str> = self.built_from.keys().map(|p| &**p).collect();
        let slot: HashMap<&str, u32> = paths
            .iter()
            .enumerate()
            .map(|(i, p)| (*p, i as u32))
            .collect();
        let at = |p: &str| slot.get(p).copied();

        let built: Vec<(&str, Option<&str>, &[(u32, u32)])> = self
            .built_from
            .values()
            .map(|b| (b.key.as_str(), b.language.as_deref(), b.annotations.as_slice()))
            .collect();
        let imports: Vec<&[extraction::Import]> = paths
            .iter()
            .map(|p| self.imports.get(*p).map(Vec::as_slice).unwrap_or(&[]))
            .collect();

        let old_paths: Vec<&str> = self.files.iter().map(|path| &**path).collect();
        let remap = |indexes: &[u32]| {
            indexes
                .iter()
                .filter_map(|index| old_paths.get(*index as usize).and_then(|path| at(path)))
                .collect::<Vec<_>>()
        };
        let symbols: BTreeMap<&str, (Vec<u32>, Vec<u32>)> = self
            .symbols
            .get_or_init(|| self.load_symbols())
            .iter()
            .map(|(name, (defined_in, used_in))| {
                (name.as_str(), (remap(defined_in), remap(used_in)))
            })
            .collect();

        let mut importers = BTreeMap::new();
        for (target, list) in &self.importers {
            let Some(t) = at(target) else { continue };
            let rows: Vec<(u32, u64, Option<&str>)> = list
                .iter()
                .filter_map(|i| {
                    Some((
                        at(&i.file)?,
                        confidence_code(i.confidence),
                        i.symbol.as_deref(),
                    ))
                })
                .collect();
            importers.insert(t.to_string(), rows);
        }

        let table = self.file_table_hash.as_str();
        (
            EdgesEntry {
                files: paths,
                names: &self.names,
                built,
                importers,
                roots: &self.roots,
            },
            SymbolsEntry { table, symbols },
            ImportsEntry { table, imports },
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
            names: stored.names,
            file_table_hash,
            repo_root: repo_root.to_path_buf(),
            roots: stored.roots,
            ..Default::default()
        };
        for (i, (key, language, annotations)) in stored.built.into_iter().enumerate() {
            let file = Arc::clone(out.files.get(i)?);
            out.built_from.insert(
                file,
                Built {
                    key,
                    language,
                    annotations,
                },
            );
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
        let document = SymbolsEntry {
            table,
            symbols: symbols
                .iter()
                .map(|(name, rows)| (name.as_str(), rows.clone()))
                .collect(),
        };
        let _ = cache::save(
            cache::NAMESPACE_FILE,
            &symbols_key(),
            &document,
            &self.repo_root,
        );
        symbols
    }

    /// Every indexed file's import rows into `imports`: from the stored entry
    /// when its table matches this index, otherwise read back out of every
    /// file's per-file entry, which is the cost the stored entry exists to
    /// pay once. Called before an update, with `files` still in stored order.
    fn load_imports(&mut self) {
        let table = &self.file_table_hash;
        if let Some(stored) =
            cache::load_bytes(cache::NAMESPACE_FILE, &imports_key(), &self.repo_root)
                .as_deref()
                .and_then(|bytes| serde_json::from_slice::<StoredImports>(bytes).ok())
                .filter(|stored| stored.table == *table)
        {
            for (index, rows) in stored.imports.into_iter().enumerate() {
                if let (false, Some(file)) = (rows.is_empty(), self.files.get(index)) {
                    self.imports.insert(Arc::clone(file), rows);
                }
            }
            return;
        }
        let files: Vec<Arc<str>> = self.built_from.keys().cloned().collect();
        for chunk in files.chunks(file_facts::RESOLVE_CHUNK) {
            let paths: Vec<PathBuf> = chunk
                .iter()
                .map(|path| self.repo_root.join(&**path))
                .collect();
            let mut facts = file_facts::get_batch(&paths, &self.repo_root);
            for path in chunk {
                let Some(extraction) = facts.remove(&**path).and_then(|f| f.extraction) else {
                    continue;
                };
                if !extraction.imports.is_empty() {
                    self.imports.insert(Arc::clone(path), extraction.imports);
                }
            }
        }
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

fn annotation_counts(extraction: &ExtractionResult) -> BTreeMap<String, u32> {
    let mut annotations = BTreeMap::new();
    for declaration in &extraction.declarations {
        for annotation in &declaration.annotations {
            *annotations.entry(annotation.clone()).or_default() += 1;
        }
    }
    annotations
}

fn directory_of(file: &str) -> String {
    match Path::new(file).parent().and_then(Path::to_str) {
        Some("") | None => "./".to_string(),
        Some(parent) => format!("{parent}/"),
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
    /// Final path segment to positions in `entries`. A suffix match ends at
    /// the entry's end, so the segments agree, and the fallback reads this
    /// list instead of every entry: re-resolving 37,000 imports against
    /// 2,600 files cost 880ms per update through the full scan.
    tails: HashMap<String, Vec<usize>>,
    roots: Roots,
}

/// The final segment of a module path, past the last `.` or `/`.
fn tail(module: &str) -> &str {
    module.rsplit(['.', '/']).next().unwrap_or(module)
}

impl ModulePaths {
    fn new(files: &[(String, Option<String>)], roots: Roots) -> Self {
        let mut entries = Vec::with_capacity(files.len());
        let mut exact: HashMap<String, Vec<usize>> = HashMap::with_capacity(files.len());
        let mut tails: HashMap<String, Vec<usize>> = HashMap::with_capacity(files.len());
        for (path, language) in files {
            let mut module = file_to_module(path, language.as_deref());
            if language.as_deref() == Some("php") {
                module = module.to_lowercase();
            }
            let position = entries.len();
            exact.entry(module.clone()).or_default().push(position);
            tails
                .entry(tail(&module).to_string())
                .or_default()
                .push(position);
            entries.push((module, path.clone(), language.clone()));
        }
        Self {
            entries,
            exact,
            tails,
            roots,
        }
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
        if language == Some("php") && self.roots.has_mapping(language) {
            let mut prefixes: Vec<&String> = self.roots.php.keys().collect();
            prefixes.sort_by_key(|prefix| std::cmp::Reverse(prefix.len()));
            for prefix in prefixes {
                if let Some(rest) = module_path.strip_prefix(prefix.as_str()) {
                    let base = self.roots.php[prefix].as_str();
                    let wanted = format!("{base}/{}.php", rest.replace('\\', "/"));
                    return Some(self.files_named(&wanted, language));
                }
            }
            return Some(Vec::new());
        }
        if language == Some("typescript") && self.roots.has_mapping(language) {
            if module_path.starts_with("./") || module_path.starts_with("../") {
                return Some(self.typescript_path(module_path, importer_path));
            }
            let mut aliases: Vec<&String> = self.roots.javascript_paths.keys().collect();
            aliases.sort_by_key(|alias| std::cmp::Reverse(alias.len()));
            for alias in aliases {
                let Some(rest) = alias_match(module_path, alias) else {
                    continue;
                };
                let mut resolved = Vec::new();
                for target in &self.roots.javascript_paths[alias] {
                    let target = target.replace('*', rest);
                    resolved.extend(self.typescript_path_from(&target));
                }
                resolved.sort();
                resolved.dedup();
                return Some(resolved);
            }
            if let Some(base_url) = &self.roots.javascript_base_url {
                return Some(self.typescript_path_from(&format!("{base_url}/{module_path}")));
            }
            return Some(Vec::new());
        }
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
            .tails
            .get(tail(&compared))
            .into_iter()
            .flatten()
            .filter_map(|position| self.entries.get(*position))
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

    fn files_named(&self, path: &str, language: Option<&str>) -> Vec<String> {
        self.entries
            .iter()
            .filter(|(_, file, entry_language)| {
                entry_language.as_deref() == language && file == path
            })
            .map(|(_, file, _)| file.clone())
            .collect()
    }

    fn typescript_path(&self, module_path: &str, importer_path: &str) -> Vec<String> {
        let base = Path::new(importer_path)
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_path_buf();
        let Some(path) = normalized_path(base, module_path) else {
            return Vec::new();
        };
        self.typescript_path_from(&path.to_string_lossy())
    }

    fn typescript_path_from(&self, path: &str) -> Vec<String> {
        let mut out = Vec::new();
        out.extend(self.files_named(path, Some("typescript")));
        for extension in ["ts", "tsx", "js", "jsx"] {
            out.extend(self.files_named(&format!("{path}.{extension}"), Some("typescript")));
        }
        for extension in ["ts", "tsx", "js", "jsx"] {
            out.extend(self.files_named(&format!("{path}/index.{extension}"), Some("typescript")));
        }
        out
    }
}

fn alias_match<'a>(module: &'a str, alias: &'a str) -> Option<&'a str> {
    if let Some(prefix) = alias.strip_suffix('*') {
        return module.strip_prefix(prefix);
    }
    (module == alias).then_some("")
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

fn imports_key() -> String {
    format!("relations_imports_v1__schema{}", cache::SCHEMA_VERSION)
}

fn directories_key() -> String {
    format!("relations_directories_v1__schema{}", cache::SCHEMA_VERSION)
}

fn load_directory_metrics(repo_root: &Path) -> Option<DirectoryIndex> {
    cache::load_bytes(cache::NAMESPACE_FILE, &directories_key(), repo_root)
        .as_deref()
        .and_then(|bytes| serde_json::from_slice(bytes).ok())
}

fn save_directory_metrics(index: &DirectoryIndex, repo_root: &Path) {
    let key = directories_key();
    if let Ok(true) = cache::save(cache::NAMESPACE_FILE, &key, index, repo_root) {
        cache::evict_prefixed(
            cache::NAMESPACE_FILE,
            "relations_directories_v1_",
            &key,
            repo_root,
        );
    }
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
static DIRECTORY_METRICS_MEMO: memo::Memo<DirectoryIndex> = OnceLock::new();

/// The two inversions for a repository, current as of this call.
///
/// Loads the stored index, compares its `built_from` against the per-file
/// content hashes on disk, and re-absorbs declarations for files that moved.
/// When files or declarations move, imports are re-resolved from the stored
/// import rows so unchanged importers follow the new declarations. A first
/// call on a cold cache absorbs everything, which is the same work the graph
/// build did minus reference resolution.
pub fn get(repo_root: &Path) -> Arc<Relations> {
    memo::get_or_build(&MEMO, repo_root, || load_and_update(repo_root))
}

fn stored(repo_root: &Path) -> Relations {
    cache::load_bytes(cache::NAMESPACE_FILE, &edges_key(), repo_root)
        .as_deref()
        .and_then(|bytes| Relations::from_bytes(bytes, repo_root))
        .unwrap_or_else(|| Relations {
            repo_root: repo_root.to_path_buf(),
            ..Default::default()
        })
}

/// The files the index no longer covers and the files whose content key
/// differs from the one their rows were absorbed from, or None when the
/// index is current.
fn stale(
    relations: &Relations,
    hashes: &BTreeMap<String, String>,
) -> Option<(Vec<String>, Vec<String>)> {
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
    (!gone.is_empty() || !moved.is_empty()).then_some((gone, moved))
}

fn load_and_update(repo_root: &Path) -> Relations {
    let relations = stored(repo_root);
    let known_extensions: HashSet<String> = relations
        .built_from
        .keys()
        .filter_map(|path| {
            Path::new(&**path)
                .extension()
                .and_then(|extension| extension.to_str())
        })
        .map(str::to_lowercase)
        .collect();
    let Some((files, stamps)) = discover_files_stamped(repo_root, &known_extensions) else {
        return stored(repo_root);
    };
    let hashes = file_facts::file_hashes_for_stamped(&files, &stamps, repo_root);
    let roots = Roots::read(repo_root);
    if stale(&relations, &hashes).is_none() && relations.roots == roots {
        return relations;
    }

    // One maintainer at a time. The holder may have absorbed exactly this
    // change while we waited, so the index is read again under the lock and
    // compared afresh: what it left is what remains. A file that moves
    // between the hash sweep and here is absorbed under its earlier key,
    // which the next call sees as moved again and corrects.
    let _lock = cache::maintain(repo_root);
    let mut relations = stored(repo_root);
    let roots = Roots::read(repo_root);
    let manifests_changed = relations.roots != roots;
    let (gone, mut moved) = stale(&relations, &hashes).unwrap_or_default();
    if manifests_changed {
        moved.extend(relations.built_from.keys().map(ToString::to_string));
        moved.sort();
        moved.dedup();
    }
    if gone.is_empty() && moved.is_empty() {
        return relations;
    }

    // A content change re-absorbs the files that moved. When a file appears
    // or disappears, or a touched file's declarations changed, module-path
    // and symbol-fallback resolution can shift for importers that never
    // changed, so the importers whose rows name an affected module tail or
    // symbol are re-resolved from their stored import rows. Nothing but the
    // touched files is read back.
    let added: Vec<String> = moved
        .iter()
        .filter(|path| !relations.built_from.contains_key(path.as_str()))
        .cloned()
        .collect();
    relations.symbols.get_or_init(|| relations.load_symbols());
    relations.load_imports();
    let moved_files: HashSet<&str> = moved.iter().map(String::as_str).collect();
    let touched_files: HashSet<&str> = moved_files
        .iter()
        .copied()
        .chain(gone.iter().map(String::as_str))
        .collect();
    let declarations_of =
        |relations: &Relations, files: &HashSet<&str>| -> BTreeSet<(String, String)> {
            relations
                .symbols
                .get()
                .expect("symbols loaded before the update")
                .iter()
                .flat_map(|(name, (defined_in, _))| {
                    defined_in
                        .iter()
                        .filter_map(|index| relations.files.get(*index as usize))
                        .filter(|file| files.contains::<str>(file.as_ref()))
                        .map(|file| (name.clone(), file.to_string()))
                })
                .collect()
        };
    let declarations_before = declarations_of(&relations, &touched_files);

    for path in gone.iter().chain(moved.iter()) {
        relations.forget(path);
    }
    for chunk in moved.chunks(file_facts::RESOLVE_CHUNK) {
        let needed: Vec<PathBuf> = chunk.iter().map(|rel| repo_root.join(rel)).collect();
        let facts = file_facts::get_batch(&needed, repo_root);
        for path in chunk {
            let (Some(f), Some(key)) = (facts.get(path), hashes.get(path)) else {
                continue;
            };
            relations.absorb_symbols(f, key);
        }
    }

    // Declarations first, then imports: an import that misses on module path
    // falls back to the symbol map, so no file's imports can resolve until
    // every touched file's declarations have landed.
    //
    // An importer is affected when one of its rows names a module whose
    // final segment is an added or removed file's stem, which is the only way
    // module-path resolution can gain or lose a candidate, or names a symbol
    // that a touched file declared before or declares now, which is the only
    // way symbol fallback can move. Re-resolving every row instead cost 140ms
    // per update on 37,000 rows.
    let declarations_after = declarations_of(&relations, &moved_files);
    let mut stems: HashSet<String> = HashSet::new();
    for path in gone.iter().chain(added.iter()) {
        if let Some(stem) = Path::new(path).file_stem().and_then(|stem| stem.to_str()) {
            stems.insert(stem.to_string());
            stems.insert(stem.to_lowercase());
            if stem == "index" {
                if let Some(parent) = Path::new(path)
                    .parent()
                    .and_then(|parent| parent.file_name())
                {
                    let parent = parent.to_string_lossy().to_string();
                    stems.insert(parent.to_lowercase());
                    stems.insert(parent);
                }
            }
        }
    }
    let names: HashSet<&str> = declarations_before
        .iter()
        .chain(declarations_after.iter())
        .map(|(name, _)| name.as_str())
        .collect();
    let imports = std::mem::take(&mut relations.imports);
    let affected: HashSet<Arc<str>> = imports
        .iter()
        .filter(|(path, rows)| {
            relations.importers.is_empty()
                || moved_files.contains::<str>(path.as_ref())
                || rows.iter().any(|row| {
                    let module = row.module.replace('\\', "/");
                    let end = tail(&module);
                    // A named import can resolve to the symbol's own file
                    // (Python's `from pkg import helper` reaching
                    // `pkg/helper.py`), so the symbol is checked as a stem
                    // as well as a name.
                    stems.contains(end)
                        || stems.contains(&end.to_lowercase())
                        || row.symbol.as_deref().is_some_and(|symbol| {
                            let lower = symbol.to_lowercase();
                            names.contains(lower.as_str())
                                || stems.contains(symbol)
                                || stems.contains(&lower)
                        })
                })
        })
        .map(|(path, _)| Arc::clone(path))
        .collect();
    relations.roots = roots;
    let modules = ModulePaths::new(&relations.listing(), relations.roots.clone());
    for list in relations.importers.values_mut() {
        list.retain(|importer| !affected.contains(&importer.file));
    }
    relations.importers.retain(|_, list| !list.is_empty());
    for path in &affected {
        let Some(rows) = imports.get(path) else {
            continue;
        };
        let language = relations.language(path).map(str::to_string);
        relations.absorb_imports(path, language.as_deref(), rows, &modules);
    }
    relations.imports = imports;

    // The table is hashed in stored order, which `from_bytes` reproduces;
    // `files` itself appends each newly interned path at the end.
    relations.file_table_hash = table_hash(relations.built_from.keys().map(|path| &**path));
    let (edges, symbols, imports) = relations.stored_forms();
    let edges_key = edges_key();
    let symbols_key = symbols_key();
    let imports_key = imports_key();
    let _ = cache::save(cache::NAMESPACE_FILE, &symbols_key, &symbols, repo_root);
    let _ = cache::save(cache::NAMESPACE_FILE, &imports_key, &imports, repo_root);
    // A schema bump rotates the keys, so the prior version's index would sit
    // in the namespace forever without this sweep. The keys are otherwise
    // stable, so the sweep runs once per rotation, never per write. The
    // edges entry is written only here, so it is the one whose first write
    // marks the rotation: `load_symbols` may have written the symbols entry
    // already.
    if let Ok(true) = cache::save(cache::NAMESPACE_FILE, &edges_key, &edges, repo_root) {
        for (prefix, keep) in [
            ("relations_edges_v1_", edges_key.as_str()),
            ("relations_symbols_v1_", symbols_key.as_str()),
            ("relations_imports_v1_", imports_key.as_str()),
            ("relations_v", ""),
        ] {
            cache::evict_prefixed(cache::NAMESPACE_FILE, prefix, keep, repo_root);
        }
    }
    save_directory_metrics(&relations.build_directory_metrics(), repo_root);
    relations
}

fn discover_files_stamped(
    repo_root: &Path,
    known_extensions: &HashSet<String>,
) -> Option<(Vec<PathBuf>, Vec<crate::repo_files::Stamp>)> {
    let listed = crate::repo_files::tracked_files(repo_root, None)?;
    let mut paths = Vec::new();
    let mut stamps = Vec::new();
    for ((path, stamp), symlink) in listed
        .paths
        .iter()
        .zip(&listed.stamps)
        .zip(&listed.symlinks)
    {
        let path = repo_root.join(path);
        let known_extension = path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| known_extensions.contains(&extension.to_lowercase()));
        if !symlink && (known_extension || extraction::is_supported(&path)) {
            paths.push(path);
            stamps.push(stamp.clone());
        }
    }
    Some((paths, stamps))
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
///
/// `Class::method` and `Class.method` name one class's member: the member's
/// declarations, kept to the ones `qualifies` says the qualifier names.
pub fn declarations(symbol: &str, repo_root: &Path) -> Vec<Candidate> {
    let (qualifier, name) = split_qualified(symbol);
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
    out.retain(|candidate| qualifies(qualifier, &candidate.file, &candidate.declaration));
    out
}

/// `Class::method` or `Class.method` as `(Some("Class"), "method")`; a bare
/// name as `(None, name)`. A namespaced qualifier keeps its last segment,
/// the way a declaration's `container` is written.
fn split_qualified(symbol: &str) -> (Option<&str>, &str) {
    let split = symbol
        .rfind("::")
        .map(|at| (at, 2))
        .or_else(|| symbol.rfind('.').map(|at| (at, 1)));
    match split {
        Some((at, width)) if at > 0 && at + width < symbol.len() => {
            let qualifier = &symbol[..at];
            let last = qualifier.rsplit(['\\', '.', ':']).next().unwrap_or(qualifier);
            (Some(last), &symbol[at + width..])
        }
        _ => (None, symbol),
    }
}

/// Whether a qualifier names this declaration: its container (a class, an
/// impl's type), or — for a declaration with no container — the file that
/// declares it, which is how `module.function` and `module::function` read.
fn qualifies(qualifier: Option<&str>, file: &str, declaration: &Declaration) -> bool {
    let Some(qualifier) = qualifier else {
        return true;
    };
    match &declaration.container {
        Some(container) => container
            .rsplit(['\\', '.', ':'])
            .next()
            .is_some_and(|last| last.eq_ignore_ascii_case(qualifier)),
        None => Path::new(file)
            .file_stem()
            .is_some_and(|stem| stem.to_string_lossy().eq_ignore_ascii_case(qualifier)),
    }
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
    // An AMBIGUOUS row names several candidate files and depends on none, so
    // it ranks nothing — the same rows every importer count skips.
    let resolved_edges = || {
        relations
            .import_edges()
            .filter(|(_, importer)| importer.confidence != CONFIDENCE_AMBIGUOUS)
    };
    let mut identity: HashMap<&str, usize> = HashMap::new();
    let mut files: Vec<&str> = Vec::new();
    for (target, importer) in resolved_edges() {
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
    // the file that holds it. The importer with the lowest path names it, so
    // the label does not drift with the order rows were absorbed in.
    let mut named: Vec<Option<(&str, &String)>> = vec![None; files.len()];
    for (target, importer) in resolved_edges() {
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
                let file: &str = &importer.file;
                if named[target].map_or(true, |(known, _)| file < known) {
                    named[target] = Some((file, symbol));
                }
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
                    symbol: named[file].map(|(_, symbol)| symbol.clone()),
                    direct: direct[file],
                    transitive,
                }
            },
        )
        .collect();
    ranked
        .sort_by(|a, b| (b.transitive, b.direct, &b.file).cmp(&(a.transitive, a.direct, &a.file)));
    ranked.truncate(limit);
    // An import can name something the file does not declare — `Path` from a
    // `use std::path::Path` the resolver pinned on it. That name labels
    // nothing in the repository, so the row names the file's module instead.
    for row in &mut ranked {
        let Some(symbol) = row.symbol.as_deref() else {
            continue;
        };
        let declared = file_facts::get(&repo_root.join(&row.file), repo_root)
            .and_then(|facts| facts.extraction)
            .is_some_and(|extraction| {
                extraction.declarations.iter().any(|declaration| declaration.name == symbol)
            });
        if !declared {
            row.symbol = None;
        }
    }
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
///
/// A qualified `Class::method` resolves against every `method` like a bare
/// name — so a call stays ambiguous when it is — and keeps the sites whose
/// target the qualifier names.
pub fn use_sites(symbol: &str, repo_root: &Path) -> Vec<UseSite> {
    let (qualifier, name) = split_qualified(symbol);
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

    let modules = ModulePaths::new(&relations.listing(), relations.roots.clone());

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
    sites.retain(|site| qualifies(qualifier, &site.target_file, &site.target));
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
    if declaration.kind == "property" {
        return false;
    }
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
        RefShape::Member => match receiver.filter(|_| types_member_receiver(referrer_language)) {
            Some(named) => is_method
                && declaration
                    .container
                    .as_deref()
                    .map(|c| c.eq_ignore_ascii_case(named))
                    .unwrap_or(false),
            None => is_method,
        },
    }
}

/// Languages where calling a class constructs an instance, so a free call is
/// a valid reference to the class.
fn constructs_by_call(language: Option<&str>) -> bool {
    matches!(language, Some("python") | Some("ruby"))
}

fn types_member_receiver(language: Option<&str>) -> bool {
    matches!(language, Some("php"))
}
