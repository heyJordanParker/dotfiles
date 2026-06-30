#!/usr/bin/env python3
"""Block git commit unless commit_requested is set in session state."""

import re
import sys

from lib import feedback
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

    # Strip quoted strings first so echo/printf content can't trip the match.
    stripped = re.sub(r"(['\"])[^'\"]*\1", "", command)
    if not re.search(r"(^|[;&|\s])git\s+commit(\s|$)", stripped):
        return 0

    session_id = owner_session(event)
    if not session_id:
        return 0

    data = load_state(session_id)
    commit_requested = data.get("commit_requested")
    if commit_requested is None or commit_requested is False:
        requested = "false"
    elif commit_requested is True:
        requested = "true"
    else:
        requested = str(commit_requested)

    if requested != "true":
        return feedback.block("block_unauthorized_commits", MSG)
    return 0


if __name__ == "__main__":
    sys.exit(main())
