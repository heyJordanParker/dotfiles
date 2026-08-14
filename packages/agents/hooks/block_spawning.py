#!/usr/bin/env python3
import sys

from lib import command, feedback
from lib.event import canonical_tool, command_str, read_event
from lib.session_mode import permits, resolve

BINDING = {
    "events": {"PreToolUse": ["Bash", "Agent", "Workflow", "CronCreate", "RemoteTrigger"]},
    "harness": "all",
    "timeout": 5,
}
_SPAWNS = frozenset(("codex-run", "codex", "claude"))

MSG = "BLOCKED: this is %s mode, which does not spawn.\n\nDo the work yourself."

def main():
    event = read_event()
    if permits(event, "spawn"):
        return 0
    if canonical_tool(event) == "agent":
        return feedback.block("block_spawning", MSG % resolve(event))
    found = command.invocations(command_str(event))
    if canonical_tool(event) == "shell" and (found is None or any(head in _SPAWNS for head, _ in found)):
        return feedback.block("block_spawning", MSG % resolve(event))
    return 0

if __name__ == "__main__":
    sys.exit(main())
