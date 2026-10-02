#!/usr/bin/env python3
"""Ensure a `trace` command's paths have their project docs in context.

When the agent runs a path-taking `trace <subcmd> <paths>` shell command, this
sends those paths' project docs not yet in context, as Markdown, in a
hookSpecificOutput.additionalContext envelope: each doc from its first unread
line, nearest first, as much as one hook message holds. A doc `trace read`
prints itself is left to the read. Blocks the command (exit 2) if `trace docs`
fails, so the agent never traces without project-docs context.

Inside a Claude Subagent the shell carries the session id but no agent id, so
the Subagent's own `trace` calls would record into the root agent's log. Each
one gets `--agent <id>` written in through `updatedInput`. Never crashes.
"""

import os
import re
import shlex
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

# `trace` in command position: at the start, or after a separator that ends the
# previous command.
_TRACE = re.compile(r"(?:(?<=^)|(?<=[;&|(\n]))(\s*(?:\S*/)?trace)(?=\s|$)(?!\s+--agent\b)")


def _targets(line, cwd):
    """(subcommand, paths) of the path-taking `trace` call on the line: every
    argument that exists on disk — `grep` and `pattern` take the pattern before
    their paths — or the working directory. ("", []) when there is none."""
    for head, args in command.invocations(line) or []:
        if head != "trace" or not args or args[0] not in PATH_TAKING:
            continue
        resolved = (tracer.resolve(arg, cwd) for arg in args[1:] if not arg.startswith("-"))
        paths = [path for path in resolved if path and os.path.exists(path)]
        return args[0], list(dict.fromkeys(paths)) or [cwd]
    return "", []


def _with_agent(event, line):
    """The tool input with `--agent <id>` written into each `trace` call, or
    None outside a Subagent or when the line calls no `trace`."""
    agent = field(event, "agent_id", "")
    if not agent:
        return None
    replaced, count = _TRACE.subn(r"\1 --agent " + shlex.quote(agent).replace("\\", r"\\"), line)
    if not count:
        return None
    # `updatedInput` replaces the whole input, so the sibling fields go back as
    # they came: `run_in_background` among them.
    tool_input = dict(field(event, "tool_input", {}) or {})
    tool_input["command"] = replaced
    return tool_input


def main():
    if not tracer.available():
        return 0
    event = read_event()
    cwd = field(event, "cwd", "") or os.getcwd()
    line = field(event, "tool_input.command", "")
    if not line:
        return 0
    subcommand, targets = _targets(line, cwd)
    rewrite = _with_agent(event, line)
    text = ""
    if targets:
        skip = [path for path in targets if os.path.isfile(path)] if subcommand == "read" else []
        rc, text, err = tracer.docs(event, targets, SOURCE, "Bash", line, skip)
        if rc != 0:
            return feedback.block(
                SOURCE,
                "BLOCKED: project-docs load failed for: %s\n\n"
                "`trace docs` exited %d. The trace command is\n"
                "blocked so the agent does not run it without project-docs context.\n\n"
                "Underlying error:\n%s" % (" ".join(targets), rc, err)
            )
    if text.strip():
        return feedback.context(SOURCE, "PreToolUse", text.strip(), rewrite)
    if rewrite:
        return feedback.updated_input("PreToolUse", rewrite)
    return 0


if __name__ == "__main__":
    sys.exit(main())
