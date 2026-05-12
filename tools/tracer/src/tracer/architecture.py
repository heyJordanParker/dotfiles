"""Architecture graph — symbols and modules connected by cross-file edges.

The architecture layer answers questions per-file data cannot:
  * What other files import this symbol?
  * What does this file's symbol depend on transitively?
  * Which modules form a dependency cycle?

Built from per-file FileFacts via cross-file resolution (graphify's
extract.py:5788 pattern). Cached at `.tracer-cache/architecture/{key}.json`
where `key` is the fingerprint of all current per-file cache hashes — any
file change produces a new key, causing rebuild from current FileFacts.

Strict separation from the per-file layer:
  * Nodes are SYMBOLS (functions, classes) and MODULES — never files.
  * Edges are cross-file relationships — `imports`, `references`. Never
    intra-file structure (lines, blocks, calls within one function).
  * File paths live as a `source_file` ATTRIBUTE on nodes — never as a
    node themselves.

Confidence on every edge: EXTRACTED (literal in source — `import X`) /
INFERRED (deduced — name match without import evidence) / AMBIGUOUS
(multiple resolution candidates).
"""

from __future__ import annotations

import os
from dataclasses import asdict, dataclass, field
from pathlib import Path

from tracer import cache, file_facts


CONFIDENCE_EXTRACTED = "EXTRACTED"
CONFIDENCE_INFERRED = "INFERRED"
CONFIDENCE_AMBIGUOUS = "AMBIGUOUS"


@dataclass
class Node:
    """A symbol or module in the architecture graph.

    `id` is a stable string built from `source_file::symbol` for symbol
    nodes and `module::path` for module nodes. `kind` is one of
    "function", "class", "interface", "type", "constant", "module".
    Files are NEVER nodes — file paths are recorded as `source_file`.
    """

    id: str
    label: str
    kind: str
    source_file: str | None
    source_line: int | None


@dataclass
class Edge:
    source: str
    target: str
    relation: str       # "imports" or "references"
    confidence: str     # one of the CONFIDENCE_* constants


@dataclass
class Graph:
    nodes: dict[str, Node] = field(default_factory=dict)
    edges: list[Edge] = field(default_factory=list)
    # Index: lowercased symbol label -> node ids that share it
    symbol_index: dict[str, list[str]] = field(default_factory=dict)
    # Index: module path -> node id
    module_index: dict[str, str] = field(default_factory=dict)
    # Index: source_file (relative path) -> module node id. Built so we can
    # find the owning module of a symbol by its file, without re-running
    # language-aware path-to-module conversion at query time.
    file_to_module_id: dict[str, str] = field(default_factory=dict)

    def to_dict(self) -> dict:
        return {
            "nodes": {nid: asdict(node) for nid, node in self.nodes.items()},
            "edges": [asdict(edge) for edge in self.edges],
            "symbol_index": self.symbol_index,
            "module_index": self.module_index,
            "file_to_module_id": self.file_to_module_id,
        }

    @classmethod
    def from_dict(cls, data: dict) -> "Graph":
        return cls(
            nodes={nid: Node(**ndata) for nid, ndata in data.get("nodes", {}).items()},
            edges=[Edge(**edata) for edata in data.get("edges", [])],
            symbol_index=data.get("symbol_index", {}),
            module_index=data.get("module_index", {}),
            file_to_module_id=data.get("file_to_module_id", {}),
        )


SKIP_DIRS = {
    ".git", "node_modules", ".venv", "venv", "__pycache__", "dist", "build",
    ".next", ".tracer-cache", ".pytest_cache", ".mypy_cache", ".ruff_cache",
    "vendor", "worktrees", "trellis", "bedrock", "public", "storage",
    "bootstrap", ".lando", ".playwright", "playwright-report", "test-results",
}


def discover_files(repo_root: Path) -> list[Path]:
    """Return source files extractors handle, respecting .gitignore.

    Uses `git ls-files --cached --others --exclude-standard` when in a git
    repo — single subprocess, automatically respects .gitignore, never
    walks `node_modules`, `vendor`, `worktrees`, etc. Falls back to
    `os.walk` with SKIP_DIRS when outside a git repo.
    """
    from tracer.extraction import supported_extensions

    extensions = supported_extensions()
    files = _git_ls_files(repo_root) or _walk_files(repo_root)
    return [
        f for f in files
        if f.suffix.lower() in extensions and not f.is_symlink()
    ]


def _git_ls_files(repo_root: Path) -> list[Path] | None:
    """Tracked + untracked-but-not-ignored files via one git invocation."""
    import subprocess

    try:
        result = subprocess.run(
            ["git", "ls-files", "--cached", "--others", "--exclude-standard"],
            cwd=repo_root,
            capture_output=True,
            text=True,
            check=True,
            timeout=30,
        )
    except (subprocess.CalledProcessError, FileNotFoundError, subprocess.TimeoutExpired):
        return None

    out: list[Path] = []
    for line in result.stdout.splitlines():
        if not line:
            continue
        full = repo_root / line
        if full.is_file():
            out.append(full)
    return out


def _walk_files(repo_root: Path) -> list[Path]:
    """Fallback file walker for non-git directories."""
    found: list[Path] = []
    for root, dirs, files in os.walk(repo_root):
        dirs[:] = [d for d in dirs if d not in SKIP_DIRS and not d.startswith(".")]
        for name in files:
            full = Path(root) / name
            if full.is_file():
                found.append(full)
    return found


def _module_node_id(module_path: str) -> str:
    return f"module::{module_path}"


def _symbol_node_id(source_file: str, symbol: str) -> str:
    return f"{source_file}::{symbol}"


def _build_from_facts(all_facts: list[file_facts.FileFacts]) -> Graph:
    """Cross-file resolution.

    Phase 1: walk every FileFacts and create nodes for each export plus a
    module node for each file with extraction data.

    Phase 2: walk every import. Resolve the target via the indexes built
    in phase 1. EXTRACTED when we find exactly one matching export or
    module; AMBIGUOUS when multiple candidates; INFERRED-against-external
    when the import target is outside our extracted scope.
    """
    graph = Graph()

    # Phase 1: nodes
    for facts in all_facts:
        if facts.extraction is None:
            continue

        # File-as-module node — represents the file's importable surface
        module_for_file = _file_to_module(facts.path, facts.language)
        module_id = _module_node_id(module_for_file)
        if module_id not in graph.nodes:
            graph.nodes[module_id] = Node(
                id=module_id,
                label=module_for_file,
                kind="module",
                source_file=facts.path,
                source_line=1,
            )
            graph.module_index[module_for_file] = module_id
            graph.file_to_module_id[facts.path] = module_id

        # Each exported symbol becomes a node
        for export in facts.extraction.exports:
            node_id = _symbol_node_id(facts.path, export.name)
            graph.nodes[node_id] = Node(
                id=node_id,
                label=export.name,
                kind=export.kind,
                source_file=facts.path,
                source_line=export.line,
            )
            graph.symbol_index.setdefault(export.name.lower(), []).append(node_id)

    # Phase 2: edges
    for facts in all_facts:
        if facts.extraction is None:
            continue
        importer_module_id = _module_node_id(
            _file_to_module(facts.path, facts.language)
        )

        for import_decl in facts.extraction.imports:
            # Try to resolve the target module
            target_module_id = _resolve_module(graph, import_decl.module, facts.language)

            if import_decl.symbol is not None:
                # `from X import Y` — Y might be a module (`from tracer import
                # cache`) or a symbol (`from tracer.cache import cache_root`).
                # Try module-as-Y first; if no match, fall back to symbol-as-Y.
                combined_module = (
                    f"{import_decl.module}.{import_decl.symbol}"
                    if facts.language == "python"
                    else f"{import_decl.module}/{import_decl.symbol}"
                )
                module_candidate = _resolve_module(
                    graph, combined_module, facts.language
                )
                if module_candidate is not None:
                    graph.edges.append(
                        Edge(
                            source=importer_module_id,
                            target=module_candidate,
                            relation="imports",
                            confidence=CONFIDENCE_EXTRACTED,
                        )
                    )
                    continue

                # Symbol-as-Y path
                candidates = graph.symbol_index.get(import_decl.symbol.lower(), [])
                target_id = _select_best_symbol(
                    candidates, target_module_id, graph
                )
                confidence = _classify_confidence(target_id, candidates, target_module_id)
                if target_id is None and target_module_id is not None:
                    target_id = target_module_id
                if target_id is None:
                    target_id = _ensure_external(graph, import_decl.module)
                graph.edges.append(
                    Edge(
                        source=importer_module_id,
                        target=target_id,
                        relation="imports",
                        confidence=confidence,
                    )
                )
            else:
                # `import X` style — edge from importer to target module
                if target_module_id is None:
                    target_module_id = _ensure_external(graph, import_decl.module)
                graph.edges.append(
                    Edge(
                        source=importer_module_id,
                        target=target_module_id,
                        relation="imports",
                        confidence=CONFIDENCE_EXTRACTED,
                    )
                )

    return graph


def _file_to_module(relative_path: str, language: str | None) -> str:
    """Convert a file path to a language-idiomatic module string.

    Python: `tracer/cache.py` -> `tracer.cache`
    TypeScript / PHP: keep the path-shaped form for now (matches how
    they're written in `import` / `use` statements).
    """
    p = Path(relative_path)
    stem = str(p.with_suffix(""))
    if language == "python":
        return stem.replace(os.sep, ".")
    return stem.replace(os.sep, "/")


def _resolve_module(graph: Graph, module_path: str, language: str | None) -> str | None:
    """Find the node id for a module reference, or None if not in scope.

    Strategy:
      1. Exact match against module_index.
      2. Suffix match — our index stores paths from the repo root, but
         imports reference short names (`tracer.file_facts` is what users
         write; `tools.tracer.src.tracer.file_facts` is what we index).
         A suffix match handles this case for any language.

    Skips synthetic `external::` nodes; those are resolution targets only,
    never sources.
    """
    if module_path in graph.module_index:
        return graph.module_index[module_path]

    candidates = [
        (indexed, node_id)
        for indexed, node_id in graph.module_index.items()
        if not indexed.startswith("external::")
    ]

    if language == "php":
        # PHP `App\Models\User` → indexed as path-style `App/Models/User`
        slashed = module_path.replace("\\", "/").lower()
        for indexed_module, node_id in candidates:
            if indexed_module.lower().endswith(slashed):
                return node_id
        return None

    # Python / TypeScript / unknown — suffix match either direction.
    for indexed_module, node_id in candidates:
        if (
            indexed_module.endswith(module_path)
            or indexed_module.endswith(f".{module_path}")
            or indexed_module.endswith(f"/{module_path}")
        ):
            return node_id
    return None


def _select_best_symbol(
    candidates: list[str], target_module_id: str | None, graph: Graph
) -> str | None:
    """Pick the candidate node id that best matches the named import.

    Prefer a candidate whose source file is the resolved target module. If
    target module is unknown, return the candidate only when there's
    exactly one (avoid AMBIGUOUS resolution to a wrong file).
    """
    if not candidates:
        return None
    if target_module_id is not None:
        target_module_node = graph.nodes.get(target_module_id)
        if target_module_node is not None:
            for candidate in candidates:
                node = graph.nodes.get(candidate)
                if node and node.source_file == target_module_node.source_file:
                    return candidate
    if len(candidates) == 1:
        return candidates[0]
    return None


def _classify_confidence(
    resolved: str | None, candidates: list[str], target_module_id: str | None
) -> str:
    if resolved is not None and target_module_id is not None:
        return CONFIDENCE_EXTRACTED
    if resolved is not None:
        return CONFIDENCE_INFERRED
    if len(candidates) > 1:
        return CONFIDENCE_AMBIGUOUS
    return CONFIDENCE_EXTRACTED  # external module — import statement IS literal


def _ensure_external(graph: Graph, module_path: str) -> str:
    """Create or fetch a synthetic node for a module outside our scope."""
    node_id = _module_node_id(f"external::{module_path}")
    if node_id not in graph.nodes:
        graph.nodes[node_id] = Node(
            id=node_id,
            label=module_path,
            kind="module",
            source_file=None,
            source_line=None,
        )
        graph.module_index[f"external::{module_path}"] = node_id
    return node_id


def get(repo_root: Path | None = None) -> Graph:
    """Return the architecture graph, building from cache if possible.

    Cache key is the fingerprint of all current per-file cache hashes. Any
    per-file change produces a new key, causing a fresh build. Build is
    cheap because per-file extraction is cached.
    """
    root = repo_root or cache.repo_root_for(".")
    files = discover_files(root)
    hashes = file_facts.file_hashes_for(files, root)
    key = cache.architecture_fingerprint(hashes)

    cached = cache.load(cache.NAMESPACE_ARCHITECTURE, key, root)
    if cached is not None:
        try:
            return Graph.from_dict(cached)
        except (KeyError, TypeError):
            pass

    all_facts = file_facts.get_many(files, root)
    graph = _build_from_facts(all_facts)
    cache.save(cache.NAMESPACE_ARCHITECTURE, key, graph.to_dict(), root)
    return graph


def load_cached(repo_root: Path | None = None) -> Graph | None:
    """Cache-only load — never triggers a build. Used by hot-path commands
    like `trace read` that want graph context for free if available, but
    don't want to pay the cold-cache build cost on a single read."""
    root = repo_root or cache.repo_root_for(".")
    files = discover_files(root)
    hashes = file_facts.file_hashes_for(files, root)
    key = cache.architecture_fingerprint(hashes)
    cached = cache.load(cache.NAMESPACE_ARCHITECTURE, key, root)
    if cached is None:
        return None
    try:
        return Graph.from_dict(cached)
    except (KeyError, TypeError):
        return None


# Query API — graph traversal helpers used by the commands

def find_symbols(graph: Graph, name: str) -> list[Node]:
    """All nodes (symbols or modules) whose label matches `name` (case-insensitive).

    First tries the symbol index (functions, classes, etc.). Falls back to
    module nodes whose path basename matches — so `find_symbols("cache")`
    returns the `tracer.cache` module when there's no exported symbol of
    that name. This is how `callers <module-name>` and
    `dependents <module-name>` resolve when the user passes a module.
    """
    lower = name.lower()
    matches = [graph.nodes[nid] for nid in graph.symbol_index.get(lower, [])]
    if matches:
        return matches

    # Module fallback: match by last path segment (.cache → tracer.cache)
    module_matches: list[Node] = []
    for module_path, node_id in graph.module_index.items():
        # Skip external::X synthetic nodes; the user is asking about local code
        if module_path.startswith("external::"):
            continue
        # Last segment of dotted (Python) or slashed (TS/PHP) path
        last_segment = module_path.replace("/", ".").split(".")[-1]
        if last_segment.lower() == lower:
            node = graph.nodes.get(node_id)
            if node:
                module_matches.append(node)
    return module_matches


def dependents_of(graph: Graph, node_id: str) -> list[Edge]:
    """Direct: every edge whose target is `node_id` or whose target is the
    module containing `node_id`. The module lookup goes through the
    `file_to_module_id` reverse index built at graph construction —
    avoids guessing the language at query time.
    """
    target_node = graph.nodes.get(node_id)
    targets = {node_id}
    if target_node and target_node.source_file:
        owner_id = graph.file_to_module_id.get(target_node.source_file)
        if owner_id:
            targets.add(owner_id)
    return [edge for edge in graph.edges if edge.target in targets]


def dependencies_of(graph: Graph, node_id: str) -> list[Edge]:
    """Direct: every edge whose source is the symbol's owning module."""
    target_node = graph.nodes.get(node_id)
    if target_node is None:
        return []
    sources = {node_id}
    if target_node.source_file:
        owner_id = graph.file_to_module_id.get(target_node.source_file)
        if owner_id:
            sources.add(owner_id)
    return [edge for edge in graph.edges if edge.source in sources]


def transitive_dependents(graph: Graph, node_id: str, max_depth: int = 5) -> list[tuple[Node, int]]:
    """BFS over reverse edges. Returns (node, depth) pairs."""
    visited = {node_id}
    frontier = [(node_id, 0)]
    out: list[tuple[Node, int]] = []
    while frontier:
        current_id, depth = frontier.pop(0)
        if depth >= max_depth:
            continue
        for edge in dependents_of(graph, current_id):
            if edge.source in visited:
                continue
            visited.add(edge.source)
            source_node = graph.nodes.get(edge.source)
            if source_node:
                out.append((source_node, depth + 1))
                frontier.append((edge.source, depth + 1))
    return out


def transitive_dependencies(
    graph: Graph, node_id: str, max_depth: int = 5
) -> list[tuple[Node, int]]:
    """BFS over forward edges. Returns (node, depth) pairs."""
    visited = {node_id}
    frontier = [(node_id, 0)]
    out: list[tuple[Node, int]] = []
    while frontier:
        current_id, depth = frontier.pop(0)
        if depth >= max_depth:
            continue
        for edge in dependencies_of(graph, current_id):
            if edge.target in visited:
                continue
            visited.add(edge.target)
            target_node = graph.nodes.get(edge.target)
            if target_node:
                out.append((target_node, depth + 1))
                frontier.append((edge.target, depth + 1))
    return out
