//! Architecture graph: cross-file resolution + `architecture/` namespace
//! owner.
//!
//! Code nodes are symbols and modules; code files remain a `source_file`
//! attribute on those nodes, never a node themselves. Doc files
//! (CLAUDE.md / Claude.md / .claude/rules/*.md) are the explicit exception:
//! they become first-class graph nodes (`kind: claude_md | local_md |
//! rules_unconditional | rules_conditional | include`) carrying their
//! `@include` edges. Code-side edges keep the `imports` / `references`
//! taxonomy with EXTRACTED / INFERRED / AMBIGUOUS confidence; doc-side
//! edges use the `includes` relation. `references` edges resolve
//! structurally (see `build_reference_edges`): a use site and a declaration
//! match only when they agree on language and call shape, so the only
//! residual AMBIGUOUS edge is a same-language member call with an unnamed
//! receiver type, narrowed to methods of that name. Cached as a single entry
//! under `architecture/{fingerprint}.bin`, a bincode-encoded `Graph` decoded
//! straight into the struct (no JSON parse, no intermediate `Value` tree),
//! where the fingerprint combines the per-file content hashes (code side)
//! with git HEAD + doc-file mtime aggregate (docs side).

use crate::cache;
use crate::docs_graph;
use crate::extraction;
use crate::file_facts::{self, FileFacts};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

pub const CONFIDENCE_EXTRACTED: &str = "EXTRACTED";
pub const CONFIDENCE_INFERRED: &str = "INFERRED";
pub const CONFIDENCE_AMBIGUOUS: &str = "AMBIGUOUS";

pub const RELATION_IMPORTS: &str = "imports";
pub const RELATION_REFERENCES: &str = "references";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub source_file: Option<String>,
    pub source_line: Option<i64>,
    /// The node's language (`python` / `typescript` / `php`), copied from
    /// the declaring file. Reference resolution requires the use site and
    /// the declaration to agree on language, so a TypeScript call never
    /// resolves onto a PHP declaration of the same name. `None` for module
    /// and external nodes, which are not reference targets.
    pub language: Option<String>,
    /// The enclosing class/interface/trait/enum for a method symbol; `None`
    /// for a free function, a top-level type, or a module. A member call
    /// resolves only to nodes with a container (methods); a free call only
    /// to nodes without one.
    pub container: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub source: String,
    pub target: String,
    pub relation: String,
    pub confidence: String,
    /// Use-site file and line for a `references` edge — overrides the
    /// source-node's coordinates when the renderer prints `file:line`.
    /// `None` for module-level `imports` edges, which keep the source
    /// node's coordinates.
    pub source_file: Option<String>,
    pub source_line: Option<i64>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Graph {
    pub nodes: HashMap<String, Node>,
    /// Insertion order of node ids; query output like `symbols` iterates
    /// the nodes in this order.
    pub node_order: Vec<String>,
    pub edges: Vec<Edge>,
    pub symbol_index: HashMap<String, Vec<String>>,
    pub module_index: HashMap<String, String>,
    /// Insertion order of module_index keys (used by `find_symbols` module
    /// fallback, which iterates `graph.module_index.items()`).
    pub module_order: Vec<String>,
    pub file_to_module_id: HashMap<String, String>,
    /// Doc-file nodes (CLAUDE.md / Claude.md / .claude/rules/*.md) carried
    /// alongside the symbol/module graph in the same cache entry. The shape
    /// differs enough (path/kind/size/paths_globs vs id/label/kind/
    /// source_file/source_line) that overloading `Node` would force
    /// unrelated fields onto code nodes — keeping the doc subset parallel
    /// preserves both shapes cleanly.
    pub doc_nodes: Vec<docs_graph::DocNode>,
    pub doc_edges: Vec<docs_graph::DocEdge>,
    /// Docs-side inputs to the unified fingerprint, retained on the graph
    /// so consumers (e.g. `trace docs --graph`) can read git HEAD + mtime
    /// aggregate without rebuilding the docs side.
    pub docs_head: String,
    pub docs_mtime_aggregate: String,
    pub docs_built_at_ms: u128,
    /// Reverse adjacency for `imports` edges, built once after load/build so
    /// `dependents_of` and the transitive-dependent walk index straight to
    /// the relevant edges instead of rescanning the whole edge list each
    /// call. Each value is edge indices into `edges` in ascending (== edge
    /// insertion) order, so a lookup reproduces the linear-scan result
    /// byte-for-byte. Derived, never serialized — rebuilt by `build_indexes`.
    #[serde(skip)]
    dependents_index: ReverseIndex,
    /// Reverse adjacency for `references` edges, keyed by edge target —
    /// serves `references_to`. Same derived-and-skipped contract as
    /// `dependents_index`.
    #[serde(skip)]
    references_index: HashMap<String, Vec<usize>>,
}

/// Reverse adjacency for the import graph, split into the two lookup keys
/// `dependents_of`'s predicate needs: edges by their literal target, and
/// edges to a symbol grouped under that symbol's owning module (the
/// `module_owns_target` branch). Both hold ascending edge indices.
#[derive(Debug, Default, Clone)]
struct ReverseIndex {
    by_target: HashMap<String, Vec<usize>>,
    by_owning_module: HashMap<String, Vec<usize>>,
}

impl Graph {
    fn insert_node(&mut self, node: Node) {
        if !self.nodes.contains_key(&node.id) {
            self.node_order.push(node.id.clone());
        }
        self.nodes.insert(node.id.clone(), node);
    }

    fn insert_module_index(&mut self, key: String, id: String) {
        if !self.module_index.contains_key(&key) {
            self.module_order.push(key.clone());
        }
        self.module_index.insert(key, id);
    }

    /// Build the reverse-edge indices from the current `edges`, run once
    /// after a build or a disk load. `imports` edges register under their
    /// literal target and (when the target is a symbol) under the target's
    /// owning module; `references` edges register under their literal
    /// target. Indices are appended in edge order, so each bucket is already
    /// ascending — the order `dependents_of` / `references_to` must return.
    fn build_indexes(&mut self) {
        let mut dependents = ReverseIndex::default();
        let mut references: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, e) in self.edges.iter().enumerate() {
            if e.relation == RELATION_IMPORTS {
                dependents
                    .by_target
                    .entry(e.target.clone())
                    .or_default()
                    .push(i);
                if let Some(owning) = self
                    .nodes
                    .get(&e.target)
                    .and_then(|n| n.source_file.as_ref())
                    .and_then(|sf| self.file_to_module_id.get(sf))
                {
                    if owning != &e.target {
                        dependents
                            .by_owning_module
                            .entry(owning.clone())
                            .or_default()
                            .push(i);
                    }
                }
            } else if e.relation == RELATION_REFERENCES {
                references.entry(e.target.clone()).or_default().push(i);
            }
        }
        self.dependents_index = dependents;
        self.references_index = references;
    }

}

fn module_node_id(module_path: &str) -> String {
    format!("module::{module_path}")
}
fn symbol_node_id(source_file: &str, symbol: &str) -> String {
    format!("{source_file}::{symbol}")
}

/// Module id for a file: extension stripped, separators turned into `.`
/// for Python and `/` for every other language.
fn file_to_module(relative_path: &str, language: Option<&str>) -> String {
    let p = Path::new(relative_path);
    let stem = match p.extension() {
        Some(_) => p.with_extension("").to_string_lossy().to_string(),
        None => relative_path.to_string(),
    };
    if language == Some("python") {
        stem.replace(std::path::MAIN_SEPARATOR, ".")
    } else {
        stem.replace(std::path::MAIN_SEPARATOR, "/")
    }
}

/// Files to feed the graph: the shared `git ls-files` enumeration (deleted-
/// in-index paths already excluded), else a SKIP_DIRS-bounded walk; filtered
/// to supported extensions, symlinks excluded.
pub fn discover_files(repo_root: &Path) -> Vec<PathBuf> {
    let exts = extraction::supported_extensions();
    let files =
        crate::repo_files::tracked_paths(repo_root, None).unwrap_or_else(|| walk_files(repo_root));
    files
        .into_iter()
        .filter(|f| {
            let ok_ext = f
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| exts.contains(&e.to_lowercase().as_str()))
                .unwrap_or(false);
            ok_ext && !f.is_symlink()
        })
        .collect()
}

fn walk_files(repo_root: &Path) -> Vec<PathBuf> {
    let skip: std::collections::HashSet<&str> = [
        ".git",
        "node_modules",
        ".venv",
        "venv",
        "__pycache__",
        "dist",
        "build",
        ".next",
        ".tracer-cache",
        ".pytest_cache",
        ".mypy_cache",
        ".ruff_cache",
        "vendor",
        "worktrees",
        "trellis",
        "bedrock",
        "public",
        "storage",
        "bootstrap",
        ".lando",
        ".playwright",
        "playwright-report",
        "test-results",
    ]
    .into_iter()
    .collect();
    let mut out = Vec::new();
    let walker = walkdir::WalkDir::new(repo_root).into_iter().filter_entry(|e| {
        let n = e.file_name().to_string_lossy();
        if e.file_type().is_dir() {
            // A nested repository is its own scope, never part of the parent's
            // file set — `.git` is a directory for a normal checkout and a file
            // for a linked worktree, so `exists` covers both.
            if e.depth() > 0 && e.path().join(".git").exists() {
                return false;
            }
            !skip.contains(n.as_ref()) && !n.starts_with('.')
        } else {
            true
        }
    });
    for e in walker.flatten() {
        if e.file_type().is_file() {
            out.push(e.path().to_path_buf());
        }
    }
    out
}

/// Two-phase build of the architecture graph from per-file facts.
fn build_from_facts(all_facts: &[FileFacts]) -> Graph {
    let mut graph = Graph::default();

    // Phase 1: nodes.
    for facts in all_facts {
        let extraction = match &facts.extraction {
            Some(e) => e,
            None => continue,
        };
        let module_for_file = file_to_module(&facts.path, facts.language.as_deref());
        let module_id = module_node_id(&module_for_file);
        if !graph.nodes.contains_key(&module_id) {
            graph.insert_node(Node {
                id: module_id.clone(),
                label: module_for_file.clone(),
                kind: "module".into(),
                source_file: Some(facts.path.clone()),
                source_line: Some(1),
                language: None,
                container: None,
            });
            graph.insert_module_index(module_for_file.clone(), module_id.clone());
            graph
                .file_to_module_id
                .insert(facts.path.clone(), module_id.clone());
        }
        // Index every declaration — methods, private/non-exported,
        // nested defs — so they all become resolvable nodes. The narrower
        // `exports` list still feeds the structure/module-API views via
        // other commands; here we want the full index.
        for decl in &extraction.declarations {
            let node_id = symbol_node_id(&facts.path, &decl.name);
            if graph.nodes.contains_key(&node_id) {
                // Same (file, name) seen twice — overloaded or duplicate
                // definition. Keep the first.
                continue;
            }
            graph.insert_node(Node {
                id: node_id.clone(),
                label: decl.name.clone(),
                kind: decl.kind.clone(),
                source_file: Some(facts.path.clone()),
                source_line: Some(decl.line),
                language: facts.language.clone(),
                container: decl.container.clone(),
            });
            graph
                .symbol_index
                .entry(decl.name.to_lowercase())
                .or_default()
                .push(node_id);
        }
    }

    // Precompute internal pairs (module_index order preserved).
    let internal_pairs: Vec<(String, String)> = graph
        .module_order
        .iter()
        .filter(|k| !k.starts_with("external::"))
        .map(|k| (k.clone(), graph.module_index[k].clone()))
        .collect();
    let internal_pairs_lower: Vec<(String, String)> = internal_pairs
        .iter()
        .map(|(i, n)| (i.to_lowercase(), n.clone()))
        .collect();

    // Phase 2: edges.
    for facts in all_facts {
        let extraction = match &facts.extraction {
            Some(e) => e,
            None => continue,
        };
        let importer_module_id = module_node_id(&file_to_module(
            &facts.path,
            facts.language.as_deref(),
        ));
        for import_decl in &extraction.imports {
            let target_module_id = resolve_module(
                &graph,
                &import_decl.module,
                facts.language.as_deref(),
                &internal_pairs,
                &internal_pairs_lower,
            );
            match &import_decl.symbol {
                Some(sym) => {
                    let combined = if facts.language.as_deref() == Some("python") {
                        format!("{}.{}", import_decl.module, sym)
                    } else {
                        format!("{}/{}", import_decl.module, sym)
                    };
                    let module_candidate = resolve_module(
                        &graph,
                        &combined,
                        facts.language.as_deref(),
                        &internal_pairs,
                        &internal_pairs_lower,
                    );
                    if let Some(mc) = module_candidate {
                        graph.edges.push(Edge {
                            source: importer_module_id.clone(),
                            target: mc,
                            relation: RELATION_IMPORTS.into(),
                            confidence: CONFIDENCE_EXTRACTED.into(),
                            source_file: None,
                            source_line: None,
                        });
                        continue;
                    }
                    let candidates = graph
                        .symbol_index
                        .get(&sym.to_lowercase())
                        .cloned()
                        .unwrap_or_default();
                    let mut target_id =
                        select_best_symbol(&candidates, &target_module_id, &graph);
                    let confidence = classify_confidence(
                        &target_id,
                        &candidates,
                        &target_module_id,
                    );
                    if target_id.is_none() && target_module_id.is_some() {
                        target_id = target_module_id.clone();
                    }
                    let final_target = match target_id {
                        Some(t) => t,
                        None => ensure_external(&mut graph, &import_decl.module),
                    };
                    graph.edges.push(Edge {
                        source: importer_module_id.clone(),
                        target: final_target,
                        relation: RELATION_IMPORTS.into(),
                        confidence,
                        source_file: None,
                        source_line: None,
                    });
                }
                None => {
                    let target = match target_module_id {
                        Some(t) => t,
                        None => ensure_external(&mut graph, &import_decl.module),
                    };
                    graph.edges.push(Edge {
                        source: importer_module_id.clone(),
                        target,
                        relation: RELATION_IMPORTS.into(),
                        confidence: CONFIDENCE_EXTRACTED.into(),
                        source_file: None,
                        source_line: None,
                    });
                }
            }
        }
    }

    // Phase 3: reference edges. Every use site resolves against the
    // declaration index using the structural context the AST carries — the
    // call shape (free / member / static), the class named at the site (for
    // static uses), and the language of both sides — never bare name alone.
    // A resolved use site becomes an edge from the referring module → the
    // declaration, carrying the use-site file:line and a confidence label.
    //
    // Structural resolution model:
    //   - candidates are restricted to the SAME LANGUAGE as the use site,
    //     so a TypeScript `process()` never resolves onto a PHP `process`
    //   - a `free` call (`foo()`) resolves only to non-method declarations
    //     (functions and classes — a bare `Foo()` is a constructor call);
    //     never to a method
    //   - a `static` use (`Foo::bar()`, `new Foo`, `Foo::class`, a type
    //     hint) names the class at the site, so it resolves exactly: to the
    //     named class, or to the method of that name whose container is the
    //     named class — never ambiguously
    //   - a `member` call (`$x->foo()`, `obj.foo()`) names a value receiver,
    //     not a type, so it resolves to methods of that name. When exactly
    //     one method survives (after the import-context narrowing the import
    //     model uses) it is EXTRACTED/INFERRED; otherwise it is the sole
    //     residual AMBIGUOUS case — one edge per candidate METHOD, never to
    //     every symbol of that name.
    build_reference_edges(&mut graph, all_facts);

    graph
}

/// The set of source files an importer module imported from (resolved to
/// internal `source_file` paths). Used to bias single-candidate references
/// to EXTRACTED when the candidate lives in an imported file.
fn imported_files_for(graph: &Graph, importer_module_id: &str) -> std::collections::HashSet<String> {
    let mut out = std::collections::HashSet::new();
    for e in &graph.edges {
        if e.relation != RELATION_IMPORTS || e.source != importer_module_id {
            continue;
        }
        if let Some(target_node) = graph.nodes.get(&e.target) {
            if let Some(sf) = &target_node.source_file {
                out.insert(sf.clone());
            }
        }
    }
    out
}

/// A reference is a "self-reference" when the file declares exactly one
/// symbol with this name (case-insensitive). That maps cleanly to the
/// self-recursion case (`fact` defined once in `rec.py`, called by name
/// inside its own body) without dropping the rarer case where two
/// homonyms share a file.
fn is_self_reference(extraction: &crate::extraction::ExtractionResult, name: &str) -> bool {
    let lower = name.to_lowercase();
    extraction
        .declarations
        .iter()
        .filter(|d| d.name.to_lowercase() == lower)
        .count()
        == 1
}

/// Languages where calling a class — `Foo()` — constructs an instance, so a
/// free call is a valid reference to the class. `new`-constructing languages
/// (TypeScript, JS, PHP, Java) are absent: there construction is a `new`
/// expression, already classified `Static`.
fn constructs_by_call(language: Option<&str>) -> bool {
    matches!(language, Some("python") | Some("ruby"))
}

/// Does a candidate declaration node structurally match a use site of the
/// given shape, in the given language, naming `receiver` (for static uses)?
/// `name` is the lowercased reference name; the candidate's name already
/// matched it via the symbol index.
fn shape_matches(
    node: &Node,
    referrer_language: Option<&str>,
    shape: crate::extraction::RefShape,
    reference_name: &str,
    receiver: Option<&str>,
) -> bool {
    use crate::extraction::RefShape;
    // Same-language only — the rule that removes every cross-language edge.
    if node.language.as_deref() != referrer_language {
        return false;
    }
    match shape {
        // A free call resolves to a non-method declaration. A class/interface
        // is a valid target only in languages that construct by calling the
        // class (Python, Ruby); in `new`-constructing languages (TypeScript,
        // JS, PHP, Java) construction goes through `Static` (`new X`), so a
        // bare call landing on a class name is coincidence, not a reference.
        // Functions and callable top-levels (arrow consts) resolve in any
        // language. Never a method.
        RefShape::Free => {
            if node.container.is_some() || node.kind == "module" {
                false
            } else if matches!(node.kind.as_str(), "class" | "interface") {
                constructs_by_call(referrer_language)
            } else {
                true
            }
        }
        // A member call resolves only to methods (declarations with a
        // container). The receiver is a value, not a type, so the method's
        // class can't be checked — this is the residual ambiguity.
        RefShape::Member => node.container.is_some(),
        // A static use names the class at the site. Two cases:
        //   - the reference IS the class (`new Foo`, `Foo::class`, a type
        //     hint, `instanceof Foo`): name == receiver → match the class
        //     declaration of that name.
        //   - a static method call (`Foo::bar()`): name != receiver → match
        //     the method `bar` whose container is the named class `Foo`.
        RefShape::Static => {
            let names_self = receiver
                .map(|r| r.eq_ignore_ascii_case(reference_name))
                .unwrap_or(false);
            if names_self {
                // Resolving the class itself.
                matches!(node.kind.as_str(), "class" | "interface")
                    && node.container.is_none()
            } else {
                // Resolving a static method on the named class.
                match (receiver, &node.container) {
                    (Some(r), Some(c)) => c.eq_ignore_ascii_case(r),
                    _ => false,
                }
            }
        }
    }
}

fn build_reference_edges(graph: &mut Graph, all_facts: &[FileFacts]) {
    let mut new_edges: Vec<Edge> = Vec::new();
    for facts in all_facts {
        let extraction = match &facts.extraction {
            Some(e) => e,
            None => continue,
        };
        if extraction.references.is_empty() {
            continue;
        }
        let importer_module_id = module_node_id(&file_to_module(
            &facts.path,
            facts.language.as_deref(),
        ));
        let imported_files = imported_files_for(graph, &importer_module_id);

        for reference in &extraction.references {
            let name_lower = reference.name.to_lowercase();
            let all_candidates = match graph.symbol_index.get(&name_lower) {
                Some(c) if !c.is_empty() => c.clone(),
                _ => continue,
            };
            // Structural narrowing: keep only candidates that agree with the
            // use site on language and shape. This is what replaces bare-name
            // fan-out — a TypeScript `process()` keeps only TypeScript
            // functions named `process`, never the PHP methods.
            let candidates: Vec<String> = all_candidates
                .into_iter()
                .filter(|cid| {
                    graph
                        .nodes
                        .get(cid)
                        .map(|n| {
                            shape_matches(
                                n,
                                facts.language.as_deref(),
                                reference.shape,
                                &reference.name,
                                reference.receiver.as_deref(),
                            )
                        })
                        .unwrap_or(false)
                })
                .collect();
            if candidates.is_empty() {
                continue;
            }
            // Function-granular source: a use site inside a declared function
            // resolves its edge source to that calling symbol's node
            // (`file::enclosing`) when the node exists in the graph, so
            // `callers` answers "function `first` calls X" rather than
            // "module app references X". A use site at module top level (no
            // enclosing) keeps the importer-module source.
            let edge_source = reference
                .enclosing
                .as_deref()
                .map(|name| symbol_node_id(&facts.path, name))
                .filter(|id| graph.nodes.contains_key(id))
                .unwrap_or_else(|| importer_module_id.clone());
            if candidates.len() == 1 {
                let target = &candidates[0];
                let target_node = match graph.nodes.get(target) {
                    Some(n) => n,
                    None => continue,
                };
                // Self-recursion is excluded by contract: a function's own
                // recursive call adds no cross-symbol caller. Same detection
                // as before — sole same-named declaration in this file.
                if target_node.source_file.as_deref() == Some(facts.path.as_str())
                    && is_self_reference(extraction, &reference.name)
                {
                    continue;
                }
                let in_imported_file = target_node
                    .source_file
                    .as_ref()
                    .map(|sf| imported_files.contains(sf))
                    .unwrap_or(false);
                let same_file =
                    target_node.source_file.as_deref() == Some(facts.path.as_str());
                let confidence = if in_imported_file || same_file {
                    CONFIDENCE_EXTRACTED
                } else {
                    CONFIDENCE_INFERRED
                };
                new_edges.push(Edge {
                    source: edge_source.clone(),
                    target: target.clone(),
                    relation: RELATION_REFERENCES.into(),
                    confidence: confidence.into(),
                    source_file: Some(facts.path.clone()),
                    source_line: Some(reference.line),
                });
            } else {
                // More than one structural match survives. Import context
                // narrows it: a single candidate in an imported (or same)
                // file is the resolved one — EXTRACTED.
                let in_imports: Vec<&String> = candidates
                    .iter()
                    .filter(|cid| {
                        graph
                            .nodes
                            .get(*cid)
                            .and_then(|n| n.source_file.as_ref())
                            .map(|sf| imported_files.contains(sf))
                            .unwrap_or(false)
                    })
                    .collect();
                if in_imports.len() == 1 {
                    let target = in_imports[0].clone();
                    new_edges.push(Edge {
                        source: edge_source.clone(),
                        target,
                        relation: RELATION_REFERENCES.into(),
                        confidence: CONFIDENCE_EXTRACTED.into(),
                        source_file: Some(facts.path.clone()),
                        source_line: Some(reference.line),
                    });
                    continue;
                }
                // Only a member call may remain AMBIGUOUS — its receiver type
                // is not named, so several same-language methods of this name
                // genuinely could be the target. A free or static call names
                // either a top-level symbol or a class exactly; when import
                // context still can't pick one, the remaining matches are
                // name coincidence (two unrelated free functions of the same
                // name), and fanning out would reintroduce the very noise
                // this model removes — so no edge is emitted. The narrowing
                // dropped no resolved relationship: a free/static call that
                // resolves cleanly took the single-candidate or single-import
                // path above.
                if reference.shape != crate::extraction::RefShape::Member {
                    continue;
                }
                for cid in &candidates {
                    new_edges.push(Edge {
                        source: edge_source.clone(),
                        target: cid.clone(),
                        relation: RELATION_REFERENCES.into(),
                        confidence: CONFIDENCE_AMBIGUOUS.into(),
                        source_file: Some(facts.path.clone()),
                        source_line: Some(reference.line),
                    });
                }
            }
        }
    }
    graph.edges.extend(new_edges);
}

/// Resolve an imported module path to a graph node id: exact module-index
/// hit, else a language-aware suffix match against indexed internal modules.
fn resolve_module(
    graph: &Graph,
    module_path: &str,
    language: Option<&str>,
    internal_pairs: &[(String, String)],
    internal_pairs_lower: &[(String, String)],
) -> Option<String> {
    if let Some(id) = graph.module_index.get(module_path) {
        return Some(id.clone());
    }
    if language == Some("php") {
        let slashed = module_path.replace('\\', "/").to_lowercase();
        for (indexed_lower, node_id) in internal_pairs_lower {
            if indexed_lower.ends_with(&slashed) {
                return Some(node_id.clone());
            }
        }
        return None;
    }
    // Strip TS/JS relative-path prefixes (`./`, `../`) so a `./helpers`
    // import matches the indexed `src/helpers` module via suffix. Without
    // this, every relative import resolved to "INFERRED" because the
    // suffix match never saw past the leading `./`.
    let normalized = strip_relative_prefix(module_path);
    let dot_suffix = format!(".{normalized}");
    let slash_suffix = format!("/{normalized}");
    for (indexed_module, node_id) in internal_pairs {
        if indexed_module.ends_with(&normalized)
            || indexed_module.ends_with(&dot_suffix)
            || indexed_module.ends_with(&slash_suffix)
        {
            return Some(node_id.clone());
        }
    }
    None
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

/// Pick the best symbol from candidates: prefer one in the resolved
/// target module's source file, else the sole candidate, else none.
fn select_best_symbol(
    candidates: &[String],
    target_module_id: &Option<String>,
    graph: &Graph,
) -> Option<String> {
    if candidates.is_empty() {
        return None;
    }
    if let Some(tmid) = target_module_id {
        if let Some(target_module_node) = graph.nodes.get(tmid) {
            for candidate in candidates {
                if let Some(node) = graph.nodes.get(candidate) {
                    if node.source_file == target_module_node.source_file {
                        return Some(candidate.clone());
                    }
                }
            }
        }
    }
    if candidates.len() == 1 {
        return Some(candidates[0].clone());
    }
    None
}

/// Confidence label for a resolved edge: EXTRACTED when resolved with a
/// target module, INFERRED when resolved without one, AMBIGUOUS when
/// multiple candidates remained.
fn classify_confidence(
    resolved: &Option<String>,
    candidates: &[String],
    target_module_id: &Option<String>,
) -> String {
    if resolved.is_some() && target_module_id.is_some() {
        return CONFIDENCE_EXTRACTED.into();
    }
    if resolved.is_some() {
        return CONFIDENCE_INFERRED.into();
    }
    if candidates.len() > 1 {
        return CONFIDENCE_AMBIGUOUS.into();
    }
    CONFIDENCE_EXTRACTED.into()
}

/// Get or create the `external::<module>` node for an unresolved import,
/// returning its node id.
fn ensure_external(graph: &mut Graph, module_path: &str) -> String {
    let node_id = module_node_id(&format!("external::{module_path}"));
    if !graph.nodes.contains_key(&node_id) {
        graph.insert_node(Node {
            id: node_id.clone(),
            label: module_path.to_string(),
            kind: "module".into(),
            source_file: None,
            source_line: None,
            language: None,
            container: None,
        });
        graph.insert_module_index(
            format!("external::{module_path}"),
            node_id.clone(),
        );
    }
    node_id
}

/// Process-wide memo of the parsed graph, keyed by repo root. The graph is
/// a function of per-file hashes + git HEAD + doc-file mtimes, all stable
/// within a single CLI invocation, so file discovery, hashing, the docs
/// walk, the disk load, and the JSON parse run at most once per root —
/// every `get` / `load_cached` caller after the first reuses the parsed
/// graph. The lock is held across the compute so a cold memo parses (or
/// builds) exactly once even when parallel callers race (the primer runs
/// its spine and dirty-rank sections concurrently).
static MEMO: OnceLock<Mutex<HashMap<PathBuf, Graph>>> = OnceLock::new();

/// The unified architecture graph for a repo: returns the cached graph
/// when the fingerprint matches, else builds it from per-file facts plus
/// the docs-graph build and caches it. The fingerprint combines per-file
/// content hashes (code side) with git HEAD + doc-file mtime aggregate
/// (docs side), so an edit to either input invalidates the entry.
pub fn get(repo_root: &Path) -> Graph {
    let memo = MEMO.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = memo.lock().unwrap();
    if let Some(cached) = guard.get(repo_root) {
        return cached.clone();
    }
    let graph = load_or_build(repo_root);
    guard.insert(repo_root.to_path_buf(), graph.clone());
    graph
}

/// Decode a bincode graph entry into the struct and build its reverse-edge
/// indexes. The on-disk form omits the derived indexes (`#[serde(skip)]`),
/// so they are rebuilt here once per load.
fn decode_graph(bytes: &[u8]) -> Option<Graph> {
    let mut graph: Graph = bincode::deserialize(bytes).ok()?;
    graph.build_indexes();
    Some(graph)
}

/// Load the parsed graph from the disk cache when the fingerprint matches,
/// else build it from per-file facts plus the docs-graph build and persist
/// it. Always returns a graph — the build path is the fallback.
fn load_or_build(repo_root: &Path) -> Graph {
    // The build exists to fill the `architecture/` entry, and `cache_root`
    // writes that entry only where `repo_root/.git` is. Outside a worktree the
    // graph is discarded at process exit and rebuilt from scratch on the next
    // call, so a directory holding many repositories (`~/Developer/references`)
    // parsed every file of every one of them into one graph, every run.
    if !repo_root.join(".git").exists() {
        return Graph::default();
    }
    let files = discover_files(repo_root);
    let hashes = file_facts::file_hashes_for(&files, repo_root);
    let (docs, docs_inputs) = docs_graph::build(repo_root);
    let key = cache::architecture_fingerprint(
        &hashes,
        &docs_inputs.head,
        &docs_inputs.mtime_aggregate,
    );
    if let Some(bytes) = cache::load_bytes(cache::NAMESPACE_ARCHITECTURE, &key, repo_root) {
        if let Some(graph) = decode_graph(&bytes) {
            return graph;
        }
    }
    let all_facts = file_facts::get_many(&files, repo_root);
    let mut graph = build_from_facts(&all_facts);
    graph.doc_nodes = docs.nodes;
    graph.doc_edges = docs.edges;
    graph.docs_head = docs.head;
    graph.docs_mtime_aggregate = docs.mtime_aggregate;
    graph.docs_built_at_ms = docs.built_at_ms;
    graph.build_indexes();
    if let Ok(bytes) = bincode::serialize(&graph) {
        let _ = cache::save_entry(cache::NAMESPACE_ARCHITECTURE, &key, &bytes, repo_root);
    }
    graph
}

/// The cached unified architecture graph if present — never builds. Reuses
/// the parsed graph from the process memo when a prior `get` / `load_cached`
/// already produced one for this root; otherwise decodes the disk entry
/// (memoizing it) and returns `None` when no valid disk entry exists.
///
/// Load-only validation never re-walks the doc tree. Eviction keeps exactly
/// one entry per repo, so the lone entry is read and decoded, then validated
/// against the current code side + git HEAD by reconstructing its fingerprint
/// from the cheap per-file hashes, a single `git rev-parse` for HEAD, and the
/// docs mtime aggregate **read off the decoded entry itself** — the doc-file
/// mtimes are never re-stat'd. The reconstructed fingerprint must equal the
/// entry's own filename, which holds exactly when no code file changed and
/// HEAD has not moved.
///
/// An uncommitted doc-file mtime change that moves no code hash and no HEAD
/// is intentionally not detected here: every `load_cached` consumer reads the
/// symbol/module graph only (callers/dependents enrichment, the context
/// Spine), never doc nodes. Doc-node freshness is owned by `trace docs
/// --graph`, which goes through `get` (the build path) and rebuilds on any
/// doc-mtime change.
pub fn load_cached(repo_root: &Path) -> Option<Graph> {
    let memo = MEMO.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = memo.lock().unwrap();
    if let Some(cached) = guard.get(repo_root) {
        return Some(cached.clone());
    }
    let (key, bytes) = cache::load_sole_entry(cache::NAMESPACE_ARCHITECTURE, repo_root)?;
    let graph = decode_graph(&bytes)?;
    let files = discover_files(repo_root);
    let hashes = file_facts::file_hashes_for(&files, repo_root);
    let head = docs_graph::git_head(repo_root);
    let expected =
        cache::architecture_fingerprint(&hashes, &head, &graph.docs_mtime_aggregate);
    if expected != key {
        return None;
    }
    guard.insert(repo_root.to_path_buf(), graph.clone());
    Some(graph)
}

/// Nodes whose symbol name matches `name` (case-insensitive); falls back
/// to modules whose last path segment matches.
pub fn find_symbols<'a>(graph: &'a Graph, name: &str) -> Vec<&'a Node> {
    let lower = name.to_lowercase();
    if let Some(ids) = graph.symbol_index.get(&lower) {
        let matches: Vec<&Node> = ids.iter().filter_map(|id| graph.nodes.get(id)).collect();
        if !matches.is_empty() {
            return matches;
        }
    }
    let mut module_matches = Vec::new();
    for module_path in &graph.module_order {
        if module_path.starts_with("external::") {
            continue;
        }
        let last_segment = module_path
            .replace('/', ".")
            .rsplit('.')
            .next()
            .unwrap_or("")
            .to_string();
        if last_segment.to_lowercase() == lower {
            if let Some(id) = graph.module_index.get(module_path) {
                if let Some(node) = graph.nodes.get(id) {
                    module_matches.push(node);
                }
            }
        }
    }
    module_matches
}

/// Edges pointing at `node_id` (or its owning module) — its direct
/// import dependents. Filters to `imports` only so the module-level
/// dependency graph stays distinct from the reference-edge index that
/// `references_to` exposes.
pub fn dependents_of<'a>(graph: &'a Graph, node_id: &str) -> Vec<&'a Edge> {
    let node = graph.nodes.get(node_id);
    // The same predicate the linear scan applied, served from the reverse
    // index: an import edge is a dependent when its target is `node_id`, or
    // its target is the owning module of `node_id`, or (`node_id` is a
    // module) its target is a symbol owned by `node_id`'s module. Collect
    // the matching edge indices from the precomputed buckets, then sort +
    // dedup so the result is in edge-insertion order — byte-identical to the
    // scan it replaces.
    let mut indices: Vec<usize> = Vec::new();
    if let Some(b) = graph.dependents_index.by_target.get(node_id) {
        indices.extend_from_slice(b);
    }
    if let Some(tn) = node {
        if let Some(owner) = tn
            .source_file
            .as_ref()
            .and_then(|sf| graph.file_to_module_id.get(sf))
        {
            if let Some(b) = graph.dependents_index.by_target.get(owner) {
                indices.extend_from_slice(b);
            }
        }
    }
    let is_module = node.map(|n| n.kind == "module").unwrap_or(false);
    if is_module {
        if let Some(b) = graph.dependents_index.by_owning_module.get(node_id) {
            indices.extend_from_slice(b);
        }
    }
    indices.sort_unstable();
    indices.dedup();
    indices.iter().map(|&i| &graph.edges[i]).collect()
}

/// Edges originating from `node_id` (or its owning module) — its direct
/// import dependencies. Filters to `imports` only, same logic as
/// `dependents_of`.
pub fn dependencies_of<'a>(graph: &'a Graph, node_id: &str) -> Vec<&'a Edge> {
    let tn = match graph.nodes.get(node_id) {
        Some(n) => n,
        None => return vec![],
    };
    let mut sources = vec![node_id.to_string()];
    if let Some(sf) = &tn.source_file {
        if let Some(owner) = graph.file_to_module_id.get(sf) {
            sources.push(owner.clone());
        }
    }
    graph
        .edges
        .iter()
        .filter(|e| e.relation == RELATION_IMPORTS)
        .filter(|e| sources.contains(&e.source))
        .collect()
}

/// Reference-edge use sites whose target is `node_id`. Distinct from the
/// import-graph: each edge carries the use-site file:line and a confidence
/// label drawn from EXTRACTED / INFERRED / AMBIGUOUS.
pub fn references_to<'a>(graph: &'a Graph, node_id: &str) -> Vec<&'a Edge> {
    // Served from the reverse index keyed by edge target. The bucket already
    // holds indices in edge-insertion order, so this returns the same edges
    // in the same order as the linear scan it replaces.
    match graph.references_index.get(node_id) {
        Some(indices) => indices.iter().map(|&i| &graph.edges[i]).collect(),
        None => Vec::new(),
    }
}

/// Transitive dependents up to `max_depth` (BFS over reverse edges).
pub fn transitive_dependents<'a>(
    graph: &'a Graph,
    node_id: &str,
    max_depth: i64,
) -> Vec<(&'a Node, i64)> {
    let mut visited = vec![node_id.to_string()];
    let mut frontier = std::collections::VecDeque::new();
    frontier.push_back((node_id.to_string(), 0i64));
    let mut out = Vec::new();
    while let Some((current_id, depth)) = frontier.pop_front() {
        if depth >= max_depth {
            continue;
        }
        for edge in dependents_of(graph, &current_id) {
            if visited.contains(&edge.source) {
                continue;
            }
            visited.push(edge.source.clone());
            if let Some(sn) = graph.nodes.get(&edge.source) {
                out.push((sn, depth + 1));
                frontier.push_back((edge.source.clone(), depth + 1));
            }
        }
    }
    out
}

/// Transitive dependencies up to `max_depth` (BFS over forward edges).
pub fn transitive_dependencies<'a>(
    graph: &'a Graph,
    node_id: &str,
    max_depth: i64,
) -> Vec<(&'a Node, i64)> {
    let mut visited = vec![node_id.to_string()];
    let mut frontier = std::collections::VecDeque::new();
    frontier.push_back((node_id.to_string(), 0i64));
    let mut out = Vec::new();
    while let Some((current_id, depth)) = frontier.pop_front() {
        if depth >= max_depth {
            continue;
        }
        for edge in dependencies_of(graph, &current_id) {
            if visited.contains(&edge.target) {
                continue;
            }
            visited.push(edge.target.clone());
            if let Some(tn) = graph.nodes.get(&edge.target) {
                out.push((tn, depth + 1));
                frontier.push_back((edge.target.clone(), depth + 1));
            }
        }
    }
    out
}
