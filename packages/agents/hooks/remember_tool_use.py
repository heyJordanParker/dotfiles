#!/usr/bin/env python3
"""Store what an agent actually did, alongside what it said about it.

`remember_agent_message.py` stores an agent's prose. Prose is where an agent
narrates, summarises, and sometimes rounds off — so a memory built from it alone
records the account of the work rather than the work. The plugin fed Honcho a
running line per tool call for exactly this reason, and losing it degrades
conclusion quality slowly enough that nobody notices for weeks.

One line per call, not the payload: the file that was written, the command that
was run, the agent that was dispatched. The content of an edit is in the
repository and its history, which is a better record than a memory server; what
memory cannot reconstruct later is that this agent touched this file at all.

Reads are excluded, and a read is a shell command as often as it is a Read tool.
`trace grep`, `trace status`, `git log`, `head` — these are the bulk of every run
and say only where an agent looked, which `trace` already records per file with
far more precision. Recording them costs a round trip each and fills the
collection with lookups the server then derives conclusions from.
"""

import shlex
import sys

from lib import agent_memory, honcho
from lib.event import agent_name, canonical_tool, command_str, field, read_event

BINDING = {
    "events": {"PostToolUse": ["Bash", "Write", "Edit", "MultiEdit", "Agent"]},
    "timeout": 10,
    "harness": "all",
    "roots": "all",
}

# A command's shape is the fact worth keeping; its full text is often a heredoc
# or a whole script, and storing that turns a work record into a paste bin.
COMMAND_LIMIT = 200

# The write is inline, in front of the tool result the agent is waiting on, so
# its ceiling is far below the one that governs writes nobody waits for.
POST_TIMEOUT = 3

# Commands that only look. `trace` is here because our own gates route every
# read through it, so it is the single most frequent command in any run.
LOOKING = {"cd", "ls", "pwd", "echo", "cat", "head", "tail", "less", "more",
           "which", "type", "file", "stat", "wc", "grep", "rg", "find", "trace",
           "printf", "date", "env", "true", "test"}

# A git subcommand that only reports. `git commit`/`push`/`checkout` change the
# repository and stay on the record.
LOOKING_GIT = {"status", "log", "diff", "show", "blame", "branch", "remote"}


def looking(command):
    """Whether the command only reads, so it is not work worth remembering."""
    try:
        words = shlex.split(command)
    except ValueError:
        words = command.split()
    if not words:
        return True
    head = words[0].rsplit("/", 1)[-1]
    if head == "git":
        return len(words) > 1 and words[1] in LOOKING_GIT
    return head in LOOKING


def observation(event):
    """The one line this tool call is worth remembering, or ""."""
    kind = canonical_tool(event)
    if kind == "write":
        path = field(event, "tool_input.file_path", "")
        return "wrote %s" % path if path else ""
    if kind == "shell":
        command = " ".join(command_str(event).split())
        if not command or looking(command):
            return ""
        return "ran `%s`" % command[:COMMAND_LIMIT]
    if kind == "agent":
        dispatched = field(event, "tool_input.subagent_type", "")
        return "dispatched the %s agent" % dispatched if dispatched else ""
    return ""


def main():
    event = read_event()

    cfg = honcho.config()
    if not honcho.enabled(cfg):
        return 0

    agent = agent_name(event)
    if not agent or agent_memory.denies_memory(agent_memory.definition_path(agent)):
        return 0

    line = observation(event)
    if not line:
        return 0

    honcho.post(
        cfg,
        honcho.session_name(field(event, "cwd", "")),
        agent,
        "[work] %s" % line,
        metadata={"instance_id": field(event, "session_id", "")},
        timeout=POST_TIMEOUT,
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
