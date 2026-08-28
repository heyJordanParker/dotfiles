# Tracer restructure

Status: proposed, not started. The memory and latency fixes are landed
(`8016a70`); everything below is the next, larger piece.

Every number here was measured this session on
`/Users/jordan/Developer/creator-income-blueprint` (2,987 files, 50 MB
architecture graph) and `/Users/jordan/dotfiles` (775 files, 4.1 MB graph),
warm cache, via `/usr/bin/time -l`.

## The headline: every file read costs 182 MB to print two integers

| command | creator-income-blueprint | dotfiles |
|---|---|---|
| `trace read <file> --lines 1:5` | **182 MB / 0.25s** | 35 MB / 0.09s |
| `trace context <file>` | **181 MB / 0.39s** | — |
| `trace docs <file>` (loads no graph) | 7.8 MB / 0.03s | — |

`trace read` is the most-mentioned command in the transcript corpus (161,860)
and `guard_trace.py` forces every `cat`, `head`, and `sed` onto it.
`enrich_on_read.py` additionally runs `trace context <file>` on every
Read/Edit/Write, and up to `MATCH_CAP = 20` times per Grep/Glob.

The cause is `passive_context::render(facts, graph)`: the shoulder carries
`callers: N · dependents: N`. Producing those two numbers calls
`architecture::load_cached` (`context.rs:124-131`), which at
`architecture.rs:928-934` decodes the whole graph **and validates it by
walking every tracked file and hashing all of them**. Cost scales with graph
size, which is why the same command is five times cheaper on dotfiles.

A second defect sits in the same function: `load_cached` memoizes, then
returns `cached.clone()` — a full deep copy of the graph on a memo *hit*.

## Decision 1 — the shoulder must not decode the graph

Precompute `path -> (callers, dependents)` into a sidecar in the `file/`
namespace, keyed by the same fingerprint the graph already uses
(`cache::architecture_fingerprint`). The shoulder then reads a small map
instead of a symbol graph.

- Removes roughly 174 MB and 0.16s from the most frequent operation in the
  system, for every command that renders a shoulder: `read`, `context`,
  `info`, `status`, `list`, `find`, `glob`.
- Invalidation is already solved; the sidecar is written where the graph is.
- The validation walk stays for commands that genuinely need the graph:
  `callers`, `downstream`, `upstream`, `defines`.
- Independent of every other decision here. Lands alone.
- Confidence: high. Measured on two repos of different graph size, isolated
  against `trace docs` as a no-graph baseline.

## Decision 2 — one document envelope

Measured across the live binary, the 24 commands use **nine** names for "the
rows" (`matches`, `entries`, `files`, `results`, `definitions`, `symbols`,
`regions`, `docs`, `top_complex`), **ten** for "how many", and put enrichment
in three different places — inside each row, at top level, or nowhere.
`callers` has no fixed key set at all: its top level is a map keyed by symbol
node id.

Fixed slots for every command: `query`, `context`, `results`, `counts`.
Enrichment moves into `context`, keyed by path, **outside** `results`.

- A row projection then physically cannot strip enrichment. That is the
  `--filter` hole, closed by shape rather than by policing.
- One native vocabulary becomes possible across all commands instead of one
  flag per command. This is why an earlier `--section <name>` idea failed:
  there were only per-command ad-hoc keys to name.
- An Agent learns one shape, not twenty, so it stops writing bespoke jq.
- Cost: every command's JSON changes. 24 commands, assertions across 22 test
  files, `guard_trace.py`, `SKILL.md`, `references/commands.md`.
- Confidence: high on the diagnosis, medium-high on scope.

## Decision 3 — native selectors, jq as the fallback

Only after Decision 2. `--counts`, `--paths`, `--results` mean the same thing
on every command. `--filter` survives for the genuinely novel question, and
because `context` is an envelope slot, enrichment is emitted alongside any
filtered result.

Grounded in what the 2,206 recorded `--filter` expressions ask for:

| intent | ~uses |
|---|---|
| section selection (`.content`, `.methods`, `.symbols`) | 250 |
| path lists (`[.matches[].file] \| unique`) | 150 |
| counts (`.match_count`, `.matches \| length`) | 130 |
| whole document (`.`, `keys`) | 190 |
| slices (`.entries[0:40]`) | 30 |

Not one uses jq's expressive power — no user-defined functions, no reduce.
The single worst case is `trace read --json --filter '.content'` (129 uses),
which returns raw file text and drops the shoulder, the docs block, and the
truncation metadata. It is `cat` with extra steps, reached through the flag
`guard_trace.py:47-48` teaches as the sanctioned replacement for the pipe it
just blocked.

## Decision 4 — collapse the overlaps

Same purpose, different parameter:

| today | becomes |
|---|---|
| `find` (basename), `glob` (path glob) | one file selector, two pattern kinds |
| `list` (one level), `tree` (recursive) | one listing command with depth |
| `structure`, `symbols` (both enumerate declarations) | one command; `info` is the same thing deeper |
| `callers` (= `downstream` depth 1 over reference edges) | folded into the direction walk |

`grep` and `struct` share a purpose but differ in matcher. `logs` is
genuinely distinct: `logs.rs:3-5` records that ripgrep's ignore walk skips
log files and `read` cannot load an 80 MB rotated directory. `history`,
`blame`, `docs`, `context`, `diff`, `status`, `survey`, `doctor`, and `cache`
each answer something no other command does.

- Most user-visible change, and the one most likely to break Agent habit
  across roughly 500,000 recorded invocations.
- Confidence: medium. The overlaps are measured from real document shapes,
  but which commands merge is a judgment about how Agents think.

## Decision 5 — the git surface tracer must own before git can be banned

`guard_trace.py` matches the first word of a segment, so every `git`
subcommand passes untouched. The transcript census counts `git show
<ref>:<path>` 40,433 times, `git diff` on content 48,452, `git log -S` 3,799,
`git log -L` 3,656, `git grep` 2,280. Each of those reads repository code
with no complexity, no callers, and no nearest `Claude.md`.

The census also separates the git commands that read code from the ones that
do version-control work. That line is the design: tracer owns the first set,
and the second set stays raw git.

Already covered, so banning these costs nothing:

| raw form | trace command |
|---|---|
| `git show <ref>:<path>`, `git cat-file -p` | `trace read <path> --at <ref>` |
| `git grep <pattern>` (worktree) | `trace grep <pattern> [--path <dir>]` |
| `git blame` | `trace blame <file> [<symbol>] [--lines L1:L2]` |
| `git log -- <file>` | `trace history <file>` |
| `git log -L :<symbol>:<file>` | `trace history <file> <symbol>` |
| `git log -S <literal>` | `trace history --contains <pattern>` |
| `git diff --name-status` | `trace diff [--base <ref>] [--symbols]` |

Not covered, and each one reads code, so each is tracer work:

- **The commit body.** `history.rs:250-258` asks git for `%s` only, so every
  path returns a subject. `skills/understand/SKILL.md` step 3.2 prescribes
  `git show -s --format=full <commit>` because of this, which makes the one
  skill that teaches deep reading also teach the escape. `trace history`
  gains a commit mode returning body, author, parents, and the changed files
  with their shoulders.
- **The patch.** `git log -p` (487) and `git show <ref> -- <path>` (1,217)
  print a full patch; `trace read --at <ref> --diff` returns a symbol-level
  diff instead.
- **Regex pickaxe.** `--contains` is literal `-S`; `git log -G` (41) has no
  equivalent.
- **`git grep` at a ref.** `trace grep` searches the worktree only.
- **`git stash show -p`** (84). No stash surface exists.

Everything else the census found — `git status` porcelain, `git branch`,
`git tag`, `git rev-parse`, `git reflog`, `git stash list`, `git merge-base`,
`git describe`, `git for-each-ref`, `git worktree list`, `git ls-files`,
`git diff --stat`, `git diff --check`, `git cat-file -e/-t/-s` — returns no
code content. Tracer never grows those, and the ban never touches them.

## Decision 6 — nothing bounds a pipeline's memory

`rg -o '"(command|cmd)"[^\n]*'` over `~/.claude/projects` and
`~/.claude/sessions` emits 5.26 GB of text, because the extraction runs
unbounded to end of line and each transcript line is a whole entry. Piping
that into `sort` costs 4.56x the input in resident memory, measured: a 200 MB
slice peaks at 912 MB. Four such searches at once produced the 10 GB `sort`
processes.

`sort` already sits in `guard_trace.TRIMMERS`, but `TRIMMERS` fires only when
the upstream segment is `trace`, and the corpus sits outside the repository,
so neither guard rule saw the command. The rule to add: a pipe into `sort` or
`uniq` requires a bounded upstream — `rg -c`, `--max-count`, or a `head`
before the sort.

## Ordering

1. Decision 1 — independent, largest measured win, touches no contract.
2. Decisions 2 and 3 together; 3 is meaningless without 2.
3. Decision 4 after 2, since the envelope is what makes merged commands
   coherent.
4. Decisions 5 and 6 are independent of 1-4 and of each other. Decision 5
   must land before `guard_trace` bans any git form, or the ban blocks work
   with no replacement.

## What `logs.rs` already got right

`logs.rs` (2026-08-16, `739d9a6`) is the newest search command and already
holds the shape the others need: it collects `Vec<&Entry>` — borrowed, not
cloned (`:625`) — carries no per-entry enrichment in its payload
(`:634-639`), and groups by file in its human renderer (`:661-667`). The
pattern is not novel; `grep` and `struct` were simply never brought forward
to it.
