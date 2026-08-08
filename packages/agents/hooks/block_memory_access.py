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

Memory is reachable one way: the `honcho` command. The plugin's MCP tools were
the old surface and are gone with it, so the gate reads the shell command, which
makes it the gate on both harnesses — codex used to be covered by switching off
an MCP server that no longer exists. The injection hook is the other way in, and
it reads the same declaration before it puts anything in a turn or in a dispatch
brief, so a blank agent stays blank whichever direction memory travels.

codex names no agent in its payload, so a codex run's definition comes from the
path its launcher exported, the same variable lib/codex_run.py sets.

One gate covers both directions the command travels: `context`, `search` and
`ask` read, and `remember` and `forget` write. The hooks that write a turn's
messages carry the same declaration check of their own, because nothing routes
them through here.
"""

import os
import re
import sys

from lib import agent_memory, feedback
from lib.event import agent_name, command_str, field, read_event

BINDING = {
    "events": {"PreToolUse": ["Bash"]},
    "timeout": 5,
    "harness": "all",
    "roots": "all",
}

# The command as a word, so `honcho context …` and an absolute path to it match
# while `echo honcho` in prose does not.
_HONCHO = re.compile(r"(^|[\s;&|(])(\S*/)?honcho(\s|$)")

MSG = """BLOCKED: the %s agent declares `memory: none`.

You run blank on purpose. Memory is not yours to read or write — a stale
conclusion would sidetrack a one-shot task, and a write from a one-shot task
would pollute Memory for every agent after you.

Work from what this task's prompt and the repository tell you. Do not retry
through another memory tool, and do not ask another agent to reach it for you."""


def gated(event):
    """The agent whose declaration governs this call, or "".

    A subagent dispatch and a codex run are the two one-shot executions the
    declaration is for. The main thread of a session started with `--agent` is
    not one, and keeps Memory whatever that agent declares.
    """
    if not field(event, "agent_id", "") and not os.environ.get(agent_memory.AGENT_FILE_VAR):
        return ""
    return agent_name(event)


def main():
    event = read_event()
    if not _HONCHO.search(command_str(event)):
        return 0
    name = gated(event)
    if not name or not agent_memory.denies_memory(agent_memory.definition_path(name)):
        return 0
    return feedback.block("block_memory_access", MSG % name)


if __name__ == "__main__":
    sys.exit(main())
