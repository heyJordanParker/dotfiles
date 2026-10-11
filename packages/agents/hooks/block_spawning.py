#!/usr/bin/env python3
import sys

from lib import command, feedback
from lib.event import canonical_tool, command_str, read_event
from lib.session_mode import is_dispatched, may_spawn, resolve

BINDING = {
    "events": {"PreToolUse": ["Bash", "Agent", "Workflow", "RemoteTrigger"]},
    "harness": "all",
    "timeout": 5,
}
# `codex` and `claude` start a session with anything but a lone version or help flag.
_INFO_FLAGS = frozenset(("--version", "-V", "-v", "--help", "-h"))

MSG = "BLOCKED: %s mode does not spawn subagents.\n\nIn build mode, do the work yourself. In interview mode, report what the work needs."

SUBAGENT_MSG = ("BLOCKED: a Subagent never starts another agent.\n\n"
                "Do the work yourself, or return what needs another agent to your dispatcher.")


def _spawns(head, args):
    if head in ("codex", "claude"):
        return not (len(args) == 1 and args[0] in _INFO_FLAGS)
    return False

def _refusal(event):
    return SUBAGENT_MSG if is_dispatched(event) else MSG % resolve(event)

def main():
    event = read_event()
    if may_spawn(event):
        return 0
    if canonical_tool(event) == "agent":
        return feedback.block("block_spawning", _refusal(event))
    found = command.invocations(command_str(event))
    if canonical_tool(event) == "shell" and (found is None or any(_spawns(head, args) for head, args in found)):
        return feedback.block("block_spawning", _refusal(event))
    return 0

if __name__ == "__main__":
    sys.exit(main())
