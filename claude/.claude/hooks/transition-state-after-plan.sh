#!/bin/bash
# Transition state to "executing" after plan approval
# Fixes stale "proposing" state when plan approval bypasses UserPromptSubmit
# Gracefully allows on any error

export CLAUDE_SESSION_HOOK=true

input=$(cat)
session_id=$(echo "$input" | jq -r '.session_id // ""' 2>/dev/null) || exit 0
[[ -z "$session_id" || "$session_id" == agent-* ]] && exit 0

state_file="/tmp/claude-session-state-${session_id}"
[ ! -f "$state_file" ] && exit 0

jq '.state = "executing"' "$state_file" > "${state_file}.tmp" 2>/dev/null && \
    mv "${state_file}.tmp" "$state_file" 2>/dev/null || true
exit 0
