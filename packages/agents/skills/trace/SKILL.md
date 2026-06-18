---
name: trace
description: Code intelligence for the local codebase — search, callers, definitions, symbols, complexity, file/method reads with rich architectural context. Use whenever you need to find, understand, or trace relationships in code instead of reaching for raw grep or unfiltered file reads.
---

# trace

The `trace` CLI returns rich code intelligence — matches plus per-file complexity, callers, callees, nearest project docs, git activity, deploy-branch presence, and a repo-wide complexity baseline on every query.

## Install

**Plugin users**: the `trace` binary auto-lands on your `PATH` when this plugin is enabled. Run `trace doctor` once to verify required external binaries (ast-grep, scc, universal-ctags, ripgrep, git) are installed.

**Standalone**: build from `tools/tracer` with `cargo build --release`, put `target/release/trace` on `PATH`, then `trace doctor`.

If a `trace` command errors with "missing dependencies", run `trace doctor` for per-platform install instructions.

## Cache behavior (read first when entering a new repo)

`trace` keeps a three-namespace disk cache at `.tracer-cache/` in the repo root:

- **`file/`** — per-file facts (complexity, loc, language, imports, exports, git activity). Invalidated per file when content changes.
- **`architecture/`** — the unified cross-file graph: symbol/module nodes for code, doc-file nodes for `Claude.md` / `CLAUDE.md` / `Agents.md` / `AGENTS.md` (and their `.local.md` peers) plus every `.claude/rules/*.md`, with `@include` edges and conditional `paths:` frontmatter. One entry per repo state; invalidated when any per-file SHA changes, when git HEAD moves, or when any tracked doc-file mtime changes.
- **`sessions/<session_id>/<agent_id>/`** — the per-session, per-agent context log (`events.jsonl` + `view.json`). Tracks which project docs the agent already has in context so subsequent `trace docs` / `trace read --docs` / Read-enrichment calls don't re-emit them. No-ops without a session id.

The first architecture command in a fresh repo builds the graph (typically 5–30s for ~1000 files, respecting `.gitignore`); every subsequent command returns in well under a second.

Prebuild explicitly before heavy use:

```
trace cache build [<path>]
```

Idempotent. Other cache verbs: `trace cache stats` (entries + bytes per namespace) and `trace cache clear [--namespace file|architecture] [--all]`.

## Core workflow

```
trace doctor                            # verify dependencies (run first if anything errors)
trace cache build [<path>]              # prebuild architecture + per-file caches
trace cache stats                       # entries + bytes per namespace
trace cache clear [--namespace file|architecture] [--all]
trace context                           # repo primer (no args): environment, identity, tech stack, layout, common dirs, git, rules, spine
trace context <path>                    # single-file enrichment: one passive-context shoulder line
trace context prime --reason session_start|post_compact [--observed-from PATH|-]
                                        # hook-only: mirror harness auto-loads into the session log
trace list <dir> [--all]                # one-shot orientation: children with file count, ccn, recency
trace tree <path> [--depth N]           # annotated file tree with complexity ranks (default depth 4)
trace info <path> [--brief]             # complexity structure + architectural overview
trace structure <file>                  # methods, properties, variables, imports, exports
trace symbols <file>                    # module-level symbols of a file from the graph
trace defines <symbol>                  # where a symbol is defined
trace callers <symbol>                  # direct callers / use sites of a symbol
trace upstream <symbol> [--depth N]     # what a symbol depends on (transitive)
trace upstream --path <path> [--limit N]    # top-N highest-coupling symbols in a path
trace downstream <symbol> [--depth N]   # what depends on a symbol (transitive)
trace downstream --path <path> [--limit N]  # top-N most-depended-on symbols in a path
trace survey [<path>]                   # repo-wide language + LOC + complexity distribution
trace grep <pattern> [-l <lang>] [--path <path>]    # text search via ripgrep with per-match enrichment
trace struct <pattern> -l <lang> [--path <path>]    # structural (AST) search via ast-grep
trace find <pattern> [<base>] [--path <p>] [--exclude <p>]... [--type f|d] [--limit N] [--sort complexity|recent|path]
                                        # basename fnmatch (mirrors `find -name`); defaults files, path-sorted, limit 200
trace glob <pattern> [<base>] [--details]   # full-path shell-glob match; `**` recurses, gitignore-respecting; `--details` adds ccn + rank + lifecycle shoulder
trace read <paths...> [--method <name>] [--at <ref>] [--lines L1:L2] [--between START END] [--diff] [--raw] [--docs]
                                        # cleaned read; whole file, method by name, line range, or anchor section; worktree or git ref
trace docs <path> [--directory] [--source <s>] [--triggering-tool <t>] [--triggering-command <c>]
                                        # deduped project-docs set for a path; records new emissions in the session log
trace docs <path> --graph               # whole-repo docs graph: every recognized rules-markdown file with @include edges, plus the available-but-not-loaded set
trace docs load <path> [--source <s>] [--triggering-tool <t>] [--triggering-command <c>]
                                        # hook-facing alias forwarding to path-mode (--source defaults to trace_docs_load)
trace docs status [<path>]              # pure read; no path → session manifest, with path → ancestor chain partitioned loaded/not_loaded
trace docs reset [--source <s>]         # clear the session's surfaced-docs state so subsequent `trace docs` re-surfaces docs as new (post-compaction/clear); preserves append-only history
trace diff [--base <ref>] [--symbols]   # files (or module-level symbols) changed vs base ref; load-bearing first. Default base: origin/development
trace status [--state added|renamed|modified|deleted|untracked]
                                        # working-tree dirty set ordered by blast radius
trace history [<file>] [<symbol>] [--contains <pattern>]
                                        # whole-file log, function-line history (git log -L), or pickaxe (git log -S)
trace blame <file> [<symbol>] [--lines L1:L2]
                                        # symbol-aware blame; regions collapse runs of one commit
```

Every value-producing command accepts `--json` and the global `--filter '<jq expression>'` for in-process jq over the JSON value (requires `--json`; no pipe).

## Passive context on every read — hypothesis, not conclusion

`trace read`, `trace info`, `trace tree`, `trace list`, and the Read/Glob hook attach a passive-context shoulder per file:

```
[git: <state> · age: <age> · presence: <branches|local-only> · [callers: N · dependents: N] · ccn: <total> <rank> · owner: <name> · last: <subject>]
[docs: M/N in context · not loaded: <path>, <path>]
```

The second line surfaces only on Read enrichment (and `trace context <file>`); it names how many of the file's ancestor docs the agent already has in context vs which are still missing.

Lifecycle state labels (in order of precedence): `untracked`, `added (uncommitted)`, `renamed (uncommitted)`, `modified (new file)` (uncommitted modification with ≤1 commit), `modified (N commits)`, `renamed-from <path>`, `no-history`, `new (1 commit)`, `N commits`. Presence is the deploy-branch set the file is reachable from (e.g. `main`, `production`); `local-only` means the file isn't on any tracked branch.

**Treat each shoulder as a hypothesis to validate, not a conclusion to act on.**

What each label suggests:

- New / untracked / added → likely no callers yet. Probably don't need backward-compatibility, deprecation paths, or migration wrappers.
- Renamed-from → likely continuation, not new code. Carry prior knowledge forward.
- Recent modification with low commit count → likely active development; current shape may not be final.
- Many commits + old last_modified → likely settled code; existing patterns are load-bearing.
- `presence: local-only` → not deployed anywhere; safe to evolve freely.
- `presence: main, production` → in a deploy branch; capability regression matters.

For decision-shaped questions ("should I modify or stack?", "should I add this here or elsewhere?"), cross-check the lifecycle signal against project rules before recommending. Two cheap verification moves:

1. `trace docs <path>` (or read the nearest `Claude.md` / `Agents.md`) — projects encode their own rules about what can be edited.
2. `git show origin/production:<path>` — confirms whether a file the local view thinks is "new" is in fact already deployed.

Worktrees, squashed-baseline commits, and branch divergence can all make a file look "fresh" locally while being settled in production. Only recommend modify when lifecycle signal AND project rule AND production check all agree.

## Project docs: Claude.md, Agents.md, .claude/rules

The architecture graph treats project-docs as first-class nodes: `CLAUDE.md` / `Claude.md`, `AGENTS.md` / `Agents.md`, their `.local.md` peers, and every `.claude/rules/*.md`. `@include` edges and conditional `paths:` frontmatter are preserved. `Agents.md` / `AGENTS.md` is OpenAI's cross-harness project-rules convention (Codex, Cursor, Aider, Jules, Amp); tracer recognizes it alongside the Claude family so multi-harness repos work without dropping context.

`trace docs <path>` walks the ancestor chain for `<path>`, partitions against the session log, and returns the freshly-surfaced slice with content plus the skipped slice with per-entry source attribution:

```json
{
  "path": "<relative path>",
  "directory_scoped": <bool>,
  "source": "<calling surface>",
  "triggering_tool": "<tool|null>",
  "triggering_command": "<command|null>",
  "docs": [
    { "path": "Claude.md", "kind": "claude_md", "size": 12345, "large": false, "content": "..." }
  ],
  "doc_count": 1,
  "already_loaded": [
    { "path": "packages/claude/Claude.md", "kind": "claude_md", "size": 15388, "large": false, "source": "trace_inject_hook" }
  ]
}
```

`already_loaded` is omitted entirely when empty.

`trace docs <path> --graph` projects the whole-repo docs graph out of the unified `architecture/` cache entry plus the available-but-not-loaded set. `<path>` is optional under `--graph` (defaults to the cwd's repo root):

```json
{
  "graph": {
    "head": "<git HEAD>",
    "mtime_aggregate": "<fingerprint>",
    "built_at_ms": 1234567890,
    "nodes": [ { "path": "Claude.md", "kind": "claude_md", "size": 12345 } ],
    "edges": [ { "source": "...", "relation": "includes", "target": "..." } ]
  },
  "available_not_loaded": [ "Claude.md", "tools/tracer/Claude.md" ],
  "node_count": 12,
  "edge_count": 4
}
```

`trace docs status` is a pure read. Without a path, returns the full session manifest (`{ scope: "session", session_active, loaded[], loaded_count, by_source }`). With a path, returns the ancestor chain partitioned into `loaded` and `not_loaded` (`{ scope: "path", path, session_active, loaded[], not_loaded[], loaded_count, not_loaded_count, chain_size }`). Status never records — only `trace docs <path>` / `docs load` / `read --docs` write to the log.

`trace docs load` is the hook-facing alias: same `{ docs, doc_count, already_loaded? }` shape as path-mode, with `--source` defaulting to `trace_docs_load`. `inject_docs.py` invokes path-mode directly with `--source trace_inject_hook`.

## `trace read` and project-docs injection

`trace read` returns a fluff-stripped read of a file (license/generated banners stripped, decorative separators removed, runs of blank lines collapsed, every preserved line prefixed `L<n>:`). Modes:

- whole file (default), `--method <name>` (AST-resolved by exact or qualified name; pulls in the directly-attached leading comment block), `--lines L1:L2`, `--between START END` (regex anchors).
- `--at <ref>` reads at a git ref instead of the worktree. `--diff` (requires `--at`) appends a symbol-level diff of added/removed/changed top-level exports.
- `--raw` skips cleaning.

**Project-docs injection is OFF by default.** Pass `--docs` to opt in; the ancestor chain is loaded, partitioned against the session log, and the new slice is recorded under source `trace_read`. There is no `--no-docs` flag — the default is already off.

`trace context <file>` (file mode) emits the same passive-context shoulder plus the `docs: M/N in context` awareness line. `trace context` (no args) emits the eight-section session primer (environment, identity, tech stack, layout, common directories, git, rules, spine).

## Execution rules

- Never pipe trace output into anything
  (`grep`/`rg`/`head`/`tail`/`sed`/`awk`/`cut`/`sort`/`uniq`/`wc`/`column`/`fold`/`tr`/`jq`) or
  redirect it into a repo file — that discards the context trace exists to
  give you. For partial output, use the in-binary filter:
  `trace <cmd> --json --filter '<jq expression>'` runs a jq program over
  the value in-process (requires `--json` explicitly; never implied).
  Raw `cat`/`grep`/`rg`/`find`/`sed`/`awk`/`head`/`tail` on an in-repo path is
  also blocked — use the matching trace subcommand. Enforced by the
  `guard_trace.py` PreToolUse hook.
- Run `trace doctor` first if any command errors with a missing-dependency error.
- Use `trace info` and `trace structure` for architectural orientation **before** deep reads.
- Use `trace read` instead of raw Read whenever you want fluff-stripped output.
- Calibrate read depth using the survey's complexity distribution — files exceeding p95 get full reads; uniformly low complexity gets skims.

## When to use which command

| You want to… | Use |
|---|---|
| Find the most architecturally central files in a repo | `trace downstream --path <path>` — top-N ranked list |
| Find the highest-coupling files (most outgoing deps) | `trace upstream --path <path>` |
| Find what depends on X transitively (blast radius) | `trace downstream X --depth N` |
| Find what X depends on transitively | `trace upstream X --depth N` |
| Find every place X is used directly | `trace callers X` |
| Find where X is defined | `trace defines X` |
| Get oriented in an unfamiliar repo | `trace context` then `trace survey` then `trace list` |
| List the contents of one directory (1 level) | `trace list <dir>` |
| Walk a directory recursively with complexity | `trace tree <dir>` |
| Understand what a file/dir does at a glance | `trace info` |
| List a file's methods/props/vars and their connections | `trace structure` |
| See a file's outline | `trace symbols <file>` |
| Find a string anywhere | `trace grep <pattern>` |
| Find a structural pattern (e.g. all controllers calling a service) | `trace struct '<pattern>' -l <lang>` |
| Find files by basename (`find -name` mental model — e.g. `*.test.ts`, `Migration*.php`) | `trace find <pattern>` |
| Find files by full-path glob (shell-glob mental model — e.g. `src/**/*.tsx`, `app/Models/*.php`) | `trace glob <pattern>` |
| Read one method without the rest of the file | `trace read <file> --method <name>` |
| Read a whole file with token-wasting fluff cut | `trace read <file>` |
| Read a file with project docs inlined | `trace read <file> --docs` |
| Read a file at a git ref | `trace read <file> --at <ref>` |
| See which top-level symbols changed between a ref and worktree | `trace read <file> --at <ref> --diff` |
| Get the project docs (Claude.md / Agents.md / rules) for a path | `trace docs <path>` |
| Check which docs the agent already has in context | `trace docs status` or `trace docs status <path>` |
| Reset surfaced-docs state after a context reset so docs re-surface | `trace docs reset` |
| Browse the whole-repo docs graph | `trace docs --graph` |
| See what's changed vs a base ref, ordered by impact | `trace diff` (defaults to `origin/development`) |
| See the working-tree dirty set by blast radius | `trace status` |
| Understand history/why of a file | `trace history <file>` |
| Find who last touched a function or line range | `trace blame <file> <symbol>` or `trace blame <file> --lines L1:L2` |
| Get only part of a command's output | append `--json --filter '<jq expression>'` (in-process jq; never pipe to `jq`) |

## Directional intuition

`upstream` and `downstream` mirror the import direction: if A imports B, B is **upstream** of A (A depends on B); A is **downstream** of B (B is depended on by A).

- `trace upstream X` — what's upstream of X (what X depends on)
- `trace downstream X` — what's downstream of X (what depends on X)

In `--path` mode, the ranking inverts the natural-language framing:

- `trace downstream --path P` — files in P that have the most downstream dependents (i.e. the most-depended-on / most central / most load-bearing)
- `trace upstream --path P` — files in P that have the most upstream dependencies (i.e. highest-coupling / most fan-out)

## Hook surface

Six tracer hooks (Python, under `packages/agents/hooks/`) are wired locally in `settings.json` by absolute `~/.agents/hooks/<module>.py` path (never in plugin `hooks.json` — tracer hooks are the local experimental surface; plugin users get the binary, not the hooks):

- **`load_trace_context.py`** — SessionStart matcher `startup|resume|clear|compact`. Runs `trace context` (no args) and injects the eight-section repo primer as `hookSpecificOutput.additionalContext`.
- **`reload_harness_context.py`** — SessionStart matcher `startup|resume|clear|compact`. Runs `trace context prime --reason {session_start|post_compact}` (compact → `post_compact`, else `session_start`) so the session log mirrors what the harness re-emitted at session start or after compaction. Content-hash dedupe means unchanged docs add no events.
- **`enrich_on_read.py`** — PreToolUse matcher `Read|Glob|Grep|Edit|Write`. Read/Edit/Write → one `trace context <file>` for the touched file (passive shoulder + docs-awareness line). Glob/Grep → resolve the matched files and emit a full `trace context` shoulder for each, capped at 20. All branches inject as `additionalContext`; 5s timeout, silent fallback.
- **`guard_trace.py`** — PreToolUse matcher `Bash`. Blocks (a) `trace …` piped into `grep|rg|sed|awk|head|tail|cut|sort|uniq|wc|column|fold|tr|jq` or redirected into a repo file, and (b) raw `cat`/`grep`/`rg`/`find`/`sed`/`awk`/`head`/`tail` against any in-repo path. Whitelists `/tmp`, `/dev/null`, paths under `.claude/shaping/`, `.claude/plans/`, and `.tracer-cache/`. Block message names the trace subcommand to use instead.
- **`inject_docs.py`** — PreToolUse matcher `Bash`. When the agent runs a path-taking `trace` subcommand (`read|info|list|tree|structure|grep|struct|find|glob|blame|history|diff`), resolves the path argument and runs `trace docs <path> --source trace_inject_hook --triggering-tool Bash --triggering-command <cmd>` as a direct subprocess; injects the response as `additionalContext`. **Blocks the trace command (exit 2) if `trace docs` itself fails** — the agent must not run trace without docs context. Non-path-taking subcommands and unresolvable paths are clean no-ops.
- **`inject_rules.py`** — SessionStart + PreToolUse matcher `Read|Write|Edit|apply_patch`. Codex-only, because Claude Code loads `Claude.md` (root and nested) itself: injects the nearest `Claude.md` via `trace docs` — repo-root rules on SessionStart (with a `trace docs reset` on `clear`/`compact`), the touched file's rules on a file edit — wrapped in a `hookSpecificOutput.additionalContext` envelope. Best-effort: never blocks, silent fallback.
- **`archive_subagent_log.py`** — Invoked from UserPromptSubmit's `<task-notification>` parse when a subagent completes. Moves `<repo>/.tracer-cache/sessions/<sid>/<aid>/` to `<repo>/.tracer-cache/sessions/<sid>/archived/<aid>/`. The tracer's read path falls back to the archived directory when the active one is missing, so post-stop log queries keep working.

Identity propagation: `inject_docs.py`, `inject_rules.py`, `enrich_on_read.py`, and `reload_harness_context.py` set `AGENT_SESSION_ID` and `TRACER_AGENT_ID` from the hook payload on a **local env copy** passed to `trace` — never mutating `os.environ`. `AGENT_SESSION_ID` is the harness-neutral carrier `trace` resolves first, so it keys the session log to the run. The launcher's `CLAUDE_CODE_SESSION_ID` is left untouched in the process env, so on a nested codex run the proposal/commit guards can still resolve the governing proposing/executing mode through `owner_session`. Without a session id the log no-ops and dedupe stops working.

## Environment variables

- `AGENT_SESSION_ID` / `CODEX_THREAD_ID` / `CLAUDE_CODE_SESSION_ID` — session id for the log; resolved in that order. `AGENT_SESSION_ID` is the harness-neutral carrier our hooks set; the harness-native names are the fallback, innermost (codex thread) before outermost (claude session).
- `TRACER_AGENT_ID` — agent id within the session; defaults to `root` when absent.
- `TRACER_TRIGGERING_TOOL` / `TRACER_TRIGGERING_COMMAND` — stamped on log events at append time. `trace docs` sets these from its `--triggering-tool` / `--triggering-command` flags.
- `TRACE_BIN` — override the binary path used by the plugin launcher.
- `TRACER_CCN_BACKEND` — CCN backend selection; the AST tree-sitter walker is the single supported backend.
