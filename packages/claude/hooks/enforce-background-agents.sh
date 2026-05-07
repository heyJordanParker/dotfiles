#!/bin/bash
# Block foreground Agent dispatches — all agents must run in background
# No session state dependency

read -r input

run_in_background=$(echo "$input" | jq -r '.tool_input.run_in_background // false' 2>/dev/null) || exit 0

if [ "$run_in_background" != "true" ]; then
    cat >&2 <<'EOF'
BLOCKED: Agent dispatches must run in the background.

Set run_in_background: true to avoid interrupting the user's flow.
EOF
    exit 2
fi

exit 0
