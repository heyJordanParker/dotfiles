# trace Hooks

Use this Reference when diagnosing trace Hook behavior, docs injection, identity propagation, or session logs.

## 1. Identify the Hook surface

### Hooks are local Python files
The Hooks live under `packages/agents/hooks/` and are wired in `settings.json` by absolute `~/.agents/hooks/<module>.py` paths. Plugin Users get the binary, not the Hooks.

## 2. Match the Hook to the event

### `load_trace_context.py` loads the repo primer
SessionStart matcher `startup|resume|clear|compact` runs `trace context` and injects the eight-section repo primer.

### `reload_harness_context.py` mirrors Harness auto-loads
SessionStart matcher `startup|resume|clear|compact` runs `trace context prime --reason session_start|post_compact`; compact maps to `post_compact`, all other starts map to `session_start`.

### `enrich_on_read.py` attaches shoulders to file operations
PreToolUse matcher `Read|Glob|Grep|Edit|Write` runs `trace context <file>`. Edit and Write pass `--no-record` because an edit is not a read. Glob and Grep resolve matched files and cap enrichment at twenty files.

### `guard_trace.py` blocks lossy commands
PreToolUse matcher `Bash` blocks trace output piped to shell filters or redirected into a repository file, and raw file-search commands against in-repo paths. It whitelists `/tmp`, `/dev/null`, `docs/shaping/`, `docs/plans/`, `.claude/shaping/`, `.claude/plans/`, and `.tracer-cache/`.

### `inject_docs.py` blocks trace without docs Context
PreToolUse matcher `Bash` runs `trace docs <path> --source trace_inject_hook --triggering-tool Bash --triggering-command <cmd>` before path-taking trace subcommands. It blocks the trace command with exit code 2 if docs loading fails.

### `inject_rules.py` gives Codex nearest Rules
SessionStart and PreToolUse matcher `Read|Write|Edit|apply_patch` is Codex-only because Claude Code loads `Claude.md` itself. It injects nearest `Claude.md` through `trace docs`, resets docs on `clear` and `compact`, and never blocks.

### `archive_subagent_log.py` preserves stopped Subagent logs
UserPromptSubmit parses Subagent completion notifications and moves `<repo>/.tracer-cache/sessions/<sid>/<aid>/` into `<repo>/.tracer-cache/sessions/<sid>/archived/<aid>/`. Trace reads fall back to the archived directory.

## 3. Preserve identity propagation

### Hooks pass identity on a local environment copy
`inject_docs.py`, `inject_rules.py`, `enrich_on_read.py`, and `reload_harness_context.py` set `AGENT_SESSION_ID` and `TRACER_AGENT_ID` on the subprocess environment copy passed to `trace`; they do not mutate `os.environ`.

### `AGENT_SESSION_ID` is the Harness-neutral carrier
Trace resolves `AGENT_SESSION_ID` first for the session log. `CLAUDE_CODE_SESSION_ID` remains untouched so nested Codex runs can still resolve governing session state through `owner_session`.

### Missing session id makes the log a no-op
Without a session id, docs dedupe and read coverage cannot work.

## 4. Read the environment variables

### Session identity variables are ordered
`trace` resolves `AGENT_SESSION_ID`, then `CODEX_THREAD_ID`, then `CLAUDE_CODE_SESSION_ID`.

### Agent identity defaults to root
`TRACER_AGENT_ID` identifies the Agent within the session and defaults to `root`.

### Trigger variables stamp log events
`TRACER_TRIGGERING_TOOL` and `TRACER_TRIGGERING_COMMAND` are written on log events. `trace docs` sets them from `--triggering-tool` and `--triggering-command`.

### Binary and complexity backend variables are explicit
`TRACE_BIN` overrides the binary path used by the plugin launcher. `TRACER_CCN_BACKEND` selects the complexity backend; the AST tree-sitter walker is the only supported backend.
