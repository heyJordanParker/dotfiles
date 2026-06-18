#!/usr/bin/env python3
"""Block the EnterWorktree tool."""

import sys

from lib.event import field, read_event

BINDING = {
    "events": {"PreToolUse": ["EnterWorktree"]},
    "timeout": 5,
    "harness": "claude",
}

MSG = """BLOCKED: EnterWorktree is BANNED.

This project uses a single shared worktree across the main session and every
subagent. Entering a parallel worktree fragments the team — siblings stop
seeing each other's files, branch state diverges, and coordination breaks.
The harness primitive that creates or enters a separate worktree must never
be used here.

If a separate working directory is genuinely required, return to the user
and say so. The user controls worktree lifecycle."""


def main():
    event = read_event()
    if field(event, "tool_name", "") == "EnterWorktree":
        sys.stderr.write(MSG + "\n")
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
