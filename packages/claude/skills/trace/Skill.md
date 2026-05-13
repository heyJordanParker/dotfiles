---
name: trace
description: Code intelligence for the local codebase — search, callers, definitions, symbols, complexity, file/method reads with rich architectural context. Use whenever you need to find, understand, or trace relationships in code instead of reaching for raw grep or unfiltered file reads.
---

# trace

The `trace` CLI returns rich code intelligence — matches plus per-file complexity, callers, callees, nearest doc, git activity, and a repo-wide complexity baseline on every query.

## Install

**Plugin users**: the `trace` binary auto-lands on your `PATH` when this plugin is enabled. Run `trace doctor` once to verify required external binaries (ast-grep, scc, universal-ctags, ripgrep, git) are installed.

**Standalone**: `pipx install tracer && trace doctor`.

If a `trace` command errors with "missing dependencies", run `trace doctor` for per-platform install instructions.

## Cache behavior (read first when entering a new repo)

`trace` keeps a two-layer disk cache at `.tracer-cache/` in the repo root. The first architecture command in a fresh repo builds the cache (typically 5–30s for ~1000 files, respecting `.gitignore`); every subsequent command returns in well under a second.

Prebuild explicitly before heavy use:

```
trace cache build [<path>]
```

Idempotent. Cache invalidates per file when content changes.

## Core workflow

```
trace doctor                            # verify dependencies (run first if anything errors)
trace cache build [<path>]              # prebuild architecture + per-file caches
trace list <dir>                        # one-shot orientation: children of a dir with file count, ccn, recency
trace downstream <symbol> [--depth N]   # what depends on a symbol (transitive)
trace downstream --path <path>          # top-N most-depended-on symbols in a path (centrality)
trace upstream <symbol> [--depth N]     # what a symbol depends on (transitive)
trace upstream --path <path>            # top-N highest-coupling symbols in a path
trace callers <symbol>                  # direct callers of a symbol (one hop)
trace defines <symbol>                  # where a symbol is defined
trace symbols <file>                    # module-level symbols of a file
trace survey <path>                     # repo-wide complexity distribution
trace tree <path>                       # annotated file tree with complexity ranks (recursive)
trace info <path>                       # complexity structure + architectural overview
trace structure <path>                  # methods, properties, variables, imports, exports
trace grep <pattern> [-l <lang>]        # text search with rich context
trace struct <pattern> -l <lang>        # structural (AST) search via ast-grep
trace read <file> [<method>]            # cleaned read; method or full file; preserves comments, cuts fluff
trace history <file>                    # git log/blame summary
trace blame <file> [<symbol>]           # symbol-aware blame; regions collapse runs of one commit, include the subject
trace blame <file> --lines L1:L2        # blame an explicit line range
```

## Passive context on every read — hypothesis, not conclusion

`trace read`, `trace info`, `trace tree`, and `trace list` attach a one-line shoulder per file:

```
[git: <state> · age: <age> · ccn: <total> <rank>]
```

State labels: `new (N commits)`, `renamed-from <old path>`, `untracked`, `added (uncommitted)`, `modified (uncommitted)`, `N commits` (settled history). **Treat each as a hypothesis to validate, not a conclusion to act on.**

What each label suggests:

- New / untracked / added → likely no callers yet. Probably don't need backward-compatibility, deprecation paths, or migration wrappers.
- Renamed-from → likely continuation, not new code. Carry prior knowledge forward.
- Recent modification with low commit count → likely active development; current shape may not be final.
- Many commits + old last_modified → likely settled code; existing patterns are load-bearing.

For decision-shaped questions ("should I modify or stack?", "should I add this here or elsewhere?"), cross-check the lifecycle signal against project rules before recommending. Two cheap verification moves:

1. Read the nearest `Claude.md` (e.g. `database/migrations/Claude.md`) — projects encode their own rules about what can be edited.
2. `git show origin/production:<path>` — confirms whether a file the local view thinks is "new" is in fact already deployed.

Worktrees, squashed-baseline commits, and branch divergence can all make a file look "fresh" locally while being settled in production. Only recommend modify when lifecycle signal AND project rule AND production check all agree.

## Execution rules

- Always pass `--json` when piping output to subsequent reasoning.
- Run `trace doctor` first if any command errors with a missing-dependency error.
- Use `trace info` and `trace structure` for architectural orientation **before** deep reads.
- Use `trace read` instead of raw Read whenever you want fluff-stripped output.
- Calibrate read depth using `repo_context.complexity_p95` — files exceeding p95 get full reads; uniformly low complexity gets skims.

## When to use which command

| You want to… | Use |
|---|---|
| Find the most architecturally central files in a repo | `trace downstream --path <path>` — top-N ranked list |
| Find the highest-coupling files (most outgoing deps) | `trace upstream --path <path>` |
| Find what depends on X transitively (blast radius) | `trace downstream X --depth N` |
| Find what X depends on transitively | `trace upstream X --depth N` |
| Find every place X is used directly | `trace callers X` |
| Find where X is defined | `trace defines X` |
| Get oriented in an unfamiliar repo | `trace survey` then `trace list` |
| List the contents of one directory (1 level) | `trace list <dir>` |
| Walk a directory recursively with complexity | `trace tree <dir>` |
| Understand what a file/dir does at a glance | `trace info` |
| List a file's methods/props/vars and their connections | `trace structure` |
| See a file's outline | `trace symbols <file>` |
| Find a string anywhere | `trace grep <pattern>` |
| Find a structural pattern (e.g. all controllers calling a service) | `trace struct '<pattern>' -l <lang>` |
| Read one method without the rest of the file | `trace read <file> <method>` |
| Read a whole file with token-wasting fluff cut | `trace read <file>` |
| Understand history/why of a file | `trace history <file>` |
| Find who last touched a function or line range | `trace blame <file> <symbol>` or `trace blame <file> --lines L1:L2` |

## Directional intuition

`upstream` and `downstream` mirror the import direction: if A imports B, B is **upstream** of A (A depends on B); A is **downstream** of B (A is depended on by B isn't quite right — A is the dependent). The convention:

- `trace upstream X` — what's upstream of X (what X depends on)
- `trace downstream X` — what's downstream of X (what depends on X)

In `--path` mode, the ranking inverts the natural-language framing:
- `trace downstream --path P` — files in P that have the most downstream dependents (i.e. the most-depended-on / most central / most load-bearing)
- `trace upstream --path P` — files in P that have the most upstream dependencies (i.e. highest-coupling / most fan-out)

## References

Detailed patterns and per-language behavior live in `references/` (added as the CLI matures).
