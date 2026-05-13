# tracer

Code intelligence CLI for mapping architectural relationships in any local codebase. Returns rich, structured output: matches plus per-file complexity, callers, dependents, nearest docs, and git context on every query.

Built for use by AI coding agents (via the `trace` Claude Code skill), but works as a standalone CLI for any developer who wants ast-grep, complexity metrics, git context, and a tree-sitter-built architecture graph unified behind one interface.

## Install

```bash
pipx install tracer
trace doctor
```

`trace doctor` validates that required external binaries are installed and prints per-platform install commands if any are missing. Every other `trace` command hard-fails with the same diagnostics if dependencies are missing.

## Required external binaries

The CLI orchestrates these tools — install them via your platform's package manager:

| Binary | Purpose | macOS | Debian/Ubuntu | Windows |
|---|---|---|---|---|
| `ast-grep` | Structural code search | `brew install ast-grep` | see project page | `scoop install ast-grep` |
| `scc` | Repo-wide complexity sweep | `brew install scc` | see project page | `scoop install scc` |
| `universal-ctags` | Symbol fallback | `brew install universal-ctags` | `apt install universal-ctags` | see project page |
| `ripgrep` (`rg`) | Text grep | `brew install ripgrep` | `apt install ripgrep` | `scoop install ripgrep` |
| `git` | History/blame context | `xcode-select --install` | `apt install git` | git-scm.com |

Tree-sitter grammars (Python, TypeScript, PHP) are pulled in as Python deps — no extra install.

LSP servers (used by the legacy multilspy path) are auto-downloaded by `multilspy` on first use per language — no manual install.

## Commands

**Per-file** (cached under `.tracer-cache/file/`):
```
trace doctor                       Verify dependencies; print install instructions for any missing
trace read <file> [<method>]       Cleaned read; method by name or full file; preserves context, cuts fluff
trace list <dir>                   One-level annotated ls: files + sub-directories with complexity and recency
trace survey <path>                Repo-wide language + LOC + complexity distribution
trace tree <path>                  Annotated file tree with complexity ranks (recursive)
trace info <path>                  Complexity structure + architectural overview of a file or directory
trace structure <file>             Methods, properties, variables, imports, exports
trace grep <pattern>               Text search with per-match enrichment
trace struct <pattern> -l <lang>   Structural AST search via ast-grep with per-match enrichment
trace history <file>               Git log/blame summary for a file
trace blame <file> [<symbol>]      Symbol-aware blame; collapsed regions with commit subjects
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
trace cache stats                  Show entries and size per namespace
trace cache clear [--namespace file|architecture] [--all]   Invalidate cache entries
```

All commands accept `--json` for machine-parseable output.

## Disk cache

`.tracer-cache/` at the repo root, two namespaces that never cross-read:

- `file/{hash}.json` — per-file facts (complexity, imports list, exports list, git activity). One entry per file, keyed by SHA-256 of file contents + path + cache schema version.
- `architecture/{hash}.json` — the cross-file architecture graph (symbols, modules, import edges with confidence labels). One entry per repo state, keyed by the fingerprint of all current per-file cache hashes. Rebuilds automatically when any file changes.

Add `.tracer-cache/` to your project's `.gitignore`. Use `trace cache clear` to invalidate manually.

## Status

All 16 commands implemented. Architecture extraction supports Python, TypeScript / TSX / JSX, and PHP — extensions without an extractor still get per-file facts (complexity, git activity) but no architecture-graph entry.

## License

MIT
