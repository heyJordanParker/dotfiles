# Claude Code Hooks
v1.5 | Updated: 2026-05-14

## Why

Claude Code's hook system is the only place to enforce behavior across every session. The agent forgets, the model drifts, prompts decay — but a hook fires deterministically on every event. Hooks turn instructions into invariants.

The directory's center of gravity is `session-state.sh` — a unified per-session state helper that every other hook reads from or writes to. State scattered across inline jq calls in 8 different hooks caused schema drift, race-window bugs, and made it impossible to add new tracked fields without touching every consumer. Centralizing it behind one helper script with a small public surface fixed the dependency mess and made new state cheap to add.

## What

This directory contains the safety, enforcement, and state-tracking hooks wired into Claude Code via `settings.json` and `hooks.json`. The session-state helper underpins all of them.

### Requirements

- Every state read/write goes through `session-state.sh` — never inline `jq` against `~/.claude/sessions/*/state.json` from a hook
- Every hook in this directory exits 0 on infrastructure failure (missing state file, parse error, missing tool) — a hook never blocks the agent because of the hook's own brokenness
- New fields added to the state schema also get `// 0` / `// null` defaults at every read site — older session files on disk must keep working
- Subagent sessions (`session_id` starting with `agent-`) get their own state files under their parent's `subagents/` directory; no global parent-state mutation from a subagent's hook
- Every persistent file lives under `~/.claude/` (overridable via `$CLAUDE_DATA_ROOT`); never `/tmp/` — sessions must survive reboot
- Helper hook invocations always redirect stderr to `/tmp/session-state-hook.log` (never `/dev/null`) — silent failures hide the cause

### Boundaries

- Never call `claude -p` (the LLM) from a hook that fires on every event — LLM calls are reserved for `classify-intent.sh`, `validate-completion.sh`, `validate-plan-quality.sh`, and `validate-planning-docs.sh`, all gated by structural pre-filters or rare events
- Never expose internal helpers (functions prefixed `_`) through the helper's main dispatch — they're implementation details
- Never count system-injected `UserPromptSubmit` events as human turns (skill expansions, task notifications, slash-command echoes) — use `session-state prompt` which filters via the documented structural predicate
- Never relocate or rewrite a session's `role` / `parent_session_id` after `_ensure_session` wrote them — heal corrupt JSON, but don't second-guess the parent linkage once set
- Never use `jq -e .` to detect "is this valid JSON" — `jq -e` returns 1 for valid JSON `null` / `false`; use `jq empty` or `jq -e 'type == "object"'`

## Architecture

```
hooks/
├── session-state.sh        # unified session-state helper (the API)
├── session-state-test.sh   # 500+ test harness for the helper
├── hooks.json              # plugin-distributed wiring (mirrors settings.json)
│
├── classify-intent.sh      # UserPromptSubmit — LLM intent classifier
├── classify-intent-test.sh
│
├── auto-approve-permissions.sh           # PermissionRequest matcher
├── block-builtin-subagents.sh            # PreToolUse Agent matcher
├── block-edits-during-proposal.sh        # PreToolUse Write|Edit|NotebookEdit + Bash matchers
├── block-git-revert.sh                   # PreToolUse Bash matcher
├── block-team-deletion.sh                # PreToolUse TeamDelete matcher
├── block-unauthorized-commits.sh         # PreToolUse Bash matcher
├── block-unsafe-delete.sh                # PreToolUse Bash matcher
│
├── enforce-background-agents.sh          # PreToolUse Agent matcher
├── enforce-solo-mode.sh                  # PreToolUse Agent matcher
├── protect-session-state.sh              # PreToolUse Write|Edit|Bash
│
├── transition-state-after-plan.sh        # PostToolUse ExitPlanMode
├── validate-completion.sh                # Stop — LLM completion gate
├── validate-plan-quality.sh              # PreToolUse ExitPlanMode — LLM
├── validate-planning-docs.sh             # PreToolUse Write|Edit — LLM
├── validate-ledger-entries.sh            # PreToolUse Write|Edit *.md
│
├── sync-shaping.sh                       # PostToolUse Write|Edit
├── load-trace-context.sh                 # SessionStart — injects `trace context` primer as additionalContext
└── initialize-session-state.sh           # legacy — superseded by session-state start
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

### `session-state.sh` public API

```
start <session_id> [--transcript-path <path>]    open/heal session, record session_start
end <session_id>                                  rm -rf, cascades subagents

get <session_id> <field>                          read field (soft on missing)
get --path [target]                               resolve a path: <session_id>, data-root, sessions, shaping, or current $CLAUDE_SESSION_ID

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
approach             string         solo | subagents | team — set by classify-intent
state                string         proposing | executing | auto — state machine
intent               string         most recent classified intent
commit_requested     boolean
notes                array          surprise/correction notes from classify-intent
validation_phase     int            counter for validate-completion's max-3 block rule
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

Subagent state files omit the intent-classifier fields (`approach`, `state`, `intent`, `commit_requested`, `notes`, `validation_phase`).

## Workflow

### Hook → command wiring

- **SessionStart** — `session-state start "$SESSION_ID" --transcript-path "$TRANSCRIPT"`
- **UserPromptSubmit** — `printf '%s' "$PROMPT" | session-state prompt "$SESSION_ID"`
- **Stop** — `session-state stopped "$SESSION_ID"`
- **PreToolUse `"*"`** — `session-state tool-used "$SESSION_ID"` (literal asterisk; PostToolUse omit-matcher does not fire reliably in 2.1.131)
- **PreToolUse Read** — `session-state read "$SESSION_ID" "$FILE_PATH"`
- **PreToolUse Skill** — `session-state skill "$SESSION_ID" "$SKILL_NAME"`
- **PostCompact** (matchers `manual|auto`) — `session-state compacted "$SESSION_ID"`

For per-tool-call events, the wiring overwrites `SESSION_ID` with `agent-<agent_id>` when the payload carries `agent_id` (event fires inside a subagent's execution context), otherwise leaves it as the parent's `session_id`:

```bash
SESSION_ID=$(echo "$INPUT" | jq -r '.session_id')
AGENT_ID=$(echo "$INPUT" | jq -r '.agent_id // empty')
[ -n "$AGENT_ID" ] && SESSION_ID="agent-$AGENT_ID"
```

This routes subagent inner tool calls into `<sessions>/<parent>/subagents/agent-<agent_id>/`. The helper's `_resolve_parent_id` then globs Claude Code's transcript layout to populate `parent_session_id` correctly.

`SessionEnd` is intentionally not wired — sessions can be resumed after terminal close, and state must survive reboot. `end` is a manual deletion command, not a lifecycle hook target.

In Claude Code 2.1.131: `SessionStart` does not fire for subagent sessions; `SubagentStart`/`SubagentStop`/`TaskCreated`/`TaskCompleted` did not appear in any of the 138 debug logs surveyed. Subagent identity reaches hooks only via `agent_id`/`agent_type` payload fields on `PreToolUse`/`PostToolUse` — `session_id` always carries the parent's UUID. The helper's full subagent infrastructure remains in place: it's the wiring's responsibility to construct `agent-<agent_id>` from the payload before calling the helper.

### Running the test suite

```sh
bash session-state-test.sh
```

Runs ~500 tests in under 60 seconds. Tests isolate `CLAUDE_DATA_ROOT` and `CLAUDE_PROJECTS_ROOT` to a `mktemp -d` per-test; nothing touches the user's real `~/.claude/`. Concurrency tests fork bash subshells; mocked-time tests override `date` via PATH.

### Distribution

Hook scripts live here in dotfiles. Two wiring files reference them:

- `settings.json` — local-use wiring via stow (includes tmux integration)
- `hooks.json` — plugin-marketplace wiring with `${CLAUDE_PLUGIN_ROOT}` paths

When adding or modifying a non-tmux hook, update both.

## How

### Adding a new tracked field to the state schema

1. Add the field to `_default_main_state` and (if applicable) `_default_subagent_state` in `session-state.sh`. Pick a sensible initial value (`null` for nullable scalars, `0` for counters, `[]` for lists).
2. Add a write path — either reuse `set`/`merge`/`_bump`, or add an event command (`cmd_*`) and route it through `_ensure_session` then `_atomic_write`.
3. At every read site (including external hooks), use `(.field // <default>)` so older session files on disk that predate the field stay readable.
4. If the field is part of the `stats` snapshot, add it to `cmd_stats`'s jq filter.
5. Add tests covering: the field's initial value, the write path under sequential and concurrent invocation, the read fallback for legacy state files.

### Adding a new event command

1. Add `cmd_<name>` in `session-state.sh`. Validate `session_id` first; route through `_ensure_session` to lazy-create + heal.
2. Add a dispatch entry in the `case "$cmd"` block at the bottom.
3. If the event requires content from stdin, capture it with `content=$(cat)` and pass to internal predicates explicitly via `printf '%s' "$content" | _internal_helper`.
4. Add tests; for time-dependent behavior, mock `date` via PATH override (the `mock_now` pattern in `session-state-test.sh`).
5. Document the new command in this file's API surface table.

### Adding a new structural rule to `_human_prompt`

Only add a rule that's backed by a real transcript shape observed in `~/.claude/projects/`. Add the rule, add a positive test (system-injected → returns 1) and a negative test (similar-looking human input → returns 0). Update the comment in `_human_prompt`.

### Heal vs clobber

- Reads (`get`, `find-by-pane`, `list`) are read-only — they never trigger heal
- Writes (`start`, `set`, `merge`, `prompt`, `stopped`, `tool-used`, `read`, `skill`) route through `_ensure_session`, which heals corrupt/missing/empty `state.json` to defaults before applying the operation
- `_ensure_session` does NOT relocate sessions or rewrite role/parent linkage — once those are set on disk, they're immutable for the session's lifetime

### Race semantics

- `_atomic_write` uses `mktemp + mv` — atomic on the same filesystem; concurrent writers see either the old or new state, never a half-written file
- All read-modify-write paths (`start`, `set`, `merge`, `prompt`, `stopped`, `tool-used`/`_bump`) acquire a per-state-file mutex via `_with_lock` (atomic `mkdir` of `<state.json>.lock`, 5s timeout). Concurrent invocations serialize through the lock; no increments are lost. Test `concurrent tool-used: 50 increments → tools_used=50 exactly` pins this
- Append (`read`, `skill`) uses `>> file` — POSIX guarantees atomicity for writes under PIPE_BUF (4KB); JSONL entries are well below that, no lock needed

### `agent-*` session resolution

Subagent session IDs start with `agent-`. Lazy-create paths (`set`, `merge`, `prompt`, `stopped`, `tool-used`, `read`, `skill`) need to know the parent to nest correctly. They glob `$CLAUDE_PROJECTS_ROOT/*/*/subagents/agent-<id>.jsonl` and extract the parent UUID from the path. Claude Code writes the transcript before any hook fires, so the glob succeeds in production. If the transcript is missing (test environment, manual invocation), `_ensure_session` fails loud rather than landing the subagent at flat top-level.

### Compaction

Claude Code compacts long conversations server-side, summarizing earlier turns when context approaches the limit. After compaction, the agent no longer "has" the pre-compaction history — but `reads.jsonl` and `skills.jsonl` would still show those old events. The PostCompact hook calls `session-state compacted <session_id>` which atomically truncates both logs to zero bytes via `_truncate` (mktemp + mv). State.json fields (`human_turns`, `tools_used`, turn timestamps) are NOT reset — only the append-only event logs that downstream hooks use to gate "events since the agent's effective memory started."

## Ledger

- v1.5: SessionStart injects trace context primer
- v1.4: Proposing-mode block extends to Bash file mutations
- v1.3: Lock RMW paths so concurrent counters land exactly
- v1.2: Wire helper into Claude Code hook events
- v1.1: Document compacted command and PostCompact hook
- v1.0: Document hooks dir centered on session-state helper
