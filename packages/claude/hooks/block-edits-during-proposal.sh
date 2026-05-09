#!/bin/bash
# Block Write/Edit/NotebookEdit when state is "proposing" (not yet approved)
# Reads session state independently — no coupling to other hooks
# Gracefully allows on any error (file missing, parse error)

read -r input

session_id=$(echo "$input" | jq -r '.session_id // ""' 2>/dev/null) || exit 0
[ -z "$session_id" ] && exit 0

state_file="/tmp/claude-session-state-${session_id}"
[ ! -f "$state_file" ] && "$HOME/.claude/hooks/initialize-session-state.sh" "$session_id"

state=$(jq -r '.state // "proposing"' "$state_file" 2>/dev/null) || exit 0

# Allow writes to planning artifact directories
file_path=$(echo "$input" | jq -r '.tool_input.file_path // ""' 2>/dev/null) || file_path=""
[[ "$file_path" == *"/.claude/shaping/"* ]] && exit 0
[[ "$file_path" == *"/.claude/plans/"* ]] && exit 0

if [ "$state" = "proposing" ]; then
    cat >&2 <<'EOF'
BLOCKED: A proposal is expected — do not edit code.

Update your proposal based on the user's feedback and present it again.
Only edit code after the user approves.
EOF
    exit 2
fi

exit 0
