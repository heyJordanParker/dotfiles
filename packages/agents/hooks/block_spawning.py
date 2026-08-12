#!/usr/bin/env python3
import sys

from lib import command, feedback
from lib.event import canonical_tool, command_str, read_event
from lib.session_mode import permits

BINDING = {"events": {"PreToolUse": ["Bash", "Agent"]}, "harness": "all", "timeout": 5}
_SPAWNS = frozenset(("codex-run", "codex", "claude"))

def main():
    event = read_event()
    if permits(event, "spawn"):
        return 0
    if canonical_tool(event) == "agent":
        return feedback.block("block_spawning", "BLOCKED: spawning is disabled in this mode.")
    found = command.invocations(command_str(event))
    if canonical_tool(event) == "shell" and (found is None or any(head in _SPAWNS for head, _ in found)):
        return feedback.block("block_spawning", "BLOCKED: spawning is disabled in this mode.")
    return 0

if __name__ == "__main__":
    sys.exit(main())
