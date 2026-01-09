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
- **PreCompact:** Preserve critical context — matcher: manual|auto
- **Notification:** React to user notifications — matcher: notification types
- **PermissionRequest:** Auto-allow/deny permissions — matcher: tool names

## Configuration

**Locations (precedence order):**
1. `~/.claude/settings.json` (user)
2. `.claude/settings.json` (project)
3. `.claude/settings.local.json` (local, not committed)
4. Plugin `hooks/hooks.json`
5. Component frontmatter (skills, agents, commands)

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

## Environment Variables

- `$CLAUDE_PROJECT_DIR` — project root
- `$CLAUDE_ENV_FILE` — SessionStart only: persist env vars here

## Lifecycle

**Hooks load at session start.** Changes require restart. Use `/hooks` to verify.

## References

- [hooks-patterns.md](hooks-patterns.md) — Common patterns, security, debugging
- [Official docs](https://docs.anthropic.com/en/docs/claude-code/hooks)
