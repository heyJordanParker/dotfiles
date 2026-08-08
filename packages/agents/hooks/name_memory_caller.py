#!/usr/bin/env python3
"""Write the running agent's name into a `honcho remember` before it runs.

`honcho remember` stores in the running agent's own collection and resolves that
agent from the environment. The environment is exact in a codex run and in a
Claude session, and wrong in exactly one place: inside a Claude subagent,
`CLAUDE_CODE_AGENT` still holds the agent that dispatched it — verified live from
a `code-reviewer` dispatch that read back `cto`.

Claude does name the agent, on the `PreToolUse` payload of the very call about to
run, and a `PreToolUse` hook can hand back an `updatedInput` that replaces the
command — verified live on the Bash tool, and without `permissionDecision`, so
this grants nothing that another gate would refuse. So the name is written into
the command as `--as <agent>`, and by the time the command runs there is nothing
left to infer.

The alternative was leaving the name in a note for the command to find, keyed by
the text being remembered. That keys two different parsers off one string: a
`$VAR` the shell expands, a `&&` tail, or an extra quote makes the keys differ,
and the miss is silent — the write lands in the dispatching agent's collection.
Rewriting the command has no such gap.

A call this cannot rewrite — `FOO=1 honcho remember …`, where the invocation is
not in command position — is refused rather than left to resolve wrongly, and the
refusal names the `--as` form to use.
"""

import os
import re
import shlex
import sys

from lib import feedback
from lib.event import agent_name, command_str, field, read_event

BINDING = {
    "events": {"PreToolUse": ["Bash"]},
    "timeout": 5,
    "harness": "claude",
    "roots": "all",
}

# The invocation in command position: at the start, or after a separator that
# ends the previous command. Bare whitespace is deliberately not a separator, so
# the word inside `echo "run honcho remember x"` is left alone.
_REWRITABLE = re.compile(
    r"(?:(?<=^)|(?<=[;&|(\n]))(\s*(?:\S*/)?honcho\s+remember)\b(?!\s+--as\b)")


def invoked(command):
    """Whether the line really calls `honcho remember`, wherever it sits.

    Tokenized the way the shell will read it, so `honcho` inside a quoted string
    is one word of an argument and not a call — a regex cannot tell those apart,
    and guessing either way is wrong: refusing a mention blocks honest work, and
    missing a real call lets the write land in the wrong collection.
    """
    try:
        words = shlex.split(command or "")
    except ValueError:
        return False
    return any(os.path.basename(word) == "honcho" and words[i + 1] == "remember"
               for i, word in enumerate(words[:-1]))

CANNOT_NAME = """BLOCKED: this `honcho remember` cannot be given your name.

`remember` writes into the running agent's own collection and takes the name from
the environment, which inside a subagent still holds the agent that dispatched
you. The name is normally written into the command before it runs, and that only
works when the call is in command position.

Run it as its own command, or name yourself explicitly:

  honcho remember --as %s "<what to keep>\""""


def rewritten(command, agent):
    """The command with `--as <agent>` written in, or "" when it already names one."""
    replaced, count = _REWRITABLE.subn(r"\1 --as " + shlex.quote(agent), command)
    return replaced if count else ""


def main():
    event = read_event()
    command = command_str(event)
    agent = agent_name(event)
    if not agent:
        return 0

    replaced = rewritten(command, agent)
    if replaced:
        # The rest of the call is handed back as it came: `updatedInput` replaces
        # the whole input, so rebuilding it from the command alone drops the
        # sibling fields — `run_in_background` among them.
        tool_input = dict(field(event, "tool_input", {}) or {})
        tool_input["command"] = replaced
        return feedback.updated_input("PreToolUse", tool_input)
    if "--as" not in command and invoked(command):
        return feedback.block("name_memory_caller", CANNOT_NAME % agent)
    return 0


if __name__ == "__main__":
    sys.exit(main())
