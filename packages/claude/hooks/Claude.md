# Claude Code Hooks

## Why

Claude Code's hook system is the only place to enforce behavior across every session. The agent forgets, the model drifts, prompts decay — but a hook fires deterministically on every event. Hooks turn instructions into invariants.

The hooks are Python, shared with Codex. Their source of truth is `packages/agents/hooks/<module>.py` (stowed to `~/.agents/hooks/`), and the wiring invokes them by absolute path so one set of hooks serves both harnesses — see the cross-tool sharing section of the repo-root `Claude.md`. This directory holds only the plugin-distributed shell copies (below); the Python hooks live one package over.

The hooks' center of gravity is `lib/session_state.py` — a unified per-session state store every state-recording hook reads from or writes to. State scattered across inline `jq` calls in the wiring caused schema drift, race-window bugs, and made it impossible to add new tracked fields without touching every consumer. Centralizing it behind one module with a small public surface fixed the dependency mess and made new state cheap to add. One hook — `record_session_event.py` — owns every recording event and drives the spine in-process.

## What

Two layers live here, with different homes:

- **The live hooks Claude (and Codex) run** are Python under `packages/agents/hooks/`. Claude wires them in `settings.json` by absolute `~/.agents/hooks/<module>.py` path; Codex wires nearly all of them in its `config.toml` (all but the guards for Claude-only tools and events). This directory does **not** hold them.
- **This directory (`packages/claude/hooks/`)** holds only the small set of shell hooks the plugin marketplace still distributes to external installs that have no Python layer, plus the plugin wiring file (`hooks.json`) that references them, plus one third-party vendor script, plus this doc.

### Requirements

- Every hook's state read/write goes through the `session_state.py` spine (Python hooks via its `cmd_*` / `load_state` / `merge_state`), never inline `jq` or hand-rolled JSON. The one exception is the statusline — it is shell, runs on every render, and reads the main session's `state.json` directly at the deterministic path
- Every recording event routes through `record_session_event.py` — it reads the event JSON once, routes on `hook_event_name`, and drives the spine's `cmd_*` functions in-process. No per-event inline glue
- Every hook exits 0 on infrastructure failure (missing state file, parse error, missing tool) — a hook never blocks the agent because of the hook's own brokenness
- New fields added to the state schema also get `// 0` / `// null` defaults at every read site — older session files on disk must keep working
- Subagent sessions (`session_id` starting with `agent-`) get their own state files under their parent's `subagents/` directory; no global parent-state mutation from a subagent's hook
- Every persistent file lives under `~/.claude/` (overridable via `$CLAUDE_DATA_ROOT`); never `/tmp/` — sessions must survive reboot. Session control state (`approach`, `state`, `commit_requested`, `validation_phase`) and the session goal (`goal`, `requirements`, `boundaries`) live on the spine record alongside the telemetry — `classify_intent.py` writes the control state from typed mode-commands, `update_goal.py` writes the goal triplet, the plan transition writes `state` — read by the proposal/commit/solo guards, the completion validator, and the statusline
- Pick the mechanism by the decision's nature — a deterministic predicate over structured input (a flag, a path, an exit code) is code; a judgment over natural language or intent is an LLM call. Keyword and regex matching on prose is fragile and wrong. An LLM gate on what code can decide is nondeterminism and cost for nothing

### Boundaries

- Never call the LLM from a hook that fires on every event — model calls are reserved for `update_goal.py`, `validate_completion.py`, `validate_plan_quality.py`, and `validate_planning_docs.py`, all gated by structural pre-filters or rare events
- Never expose internal helpers (functions prefixed `_`) through the spine's main dispatch — they're implementation details
- Never count system-injected `UserPromptSubmit` events as human turns (skill expansions, task notifications, slash-command echoes) — the spine's `prompt` command filters via the documented structural predicate
- Never relocate or rewrite a session's `role` / `parent_session_id` after `_ensure_session` wrote them — heal corrupt JSON, but don't second-guess the parent linkage once set
- Never check "is this valid JSON" by truthiness — a valid JSON `null` / `false` is not the same as a parse failure; the spine distinguishes them explicitly

## Architecture

### Live hooks (Python — `packages/agents/hooks/`)

The hooks Claude and Codex run. Wired by absolute `~/.agents/hooks/<module>.py` path.

```
packages/agents/hooks/
├── lib/
│   ├── session_state.py    # unified per-session state store (the spine; cmd_*, load_state, merge_state)
│   ├── event.py            # event-payload parsing (read_event, field)
│   ├── command.py          # bash-command parsing for the Bash guards
│   ├── transcript.py       # Claude Code transcript layer
│   ├── model_call.py       # the single LLM-call helper the gate hooks share
│   └── codex_run.py        # runs codex as a named agent for the `codex-run` wrapper (stores output via the spine)
│
├── record_session_event.py # the one recording hook — routes every state-recording event to the spine
├── classify_intent.py      # UserPromptSubmit — typed mode-command → state/approach/commit (deterministic, no LLM)
├── update_goal.py          # UserPromptSubmit — LLM; maintains goal/requirements/boundaries on the spine
│
├── block_git_revert.py             # PreToolUse Bash
├── block_branch_change.py          # PreToolUse Bash — subagent-only (agent_id gate)
├── block_unsafe_delete.py          # PreToolUse Bash
├── block_path_assignment.py        # PreToolUse Bash — blocks bare assignment to zsh tied params (path/cdpath/fpath/manpath)
├── block_unauthorized_commits.py   # PreToolUse Bash
├── block_edits_during_proposal.py  # PreToolUse Write|Edit|MultiEdit|NotebookEdit + Bash — also blocks interpreter execution (python/node/bash/…) while proposing
├── block_builtin_subagents.py      # PreToolUse Agent
├── block_enter_worktree.py         # PreToolUse EnterWorktree
├── block_team_deletion.py          # PreToolUse TeamDelete
├── block_worktree_isolation.py     # PreToolUse Agent
├── protect_session_state.py        # PreToolUse Write|Edit|MultiEdit + Bash
├── guard_trace.py                  # PreToolUse Bash — force code reads through trace
│
├── enforce_background_codex_run.py # PreToolUse Bash — codex-run must run in background
├── enforce_solo_mode.py            # PreToolUse Agent + Bash — blocks the Agent tool, and codex/codex-run/claude, in solo
│
├── transition_state_after_plan.py  # PostToolUse ExitPlanMode
├── validate_completion.py          # Stop — LLM completion gate
├── validate_plan_quality.py        # PreToolUse ExitPlanMode — LLM
├── validate_planning_docs.py       # PreToolUse Write|Edit|MultiEdit — LLM
│
├── sync_shaping.py                 # PostToolUse Write|Edit
├── load_trace_context.py           # SessionStart — injects `trace context` primer as additionalContext
├── reload_harness_context.py       # SessionStart — runs `trace context prime` so the tracer log mirrors what the harness auto-loaded
├── enrich_on_read.py               # PreToolUse Read|Glob|Grep|Edit|Write — passive shoulder on each target; full `trace context` per match on Glob/Grep
├── inject_docs.py                  # PreToolUse Bash — injects project-docs for path-taking `trace` invocations; blocks the command if the docs load fails
├── inject_rules.py                 # SessionStart + PreToolUse Read|Write|Edit|apply_patch — Codex-only; injects the nearest Claude.md (Claude loads Claude.md itself)
├── auto_approve_permissions.py     # PermissionRequest — auto-allow every permission request
└── archive_subagent_log.py         # subagent-stop signal — moves the subagent's tracer log under archived/
```

### Plugin-distributed shell copies (this directory — `packages/claude/hooks/`)

The shell hooks the plugin marketplace ships to external installs, which have no Python layer. The plugin wiring file (`hooks.json`) references exactly these five; everything else here is the wiring file itself, this doc, and one third-party vendor script.

```
packages/claude/hooks/
├── hooks.json              # plugin-marketplace wiring (${CLAUDE_PLUGIN_ROOT} paths) — references the five below
├── block-git-revert.sh         # PreToolUse Bash
├── block-unsafe-delete.sh       # PreToolUse Bash
├── validate-planning-docs.sh    # PreToolUse Write|Edit|MultiEdit
├── validate-plan-quality.sh     # PreToolUse ExitPlanMode
├── sync-shaping.sh              # PostToolUse Write|Edit
├── herdr-agent-state.sh        # third-party vendor script (agent-state notifications); not ours, not plugin-distributed
└── Claude.md                   # this file
```

### State storage

```
~/.claude/                                 # data root ($CLAUDE_DATA_ROOT)
└── sessions/
    ├── <main_session_id>/
    │   ├── state.json                    # canonical state document
    │   ├── reads.jsonl                   # append-only file-read log
    │   ├── skills.jsonl                  # append-only skill-invocation log
    │   ├── transcript                    # path to Claude Code's JSONL transcript
    │   └── subagents/
    │       └── <agent_session_id>/       # nested subagent state
    │           └── state.json
    └── <another_main_session_id>/
```

The control fields (`approach`, `state`, `commit_requested`, `validation_phase`) and the session goal (`goal`, `requirements`, `boundaries`) live on the same `state.json` record — `classify_intent.py` writes the control state from typed mode-commands, `update_goal.py` writes the goal triplet, and `transition_state_after_plan.py` writes `state`, all through the spine's `merge_state`; the proposal, commit, and solo guards, the completion validator, and the statusline read them back. There is no separate control store; the spine owns control state and telemetry alike.

### `session_state.py` public surface

The spine dispatches these commands (used in-process by `record_session_event.py` and the gate hooks, and runnable directly as `python3 lib/session_state.py <command> [args...]`):

```
start <session_id> [--transcript-path <path>]    open/heal session, record session_start
end <session_id>                                  rm -rf, cascades subagents

get <session_id> <field>                          read field (soft on missing)
get --path [target]                               resolve a path: <session_id>, data-root, sessions, shaping, or the current session (AGENT_SESSION_ID / CODEX_THREAD_ID / CLAUDE_CODE_SESSION_ID, in that order)

set <session_id> <field> <value>                  atomic single-field
merge <session_id> <json_fragment>                atomic multi-field

prompt <session_id>                               reads stdin; if human, rotates turn timestamps + bumps human_turns
stopped <session_id>                              records last_stop (last-write-wins)
tool-used <session_id>                            atomic ++ on tools_used
read <session_id> <file_path>                     append {path, ts} to reads.jsonl
skill <session_id> <skill_name>                   append {skill, ts} to skills.jsonl
compacted <session_id>                            atomically truncate reads.jsonl and skills.jsonl after compaction

find-by-pane <pane_id>                            default zellij; --tmux for .tmux-pane
list                                              main sessions; --subagents <id> for nested

stats <session_id>                                JSON snapshot: durations + counters
is-long-running <session_id> [thresholds]         gate; defaults --turns 5 --seconds 600 --tools 30
```

### State schema

`state.json` for a main session:

```
session_id           string         the session's UUID (or agent-<hex> for subagents)
role                 main|subagent
parent_session_id    string|null    only set for subagents
approach             string         solo | subagents | team — set by classify_intent from typed commands
state                string         proposing | executing | auto — set by classify_intent / plan transition
goal                 string|null    one-paragraph session goal — set by update_goal
requirements         array          what the work must do — set by update_goal (capped at 10)
boundaries           array          what the work must never do — set by update_goal (capped at 10)
commit_requested     boolean
validation_phase     int            counter for validate_completion's max-3 block rule
pane                 string|null    zellij pane id
tmux-pane            string|null    tmux pane address (transitional during zellij migration)
session_start        int|null       epoch seconds; set once on first `start`
human_turns          int            count of real human prompts
current_turn_start   int|null       epoch seconds when in-progress turn opened
previous_turn_start  int|null       epoch seconds when last completed turn opened
last_stop            int|null       epoch seconds of most recent Stop event
tools_used           int            count of tool invocations
schema_version       int            currently 1
```

Subagent state files omit the control + goal fields (`approach`, `state`, `goal`, `requirements`, `boundaries`, `commit_requested`, `validation_phase`).

## Workflow

### Recording: one hook, in-process

`record_session_event.py` is wired on every state-recording event and routes each to the spine:

- **SessionStart** → `start` (plain session id, passes the transcript path through)
- **UserPromptSubmit** → `prompt` (plain session id, prompt text on the spine's stdin) — and, when the prompt is a completed `<task-notification>`, `stopped agent-<task-id>` plus an archive of that subagent's tracer log (the one subprocess, reused from `archive_subagent_log.py`)
- **Stop** → `stopped` (plain session id)
- **PostCompact** → `compacted` (plain session id)
- **PostToolUse** → `tool-used` (agent-rewritten session id)
- **PreToolUse Read** → `read` (agent-rewritten session id, file path; skip if none)
- **PreToolUse Skill** → `skill` (agent-rewritten session id, skill name; skip if none)

The hook spawns no subprocess on the hot path — it imports the spine's `cmd_*` functions and calls them directly, so PostToolUse (once per tool call) stays in-process. It returns 0 on every path, including malformed/empty payloads and a missing session id.

The agent-id rewrite: when the event carries `agent_id`, the per-tool-call recordings and the task-notification path record against `agent-<agent_id>` so a subagent's events nest under its parent. No `agent_id` → the plain `session_id`. No session id at all → do nothing. `start` / `prompt` / `stopped` / `compacted` always record against the plain session id.

This routes subagent inner tool calls into `<sessions>/<parent>/subagents/agent-<agent_id>/`. The spine's `_resolve_parent_id` then globs Claude Code's transcript layout to populate `parent_session_id` correctly.

### Other lifecycle wiring

- **SessionStart** — `record_session_event.py` runs the spine's `start`, creating the session record with control defaults; `load_trace_context.py` and `reload_harness_context.py` (matcher `startup|resume|clear|compact`) inject the tracer primer and mirror the harness auto-loads into the tracer log
- **Subagent stop** (no native event; signalled by the `<task-notification>` parse on UserPromptSubmit) — `record_session_event.py` records the stop and calls `archive_subagent_log.main([session_id, task_id])` to move the subagent's tracer log from `<repo>/.tracer-cache/sessions/<sid>/<aid>/` to `<repo>/.tracer-cache/sessions/<sid>/archived/<aid>/`. The archive resolves `<repo>` from its inherited cwd via `git -C "$PWD" rev-parse --show-toplevel` and silently no-ops when cwd is not inside a git repo. The tracer's read path falls back to the archived directory when the active one is missing, so queries against the stopped subagent's log keep working

`SessionEnd` is intentionally not wired for state cleanup — sessions can be resumed after terminal close, and state must survive reboot. `end` is a manual deletion command, not a lifecycle hook target.

In Claude Code 2.1.131: `SessionStart` does not fire for subagent sessions; `SubagentStart`/`SubagentStop`/`TaskCreated`/`TaskCompleted` did not appear in any of the 138 debug logs surveyed. Subagent identity reaches hooks only via `agent_id`/`agent_type` payload fields on `PreToolUse`/`PostToolUse` — `session_id` always carries the parent's UUID. The spine's full subagent infrastructure is in place: `record_session_event.py` constructs `agent-<agent_id>` from the payload before driving the spine.

### Running the test suite

The pytest suite under `tests/hooks/` exercises the Python hooks directly, in-process:

```sh
python3 -m pytest tests/hooks/
```

`test_session_state.py` guards the spine's properties (atomic concurrent increments, corrupt/empty/missing-state healing, subagent nesting and resolution, the append-only event logs and their truncation on compaction). `test_record_session_event.py` covers the recording router; `test_local_llm_fallbacks.py` pins the deterministic fallbacks the model-backed hooks take when the local LLM returns no verdict, and `test_enrich_on_read.py` pins which tools `enrich_on_read.py` enriches and over which files; `test_model_call.py`, `test_transcript.py`, and `test_proposal_guard_redirects.py` cover the shared spine and guards. Every test runs against a per-test `CLAUDE_DATA_ROOT` / `CLAUDE_PROJECTS_ROOT` under `tmp_path` — nothing touches the real `~/.claude/`. Time is driven by monkeypatching the spine's two clock functions; concurrency tests use threads.

### Distribution

The live Python hooks live in `packages/agents/hooks/` and are wired locally for Claude in `settings.json` (by absolute `~/.agents/hooks/<module>.py` path) and for Codex in its `config.toml`. They are never plugin-distributed.

The plugin marketplace distributes the shell copies in this directory. Two facts make this a small, fixed set:

- `hooks.json` is the plugin wiring (`${CLAUDE_PLUGIN_ROOT}` paths). It references exactly five hooks: `block-git-revert.sh`, `block-unsafe-delete.sh`, `validate-planning-docs.sh`, `validate-plan-quality.sh`, `sync-shaping.sh` — the universally-safe subset that needs no Python layer or local workflow assumptions.
- Tracer hooks, subagent-only hooks, the intent classifier and the state guards that depend on it, and the Claude-only-tool guards (Agent / EnterWorktree / TeamDelete / ExitPlanMode) are all local-only and never plugin-distributed. Plugin users get the tracer binary as a command, not its hooks.

When changing a plugin-distributed hook, update both the Python source (`packages/agents/hooks/<module>.py`) and its shell copy here, and keep `hooks.json` in sync. When changing a local-only hook, only the Python source and the `settings.json` / `config.toml` wiring matter.

## How

### Adding a new tracked field to the state schema

1. Add the field to `_default_main_state` and (if applicable) `_default_subagent_state` in `lib/session_state.py`. Pick a sensible initial value (`null` for nullable scalars, `0` for counters, `[]` for lists).
2. Add a write path — either reuse `set`/`merge`/`_bump`, or add an event command (`cmd_*`) and route it through `_ensure_session` then `_atomic_write`.
3. At every read site (including external hooks), default the field (`.get("field") or <default>`) so older session files on disk that predate the field stay readable.
4. If the field is part of the `stats` snapshot, add it to `_stats_obj`.
5. Add tests covering: the field's initial value, the write path under sequential and concurrent invocation, the read fallback for legacy state files.

### Adding a new event command

1. Add `cmd_<name>` in `lib/session_state.py`. Validate `session_id` first; route through `_ensure_session` to lazy-create + heal.
2. Add a dispatch entry in `_DISPATCH`.
3. If the event reads content from stdin, read it inside the command (matching `cmd_prompt`).
4. Add tests; for time-dependent behavior, monkeypatch the spine's clock functions (the `clock` fixture in `test_session_state.py`).
5. Document the new command in this file's surface table.

### Adding a new structural rule to `_human_prompt`

Only add a rule that's backed by a real transcript shape observed in `~/.claude/projects/`. Add the rule, add a positive test (system-injected → filtered) and a negative test (similar-looking human input → counted). Update the comment in `_human_prompt`.

### Heal vs clobber

- Reads (`get`, `find-by-pane`, `list`) are read-only — they never trigger heal
- Writes (`start`, `set`, `merge`, `prompt`, `stopped`, `tool-used`, `read`, `skill`) route through `_ensure_session`, which heals corrupt/missing/empty `state.json` to defaults before applying the operation
- `_ensure_session` does NOT relocate sessions or rewrite role/parent linkage — once those are set on disk, they're immutable for the session's lifetime

### Race semantics

- `_atomic_write` uses `mkstemp + os.replace` — atomic on the same filesystem; concurrent writers see either the old or new state, never a half-written file
- All read-modify-write paths (`start`, `set`, `merge`, `prompt`, `stopped`, `tool-used`/`_bump`) acquire a per-state-file mutex via `_with_lock` (atomic `mkdir` of `<state.json>.lock`, 5s timeout). Concurrent invocations serialize through the lock; no increments are lost. The test `concurrent tool-used loses no increment` pins this
- Append (`read`, `skill`) uses an append open — POSIX guarantees atomicity for writes under PIPE_BUF (4KB); JSONL entries are well below that, no lock needed

### `agent-*` session resolution

Subagent session IDs start with `agent-`. Lazy-create paths (`set`, `merge`, `prompt`, `stopped`, `tool-used`, `read`, `skill`) need to know the parent to nest correctly. They glob `$CLAUDE_PROJECTS_ROOT/*/*/subagents/agent-<id>.jsonl` and extract the parent UUID from the path. Claude Code writes the transcript before any hook fires, so the glob succeeds in production. If the transcript is missing (test environment, manual invocation), `_ensure_session` fails loud rather than landing the subagent at flat top-level.

### Compaction

Claude Code compacts long conversations server-side, summarizing earlier turns when context approaches the limit. After compaction, the agent no longer "has" the pre-compaction history — but `reads.jsonl` and `skills.jsonl` would still show those old events. The PostCompact recording calls the spine's `compacted` command, which atomically truncates both logs to zero bytes via `_truncate` (mkstemp + replace). State.json fields (`human_turns`, `tools_used`, turn timestamps) are NOT reset — only the append-only event logs that downstream hooks use to gate "events since the agent's effective memory started."

### Tracer log lifecycle

Two hooks own the tracer session log's lifecycle at the harness boundary, separate from the `session_state.py` state document. The session-context store lives at `<repo>/.tracer-cache/sessions/` (same root as the per-file and architecture namespaces); both hooks resolve `<repo>` from their inherited cwd. `reload_harness_context.py` runs on SessionStart matcher `startup|resume|clear|compact` and re-runs `trace context prime --reason {post_compact|session_start}` (picked from the matcher value in the payload's `source` field) so the log mirrors what the harness auto-loaded at session start and after compaction alike, keeping the log's "already in context" set in sync with what the agent actually has. `archive_subagent_log.py` runs on subagent stop (signalled by the `<task-notification>` parse on UserPromptSubmit, driven by `record_session_event.py`) and moves the subagent's per-agent directory from `<repo>/.tracer-cache/sessions/<sid>/<aid>/` to `<repo>/.tracer-cache/sessions/<sid>/archived/<aid>/`, with `<repo>` resolved via `git -C "$PWD" rev-parse --show-toplevel` (silent no-op when no repo root resolves). The tracer's read path falls back to `archived/<aid>/` when the active dir is missing, so post-stop log queries keep returning the same data — the archive is a directory rename, not a destructive operation.
