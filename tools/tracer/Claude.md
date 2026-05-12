# Tracer
v3.2 | Updated: 2026-05-11

## Why

Code-intelligence CLI for AI coding agents working in-repo. Each command returns dense, structured information about a file, symbol, or directory in a single response: complexity profile, git lifecycle (commits, age, rename history, deploy-branch presence), architectural-graph edges (callers, immediate dependencies), nearest project rules from `Claude.md` ancestors, leading docblock, and language-aware structure (methods, classes, imports). A single `trace info <path>` contains what an asking agent would otherwise reconstruct from several grep / read / log invocations.

The chain that consumes tracer (skill + explorer agent) is a separate concern. Tracer's responsibility ends at the output: rich, accurate, attribute-only. What anyone does with that output is theirs.

Standalone developers who want the same unified interface are a secondary audience — the CLI works without Claude Code.

## What

A Python CLI exposing 16 commands that orchestrate external code-intelligence binaries plus an in-process two-layer disk cache: per-file facts and an architecture graph of symbols and modules connected by cross-file edges. Reads and directory listings include passive context: lifecycle state (new / renamed / modified / settled), age, and complexity rank inline.

### Commands

**Per-file commands** (read the `file/` cache):
- `doctor` — verify required external binaries; print per-platform install instructions
- `read <file> [<method>]` — cleaned read; method by name or full file; preserves comments, cuts fluff
- `list <dir>` — one-level annotated ls; files + sub-directories with file count, ccn, recency per entry
- `survey <path>` — repo-wide complexity distribution via scc
- `tree <path>` — annotated file tree with complexity ranks (recursive)
- `info <path>` — complexity structure + architectural overview of a file or directory
- `structure <file>` — methods, properties, variables, imports, exports for one file
- `grep <pattern>` — text search via ripgrep with per-match enrichment
- `struct <pattern> -l <lang>` — structural AST search via ast-grep with per-match enrichment
- `history <file>` — git log/blame summary

**Architecture commands** (read the `architecture/` cache):
- `defines <symbol>` — all places a symbol is defined
- `callers <symbol>` — direct callers of a symbol or module via the architecture graph
- `downstream <symbol> [--depth N]` — transitive dependents (BFS over reverse edges)
- `downstream --path <path> [--limit N]` — top-N most-depended-on symbols in a path (architectural centrality)
- `upstream <symbol> [--depth N]` — transitive dependencies (BFS over forward edges)
- `upstream --path <path> [--limit N]` — top-N highest-coupling symbols in a path
- `symbols <file>` — module-level symbols of a file from the architecture graph

**Cache management**:
- `cache stats` — entries and size per namespace
- `cache clear [--namespace file|architecture] [--all]` — invalidate cache entries

### External binaries wrapped

`ast-grep` for structural search, `scc` for repo-wide complexity sweep, `universal-ctags` for symbol fallback in `structure`, `ripgrep` for text grep, `git` for history. Tree-sitter (Python, TypeScript, PHP) ships as Python deps and is bundled in the zipapp.

### Two-layer cache (the architectural decision)

`.tracer-cache/` at the repo root, with two namespaces that **never read each other**:

- **`file/`** — per-file facts: complexity, loc, language, raw imports list, raw exports list, git activity. One entry per file. Cache key = `sha256(SCHEMA_VERSION + file_contents + relative_path)`. Used by per-file commands and as enrichment in `grep`/`struct`. Flat dicts. No edges. No graph membership.
- **`architecture/`** — the cross-file architecture graph. Nodes are **symbols and modules — never files**. Edges are cross-file relationships (`imports`) tagged with `confidence` (`EXTRACTED | INFERRED | AMBIGUOUS`, vocabulary borrowed from graphify). One entry per repo state, keyed by the SHA-256 fingerprint of all current `file/` cache entry hashes. Built incrementally by re-running cross-file resolution whenever per-file caches invalidate.

Joins between the two layers happen at the rendering layer (in command output) at query time only — never persisted, never cached together.

### Requirements

- Every command except `doctor` must call `require_dependencies()` before doing work — missing binaries hard-fail with the same per-platform install instructions `doctor` prints
- Every query must return rich, structured output enriched with file-level facts and (for architecture commands) graph-derived edges. `--json` must be accepted on every command
- Each command's output includes every adjacent fact a reader is likely to want next about the same subject — callers, dependencies, lifecycle, presence, rules, complexity, leading docblock — so a single call carries the information a several-call exploration would otherwise gather
- Every field in an output names a real attribute of the file or symbol it describes. Output is descriptive, not procedural — no imperative prose telling the reader what to run, look at, or check
- Line numbers in any tracer output must be the authoritative source for that file at the moment of the call — extracted via lizard / tree-sitter / direct file read, never inferred or cached separately from content
- Must work standalone via `pipx install tracer` — Claude Code is one consumer, not a prerequisite
- One responsibility per file under `src/tracer/commands/` — each command is its own module registered on the click group in `__main__.py`
- Per-file commands read only the `file/` namespace. Architecture commands read only the `architecture/` namespace. The two layers join only in rendering code, never in storage
- `doctor` must be cross-platform — print install commands for macOS, Debian/Ubuntu, and Windows. Never assume `brew`
- The architecture graph models **symbols and modules**, never files. File paths live as a `source_file` attribute on nodes

### Boundaries

- Never bundle the external binaries — users install via their platform's package manager. `doctor` only verifies and instructs
- Never depend on Claude Code, its session state, or its tools at runtime — the CLI is invoked from a subprocess and must stand alone
- Never write commands that bypass `require_dependencies()` — silent fallback to a missing binary produces wrong output, not a clear error
- Never put files as nodes in the architecture graph — files are an attribute (`source_file`) on symbol/module nodes, never the node itself. This is the layer-separation invariant; violating it brings back graphify's defensive `_is_file_node` filtering
- Never cross-read between the two cache namespaces — per-file commands stay in `file/`, architecture commands stay in `architecture/`. Joins happen in command code at render time, are disposable, and never persist
- Never store cache entries with stale schemas — bump `cache.SCHEMA_VERSION` whenever extraction or FileFacts shape changes; old entries become unreachable automatically

## Architecture

```
tracer/
├── pyproject.toml                       hatchling build; click + multilspy + lizard + tree-sitter
├── Readme.md
├── scripts/
│   └── build-zipapp.sh                  → packages/claude/bin/trace plugin artifact
└── src/tracer/
    ├── __main__.py                      click group; registers 16 commands
    ├── passive_context.py                renders one-line lifecycle/complexity shoulder for any FileFacts
    ├── deps.py                          require_dependencies; per-platform install hints
    ├── cache.py                         two-namespace disk cache (file/, architecture/)
    ├── repo_context.py                  scc-based repo-wide complexity_p95 (disk-cached)
    ├── enrich.py                        per-file enrichment for grep/struct (reads file_facts)
    ├── file_facts.py                    per-file extraction + file/ namespace owner
    ├── architecture.py                  cross-file resolution + architecture/ namespace owner
    ├── lsp_client.py                    multilspy wrapper (legacy; not used by current commands)
    ├── language.py                      extension → language mapping (legacy)
    ├── extraction/                      tree-sitter per-language extractors
    │   ├── dispatch.py                  extension → extractor + ExtractionResult schema
    │   ├── python.py                    Python imports + module-level definitions
    │   ├── typescript.py                TS/TSX/JS/JSX imports + exports
    │   └── php.py                       PHP `use` statements + class/interface/function
    └── commands/                        one file per CLI command (16 total)
        ├── doctor.py    read.py         tree.py       survey.py
        ├── info.py      structure.py    grep.py       struct_.py
        ├── history.py   defines.py      callers.py    symbols.py
        ├── upstream.py  downstream.py   cache.py      list_.py
```

## Workflow

### Distribution

Three audiences, three paths — all from this single source tree:

- **Dotfiles users (Jordan)** — `pipx install -e ~/dotfiles/tools/tracer` once; edits to source land live, no rebuild
- **Plugin users** — built zipapp at `packages/claude/bin/trace` ships with the Claude Code plugin and lands on `PATH` automatically when the plugin is enabled. 9 of 15 commands work in the zipapp. The 3 LSP commands and the 3 architecture commands that need tree-sitter (whose grammars are C extensions) require `pipx install tracer` for full functionality
- **External users** — `pipx install tracer` from PyPI

### Building the plugin artifact

```bash
./scripts/build-zipapp.sh
```

Bundles `tracer` plus pure-Python deps (click, lizard, multilspy) into a single executable zipapp at `packages/claude/bin/trace`. Tree-sitter grammars and psutil have C extensions that can't load from a zipapp — those commands fail with an actionable "Run: pipx install tracer" message in the zipapp.

### Adding a new command

1. Create `src/tracer/commands/<name>.py` exposing a `command` click callback
2. Call `require_dependencies()` at the top of the callback unless the command is itself a dependency check
3. Decide which cache namespace it reads (`file/` for per-file data, `architecture/` for cross-file relationships) — never read both directly; if the command joins them, do it in the command's render code, never in the cache layer
4. Register on the group in `__main__.py`: `main.add_command(<name>.command, name="<name>")`
5. Document the command in `Readme.md` and in the `trace` skill's command table

### Adding a new language extractor

1. Add `src/tracer/extraction/<lang>.py` exposing `extract(source: bytes, path: str) -> ExtractionResult`
2. Wire its extension(s) and factory into `extraction/dispatch.py`'s `_EXTRACTORS` table
3. Add the per-language tree-sitter package to `pyproject.toml`
4. Bump `cache.SCHEMA_VERSION` if the FileFacts shape changes; otherwise existing cache entries for the new extension simply stay missing until invalidated

### Cache invalidation

- Per-file (`file/`): file SHA changes → cache key changes → next read re-extracts
- Architecture (`architecture/`): any per-file SHA change → fingerprint changes → next read rebuilds the graph from current per-file facts (cheap because per-file is cached)
- Schema changes: bump `cache.SCHEMA_VERSION` in `cache.py`. Old entries become unreachable across all namespaces

## Ledger

- v3.1: Passive lifecycle context on reads and listings
- v3.0: Directional upstream/downstream for graph queries
- v2.0: Split cache to isolate per-file from architecture
- v1.0: Baseline for code-intelligence CLI
