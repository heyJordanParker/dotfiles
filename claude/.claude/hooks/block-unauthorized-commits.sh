#!/bin/bash
# Block git commit unless finalize flag is set in session state
# Gracefully allows on any error (file missing, parse error)

read -r input

command=$(echo "$input" | jq -r '.tool_input.command // ""' 2>/dev/null) || exit 0

# Only check commands containing git commit (strip quoted strings first to avoid matching echo/printf content)
stripped=$(echo "$command" | sed -E "s/(['\"])[^'\"]*\\1//g")
if ! echo "$stripped" | grep -qE '(^|[;&|[:space:]])git[[:space:]]+commit([[:space:]]|$)'; then
    exit 0
fi

session_id=$(echo "$input" | jq -r '.session_id // ""' 2>/dev/null) || exit 0
[ -z "$session_id" ] && exit 0

state_file="/tmp/claude-session-state-${session_id}"
[ ! -f "$state_file" ] && exit 0

finalize=$(jq -r '.finalize // false' "$state_file" 2>/dev/null) || exit 0

if [ "$finalize" != "true" ]; then
    cat >&2 <<'EOF'
BLOCKED: Commits require user authorization.

The user must explicitly ask for a commit. Use /commit when the user is ready to finalize.
Do not commit without being asked.
EOF
    exit 2
fi

exit 0
