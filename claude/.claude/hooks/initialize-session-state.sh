#!/bin/bash
# Initialize session state file with canonical defaults if missing
# Idempotent — no-op if file already exists
# Called from: SessionStart hook (primary), any hook fallback (safety net)
# Usage: initialize-session-state.sh <session_id>

SESSION_ID="${1:-}"
[ -z "$SESSION_ID" ] && exit 0
[[ "$SESSION_ID" == agent-* ]] && exit 0

STATE_FILE="/tmp/claude-session-state-${SESSION_ID}"
[ -f "$STATE_FILE" ] && exit 0

jq -n '{approach: "solo", state: "proposing", intent: "instructions", commit_requested: false, notes: [], validation_phase: 0}' > "$STATE_FILE" 2>/dev/null || true
