#!/usr/bin/env python3
"""Hold every Bash call to one step.

Chaining, looping, piping into a change, and carrying a file in a heredoc are all
ways of putting several steps in one call. They were banned in the prompts and the
ban did not hold: across 214,791 recorded Bash calls, 53.8% composed something, at
the same rate in dispatched agents as in the architect's own sessions.

The refusal names the operator and the plainer replacement, because every shape
here has one — a second call, or the tool that already owns the job. Reading the
line is `lib/command.composition_refusal`; this hook owns only where it applies,
which is everywhere: no state, no mode, and no declaration exempts a session.
"""

import sys

from lib import feedback
from lib.command import composition_refusal
from lib.event import command_str, read_event

BINDING = {
    "events": {"PreToolUse": ["Bash"]},
    "harness": "all",
    "roots": "all",
    "timeout": 5,
}

MSG = """BLOCKED: one Bash call runs one command — %s.

Work in atomic steps. Each call does one thing, and you read its result before
choosing the next one. A chain that fails halfway costs the whole payload to
re-send, and hides which part failed.

To write a file, use Write. To edit one, use Edit. To run several commands, send
several calls."""


def main():
    event = read_event()
    command = command_str(event)
    if not command:
        return 0
    refusal = composition_refusal(command)
    if refusal:
        return feedback.block("block_composed_commands", MSG % refusal)
    return 0


if __name__ == "__main__":
    sys.exit(main())
