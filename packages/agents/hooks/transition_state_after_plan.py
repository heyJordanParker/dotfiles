#!/usr/bin/env python3
"""Transition session state to "executing" after plan approval."""

import os
import sys

from lib.event import field, read_event
from lib.session_state import merge_state

BINDING = {
    "events": {"PostToolUse": ["ExitPlanMode"]},
    "timeout": 5,
    "harness": "claude",
}


def main():
    os.environ["CLAUDE_SESSION_HOOK"] = "true"
    event = read_event()
    session_id = field(event, "session_id", "")
    if not session_id or session_id.startswith("agent-"):
        return 0
    merge_state(session_id, {"state": "executing"})
    return 0


if __name__ == "__main__":
    sys.exit(main())
