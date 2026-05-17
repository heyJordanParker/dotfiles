//! Architecture graph: cross-file resolution + `architecture/` namespace owner.
//!
//! Nodes are symbols and modules — never files. Edges are cross-file
//! `imports` relations with EXTRACTED/INFERRED/AMBIGUOUS confidence.
//! Cached under `architecture/{fingerprint}.json`.

use crate::cache;
use crate::extraction;
use crate::file_facts::{self, FileFacts};
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const CONFIDENCE_EXTRACTED: &str = "EXTRACTED";
pub const CONFIDENCE_INFERRED: &str = "INFERRED";
pub const CONFIDENCE_AMBIGUOUS: &str = "AMBIGUOUS";

#[derive(Debug, Clone)]
pub struct Node {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub source_file: Option<String>,
    pub source_line: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct Edge {
    pub source: String,
    pub target: String,
    pub relation: String,
    pub confidence: String,
}

#[derive(Debug, Default)]
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

    pub fn to_json(&self) -> Value {
        let mut nodes = Map::new();
        for nid in &self.node_order {
            let n = &self.nodes[nid];
            nodes.insert(
                nid.clone(),
                json!({
                    "id": n.id,
                    "label": n.label,
                    "kind": n.kind,
                    "source_file": n.source_file,
                    "source_line": n.source_line,
                }),
            );
        }
        let edges: Vec<Value> = self
            .edges
            .iter()
            .map(|e| {
                json!({
                    "source": e.source,
                    "target": e.target,
                    "relation": e.relation,
                    "confidence": e.confidence,
                })
            })
            .collect();
        json!({
            "nodes": Value::Object(nodes),
            "edges": edges,
            "symbol_index": self.symbol_index,
            "module_index": self.module_index,
            "file_to_module_id": self.file_to_module_id,
        })
    }

    pub fn from_json(v: &Value) -> Graph {
        let mut g = Graph::default();
        if let Some(nodes) = v.get("nodes").and_then(|x| x.as_object()) {
            for (nid, nd) in nodes {
                g.node_order.push(nid.clone());
                g.nodes.insert(
                    nid.clone(),
                    Node {
                        id: nd
                            .get("id")
                            .and_then(|x| x.as_str())
                            .unwrap_or(nid)
                            .to_string(),
                        label: nd
                            .get("label")
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_string(),
                        kind: nd
                            .get("kind")
                            .and_then(|x| x.as_str())
                            .unwrap_or("")
                            .to_string(),
                        source_file: nd
                            .get("source_file")
                            .and_then(|x| x.as_str())
                            .map(|s| s.to_string()),
                        source_line: nd
                            .get("source_line")
                            .and_then(|x| x.as_i64()),
                    },
                );
            }
        }
        if let Some(edges) = v.get("edges").and_then(|x| x.as_array()) {
            for e in edges {
                g.edges.push(Edge {
                    source: e
                        .get("source")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string(),
                    target: e
                        .get("target")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string(),
                    relation: e
                        .get("relation")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string(),
                    confidence: e
                        .get("confidence")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string(),
                });
            }
        }
        if let Some(si) = v.get("symbol_index").and_then(|x| x.as_object()) {
            for (k, arr) in si {
                g.symbol_index.insert(
                    k.clone(),
                    arr.as_array()
                        .map(|a| {
                            a.iter()
                                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                                .collect()
                        })
                        .unwrap_or_default(),
                );
            }
        }
        if let Some(mi) = v.get("module_index").and_then(|x| x.as_object()) {
            for (k, val) in mi {
                g.module_order.push(k.clone());
                if let Some(s) = val.as_str() {
                    g.module_index.insert(k.clone(), s.to_string());
                }
            }
        }
        if let Some(fm) = v.get("file_to_module_id").and_then(|x| x.as_object()) {
            for (k, val) in fm {
                if let Some(s) = val.as_str() {
                    g.file_to_module_id.insert(k.clone(), s.to_string());
                }
            }
        }
        g
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

/// Files to feed the graph: git ls-files, else a SKIP_DIRS-bounded walk;
/// filtered to supported extensions, symlinks excluded.
pub fn discover_files(repo_root: &Path) -> Vec<PathBuf> {
    let exts = extraction::supported_extensions();
    let files = git_ls_files(repo_root).unwrap_or_else(|| walk_files(repo_root));
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

fn git_ls_files(repo_root: &Path) -> Option<Vec<PathBuf>> {
    let out = Command::new("git")
        .args(["ls-files", "--cached", "--others", "--exclude-standard"])
        .current_dir(repo_root)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| repo_root.join(l))
            .filter(|p| p.is_file())
            .collect(),
    )
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
            });
            graph.insert_module_index(module_for_file.clone(), module_id.clone());
            graph
                .file_to_module_id
                .insert(facts.path.clone(), module_id.clone());
        }
        for export in &extraction.exports {
            let node_id = symbol_node_id(&facts.path, &export.name);
            graph.insert_node(Node {
                id: node_id.clone(),
                label: export.name.clone(),
                kind: export.kind.clone(),
                source_file: Some(facts.path.clone()),
                source_line: Some(export.line),
            });
            graph
                .symbol_index
                .entry(export.name.to_lowercase())
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
                            relation: "imports".into(),
                            confidence: CONFIDENCE_EXTRACTED.into(),
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
                        relation: "imports".into(),
                        confidence,
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
                        relation: "imports".into(),
                        confidence: CONFIDENCE_EXTRACTED.into(),
                    });
                }
            }
        }
    }

    graph
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
    let dot_suffix = format!(".{module_path}");
    let slash_suffix = format!("/{module_path}");
    for (indexed_module, node_id) in internal_pairs {
        if indexed_module.ends_with(module_path)
            || indexed_module.ends_with(&dot_suffix)
            || indexed_module.ends_with(&slash_suffix)
        {
            return Some(node_id.clone());
        }
    }
    None
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
        });
        graph.insert_module_index(
            format!("external::{module_path}"),
            node_id.clone(),
        );
    }
    node_id
}

/// The architecture graph for a repo: returns the cached graph when the
/// fingerprint matches, else builds it from per-file facts and caches it.
pub fn get(repo_root: &Path) -> Graph {
    let files = discover_files(repo_root);
    let hashes = file_facts::file_hashes_for(&files, repo_root);
    let key = cache::architecture_fingerprint(&hashes);
    if let Some(cached) = cache::load(cache::NAMESPACE_ARCHITECTURE, &key, repo_root) {
        return Graph::from_json(&cached);
    }
    let all_facts = file_facts::get_many(&files, repo_root);
    let graph = build_from_facts(&all_facts);
    let _ = cache::save(
        cache::NAMESPACE_ARCHITECTURE,
        &key,
        &graph.to_json(),
        repo_root,
    );
    graph
}

/// The cached architecture graph if present — never builds.
pub fn load_cached(repo_root: &Path) -> Option<Graph> {
    let files = discover_files(repo_root);
    let hashes = file_facts::file_hashes_for(&files, repo_root);
    let key = cache::architecture_fingerprint(&hashes);
    cache::load(cache::NAMESPACE_ARCHITECTURE, &key, repo_root)
        .map(|c| Graph::from_json(&c))
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
/// dependents.
pub fn dependents_of<'a>(graph: &'a Graph, node_id: &str) -> Vec<&'a Edge> {
    let mut targets = vec![node_id.to_string()];
    let node = graph.nodes.get(node_id);
    if let Some(tn) = node {
        if let Some(sf) = &tn.source_file {
            if let Some(owner) = graph.file_to_module_id.get(sf) {
                targets.push(owner.clone());
            }
        }
    }
    // When `node_id` is a module, edges that resolved to a *symbol* in that
    // module's file (importer module → `file.py::symbol`) are also dependents
    // of the module. Reverse traversal arrives here holding an importer
    // module id (edge sources are modules), so without this the dependents
    // chain dead-ends after one hop on any symbol-resolved import. Mirrors
    // the symbol→owning-module widening `dependencies_of` does forward.
    let module_owns_target = |e: &Edge| -> bool {
        let target_node = match graph.nodes.get(&e.target) {
            Some(n) => n,
            None => return false,
        };
        let sf = match &target_node.source_file {
            Some(s) => s,
            None => return false,
        };
        graph.file_to_module_id.get(sf) == Some(&node_id.to_string())
    };
    let is_module = node.map(|n| n.kind == "module").unwrap_or(false);
    graph
        .edges
        .iter()
        .filter(|e| targets.contains(&e.target) || (is_module && module_owns_target(e)))
        .collect()
}

/// Edges originating from `node_id` (or its owning module) — its direct
/// dependencies.
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
        .filter(|e| sources.contains(&e.source))
        .collect()
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
