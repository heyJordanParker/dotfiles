#!/usr/bin/env python3
"""Hold every session and subagent in the one shared worktree.

Two surfaces reach a separate worktree: the EnterWorktree tool, and an Agent
dispatch carrying isolation: "worktree". One policy, so one gate.
"""

import sys

from lib import feedback
from lib.event import field, read_event

BINDING = {
    "events": {"PreToolUse": ["EnterWorktree", "Agent"]},
    "timeout": 5,
    "harness": "claude",
}

MSG = """BLOCKED: %s is BANNED.

### Work in the one shared worktree
The main session and every subagent share a single worktree. A parallel worktree
fragments them: siblings stop seeing each other's files, branch state diverges,
and coordination breaks. Read "in a worktree" in the architect's prose as "on one
of the project's named branches", never as this harness primitive.

IF a separate worktree is genuinely required:
### Return to the architect and say so
The architect controls worktree lifecycle."""


def main():
    event = read_event()
    if field(event, "tool_name", "") == "EnterWorktree":
        return feedback.block("block_worktree_isolation", MSG % "EnterWorktree")
    if field(event, "tool_input.isolation", "") == "worktree":
        return feedback.block("block_worktree_isolation", MSG % 'isolation: "worktree"')
    return 0


if __name__ == "__main__":
    sys.exit(main())
