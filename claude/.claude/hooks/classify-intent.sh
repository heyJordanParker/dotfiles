#!/bin/bash

# Guard against recursion from claude -p
[[ -n "${CLAUDE_CODE_SIMPLE:-}" ]] && exit 0

INPUT=$(cat)
PROMPT=$(echo "$INPUT" | jq -r '.prompt' 2>/dev/null || echo "")
SESSION_ID=$(echo "$INPUT" | jq -r '.session_id' 2>/dev/null || echo "")

# Skip agent sessions or empty input
[[ -z "$SESSION_ID" || "$SESSION_ID" == agent-* ]] && exit 0

# Read current state
CURRENT_MODE=$($HOME/.claude/hooks/claude-mode.sh --session "$SESSION_ID" get mode 2>/dev/null || echo "")
CURRENT_APPROACH=$($HOME/.claude/hooks/claude-mode.sh --session "$SESSION_ID" get approach 2>/dev/null || echo "")

# Read the classifier prompt from file
CLASSIFIER_PROMPT=$(cat "$HOME/.claude/hooks/intent-classifier.md" 2>/dev/null || echo "Classify into Question/Proposal/Plan/Execute.")

# Build the full prompt for the classifier
FULL_PROMPT="$CLASSIFIER_PROMPT

---

Current state:
- Mode: ${CURRENT_MODE:-none}
- Approach: ${CURRENT_APPROACH:-none}

User message to classify:
$PROMPT

Remember: mode and approach are sticky. Only change if the user's intent clearly shifted."

# Call sonnet to classify (--setting-sources "" skips hooks/skills/MCPs, prevents recursion)
RESULT=$(echo "$FULL_PROMPT" | claude -p --model sonnet --setting-sources "" --strict-mcp-config --output-format text 2>/dev/null || echo "")

# Extract mode and approach from result and persist
MODE=$(echo "$RESULT" | grep -i "^Mode:" | head -1 | sed 's/^Mode: *//' | awk '{print $1}' || echo "")
APPROACH=$(echo "$RESULT" | grep -i "^Approach:" | head -1 | sed 's/^Approach: *//' | awk '{print $1}' || echo "")

if [[ -n "$MODE" ]]; then
  $HOME/.claude/hooks/claude-mode.sh --session "$SESSION_ID" set mode "$MODE" 2>/dev/null || true
fi

if [[ -n "$APPROACH" ]]; then
  $HOME/.claude/hooks/claude-mode.sh --session "$SESSION_ID" set approach "$APPROACH" 2>/dev/null || true
fi

# Output the classifier result as context for the main agent
# Always exit 0 — this hook should NEVER block user messages
echo "$RESULT"
exit 0
