# Hooks

Event-driven automation for Claude Code.

## Hook Types

- **Command** (`type: "command"`) — Bash scripts, deterministic
- **Prompt** (`type: "prompt"`) — LLM-driven, context-aware (recommended for complex logic)

## Events

- **PreToolUse:** Validate/block/modify tool calls — matcher: tool names
- **PostToolUse:** React to results, logging — matcher: tool names
- **Stop:** Completeness check before agent stops
- **SubagentStop:** Validate subagent task completion
- **UserPromptSubmit:** Add context, validate prompts
- **SessionStart:** Load context, set env vars — matcher: startup|resume|clear|compact
- **SessionEnd:** Cleanup
- **Setup:** Repository setup/maintenance — trigger: --init, --init-only, --maintenance
- **PreCompact:** Preserve critical context — matcher: manual|auto
- **PostCompact:** React after compaction completes (e.g., log, notify, refresh state)
- **Notification:** React to user notifications — matcher: notification types
- **PermissionRequest:** Auto-allow/deny permissions — matcher: tool names
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

PreToolUse hooks can return `additionalContext` to inject context into the model.

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

For complex logic, use LLM evaluation:

```json
{
  "type": "prompt",
  "prompt": "Evaluate if this tool use is safe. Check for: system paths, credentials, path traversal. Input: $ARGUMENTS. Return JSON: {\"decision\": \"approve|block\", \"reason\": \"...\"}",
  "timeout": 30
}
```

**Supported events:** PreToolUse, PostToolUse, Stop, SubagentStop, UserPromptSubmit, PermissionRequest, Elicitation, ElicitationResult

## Performance

- Hooks run in parallel (no guaranteed order)
- Design for independence (no shared state)
- Use command hooks for fast deterministic checks
- Use prompt hooks for complex reasoning

## References

- [Official docs](https://docs.anthropic.com/en/docs/claude-code/hooks)
