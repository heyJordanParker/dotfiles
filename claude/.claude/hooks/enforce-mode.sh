#!/bin/bash
set -euo pipefail
INPUT=$(cat)
TOOL=$(echo "$INPUT" | jq -r '.tool_name')
SESSION_ID=$(echo "$INPUT" | jq -r '.session_id')

# Skip for agent sessions (subagents shouldn't be blocked by parent's mode)
[[ "$SESSION_ID" == agent-* ]] && exit 0

MODE=$(echo "$($HOME/.claude/hooks/claude-mode.sh --session "$SESSION_ID" get mode)" | tr '[:upper:]' '[:lower:]')
APPROACH=$(echo "$($HOME/.claude/hooks/claude-mode.sh --session "$SESSION_ID" get approach)" | tr '[:upper:]' '[:lower:]')

# Block Write|Edit in Question and Proposal modes
case "$TOOL" in
  Write|Edit)
    case "$MODE" in
      question|proposal)
        echo "Blocked: $TOOL is not allowed in $MODE mode. No file changes permitted." >&2
        exit 2
        ;;
    esac
    ;;
  Agent)
    if [[ "$APPROACH" == "solo" ]]; then
      echo "Blocked: Agent tool is not allowed in Solo approach. Do the work yourself." >&2
      exit 2
    fi
    ;;
esac

exit 0
