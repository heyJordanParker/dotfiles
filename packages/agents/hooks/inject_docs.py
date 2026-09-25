#!/usr/bin/env python3
"""Ensure a `trace` command's target has its project docs in context.

When the agent runs a path-taking `trace <subcmd> <path>` shell command, this
sends that path's project docs not yet in context, as Markdown, in a
hookSpecificOutput.additionalContext envelope. Blocks the command (exit 2) if
`trace docs` fails, so the agent never traces without project-docs context.
Both harnesses run trace, so both run this. Never crashes.
"""

import os
import sys

from lib import command, feedback, tracer
from lib.event import field, read_event

BINDING = {
    "events": {"PreToolUse": ["Bash"]},
    "harness": "all",
    "timeout": 15,
    "standalone": True,
    "additionalContextLimit": 0,
}

SOURCE = "inject_docs"
PATH_TAKING = {
    "read", "info", "list", "tree", "structure", "grep",
    "pattern", "find", "blame", "history", "diff",
}


def _target(line, cwd):
    """The path a `trace <subcmd> ...` command reads: its first argument that
    exists on disk — `grep` and `pattern` take the pattern before their paths —
    or the working directory. "" when the line runs no path-taking trace."""
    for head, args in command.invocations(line) or []:
        if head != "trace" or not args or args[0] not in PATH_TAKING:
            continue
        for arg in args[1:]:
            path = tracer.resolve(arg, cwd) if not arg.startswith("-") else ""
            if path and os.path.exists(path):
                return path
        return cwd
    return ""


def main():
    if not tracer.available():
        return 0
    event = read_event()
    cwd = field(event, "cwd", "") or os.getcwd()
    command = field(event, "tool_input.command", "")
    target = _target(command, cwd) if command else ""
    if not target:
        return 0
    rc, text, err = tracer.docs(event, target, SOURCE, "Bash", command)
    if rc != 0:
        return feedback.block(
            SOURCE,
            "BLOCKED: project-docs load failed for: %s\n\n"
            "`trace docs \"%s\" ...` exited %d. The trace command is\n"
            "blocked so the agent does not run it without project-docs context.\n\n"
            "Underlying error:\n%s" % (target, target, rc, err)
        )
    if text.strip():
        feedback.context(SOURCE, "PreToolUse", text.strip())
    return 0


if __name__ == "__main__":
    sys.exit(main())
