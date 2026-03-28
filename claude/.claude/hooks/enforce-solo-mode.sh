#!/bin/bash
# Block Agent tool when approach is solo
# Reads session state independently — no coupling to other hooks
# Gracefully allows on any error (file missing, parse error, wrong mode)

read -r input

session_id=$(echo "$input" | jq -r '.session_id // ""' 2>/dev/null) || exit 0
[ -z "$session_id" ] && exit 0

state_file="/tmp/claude-session-state-${session_id}"
[ ! -f "$state_file" ] && exit 0

approach=$(jq -r '.approach // "default"' "$state_file" 2>/dev/null) || exit 0

if [ "$approach" = "solo" ]; then
    cat >&2 <<'EOF'
BLOCKED: Solo mode is active — do not spawn subagents.

Read the relevant files yourself and keep context in this conversation.
If you need to switch modes, ask the user.
EOF
    exit 2
fi

exit 0
