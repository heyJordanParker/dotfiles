#!/usr/bin/env python3
"""Block modifications to session-state files unless a session hook is active."""

import os
import re
import sys

from lib import feedback
from lib.event import command_str, field, read_event

BINDING = {
    "events": {"PreToolUse": ["Bash", "Write", "Edit", "MultiEdit"]},
    "timeout": 5,
    "harness": "all",
}

PATTERN = (
    r"(echo|printf|cat|rm|mv|cp|jq|tee|chmod|chown|ls|stat|head|tail|sed|awk)\b"
    r".*/sessions/[^ ]*state\.json|[>].*/sessions/[^ ]*state\.json"
)

MSG = """BLOCKED: Session state files are managed by session hooks.

To change modes, tell the user — e.g. "enter solo mode" or "switch to subagents".
Do not modify session state files directly."""


def main():
    if os.environ.get("CLAUDE_SESSION_HOOK") == "true":
        return 0
    event = read_event()
    file_path = field(event, "tool_input.file_path", "")
    command = command_str(event)
    if re.search(r"/sessions/[^ ]*state\.json", file_path) or re.search(PATTERN, command):
        return feedback.block("protect_session_state", MSG)
    return 0


if __name__ == "__main__":
    sys.exit(main())
