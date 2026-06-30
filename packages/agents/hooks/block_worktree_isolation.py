#!/usr/bin/env python3
"""Block Agent dispatches with isolation: "worktree"."""

import sys

from lib import feedback
from lib.event import field, read_event

BINDING = {
    "events": {"PreToolUse": ["Agent"]},
    "timeout": 5,
    "harness": "claude",
}

MSG = """BLOCKED: subagent isolation: "worktree" is BANNED.

The main session and every subagent in this project share a single worktree.
Spawning a subagent into its own worktree fragments the team — siblings stop
seeing each other's files, branch state diverges, and coordination breaks.
The recent failure mode: an agent dispatched with isolation: "worktree" ran
in a parallel tree and the parent never saw its work.

Do NOT pass isolation: "worktree". Omit the field, or use a non-worktree
value supported by the harness. "In a worktree" in user prose means "on one
of the project's named branches", not this harness primitive.

If a separate worktree is genuinely required, return to the user and say so.
The user controls worktree lifecycle."""


def main():
    event = read_event()
    if field(event, "tool_input.isolation", "") == "worktree":
        return feedback.block("block_worktree_isolation", MSG)
    return 0


if __name__ == "__main__":
    sys.exit(main())
