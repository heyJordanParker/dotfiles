#!/bin/bash
# Block EnterWorktree tool — this project uses a single shared worktree
# Gracefully allows on any error

read -r input

tool_name=$(echo "$input" | jq -r '.tool_name // ""' 2>/dev/null) || exit 0

if [ "$tool_name" = "EnterWorktree" ]; then
    cat >&2 <<'EOF'
BLOCKED: EnterWorktree is BANNED.

This project uses a single shared worktree across the main session and every
subagent. Entering a parallel worktree fragments the team — siblings stop
seeing each other's files, branch state diverges, and coordination breaks.
The harness primitive that creates or enters a separate worktree must never
be used here.

If a separate working directory is genuinely required, return to the user
and say so. The user controls worktree lifecycle.
EOF
    exit 2
fi

exit 0
