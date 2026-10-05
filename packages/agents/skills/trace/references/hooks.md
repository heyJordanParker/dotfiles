# trace Hooks

Use this Reference when diagnosing trace Hook behavior, docs injection, identity propagation, or session logs.

## 1. Identify the Hook surface

### Hooks are local Python files
The Hooks live under `packages/agents/hooks/` and are wired in `settings.json` by absolute `~/.agents/hooks/<module>.py` paths. Plugin Users get the binary, not the Hooks.

- Tracer's injecting Hooks keep their text under `lib/feedback.py`'s `CONTEXT_LIMIT`, 10,000 characters, because Claude Code saves a longer hook message to a file and shows the agent its path and a 2,000-character preview.
- `lib/tracer.py` passes each `trace` call the room its message has left as `--budget`.
- `load_trace_context.py`, `inject_docs.py`, `inject_rules.py`, and `enrich_on_read.py` declare `standalone`, so each runs in its own process.
- The quick checks bound to the same event and tool, `guard_trace.py` among them, run together in one `combine_hooks.py` process.

## 2. Match the Hook to the event

### `load_trace_context.py` loads the repo primer
- SessionStart matcher `startup|resume|clear|compact` runs `trace context` and injects the repo primer.

### `reload_harness_context.py` mirrors Claude Code's own doc loads
- It runs on Claude Code only.
- `InstructionsLoaded` runs `trace docs prime <file>`, so every doc Claude Code loads while the session runs is recorded as loaded.
- `PreCompact` and SessionStart `clear` reset the record.
- Every SessionStart records the session-start docs, which Claude Code puts back after a compaction without reporting them: `trace docs prime` for the `Claude.md` chain, and `trace docs <cwd> --json` for the working directory's docs.
- SessionStart `compact` sends back, from disk, the user Rules the closed window loaded, and records the ones sent whole; the first that does not fit is cut at a whole line with `trace read`'s marker, and the rest are named. Claude Code reloads project docs after a compaction but never a user Rule the session already loaded.

### `enrich_on_read.py` attaches facts to file operations
PreToolUse matcher `Read|Glob|Grep|Edit|Write` runs one `trace` call per tool, fitted to one 10,000-character hook message.

- Read runs `trace context <file>` with the Read's offset and limit; Edit and Write add `--no-record`.
- Grep runs `trace grep` with the native Grep's pattern, `-i`, `glob`, `type`, and `multiline`, so it names the same files.
- Glob runs `trace find` with the same pattern and path.
- A failed call on an existing file emits `[trace context unavailable: trace failed]` under its path, and a timed-out call emits `[trace context unavailable: enrichment timed out]`.

### `guard_trace.py` blocks lossy commands
PreToolUse matcher `Bash` blocks trace output piped to shell filters or redirected into a repository file, and raw file-search commands against in-repo paths. It whitelists `/tmp`, `/dev/null`, `docs/shaping/`, `docs/plans/`, `docs/agents/`, `.claude/shaping/`, `.claude/plans/`, and `.tracer-cache/`.

### `inject_docs.py` blocks trace without docs Context
- PreToolUse matcher `Bash` runs `trace docs <paths> --source inject_docs --triggering-tool Bash --triggering-command <cmd>` before a trace subcommand.
- The paths are every argument of the command that exists as a path, else the working directory; a `trace read` passes its files as `--skip`, because the read prints them. A `trace -C <dir>` call resolves its paths from `<dir>`.
- It injects the docs not yet loaded as Markdown, a doc too long for the message cut at a whole line and continued on the next trace command.
- Inside a Subagent it writes `--agent <agent_id>` into each `trace` call through `updatedInput`, because the Subagent's shell carries no agent id.
- It blocks the trace command with exit code 2 if docs loading fails.

### `inject_rules.py` gives Codex nearest Rules
- It runs on codex only, because Claude Code loads `Claude.md` itself.
- SessionStart resets the record on `clear`, records the root-to-cwd `Claude.md` chain, and sends the working directory's docs.
- `PreCompact` resets the record.
- PreToolUse matcher `Read|Write|Edit|apply_patch` sends the touched file's docs.
- It never blocks.

### `archive_subagent_log.py` preserves stopped Subagent logs
UserPromptSubmit parses Subagent completion notifications and moves `<repo>/.tracer-cache/sessions/<sid>/<aid>/` into `<repo>/.tracer-cache/sessions/<sid>/archived/<aid>/`. Trace reads fall back to the archived directory.

## 3. Preserve identity propagation

### Hooks pass identity on a local environment copy
- Every tracer Hook calls `trace` through `lib/tracer.py`, which sets `AGENT_SESSION_ID` and `TRACER_AGENT_ID` on the subprocess environment copy.
- `lib/tracer.py` runs `trace` from the event's `cwd`, so the session log lands in the event's repository. An agent's `trace -C <dir>` call writes the same log, because `-C` keeps the session log in the directory the call started in.
- No Hook mutates `os.environ`.

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
