#!/usr/bin/env python3
"""Block subagent spawning when approach is solo.

Solo mode forbids every way to start another agent: the Agent tool (Claude
subagents), the `codex` / `codex-run` commands (codex subagents), and `claude`
run from the shell."""

import re
import sys

from lib import feedback
from lib.event import command_str, field, read_event
from lib.session_state import load_state

BINDING = {
    "events": {"PreToolUse": ["Bash", "Agent"]},
    "timeout": 5,
    "harness": "claude",
}

MSG = """BLOCKED: Solo mode is active — do not spawn subagents.

Read the relevant files yourself and keep context in this conversation.
This covers the Agent tool and the codex / codex-run / claude commands.
If you need to switch modes, ask the user."""

# A subagent-spawning command at the head of a shell segment.
_SUBAGENT_CMD = re.compile(r"(?:^|[;&|(]\s*)(?:codex-run|codex|claude)(?:\s|$)")


def main():
    event = read_event()
    session_id = field(event, "session_id", "")
    if not session_id:
        return 0
    if (load_state(session_id).get("approach") or "solo") != "solo":
        return 0
    # Bash blocks only when it actually launches a subagent; the Agent tool (the
    # other matcher this hook is wired on) is always a spawn.
    if field(event, "tool_name", "") == "Bash" and not _SUBAGENT_CMD.search(command_str(event)):
        return 0
    return feedback.block("enforce_solo_mode", MSG)


if __name__ == "__main__":
    sys.exit(main())
