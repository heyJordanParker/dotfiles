#!/bin/bash
# Block TeamDelete tool — Jordan controls team lifecycle
# Gracefully allows on any error

read -r input

tool_name=$(echo "$input" | jq -r '.tool_name // ""' 2>/dev/null) || exit 0

if [ "$tool_name" = "TeamDelete" ]; then
    cat >&2 <<'EOF'
BLOCKED: Teams are managed by Jordan. Use SendMessage to reassign teammates.
EOF
    exit 2
fi

exit 0
