#!/usr/bin/env python3
"""Block git commit unless commit_requested is set in session state."""

import sys

from lib import feedback
from lib.command import git_subcommand, segments
from lib.event import command_str, owner_session, read_event
from lib.session_state import load_state

BINDING = {
    "events": {"PreToolUse": ["Bash"]},
    "harness": "all",
    "timeout": 5,
}

MSG = """BLOCKED: Commits require user authorization.

The user must explicitly ask for a commit. Use /commit when the user is ready to commit.
Do not commit without being asked."""


def main():
    event = read_event()
    command = command_str(event)

    # Resolve each segment's git subcommand past every global option — a flag
    # inventory can be walked around (6427d59 landed past one), the option
    # grammar cannot. An unparseable line (unbalanced quotes) cannot run in the
    # shell either, so mentioning commit there gates fail-closed rather than
    # resurrecting a weaker scanner.
    segs = segments(command)
    if segs is not None:
        if not any(git_subcommand(words) == "commit" for words in segs):
            return 0
    elif "commit" not in command:
        return 0

    session_id = owner_session(event)
    if not session_id:
        return 0

    if load_state(session_id).get("commit_requested") is not True:
        return feedback.block("block_unauthorized_commits", MSG)
    return 0


if __name__ == "__main__":
    sys.exit(main())
