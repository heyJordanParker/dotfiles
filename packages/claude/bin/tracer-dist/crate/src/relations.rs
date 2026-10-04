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
use crate::repo_files::Stamp;
use crate::{cache, extraction, memo};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::borrow::Cow;
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

/// An inline `mod` a file declares: its name, and its header and end lines.
type InlineModule = (String, (i64, i64));

/// A Rust struct, enum, union, or trait a file declares: its name, and the
/// inline `mod` that holds it.
type DeclaredType = (String, Option<String>);

/// What the index keeps of one file's modules: its import rows, the inline
/// `mod`s it declares, and the Rust types it declares.
type ModuleRows = (Vec<extraction::Import>, Vec<InlineModule>, Vec<DeclaredType>);

/// A `mod` item a Rust file declares with `#[path]`: its name and the file
/// the attribute names.
type ModuleFile = (String, String);

/// A type a PHP file declares, with each type it inherits from by its last
/// segment, nearest first, and whether a `use` of the file binds that name.
type Inherits = (String, Vec<(String, bool)>);

/// One [`Built`] as the edges entry stores it, at its file's position in the
/// file table: its key, language, annotation counts, `#[path]` modules, and
/// the PHP types it declares with what each inherits from.
type StoredBuilt = (String, Option<String>, Vec<(u32, u32)>, Vec<ModuleFile>, Vec<Inherits>);

/// [`StoredBuilt`] borrowed for a save.
type BuiltEntry<'a> = (&'a str, Option<&'a str>, &'a [(u32, u32)], &'a [ModuleFile], &'a [Inherits]);

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
    /// inversion is derived from, beside the inline `mod`s and the Rust types
    /// it declares. Stored in their own entry and loaded only for an update
    /// or a Rust path that reaches a module, the way the symbol map is loaded
    /// only for a symbol query. Files with none of them are absent.
    imports: OnceLock<HashMap<Arc<str>, ModuleRows>>,
    built_from: BTreeMap<Arc<str>, Built>,
    roots: Roots,
    cargo_manifests: CargoManifests,
    repo_root: PathBuf,
}

/// What the index knows about a file without opening it: the content key its
/// rows were absorbed from, and its language. The language is stored because
/// every module path is spelled from it, and reading it back out of 3,030
/// per-file entries cost 0.12s on laravel-framework — on `callers`, `usages`,
/// `dependencies`, and the primer alike. `module_files` are the `mod` items
/// of a Rust file whose `#[path]` names their file, each with that file,
/// because every Rust path reads its file's module from them. `inherits` are
/// the PHP types the file declares with what each inherits from, because a
/// PHP call on a class reads the methods of its whole lineage.
#[derive(Debug, Clone)]
struct Built {
    key: String,
    language: Option<String>,
    annotations: Vec<(u32, u32)>,
    module_files: Vec<ModuleFile>,
    inherits: Vec<Inherits>,
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
    built: Vec<StoredBuilt>,
    importers: HashMap<String, Vec<(u32, u64, Option<String>)>>,
    #[serde(default)]
    roots: Roots,
    #[serde(default)]
    cargo_manifests: CargoManifests,
}

#[derive(Deserialize)]
struct StoredSymbols {
    table: String,
    symbols: HashMap<String, (Vec<u32>, Vec<u32>)>,
}

#[derive(Deserialize)]
struct StoredImports {
    table: String,
    imports: Vec<ModuleRows>,
}

/// The same three shapes on the way out, borrowing the rows they write so
/// a save serializes the index once, straight to bytes: building a JSON
/// value tree first cost 86ms per update on 37,000 import rows. Sorted maps
/// keep the bytes stable across writes of an unchanged index.
#[derive(Serialize)]
struct EdgesEntry<'a> {
    files: Vec<&'a str>,
    names: &'a [String],
    built: Vec<BuiltEntry<'a>>,
    importers: BTreeMap<String, Vec<(u32, u64, Option<&'a str>)>>,
    roots: &'a Roots,
    cargo_manifests: &'a CargoManifests,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
struct Roots {
    php: BTreeMap<String, String>,
    javascript_paths: BTreeMap<String, Vec<String>>,
    javascript_base_url: Option<String>,
    /// Every Rust crate root file, with the name other crates `use` it by
    /// when it is a library.
    rust_crates: BTreeMap<String, Option<String>>,
    /// Every Cargo package's directory, with each name its code reaches a
    /// crate by, its dependencies and its own library, and that crate's
    /// library root in the repository, or none outside it.
    rust_dependencies: BTreeMap<String, BTreeMap<String, Option<String>>>,
    /// Every Cargo package's directory, with each name its dependencies
    /// leave out, and the library roots of the packages of the repository
    /// that name may reach.
    rust_left_out: BTreeMap<String, BTreeMap<String, Vec<String>>>,
    /// Every directory `tests/Pest.php` binds, with the test case class its
    /// closures run as, so `$this` in a Pest test names that class.
    php_test_cases: BTreeMap<String, String>,
}

/// The parts of a `Cargo.toml` that name its crate roots, its
/// dependencies, and the sources its `[patch]` redirects.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
struct CargoManifest {
    package: Option<CargoPackage>,
    lib: Option<CargoTarget>,
    #[serde(default)]
    bin: Vec<CargoTarget>,
    #[serde(flatten)]
    dependencies: CargoDependencies,
    #[serde(default)]
    target: BTreeMap<String, CargoDependencies>,
    workspace: Option<CargoWorkspace>,
    #[serde(default)]
    patch: BTreeMap<String, toml::Table>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
struct CargoDependencies {
    #[serde(default)]
    dependencies: toml::Table,
    #[serde(default, rename = "dev-dependencies")]
    dev_dependencies: toml::Table,
    #[serde(default, rename = "build-dependencies")]
    build_dependencies: toml::Table,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
struct CargoWorkspace {
    #[serde(default)]
    dependencies: toml::Table,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
struct CargoPackage {
    name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
struct CargoTarget {
    name: Option<String>,
    path: Option<String>,
}

/// Every listed `Cargo.toml` as parsed, beside the stamp of the file it was
/// parsed from.
type CargoManifests = BTreeMap<String, (Stamp, CargoManifest)>;

impl Roots {
    /// The root mappings, and the Cargo manifests they were read from. A
    /// manifest whose stamp matches `parsed` is not read again.
    fn read(repo_root: &Path, parsed: &CargoManifests) -> (Self, CargoManifests) {
        let mut roots = Self::default();
        let manifests = roots.read_cargo_manifests(repo_root, parsed);
        for name in ["composer.json", "tsconfig.json", "jsconfig.json"] {
            let path = repo_root.join(name);
            let Ok(bytes) = std::fs::read(&path) else {
                continue;
            };
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
        if let Ok(source) = std::fs::read(repo_root.join("tests/Pest.php")) {
            for (class, directories) in crate::extraction::php::pest_bindings(&source) {
                for directory in directories {
                    if let Some(bound) = normalized_path(PathBuf::from("tests"), &directory) {
                        roots.php_test_cases.insert(bound.to_string_lossy().to_string(), class.clone());
                    }
                }
            }
        }
        (roots, manifests)
    }

    /// The test case class `$this` names in a Pest test written in `file`:
    /// the class bound to the nearest directory holding it.
    fn test_case_of(&self, file: &str) -> Option<&str> {
        Path::new(file)
            .ancestors()
            .skip(1)
            .find_map(|directory| self.php_test_cases.get(&*directory.to_string_lossy()))
            .map(String::as_str)
    }

    /// Every listed `Cargo.toml` with a `[package]` is a crate. Its library
    /// root is the `[lib]` path or `src/lib.rs`, named by `[lib] name`, else
    /// by the package name with `-` read as `_`. Its binary roots are
    /// `src/main.rs`, `src/bin/*.rs`, `src/bin/*/main.rs`, each `[[bin]]`
    /// path, and `src/bin/<name>.rs` for a `[[bin]]` with no path. A root
    /// counts wherever its file exists. Its code reaches its own library by
    /// its name, and each key of `[dependencies]`, `[dev-dependencies]`,
    /// `[build-dependencies]`, and the same three under every `[target.*]`.
    /// Its workspace root is the nearest manifest holding `[workspace]`, else
    /// its own, and a `workspace = true` key reads its entry there. An entry
    /// with a `path` reaches the library of the package it names; any other
    /// reaches the library of the workspace root's `[patch]` entry whose
    /// source and package match its own, and none otherwise. An entry with no
    /// `path` that reaches no library is left out when its package is the
    /// name of a package of the repository, because a redirect tracer does
    /// not read may lead there, so it is never proven external. A left-out
    /// key keeps the library roots of the packages of that name. A key is named
    /// by the library it reaches, as Cargo names it, a left-out key by the
    /// library of each package of its name, unless a `package = "…"` rename or
    /// no library leaves it named by itself with `-` read as `_`.
    fn read_cargo_manifests(&mut self, repo_root: &Path, parsed: &CargoManifests) -> CargoManifests {
        let Some(listed) = crate::repo_files::tracked_files(repo_root, None) else {
            return CargoManifests::new();
        };
        let manifests: CargoManifests = listed
            .paths
            .iter()
            .zip(&listed.stamps)
            .filter(|(path, _)| path.rsplit('/').next() == Some("Cargo.toml"))
            .map(|(path, stamp)| {
                let manifest = match parsed.get(path) {
                    Some((known, manifest)) if known == stamp => manifest.clone(),
                    _ => std::fs::read_to_string(repo_root.join(path))
                        .ok()
                        .and_then(|text| toml::from_str(&text).ok())
                        .unwrap_or_default(),
                };
                (path.clone(), (stamp.clone(), manifest))
            })
            .collect();
        let exists: HashSet<&str> = listed.iter().map(String::as_str).collect();
        let mut automatic: HashMap<&str, Vec<&str>> = HashMap::new();
        for path in listed.iter() {
            let Some((directory, target)) = path.rsplit_once("src/bin/") else {
                continue;
            };
            let binary = match target.split_once('/') {
                None => target.ends_with(".rs"),
                Some((_, file)) => file == "main.rs",
            };
            if binary && (directory.is_empty() || directory.ends_with('/')) {
                automatic.entry(directory).or_default().push(path);
            }
        }
        let directories: Vec<(&str, &CargoManifest)> = manifests
            .iter()
            .map(|(path, (_, manifest))| (path.strip_suffix("Cargo.toml").unwrap_or_default(), manifest))
            .collect();
        let mut libraries: HashMap<&str, (String, String)> = HashMap::new();
        for &(directory, manifest) in &directories {
            let Some(package) = &manifest.package else {
                continue;
            };
            let declared = manifest.bin.iter().filter_map(|target| match (&target.path, &target.name) {
                (Some(path), _) => Some(format!("{directory}{path}")),
                (None, Some(name)) => Some(format!("{directory}src/bin/{name}.rs")),
                (None, None) => None,
            });
            let binaries: Vec<String> = std::iter::once(format!("{directory}src/main.rs"))
                .chain(declared)
                .chain(automatic.get(directory).into_iter().flatten().map(|path| path.to_string()))
                .collect();
            for binary in binaries {
                if exists.contains(binary.as_str()) {
                    self.rust_crates.insert(binary, None);
                }
            }
            let library = manifest.lib.as_ref();
            let library_path = format!(
                "{directory}{}",
                library.and_then(|lib| lib.path.as_deref()).unwrap_or("src/lib.rs")
            );
            if exists.contains(library_path.as_str()) {
                let name = library
                    .and_then(|lib| lib.name.clone())
                    .unwrap_or_else(|| package.name.replace('-', "_"));
                self.rust_crates.insert(library_path.clone(), Some(name.clone()));
                libraries.insert(directory, (library_path, name));
            }
        }
        let library_at = |base: &str, entry: &toml::Value| -> Option<&(String, String)> {
            let target = normalized_path(PathBuf::from(base), entry.get("path")?.as_str()?)?;
            let directory = match target.to_str()? {
                "" => String::new(),
                target => format!("{target}/"),
            };
            libraries.get(directory.as_str())
        };
        let packages: Vec<(&str, &CargoManifest, (&str, &CargoManifest))> = directories
            .iter()
            .filter(|(_, manifest)| manifest.package.is_some())
            .map(|&(directory, manifest)| {
                let root = directories
                    .iter()
                    .filter(|(root, holder)| directory.starts_with(root) && holder.workspace.is_some())
                    .max_by_key(|(root, _)| root.len())
                    .copied()
                    .unwrap_or((directory, manifest));
                (directory, manifest, root)
            })
            .collect();
        let mut repository_packages: HashMap<&str, Vec<&(String, String)>> = HashMap::new();
        for &(directory, manifest, _) in &packages {
            let Some(package) = &manifest.package else {
                continue;
            };
            repository_packages
                .entry(package.name.as_str())
                .or_default()
                .extend(libraries.get(directory));
        }
        for &(directory, manifest, (root_directory, root_manifest)) in &packages {
            let mut names: BTreeMap<String, Option<String>> = BTreeMap::new();
            let mut left_out: BTreeMap<String, Vec<String>> = BTreeMap::new();
            let tables = std::iter::once(&manifest.dependencies)
                .chain(manifest.target.values())
                .flat_map(|table| [&table.dependencies, &table.dev_dependencies, &table.build_dependencies]);
            for (key, entry) in tables.flatten() {
                let (base, entry) = match entry.get("workspace").and_then(toml::Value::as_bool) {
                    Some(true) => (
                        root_directory,
                        root_manifest.workspace.as_ref().and_then(|shared| shared.dependencies.get(key)),
                    ),
                    _ => (directory, Some(entry)),
                };
                let package = entry
                    .and_then(|entry| entry.get("package"))
                    .and_then(toml::Value::as_str)
                    .unwrap_or(key);
                let library = entry.and_then(|entry| match entry.get("path") {
                    Some(_) => library_at(base, entry),
                    None => {
                        let source = canonical_source(
                            ["git", "registry"].into_iter().find_map(|field| entry.get(field)?.as_str()),
                        );
                        let patch = root_manifest
                            .patch
                            .iter()
                            .filter(|(patched, _)| canonical_source(Some(patched.as_str())) == source)
                            .flat_map(|(_, patches)| patches)
                            .find_map(|(name, patch)| {
                                let patched = patch.get("package").and_then(toml::Value::as_str).unwrap_or(name);
                                (patched == package).then_some(patch)
                            })?;
                        library_at(root_directory, patch)
                    }
                });
                let renamed = entry.is_some_and(|entry| entry.get("package").is_some());
                if let Some(reached) = repository_packages
                    .get(package)
                    .filter(|_| library.is_none() && entry.is_some_and(|entry| entry.get("path").is_none()))
                {
                    for (root, library) in reached {
                        let name = if renamed { key.replace('-', "_") } else { library.clone() };
                        let roots = left_out.entry(name).or_default();
                        if !roots.contains(root) {
                            roots.push(root.clone());
                        }
                    }
                    continue;
                }
                let name = match library {
                    Some((_, name)) if !renamed => name.clone(),
                    _ => key.replace('-', "_"),
                };
                let reached = names.entry(name).or_default();
                if reached.is_none() {
                    *reached = library.map(|(root, _)| root.clone());
                }
            }
            if let Some((root, name)) = libraries.get(directory) {
                names.entry(name.clone()).or_insert_with(|| Some(root.clone()));
            }
            self.rust_dependencies.insert(directory.to_string(), names);
            self.rust_left_out.insert(directory.to_string(), left_out);
        }
        manifests
    }

    fn has_mapping(&self, language: Option<&str>) -> bool {
        match language {
            Some("php") => !self.php.is_empty(),
            Some("typescript") => {
                !self.javascript_paths.is_empty() || self.javascript_base_url.is_some()
            }
            Some("rust") => !self.rust_crates.is_empty(),
            _ => false,
        }
    }

    fn has_same_mapping(&self, other: &Roots, language: Option<&str>) -> bool {
        match language {
            Some("php") => self.php == other.php,
            Some("typescript") => {
                self.javascript_paths == other.javascript_paths
                    && self.javascript_base_url == other.javascript_base_url
            }
            Some("rust") => {
                self.rust_crates == other.rust_crates
                    && self.resolving_names() == other.resolving_names()
                    && self.rust_left_out == other.rust_left_out
            }
            _ => true,
        }
    }

    /// The package names that move where a Rust path resolves: the names
    /// that reach a library of the repository. A new dependency from outside
    /// the repository moves none.
    fn resolving_names(&self) -> Vec<(&str, &str, &str)> {
        self.rust_dependencies
            .iter()
            .flat_map(|(directory, names)| {
                names
                    .iter()
                    .filter_map(move |(name, root)| Some((directory.as_str(), name.as_str(), root.as_deref()?)))
            })
            .collect()
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

fn canonical_source(source: Option<&str>) -> String {
    let source = source.unwrap_or("crates-io");
    let source = source.strip_suffix('/').unwrap_or(source);
    let canonical = match source.split_once("://") {
        Some((scheme, address)) => {
            let (authority, path) = address.split_once('/').unwrap_or((address, ""));
            let (user, host) = authority.split_at(authority.rfind('@').map_or(0, |at| at + 1));
            let host = host.to_lowercase();
            let (scheme, path) = match host.split_once(':').map_or(host.as_str(), |(host, _)| host) {
                "github.com" => ("https".to_string(), path.to_lowercase()),
                _ => (scheme.to_lowercase(), path.to_string()),
            };
            format!("{scheme}://{user}{host}/{}", path.strip_suffix(".git").unwrap_or(&path))
        }
        None => source.to_string(),
    };
    match canonical.as_str() {
        "https://github.com/rust-lang/crates.io-index" => "crates-io".to_string(),
        _ => canonical,
    }
}

#[derive(Serialize)]
struct SymbolsEntry<'a> {
    table: &'a str,
    symbols: BTreeMap<&'a str, (Vec<u32>, Vec<u32>)>,
}

#[derive(Serialize)]
struct ImportsEntry<'a> {
    table: &'a str,
    imports: Vec<(&'a [extraction::Import], &'a [InlineModule], &'a [DeclaredType])>,
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

    /// The import rows `file` was extracted with.
    fn imports_of(&self, file: &str) -> &[extraction::Import] {
        self.imports
            .get_or_init(|| self.load_imports())
            .get(file)
            .map_or(&[], |(imports, _, _)| imports.as_slice())
    }

    /// The inline `mod`s `file` declares.
    fn inline_modules_of(&self, file: &str) -> &[InlineModule] {
        self.imports
            .get_or_init(|| self.load_imports())
            .get(file)
            .map_or(&[], |(_, inline_modules, _)| inline_modules.as_slice())
    }

    /// The Rust types `file` declares.
    fn types_of(&self, file: &str) -> &[DeclaredType] {
        self.imports
            .get_or_init(|| self.load_imports())
            .get(file)
            .map_or(&[], |(_, _, types)| types.as_slice())
    }

    fn imports_mut(&mut self) -> &mut HashMap<Arc<str>, ModuleRows> {
        self.imports.get_or_init(|| self.load_imports());
        self.imports.get_mut().expect("imports cell initialized")
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
        self.imports_mut().remove(file);
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
                module_files: facts
                    .extraction
                    .as_ref()
                    .map(|extraction| module_files(&facts.path, extraction))
                    .unwrap_or_default(),
                inherits: facts.extraction.as_ref().map(inherits).unwrap_or_default(),
            },
        );
        let extraction = match &facts.extraction {
            Some(e) => e,
            None => return,
        };
        if let Some(rows) = module_rows(extraction, facts.language.as_deref()) {
            self.imports_mut().insert(Arc::clone(&path), rows);
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
    fn absorb_imports(&mut self, path: &str, targets: Vec<(String, &'static str, Option<String>)>) {
        let (importer, _) = self.intern(path);
        for (target, confidence, symbol) in targets {
            if target == path {
                continue;
            }
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
            let resolution = self.resolve_import(importer_path, import, language, modules, imports);
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
        imports: &[extraction::Import],
    ) -> ImportResolution<'a> {
        // `from module import symbol` names the symbol's own file when
        // one exists, and the module otherwise — Python's package form,
        // and the reason a from-imported file must not read as having no
        // importers. Rust's `use crate::a::b` names `b`'s own file the same
        // way when `b` is a module. Other languages' named imports still
        // name the module written after `from`, not a child path made from
        // the symbol.
        let separator = match language {
            Some("python") => Some("."),
            Some("rust") => Some("::"),
            _ => None,
        };
        let combined = separator.and_then(|separator| {
            import
                .symbol
                .as_ref()
                .map(|symbol| format!("{}{separator}{}", import.module, symbol))
        });
        let combined_resolved = combined
            .and_then(|module| modules.resolve(&module, importer_path, language, imports))
            .unwrap_or_default();
        let php_combined = (language == Some("php"))
            .then(|| {
                import
                    .symbol
                    .as_ref()
                    .map(|symbol| format!("{}\\{}", import.module, symbol))
            })
            .flatten()
            .and_then(|module| modules.resolve(&module, importer_path, language, imports))
            .unwrap_or_default();
        let module_resolved = modules.resolve(&import.module, importer_path, language, imports);
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
            let written = import
                .symbol
                .as_ref()
                .map_or_else(|| import.module.clone(), |symbol| format!("{}::{symbol}", import.module));
            if let Some(libraries) = (language == Some("rust"))
                .then(|| modules.left_out_crates(&written, importer_path))
                .flatten()
            {
                resolved = libraries
                    .iter()
                    .flat_map(|library| modules.rust_path_within(&written, library).0)
                    .collect();
                CONFIDENCE_INFERRED
            } else if modules.roots.has_mapping(language) {
                let external = language != Some("rust") || modules.is_external(&written, importer_path);
                return ImportResolution {
                    invalid_bindings: if !external {
                        &[]
                    } else if import.locals.is_empty() {
                        import.symbol.as_slice()
                    } else {
                        &import.locals
                    },
                    targets: Vec::new(),
                };
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
        let edges = self.edges_entry();
        let slot: HashMap<&str, u32> = edges
            .files
            .iter()
            .enumerate()
            .map(|(i, p)| (*p, i as u32))
            .collect();
        let at = |p: &str| slot.get(p).copied();
        let imports: Vec<(&[extraction::Import], &[InlineModule], &[DeclaredType])> = edges
            .files
            .iter()
            .map(|p| (self.imports_of(p), self.inline_modules_of(p), self.types_of(p)))
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

        let table = self.file_table_hash.as_str();
        (
            edges,
            SymbolsEntry { table, symbols },
            ImportsEntry { table, imports },
        )
    }

    /// The edges entry alone, which is all a manifest change that moves no
    /// root mapping rewrites.
    fn edges_entry(&self) -> EdgesEntry<'_> {
        // `built_from` holds every indexed file, and no row can name a file
        // outside it: rows are absorbed per file and `forget` drops both.
        let paths: Vec<&str> = self.built_from.keys().map(|p| &**p).collect();
        let slot: HashMap<&str, u32> = paths
            .iter()
            .enumerate()
            .map(|(i, p)| (*p, i as u32))
            .collect();
        let at = |p: &str| slot.get(p).copied();
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
        EdgesEntry {
            files: paths,
            names: &self.names,
            built: self
                .built_from
                .values()
                .map(|b| {
                    (
                        b.key.as_str(),
                        b.language.as_deref(),
                        b.annotations.as_slice(),
                        b.module_files.as_slice(),
                        b.inherits.as_slice(),
                    )
                })
                .collect(),
            importers,
            roots: &self.roots,
            cargo_manifests: &self.cargo_manifests,
        }
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
            cargo_manifests: stored.cargo_manifests,
            ..Default::default()
        };
        for (i, (key, language, annotations, module_files, inherits)) in stored.built.into_iter().enumerate() {
            let file = Arc::clone(out.files.get(i)?);
            out.built_from.insert(
                file,
                Built {
                    key,
                    language,
                    annotations,
                    module_files,
                    inherits,
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
            cache::load_bytes(cache::NAMESPACE_FILE, symbols_key(), &self.repo_root)
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
        let _ = cache::save(cache::NAMESPACE_FILE, symbols_key(), &document, &self.repo_root);
        symbols
    }

    /// Every indexed file's import rows, inline `mod`s, and Rust types: from
    /// the stored entry when its table matches this index, otherwise read
    /// back out of every file's per-file entry, which is the cost the stored
    /// entry exists to pay once. Loaded while `files` is still in stored
    /// order.
    fn load_imports(&self) -> HashMap<Arc<str>, ModuleRows> {
        let mut imports = HashMap::new();
        let table = &self.file_table_hash;
        if let Some(stored) =
            cache::load_bytes(cache::NAMESPACE_FILE, imports_key(), &self.repo_root)
                .as_deref()
                .and_then(|bytes| serde_json::from_slice::<StoredImports>(bytes).ok())
                .filter(|stored| stored.table == *table)
        {
            for (index, rows) in stored.imports.into_iter().enumerate() {
                let empty = rows.0.is_empty() && rows.1.is_empty() && rows.2.is_empty();
                if let (false, Some(file)) = (empty, self.files.get(index)) {
                    imports.insert(Arc::clone(file), rows);
                }
            }
            return imports;
        }
        let files: Vec<Arc<str>> = self.built_from.keys().cloned().collect();
        for chunk in files.chunks(file_facts::RESOLVE_CHUNK) {
            let paths: Vec<PathBuf> = chunk
                .iter()
                .map(|path| self.repo_root.join(&**path))
                .collect();
            let mut facts = file_facts::get_batch(&paths, &self.repo_root);
            for path in chunk {
                let Some(facts) = facts.remove(&**path) else {
                    continue;
                };
                if let Some(rows) = facts
                    .extraction
                    .as_ref()
                    .and_then(|extraction| module_rows(extraction, facts.language.as_deref()))
                {
                    imports.insert(Arc::clone(path), rows);
                }
            }
        }
        imports
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
struct ModulePaths<'a> {
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
    roots: &'a Roots,
    /// Each file a `#[path]` names, with the file whose `mod` item names it
    /// and that item's name.
    declared_by: HashMap<String, (String, String)>,
    /// The same `#[path]` modules keyed the other way, by the declaring
    /// file and the item's name.
    declared_files: HashMap<(String, String), String>,
    /// Each directory that holds a Rust crate root, with the roots it holds.
    crate_directories: HashMap<&'a Path, Vec<String>>,
    crate_members: OnceLock<HashMap<String, Vec<String>>>,
    relations: &'a Relations,
}

/// The final segment of a module path, past the last `.`, `/`, or `::`.
fn tail(module: &str) -> &str {
    module.rsplit(['.', '/', ':']).next().unwrap_or(module)
}

/// Of several readings of one Rust path, each the files it reaches and the
/// segments past them, the files of every reading that reaches a file with
/// the fewest segments left, and those segments.
fn deepest(readings: Vec<(Vec<String>, Vec<String>)>) -> Option<(Vec<String>, Vec<String>)> {
    let shortest = readings
        .iter()
        .filter(|(files, _)| !files.is_empty())
        .map(|(_, rest)| rest.len())
        .min()?;
    let (mut files, rest): (Vec<String>, Vec<String>) = readings
        .into_iter()
        .filter(|(files, rest)| !files.is_empty() && rest.len() == shortest)
        .fold((Vec::new(), Vec::new()), |(mut files, _), (reached, rest)| {
            files.extend(reached);
            (files, rest)
        });
    files.sort();
    files.dedup();
    Some((files, rest))
}

impl<'a> ModulePaths<'a> {
    /// The module paths of every file `relations` indexes, read through its
    /// root mappings, its `#[path]` modules, and its stored import rows.
    fn new(relations: &'a Relations) -> Self {
        let files = &relations.built_from;
        let mut entries = Vec::with_capacity(files.len());
        let mut exact: HashMap<String, Vec<usize>> = HashMap::with_capacity(files.len());
        let mut tails: HashMap<String, Vec<usize>> = HashMap::with_capacity(files.len());
        let mut declared_by = HashMap::new();
        let mut declared_files = HashMap::new();
        for (path, built) in files {
            let language = built.language.as_deref();
            let mut module = file_to_module(path, language);
            if language == Some("php") {
                module = module.to_lowercase();
            }
            let position = entries.len();
            exact.entry(module.clone()).or_default().push(position);
            tails
                .entry(tail(&module).to_string())
                .or_default()
                .push(position);
            entries.push((module, path.to_string(), built.language.clone()));
            for (name, file) in &built.module_files {
                declared_by.insert(file.clone(), (path.to_string(), name.clone()));
                declared_files.insert((path.to_string(), name.clone()), file.clone());
            }
        }
        let mut crate_directories: HashMap<&Path, Vec<String>> = HashMap::new();
        for root in relations.roots.rust_crates.keys() {
            if let Some(directory) = Path::new(root).parent() {
                crate_directories.entry(directory).or_default().push(root.clone());
            }
        }
        Self {
            entries,
            exact,
            tails,
            roots: &relations.roots,
            declared_by,
            declared_files,
            crate_directories,
            crate_members: OnceLock::new(),
            relations,
        }
    }

    /// The compatible-language files an import may name. Explicit relative
    /// imports are first made repository-relative from the importing file,
    /// and a Rust path tries the importing file's `imports` globs before it
    /// names no file; suffix fallback retains every honest candidate
    /// rather than choosing whichever file happened to be discovered first.
    fn resolve(
        &self,
        module_path: &str,
        importer_path: &str,
        language: Option<&str>,
        imports: &[extraction::Import],
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
        if language == Some("rust") && self.roots.has_mapping(language) {
            return Some(self.rust_item_path(module_path, importer_path, imports).0);
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
        self.relations
            .built_from
            .get(path)
            .filter(|built| built.language.as_deref() == language)
            .map(|_| vec![path.to_string()])
            .unwrap_or_default()
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

    /// The files a Rust path names from `file`, whose `use` items are
    /// `imports`, and the segments of it past the deepest module file it
    /// reaches, so it names a module when none are left. `crate`,
    /// `self`, and `super` start in the file's own crate; another first
    /// segment is a child module of the file, else the library a crate name
    /// of its package reaches, or for a file no package holds a library of
    /// the repository by name, else a module `X::<first>` that a `use X::*`
    /// brings in, else no file at all, which leaves every segment. The path
    /// descends module by module, `a/b.rs` or `a/b/mod.rs`, and names the
    /// deepest module file it reaches, so `crate::summary::Facts` names
    /// `summary.rs` and leaves `Facts`. When the first segment left names no
    /// module file, a `use` at the top level of a file reached that binds it
    /// leads one hop on, the way the walk follows a last segment, so after
    /// `pub use engine::config;` in `lib.rs`, `crate::config::Builder` and
    /// the module path `crate::config` both name `engine/config.rs`.
    fn rust_path(&self, path: &str, file: &str, imports: &[extraction::Import]) -> (Vec<String>, Vec<String>) {
        let (files, rest) = self.descend(path, file, imports);
        self.through_use(files, rest)
    }

    /// [`ModulePaths::rust_path`] for a path whose last segment is an item,
    /// a `use` row's or a type path's, which the walk follows itself, so
    /// only a segment with another after it takes the hop.
    fn rust_item_path(&self, path: &str, file: &str, imports: &[extraction::Import]) -> (Vec<String>, Vec<String>) {
        let (files, rest) = self.descend(path, file, imports);
        if rest.len() < 2 {
            return (files, rest);
        }
        self.through_use(files, rest)
    }

    /// The files and the segments left past them, one hop on through a
    /// `use` at the top level of a file of `files` that binds the first
    /// segment of `rest`, when the hop leaves fewer segments or reaches
    /// another file, such as a module another crate declares inline.
    fn through_use(&self, files: Vec<String>, rest: Vec<String>) -> (Vec<String>, Vec<String>) {
        let Some(first) = rest.first() else {
            return (files, rest);
        };
        let mut hops = Vec::new();
        for reached in &files {
            for import in self.relations.imports_of(reached) {
                let Some(symbol) = import.symbol.as_deref().filter(|_| import.block.is_none()) else {
                    continue;
                };
                if import.binding() != Some(first.as_str()) {
                    continue;
                }
                let bound: Vec<&str> = [import.module.as_str(), symbol]
                    .into_iter()
                    .chain(rest[1..].iter().map(String::as_str))
                    .filter(|segment| !segment.is_empty())
                    .collect();
                let hop = self.descend(&bound.join("::"), reached, &[]);
                if hop.1.len() < rest.len() || hop.0.iter().any(|target| target != reached) {
                    hops.push(hop);
                }
            }
        }
        deepest(hops).unwrap_or((files, rest))
    }

    /// The files a Rust path names by its module files alone, before any
    /// `use` partway along it, and the segments past the deepest one. A
    /// first segment that is no child module of `file` reads through each
    /// `use` of `imports` that binds it, one hop, before a crate name,
    /// because in Rust a name in scope outranks the extern prelude.
    fn descend(&self, path: &str, file: &str, imports: &[extraction::Import]) -> (Vec<String>, Vec<String>) {
        let segments: Vec<String> = path
            .split("::")
            .filter(|segment| !segment.is_empty())
            .map(str::to_string)
            .collect();
        let own = self.crate_roots(file);
        let module = self.rust_module(file, &own);
        let (roots, base, rest) = match segments.first().map(String::as_str) {
            None => return (Vec::new(), Vec::new()),
            Some("crate") => (own, Vec::new(), &segments[1..]),
            Some("self") => (own, module, &segments[1..]),
            Some("super") => {
                let supers = segments.iter().take_while(|segment| *segment == "super").count();
                let Some(depth) = module.len().checked_sub(supers) else {
                    return (Vec::new(), segments);
                };
                (own, module[..depth].to_vec(), &segments[supers..])
            }
            Some(first) => {
                if !self.rust_module_files(&own, &[module.as_slice(), &segments[..1]].concat()).is_empty() {
                    (own, module, &segments[..])
                } else {
                    let bound = path_after_binding(path, imports)
                        .iter()
                        .map(|bound| self.descend(bound, file, &[]))
                        .collect();
                    if let Some(found) = deepest(bound) {
                        return found;
                    }
                    let named = self.crates_named(first, file);
                    if named.is_empty() {
                        return imports
                            .iter()
                            .filter(|import| import.symbol.is_none())
                            .find(|glob| {
                                let (files, rest) = self.rust_path(&format!("{}::{first}", glob.module), file, &[]);
                                files.iter().any(|reached| named_module(reached, &rest, self.relations).is_some())
                            })
                            .map_or((Vec::new(), segments), |glob| {
                                self.descend(&format!("{}::{path}", glob.module), file, &[])
                            });
                    }
                    (named, Vec::new(), &segments[1..])
                }
            }
        };
        for taken in (0..=rest.len()).rev() {
            let files = self.rust_module_files(&roots, &[base.as_slice(), &rest[..taken]].concat());
            if !files.is_empty() {
                return (files, rest[taken..].to_vec());
            }
        }
        (Vec::new(), segments)
    }

    /// The crate roots `crate::` names from `file`: the file itself when it
    /// is a root, else the roots whose directory holds it most closely. A
    /// file no root's directory holds, such as a test, an example, or a
    /// build script, is a crate root itself.
    fn crate_roots(&self, file: &str) -> Vec<String> {
        if self.roots.rust_crates.contains_key(file) {
            return vec![file.to_string()];
        }
        Path::new(file)
            .ancestors()
            .skip(1)
            .find_map(|directory| self.crate_directories.get(directory))
            .cloned()
            .unwrap_or_else(|| vec![file.to_string()])
    }

    fn crate_files(&self, file: &str) -> Vec<String> {
        let members = self.crate_members.get_or_init(|| {
            let mut members: HashMap<String, Vec<String>> = HashMap::new();
            for (_, member, language) in &self.entries {
                if language.as_deref() == Some("rust") {
                    for root in self.crate_roots(member) {
                        members.entry(root).or_default().push(member.clone());
                    }
                }
            }
            members
        });
        self.crate_roots(file)
            .iter()
            .filter_map(|root| members.get(root))
            .flatten()
            .cloned()
            .collect()
    }

    /// The library roots `name` reaches from `file`: the root its package's
    /// name `name` reaches, and none for a name its package does not give or
    /// that reaches a crate outside the repository, because Cargo gives a
    /// package's code no other crate. For a file no package holds, the
    /// libraries of the repository named `name` nearest `file`.
    fn crates_named(&self, name: &str, file: &str) -> Vec<String> {
        if let Some(names) = self.package_names(file) {
            return names.get(name).cloned().flatten().into_iter().collect();
        }
        let shared = |root: &str| {
            Path::new(root)
                .components()
                .zip(Path::new(file).components())
                .take_while(|(root, file)| root == file)
                .count()
        };
        let named: Vec<&String> = self
            .roots
            .rust_crates
            .iter()
            .filter(|(_, library)| library.as_deref() == Some(name))
            .map(|(root, _)| root)
            .collect();
        let nearest = named.iter().map(|root| shared(root)).max();
        named
            .into_iter()
            .filter(|root| Some(shared(root)) == nearest)
            .cloned()
            .collect()
    }

    /// Whether a Rust module file of the repository is named `name`: a file
    /// whose stem is `name`, or the `mod.rs` of a directory named `name`.
    fn has_module_file(&self, name: &str) -> bool {
        let rust = |position: &usize| {
            self.entries
                .get(*position)
                .filter(|(_, _, language)| language.as_deref() == Some("rust"))
        };
        self.tails.get(name).into_iter().flatten().any(|position| rust(position).is_some())
            || self
                .tails
                .get("mod")
                .into_iter()
                .flatten()
                .filter_map(rust)
                .any(|(module, _, _)| module.strip_suffix("/mod").is_some_and(|directory| tail(directory) == name))
    }

    /// Whether a Rust path written in `file` is proven to live outside the
    /// repository: its first segment is a crate of the standard library, or
    /// a name of `file`'s package that reaches no library of the repository.
    fn is_external(&self, path: &str, file: &str) -> bool {
        let Some(first) = path.split("::").find(|segment| !segment.is_empty()) else {
            return false;
        };
        matches!(first, "std" | "core" | "alloc" | "proc_macro" | "test")
            || self
                .package_names(file)
                .and_then(|names| names.get(first))
                .is_some_and(Option::is_none)
    }

    /// The crate names of the Cargo package whose directory holds `file`
    /// most closely.
    fn package_names(&self, file: &str) -> Option<&BTreeMap<String, Option<String>>> {
        nearest_package(&self.roots.rust_dependencies, file)
    }

    /// The library roots a Rust path written in `file` may reach when its
    /// first segment is a name `file`'s package leaves out.
    fn left_out_crates(&self, path: &str, file: &str) -> Option<&[String]> {
        let first = path.split("::").find(|segment| !segment.is_empty())?;
        nearest_package(&self.roots.rust_left_out, file)?.get(first).map(Vec::as_slice)
    }

    /// The files a Rust path names inside the library whose root is
    /// `library`, its first segment read as that library's `crate`, and the
    /// segments of it past the deepest module file it reaches there.
    fn rust_path_within(&self, path: &str, library: &str) -> (Vec<String>, Vec<String>) {
        let rest = path.trim_start_matches("::").split_once("::").map_or("", |(_, rest)| rest);
        self.rust_path(&format!("crate::{rest}"), library, &[])
    }

    /// The files of module `segments` below the directory of `roots`: the
    /// file a `#[path]` names for the last segment in the files of the ones
    /// before it, else the file its path below their directory names.
    fn rust_module_files(&self, roots: &[String], segments: &[String]) -> Vec<String> {
        let Some((last, parent)) = segments.split_last() else {
            return roots.to_vec();
        };
        if !self.declared_files.is_empty() {
            let declared: Vec<String> = self
                .rust_module_files(roots, parent)
                .into_iter()
                .filter_map(|declaring| self.declared_files.get(&(declaring, last.clone())).cloned())
                .collect();
            if !declared.is_empty() {
                return declared;
            }
        }
        let mut files: Vec<String> = roots
            .iter()
            .map(|root| Path::new(root).parent().unwrap_or(Path::new("")).join(segments.join("/")))
            .flat_map(|module| [module.join("mod"), module])
            .filter_map(|module| self.exact.get(module.to_str()?))
            .flatten()
            .filter_map(|position| self.entries.get(*position))
            .filter(|(_, _, language)| language.as_deref() == Some("rust"))
            .map(|(_, file, _)| file.clone())
            .collect();
        files.sort();
        files.dedup();
        files
    }

    /// The module path of `file` in the crate whose roots are `roots`. A
    /// file a `#[path]` names is a child of the file that declares it, by
    /// that `mod` item's name. Any other file is its path below their
    /// directory, where `mod.rs` names its own directory.
    fn rust_module(&self, file: &str, roots: &[String]) -> Vec<String> {
        let mut names = Vec::new();
        let mut file = file;
        while let Some((declaring, name)) = self.declared_by.get(file).filter(|_| names.len() < self.declared_by.len()) {
            names.push(name.clone());
            file = declaring;
        }
        let mut module = if roots.iter().any(|root| root == file) {
            Vec::new()
        } else {
            let directory = roots
                .first()
                .and_then(|root| Path::new(root).parent())
                .unwrap_or(Path::new(""));
            let mut module: Vec<String> = Path::new(file)
                .strip_prefix(directory)
                .unwrap_or(Path::new(file))
                .with_extension("")
                .iter()
                .map(|segment| segment.to_string_lossy().to_string())
                .collect();
            if module.last().map(String::as_str) == Some("mod") {
                module.pop();
            }
            module
        };
        module.extend(names.into_iter().rev());
        module
    }
}

/// The entry of the Cargo package whose directory holds `file` most closely.
fn nearest_package<'a, T>(packages: &'a BTreeMap<String, T>, file: &str) -> Option<&'a T> {
    packages
        .iter()
        .filter(|(directory, _)| file.starts_with(directory.as_str()))
        .max_by_key(|(directory, _)| directory.len())
        .map(|(_, entry)| entry)
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
/// carries a fingerprint. It lives in its schema's directory because the rows
/// describe extraction output, which a schema bump can reshape.
fn edges_key() -> &'static str {
    "relations_edges_v1"
}

fn symbols_key() -> &'static str {
    "relations_symbols_v1"
}

fn imports_key() -> &'static str {
    "relations_imports_v1"
}

fn directories_key() -> &'static str {
    "relations_directories_v1"
}

fn load_directory_metrics(repo_root: &Path) -> Option<DirectoryIndex> {
    cache::load_bytes(cache::NAMESPACE_FILE, directories_key(), repo_root)
        .as_deref()
        .and_then(|bytes| serde_json::from_slice(bytes).ok())
}

fn save_directory_metrics(index: &DirectoryIndex, repo_root: &Path) {
    let _ = cache::save(cache::NAMESPACE_FILE, directories_key(), index, repo_root);
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
    cache::load_bytes(cache::NAMESPACE_FILE, edges_key(), repo_root)
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
    let (roots, manifests) = Roots::read(repo_root, &relations.cargo_manifests);
    if stale(&relations, &hashes).is_none()
        && relations.roots == roots
        && relations.cargo_manifests == manifests
    {
        return relations;
    }

    // One maintainer at a time. The holder may have absorbed exactly this
    // change while we waited, so the index is read again under the lock and
    // compared afresh: what it left is what remains. A file that moves
    // between the hash sweep and here is absorbed under its earlier key,
    // which the next call sees as moved again and corrects.
    let _lock = cache::maintain(repo_root);
    let mut relations = stored(repo_root);
    let (roots, manifests) = Roots::read(repo_root, &relations.cargo_manifests);
    let manifests_changed = relations.cargo_manifests != manifests;
    relations.cargo_manifests = manifests;
    let roots_changed = ["php", "typescript", "rust"]
        .into_iter()
        .any(|language| !relations.roots.has_same_mapping(&roots, Some(language)));
    let (gone, moved) = stale(&relations, &hashes).unwrap_or_default();
    if gone.is_empty() && moved.is_empty() && !roots_changed {
        let roots_moved = relations.roots != roots;
        relations.roots = roots;
        if manifests_changed || roots_moved {
            let _ = cache::save(cache::NAMESPACE_FILE, edges_key(), &relations.edges_entry(), repo_root);
        }
        return relations;
    }

    // A content change re-absorbs the files that moved. When a file appears
    // or disappears, or a touched file's declarations changed, module-path
    // and symbol-fallback resolution can shift for importers that never
    // changed, so the importers whose rows name an affected module tail or
    // symbol are re-resolved from their stored import rows. A changed root
    // mapping re-resolves the rows of every file in its language. Nothing
    // but the touched files is read back.
    let added: Vec<String> = moved
        .iter()
        .filter(|path| !relations.built_from.contains_key(path.as_str()))
        .cloned()
        .collect();
    relations.symbols.get_or_init(|| relations.load_symbols());
    relations.imports.get_or_init(|| relations.load_imports());
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
    // A Rust path reads a module from the `#[path]` items and, one hop, from
    // the top-level `use` items of the files it passes, so a touched file
    // whose either changed moves resolution in files that never changed.
    let bindings_of = |relations: &Relations, files: &HashSet<&str>| -> BTreeSet<(String, String, String, Option<String>)> {
        files
            .iter()
            .flat_map(|file| {
                relations
                    .imports_of(file)
                    .iter()
                    .filter(|import| import.block.is_none())
                    .filter_map(|import| {
                        Some((file.to_string(), import.binding()?.to_string(), import.module.clone(), import.symbol.clone()))
                    })
            })
            .collect()
    };
    let module_files_of = |relations: &Relations, files: &HashSet<&str>| -> BTreeSet<(String, String, String)> {
        files
            .iter()
            .filter_map(|file| relations.built_from.get(*file).map(|built| (file, built)))
            .flat_map(|(file, built)| {
                built.module_files.iter().map(|(name, target)| (file.to_string(), name.clone(), target.clone()))
            })
            .collect()
    };
    let bindings_before = bindings_of(&relations, &touched_files);
    let module_files_before = module_files_of(&relations, &touched_files);

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
            if stem == "index" || stem == "mod" {
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
    let rebound: HashSet<String> = bindings_before
        .symmetric_difference(&bindings_of(&relations, &moved_files))
        .map(|(_, binding, _, _)| binding.clone())
        .collect();
    let modules_moved = module_files_before != module_files_of(&relations, &moved_files);
    let affected: HashSet<Arc<str>> = relations
        .imports
        .get()
        .expect("imports loaded before the update")
        .iter()
        .filter(|(path, (rows, _, _))| {
            relations.importers.is_empty()
                || moved_files.contains::<str>(path.as_ref())
                || !relations.roots.has_same_mapping(&roots, relations.language(path))
                || (modules_moved && relations.language(path) == Some("rust"))
                || rows.iter().any(|row| {
                    let module = row.module.replace('\\', "/");
                    let end = tail(&module);
                    // A named import can resolve to the symbol's own file
                    // (Python's `from pkg import helper` reaching
                    // `pkg/helper.py`), so the symbol is checked as a stem
                    // as well as a name.
                    stems.contains(end)
                        || stems.contains(&end.to_lowercase())
                        || row.module.split("::").any(|segment| rebound.contains(segment))
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
    for list in relations.importers.values_mut() {
        list.retain(|importer| !affected.contains(&importer.file));
    }
    relations.importers.retain(|_, list| !list.is_empty());
    let resolved: Vec<_> = {
        let modules = ModulePaths::new(&relations);
        affected
            .iter()
            .map(|path| {
                let targets = relations.resolve_imports(path, relations.imports_of(path), relations.language(path), &modules);
                (Arc::clone(path), targets)
            })
            .collect()
    };
    for (path, targets) in resolved {
        relations.absorb_imports(&path, targets);
    }

    // The table is hashed in stored order, which `from_bytes` reproduces;
    // `files` itself appends each newly interned path at the end.
    relations.file_table_hash = table_hash(relations.built_from.keys().map(|path| &**path));
    let (edges, symbols, imports) = relations.stored_forms();
    let _ = cache::save(cache::NAMESPACE_FILE, symbols_key(), &symbols, repo_root);
    let _ = cache::save(cache::NAMESPACE_FILE, imports_key(), &imports, repo_root);
    let _ = cache::save(cache::NAMESPACE_FILE, edges_key(), &edges, repo_root);
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
/// that declares it and the inline `mod` that holds it. Language is a
/// per-file fact, so it sits beside the declaration rather than being copied
/// into every one.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub file: String,
    pub language: Option<String>,
    pub inline_module: Option<String>,
    pub is_file_local: bool,
    pub declaration: Declaration,
}

/// Every declaration of `name` in the file at `path`, as a candidate.
fn candidates_in<'a>(path: &'a str, facts: &'a FileFacts, name: &'a str) -> impl Iterator<Item = Candidate> + 'a {
    facts.extraction.iter().flat_map(move |extraction| {
        extraction
            .declarations
            .iter()
            .filter(move |declaration| same_name(facts.language.as_deref(), &declaration.name, name))
            .map(move |declaration| Candidate {
                file: path.to_string(),
                language: facts.language.clone(),
                inline_module: inline_module(extraction, declaration.line, Some(declaration)),
                is_file_local: (declared_with(declaration, "private") || declaration.name.starts_with('#'))
                    && !declaration
                        .parent
                        .and_then(|parent| extraction.declarations.get(parent as usize))
                        .is_some_and(|parent| declared_with(parent, "trait")),
                declaration: declaration.clone(),
            })
    })
}

fn declared_with(declaration: &Declaration, modifier: &str) -> bool {
    crate::surface::modifiers(&declaration.header, declaration.line - declaration.header_line, &declaration.name)
        .contains(&modifier)
}

/// The name of the innermost inline `mod` of `extraction` that holds
/// `line`, other than `declaration` itself.
fn inline_module(extraction: &ExtractionResult, line: i64, declaration: Option<&Declaration>) -> Option<String> {
    let modules: Vec<&Declaration> = extraction
        .declarations
        .iter()
        .filter(|module| {
            module.kind == "module" && !declaration.is_some_and(|declaration| std::ptr::eq(*module, declaration))
        })
        .collect();
    crate::surface::innermost(modules.iter().map(|module| (module.header_line, module.end_line)), line)
        .map(|index| modules[index].name.clone())
}

/// The inline `mod`s `extraction` declares: each `mod` with a body, whose
/// header elides it.
fn inline_modules(extraction: &ExtractionResult) -> Vec<InlineModule> {
    extraction
        .declarations
        .iter()
        .filter(|declaration| declaration.kind == "module" && declaration.header.ends_with("{ … }"))
        .map(|declaration| (declaration.name.clone(), (declaration.header_line, declaration.end_line)))
        .collect()
}

/// What the index keeps of one file's modules, or `None` when the file
/// imports and declares none of it.
fn module_rows(extraction: &ExtractionResult, language: Option<&str>) -> Option<ModuleRows> {
    let inline_modules = inline_modules(extraction);
    let types: Vec<DeclaredType> = extraction
        .declarations
        .iter()
        .filter(|declaration| {
            language == Some("rust") && matches!(declaration.kind.as_str(), "class" | "enum" | "interface")
        })
        .map(|declaration| (declaration.name.clone(), inline_module(extraction, declaration.line, Some(declaration))))
        .collect();
    (!extraction.imports.is_empty() || !inline_modules.is_empty() || !types.is_empty())
        .then(|| (extraction.imports.clone(), inline_modules, types))
}

/// The top-level `mod` items of the file at `path` whose `#[path]` names
/// their file, each with that file, read from the directory that holds
/// `path` the way Cargo reads it.
fn module_files(path: &str, extraction: &ExtractionResult) -> Vec<ModuleFile> {
    let directory = Path::new(path).parent().unwrap_or(Path::new("")).to_path_buf();
    extraction
        .declarations
        .iter()
        .filter(|declaration| inline_module(extraction, declaration.line, Some(declaration)).is_none())
        .filter_map(|declaration| {
            let file = normalized_path(directory.clone(), declaration.module_file.as_deref()?)?;
            Some((declaration.name.clone(), file.to_string_lossy().to_string()))
        })
        .collect()
}

/// Each class, interface, trait, and enum a PHP file declares that inherits
/// from another type, with each type it names and whether a `use` of the
/// file binds that name.
fn inherits(extraction: &ExtractionResult) -> Vec<Inherits> {
    extraction
        .declarations
        .iter()
        .filter(|declaration| declaration.container.is_none() && !declaration.supertypes.is_empty())
        .map(|declaration| {
            let supertypes = declaration
                .supertypes
                .iter()
                .map(|supertype| {
                    let bound = extraction
                        .imports
                        .iter()
                        .any(|import| import.binding().is_some_and(|binding| binding.eq_ignore_ascii_case(supertype)));
                    (supertype.clone(), bound)
                })
                .collect();
            (declaration.name.clone(), supertypes)
        })
        .collect()
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

/// Every declaration of `symbol`, loaded from the files the index names.
///
/// The index carries no payload, so kind, line, container and language come
/// from the declaring files' own entries — and those are exactly the files
/// the answer is about, so the load is the answer, not overhead.
///
/// `Class::method` and `Class.method` name one class's member: the member's
/// declarations, kept to the ones `qualifies` says the qualifier names.
pub fn declarations(symbol: &str, repo_root: &Path) -> Vec<Candidate> {
    candidates(&[symbol], repo_root)
        .iter()
        .flat_map(Candidates::declarations)
        .cloned()
        .collect()
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
fn qualifies(qualifier: Option<&str>, language: Option<&str>, file: &str, declaration: &Declaration) -> bool {
    let Some(qualifier) = qualifier else {
        return true;
    };
    match &declaration.container {
        Some(container) => container
            .rsplit(['\\', '.', ':'])
            .next()
            .is_some_and(|last| same_name(language, last, qualifier)),
        None => Path::new(file)
            .file_stem()
            .is_some_and(|stem| same_name(language, &stem.to_string_lossy(), qualifier)),
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

/// One symbol's name, its qualifier, and every declaration a reference to
/// the name could resolve to, loaded once so `use_sites` can resolve the
/// symbol in one file and then everywhere without loading its declaring
/// files again.
pub struct Candidates {
    qualifier: Option<String>,
    name: String,
    candidates: Vec<Candidate>,
    declared_modules: HashSet<String>,
}

impl Candidates {
    /// The candidates the symbol's qualifier names, the declarations
    /// `Class::method` or `module.function` means; every candidate of a bare
    /// name.
    pub fn declarations(&self) -> impl Iterator<Item = &Candidate> {
        self.candidates.iter().filter(|candidate| {
            qualifies(
                self.qualifier.as_deref(),
                candidate.language.as_deref(),
                &candidate.file,
                &candidate.declaration,
            )
        })
    }
}

/// The `Candidates` of each of `symbols`, in their order, loaded from the
/// files the index names, each name's in the order the index names them.
pub fn candidates(symbols: &[&str], repo_root: &Path) -> Vec<Candidates> {
    let relations = get(repo_root);
    let names: Vec<(Option<&str>, &str)> = symbols.iter().map(|symbol| split_qualified(symbol)).collect();
    let defined: Vec<Vec<&str>> = names.iter().map(|(_, name)| relations.defined_in(name).collect()).collect();
    let defining: Vec<&str> = defined.iter().flatten().copied().collect::<BTreeSet<&str>>().into_iter().collect();
    let mut loaded: Vec<Candidates> = names
        .iter()
        .map(|(qualifier, name)| Candidates {
            qualifier: qualifier.map(str::to_string),
            name: name.to_string(),
            candidates: Vec::new(),
            declared_modules: HashSet::new(),
        })
        .collect();
    for chunk in defining.chunks(file_facts::RESOLVE_CHUNK) {
        let declaring: Vec<PathBuf> = chunk.iter().map(|path| repo_root.join(*path)).collect();
        let facts = file_facts::get_batch(&declaring, repo_root);
        for path in chunk {
            let Some(f) = facts.get(*path) else {
                continue;
            };
            for (named, _) in loaded.iter_mut().zip(&defined).filter(|(_, defined)| defined.contains(path)) {
                named.declared_modules.extend(module_names(f.extraction.as_ref()).map(str::to_string));
                named.candidates.extend(candidates_in(path, f, &named.name));
            }
        }
    }
    for (named, defined) in loaded.iter_mut().zip(&defined) {
        named.candidates.sort_by_cached_key(|candidate| defined.iter().position(|file| *file == candidate.file));
    }
    loaded
}

/// Every resolved use site of each of `names`, in their order, computed now
/// from the files the index names rather than read out of a stored edge list.
///
/// Resolution stays structural: candidates are restricted to the same
/// language as the use site, a free call resolves only to non-method
/// declarations, a static use resolves to the named class or its method, and
/// a member call resolves to methods. Member calls remain ambiguous when the
/// receiver type is unknown; free and static calls also remain ambiguous when
/// an ambiguous import legitimately names multiple candidates. Resolution
/// runs only over the files the index names for this name.
///
/// A Rust member call's stated receiver type narrows its candidates and
/// never empties them: a call it leaves with no candidate resolves as one
/// with no stated type, `AMBIGUOUS` against every method of its name. A Rust
/// path left to the name rule whose last segment names no type of a
/// candidate resolves as the free call of a module path, when its first
/// segment names a module of the repository, against only the candidates in
/// a module the path names.
///
/// A qualified `Class::method` resolves against every `method` like a bare
/// name — so a call stays ambiguous when it is — and keeps the sites whose
/// target the qualifier names.
pub fn use_sites(names: &[Candidates], in_file: Option<&str>, repo_root: &Path) -> Vec<Vec<UseSite>> {
    let relations = get(repo_root);
    let in_scope = |file: &&str| in_file.is_none_or(|in_file| in_file == *file);
    let mut wanted: Vec<&str> = names
        .iter()
        .filter(|named| !named.candidates.is_empty())
        .flat_map(|named| relations.used_in(&named.name).filter(in_scope))
        .collect::<BTreeSet<&str>>()
        .into_iter()
        .collect();
    let mut sites: Vec<Vec<UseSite>> = vec![Vec::new(); names.len()];
    if wanted.is_empty() {
        return sites;
    }

    let modules = ModulePaths::new(&relations);
    let candidates: Vec<Vec<&Candidate>> = names.iter().map(|named| named.candidates.iter().collect()).collect();
    let crate_scoped = relations.roots.has_mapping(Some("rust"));
    let candidate_types: Vec<OnceLock<TypeOwners>> = names.iter().map(|_| OnceLock::new()).collect();

    // Each mentioning file resolves against the same candidate set and
    // nothing else, so the files run in parallel. On laravel-framework
    // `Collection` is named in over a thousand files, and resolving them one
    // after another was the whole cost of the command.
    //
    // A chunk at a time, like every other whole-repository walk: a name used
    // in thousands of files held every one of their extractions at once, and
    // a use site is finished the moment its file is resolved.
    wanted.sort();
    let php = Some("php");
    let mut return_types = ReturnTypes::default();
    let ancestry = &Ancestry::new(&relations);
    let lineages: Vec<OnceLock<Vec<Vec<(String, Vec<&str>)>>>> = names.iter().map(|_| OnceLock::new()).collect();
    for chunk in wanted.chunks(file_facts::RESOLVE_CHUNK) {
        let needed: Vec<PathBuf> = chunk.iter().map(|p| repo_root.join(*p)).collect();
        let facts = file_facts::get_batch(&needed, repo_root);
        let receivers: Vec<String> = facts
            .iter()
            .filter(|(_, f)| f.language.as_deref() == php)
            .flat_map(|(path, f)| {
                let test_case = relations.roots.test_case_of(path);
                f.extraction
                    .iter()
                    .flat_map(|extraction| extraction.references.iter())
                    .filter(|reference| names.iter().any(|named| same_name(php, &reference.name, &named.name)))
                    .filter_map(move |reference| {
                        bound_this(reference, test_case).map_or_else(|| reference.receiver.clone(), |bound| bound.receiver)
                    })
            })
            .filter(|receiver| receiver.contains("->"))
            .collect();
        let chains: Vec<&str> = receivers.iter().map(String::as_str).collect();
        return_types.read(&chains, &relations, &modules, repo_root);
        let return_types = &return_types;
        let resolved: Vec<Vec<Vec<UseSite>>> = chunk
            .par_iter()
            .map(|path| {
                let mut outs: Vec<Vec<UseSite>> = vec![Vec::new(); names.len()];
                let Some(f) = facts.get(*path) else {
                    return outs;
                };
                let Some(extraction) = &f.extraction else {
                    return outs;
                };
                let import_resolutions: Vec<ImportResolution<'_>> = extraction
                    .imports
                    .iter()
                    .map(|import| {
                        relations.resolve_import(path, import, f.language.as_deref(), &modules, &extraction.imports)
                    })
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
                let mut type_paths = TypePaths::new();
                let is_php = f.language.as_deref() == php;
                let test_case = relations.roots.test_case_of(path);
                for (index, reference) in names.iter().enumerate().flat_map(|(index, named)| {
                    extraction
                        .references
                        .iter()
                        .filter(move |r| same_name(f.language.as_deref(), &r.name, &named.name))
                        .map(move |reference| (index, reference))
                }) {
                    let out = &mut outs[index];
                    let candidates = &candidates[index];
                    let candidate_types = &candidate_types[index];
                    let declared_modules = &names[index].declared_modules;
                    let rust_scoped = crate_scoped && f.language.as_deref() == Some("rust");
                    let bound_outside = |reference: &extraction::Reference| {
                        import_resolutions.iter().zip(&extraction.imports).any(|(resolution, import)| {
                            resolution
                                .invalid_bindings
                                .iter()
                                .any(|binding| reference_uses_binding(f.language.as_deref(), reference, binding))
                                && import.in_force(reference.line, &extraction.imports)
                        })
                    };
                    let bound = is_php.then(|| bound_this(reference, test_case)).flatten();
                    let reference = bound.as_ref().unwrap_or(reference);
                    if (!rust_scoped || reference.shape == RefShape::Free) && bound_outside(reference) {
                        continue;
                    }
                    let returned = is_php
                        .then(|| return_types.stated(reference, &bound_outside, candidates, ancestry))
                        .flatten();
                    let reference = returned.as_ref().unwrap_or(reference);
                    let typed = (is_php && reference.shape == RefShape::Member)
                        .then(|| reference.receiver.as_deref())
                        .flatten()
                        .map(|class| (class.to_string(), ancestry.named_in(path, class, &extraction.imports)));
                    let all_candidates = candidates;
                    let inherited =
                        is_php.then(|| ancestry.inherited(path, &extraction.imports, reference, candidates)).flatten();
                    let (reference, candidates) = match &inherited {
                        Some((inherited, reached)) => (inherited, reached),
                        None => (reference, candidates),
                    };
                    let imports: Vec<extraction::Import> = extraction
                        .imports
                        .iter()
                        .filter(|import| import.in_force(reference.line, &extraction.imports))
                        .cloned()
                        .collect();
                    if let Some(scope) = rust_scoped
                        .then(|| {
                            let inline_module = inline_module(extraction, reference.line, None);
                            crate_scope(
                                reference,
                                path,
                                inline_module.as_deref(),
                                &imports,
                                &modules,
                                &relations,
                                candidates,
                                candidate_types,
                                &mut type_paths,
                            )
                        })
                        .flatten()
                    {
                        let before = out.len();
                        let matched = resolve_one(
                            &scope.candidates,
                            |_| true,
                            path,
                            f,
                            extraction,
                            &scope.reference,
                            reference,
                            &scope.reached,
                            out,
                        );
                        cap(&mut out[before..], scope.ceiling);
                        if matched || (scope.external && reference.shape != RefShape::Member) {
                            continue;
                        }
                    }
                    let before = out.len();
                    let language = f.language.as_deref();
                    let in_calling_file: Vec<&Candidate> = candidates
                        .iter()
                        .copied()
                        .filter(|candidate| {
                            !rust_scoped
                                && reference.shape == RefShape::Free
                                && candidate.file == *path
                                && shape_matches(candidate, language, RefShape::Free, None)
                        })
                        .collect();
                    let preferred = if in_calling_file.len() == 1 { &in_calling_file } else { candidates };
                    let matched =
                        resolve_one(preferred, |_| true, path, f, extraction, reference, reference, &imported, out);
                    let without_receiver = match reference.shape {
                        RefShape::Member if untyped_member_is_ambiguous(language) => Some(RefShape::Member),
                        RefShape::Member if inside_receiver_type(extraction, reference, language) => {
                            Some(RefShape::Member)
                        }
                        RefShape::Static
                            if paths_name_modules(language)
                                && reference.receiver.as_deref().is_some_and(|receiver| {
                                    starts_at_module(receiver, declared_modules, extraction, &modules)
                                }) =>
                        {
                            Some(RefShape::Free)
                        }
                        _ => None,
                    };
                    if let Some(shape) = without_receiver.filter(|_| !matched && reference.receiver.is_some()) {
                        let written = reference.receiver.as_deref().unwrap_or_default();
                        let named_modules: Vec<(Vec<String>, Vec<String>)> = if crate_scoped && shape == RefShape::Free {
                            let bound = path_after_binding(written, &imports);
                            let paths = if bound.is_empty() { vec![written.to_string()] } else { bound };
                            paths.iter().map(|bound| modules.rust_path(bound, path, &imports)).collect()
                        } else {
                            vec![(Vec::new(), Vec::new())]
                        };
                        let named = |candidate: &Candidate| {
                            shape != RefShape::Free
                                || named_modules.iter().any(|(named_files, rest)| {
                                    in_named_module(candidate, written, named_files, rest, &relations)
                                })
                        };
                        let from_root = shape == RefShape::Free
                            && matches!(
                                written.split("::").find(|segment| !segment.is_empty()),
                                Some("crate" | "self" | "super")
                            );
                        let admitted: Vec<&Candidate> =
                            candidates.iter().copied().filter(|candidate| from_root || named(candidate)).collect();
                        let retry = extraction::Reference {
                            shape,
                            receiver: None,
                            ..reference.clone()
                        };
                        resolve_one(&admitted, named, path, f, extraction, &retry, reference, &imported, out);
                    }
                    if rust_scoped {
                        cap(&mut out[before..], CONFIDENCE_INFERRED);
                    }
                    if let Some((class, declaring)) = typed {
                        let caller = caller_at(extraction, reference.line);
                        let lineages = lineages[index].get_or_init(|| ancestry.lineages(all_candidates));
                        for candidate in overrides(&class, &declaring, all_candidates, lineages) {
                            if !is_recursion(path, caller.as_ref(), candidate) {
                                out.push(UseSite {
                                    file: path.to_string(),
                                    line: reference.line,
                                    caller: caller.clone(),
                                    target_file: candidate.file.clone(),
                                    target: candidate.declaration.clone(),
                                    confidence: CONFIDENCE_AMBIGUOUS,
                                });
                            }
                        }
                    }
                }
                for out in &mut outs {
                    out.sort_by(|a, b| {
                        (a.line, &a.target_file, a.target.line, &a.target.name, confidence_code(a.confidence)).cmp(&(
                            b.line,
                            &b.target_file,
                            b.target.line,
                            &b.target.name,
                            confidence_code(b.confidence),
                        ))
                    });
                    out.dedup_by(|later, kept| {
                        (later.line, &later.target_file, later.target.line, &later.target.name)
                            == (kept.line, &kept.target_file, kept.target.line, &kept.target.name)
                    });
                }
                outs
            })
            .collect();
        for outs in resolved {
            for (sites, out) in sites.iter_mut().zip(outs) {
                sites.extend(out);
            }
        }
    }
    for (sites, named) in sites.iter_mut().zip(names) {
        sites.retain(|site| {
            qualifies(named.qualifier.as_deref(), relations.language(&site.target_file), &site.target_file, &site.target)
        });
    }
    sites
}

fn reference_uses_binding(language: Option<&str>, reference: &extraction::Reference, binding: &str) -> bool {
    match reference.shape {
        RefShape::Free => same_name(language, &reference.name, binding),
        RefShape::Member | RefShape::Static => reference
            .receiver
            .as_deref()
            .is_some_and(|receiver| same_name(language, receiver, binding)),
    }
}

#[derive(Default)]
struct ReturnTypes(HashMap<(String, String), Option<String>>);

impl ReturnTypes {
    fn stated(
        &self,
        reference: &extraction::Reference,
        bound_outside: impl Fn(&extraction::Reference) -> bool,
        candidates: &[&Candidate],
        ancestry: &Ancestry,
    ) -> Option<extraction::Reference> {
        let receiver = reference.receiver.as_deref().filter(|receiver| receiver.contains("->"))?;
        let first = extraction::Reference {
            receiver: receiver.split("->").next().map(str::to_string),
            ..reference.clone()
        };
        let class = if bound_outside(&first) {
            None
        } else {
            self.class_of(receiver).ok().flatten()
        }
        .filter(|class| {
            class_declares(candidates, class)
                || ancestry
                    .ancestors(class, &ancestry.relations.defined_in(class).collect::<Vec<&str>>())
                    .iter()
                    .any(|(ancestor, _)| class_declares(candidates, ancestor))
        });
        Some(extraction::Reference {
            receiver: class,
            ..reference.clone()
        })
    }

    fn class_of(&self, receiver: &str) -> Result<Option<String>, (String, String)> {
        let mut steps = receiver.split("->");
        let mut class = steps.next().unwrap_or_default().to_string();
        for step in steps {
            let method = step.trim_end_matches("()");
            match self.0.get(&(class.to_ascii_lowercase(), method.to_ascii_lowercase())) {
                Some(Some(returned)) => class = returned.clone(),
                Some(None) => return Ok(None),
                None => return Err((class, method.to_string())),
            }
        }
        Ok(Some(class))
    }

    fn read(&mut self, receivers: &[&str], relations: &Relations, modules: &ModulePaths, repo_root: &Path) {
        loop {
            let unread: BTreeSet<(String, String)> =
                receivers.iter().filter_map(|receiver| self.class_of(receiver).err()).collect();
            if unread.is_empty() {
                return;
            }
            let declaring: Vec<PathBuf> = unread
                .iter()
                .flat_map(|(_, method)| relations.defined_in(method))
                .collect::<BTreeSet<&str>>()
                .into_iter()
                .map(|path| repo_root.join(path))
                .collect();
            let facts = file_facts::get_batch(&declaring, repo_root);
            for (class, method) in unread {
                let returned = declared_return(&class, &method, &facts, relations, modules);
                self.0.insert((class.to_ascii_lowercase(), method.to_ascii_lowercase()), returned);
            }
        }
    }
}

fn declared_return(
    class: &str,
    method: &str,
    facts: &HashMap<String, FileFacts>,
    relations: &Relations,
    modules: &ModulePaths,
) -> Option<String> {
    let php = Some("php");
    let mut returned = facts
        .iter()
        .filter(|(_, f)| f.language.as_deref() == php)
        .filter_map(|(path, f)| f.extraction.as_ref().map(|extracted| (path, extracted)))
        .flat_map(|(path, extracted)| {
            extracted
                .declarations
                .iter()
                .filter(|declaration| {
                    declaration.kind == "function"
                        && same_name(php, &declaration.name, method)
                        && declaration.container.as_deref().is_some_and(|container| same_name(php, container, class))
                })
                .map(move |declaration| {
                    extraction::php::returned_class(declaration).filter(|named| {
                        !extracted.imports.iter().any(|import| {
                            relations
                                .resolve_import(path, import, php, modules, &extracted.imports)
                                .invalid_bindings
                                .iter()
                                .any(|binding| same_name(php, binding, named))
                        })
                    })
                })
        });
    let first = returned.next()??;
    returned
        .all(|other| other.is_some_and(|other| same_name(php, &other, &first)))
        .then_some(first)
}

/// The PHP types the index's files declare, each with what it inherits from,
/// read from each file's [`Built`], so walking a lineage reads no file.
struct Ancestry<'r> {
    relations: &'r Relations,
    declared: HashMap<String, Vec<(&'r str, &'r [(String, bool)])>>,
}

impl<'r> Ancestry<'r> {
    fn new(relations: &'r Relations) -> Self {
        let mut declared: HashMap<String, Vec<(&'r str, &'r [(String, bool)])>> = HashMap::new();
        for (file, built) in &relations.built_from {
            for (name, supertypes) in &built.inherits {
                declared
                    .entry(name.to_ascii_lowercase())
                    .or_default()
                    .push((&**file, supertypes.as_slice()));
            }
        }
        Ancestry { relations, declared }
    }

    /// The files that declare the type `file` names `supertype`: the ones it
    /// imports when a `use` of it binds the name, else the ones in its own
    /// directory, its namespace under PSR-4, else every one. A `use` that
    /// reaches no file of the repository names a type outside it.
    fn declaring(&self, file: &str, supertype: &str, bound: bool) -> Vec<&'r str> {
        let declaring: Vec<&'r str> = self.relations.defined_in(supertype).collect();
        if bound {
            return declaring
                .into_iter()
                .filter(|declared| self.relations.importers_of(declared).iter().any(|importer| &*importer.file == file))
                .collect();
        }
        let directory = Path::new(file).parent();
        let beside: Vec<&'r str> = declaring
            .iter()
            .copied()
            .filter(|declared| Path::new(declared).parent() == directory)
            .collect();
        if beside.is_empty() { declaring } else { beside }
    }

    /// Each supertype the declarations of `class` in the files `keep` admits
    /// write, with the file that writes it, last first.
    fn supertypes_of(&self, class: &str, keep: impl Fn(&str) -> bool) -> Vec<(&'r str, &'r str, bool)> {
        let mut written: Vec<(&'r str, &'r str, bool)> = self
            .declared
            .get(&class.to_ascii_lowercase())
            .into_iter()
            .flatten()
            .filter(|(declared, _)| keep(declared))
            .flat_map(|&(declared, supertypes)| {
                supertypes.iter().map(move |(supertype, bound)| (declared, supertype.as_str(), *bound))
            })
            .collect();
        written.reverse();
        written
    }

    /// The files that declare the class a reference in `file` names `class`,
    /// read through the `use` items of the file.
    fn named_in(&self, file: &str, class: &str, imports: &[extraction::Import]) -> Vec<&'r str> {
        let bound = imports
            .iter()
            .any(|import| import.binding().is_some_and(|binding| binding.eq_ignore_ascii_case(class)));
        self.declaring(file, class, bound)
    }

    /// Every type the declarations of `class` in `files` inherit from,
    /// nearest first, each with the files that declare it, the way PHP looks
    /// a method up: its traits, then its parent and all the parent inherits,
    /// then its interfaces.
    fn ancestors(&self, class: &str, files: &[&str]) -> Vec<(String, Vec<&'r str>)> {
        let mut seen = HashSet::from([class.to_ascii_lowercase()]);
        let mut lineage = Vec::new();
        let mut stack = self.supertypes_of(class, |declared| files.contains(&declared));
        while let Some((written_in, supertype, bound)) = stack.pop() {
            let declaring = self.declaring(written_in, supertype, bound);
            if declaring.is_empty() || !seen.insert(supertype.to_ascii_lowercase()) {
                continue;
            }
            stack.extend(self.supertypes_of(supertype, |declared| declaring.contains(&declared)));
            lineage.push((supertype.to_string(), declaring));
        }
        lineage
    }

    /// A PHP member or static call in `file` on a class that declares no
    /// method of its name, read as a call on the nearest ancestor that does,
    /// with the methods of that ancestor it reaches.
    fn inherited<'c>(
        &self,
        file: &str,
        imports: &[extraction::Import],
        reference: &extraction::Reference,
        candidates: &[&'c Candidate],
    ) -> Option<(extraction::Reference, Vec<&'c Candidate>)> {
        let php = Some("php");
        let class = reference.receiver.as_deref()?;
        if reference.shape == RefShape::Free
            || candidates
                .iter()
                .any(|candidate| shape_matches(candidate, php, reference.shape, Some(class)))
        {
            return None;
        }
        self.ancestors(class, &self.named_in(file, class, imports))
            .into_iter()
            .find_map(|(ancestor, declaring)| {
                let reached: Vec<&'c Candidate> = candidates
                    .iter()
                    .copied()
                    .filter(|candidate| {
                        declaring.contains(&candidate.file.as_str())
                            && shape_matches(candidate, php, reference.shape, Some(&ancestor))
                    })
                    .collect();
                (!reached.is_empty()).then(|| {
                    let inherited = extraction::Reference {
                        receiver: Some(ancestor),
                        ..reference.clone()
                    };
                    (inherited, reached)
                })
            })
    }

    /// Each candidate's lineage: the types its PHP class inherits from.
    fn lineages(&self, candidates: &[&Candidate]) -> Vec<Vec<(String, Vec<&'r str>)>> {
        candidates
            .iter()
            .map(|candidate| match (candidate.language.as_deref(), &candidate.declaration.container) {
                (Some("php"), Some(container)) => self.ancestors(container, &[candidate.file.as_str()]),
                _ => Vec::new(),
            })
            .collect()
    }
}

/// The candidates a call on `class`, declared in `declaring`, may also reach
/// at run time: each method of a type whose lineage holds that class.
fn overrides<'c>(
    class: &str,
    declaring: &[&str],
    candidates: &[&'c Candidate],
    lineages: &[Vec<(String, Vec<&str>)>],
) -> Vec<&'c Candidate> {
    candidates
        .iter()
        .zip(lineages)
        .filter(|(_, lineage)| {
            lineage.iter().any(|(ancestor, files)| {
                same_name(Some("php"), ancestor, class) && files.iter().any(|file| declaring.contains(file))
            })
        })
        .map(|(candidate, _)| *candidate)
        .collect()
}

/// Whether `class` declares a method among `candidates`.
fn class_declares(candidates: &[&Candidate], class: &str) -> bool {
    let php = Some("php");
    candidates.iter().any(|candidate| {
        candidate.language.as_deref() == php
            && candidate
                .declaration
                .container
                .as_deref()
                .is_some_and(|container| same_name(php, container, class))
    })
}

/// A PHP reference whose receiver is `$this` outside every class, as in a
/// Pest test's closure, with `$this` read as the test case class its
/// directory binds, or as no class.
fn bound_this(reference: &extraction::Reference, test_case: Option<&str>) -> Option<extraction::Reference> {
    let rest = reference.receiver.as_deref()?.strip_prefix("$this")?;
    Some(extraction::Reference {
        receiver: test_case.map(|class| format!("{class}{rest}")),
        ..reference.clone()
    })
}

/// The innermost declaration of `extraction` whose span holds `line`.
fn caller_at(extraction: &ExtractionResult, line: i64) -> Option<Declaration> {
    crate::surface::innermost(
        extraction
            .declarations
            .iter()
            .map(|declaration| (declaration.header_line, declaration.end_line)),
        line,
    )
    .map(|index| extraction.declarations[index].clone())
}

/// A call whose calling declaration is the candidate itself is recursion and
/// adds no caller.
fn is_recursion(path: &str, caller: Option<&Declaration>, candidate: &Candidate) -> bool {
    candidate.file == path
        && caller.is_some_and(|caller| {
            caller.line == candidate.declaration.line && caller.name == candidate.declaration.name
        })
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Module {
    Every,
    TopLevel,
    Inline(String),
}

/// Whether `candidate` counts at a position that stands for `module`: its
/// innermost inline `mod` is that module.
fn admits(module: &Module, candidate: &Candidate) -> bool {
    in_module(module, candidate.inline_module.as_deref())
}

/// Whether a declaration whose innermost inline `mod` is `inline_module`
/// counts at a position that stands for `module`.
fn in_module(module: &Module, inline_module: Option<&str>) -> bool {
    match module {
        Module::Every => true,
        Module::TopLevel => inline_module.is_none(),
        Module::Inline(name) => inline_module == Some(name.as_str()),
    }
}

/// The module `rest`, the segments of a Rust path past `file`, the file it
/// reaches, names there: the inline `mod` the last of them names when each
/// names an inline `mod` `file` declares, the top level when there are
/// none, and `None` when one names a type.
fn named_module(file: &str, rest: &[String], relations: &Relations) -> Option<Module> {
    rest.iter()
        .all(|segment| relations.inline_modules_of(file).iter().any(|(name, _)| name == segment))
        .then(|| rest.last().map_or(Module::TopLevel, |name| Module::Inline(name.clone())))
}

/// A Rust reference's scope: the reference as its crate reads it, the
/// `candidates` in its scope, the files they sit in, each with the
/// confidence it is reached by, the strongest confidence the scope proves,
/// and whether the path is proven to live outside the repository, which
/// leaves a reference the scope cannot place unresolved.
struct Scope<'a> {
    reference: extraction::Reference,
    candidates: Vec<&'a Candidate>,
    reached: HashMap<String, &'static str>,
    ceiling: &'static str,
    external: bool,
}

/// One identity a Rust type can have: a declaration of the repository, a
/// module position the walk reached and found no declaration in, with the
/// name it looked for there, or a type outside the repository, by name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum TypeKey {
    Declared(String, String),
    Unreached(String, Module, String),
    Outside(String),
}

/// The identities a type path names, each with the confidence it is
/// reached by. An `Outside` identity is never more than `AMBIGUOUS`.
type ResolvedType = HashMap<TypeKey, &'static str>;

/// Each identity the candidates' own types name, with every candidate that
/// has it, by its position, and the confidence it has it at.
type TypeOwners = HashMap<TypeKey, Vec<(usize, &'static str)>>;

/// The type paths one file resolved, each by the inline `mod` it is written
/// in, its text, and the imports in force there, with what [`resolve_type`]
/// returned for it.
type TypePaths = HashMap<(Option<String>, String, Vec<extraction::Import>), (ResolvedType, bool)>;

/// A Rust reference as its crate reads it, or `None`, which leaves it to
/// the name rule: a member call with no stated receiver type. A path names
/// a module, where the call is a free call, or a type, where the call's
/// candidates are the methods whose `Self` resolves to the type the path
/// resolves to, in any file of the repository, by [`resolve_type`]. The
/// path's first segment may be a name each `use` of `imports`, the imports
/// in force at the reference, binds, or a module a `*` imports. A path whose
/// first segment is a name the file's package leaves out and whose rest
/// names a module of that library reaches every file of the libraries of the
/// packages of that name, each `INFERRED`.
#[allow(clippy::too_many_arguments)]
fn crate_scope<'a>(
    reference: &extraction::Reference,
    file: &str,
    inline_module: Option<&str>,
    imports: &[extraction::Import],
    modules: &ModulePaths,
    relations: &Relations,
    candidates: &[&'a Candidate],
    candidate_types: &OnceLock<TypeOwners>,
    type_paths: &mut TypePaths,
) -> Option<Scope<'a>> {
    let call_site = (
        file.to_string(),
        inline_module.map_or(Module::TopLevel, |name| Module::Inline(name.to_string())),
        true,
        reference.name.clone(),
    );
    let free = extraction::Reference {
        shape: RefShape::Free,
        receiver: None,
        ..reference.clone()
    };
    let starts = match reference.receiver.as_deref() {
        None if reference.shape == RefShape::Member => return None,
        None => vec![call_site],
        Some(written) => {
            let paths = bound_paths(written, imports);
            let modules_named: Vec<(String, Module, bool, String)> = paths
                .iter()
                .flat_map(|path| {
                    let (files, rest) = modules.rust_path(path, file, imports);
                    files.into_iter().map(move |target| position_of(path, file, target, &rest, relations))
                })
                .filter(|(_, _, type_path)| !type_path)
                .map(|(target, module, _)| (target, module, false, reference.name.clone()))
                .collect();
            if modules_named.is_empty() {
                let left_out: Vec<&String> = paths
                    .iter()
                    .filter_map(|path| modules.left_out_crates(path, file).map(|libraries| (path, libraries)))
                    .filter(|(path, libraries)| {
                        libraries.iter().any(|library| modules.rust_path_within(path, library).1.is_empty())
                    })
                    .flat_map(|(_, libraries)| libraries)
                    .collect();
                if left_out.is_empty() {
                    return Some(type_scope(
                        written,
                        reference,
                        file,
                        inline_module,
                        imports,
                        modules,
                        relations,
                        candidates,
                        candidate_types,
                        type_paths,
                    ));
                }
                let reached: HashMap<String, &'static str> = left_out
                    .into_iter()
                    .flat_map(|library| modules.crate_files(library))
                    .map(|member| (member, CONFIDENCE_INFERRED))
                    .collect();
                return Some(Scope {
                    candidates: candidates.iter().copied().filter(|candidate| reached.contains_key(&candidate.file)).collect(),
                    reference: free,
                    reached,
                    ceiling: CONFIDENCE_EXTRACTED,
                    external: false,
                });
            }
            modules_named
        }
    };
    let declares = |path: &str, module: &Module, name: &str| {
        candidates.iter().any(|candidate| {
            candidate.file == path
                && same_name(Some("rust"), &candidate.declaration.name, name)
                && admits(module, candidate)
                && shape_matches(candidate, Some("rust"), RefShape::Free, None)
        })
    };
    let walk = walk_modules(starts, imports, declares, modules, relations)?;
    Some(Scope {
        candidates: candidates
            .iter()
            .copied()
            .filter(|candidate| {
                walk.positions.iter().any(|(path, module, name)| {
                    *path == candidate.file
                        && admits(module, candidate)
                        && same_name(Some("rust"), &candidate.declaration.name, name)
                })
            })
            .collect(),
        reference: free,
        external: walk.external(),
        reached: walk.reached,
        ceiling: CONFIDENCE_EXTRACTED,
    })
}

/// The scope of a Rust call whose path names a type: the `candidates` whose
/// own type, by [`own_types`], shares an identity with the type the path
/// `written` resolves to where the reference is, each sitting in a file
/// reached by the weaker of the two sides' confidence. A type outside the
/// repository proves no target, so its candidates are at most `AMBIGUOUS`.
/// The path resolves once per file, read back from `type_paths` after that.
#[allow(clippy::too_many_arguments)]
fn type_scope<'a>(
    written: &str,
    reference: &extraction::Reference,
    file: &str,
    inline_module: Option<&str>,
    imports: &[extraction::Import],
    modules: &ModulePaths,
    relations: &Relations,
    candidates: &[&'a Candidate],
    candidate_types: &OnceLock<TypeOwners>,
    type_paths: &mut TypePaths,
) -> Scope<'a> {
    let (resolved, external) = type_paths
        .entry((inline_module.map(str::to_string), written.to_string(), imports.to_vec()))
        .or_insert_with(|| {
            crate::timing::phase("resolve_type", || resolve_type(written, file, inline_module, imports, modules, relations))
        });
    let owners = candidate_types.get_or_init(|| crate::timing::phase("own_types", || own_types(candidates, modules, relations)));
    let mut matched: BTreeMap<usize, &'static str> = BTreeMap::new();
    for (key, ours) in resolved.iter() {
        for (index, theirs) in owners.get(key).into_iter().flatten() {
            let confidence = weaker(ours, theirs);
            let held = matched.entry(*index).or_insert(confidence);
            if confidence_code(confidence) < confidence_code(held) {
                *held = confidence;
            }
        }
    }
    let mut reached: HashMap<String, &'static str> = HashMap::new();
    let mut in_scope = Vec::new();
    for (index, confidence) in matched {
        let candidate = candidates[index];
        let held = reached.entry(candidate.file.clone()).or_insert(confidence);
        if confidence_code(confidence) < confidence_code(held) {
            *held = confidence;
        }
        in_scope.push(candidate);
    }
    let ceiling = reached
        .values()
        .copied()
        .min_by_key(|confidence| confidence_code(confidence))
        .unwrap_or(CONFIDENCE_EXTRACTED);
    Scope {
        reference: extraction::Reference {
            shape: RefShape::Static,
            receiver: None,
            ..reference.clone()
        },
        candidates: in_scope,
        reached,
        ceiling,
        external: *external,
    }
}

/// The type a Rust type path `written` names where it is written, in
/// `file` at `inline_module` with `imports` in force, and whether the path
/// is proven to live outside the repository. A one-segment path walks from
/// where it is written, through the `use` items in force there, which a
/// `type` alias is one of; a longer one starts in the files its path names,
/// through each `use` that binds its first segment. The walk stops at a
/// position that declares the type by [`Relations::types_of`], and the
/// type's identities are those declarations. A walk that reaches none names
/// the module positions past a hop that it reached and found nothing in;
/// a walk that a hop proves external names a type outside the repository by
/// the name that hop leads to, and a walk that reaches neither by the path's
/// last segment.
fn resolve_type(
    written: &str,
    file: &str,
    inline_module: Option<&str>,
    imports: &[extraction::Import],
    modules: &ModulePaths,
    relations: &Relations,
) -> (ResolvedType, bool) {
    let mut outside_names = Vec::new();
    let mut inferred: HashSet<String> = HashSet::new();
    let mut starts = Vec::new();
    if written.contains("::") {
        for path in bound_paths(written, imports) {
            let (files, rest) = modules.rust_item_path(&path, file, imports);
            let reached: Vec<(Vec<String>, Vec<String>)> = if !files.is_empty() {
                vec![(files, rest)]
            } else if let Some(libraries) = modules.left_out_crates(&path, file) {
                inferred.extend(libraries.iter().flat_map(|library| modules.crate_files(library)));
                libraries.iter().map(|library| modules.rust_path_within(&path, library)).collect()
            } else {
                if modules.is_external(&path, file) {
                    outside_names.push(tail(&path).to_string());
                }
                Vec::new()
            };
            for (files, rest) in reached {
                for target in files {
                    let (target, module, _) = position_of(&path, file, target, &rest, relations);
                    starts.push((target, module, false, tail(&path).to_string()));
                }
            }
        }
    } else {
        starts.push((
            file.to_string(),
            inline_module.map_or(Module::TopLevel, |name| Module::Inline(name.to_string())),
            true,
            written.to_string(),
        ));
    }
    let declares = |path: &str, module: &Module, name: &str| {
        relations
            .types_of(path)
            .iter()
            .any(|(declared, inline_module)| declared == name && in_module(module, inline_module.as_deref()))
    };
    let outside = |names: Vec<String>| -> ResolvedType {
        let names = if names.is_empty() { vec![tail(written).to_string()] } else { names };
        names.into_iter().map(|name| (TypeKey::Outside(name), CONFIDENCE_AMBIGUOUS)).collect()
    };
    let Some(walk) = walk_modules(starts, imports, declares, modules, relations) else {
        let external = !outside_names.is_empty();
        return (outside(outside_names), external);
    };
    let declared: ResolvedType = walk
        .positions
        .iter()
        .filter(|(path, module, name)| declares(path, module, name))
        .map(|(path, _, name)| {
            let confidence = match inferred.contains(path) {
                true => CONFIDENCE_INFERRED,
                false => walk.reached.get(path).copied().unwrap_or(CONFIDENCE_EXTRACTED),
            };
            (TypeKey::Declared(path.clone(), name.clone()), confidence)
        })
        .collect();
    let external = !outside_names.is_empty() || walk.external();
    outside_names.extend(walk.outside);
    if !declared.is_empty() {
        return (declared, external);
    }
    if external || walk.dead_ends.is_empty() {
        return (outside(outside_names), external);
    }
    let unreached = walk
        .dead_ends
        .into_iter()
        .map(|(path, module, name)| (TypeKey::Unreached(path, module, name), CONFIDENCE_EXTRACTED))
        .collect();
    (unreached, external)
}

/// Each candidate's own type, the way [`resolve_type`] reads a type path
/// where the candidate is declared: a Rust method's `Self` and a Rust type
/// itself, and none for anything else, keyed by identity, so a type path
/// finds its candidates by its own identities. It runs inside the parallel
/// resolve of a `OnceLock`, so it runs on one thread: a rayon join here could
/// steal a task that waits on the same `OnceLock`.
fn own_types(candidates: &[&Candidate], modules: &ModulePaths, relations: &Relations) -> TypeOwners {
    let mut owners = TypeOwners::new();
    for (index, candidate) in candidates.iter().enumerate() {
        let declaration = &candidate.declaration;
        if candidate.language.as_deref() != Some("rust") {
            continue;
        }
        let own = if matches!(declaration.kind.as_str(), "class" | "enum" | "interface") {
            HashMap::from([(TypeKey::Declared(candidate.file.clone(), declaration.name.clone()), CONFIDENCE_EXTRACTED)])
        } else if let Some(self_type) = &declaration.self_type {
            let rows = relations.imports_of(&candidate.file);
            let imports: Vec<extraction::Import> =
                rows.iter().filter(|import| import.in_force(declaration.line, rows)).cloned().collect();
            resolve_type(self_type, &candidate.file, candidate.inline_module.as_deref(), &imports, modules, relations).0
        } else {
            continue;
        };
        for (key, confidence) in own {
            owners.entry(key).or_default().push((index, confidence));
        }
    }
    owners
}

/// The weaker of two confidences.
fn weaker(left: &'static str, right: &'static str) -> &'static str {
    if confidence_code(left) >= confidence_code(right) {
        left
    } else {
        right
    }
}

/// Lowers every site of `sites` to at most `ceiling`.
fn cap(sites: &mut [UseSite], ceiling: &'static str) {
    for site in sites {
        site.confidence = weaker(site.confidence, ceiling);
    }
}

/// What a walk from a Rust reference's start positions reached: every
/// position it counted, with the name it looks for there, each file those
/// positions sit in, with the confidence it was reached by, the dead ends,
/// positions a path or a `use` that binds the name reaches that declare
/// nothing and lead nowhere, where a `*` that brings no such name is no
/// dead end, the names each `use`
/// that binds the name and whose path is proven to live outside the
/// repository leads to, and whether a `*` it followed is proven to.
#[derive(Default)]
struct Walk {
    positions: Vec<(String, Module, String)>,
    reached: HashMap<String, &'static str>,
    dead_ends: Vec<(String, Module, String)>,
    outside: Vec<String>,
    outside_glob: bool,
}

impl Walk {
    /// Whether the walk proves its name lives outside the repository: a
    /// `use` that binds it is external, or a `*` is external and every lead
    /// into the repository was followed to its end, because a dead end may
    /// still hold the name.
    fn external(&self) -> bool {
        !self.outside.is_empty() || (self.outside_glob && self.dead_ends.is_empty())
    }
}

/// The walk from `starts`, each a file, the module it stands for, whether
/// it is the bare-name start at the reference, and the name it looks for,
/// until each position `declares` its name; `None` leaves the reference to
/// the name rule.
///
/// The walk moves between positions, each a file and the [`Module`] it
/// stands for there: `Module::Inline` one inline `mod` of it,
/// `Module::TopLevel` its top level, or `Module::Every` every module of it,
/// for a path that starts with `self` and stays in the file that wrote it,
/// because the extractor folds a path into an inline `mod` into `self` and
/// drops the `mod`. A bare name starts in its own file at the innermost
/// inline `mod` that holds the reference. A path or a `use` reaches the file
/// it names, then each leading segment of the rest that names an inline
/// `mod` the file declares, and stands for the last of them, else the file's
/// top level, the module [`named_module`] reads from those segments, by
/// [`position_of`]. A segment after them names a type, so
/// `engine::Thing::new()` starts at the top level of `engine.rs` as a type
/// path and `nested::imp::start()` starts at `imp` in `nested.rs` as a free
/// call. A `*` over a type adds no position, because it brings in only the
/// type's variants and associated items. A candidate counts at a position
/// only when [`admits`] accepts it there.
///
/// From each position the walk follows the `use` items of its module that
/// bind the name, a `use … as Local` by `Local` and onward by the name it
/// renames, and the modules a `*` imports, one hop at a time until a
/// position declares a target the call can reach, so a `pub use` re-export
/// and a glob reach the declaring module. Each stored `use` row is owned by
/// the module whose own body holds its block: a top level owns the rows
/// outside every block, an inline `mod` the rows whose innermost inline
/// `mod`, by the lines each stored inline `mod` spans, is that `mod` and
/// whose block ends on that `mod`'s end line, and no module owns a row in a
/// function body. Only the bare-name start, where a free call or a
/// one-segment path starts at the call, follows the `use` items in force at
/// the call, the calling function's own included, because only a bare name
/// sees them. When one of them that binds the name sits in a function body
/// and its path names no module of the repository, by [`named_module`], it
/// hides the start's own declaration of the name, because in Rust a
/// function's own `use` hides a same-named item of its module in the
/// namespace it binds, so that start neither stops on its declaration nor
/// counts it. A `use` of a module binds the module namespace alone, so it
/// hides no function. When a `*` in force at the call sits in a function
/// body, the start counts its declaration and still follows the glob,
/// because the glob's names are known only where it leads, and in Rust a
/// function's own glob outranks its module's items. Two limits are
/// accepted: a `use` of a standard-library module still hides, because
/// tracer cannot tell a `std` module from a `std` function, and a `fn`
/// declared in a nested block counts as an item of its module. Every other
/// position, a path's start and every hop, a hop back into the calling
/// module included, follows the stored module-body rows of its file that its
/// module owns, and `Module::Every` every module-body row. A hop's
/// [`ModulePaths::rust_path`] reads its first segment through the globs of
/// its position's followed rows alone, never a sibling module's. A hop into
/// a left-out name adds that name's library files as `INFERRED` with every
/// module, a `use` that binds the name and whose path is external records
/// the name it leads to as outside the repository, a `*` whose path is
/// external is recorded too, and a hop that names no other file and is not
/// external leaves the reference to the name rule.
fn walk_modules(
    starts: Vec<(String, Module, bool, String)>,
    imports: &[extraction::Import],
    declares: impl Fn(&str, &Module, &str) -> bool,
    modules: &ModulePaths,
    relations: &Relations,
) -> Option<Walk> {
    let mut walk = Walk::default();
    let mut seen: HashSet<_> = starts.iter().cloned().collect();
    let mut bound: HashSet<_> = starts.iter().map(|(file, module, _, name)| (file.clone(), module.clone(), name.clone())).collect();
    let mut frontier = starts;
    while let Some((current, module, at_call, name)) = frontier.pop() {
        walk.reached.entry(current.clone()).or_insert(CONFIDENCE_EXTRACTED);
        let declared = declares(&current, &module, &name);
        let hidden = declared
            && at_call
            && imports.iter().any(|import| {
                import.binding() == Some(name.as_str())
                    && owner_module(relations, &current, import).is_none()
                    && !import.symbol.as_ref().is_some_and(|symbol| {
                        let path = format!("{}::{symbol}", import.module);
                        let (files, rest) = modules.rust_path(&path, &current, imports);
                        files.iter().any(|target| named_module(target, &rest, relations).is_some())
                    })
            });
        let shared = declared
            && at_call
            && imports
                .iter()
                .any(|import| import.symbol.is_none() && owner_module(relations, &current, import).is_none());
        if !hidden {
            walk.positions.push((current.clone(), module.clone(), name.clone()));
        }
        if declared && !hidden && !shared {
            continue;
        }
        let followed: Cow<[extraction::Import]> = if at_call {
            Cow::Borrowed(imports)
        } else {
            let follows = |import: &extraction::Import| {
                owner_module(relations, &current, import).is_some_and(|owner| module == Module::Every || owner == module)
            };
            Cow::Owned(relations.imports_of(&current).iter().filter(|import| follows(import)).cloned().collect())
        };
        let mut leads = false;
        for import in followed.iter() {
            let (path, next) = match &import.symbol {
                None => (import.module.clone(), name.clone()),
                Some(symbol) if import.binding() == Some(name.as_str()) => {
                    (format!("{}::{symbol}", import.module), symbol.clone())
                }
                Some(_) => continue,
            };
            let (targets, mut rest) = modules.rust_item_path(&path, &current, &followed);
            if targets.is_empty() {
                match modules.left_out_crates(&path, &current) {
                    Some(libraries) => {
                        for member in libraries.iter().flat_map(|library| modules.crate_files(library)) {
                            walk.reached.entry(member.clone()).or_insert(CONFIDENCE_INFERRED);
                            walk.positions.push((member, Module::Every, next.clone()));
                        }
                        leads = true;
                    }
                    None if !modules.is_external(&path, &current) => return None,
                    None if import.symbol.is_some() => walk.outside.push(next.clone()),
                    None => walk.outside_glob = true,
                }
            }
            if import.symbol.is_some() {
                rest.pop();
            }
            for target in targets {
                let (target, module, type_path) = position_of(&path, &current, target, &rest, relations);
                if type_path && import.symbol.is_none() {
                    continue;
                }
                leads = true;
                if import.symbol.is_some() {
                    bound.insert((target.clone(), module.clone(), next.clone()));
                }
                if seen.insert((target.clone(), module.clone(), false, next.clone())) {
                    frontier.push((target, module, false, next.clone()));
                }
            }
        }
        if !at_call && !declared && !leads && bound.contains(&(current.clone(), module.clone(), name.clone())) {
            walk.dead_ends.push((current, module, name));
        }
    }
    Some(walk)
}

/// The position a Rust path `path`, written in `writer`, reaches in
/// `target`, a file it names with `rest` left past it, and whether a type
/// segment follows the module there.
fn position_of(path: &str, writer: &str, target: String, rest: &[String], relations: &Relations) -> (String, Module, bool) {
    let (module, type_path) = (0..=rest.len())
        .rev()
        .find_map(|end| named_module(&target, &rest[..end], relations).map(|module| (module, end < rest.len())))
        .expect("a path with no segment past its file names the file's top level");
    let folded = target == writer && path.split("::").find(|segment| !segment.is_empty()) == Some("self");
    (target, if folded { Module::Every } else { module }, type_path)
}

/// The module whose own body holds `import` in `file`: its top level when no
/// block holds it, the inline `mod` whose body is its block, and none for a
/// block inside a function.
fn owner_module(relations: &Relations, file: &str, import: &extraction::Import) -> Option<Module> {
    let (_, last) = match import.block {
        None => return Some(Module::TopLevel),
        Some(block) => block,
    };
    let inline_modules = relations.inline_modules_of(file);
    crate::surface::innermost(inline_modules.iter().map(|(_, span)| *span), import.line)
        .map(|index| &inline_modules[index])
        .filter(|(_, (_, end))| last == *end)
        .map(|(declared, _)| Module::Inline(declared.clone()))
}

/// The paths a Rust path `written` stands for: one through each `use` of
/// `imports` that binds its first segment, else `written` itself.
fn bound_paths(written: &str, imports: &[extraction::Import]) -> Vec<String> {
    let bound = path_after_binding(written, imports);
    if bound.is_empty() {
        vec![written.to_string()]
    } else {
        bound
    }
}

/// The paths a Rust path `written` stands for through each `use` of
/// `imports` that binds its first segment: that `use`'s path, then the rest
/// of `written`. Several `use` items bind one name under `cfg`
/// alternatives, and each counts.
fn path_after_binding(written: &str, imports: &[extraction::Import]) -> Vec<String> {
    let (first, rest) = written.split_once("::").map_or((written, None), |(first, rest)| (first, Some(rest)));
    imports
        .iter()
        .filter(|import| import.binding() == Some(first))
        .map(|import| {
            [import.module.as_str()]
                .into_iter()
                .chain(import.symbol.as_deref())
                .chain(rest)
                .filter(|segment| !segment.is_empty())
                .collect::<Vec<_>>()
                .join("::")
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn resolve_one(
    candidates: &[&Candidate],
    proven: impl Fn(&Candidate) -> bool,
    path: &str,
    facts: &FileFacts,
    extraction: &ExtractionResult,
    reference: &extraction::Reference,
    written: &extraction::Reference,
    imported: &HashMap<String, &'static str>,
    out: &mut Vec<UseSite>,
) -> bool {
    let matching: Vec<&Candidate> = candidates
        .iter()
        .copied()
        .filter(|c| {
            shape_matches(
                c,
                facts.language.as_deref(),
                reference.shape,
                reference.receiver.as_deref(),
            ) && (!c.is_file_local || c.file == path)
        })
        .collect();
    if matching.is_empty() {
        return false;
    }
    let caller = caller_at(extraction, reference.line);
    let site = |candidate: &Candidate, confidence| UseSite {
        file: path.to_string(),
        line: reference.line,
        caller: caller.clone(),
        target_file: candidate.file.clone(),
        target: candidate.declaration.clone(),
        confidence,
    };
    if reference.shape == RefShape::Member
        && reference.receiver.is_none()
        && untyped_member_is_ambiguous(facts.language.as_deref())
    {
        for candidate in matching {
            out.push(site(candidate, CONFIDENCE_AMBIGUOUS));
        }
        return true;
    }
    if matching.len() == 1 {
        let candidate = matching[0];
        if is_recursion(path, caller.as_ref(), candidate) {
            return true;
        }
        let has_value_receiver = written.shape == RefShape::Member
            && match written.receiver.as_deref() {
                None => types_member_receiver(facts.language.as_deref()),
                Some(receiver) => {
                    let named = receiver.rsplit(['\\', '.', ':']).next().unwrap_or(receiver);
                    !types_member_receiver(facts.language.as_deref())
                        && !is_self_receiver(receiver)
                        && !candidate
                            .declaration
                            .container
                            .as_deref()
                            .is_some_and(|container| same_name(facts.language.as_deref(), container, named))
                }
            };
        let confidence = if !proven(candidate) {
            CONFIDENCE_INFERRED
        } else if candidate.file == path && !has_value_receiver {
            CONFIDENCE_EXTRACTED
        } else {
            imported.get(&candidate.file).copied().unwrap_or(if has_value_receiver {
                CONFIDENCE_AMBIGUOUS
            } else {
                CONFIDENCE_INFERRED
            })
        };
        out.push(site(candidate, confidence));
        return true;
    }
    // Import context narrows a multi-candidate match. One imported candidate
    // keeps the import edge's confidence; several are all legitimate targets
    // and therefore remain ambiguous.
    let in_imports: Vec<(&Candidate, &str)> = matching
        .iter()
        .filter(|candidate| proven(candidate))
        .filter_map(|candidate| {
            imported
                .get(&candidate.file)
                .map(|confidence| (*candidate, *confidence))
        })
        .collect();
    if in_imports.len() == 1 {
        out.push(site(in_imports[0].0, in_imports[0].1));
        return true;
    }
    if in_imports.len() > 1 {
        for (candidate, _) in in_imports {
            out.push(site(candidate, CONFIDENCE_AMBIGUOUS));
        }
        return true;
    }
    // Only a member call may stay ambiguous: its receiver type is not named,
    // so several same-language methods genuinely could be the target. A free
    // or static call names its target exactly, so an unresolved one is name
    // coincidence, and fanning out would reintroduce the noise this model
    // removes.
    if reference.shape == RefShape::Member {
        for candidate in matching {
            out.push(site(candidate, CONFIDENCE_AMBIGUOUS));
        }
    }
    true
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
    let receiver = receiver.map(|receiver| receiver.rsplit(['\\', '.', ':']).next().unwrap_or(receiver));
    let contained_by = |named: &str| {
        declaration
            .container
            .as_deref()
            .is_some_and(|container| same_name(referrer_language, container, named))
    };
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
            Some(named) => (is_type && same_name(referrer_language, &declaration.name, named)) || contained_by(named),
            None => is_type || is_method,
        },
        RefShape::Member => match receiver.filter(|_| types_member_receiver(referrer_language)) {
            Some(named) => is_method && contained_by(named),
            None => is_method,
        },
    }
}

/// Whether two names are one name in `language`: PHP ignores the case of a
/// function, class, or method name, and every other language compares it
/// exactly.
fn same_name(language: Option<&str>, left: &str, right: &str) -> bool {
    match language {
        Some("php") => left.eq_ignore_ascii_case(right),
        _ => left == right,
    }
}

/// Languages where calling a class constructs an instance, so a free call is
/// a valid reference to the class.
fn constructs_by_call(language: Option<&str>) -> bool {
    matches!(language, Some("python") | Some("ruby"))
}

fn types_member_receiver(language: Option<&str>) -> bool {
    matches!(language, Some("php") | Some("rust"))
}

/// Receivers that name the object the call is written in, in every language
/// whose member call carries the receiver's text, so a call on one is not a
/// call on a value.
fn is_self_receiver(receiver: &str) -> bool {
    matches!(receiver, "this" | "self" | "super" | "super()" | "cls")
}

/// Languages whose method call reaches the trait methods of every type in
/// scope, the standard library's and other crates' included, so a call with
/// no stated receiver type has no in-repository method known to be its
/// target.
fn untyped_member_is_ambiguous(language: Option<&str>) -> bool {
    matches!(language, Some("rust"))
}

/// Languages whose path call `a::name()` names a module or a type with one
/// syntax, so a path that names no type is the free call of a module path.
fn paths_name_modules(language: Option<&str>) -> bool {
    matches!(language, Some("rust"))
}

/// Whether a Rust path's first segment names a module of the repository:
/// `crate`, `self`, or `super`, a `mod` the calling file or a candidate's
/// file declares, or a module file of that name.
fn starts_at_module(path: &str, declared: &HashSet<String>, extraction: &ExtractionResult, modules: &ModulePaths) -> bool {
    let Some(first) = path.split("::").find(|segment| !segment.is_empty()) else {
        return false;
    };
    matches!(first, "crate" | "self" | "super")
        || declared.contains(first)
        || module_names(Some(extraction)).any(|name| name == first)
        || modules.has_module_file(first)
}

fn inside_receiver_type(extraction: &ExtractionResult, reference: &extraction::Reference, language: Option<&str>) -> bool {
    reference.receiver.as_deref().is_some_and(|receiver| {
        extraction.declarations.iter().any(|declaration| {
            matches!(declaration.kind.as_str(), "class" | "interface" | "trait" | "enum" | "struct")
                && same_name(language, &declaration.name, receiver)
                && (declaration.header_line..=declaration.end_line).contains(&reference.line)
        })
    })
}

/// Whether `candidate` sits in a module the Rust module path `path` names.
/// `named_files` are the module files `rust_path` resolves the path to, and
/// `rest` the segments past them. A candidate in those files is in the
/// module when [`admits`] accepts it at the module [`named_module`] reads
/// from `rest` in its file, the rule `crate_scope`'s positions read, so a
/// path that names a type admits no free function and a path that names
/// only the file admits its top level. For a path that names no file, the
/// path's last segment is the candidate's module: its enclosing inline
/// `mod`, else its file, named by its stem or, for a `mod.rs`, its
/// directory.
fn in_named_module(
    candidate: &Candidate,
    path: &str,
    named_files: &[String],
    rest: &[String],
    relations: &Relations,
) -> bool {
    if !named_files.is_empty() {
        return named_files.contains(&candidate.file)
            && named_module(&candidate.file, rest, relations).is_some_and(|module| admits(&module, candidate));
    }
    let declaring = file_to_module(&candidate.file, Some("rust"));
    let module = candidate
        .inline_module
        .as_deref()
        .unwrap_or_else(|| tail(declaring.strip_suffix("/mod").unwrap_or(&declaring)));
    module == tail(path)
}

/// The names of the `mod` items a file declares, inline or not.
fn module_names(extraction: Option<&ExtractionResult>) -> impl Iterator<Item = &str> {
    extraction
        .into_iter()
        .flat_map(|extraction| extraction.declarations.iter())
        .filter(|declaration| declaration.kind == "module")
        .map(|declaration| declaration.name.as_str())
}
