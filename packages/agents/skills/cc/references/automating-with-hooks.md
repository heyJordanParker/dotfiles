# Hooks

Event-driven automation for Claude Code.

## Hook Types

- **Command** (`type: "command"`) — Bash scripts, deterministic
- **Prompt** (`type: "prompt"`) — Single LLM turn, no tools, context-aware
- **Agent** (`type: "agent"`) — Spawns a full subagent with tool access (Read, Grep, Glob, Bash, etc.)
- **HTTP** (`type: "http"`) — Delegates to an external HTTP service
- **MCP tool** (`type: "mcp_tool"`) — Invokes an MCP tool directly (v2.1.118+)

## Events

- **PreToolUse:** Validate/block/modify tool calls — matcher: tool names
- **PostToolUse:** React to results, logging — matcher: tool names
- **PostToolUseFailure:** Fires when a tool call fails — input carries `duration_ms` (v2.1.119+)
- **Stop:** Completeness check before agent stops
- **SubagentStart:** Fires when a subagent is dispatched — command-type only
- **SubagentStop:** Validate subagent task completion
- **TaskCompleted:** React to completed subagent tasks
- **UserPromptSubmit:** Add context, validate prompts
- **MessageDisplay:** Transform or hide assistant message text as it is displayed (v2.1.152+)
- **SessionStart:** Load context, set env vars — matcher: startup|resume|clear|compact
- **SessionEnd:** Cleanup
- **ConfigChange:** Fires when settings change (hot-reload)
- **Setup:** Repository setup/maintenance — trigger: --init, --init-only, --maintenance
- **StopFailure:** Fires when turn ends due to API error (rate limit, auth failure, etc.) — not the same as Stop
- **TaskCreated:** Fires when a task is created via TaskCreate
- **CwdChanged:** Fires when working directory changes — use for reactive environment management (e.g. direnv)
- **FileChanged:** Fires when a watched file changes
- **PreCompact:** Preserve critical context — matcher: manual|auto. Can block compaction by exiting 2 or returning `{"decision":"block"}` (v2.1.105+)
- **PostCompact:** React after compaction completes (e.g., log, notify, refresh state)
- **Notification:** React to user notifications — matcher: notification types
- **PermissionRequest:** Auto-allow/deny user-facing approval prompts (file access, tool confirmation, user interaction) — matcher: tool names
- **PermissionDenied:** Fires after an auto-mode classifier denial — return `{retry: true}` to tell the model it may retry (v2.1.89+)
- **Elicitation:** Intercept MCP elicitation requests before showing to user — matcher: MCP server names
- **ElicitationResult:** Override/modify elicitation responses before sending back to MCP server — matcher: MCP server names
- **WorktreeCreate:** Replace default git worktree behavior (e.g. for non-git VCS). Must return worktree path (stdout for command hooks, `hookSpecificOutput.worktreePath` for HTTP). `.worktreeinclude` NOT processed when custom hook is configured — copy config files in your hook script instead
- **WorktreeRemove:** Cleanup companion for WorktreeCreate

## Configuration

**Locations (precedence order):**
1. `~/.claude/settings.json` (user)
2. `.claude/settings.json` (project)
3. `.claude/settings.local.json` (local, not committed)
4. Plugin `hooks/hooks.json`
5. Component frontmatter (skills, agents)

**Settings format:**
```json
{
  "hooks": {
    "PreToolUse": [{
      "matcher": "Write|Edit",
      "hooks": [{ "type": "command", "command": "./validate.sh" }]
    }]
  }
}
```

## Hook Options

- `once: true` — run hook only once per session
- `args: [...]` — exec form for command hooks: spawns the command directly without a shell, so path placeholders never need quoting (v2.1.139+)
- `continueOnBlock: true` — `PostToolUse` only: feed the hook's rejection reason back to Claude and continue the turn instead of erroring (v2.1.139+)
- `if: "ToolName(pattern)"` — only spawn this handler when the tool call matches the pattern. Uses permission rule syntax (gitignore-style globs for file paths). Avoids process overhead for non-matching calls. Requires v2.1.85+

**`if` patterns:**
- Bash: `"if": "Bash(git *)"` — matches commands starting with `git`
- File tools: `"if": "Edit(**/Claude.md)"` — matches Claude.md at any depth
- Path prefixes: `//` (filesystem root), `~/` (home), `/` (project root), bare (cwd-relative)
- `*` matches within one directory, `**` matches recursively
- No pipe alternation inside `if` — use separate handler entries for Write vs Edit

```json
{
  "matcher": "Write|Edit",
  "hooks": [
    { "type": "command", "if": "Write(**/Claude.md)", "command": "./validate.sh" },
    { "type": "command", "if": "Edit(**/Claude.md)", "command": "./validate.sh" }
  ]
}
```

## Frontmatter Hooks

Skills and agents can define scoped hooks in frontmatter:

```yaml
---
hooks:
  PreToolUse:
    - matcher: "Write"
      hooks: [{ type: "command", command: "./validate.sh" }]
---
```

## Matchers

- `"Write"` — exact tool
- `"Write|Edit"` — multiple tools (canonical alternation); comma-separated (`"Bash,PowerShell"`) also fires as of v2.1.191
- `"*"` — all tools (use the literal asterisk — observed in 2.1.131: omitting the matcher field results in the hook not firing reliably across tools, even though the doc treats omit as equivalent to `"*"`)
- `"mcp__.*"` — regex pattern

**Hyphenated identifiers exact-match (v2.1.195+).** A matcher like `code-reviewer` or `mcp__brave-search` no longer substring-matches — it matches that exact name only. To match all tools from a hyphenated MCP server, use the regex form `mcp__brave-search__.*`.

**Case-sensitive.** Use `/hooks` to verify tool names.

## Exit Codes

- **0:** Success (stdout processed)
- **2:** Block (stderr shown to Claude/user)
- **Other:** Non-blocking error

**Event-specific behavior:**
- **PreToolUse:** exit 2 blocks the tool call. Stderr shown to agent. Can also satisfy `AskUserQuestion` by returning `updatedInput` alongside `permissionDecision: "allow"` — enables headless integrations that collect answers via their own UI. A `"defer"` permission decision pauses a headless session at the tool call so `-p --resume` re-evaluates the hook (v2.1.89+)
- **PreToolUse security note:** hooks returning `"allow"` do NOT bypass `deny` permission rules (including enterprise managed settings). Deny rules always win — a `deny` rule also overrides a hook's `permissionDecision: "ask"` rather than being downgraded to a prompt (v2.1.101+)
- **UserPromptSubmit:** exit 2 blocks message submission, stderr shown to user. Any non-zero exit also blocks (not graceful like other events)
- **Stop:** exit 2 blocks the stop. Stderr becomes instructions to the agent, which acts on them and tries to stop again (re-triggering the hook). Can fire multiple times per turn — the turn ends with a warning after 8 consecutive blocks (override with `CLAUDE_CODE_STOP_HOOK_BLOCK_CAP`) (v2.1.143+)

## Event Input Schemas

Each hook event receives a JSON object on stdin. Fields vary by event. All events carry `effort.level` (the active effort level), also exposed to command hooks and Bash tool commands as `$CLAUDE_EFFORT` (v2.1.133+).

**PreToolUse / PostToolUse:**
- `session_id` — session UUID. In Claude Code 2.1.131 this is the parent session's UUID even when the tool runs inside a subagent — verified by directly reading hook payloads from `/tmp/payload-trace.log` during a controlled subagent dispatch. Subagent identity is NOT carried in `session_id`; see `agent_id` / `agent_type` below.
- `agent_id` — present only when the event fires inside a subagent's execution (e.g. `"a3b88a27b1667264e"`). Absent for parent's own tool calls.
- `agent_type` — present alongside `agent_id`; the dispatched subagent type (e.g. `"researcher"`).
- `tool_name` — the tool being called (e.g. `"Write"`, `"Bash"`, `"Agent"`)
- `tool_input` — tool-specific parameters (e.g. `{file_path, content}` for Write, `{command}` for Bash, `{prompt, run_in_background}` for Agent)
- `cwd` — working directory
- `duration_ms` — PostToolUse / PostToolUseFailure only: tool execution time, excluding permission prompts and PreToolUse hooks (v2.1.119+)

**To route per-tool-call events per-subagent**, overwrite `SESSION_ID` with `agent-<agent_id>` when the payload carries it:
```bash
SESSION_ID=$(echo "$INPUT" | jq -r '.session_id')
AGENT_ID=$(echo "$INPUT" | jq -r '.agent_id // empty')
[ -n "$AGENT_ID" ] && SESSION_ID="agent-$AGENT_ID"
```

**UserPromptSubmit:**
- `session_id` — session UUID
- `prompt` — the user's message text
- `transcript_path` — path to the session's JSONL file

**Caveat:** System-generated messages (background agent completions, skill content injections, interrupt notifications) enter conversations as `type: "user"` and trigger UserPromptSubmit. Guard against these structurally — see [Skip System-Generated Messages](#skip-system-generated-messages) pattern.

**Stop:**
- `session_id` — session UUID
- `transcript_path` — path to the session's JSONL file
- `cwd` — working directory
- `last_assistant_message` — the agent's final text before stopping
- `stop_hook_active` — boolean (behavior under investigation)
- `permission_mode` — e.g. `"default"`, `"bypassPermissions"`
- `hook_event_name` — `"Stop"`
- `background_tasks` — running background tasks; `session_crons` — scheduled crons (Stop / SubagentStop, v2.1.145+)

**PermissionRequest:**
- `session_id` — session UUID
- `tool_name` — the tool that triggered the approval prompt (file tools, Bash, AskUserQuestion, ExitPlanMode, etc.)

### Transcript Path

`transcript_path` points to project JSONL files at `~/.claude/projects/<project-path>/<session-uuid>.jsonl`. These are NOT the files in `~/.claude/transcripts/`.

JSONL entry types: `"assistant"`, `"user"`, `"system"`, `"file-history-snapshot"`, `"queue-operation"`

`"type":"user"` entries include both real user messages AND tool results. To filter for real user messages only, exclude entries containing `tool_use_id`.

Not every `type: "user"` transcript entry fires a UserPromptSubmit hook. The transcript stores message format; the hook system decides independently when to fire.

## Hook Output Format

**UserPromptSubmit** hooks inject context into the agent by outputting JSON to stdout:

```json
{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":"text injected as system-reminder"}}
```

The `additionalContext` string appears in the agent's context before it processes the user's message.

**PreToolUse** hooks can also return `additionalContext` to inject context.

**A hook cannot make the harness invoke a `disable-model-invocation` skill.** Injecting `Load /<skill> now via the Skill tool` as `additionalContext` routes the model to the Skill tool, which hits the same gate the user bypasses by literally typing `/<skill>` — it refuses with `Skill <name> cannot be used with Skill tool due to disable-model-invocation` and the directive dead-ends. The trick works for normal skills (no gate); it silently fails for `disable-model-invocation` ones. A hook's only way to get such a skill's content into context is to inline the skill body itself as plain `additionalContext` — which carries none of the skill machinery (no `allowed-tools`, no `references/`, no `!`-autorun, no skill registration). See `building-skills.md` for the `disable-model-invocation` flag.

Other `hookSpecificOutput` fields by event:
- **Stop / SubagentStop:** `additionalContext` — give Claude feedback and keep the turn going without being labeled a hook error (v2.1.163+)
- **PostToolUse:** `updatedToolOutput` — replace the tool's output for any tool, not just MCP (v2.1.121+)
- **SessionStart:** `sessionTitle` to set the session title on startup/resume, and a top-level `reloadSkills: true` to re-scan skill directories so skills installed by the hook load in the same session (v2.1.152+)
- **UserPromptSubmit:** `sessionTitle` to set the session title (v2.1.94+)

`terminalSequence` (top-level) emits desktop notifications, window titles, and bells without a controlling terminal (v2.1.141+).

Hook output over 50K characters is saved to disk and replaced with a file path + preview instead of being injected directly into context (v2.1.89+).

## Related Settings

- `permissions.additionalDirectories` — Array of paths in settings.json. Equivalent to `--add-dir` on CLI. Relative paths supported. Example: `{ "permissions": { "additionalDirectories": ["../docs/"] } }`
- `disableAllHooks` / `allowManagedHooksOnly` — gate whether hooks run at all; if your hook never fires, check these first. An unrecognized hook event name in `settings.json` no longer invalidates the whole file (v2.1.101+)

## Environment Variables

- `$CLAUDE_PROJECT_DIR` — project root
- `$CLAUDE_ENV_FILE` — SessionStart only: persist env vars here
- `$CLAUDE_EFFORT` — active effort level, available to command hooks and Bash tool commands (v2.1.133+)
- `$CLAUDE_CODE_SESSION_ID` — session id, matching the `session_id` passed to hooks; also set in the Bash tool subprocess and stdio MCP servers (v2.1.132+)
- `$CLAUDE_CODE_SUBPROCESS_ENV_SCRUB=1` — strips Anthropic/cloud credentials from subprocess environments (Bash tool, hooks, MCP stdio servers)

## Lifecycle

**Hooks load at session start.** Changes require restart. Use `/hooks` to verify.

## Common Patterns

### Block Sensitive Files

```bash
#!/bin/bash
set -euo pipefail
input=$(cat)
file_path=$(echo "$input" | jq -r '.tool_input.file_path // empty')

# Block path traversal
[[ "$file_path" == *".."* ]] && { echo "Path traversal" >&2; exit 2; }

# Block sensitive files
for pattern in ".env" ".git/" "credentials" "secrets"; do
  [[ "$file_path" == *"$pattern"* ]] && { echo "Sensitive file" >&2; exit 2; }
done
```

### Flag File Activation

Conditionally enable hooks:

```bash
#!/bin/bash
FLAG="$CLAUDE_PROJECT_DIR/.strict-mode"
[ ! -f "$FLAG" ] && exit 0  # Skip if flag absent

# Run validation only when flag exists
input=$(cat)
# ... validation logic ...
```

### Skip System-Generated Messages

UserPromptSubmit hooks fire on system-generated pseudo-user messages. Detect structurally instead of enumerating tags:

```bash
#!/bin/bash
PROMPT=$(echo "$EVENT" | jq -r '.prompt // ""')

# XML-tagged: starts with <tag>, contains matching </tag>
if [[ "$PROMPT" == "<"* ]]; then
    TAG_REST="${PROMPT#<}"
    TAG_NAME="${TAG_REST%%[> ]*}"
    [[ -n "$TAG_NAME" && "$PROMPT" == *"</${TAG_NAME}>"* ]] && exit 0
fi
# Bracket-enclosed: entire message is a single [...] line
if [[ "$PROMPT" == "["* ]]; then
    FIRST_LINE="${PROMPT%%$'\n'*}"
    [[ "$FIRST_LINE" == *"]" && "$PROMPT" == "$FIRST_LINE" ]] && exit 0
fi
# Raw text system messages (rare, no structural signal)
[[ "$PROMPT" == "This session is being continued"* ]] && exit 0
[[ "$PROMPT" == "Base directory for this skill:"* ]] && exit 0
```

Covers `<task-notification>`, `<teammate-message>`, `<command-name>`, `<local-command-*>`, `<system-reminder>`, `[Request interrupted by user]`, `[Image: source: ...]`, and any future tagged system messages automatically.

### SessionStart Context Loading

```bash
#!/bin/bash
if [ -n "$CLAUDE_ENV_FILE" ]; then
  echo "export NODE_ENV=development" >> "$CLAUDE_ENV_FILE"
  [ -f .nvmrc ] && echo "source ~/.nvm/nvm.sh && nvm use" >> "$CLAUDE_ENV_FILE"
fi

# Output becomes context for Claude
echo "Project initialized with development settings"
```

## Security

**Always:**
- Quote variables: `"$file_path"` not `$file_path`
- Validate inputs before use
- Use `set -euo pipefail`
- Check for path traversal (`..`)

**Never:**
- Trust tool input blindly
- Log sensitive data
- Use unquoted expansions

## Debugging

1. **Verify registration:** `/hooks` command
2. **Test manually:**
   ```bash
   echo '{"tool_name":"Write","tool_input":{"file_path":"/test"}}' | ./hook.sh
   echo "Exit: $?"
   ```
3. **Debug mode:** `claude --debug` (filter with `--debug hooks`)
4. **Validate JSON:** `./hook.sh < input.json | jq .`
5. **Stream events:** `claude -p --output-format=stream-json --include-hook-events` emits every hook lifecycle event in the output stream

## Prompt-Based Hooks

For complex logic, use LLM evaluation (single turn, no tools):

```json
{
  "type": "prompt",
  "prompt": "Evaluate if this tool use is safe. Check for: system paths, credentials, path traversal. Input: $ARGUMENTS. Return JSON: {\"decision\": \"approve|block\", \"reason\": \"...\"}",
  "timeout": 30
}
```

**Supported events:** PreToolUse, PostToolUse, Stop, StopFailure, SubagentStop, UserPromptSubmit, PermissionRequest, TaskCompleted, TaskCreated, CwdChanged, FileChanged, Elicitation, ElicitationResult

`SessionStart`, `Setup`, and `SubagentStart` accept **command-type hooks only** — configuring a prompt- or agent-type hook for these errors with "use a command-type hook instead" (v2.1.142+).

**Observed in 2.1.131:** `SubagentStop`, `TaskCompleted`, `TaskCreated`, and `SubagentStart` did not appear in any of 138 debug logs surveyed (134 historical plus 4 controlled `claude --debug` runs covering subagent dispatch). Subagent inner tool calls DO fire `PreToolUse`/`PostToolUse` — with the parent's `session_id` plus the subagent identity carried in `agent_id` / `agent_type` payload fields (see Event Input Schemas).

## Agent Hooks

Spawns a full subagent with tool access (Read, Grep, Glob, Bash, etc.). Use when the hook needs to **inspect files or the codebase** before deciding.

```json
{
  "type": "agent",
  "prompt": "Check if the file being written has corresponding tests. Input: $ARGUMENTS. If no tests exist, block with reason.",
  "model": "fast-model-id",
  "timeout": 60,
  "statusMessage": "Verifying test coverage..."
}
```

**Schema:**
- `type: "agent"` (required)
- `prompt: string` (required) — `$ARGUMENTS` replaced with hook input JSON
- `model: string` (optional) — defaults to a fast model
- `timeout: number` (optional) — default 60s (vs 30s for prompt hooks)
- `statusMessage: string` (optional) — custom spinner text
- `once: boolean` (optional) — run once per session

**Supported events:** Same as prompt hooks.

**Output format:** Same JSON decision format as prompt hooks (event-dependent).

**When to use agent vs prompt:**
- **Prompt** — decision can be made from the hook input JSON alone (fast, cheap)
- **Agent** — needs to read files, check build artifacts, scan code patterns (slow, costs API credits)

Agent hooks spawn full Claude sessions. Reserve for high-value workflows where the automation justifies the overhead.

## HTTP Hooks

Delegates to an external HTTP service:

```json
{
  "type": "http",
  "url": "https://example.com/hooks/validate",
  "timeout": 30
}
```

Posts hook input JSON to the URL, expects the same decision JSON format back.

## Performance

- Hooks run in parallel (no guaranteed order)
- Design for independence (no shared state)
- Use command hooks for fast deterministic checks
- Use prompt hooks for context-aware single-turn reasoning
- Use agent hooks only when filesystem inspection is required
- Use HTTP hooks to delegate to external services

## References

- [Official docs](https://code.claude.com/docs/en/hooks)
