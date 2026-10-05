# trace commands

Use this Reference when selecting an exact `trace` command, flag, JSON shape, docs command, or read mode.

## 1. Use the command catalog

### Start with the narrowest command that answers the question
Do not run a broad command and filter it outside trace. Every command takes the global `--budget <chars>` (30,000 by default, 0 unbounded) and, with `--json`, the global `--filter '<jq expression>'`. `trace -C <dir>` before the subcommand runs it in another repository, and every follow-up command the output names carries the same `-C`.

Template:
  ```bash
  trace doctor
  trace cache build [<path>]
  trace cache stats
  trace cache clear [--all]
  trace context
  trace context <paths...> [--directory] [--offset N] [--limit N] [--no-record]
  trace docs prime [<files...>] [--reason session_start|post_compact]
  trace list <dirs...> [--all] [--recent] [--limit N]
  trace tree <path> [--depth N]
  trace info <paths...> [--brief]
  trace structure <paths...>
  trace defines <symbol>
  trace callers <symbol> [--limit N]
  trace dependencies <symbol> [--depth N]
  trace dependencies --path <path> [--limit N]
  trace usages <symbol> [--depth N]
  trace usages --path <path> [--limit N]
  trace stats [<path>]
  trace grep <pattern | -e pattern> [paths...] [-i] [-l] [-c] [-C N] [-A N] [-B N] [-U] [-t <type>]... [-g <glob>]... [--at <ref>]
  trace pattern <pattern> -t <type> [paths...]
  trace logs [<pattern>] [--path <p>] [--file <glob>] [--since <when>] [--until <when>] [--around N] [--limit N]
  trace find <pattern> [bases...] [--path <p>] [--exclude <p>]... [--type f|d] [--limit N] [--sort complexity|recent|path]
  trace read <paths...> [--method <name>] [--at <ref>] [--lines L1:L2] [--between START END] [--diff] [--raw] [--all] [--docs]
  trace docs <paths...> [--directory] [--skip <path>]... [--source <s>] [--triggering-tool <t>] [--triggering-command <c>]
  trace docs <path> --graph
  trace docs status [<path>]
  trace docs reset [--source <s>]
  trace diff [paths...] [--base <ref>] [--symbols]
  trace status [--state added|renamed|modified|deleted|untracked]
  trace history [<file>] [<symbol>] [--contains <pattern>] [--regex] [--commit <ref>]
  trace blame <file> [<symbol>] [--lines L1:L2]
  ```

IF a trace call is slower than expected:
### Time its phases with `TRACE_TIMING=1`
`TRACE_TIMING=1 trace <cmd>` prints one line per phase to stderr in microseconds — the freshness sweep, each decoded cache entry by key, every git subprocess as `timing git <first two args>`, facts, render, and total. Stdout is unchanged.
Example: a warm `TRACE_TIMING=1 trace context <file>` prints `timing git status`, `timing git rev-parse HEAD`, and `timing git for-each-ref`, and no `ls-files` or `show-toplevel` line.

IF passing more than one path to `trace context`:
### Add `--no-record`
Multiple paths require `--no-record` and cannot combine with `--directory`, `--offset`, or `--limit`.

IF a path argument names a path that does not exist:
### Read the error, then the rest of the answer
The missing path is named on stderr, every other path still answers, and the command exits 2, as ripgrep does.

## 2. Know each command's edge cases

### `-t` takes a language name or any `rg --type-list` type
`trace grep -t` and `trace pattern -t` name the same languages. An unknown type exits 2 and names the accepted ones, never an empty result.

IF a search backend fails to run:
### Expect a nonzero exit, not an empty match
`grep`, `pattern`, and `grep --at` exit nonzero and name the failed backend when the search process itself fails; a genuine no-match prints `(no matches)` and exits zero.

### Take a `trace diff` row's declaration groups from its four arrays
`--json` carries `changed`, `removed`, `added`, and `touches` on the row. `changed` holds a `{before, after}` pair of surface rows; the other three hold surface rows. The hunk ranges come from a `--unified=0` pass, not the rendered lines.

### Check a declaration's classification with `trace structure --json`
`results.symbols_by_kind` groups the files' surface rows by `kind`, each row carrying its `file`, `node_id`, and `annotations`; `results.imports` and `results.exports` carry their `file` too, so one file and many share one shape.

IF asking who calls a property:
### Run `trace defines` instead
`callers` and `usages` on a property exit 2 with `a property has readers, not callers; tracer does not extract reads`.

## 3. Use `trace docs` correctly

### `trace docs <paths...>` sends the docs not yet loaded, nearest first
Text is Markdown: `## <path>` and the document below its frontmatter, nearest directory first, each from the first line the agent has not seen. The first doc longer than the room left under `--budget` arrives as `## <path> (L<from>-L<to> of <total>)`, cut at a whole line and ending in `read`'s trim marker; the docs after it are named under the `trace docs` command that sends them. A doc is loaded once every line arrived, so the next call continues it. `--skip <path>` leaves out a doc the calling command prints itself. Nothing prints when every doc is already loaded. `--json` returns every doc whole and records them all.

Template:
  ```json
  {
    "query": {
      "path": "relative/path",
      "directory_scoped": false,
      "source": "trace_docs",
      "triggering_tool": "Bash",
      "triggering_command": "trace read relative/path"
    },
    "context": {
      "already_loaded": [
        { "path": "packages/agents/Claude.md", "kind": "claude_md", "size": 15388, "large": false, "source": "inject_docs" }
      ]
    },
    "results": [
      { "path": "Claude.md", "kind": "claude_md", "size": 12345, "large": false, "content": "..." }
    ],
    "counts": { "docs": 1, "skipped": 1 }
  }
  ```

### `trace docs prime` records what the harness loaded
With files it records exactly those files as loaded, source `instructions_loaded`. With no file it records the root-to-cwd `Claude.md` chain.

### `trace docs <path> --graph` projects the docs graph
The path is optional with `--graph`; it defaults to the repository root for the current working directory.

Template:
  ```json
  {
    "query": { "path": "relative/path", "scope": "repo" },
    "context": {
      "available_not_loaded": [ "Claude.md", "tools/tracer/Claude.md" ]
    },
    "results": {
      "head": "git HEAD",
      "mtime_aggregate": "fingerprint",
      "built_at_ms": 1234567890,
      "nodes": [ { "path": "Claude.md", "kind": "claude_md", "size": 12345 } ],
      "edges": [ { "source": "...", "relation": "includes", "target": "..." } ]
    },
    "counts": { "nodes": 12, "edges": 4 }
  }
  ```

### `trace docs status` is a pure read
Without a path it returns the session manifest: `results.loaded[]`, with `session_active` and `by_source` in `context` and the count in `counts`. Each loaded entry includes `total_lines`, `lines_read`, and `read_fraction`; a doc-injected file never read has `read_fraction: 0.0`. With a path it partitions the ancestor chain into `loaded` and `not_loaded`, nearest first.

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

### Several files share one budget
Each file of a multi-file read gets an even share of `--budget`, and a read cut at its share ends with its own trim marker. The marker sits inside the content stream and survives `--raw`. `--json` carries `truncated`, `shown_lines` as `[first, last]`, and `total_lines` on every read.

### `--at` reads a git ref
Use `--diff` with `--at` to append a symbol-level diff of added, removed, and changed top-level exports.

## 6. Take facts from the JSON document

### Take a file's facts from `context.files[<path>]`
The keys are the front matter's: `file`, `lines`, `cyclomatic_complexity`, `complexity_rank`, `imported_by`, `imports`, and `git` with `status`, `renamed_from`, `commits` (or `commits_at_least` when the history walk stopped at its cap), `commits_last_30_days`, `first_commit`, `last_commit`, `on_deploy_branches`, `usually_changed_with`, and `main_author`. `git.status` is `unmodified`, `modified`, `added`, `renamed`, `deleted`, or `untracked`; a file outside any git repository carries no `git`.

### Take the surface from `context.files[<path>].surface[]`
Each row is `{header_line, line, end_line, kind, name, container, parent, header, annotations, cyclomatic_complexity}`. `parent` is the index of the row that holds it in the same array. The batch form `trace context <paths...> --no-record --json` keeps the rendered text per file, surface included, in `results[].content`.

### Take a search hit's declarations from the result row itself
A `grep` or `pattern` row carries `declaration` and `type` as whole surface rows, so nothing joins back to `context.files`. A `-C` row adds `before` and `after`.

## 7. Know which project docs trace recognizes

### Project docs are graph nodes
The graph recognizes `CLAUDE.md`, `Claude.md`, `AGENTS.md`, `Agents.md`, their `.local.md` peers, and every `.claude/rules/*.md`. It preserves `@include` edges and conditional `paths:` frontmatter.
