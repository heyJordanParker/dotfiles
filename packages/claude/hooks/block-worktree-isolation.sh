#!/bin/bash
# Block Agent dispatches with isolation: "worktree" — we share one worktree
# Gracefully allows on any error.

read -r input

isolation=$(echo "$input" | jq -r '.tool_input.isolation // ""' 2>/dev/null) || exit 0

if [ "$isolation" = "worktree" ]; then
    cat >&2 <<'EOF'
BLOCKED: subagent isolation: "worktree" is BANNED.

The main session and every subagent in this project share a single worktree.
Spawning a subagent into its own worktree fragments the team — siblings stop
seeing each other's files, branch state diverges, and coordination breaks.
The recent failure mode: an agent dispatched with isolation: "worktree" ran
in a parallel tree and the parent never saw its work.

Do NOT pass isolation: "worktree". Omit the field, or use a non-worktree
value supported by the harness. "In a worktree" in user prose means "on one
of the project's named branches", not this harness primitive.

If a separate worktree is genuinely required, return to the user and say so.
The user controls worktree lifecycle.
EOF
    exit 2
fi

exit 0
