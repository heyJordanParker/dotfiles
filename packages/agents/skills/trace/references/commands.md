# trace commands

Use this Reference when selecting an exact `trace` command, flag, JSON shape, docs command, or read mode.

## 1. Use the command catalog

### Start with the narrowest command that answers the question
Do not run a broad command and filter it outside trace.

Template:
  ```bash
  trace doctor
  trace cache build [<path>]
  trace cache stats
  trace cache clear [--namespace file|architecture] [--all]
  trace context
  trace context <path> [--offset N] [--limit N] [--no-record]
  trace context prime --reason session_start|post_compact [--observed-from PATH|-]
  trace list <dir> [--all] [--recent] [--limit N]
  trace tree <path> [--depth N]
  trace info <path> [--brief]
  trace structure <file>
  trace symbols <file>
  trace defines <symbol>
  trace callers <symbol>
  trace upstream <symbol> [--depth N]
  trace upstream --path <path> [--limit N]
  trace downstream <symbol> [--depth N]
  trace downstream --path <path> [--limit N]
  trace survey [<path>]
  trace grep <pattern> [-l <lang>] [--path <path>]
  trace struct <pattern> -l <lang> [--path <path>]
  trace find <pattern> [<base>] [--path <p>] [--exclude <p>]... [--type f|d] [--limit N] [--sort complexity|recent|path]
  trace glob <pattern> [<base>] [--details]
  trace read <paths...> [--method <name>] [--at <ref>] [--lines L1:L2] [--between START END] [--diff] [--raw] [--all] [--docs]
  trace docs <path> [--directory] [--source <s>] [--triggering-tool <t>] [--triggering-command <c>]
  trace docs <path> --graph
  trace docs load <path> [--source <s>] [--triggering-tool <t>] [--triggering-command <c>]
  trace docs status [<path>]
  trace docs reset [--source <s>]
  trace diff [--base <ref>] [--symbols]
  trace status [--state added|renamed|modified|deleted|untracked]
  trace history [<file>] [<symbol>] [--contains <pattern>]
  trace blame <file> [<symbol>] [--lines L1:L2]
  ```

### Every value command supports in-process filtering
Use `--json --filter '<jq expression>'`. The filter requires `--json`.
Never: pipe to `jq`.

## 2. Match common questions to commands

### Use centrality commands for Architecture questions
`trace downstream --path <path>` finds the most-depended-on files. `trace upstream --path <path>` finds the highest-coupling files.

### Use symbol commands for relationship questions
`trace downstream X --depth N` finds what depends on X. `trace upstream X --depth N` finds what X depends on. `trace callers X` finds direct use sites. `trace defines X` finds definitions.

### Use orientation commands for unfamiliar code
Start with `trace context`, then `trace survey`, then `trace list`, `trace tree`, `trace info`, `trace structure`, or `trace symbols`.

### Use search commands by match type
Use `trace grep` for text in code, `trace logs` for text in a log file, `trace struct` for structural search, `trace find` for basenames, and `trace glob` for full-path globs.

### Use history commands for why and ownership
Use `trace diff` for changed files, `trace status` for dirty files by blast radius, `trace history` for file or symbol history, and `trace blame` for function or line ownership.

## 3. Use `trace docs` payloads correctly

### `trace docs <path>` surfaces ancestor docs once per session
It returns new docs plus already-loaded docs. `already_loaded` is omitted when empty.

Template:
  ```json
  {
    "path": "relative/path",
    "directory_scoped": false,
    "source": "calling_surface",
    "triggering_tool": "Bash",
    "triggering_command": "trace read relative/path",
    "docs": [
      { "path": "Claude.md", "kind": "claude_md", "size": 12345, "large": false, "content": "..." }
    ],
    "doc_count": 1,
    "already_loaded": [
      { "path": "packages/agents/Claude.md", "kind": "claude_md", "size": 15388, "large": false, "source": "trace_inject_hook" }
    ]
  }
  ```

### `trace docs <path> --graph` projects the docs graph
The path is optional with `--graph`; it defaults to the repository root for the current working directory.

Template:
  ```json
  {
    "graph": {
      "head": "git HEAD",
      "mtime_aggregate": "fingerprint",
      "built_at_ms": 1234567890,
      "nodes": [ { "path": "Claude.md", "kind": "claude_md", "size": 12345 } ],
      "edges": [ { "source": "...", "relation": "includes", "target": "..." } ]
    },
    "available_not_loaded": [ "Claude.md", "tools/tracer/Claude.md" ],
    "node_count": 12,
    "edge_count": 4
  }
  ```

### `trace docs status` is a pure read
Without a path, it returns the session manifest: `scope`, `session_active`, `loaded[]`, `loaded_count`, and `by_source`. Each loaded entry includes `total_lines`, `lines_read`, and `read_fraction`; a doc-injected file never read has `read_fraction: 0.0`. With a path, it partitions the ancestor chain into `loaded` and `not_loaded`.

### `trace docs load` is hook-facing
It forwards to path mode and uses `--source trace_docs_load` by default. `inject_docs.py` invokes path mode with `--source trace_inject_hook`.

## 4. Use `trace logs` for logs

### `trace logs` returns entries, not lines
One line is one entry. A line carrying no timestamp attaches to the entry above it, so a stack trace comes back whole. Laravel, PHP and WordPress `debug.log`, nginx access and error, syslog, log4j, Python logging, Go, and JSON lines all frame; a file with no timestamp anywhere returns one entry per line.

### The defaults answer the common question with no flags
`--path .`, `--file *.log*`, `--around 0`, `--limit 20` newest first, and smart-case matching. `--file *.log*` reaches `access.log`, `access.log.1`, `access.log.2.gz`, and `laravel-2026-08-15.log`. A `.git`, `node_modules`, `vendor`, or `.tracer-cache` directory is never walked.

### Windows compare against the log's own clock
`--since` and `--until` take `YYYY-MM-DD`, `YYYY-MM-DD HH:MM[:SS]`, or `HH:MM[:SS]`. A bare time means that time on the date of the newest log selected. A dated filename outside the window is never opened, so a window over a rotated directory costs one file.

## 5. Use `trace read` modes correctly

### Project docs are opt-in for direct reads
Pass `--docs` to load ancestor docs. There is no `--no-docs` flag because direct reads default to docs off.

### `--raw` skips cleaning
Default reads strip generated banners, decorative separators, runs of blank lines, and prefix preserved lines with `L<n>:`.

### Continue a trimmed read by running the command its marker prints
Every read carries a size budget. A read past it is cut at a whole line and ends with a marker naming the next window.

- The marker reads `[trimmed at L<n> of <total> — continue: trace read <file> --lines <n+1>:<end>]`.
- The marker sits inside the content stream and survives `--raw`.
- The budget applies to each file of a multi-file read separately.
- `--all` returns every line of the selection with no marker.
- `--json` carries `truncated`, `shown_lines` as `[first, last]`, and `total_lines` on every read.

### `--at` reads a git ref
Use `--diff` with `--at` to append a symbol-level diff of added, removed, and changed top-level exports.

## 6. Interpret passive-Context shoulders

### The shoulder has two lines when docs awareness is available
The first line carries lifecycle and complexity; the second line carries docs Context coverage.

Template:
  ```text
  [git: <state> · age: <age> · presence: <branches|local-only> · churn: N commits, N/30d · callers: N · dependents: N · loc: N · ccn: <total> <rank> · together: <files> · owner: <name> · last: <subject>]
  [docs: M/N in Context · not loaded: <path>, <path>]
  ```

### Lifecycle labels are ordered by confidence
Labels include `untracked`, `added (uncommitted)`, `renamed (uncommitted)`, `modified (new file)`, `modified (N commits)`, `renamed-from <path>`, `no-history`, `new (1 commit)`, and `N commits`. Presence names deploy branches such as `main` or `production`; `local-only` means no tracked branch reaches the file.

## 7. Know which project docs trace recognizes

### Project docs are graph nodes
The graph recognizes `CLAUDE.md`, `Claude.md`, `AGENTS.md`, `Agents.md`, their `.local.md` peers, and every `.claude/rules/*.md`. It preserves `@include` edges and conditional `paths:` frontmatter.
