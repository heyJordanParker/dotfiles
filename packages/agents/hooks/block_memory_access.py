#!/usr/bin/env python3
"""Refuse Memory to an agent whose definition declares `memory: none`.

An agent the architect converses with should keep Memory; a one-shot execution
agent should start blank, so a conclusion drawn weeks ago cannot sidetrack it.

The goal has always been subagents, so `agent_id` is the presence test: it is the
field that means "inside a subagent" and nothing else. `agent_type` does not mean
that — it is also present on the main thread of any session started with
`--agent`, and the architect's shell alias starts every session that way, so
keying on it would have taken Memory off his main thread the moment he launched
one as an agent declaring blank. `agent_type` still names which agent was
refused, in the message and in the definition lookup.

The declaration lives in the agent's own definition file and nowhere else, so
the name resolves to its definition under the *active* config root — profiles
carry their own agents/ directory, and a name means whichever file that root
holds. No declaration means Memory stays reachable.

Honcho's plugin reaches a session two ways: the mcp__plugin_honcho_honcho__*
tools, and its own SessionStart/UserPromptSubmit/Stop hooks. Only the tools are
reachable from inside a subagent — the lifecycle events fire for the main session
alone — so gating the tools closes the whole surface an agent has.
"""

import os
import sys

from lib import agent_memory, feedback
from lib.event import field, read_event

BINDING = {
    "events": {"PreToolUse": ["mcp__plugin_honcho_honcho__.*"]},
    "timeout": 5,
    "harness": "claude",
    "roots": "all",
}

MSG = """BLOCKED: the %s agent declares `memory: none`.

You run blank on purpose. Memory is not yours to read or write — a stale
conclusion would sidetrack a one-shot task, and a write from a one-shot task
would pollute Memory for every agent after you.

Work from what this task's prompt and the repository tell you. Do not retry
through another memory tool, and do not ask another agent to reach it for you."""


def _agent_file(name):
    root = os.environ.get("CLAUDE_CONFIG_DIR") or os.path.expanduser("~/.claude")
    return os.path.join(root, "agents", name + ".md")


def main():
    event = read_event()
    if not field(event, "agent_id", ""):
        return 0
    agent = field(event, "agent_type", "")
    if not agent or not agent_memory.denies_memory(_agent_file(agent)):
        return 0
    return feedback.block("block_memory_access", MSG % agent)


if __name__ == "__main__":
    sys.exit(main())
