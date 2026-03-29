# Hooks

Event-driven automation for Claude Code.

## Hook Types

- **Command** (`type: "command"`) — Bash scripts, deterministic
- **Prompt** (`type: "prompt"`) — Single LLM turn, no tools, context-aware
- **Agent** (`type: "agent"`) — Spawns a full subagent with tool access (Read, Grep, Glob, Bash, etc.)
- **HTTP** (`type: "http"`) — Delegates to an external HTTP service

## Events

- **PreToolUse:** Validate/block/modify tool calls — matcher: tool names
- **PostToolUse:** React to results, logging — matcher: tool names
- **Stop:** Completeness check before agent stops
- **SubagentStop:** Validate subagent task completion
- **TaskCompleted:** React to completed subagent tasks
- **UserPromptSubmit:** Add context, validate prompts
- **SessionStart:** Load context, set env vars — matcher: startup|resume|clear|compact
- **SessionEnd:** Cleanup
- **Setup:** Repository setup/maintenance — trigger: --init, --init-only, --maintenance
- **PreCompact:** Preserve critical context — matcher: manual|auto
- **PostCompact:** React after compaction completes (e.g., log, notify, refresh state)
- **Notification:** React to user notifications — matcher: notification types
- **PermissionRequest:** Auto-allow/deny user-facing approval prompts (file access, tool confirmation, user interaction) — matcher: tool names
- **Elicitation:** Intercept MCP elicitation requests before showing to user — matcher: MCP server names
- **ElicitationResult:** Override/modify elicitation responses before sending back to MCP server — matcher: MCP server names

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
- `"Write|Edit"` — multiple tools
- `"*"` or omit — all tools
- `"mcp__.*"` — regex pattern

**Case-sensitive.** Use `/hooks` to verify tool names.

## Exit Codes

- **0:** Success (stdout processed)
- **2:** Block (stderr shown to Claude/user)
- **Other:** Non-blocking error

**Event-specific behavior:**
- **PreToolUse:** exit 2 blocks the tool call. Stderr shown to agent
- **UserPromptSubmit:** exit 2 blocks message submission, stderr shown to user. Any non-zero exit also blocks (not graceful like other events)
- **Stop:** exit 2 blocks the stop. Stderr becomes instructions to the agent, which acts on them and tries to stop again (re-triggering the hook). Can fire multiple times per turn

## Event Input Schemas

Each hook event receives a JSON object on stdin. Fields vary by event.

**PreToolUse / PostToolUse:**
- `session_id` — session UUID (prefixed `agent-` for subagent sessions)
- `tool_name` — the tool being called (e.g. `"Write"`, `"Bash"`, `"Agent"`)
- `tool_input` — tool-specific parameters (e.g. `{file_path, content}` for Write, `{command}` for Bash, `{prompt, run_in_background}` for Agent)
- `cwd` — working directory

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

## Environment Variables

- `$CLAUDE_PROJECT_DIR` — project root
- `$CLAUDE_ENV_FILE` — SessionStart only: persist env vars here

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
3. **Debug mode:** `claude --debug`
4. **Validate JSON:** `./hook.sh < input.json | jq .`

## Prompt-Based Hooks

For complex logic, use LLM evaluation (single turn, no tools):

```json
{
  "type": "prompt",
  "prompt": "Evaluate if this tool use is safe. Check for: system paths, credentials, path traversal. Input: $ARGUMENTS. Return JSON: {\"decision\": \"approve|block\", \"reason\": \"...\"}",
  "timeout": 30
}
```

**Supported events:** PreToolUse, PostToolUse, Stop, SubagentStop, UserPromptSubmit, PermissionRequest, TaskCompleted, Elicitation, ElicitationResult

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

**Supported events:** Same as prompt hooks — PreToolUse, PostToolUse, Stop, SubagentStop, UserPromptSubmit, PermissionRequest, TaskCompleted, Elicitation, ElicitationResult

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
