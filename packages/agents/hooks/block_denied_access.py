#!/usr/bin/env python3
"""Hold an agent to the access its definition declares, on both harnesses.

Two declarations, opposite defaults. `readonly: true` takes writing away from an
agent whose whole job is to read and report; absent, the agent writes. `ssh:
enabled` gives one agent a reachable machine; absent, no agent has one, because
the architect is the only one who touches production.

Both used to be Claude-shaped and neither held. `tools:` is Claude's own field
and Claude honours it natively, but codex has no per-tool allowlist and drops the
field, so the read-only agents ran there with full write access: one definition,
two different agents. And on *both* harnesses the shell stayed wide open — every
read-only agent declares `Bash`, so `sed -i`, `cat > file`, and `ssh prod` were
always one command away. A prompt that says an agent never writes code, on a
runtime where it can, is a capability claim the system does not keep.

The write family is a tool-name match and needs nothing clever. The shell is a
string, and a string is where enforcement is won or lost: a denylist of writing
commands fails open on the first shape nobody listed, and every bypass is a
one-liner (`env X=1 sed -i`, `bash -c '…'`, an absolute path). So the shell is an
allowlist instead. A read-only agent runs the commands that inspect and nothing
else, and anything this guard cannot resolve — an unbalanced quote, a command
behind an unexpanded variable — blocks rather than passes. The list is the cost:
it grows when a read-only agent needs a reader nobody listed yet, and that shows
up as a block naming the command, not as silent damage.

`ssh: enabled` reads off the same allowlist machinery from the other side, so a
writing agent keeps its shell and loses only the remote commands. What the agent
then runs on that machine is not read here: the declaration grants the machine,
and `readonly` governs this repository, not the far end of the connection.

The declaration governs a subagent dispatch and a codex run — the two one-shot
executions it is written for. The architect's own session is not one, even when
he starts it with `--agent`, so his shell keeps everything whatever that agent
declares.
"""

import os
import sys

from lib import agent_memory, command, feedback, session_mode
from lib.event import agent_name, canonical_tool, command_str, field, read_event

BINDING = {
    "events": {"PreToolUse": ["Bash", "Write", "Edit", "MultiEdit", "NotebookEdit"]},
    "timeout": 5,
    "harness": "all",
    "roots": "all",
}

# The Claude tool names that grant writing. An agent whose `tools:` names one
# keeps writing on codex too; one that names none never had a write tool to begin
# with, and codex granting it would be the divergence pointing the other way.
_WRITE_TOOLS = ("Write", "Edit", "MultiEdit", "NotebookEdit")

# The classification itself lives in lib/command.py. This lever asks whether the
# command is one a reader runs at all (`readonly_refusal`); block_writes asks which
# paths it changes (`mutation_targets`). Two halves of one question, one home.
_REMOTE = command.REMOTE


READONLY_MSG = """BLOCKED: the %s agent declares `readonly: true`.

%s

You read, you trace, and you report what you found. Changing the repository is
the caller's move to make from your report, on both harnesses — this is not a
codex-only restriction you can retry through the other one.

Do not route the change through the shell. If reading genuinely needs a command
this guard refuses, say so in your report and name it.%s"""

# An orchestrating agent dispatches the implementer by design, so the sentence
# that forbids it would refuse the agent its own job.
_NO_PROXY = "\nDo not ask another agent to make it for you."

SSH_MSG = """BLOCKED: the %s agent does not declare `ssh: enabled`.

`%s` reaches another machine. Remote access is opt-in per agent and this one did
not opt in, so production is not reachable from here on either harness.

Report what you need from that machine and let the caller run it."""

UNREADABLE_MSG = """BLOCKED: this command cannot be read by the access guard.

The %s agent declares limited access, and this command line cannot be parsed —
an unbalanced quote, or a command reached through an unexpanded variable. The
guard refuses what it cannot resolve rather than guessing at it.

Run the command with literal words and balanced quotes."""


def governing_definition(event):
    """The definition file whose declarations govern this call, and its agent.

    A subagent dispatch and a codex run are the two one-shot executions the
    declarations are written for. The main thread of a session started with
    `--agent` is not one — `agent_id` is the field that means "inside a subagent"
    and nothing else — so the architect's own session is never gated.
    """
    if not field(event, "agent_id", "") and not os.environ.get(agent_memory.AGENT_FILE_VAR):
        return "", ""
    name = agent_name(event)
    return agent_memory.definition_path(name), name


def declares_no_write_tool(path):
    """Whether `tools:` is present and names nothing that writes.

    Claude already withholds the write tools from such an agent; this is how codex
    reaches the same answer from the same line. An unreadable definition does not
    withhold here — the capability at stake is one Claude would have granted, and
    a transient read failure must not break a writing agent. `readonly: true` is
    the declaration that denies on an unreadable definition.
    """
    try:
        declared = agent_memory.declaration(path, "tools")
    except (OSError, UnicodeDecodeError):
        return False
    if declared is None:
        return False
    tools = [t.strip() for t in declared.strip("[]").split(",") if t.strip()]
    return bool(tools) and not any(t in _WRITE_TOOLS for t in tools)


def refuse_readonly(event, name, why):
    """Block the call, dropping the no-proxy sentence for an orchestrating agent."""
    try:
        mode = session_mode.declared_mode(event)
    except UnicodeDecodeError:
        mode = "build"
    proxy = "" if mode == "orchestrate" else _NO_PROXY
    return feedback.block("block_denied_access", READONLY_MSG % (name, why, proxy))


def main():
    event = read_event()
    path, name = governing_definition(event)
    if not path:
        return 0

    tool = canonical_tool(event)
    readonly = agent_memory.is_readonly(path)

    if tool == "write":
        if readonly:
            return refuse_readonly(event, name, "Editing files is not yours to do.")
        if declares_no_write_tool(path):
            return refuse_readonly(event, name, "Its `tools:` grants nothing that "
                                                "writes, so editing files is not yours to do.")
        return 0

    if tool != "shell":
        return 0

    # `git -C <dir> status` puts a path where the subcommand belongs; the shared
    # normalizer is what the other git guards read through too.
    line = command.git_normalize(command_str(event))
    ssh_allowed = agent_memory.allows_ssh(path)
    if ssh_allowed and not readonly:
        return 0

    found = command.invocations(line)
    if found is None or command.redirects_output(line) is None:
        return feedback.block("block_denied_access", UNREADABLE_MSG % name)

    if not ssh_allowed:
        for head, _args in found:
            if head in _REMOTE:
                return feedback.block("block_denied_access", SSH_MSG % (name, head))

    if readonly:
        if command.redirects_output(line):
            return refuse_readonly(event, name, "Redirecting output into a file writes it.")
        for head, args in found:
            why = command.readonly_refusal(head, args)
            if why:
                return refuse_readonly(event, name, why)

    return 0


if __name__ == "__main__":
    sys.exit(main())
