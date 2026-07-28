#!/usr/bin/env python3
"""Refuse a codex agent the write tool its definition never declared.

`tools:` is the Harness's own field and Claude honours it natively — a tool left
off the list is never offered. codex has no per-tool allowlist, and the field is
dropped when its artifacts are generated, so the read-only agents ran there with
full write access: one definition, two different agents. The divergence is a
capability one before it is a safety one, because an agent whose Prompt says it
reports findings and never writes code behaves differently when it can write.

What `tools:` actually withholds from these agents is the write family — Write,
Edit, MultiEdit, whose codex counterpart is apply_patch. Every one of them
declares Bash, so shell stays reachable on both harnesses and this gate does not
touch it. Denying more than the declaration does would make codex the stricter
agent, which is the same divergence pointing the other way.

The declaration is read off the definition file per run, like `memory: none` and
`codex-model`, so an edit takes effect without regenerating anything. codex_run
puts the resolved definition path in the environment rather than the agent's
name: the roster resolution behind it is subtle, and re-deriving it here would be
a second implementation of the thing that must not drift. An interactive codex
session sets nothing, declares nothing, and is not gated.
"""

import os
import sys

from lib import agent_memory, feedback
from lib.event import canonical_tool, read_event

BINDING = {
    "events": {"PreToolUse": ["apply_patch"]},
    "timeout": 5,
    "harness": "codex",
}

# The Claude tool names that grant writing. An agent declaring any of them keeps
# apply_patch; one declaring none never had a write tool to begin with.
_WRITE_TOOLS = ("Write", "Edit", "MultiEdit", "NotebookEdit")

MSG = """BLOCKED: the %s agent declares no write tool.

Its `tools:` grants %s and nothing that writes, so editing files is not yours to
do on either harness. On Claude the tool is simply absent; here it is refused, so
the same definition means the same agent.

Report what you found instead. Do not route the edit through the shell, and do
not ask another agent to make it for you."""


def _declared_tools(path):
    """The agent's `tools:` value, or None when it declares no such key.

    An agent with no `tools:` line has every tool on Claude, so it has nothing
    withheld here either."""
    try:
        return agent_memory.declaration(path, "tools")
    except (OSError, UnicodeDecodeError):
        # Unreadable declarations deny for memory because permission must never be
        # assumed. Here the same reasoning inverts: refusing the write tool of an
        # agent whose declaration cannot be read would break a writing agent on a
        # transient read failure, and the capability it is being denied is one
        # Claude would have granted.
        return None


def main():
    event = read_event()
    path = os.environ.get(agent_memory.AGENT_FILE_VAR, "")
    if not path:
        return 0
    if canonical_tool(event) != "write":
        return 0

    declared = _declared_tools(path)
    if declared is None:
        return 0
    tools = [t.strip() for t in declared.strip("[]").split(",") if t.strip()]
    if not tools or any(t in _WRITE_TOOLS for t in tools):
        return 0

    agent = os.path.splitext(os.path.basename(path))[0]
    return feedback.block("block_undeclared_tools", MSG % (agent, ", ".join(tools)))


if __name__ == "__main__":
    sys.exit(main())
