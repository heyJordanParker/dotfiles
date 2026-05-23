# Tracer
v4.10 | Updated: 2026-05-23

## Why

Code-intelligence CLI for AI coding agents working in-repo. Each command returns dense, structured information about a file, symbol, or directory in a single response: complexity profile, git lifecycle (commits, age, rename history, deploy-branch presence), architectural-graph edges (callers, immediate dependencies), nearest project rules from `Claude.md` ancestors, leading docblock, and language-aware structure (methods, classes, imports). A single `trace info <path>` contains what an asking agent would otherwise reconstruct from several grep / read / log invocations.

The chain that consumes tracer (skill + explorer agent) is a separate concern. Tracer's responsibility ends at the output: rich, accurate, attribute-only. What anyone does with that output is theirs.

Standalone developers who want the same unified interface are a secondary audience — the CLI works without Claude Code.

## What

A single static Rust binary (`trace`) exposing 24 commands that orchestrate external code-intelligence binaries plus an in-process two-layer disk cache: per-file facts and an architecture graph of symbols and modules connected by cross-file edges. Reads and directory listings include passive context: lifecycle state (new / renamed / modified / settled), age, and complexity rank inline. Per-function cyclomatic complexity is computed by an in-process tree-sitter AST decision-node walker covering ten languages — the single CCN backend, no external complexity tool.

### Commands

**Per-file commands** (read the `file/` cache):
- `doctor` — verify required external binaries; print per-platform install instructions
- `read <paths...> [--method <name>] [--at <ref>] [--lines L1:L2] [--between S E] [--diff] [--raw] [--docs]` — cleaned read; one or more paths; whole file, method by name, line range, or anchor section; worktree or git ref; optional symbol-level diff vs worktree when reading at a ref; `--raw` skips cleaning. Project-docs are not injected by default; `--docs` opts in to inline project-docs content
- `list <dir> [--all]` — one-level annotated ls; files + sub-directories with file count, ccn, recency per entry; `--all` includes hidden entries
- `survey <path>` — repo-wide language + LOC + complexity distribution via scc
- `tree <path> [--depth N]` — annotated file tree with complexity ranks (recursive; default depth 4)
- `info <path> [--brief]` — complexity structure + architectural overview of a file or directory; `--brief` trims to the headline facts
- `structure <file>` — methods, properties, variables, imports, exports for one file
- `grep <pattern> [-l <lang>] [--path <path>]` — text search via ripgrep with per-match enrichment
- `struct <pattern> -l <lang> [--path <path>]` — structural AST search via ast-grep with per-match enrichment
- `glob <pattern> [<base>]` — full-path pattern search (Claude Glob shape: `**` recursive, gitignore-respecting). Returns the complete deterministically-sorted match list; bare paths by default, `--details` adds per-line ccn + rank + lifecycle shoulder
- `find <pattern> [<base>] [--path <p>] [--exclude <p>]... [--type f|d] [--limit N] [--sort complexity|recent|path]` — filename-pattern search with complexity rank + lifecycle shoulder per match; defaults to files, path-sorted, limit 200
- `history <file> | <file> <symbol> | --contains <pattern>` — whole-file log, function-line history (git log -L), or pickaxe (git log -S)
- `blame <file> [<symbol>] [--lines L1:L2]` — symbol-aware blame collapsed into per-region commit summaries
- `diff [--base <ref>] [--symbols]` — files (or module-level symbols) changed vs a base ref, load-bearing first
- `status [--state <s>]` — working-tree dirty set ordered by blast radius
- `context [<path>] [--directory]` — session-start primer (no args) or single-file enrichment (path arg)
- `docs <path> [--directory]` — the deduped project-docs set (`Claude.md`/`CLAUDE.md`/`.claude`/rules ancestors) for a path; shares `read`'s per-session "read once" dedupe

**Architecture commands** (read the `architecture/` cache):
- `defines <symbol>` — all places a symbol is defined
- `callers <symbol>` — direct callers of a symbol or module via the architecture graph
- `downstream <symbol> [--depth N]` — transitive dependents (BFS over reverse edges)
- `downstream --path <path> [--limit N]` — top-N most-depended-on symbols in a path (architectural centrality)
- `upstream <symbol> [--depth N]` — transitive dependencies (BFS over forward edges)
- `upstream --path <path> [--limit N]` — top-N highest-coupling symbols in a path
- `symbols <file>` — module-level symbols of a file from the architecture graph

**Cache management**:
- `cache build [<path>]` — prebuild per-file facts + architecture graph so the first agent query is fast
- `cache stats` — entries and size per namespace
- `cache clear [--namespace file|architecture] [--all]` — invalidate cache entries

### External binaries wrapped

`ast-grep` for structural search, `scc` for repo-wide complexity sweep, `universal-ctags` for symbol fallback in `structure`, `ripgrep` for text grep, `git` for history. Tree-sitter grammars (Python, TypeScript/TSX/JSX, PHP, plus bash, lua, go, rust, ruby, java, c for the CCN walker) are compiled into the binary — no runtime install.

### Two-layer cache (the architectural decision)

`.tracer-cache/` at the repo root, with two namespaces that **never read each other**:

- **`file/`** — per-file facts: complexity, loc, language, raw imports list, raw exports list, git activity. One entry per file. Cache key = `sha256("v{SCHEMA_VERSION}|ccn:{backend}\0" + file_contents + "\0" + relative_path)`. Used by per-file commands and as enrichment in `grep`/`struct`. Flat records. No edges. No graph membership.
- **`architecture/`** — the cross-file architecture graph. Nodes are **symbols and modules — never files**. Edges are cross-file relationships (`imports`) tagged with `confidence` (`EXTRACTED | INFERRED | AMBIGUOUS`). One entry per repo state, keyed by the SHA-256 fingerprint of all current `file/` cache entry hashes. Rebuilt by re-running cross-file resolution whenever per-file caches invalidate.

Joins between the two layers happen at the rendering layer (in command output) at query time only — never persisted, never cached together.

### Requirements

- Every command except `doctor` must verify required external binaries before doing work — missing binaries hard-fail with the same per-platform install instructions `doctor` prints
- Every query must return rich, structured output enriched with file-level facts and (for architecture commands) graph-derived edges. `--json` must be accepted on every command
- Every value-producing command's `run` returns its `serde_json::Value` to the top level (`Result<serde_json::Value>`); it renders human text only when `--json` is off, and never prints the JSON itself. `output.rs` is the single place that decides stdout: filtered jq results, the `jsonfmt` JSON, or nothing. Commands with no JSON form (`doctor`, `cache build`, `cache clear`, `context`) keep `Result<()>` and are out of the filter contract
- `--filter '<jq>'` is a global option, accepted on every value-producing command; it requires `--json` explicitly (never implied) and runs an in-process jq (`jaq`) over the value. It is the only partial-output path — there is no supported pipe
- Each command's output includes every adjacent fact a reader is likely to want next about the same subject — callers, dependencies, lifecycle, presence, rules, complexity, leading docblock — so a single call carries the information a several-call exploration would otherwise gather
- Every field in an output names a real attribute of the file or symbol it describes. Output is descriptive, not procedural — no imperative prose telling the reader what to run, look at, or check
- Line numbers in any output must be the authoritative source for that file at the moment of the call — extracted via the tree-sitter AST or direct file read, never inferred or cached separately from content
- No lite-facts shortcut: every file that appears in any listing gets real parsed per-function CCN, real function count, real max CCN — `file_facts::get` always does the real extraction on cache miss
- Must work standalone — Claude Code is one consumer, not a prerequisite
- One responsibility per file under `src/commands/` — each command is its own module registered on the clap subcommand enum in `main.rs`
- Per-file commands read only the `file/` namespace. Architecture commands read only the `architecture/` namespace. The two layers join only in rendering code, never in storage
- `doctor` must be cross-platform — print install commands for macOS, Debian/Ubuntu, and Windows. Never assume `brew`
- The architecture graph models **symbols and modules**, never files. File paths live as a `source_file` attribute on nodes
- `--json` and on-disk cache entries use one stable byte format (ASCII-escaped non-ASCII scalars, fixed separators) produced by `jsonfmt` — every serializing path goes through it

### Boundaries

- Never bundle the external binaries — users install via their platform's package manager. `doctor` only verifies and instructs
- Never depend on Claude Code, its session state, or its tools at runtime — the CLI is invoked from a subprocess and must stand alone
- Never write commands that skip the dependency check — silent fallback to a missing binary produces wrong output, not a clear error
- Never put files as nodes in the architecture graph — files are an attribute (`source_file`) on symbol/module nodes, never the node itself. This is the layer-separation invariant
- Never cross-read between the two cache namespaces — per-file commands stay in `file/`, architecture commands stay in `architecture/`. Joins happen in command code at render time, are disposable, and never persist
- Never store cache entries with stale schemas — bump `cache::SCHEMA_VERSION` whenever extraction or `FileFacts` shape changes; old entries become unreachable automatically across all namespaces
- Never rename the `.tracer-cache/` directory — it is an on-disk contract shared with every repo's `.gitignore` and with stale caches in the wild
- Never rename the `TRACE_BIN` or `TRACER_CCN_BACKEND` environment variable names — they are an external protocol shared with the test suite, hooks, and docs
- Never resolve `repo_root` per file inside a repo-wide loop — resolve it once outside the loop and thread it through; a per-iteration `git rev-parse --show-toplevel` is 15-30ms each and dominates large-repo runs
- Never walk the whole tree before filtering for repo-wide enumeration — use `repo_files::tracked_files` (git ls-files) or `walk_files` with dir-level `SKIP_DIRS` pruning, never an unbounded recursive walk that descends `node_modules` / `vendor` / worktrees
- Never reconstruct the bulk git-activity map inside a loop — build it once via `git_activity::bulk_cached` and index the returned map per iteration; rebuilding per-iteration makes the loop O(N²)
- Never shell out to the `jq` binary or bundle it — `--filter` is served in-process by the `jaq` crate so `trace` stays a single static binary with no new external dependency for `doctor` to verify
- Never let a value-producing command print its own JSON or decide `--filter` — it returns the value; `output.rs` owns emit. Never imply `--json` from `--filter`; the combination is validated up front in `output::guard`, before the command runs

## Architecture

```
tracer/                                 cargo workspace root (tracer package + xtask member; tests excluded)
├── Cargo.toml                          workspace table + tracer manifest; clap + serde + rayon + tree-sitter (10 grammars) + jaq
├── Cargo.lock                          workspace lock (carries xtask; never mirrored)
├── .cargo/config.toml                  `cargo xtask` alias → run the xtask member
├── Readme.md
├── xtask/                              workspace automation crate (cargo xtask); owns the plugin-payload producer
│   └── src/main.rs                     `sync-dist` [`--check`]: regenerate / drift-guard the tracer-dist mirror
├── src/
│   ├── main.rs                         clap subcommand enum; dispatches all 24 commands
│   ├── cache.rs                        two-namespace disk cache (file/, architecture/)
│   ├── file_facts.rs                   per-file extraction + file/ namespace owner
│   ├── architecture.rs                 cross-file resolution + architecture/ namespace owner
│   ├── ccn.rs                          tree-sitter AST per-function complexity (10 languages)
│   ├── git_activity.rs                 single-pass git log parser + per-file lifecycle facts
│   ├── repo_context.rs                 scc-based repo-wide complexity_p95 (disk-cached)
│   ├── repo_files.rs                   git ls-files / SKIP_DIRS-bounded enumeration
│   ├── passive_context.rs              one-line lifecycle/complexity shoulder for any FileFacts
│   ├── digest.rs                       leading_comment, top_callers, immediate_dependencies, nearest_doc
│   ├── jsonfmt.rs                      central JSON byte format (wire + on-disk)
│   ├── output.rs                       single emit site: guard(--filter⇒--json), jq-filter, or jsonfmt print
│   ├── filter.rs                       in-process jq over a command's Value (jaq; replaces `| jq`)
│   ├── pathval.rs                      path-argument validation (non-zero + real error)
│   ├── facts.rs                        shared fact record types
│   ├── extraction/                     tree-sitter per-language import/export extractors
│   │   ├── mod.rs                      ExtractionResult schema + dispatch
│   │   ├── python.rs   typescript.rs   php.rs
│   └── commands/                       one file per CLI command + shared command helpers
│       ├── doctor.rs    read.rs        tree.rs       survey.rs
│       ├── info.rs      structure.rs   grep.rs       struct_.rs
│       ├── glob.rs      find.rs        history.rs    blame.rs
│       ├── diff.rs      status.rs      context.rs    docs.rs
│       ├── defines.rs   callers.rs     symbols.rs    upstream.rs
│       ├── downstream.rs cache.rs      list_.rs
│       ├── enrich.rs                   shared per-match enrichment for `grep` + `struct` (not a command)
│       ├── glob_match.rs               shared shell-glob basename matcher for `find` (not a command)
│       └── nested_memory.rs            shared project-docs walk-up + session dedupe (`read`, `docs`)
└── tests/                              black-box CLI test suite (own crate; see tests/Claude.md)
```

## Workflow

### Distribution

Two audiences, two paths — all from this single source tree:

- **Dotfiles users (Jordan)** — `setup.sh` runs `cargo build --release` in `tools/tracer` and installs the binary to `~/.local/bin/trace` (on PATH via the `bin` stow target). Re-run `setup.sh` (or rebuild + reinstall) after source changes — there is no editable install; a native binary is rebuilt, not symlinked
- **Plugin users** — `packages/claude/bin/trace` is a POSIX launcher that ships with the Claude Code plugin and lands on `PATH` automatically when the plugin is enabled. It resolves a runnable binary in this order: (1) a committed prebuilt for the host (`tracer-dist/bin/mac-arm64` or `linux-x86_64`); (2) a previously locally-built cached binary; (3) `cargo build --release` from the generated crate mirror in `tracer-dist/crate/`, cached for next time; (4) no Rust toolchain → an actionable "install Rust" error with non-zero exit, the same shape `doctor` uses for a missing external binary. The SessionStart primer hook swallows any failure and exits 0 so sessions never break; a direct `trace …` call gets the real error

### Building / refreshing the plugin payload

The committed payload under `packages/claude/bin/tracer-dist/` is: two prebuilt binaries (`bin/mac-arm64/trace`, `bin/linux-x86_64/trace`), `crate/` for the build-from-source fallback, and a `built/` cache the launcher writes when it builds locally. The plugin marketplace copies only `packages/claude/`, so the build-from-source fallback needs the crate source physically inside that payload — it cannot reach the canonical `tools/tracer/`.

`crate/` is a **generated mirror** of the tracer source, never hand-edited — it is derived, so edits belong in the canonical tracer source and would be overwritten. The tracer crate owns its plugin-payload producer the way `cargo publish` is a crate's release path: `tools/tracer/` is a cargo workspace whose `xtask` member is the mirror's single sanctioned writer, run from inside the tracer directory. `cargo xtask sync-dist` regenerates the mirror (idempotent); `cargo xtask sync-dist --check` is the drift guard, exiting non-zero with a "do not hand-edit" message when `crate/` is out of sync with the tracer source. `setup.sh` runs the xtask after the tracer release build + binary install, so the mirror is regenerated as part of the normal setup flow. `target/` and `built/` are build outputs and are never mirrored.

The mirror is the tracer crate source with one reshape forced by isolation: the marketplace copies only `packages/claude/`, so the mirror is built standalone with no workspace present. A `[workspace]` table referencing a missing `xtask` member fails `cargo build` for plugin users, so the mirrored `Cargo.toml` is the tracer package manifest with its `[workspace]` table stripped, and the mirrored `Cargo.lock` is resolved from that stripped manifest — not the workspace lock, which carries `xtask` and its dependencies. The `src/` tree is copied verbatim.

When the tracer source changes, rebuild the host-platform prebuilt from that source and re-run `cargo xtask sync-dist` (setup.sh does this automatically). Each prebuilt must come from a host of its own architecture — there is no cross-compile path — so the off-host prebuilt is refreshed from a machine of that architecture; the mirror's drift guard (`cargo xtask sync-dist --check`) is the proof that the committed payload reflects the current source. No GitHub Releases pipeline, no vendored crates — the build-from-source path is a plain network `cargo build` reaching crates.io.

The drift guard doubles as the post-source-edit release gate: an uncommitted source change leaves the mirror dirty, and `--check` exits non-zero until the producer is re-run. A clean `--check` is the contract that the committed crate/ matches the committed source — the host prebuilt is checked separately by running the v4.9 contract probes through the launcher on real repos.

### Adding a new command

1. Create `src/commands/<name>.rs` exposing a `run(...)` entry point that returns `Result<serde_json::Value>` — build the value, render human text only when `--json` is off, and return the value (never print the JSON or handle `--filter` in the command)
2. Verify required external binaries at the top of `run` unless the command is itself a dependency check
3. Decide which cache namespace it reads (`file/` for per-file data, `architecture/` for cross-file relationships) — never read both directly; if the command joins them, do it in the command's render code, never in the cache layer
4. Add a `pub mod <name>;` line to `src/commands/mod.rs` and a clap variant in `src/main.rs`; wire the match arm through `output::run_value(json, filter, || commands::<name>::run(...))` so it gets `--json`/`--filter` for free
5. Document the command in `Readme.md`, in this file's command list, and in the `trace` skill's command table — `--filter` is global, so it needs no per-command mention

### Adding a new language extractor

1. Add `src/extraction/<lang>.rs` exposing the extract entry point returning an `ExtractionResult`
2. Wire its extension(s) into `extraction/mod.rs`'s dispatch
3. Add the per-language tree-sitter crate to `Cargo.toml` (and, for CCN, a decision-node set in `ccn.rs`)
4. Bump `cache::SCHEMA_VERSION` if the `FileFacts` shape changes; otherwise existing cache entries for the new extension stay missing until invalidated

### Cache invalidation

- Per-file (`file/`): file SHA changes → cache key changes → next read re-extracts
- Architecture (`architecture/`): any per-file SHA change → fingerprint changes → next read rebuilds the graph from current per-file facts (cheap because per-file is cached)
- Schema changes: bump `cache::SCHEMA_VERSION`. Every cache key shape that namespaces a per-file hash — the on-disk entry key AND the mtime fast-path index key — must include the schema version, so an upgrade rotates them all together. A bumped key on disk + an unrotated mtime index returns stale per-file hashes, which keep the architecture fingerprint stable and serve a stale graph on the first post-upgrade query

## Ledger

- v4.10: Drift guard is the proof the payload tracks source
- v4.9: Real-world probes pinned the contract end-to-end
- v4.8: Callers returns use sites via reference edges
- v4.7: Crate owns its payload producer as a cargo xtask
- v4.6: Plugin crate mirror generated and drift-guarded
- v4.5: Search output deterministic and ranking correct
- v4.4: Dependent traversal reaches a module's owned symbols
- v4.3: CCN counts only named branch and short-circuit nodes
- v4.2: In-binary filter replaces piping trace to jq
- v4.1: Project-docs opt-in, not auto-injected
- v4.0: Plugin ships a self-resolving binary launcher
- v3.6: AST CCN backend opt-in via env var
- v3.5: Glob command for full-path match with `**`
- v3.4: Loop hot-paths banned because they hid 50x slowdowns
- v3.3: Read scopes by ref and line range, one-call lookups
- v3.1: Passive lifecycle context on reads and listings
- v3.0: Directional upstream/downstream for graph queries
- v2.0: Split cache to isolate per-file from architecture
- v1.0: Baseline for code-intelligence CLI
