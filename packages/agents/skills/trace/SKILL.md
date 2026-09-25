---
name: trace
description: Code intelligence for the local codebase — search, callers, definitions, symbols, complexity, and file or method reads, each carrying the file's facts and the project docs not yet loaded. TRIGGER whenever you need to find, understand, or trace relationships in code instead of raw grep or unfiltered file reads.
---

# trace

- `trace` returns code intelligence: matches plus each file's facts — size, complexity, importers, git history, deploy branches, and the project docs not yet loaded.
- Text output is for Agents. `--json` output is for scripts and hooks: every `--json` result is one document, `{query, context, results, counts}`, with rows in `results`, each file's facts in `context.files`, and totals in `counts`.
- Every path argument takes several paths, and a directory names the files under it.

## 1. Respect the execution Rules

### Use `trace` instead of raw grep, find, and file reads
Inside the local codebase, every search, listing, and read goes through the matching `trace` command.

### Do not pipe or redirect trace output
Use `trace <cmd> --json --filter '<jq expr>'` when you need partial output. The filter requires `--json`, and it keeps `context` for the files its result names.
Never: pipe `trace` into `grep`, `rg`, `head`, `tail`, `sed`, `awk`, `cut`, `sort`, `uniq`, `wc`, `jq`, or redirect it into a repository file.

### Do not use raw file-search or listing commands on repository paths
Use the matching `trace` subcommand instead. `guard_trace.py` blocks raw `cat`, `grep`, `rg`, `find`, `sed`, `awk`, `head`, and `tail` against in-repo paths, and blocks `ls` and `tree` reaching the repo.
Example: `trace list src/` replaces `ls src/`; `trace tree src/` replaces `tree src/`.

### Do not read repository code with git
Use the matching `trace` subcommand. `guard_trace.py` blocks the git forms trace already answers.
Example: `trace read <path> --at <ref>` replaces `git show <ref>:<path>` and `git cat-file -p`; `trace grep <pattern>` replaces `git grep <pattern>`; `trace grep <pattern> --at <ref>` replaces `git grep <pattern> <ref>`; `trace blame <file>` replaces `git blame`; `trace history <file>` replaces `git log -- <file>`; `trace history <file> <symbol>` replaces `git log -L`; `trace history --contains <pattern>` replaces `git log -S`; `trace diff` replaces `git diff --name-status`.
Every other git command passes, including `git status`, `git branch`, `git tag`, `git rev-parse`, `git reflog`, `git stash`, `git merge-base`, `git describe`, `git ls-files`, plain `git diff`, `git log -p`, and `git log -G`.

### Bound a search before sorting it
A search piped into `sort` or `uniq` holds its whole output in memory, so `guard_trace.py` blocks one that is not bounded. Count with `rg -c`, cap with `rg --max-count <n>`, or put `head -<n>` before the sort.

### Find the newest artifact with `trace list --recent`
`trace list` lists the filesystem, so gitignored artifact directories (test runs, logs, builds) list too. `--recent` orders newest-first by mtime, `--limit N` caps the rows, and `entries=N` always carries the full count.
Example: `trace list tests/.runs --recent --limit 5` replaces `ls -t tests/.runs | head -5`.

## 2. Read an output cut to its budget

### Expect every file and declaration, with less detail
Text output fits `--budget <chars>`, 24,000 by default. The budget cuts detail, never coverage: the files the fewest others import lose their detail first, down to their path, and a listing too long even for bare paths names its files one line per directory (`dir/: a.php, b.php`).

### Run the command the last line names for the rest
A cut output ends with `[N of M files shortened to fit --budget B — whole: <command> --budget 0]`. Run that command, or narrow the paths, when the cut detail is what you need.

## 3. Orient before deep reads

### Start broad, then narrow
Run `trace context` first. Then use `trace stats`, `trace list`, `trace tree`, `trace info`, or `trace structure` before reading code deeply.

Template:
  ```bash
  trace context
  trace stats
  trace list packages/agents/skills
  trace info packages/agents/skills/trace/SKILL.md --brief
  trace structure tools/tracer/src/main.rs tools/tracer/src/output.rs
  ```

## 4. Choose the smallest matching command

### Use relationship commands for symbols
Use `defines`, `callers`, `dependencies`, and `usages` when the question is about a symbol or dependency direction. A symbol takes a qualifier, `Class::method` or `Class.method`, to pick one of several same-named declarations.

Template:
  ```bash
  trace defines <symbol>
  trace callers <symbol>
  trace callers Contact::recalculateStats
  trace dependencies <symbol|--path P>
  trace usages <symbol|--path P>
  ```

### Use search commands for unknown names
Use `trace grep` for text in code, `trace logs` for text in a log file, `trace pattern` for structural search, and `trace find` for basenames and full paths. `trace grep` takes ripgrep's own flags and paths, and `-e <pattern>` for a pattern that starts with `-`, such as PHP's `->method(`.

Template:
  ```bash
  trace grep <pattern> [paths...] [-i] [-t <type>] [-g <glob>] [-C <n>] [-A <n>] [-B <n>] [-U] [-l | -c] [--at <ref>]
  trace pattern <pattern> -t <type> [paths...]
  trace find <pattern> [bases...]
  ```

### Search a commit with `trace grep --at <ref>`
`--at` searches that commit instead of the working tree, which is what `git grep <pattern> <ref>` would answer. `-t` and `-g` select the same files on a commit as on the working tree; `-U` needs the working tree.

IF a search result names a `nested repository (its own search scope)`:
### Re-run the search with the base inside the nested repository
A vendored checkout carries its own `.git`, and enumeration never crosses into it from above.
Example: `trace find "*.min.js" public/content/themes` returns no matches and names `bricks`; `trace find "*.min.js" public/content/themes/bricks` returns 128.

### Use state commands for change review
Use `trace diff`, `trace status`, `trace history`, and `trace blame` to understand change scope, file history, and ownership.

Template:
  ```bash
  trace diff [paths...] [--base ref] [--symbols]
  trace status
  trace history [<file>] [<symbol>]
  trace history --contains <pattern> [--regex]
  trace history --commit <ref>
  trace blame <file> [<symbol>] [--lines L1:L2]
  ```

### Read a commit's full body with `trace history --commit`
The subject says what changed and the body says why. `--commit <ref>` returns the message, author, parents, changed files, and changed lines in one call.
Never: `git show` for a commit message.

## 5. Read with trace

### Use `trace read` instead of raw reads
`trace read` strips Fluff and keeps line numbers. Read whole files, several files, methods, line ranges, anchor ranges, or a git ref.

Template:
  ```bash
  trace read <paths...>
  trace read <file> --method <name>
  trace read <file> --lines L1:L2
  trace read <file> --between START END
  trace read <file> --at ref
  trace read <file> --at ref --diff
  trace read <file> --docs
  trace read <file> --all
  ```

IF `--method` names a function the file does not declare:
### Read the lines the refusal names
The refusal lists the lines where the word appears and the `trace read <file> --lines a:b` command for them.

### Scope the read by `lines:` before reading a whole file
Every file's facts carry `lines:`. `trace read` fits the file to `--budget`: it keeps every declaration row and cuts the content at a whole line, ending with `[trimmed at L<n> of <total> — continue: trace read <file> --lines <n+1>:<end>]`. A trimmed read still costs a second call, so scope the first one: read the rows, then `--method <name>` or `--lines L1:L2`.
Never: `--all` to skim a large file. It returns every line and takes the whole cost.

### Calibrate read depth by complexity
Use `trace stats` for the complexity distribution. Read whole the files past the repository's p95. Skim uniformly-low files only when the task does not need every line.

IF reading or searching a log file:
### Use `trace logs`
`trace grep` does not search a gitignored path and `trace read` fits the file to its budget. `trace logs` reads the files the ignore rules skip, returns one entry per line with stack traces kept whole, and windows by time so a rotated 80 MB directory returns a window.

Template:
  ```bash
  trace logs [<pattern>] [--path P] [--file GLOB] [--since WHEN] [--until WHEN] [--around N] [--limit N]
  trace logs <pattern> --path storage/logs
  trace logs <pattern> --since "2026-08-15 10:52" --until "2026-08-15 10:55"
  trace logs <pattern> --around 2
  trace logs <pattern> --file "laravel-*.log"
  trace logs --path /var/log/nginx --limit 5
  ```

## 6. Read the facts and rows instead of opening the file

- `trace context <file>`, `read`, file `info`, `structure`, and `blame <file> <symbol>` print the file's facts as YAML front matter, then one row per declaration.
- A windowed `read --method`, `read --lines`, `read --between`, or `context --offset/--limit` prints only the rows whose span intersects the window, plus the rows that hold them.
- A list of many files prints each file's facts on one line: `path  {imported_by: 63, cyclomatic_complexity: 66, lines: 318, git: modified}`.

### Read the front matter as the file's facts
Keys use git and GitHub words; dates are ages. `docs_not_loaded` names the project docs this session has not read, and `directory` names the file's directory, its importers and imports, and its entries.

Template:
  ```yaml
  ---
  file: app/Tenant/Entities/Contact.php
  lines: 318
  cyclomatic_complexity: 66
  complexity_rank: high
  imported_by: 63
  imports: 12
  git:
    status: modified
    commits: 41
    commits_last_30_days: 6
    first_commit: 4 months ago
    last_commit: "3 days ago by Jordan Parker: fix: …"
    on_deploy_branches: [main, production]
    usually_changed_with: [app/Tenant/Services/StoreService.php]
    main_author: Jordan Parker
  docs_not_loaded: [app/Tenant/Claude.md]
  directory:
    path: app/Tenant/Entities/
    imported_by: 140
    imports: 35
    annotations: {Entity: 27, Field: 310}
    entries: [Address.php, Article.php, …]
  ---
  ```

### Read the row as the source's own text
A row is `L<line>` and the declaration's header as written — attributes, modifiers, name, parameters, return type, heritage — with every body elided as `{ … }` and a data initializer as `= …`. A row indents two spaces under the declaration that holds it, and a function's complexity follows as a comment in the file's own syntax.

Template:
  ```text
  L39   #[Entity]
        class Contact extends User implements UrlRoutable { … }
  L517    #[Action(Mode::Write)]
          #[Access(System::class)]
          public static function create(array $attributes = []): static { … }  // complexity 1
  ```

### Read a search hit under the declarations that hold it
`grep` and `pattern` print each file's facts once, then every declaration that encloses a match once, then the matched lines, `L<n>:` for a match and `L<n>-` for a `-C` context line, as `git grep --show-function` does. A match on a declaration's own line stands in for it. The footer counts matches and files, and a searched word the graph knows names `trace callers <word>`.

Template:
  ```text
  app/Tenant/Entities/Contact.php  {imported_by: 63, cyclomatic_complexity: 66, lines: 318, matches: 2}
    L39   #[Entity] class Contact extends User implements UrlRoutable { … }
    L578    public function recalculateStats(): void { … }  // complexity 3
    L580:     app(Analytics::class)->recalculateStats($this);

  3 matches in 2 files
  recalculateStats: 2 definitions (function) · mentioned in 5 files → trace callers recalculateStats
  ```

### Read the four declaration groups under a `diff` row
`changed:` gives the header before and after, `removed:` and `added:` the declarations the change dropped and introduced, and `touches:` every current-side declaration a hunk intersects.

Template:
  ```text
  changed: L493 #[Field(label: 'Audiences', hidden: true, order: 44, config: ['attachable' => false])] public function audiences(): BelongsToMany { … } → L493 #[Field(label: 'Audiences', hidden: true, order: 44)] public function audiences(): BelongsToMany { … }
  removed: L34 fn signature_lists(files: &[String], repo_root: &Path) -> HashMap<String, Vec<Signature>> { … }
  added: L28 fn record_for(file: &str, line: i64, name: &str) -> Option<surface::Row> { … }
  touches: L99 pub fn run(symbol: &str, limit: usize, as_json: bool) -> Result<Value> { … }
  ```

## 7. Treat facts as a hypothesis

### Validate lifecycle before acting
An untracked or added file likely has no callers. A renamed file continues old code. A recent file with few commits may still be moving. An old file with many commits is a settled Precedent. A file with no `on_deploy_branches` is not deployed; `main` or `production` means Capability regression matters.

### Cross-check modify-or-stack Decisions
Before recommending modify or stack, read the nearest `Claude.md` or `Agents.md` and check `trace read <path> --at origin/production`. A locally new file may already be deployed through worktrees, squashed baselines, or branch divergence.

### Read an annotation as a name, never a meaning
`directory.annotations` counts the attributes, decorators, and docblock tags the directory's files carry, and a row shows each one as written above its declaration. Tracer does not know what a name means; the project's Claude.md does. Read it before touching a declaration that carries one.

## 8. Keep dependency direction straight

- If A imports B, B is a dependency of A, and A is a usage of B.
- `trace usages --path P` returns the most-depended-on files in `P`.
- `trace dependencies --path P` returns the highest fan-out files in `P`.

## References

- Full command selection, flags, JSON payload shapes, `trace docs`, and `trace read` modes → [commands.md](references/commands.md)
- Hook events, identity propagation, and session-log environment variables → [hooks.md](references/hooks.md)
- Disk cache namespaces, invalidation, prebuild, `trace doctor`, and install checks → [cache.md](references/cache.md)
