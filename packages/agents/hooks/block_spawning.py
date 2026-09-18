#!/usr/bin/env python3
import sys

from lib import command, feedback
from lib.codex_run import launches
from lib.event import canonical_tool, command_str, read_event
from lib.session_mode import permits, resolve

BINDING = {
    "events": {"PreToolUse": ["Bash", "Agent", "Workflow", "CronCreate", "RemoteTrigger"]},
    "harness": "all",
    "timeout": 5,
}
# `codex` and `claude` start a session with anything but a lone version or help flag.
_INFO_FLAGS = frozenset(("--version", "-V", "-v", "--help", "-h"))

MSG = "BLOCKED: this is %s mode, which does not spawn.\n\nDo the work yourself."


def _spawns(head, args):
    if head == "codex-run":
        # Cancelling stops another agent's run, which is as much the orchestrator's
        # call as starting one.
        action = args[0] if args else ""
        return launches(action) or action == "cancel"
    if head in ("codex", "claude"):
        return not (len(args) == 1 and args[0] in _INFO_FLAGS)
    return False

def main():
    event = read_event()
    if permits(event, "spawn"):
        return 0
    if canonical_tool(event) == "agent":
        return feedback.block("block_spawning", MSG % resolve(event))
    found = command.invocations(command_str(event))
    if canonical_tool(event) == "shell" and (found is None or any(_spawns(head, args) for head, args in found)):
        return feedback.block("block_spawning", MSG % resolve(event))
    return 0

if __name__ == "__main__":
    sys.exit(main())
