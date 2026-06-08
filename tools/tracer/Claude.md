# Tracer

## Why

Code-intelligence CLI for AI coding agents working in-repo. Each command returns dense, structured information about a file, symbol, or directory in a single response: complexity profile, git lifecycle (commits, age, rename history, deploy-branch presence), architectural-graph edges (callers, immediate dependencies), nearest project rules from `Claude.md` ancestors, leading docblock, and language-aware structure (methods, classes, imports). A single `trace info <path>` contains what an asking agent would otherwise reconstruct from several grep / read / log invocations.

The chain that consumes tracer (skill + explorer agent) is a separate concern. Tracer's responsibility ends at the output: rich, accurate, attribute-only. What anyone does with that output is theirs.

Standalone developers who want the same unified interface are a secondary audience — the CLI works without Claude Code.

## What

A single static Rust binary (`trace`) exposing 23 commands that orchestrate external code-intelligence binaries plus an in-process three-namespace disk cache: per-file facts, a unified architecture graph (symbols/modules with cross-file edges PLUS doc-file nodes for every recognized project-rules markdown file — Claude Code's `CLAUDE.md`/`Claude.md` and OpenAI's cross-harness `AGENTS.md`/`Agents.md` (plus their `.local.md` peers), and every `.claude/rules/*.md` — with their `@include` edges and conditional-path frontmatter), and a per-(session, agent) log of session-context events. Reads and directory listings include passive context: lifecycle state (new / renamed / modified / settled), age, and complexity rank inline. Per-function cyclomatic complexity is computed by an in-process tree-sitter AST decision-node walker covering ten languages — the single CCN backend, no external complexity tool.

### Commands

**Per-file commands** (read the `file/` cache):
- `doctor` — verify required external binaries; print per-platform install instructions
- `read <paths...> [--method <name>] [--at <ref>] [--lines L1:L2] [--between S E] [--diff] [--raw] [--docs]` — cleaned read; one or more paths; whole file, method by name, line range, or anchor section; worktree or git ref; optional symbol-level diff vs worktree when reading at a ref; `--raw` skips cleaning. Project-docs are not injected by default; `--docs` opts in to inline project-docs content
- `list <dir> [--all]` — one-level annotated ls; files + sub-directories with file count, ccn, recency per entry; `--all` includes hidden entries
- `survey <path>` — repo-wide language + LOC + complexity distribution via scc
- `tree <path> [--depth N]` — annotated file tree with complexity ranks (recursive; default depth 4)
- `info <path> [--brief]` — complexity structure + architectural overview of a file or directory; `--brief` trims to the headline facts
- `structure <file>` — methods, properties, variables, imports, exports for one file, each carrying full signature surface (visibility, return type, parameters with types and defaults, attributes/decorators, class extends/implements, generic/type parameters, PHP 8.4 property hooks)
- `grep <pattern> [-l <lang>] [--path <path>]` — text search via ripgrep with per-match enrichment
- `struct <pattern> -l <lang> [--path <path>]` — structural AST search via ast-grep with per-match enrichment
- `glob <pattern> [<base>]` — full-path pattern search (Claude Glob shape: `**` recursive, gitignore-respecting). Returns the complete deterministically-sorted match list; bare paths by default, `--details` adds per-line ccn + rank + lifecycle shoulder
- `find <pattern> [<base>] [--path <p>] [--exclude <p>]... [--type f|d] [--limit N] [--sort complexity|recent|path]` — filename-pattern search with complexity rank + lifecycle shoulder per match; defaults to files, path-sorted, limit 200
- `history <file> | <file> <symbol> | --contains <pattern>` — whole-file log, function-line history (git log -L), or pickaxe (git log -S)
- `blame <file> [<symbol>] [--lines L1:L2]` — symbol-aware blame collapsed into per-region commit summaries
- `diff [--base <ref>] [--symbols]` — files (or module-level symbols) changed vs a base ref, load-bearing first
- `status [--state <s>]` — working-tree dirty set ordered by blast radius
- `context [<path>] [--directory]` — session-start primer (no args) or single-file enrichment (path arg)
- `docs <path> [--directory] [--source <s>] [--triggering-tool <t>] [--triggering-command <c>]` — the deduped project-docs set (`CLAUDE.md`/`Claude.md`/`AGENTS.md`/`Agents.md` and their `.local.md` peers, plus `.claude/rules/*.md` ancestors) for a path. Walks the full ancestor chain, partitions against the per-session log, records only the newly-surfaced slice under `--source` (default `trace_docs`), and returns `{ path, directory_scoped, docs[], doc_count, already_loaded[]? }`. `docs[]` carries the freshly surfaced docs (with content); `already_loaded[]` carries the skipped slice (without content, with per-entry `source`) and is omitted entirely when empty. Shares `read`'s per-session "read once" dedupe
- `docs load <path> [--source <s>] [--triggering-tool <t>] [--triggering-command <c>]` — hook-facing alias forwarding to path-mode with the `--source` default flipped to `trace_docs_load`. Same `{ docs, doc_count, already_loaded? }` shape, same behavior. `inject-docs.sh` invokes path-mode directly
- `docs <path> --graph` — the whole-repo docs graph projected from the unified `architecture/` cache entry: every recognized rules-markdown file (`CLAUDE.md`/`Claude.md`/`AGENTS.md`/`Agents.md` plus their `.local.md` peers, and every `.claude/rules/*.md`) with their `@include` edges and conditional-path frontmatter, plus the "available but not loaded" set computed against the session log when a session is active. `<path>` is optional under `--graph` and defaults to the cwd's repo root.
- `docs status [<path>]` — agent-facing "what do I have right now?" query against the session log. No path → the full session manifest with per-entry source attribution and a `by_source` count breakdown. With a path → the ancestor chain partitioned into `loaded` (with source) and `not_loaded` so the agent can immediately tell whether a path's rules are in context. Pure read; never records.
- `docs reset [--source <s>]` — clear the current session's surfaced-docs state so a subsequent `trace docs <path>` re-surfaces every doc as new. Drives the Codex compaction/clear lifecycle hook: a context reset drops injected rule text from the model, so the surfaced-docs state must reset to re-inject it. Appends one `context_reset` event recording the cleared set and clears `view.json`'s `emitted` map — append-only history is preserved, mirroring the drift reconciler. Returns `{ scope, session_active, source, cleared_count }`. A clean no-op (returns `cleared_count` 0) when the session id is absent or nothing was surfaced.

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

### Three-namespace cache (the architectural decision)

Tracer state lives in three namespaces that **never read each other**:

- **`file/`** (in `.tracer-cache/` at the repo root) — per-file facts: complexity, loc, language, raw imports list, raw exports list, git activity. One entry per file. Cache key = `sha256("v{SCHEMA_VERSION}|ccn:{backend}\0" + file_contents + "\0" + relative_path)`. Used by per-file commands and as enrichment in `grep`/`struct`. Flat records. No edges. No graph membership.
- **`architecture/`** (in `.tracer-cache/` at the repo root) — the unified architecture graph. Code nodes are **symbols and modules — code files are never graph nodes; their paths live as a `source_file` attribute on those nodes**. Doc files (CLAUDE.md / Claude.md / AGENTS.md / Agents.md plus their `.local.md` peers, and every `.claude/rules/*.md`) are the explicit exception: they DO become graph nodes (`kind: claude_md | local_md | agents_md | agents_local_md | rules_unconditional | rules_conditional | include`) carrying their `@include` edges and conditional-path `paths:` frontmatter. AGENTS.md is the cross-harness project-rules convention promoted by OpenAI for Codex CLI and adopted by Cursor, Aider, Jules, Amp et al.; it is recognized alongside Claude Code's CLAUDE.md so multi-harness repos work without dropping context. Code-side edges keep the `imports` / `references` taxonomy with `confidence` (`EXTRACTED | INFERRED | AMBIGUOUS`); doc-side edges use the `includes` relation. `references` edges resolve **structurally**, never by bare name: each use site carries its call shape (`free` / `member` / `static`) and the class named at the site, each declaration carries its containing class, and both sides carry their language. A use site resolves to a declaration only when they agree — same language; a free call to a non-method symbol (function or constructor-call class), a static / `new` / `::class` / type-hint use to the named class (and its exact member), a member call to methods of that name. The ONLY edge that stays `AMBIGUOUS` is a same-language member call whose receiver type the site does not name, narrowed to methods of that name — never a cross-language match, never a wrong-shape symbol, never a fan-out across every same-named declaration. Stored as a single bincode-encoded `.bin` entry decoded straight into the in-memory `Graph` struct on load — no JSON parse, no intermediate `Value` tree, so process start does not pay a full graph parse. Exactly one entry per repo state, keyed by `sha256("v{SCHEMA_VERSION}|architecture\0" + sorted_per_file_hashes + "\0docs\0" + git_head + "\0" + doc_mtime_aggregate)`; a write evicts every superseded fingerprint's `.bin` sibling so the namespace never grows without bound across HEAD moves or doc changes — a change to either input invalidates the unified entry. The in-memory graph carries a reverse-edge adjacency index built once after load/build, so `dependents_of` / `references_to` / the transitive-dependent walk index straight to the relevant edges instead of rescanning the whole edge list per call; the index is derived from the edges, never serialized to disk. The read-only load path (`load_cached`) validates the sole entry against the current code side + git HEAD without re-walking the doc tree — it reuses the docs mtime aggregate stored on the entry rather than re-stat'ing every doc file. Rebuilt by re-running cross-file resolution and the docs-graph walk whenever per-file caches or doc-file mtimes change (cheap because per-file is cached). The docs-side walker, `@include` resolver, and frontmatter parser are reused from `commands::nested_memory` — there is no second implementation of any.
- **`sessions/<session_id>/<agent_id>/`** (in `.tracer-cache/` at the repo root, alongside the other two namespaces) — the session-context log. `events.jsonl` is the append-only event log (one JSON object per line); `view.json` is the materialized projection (`emitted: canonical path → sha256:<hex>`); `.lock` carries the flock held across read + append + materialize so concurrent same-session invocations are serialized. `session_id` is resolved from `CLAUDE_CODE_SESSION_ID` / `CLAUDE_SESSION_ID` / `TRACER_SESSION_ID` via `nested_memory::session_id()`; `agent_id` is resolved from `TRACER_AGENT_ID` and defaults to the literal string `root` when absent. The log no-ops entirely when the session id is absent OR when no repo root is resolvable from cwd, keeping standalone tracer use valid in both cases. Session context that spans repos is not preserved across repos — accepted trade-off for one storage location and one cleanup story across all three namespaces. This is the single owner of session-context state — no other module persists session dedupe or related data. Stopped subagents are archived to `sessions/<session_id>/archived/<agent_id>/` by the `archive-subagent-log.sh` harness hook; the log's read path falls back to the archived directory when the active one is absent, so queries against a stopped subagent's log keep returning the same data. Writes always target the active directory — never the archived one. A `ContextPrimeDrift` event records divergence between the deterministic context primer's predicted set and the observed set Claude Code actually injected, supplied to `trace context prime --observed-from <path>` (`-` for stdin); on drift the view's `emitted` map is reconciled to observed paths and their hook-supplied content hashes while the append-only `events.jsonl` preserves the diff payload.

Per-file commands read only `file/`. Architecture commands read only `architecture/`. The docs `--graph` view projects out of the same `architecture/` entry — no second namespace. Session-context commands read only `sessions/`. Joins between any of the three layers happen at the rendering layer (in command output) at query time only — never persisted, never cached together.

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
- Per-file commands read only the `file/` namespace. Architecture commands read only the `architecture/` namespace (which holds both the symbol/module graph and the doc-file nodes — `trace docs --graph` projects out of the same entry, never a separate one). Session-context commands read only the `sessions/` namespace. The three layers join only in rendering code, never in storage
- `session_log` is the single owner of session-context state — no module persists session dedupe, doc-injection events, or related data outside it. The log no-ops when the session id is absent so standalone tracer use stays valid
- The log has a single agent-facing query surface: `trace docs status`. Other commands consume it (the docs-awareness hint on every `trace context <file>`) but no command exposes a parallel "what's loaded?" query — agents converge on one verb
- `trace docs status` is a pure read — it never records, never mutates the log; only `record_emission` / `record_read` / `record_harness_drift` / `record_context_reset` write
- `doctor` must be cross-platform — print install commands for macOS, Debian/Ubuntu, and Windows. Never assume `brew`
- The architecture graph models **symbols and modules** for code files — code files are never graph nodes; their paths live as a `source_file` attribute. Doc files (CLAUDE.md / Claude.md / AGENTS.md / Agents.md plus their `.local.md` peers, and every `.claude/rules/*.md`) are the explicit exception and DO become graph nodes carrying their `@include` edges and conditional `paths:` frontmatter
- `--json` and on-disk cache entries use one stable byte format (ASCII-escaped non-ASCII scalars, fixed separators) produced by `jsonfmt` — every serializing path goes through it

### Boundaries

- Never bundle the external binaries — users install via their platform's package manager. `doctor` only verifies and instructs
- Never depend on Claude Code, its session state, or its tools at runtime — the CLI is invoked from a subprocess and must stand alone
- Never write commands that skip the dependency check — silent fallback to a missing binary produces wrong output, not a clear error
- Never put code files as nodes in the architecture graph — code-file paths are an attribute (`source_file`) on symbol/module nodes, never the node itself. The single carve-out is doc files (CLAUDE.md / Claude.md / AGENTS.md / Agents.md plus their `.local.md` peers, and every `.claude/rules/*.md`), which ARE first-class graph nodes carrying their `@include` edges and conditional `paths:` frontmatter; this is the layer-separation invariant under the unified graph
- Never cross-read between the three cache namespaces — per-file commands stay in `file/`, architecture commands stay in `architecture/` (which also serves the docs `--graph` projection), session-context commands stay in `sessions/`. Joins happen in command code at render time, are disposable, and never persist
- Never write session-context state outside `session_log`. The flat path-list dedupe under the historical `~/.tracer-cache/sessions/<sid>/loaded-memories.txt` was retired in v5.0 — any new session-scoped surface (read tracking, drift detection) lands as additional event kinds in the log, never as a parallel file. The docs graph is repo-state, not session-state, and lives inside the unified `architecture/` cache entry alongside the symbol/module graph — never inside the session log. The session log itself now lives at `<repo>/.tracer-cache/sessions/` alongside the other namespaces — see v5.7
- Never store cache entries with stale schemas — bump `cache::SCHEMA_VERSION` whenever extraction or `FileFacts` shape changes; old entries become unreachable automatically across all namespaces
- Never rename the `.tracer-cache/` directory — it is an on-disk contract shared with every repo's `.gitignore` and with stale caches in the wild
- Never rename the `TRACE_BIN` or `TRACER_CCN_BACKEND` environment variable names — they are an external protocol shared with the test suite, hooks, and docs
- Never resolve `repo_root` per file inside a repo-wide loop — resolve it once outside the loop and thread it through; a per-iteration `git rev-parse --show-toplevel` is 15-30ms each and dominates large-repo runs
- Never walk the whole tree before filtering for repo-wide enumeration — use `repo_files::tracked_files` (git ls-files) or `walk_files` with dir-level `SKIP_DIRS` pruning, never an unbounded recursive walk that descends `node_modules` / `vendor` / worktrees
- Never reconstruct the bulk git-activity map inside a loop — build it once via `git_activity::bulk_cached` and index the returned map per iteration; rebuilding per-iteration makes the loop O(N²)
- Never shell out to the `jq` binary or bundle it — `--filter` is served in-process by the `jaq` crate so `trace` stays a single static binary with no new external dependency for `doctor` to verify
- Never let a value-producing command print its own JSON or decide `--filter` — it returns the value; `output.rs` owns emit. Never imply `--json` from `--filter`; the combination is validated up front in `output::guard`, before the command runs
- Never include the Claude memory system (`$HOME/.claude/projects/<slug>/memory/MEMORY.md`) in the context primer's auto-load set. That file is harness-internal state managed by Claude Code itself; tracer's job is to model repo docs. The primer's auto-load set is exactly the user-global CLAUDE.md plus the project-root CLAUDE.md chain (and their `@include` graphs) — no third leg

## Architecture

```
tracer/                                 cargo workspace root (tracer package + xtask member; tests excluded)
├── Cargo.toml                          workspace table + tracer manifest; clap + serde + bincode + rayon + tree-sitter (10 grammars) + jaq
├── Cargo.lock                          workspace lock (carries xtask; never mirrored)
├── .cargo/config.toml                  `cargo xtask` alias → run the xtask member
├── Readme.md
├── xtask/                              workspace automation crate (cargo xtask); owns the plugin-payload producer
│   └── src/main.rs                     `sync-dist` [`--check`]: regenerate / drift-guard the tracer-dist mirror
├── src/
│   ├── main.rs                         clap subcommand enum; dispatches all 24 commands
│   ├── cache.rs                        three-namespace disk cache (file/, architecture/, sessions/)
│   ├── file_facts.rs                   per-file extraction + file/ namespace owner
│   ├── architecture.rs                 cross-file resolution + architecture/ namespace owner (holds both symbol/module graph and doc-file nodes; one bincode `.bin` entry; reverse-edge index built on load; load-only path validates without re-walking docs)
│   ├── docs_graph.rs                   docs-graph builder; `build()` returns the in-memory graph plus the docs-side fingerprint inputs that `architecture::get` folds into the unified entry — no separate cache
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
│       ├── diff.rs      status.rs      context.rs
│       ├── docs.rs                     path-mode default ({ docs, doc_count, already_loaded? } shape), `--graph` flag for the docs-graph view, `load` sub-verb (thin alias forwarding to path-mode), `status` sub-verb (session manifest), `reset` sub-verb (clears the session's surfaced-docs view)
│       ├── defines.rs   callers.rs     symbols.rs    upstream.rs
│       ├── downstream.rs cache.rs      list_.rs
│       ├── context_prime.rs            backs `trace context prime`: walks the deterministic primer auto-load set, records new doc-injection events into the session log, and (when `--observed-from` is set) records the predicted-vs-observed diff via `drift.rs`
│       ├── drift.rs                    computes and reconciles the context primer's predicted set against the observed set Claude Code actually injected; appends `ContextPrimeDrift` events and reconciles the view's `emitted` map to observed paths + content hashes
│       ├── enrich.rs                   shared per-match enrichment for `grep` + `struct` (not a command)
│       ├── glob_match.rs               shared shell-glob basename matcher for `find` (not a command)
│       ├── paths_match.rs              shared `paths:` frontmatter glob matcher for conditional rules
│       ├── nested_memory.rs            shared project-docs walk-up; `session_id()` resolver (`read`, `docs`)
│       ├── signatures.rs               per-symbol signature extraction for `structure` (PHP/TS/Python); merged additively into ctags symbols and backfills ctags-gap entries (PHP class nodes, PHP 8.4 hooked properties)
│       └── session_log.rs              session-context log: events.jsonl + view.json under `<repo>/.tracer-cache/sessions/<sid>/<aid>/`; reads fall back to `sessions/<sid>/archived/<aid>/` when the active dir is missing (subagent-stop archive)
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
- Architecture (`architecture/`, unified): any per-file SHA change OR git HEAD move OR any doc-file mtime change → unified fingerprint changes → next read rebuilds the graph from current per-file facts plus a fresh docs-graph walk (cheap because per-file is cached). Editing a code file invalidates the symbol/module side; editing a `Claude.md`, `Agents.md`, or rules file invalidates via the docs-side input. The single bincode `.bin` entry is rebuilt and every superseded fingerprint's `.bin` is evicted on write, so the namespace holds exactly one entry — never two parallel entries, never an unbounded pile of stale fingerprints. The read-only `load_cached` path validates the sole entry against current code hashes + git HEAD reusing the entry's own stored docs mtime aggregate, so it never re-walks the doc tree; doc-node freshness is owned by `trace docs --graph` (which goes through the build path)
- Sessions (`<repo>/.tracer-cache/sessions/<sid>/<aid>/`): scoped by session and agent id, not by repo state — entries persist for the life of a session and are not invalidated by file edits. A new session id starts an empty log; the directory tree is removed by session-end cleanup at the harness layer, not by tracer. The log no-ops when the session id is absent OR when no repo root is resolvable from cwd, keeping standalone tracer use valid in both cases. The SessionStart harness hook (`reload-harness-context.sh`, wired on matcher `startup|resume|clear|compact`) re-runs `trace context prime --reason {post_compact|session_start}`, which appends new doc-injection events for any harness auto-loads Claude Code re-emitted after compaction or at session start (content-hash dedup means unchanged docs add no events). Every emission records the calling surface in the event's `source` field — `trace_docs` for path-mode, the `--source` flag value for the `docs load` hook entrypoint (defaults to `trace_docs_load`; the wired `inject-docs.sh` passes `trace_inject_hook`), `context_prime_session_start` / `context_prime_post_compact` for the context primer — so `docs load`'s `already_loaded` slice can attribute each skipped doc back to the surface that originally loaded it. Subagent stop archives the per-agent directory under `archived/<aid>/` via the `archive-subagent-log.sh` harness hook (which resolves the repo root from its inherited cwd via `git rev-parse`); read commands (`loaded_paths`, `events`, `view`) resolve to the archived path when the active one is absent, so post-stop queries return the same data. Writes always target the active directory and never the archived one
- Schema changes: bump `cache::SCHEMA_VERSION`. Every cache key shape that namespaces a per-file hash — the on-disk entry key AND the mtime fast-path index key — must include the schema version, so an upgrade rotates them all together. A bumped key on disk + an unrotated mtime index returns stale per-file hashes, which keep the unified architecture fingerprint stable and serve a stale graph on the first post-upgrade query. The unified architecture key embeds `SCHEMA_VERSION` in its `"v{SCHEMA}|architecture\0..."` prefix, so a bump rotates the unified entry along with the per-file entries. The session log's `view.json` is separately invalidated by `ContextPrimeDrift` reconciliation, never by a schema bump — it tracks what is currently in the agent's context, not what is on disk.
- Session-log queries (`loaded_paths`, `loaded_entries`, `events`, `view`, `session_active`) are not invalidated by schema bumps — they are a live projection of session state, not a cache entry; the context primer's auto-load set has no Claude-memory leg (see Boundaries). Doc-file mtime invalidation covers every recognized rules-markdown filename — both the Claude.md family and the Agents.md family — and every `.claude/rules/*.md`
