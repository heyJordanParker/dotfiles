# tracer

Code intelligence CLI for mapping architectural relationships in any local codebase. Returns rich, structured output: matches plus per-file complexity, callers, dependents, nearest docs, and git context on every query.

Built for use by AI coding agents (via the `trace` Claude Code skill), but works as a standalone CLI for any developer who wants ast-grep, complexity metrics, git context, and a tree-sitter-built architecture graph unified behind one interface.

A single static Rust binary — no runtime, no interpreter.

## Install

```bash
cargo build --release
install -m 755 target/release/trace ~/.local/bin/trace
trace doctor
```

`trace doctor` validates that required external binaries are installed and prints per-platform install commands if any are missing. Every other `trace` command hard-fails with the same diagnostics if dependencies are missing.

Inside the dotfiles repo, `setup.sh` does the build + install automatically. When shipped with the Claude Code plugin, a POSIX launcher at `packages/claude/bin/trace` resolves a host-appropriate binary (committed prebuilt, locally-cached build, or `cargo build` from the shipped crate source), so plugin users get `trace` on PATH with no manual step.

## Required external binaries

The CLI orchestrates these tools — install them via your platform's package manager:

| Binary | Purpose | macOS | Debian/Ubuntu | Windows |
|---|---|---|---|---|
| `ast-grep` | Structural code search | `brew install ast-grep` | see project page | `scoop install ast-grep` |
| `scc` | Repo-wide complexity sweep | `brew install scc` | see project page | `scoop install scc` |
| `universal-ctags` | Symbol fallback | `brew install universal-ctags` | `apt install universal-ctags` | see project page |
| `ripgrep` (`rg`) | Text grep | `brew install ripgrep` | `apt install ripgrep` | `scoop install ripgrep` |
| `git` | History/blame context | `xcode-select --install` | `apt install git` | git-scm.com |

Tree-sitter grammars (Python, TypeScript/TSX/JSX, PHP, plus bash, lua, go, rust, ruby, java, c) are compiled into the binary — no extra install.

## Commands

**Per-file** (cached under `.tracer-cache/file/`):
```
trace doctor                       Verify dependencies; print install instructions for any missing
trace read <file> [--method <n>]   Cleaned read; method by name, line range, anchor, or whole file; worktree or git ref. Each file is capped at a size budget, cut at a whole line, and ends with a `[trimmed at L<n> of <total> — continue: …]` marker naming the next window; --all lifts the cap; --docs opts in to project-docs injection (off by default)
trace docs <path> [--directory] [--source <s>] [--triggering-tool <t>] [--triggering-command <c>]   Deduped project-docs set (Claude.md / rules ancestors) for a path. Returns { docs, doc_count, already_loaded? } — `docs[]` is the freshly surfaced slice (with content); `already_loaded[]` is the dedupe-skipped slice (with per-entry source) and is omitted when empty
trace docs load <path> [--source <s>] [--triggering-tool <t>] [--triggering-command <c>]   Hook-facing alias forwarding to path-mode with --source default flipped to `trace_docs_load`. Same shape, same behavior
trace docs <path> --graph          Whole-repo docs graph (every CLAUDE.md / Claude.md / .claude/rules/*.md with @include edges + conditional-path frontmatter), plus the "available but not loaded" set. `<path>` is optional under `--graph`.
trace docs reset [--source <s>]    Clear the current session's surfaced-docs state so a subsequent `trace docs <path>` re-surfaces docs as new (drives the Codex compaction/clear hook). Preserves append-only history; clears only the materialized view. No-op when no session is active
trace list <dir>                   One-level annotated ls: files + sub-directories with complexity and recency
trace survey <path>                Repo-wide language + LOC + complexity distribution
trace tree <path>                  Annotated file tree with complexity ranks (recursive)
trace info <path>                  Complexity structure + architectural overview of a file or directory
trace structure <file>             Methods, properties, variables, imports, exports
trace grep <pattern>               Text search with per-match enrichment
trace logs [<pattern>] [--path <p>] [--file <glob>] [--since <when>] [--until <when>] [--around N] [--limit N]   Timestamped entries from log files, ignore rules never consulted. One line is one entry; an untimestamped line attaches to the entry above it, so a stack trace comes back whole. Reads `.gz` rotations, spans dated filenames across a window, and streams, so a rotated 80 MB directory costs the window and not the files
trace struct <pattern> -l <lang>   Structural AST search via ast-grep with per-match enrichment
trace glob <pattern> [<base>]      Full-path pattern search (** recursive, gitignore-respecting); bare paths, --details adds ccn + rank + lifecycle
trace find <pattern> [<base>]      Filename-pattern search with complexity rank + lifecycle shoulder
trace history <file> | --contains <p>   Whole-file log, function-line history, or pickaxe
trace blame <file> [<symbol>]      Symbol-aware blame; collapsed regions with commit subjects
trace diff [--base <ref>] [--symbols]    Files or module-level symbols changed vs a base ref, load-bearing first
trace status [--state <s>]         Working-tree dirty set ordered by blast radius
trace context [<path>] [--offset N] [--limit N] [--no-record]   Session-start primer (no args) or single-file enrichment (path arg); --offset/--limit record which line range was read, accumulating per-file read coverage; --no-record renders the shoulder without recording a read (the enrich hook sets it for Edit/Write — an edit is not a read)
```

`read`, `list`, `tree`, and `info` annotate each file with a one-line passive-context shoulder showing lifecycle state (new / renamed / modified / settled), age, and complexity rank — letting an AI agent calibrate its conclusions about how settled a file is before drawing them.

**Architecture** (cached under `.tracer-cache/architecture/`):
```
trace defines <symbol>                  Where a symbol is defined
trace callers <symbol>                  Direct callers via the architecture graph
trace downstream <symbol> [--depth N]   Transitive dependents (BFS over reverse edges)
trace downstream --path <p> [--limit N] Top-N most-depended-on symbols in a path
trace upstream <symbol> [--depth N]     Transitive dependencies (BFS over forward edges)
trace upstream --path <p> [--limit N]   Top-N highest-coupling symbols in a path
trace symbols <file>                    Module-level symbols of a file
```

**Cache management**:
```
trace cache build [<path>]         Prebuild per-file facts + architecture graph
trace cache stats                  Show entries and size per namespace
trace cache clear [--namespace file|architecture] [--all]   Invalidate cache entries
```

All commands accept `--json` for machine-parseable output. For partial
output, add `--filter '<jq expression>'` (requires `--json`): an in-process
jq (`jaq`) runs the program over the value and prints each result. This
replaces piping `trace … --json | jq` — the binary stays self-contained and
no `jq` is shelled out.

## Disk cache

`.tracer-cache/` at the repo root, two namespaces that never cross-read:

- `file/{hash}.json` — per-file facts (complexity, imports list, exports list, git activity). One entry per file, keyed by SHA-256 of file contents + path + cache schema version.
- `architecture/{hash}.json` — the cross-file architecture graph (symbols, modules, import edges with confidence labels). One entry per repo state, keyed by the fingerprint of all current per-file cache hashes. Rebuilds automatically when any file changes.

Add `.tracer-cache/` to your project's `.gitignore`. Use `trace cache clear` to invalidate manually.

## Cyclomatic complexity

Per-file and per-function cyclomatic complexity is computed by an in-process tree-sitter AST decision-node walker covering ten languages (Python, TypeScript/TSX/JSX, PHP, bash, lua, go, rust, ruby, java, c). It walks each function's body counting decision-point nodes (`if`, `for`, `while`, each `case`, each `catch`, `&&` / `||` / `??`, ternaries, comprehension clauses, language-appropriate equivalents) under the McCabe convention. This is the single backend; `TRACER_CCN_BACKEND` is recognized as an external protocol name and resolves to `ast`.

## Status

All 25 commands implemented. Architecture extraction supports Python, TypeScript / TSX / JSX, and PHP — extensions without an extractor still get per-file facts (complexity, git activity) but no architecture-graph entry.

## License

MIT
