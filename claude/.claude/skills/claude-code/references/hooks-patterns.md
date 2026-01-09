# Hook Patterns

Advanced patterns, security, and debugging.

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

**Supported events:** PreToolUse, PostToolUse, Stop, SubagentStop, UserPromptSubmit, PermissionRequest

## Performance

- Hooks run in parallel (no guaranteed order)
- Design for independence (no shared state)
- Use command hooks for fast deterministic checks
- Use prompt hooks for complex reasoning
