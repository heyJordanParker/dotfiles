---
name: trace
description: Code intelligence for the local codebase — search, callers, definitions, symbols, complexity, file/method reads with rich Architectural Context. TRIGGER whenever you need to find, understand, or trace relationships in code instead of raw grep or unfiltered file reads.
---

# trace

- `trace` returns code intelligence: matches plus per-file complexity, callers, callees, nearest project docs, git activity, and deploy-branch presence.
- Use `trace` instead of raw grep, find, and file reads when working inside the local codebase.
- Every value command accepts `--json` and the global `--filter '<jq expression>'` for in-process filtering.

## 1. Respect the execution Rules

### Do not pipe or redirect trace output
Use `trace <cmd> --json --filter '<jq expr>'` when you need partial output. The filter requires `--json`.
Never: pipe `trace` into `grep`, `rg`, `head`, `tail`, `sed`, `awk`, `cut`, `sort`, `uniq`, `wc`, `jq`, or redirect it into a repository file.

### Do not use raw file-search commands on repository paths
Use the matching `trace` subcommand instead. `guard_trace.py` blocks raw `cat`, `grep`, `rg`, `find`, `sed`, `awk`, `head`, and `tail` against in-repo paths.

IF a trace command reports missing dependencies:
### Run `trace doctor`
`trace doctor` reports the missing external tools and install guidance.

## 2. Orient before deep reads

### Start broad, then narrow
Run `trace context` first. Then use `trace survey`, `trace list`, `trace tree`, `trace info`, or `trace structure` before reading code deeply.

Template:
  ```bash
  trace context
  trace survey
  trace list packages/agents/skills
  trace info packages/agents/skills/trace/SKILL.md --brief
  trace structure tools/tracer/src/main.rs
  ```

## 3. Choose the smallest matching command

### Use relationship commands for symbols
Use `defines`, `callers`, `upstream`, and `downstream` when the question is about a symbol or dependency direction.

Template:
  ```bash
  trace defines <symbol>
  trace callers <symbol>
  trace upstream <symbol|--path P>
  trace downstream <symbol|--path P>
  ```

### Use search commands for unknown names
Use `trace grep` for text, `trace struct` for structural search, `trace find` for basenames, and `trace glob` for full paths.

Template:
  ```bash
  trace grep <pattern> [-l lang] [--path P]
  trace struct <pattern> -l lang [--path P]
  trace find <pattern> [base]
  trace glob <pattern> [base]
  ```

### Use state commands for change review
Use `trace diff`, `trace status`, `trace history`, and `trace blame` to understand change scope, file history, and ownership.

Template:
  ```bash
  trace diff [--base ref] [--symbols]
  trace status
  trace history [<file>] [<symbol>]
  trace blame <file> [<symbol>] [--lines L1:L2]
  ```

## 4. Read with trace

### Use `trace read` instead of raw reads
`trace read` strips Fluff and keeps line numbers. Read whole files, methods, line ranges, anchor ranges, or a git ref.

Template:
  ```bash
  trace read <paths>
  trace read <file> --method <name>
  trace read <file> --lines L1:L2
  trace read <file> --between START END
  trace read <file> --at ref
  trace read <file> --at ref --diff
  trace read <file> --docs
  ```

### Calibrate read depth by complexity
Use `trace survey` to find the complexity distribution. Full-read files past the repository p95. Skim uniformly-low files only when the task does not need every line.

## 5. Treat passive Context as a hypothesis

`trace read`, `trace info`, `trace tree`, `trace list`, and the Read Hook attach a shoulder with git state, age, branch presence, callers, dependents, complexity, owner, and last commit.

### Validate lifecycle labels before acting
New, untracked, or added files likely have no callers. Renamed files continue old code. Recent files with few commits may still be moving. Old files with many commits are settled Precedents. `local-only` is not deployed; `main` or `production` means capability regression matters.

### Cross-check modify-or-stack Decisions
Before recommending modify or stack, read the nearest `Claude.md` or `Agents.md` and check `git show origin/production:<path>`. A locally new file may already be deployed through worktrees, squashed baselines, or branch divergence.

## 6. Keep dependency direction straight

### Upstream is what a thing depends on
If A imports B, B is upstream of A.

### Downstream is what depends on a thing
If A imports B, A is downstream of B.

### Path mode ranks by file centrality
`trace downstream --path P` returns the most-depended-on files in `P`. `trace upstream --path P` returns the highest fan-out files in `P`.

## References

- Full command selection, flags, JSON payload shapes, `trace docs`, and `trace read` modes → [commands.md](references/commands.md)
- Hook events, identity propagation, and session-log environment variables → [hooks.md](references/hooks.md)
- Disk cache namespaces, invalidation, prebuild, and install checks → [cache.md](references/cache.md)
