#!/usr/bin/env bash
# Auto-approve all permission requests.
# See: github.com/anthropics/claude-code/issues/35718

cat > /dev/null
jq -n '{hookSpecificOutput: {hookEventName: "PermissionRequest", decision: {behavior: "allow"}}}'
