#!/usr/bin/env python3
"""Refuse a Claude dispatch of an agent whose definition sends it elsewhere.

`harness:` is ours, not the Harness's, and it is the counterpart of the BINDING
key hooks already declare: an agent states where it runs, and the two gates hold
it to that. This is the Claude half; lib/codex_run.py is the other. The key is
optional — an agent that omits it runs on both, which is every agent today.

A codex-only agent has no `model:` to run on here — its model is a `codex-model`,
which names nothing to Claude. Dispatching it anyway does not fail, which is the
problem: it runs on whatever the config root's model happens to be, under
instructions written for a different runtime, and reports back as though it were
the agent its definition describes.

The comparison does not lowercase, unlike the memory one. There, lowering widens
a denial and is safe; here the check is an allowlist, so lowering would widen
permission instead — `harness: CODEX` would pass. An unrecognized value therefore
denies on both harnesses, which surfaces a typo immediately rather than leaving
the agent unrestricted everywhere.
"""

import os
import sys

from lib import agent_memory, feedback
from lib.event import field, read_event

BINDING = {
    "events": {"PreToolUse": ["Agent"]},
    "timeout": 5,
    "harness": "claude",
    "roots": "all",
}

_HERE = ("all", "claude")

CODEX_MSG = """BLOCKED: the %s agent declares `harness: codex`.

It runs on codex only. Its model is a `codex-model`, which names nothing to
Claude, so this dispatch would run it on whatever model this root defaults to —
a different agent than its definition describes.

Run it through the Bash tool instead, with the same task prompt:

  codex-run @%s "<the same task prompt>"

The whole result comes back on stdout: the answer, then a trailer carrying the
status and the session id you resume it with."""

UNKNOWN_MSG = """BLOCKED: the %s agent declares `harness: %s`, which is not a harness.

Valid values are `all`, `claude`, and `codex`, and omitting the key means `all`.
The comparison is exact, so a wrong case is a wrong value. Until the definition is
corrected the agent runs nowhere — an unrecognized declaration denies rather than
permits, so a typo cannot quietly widen where an agent runs.

Fix the `harness:` line in its definition, then dispatch it again."""


def _agent_file(name):
    root = os.environ.get("CLAUDE_CONFIG_DIR") or os.path.expanduser("~/.claude")
    return os.path.join(root, "agents", name + ".md")


def main():
    event = read_event()
    name = field(event, "tool_input.subagent_type", "")
    if not name:
        return 0
    try:
        declared = agent_memory.declaration(_agent_file(name), "harness")
    except (OSError, UnicodeDecodeError):
        # A definition that cannot be read is not evidence the agent is barred.
        # Refusing every dispatch on a transient read failure would take the whole
        # roster down; the codex side makes the same call for the same reason.
        return 0
    if declared is None or declared in _HERE:
        return 0
    if declared == "codex":
        return feedback.block("block_wrong_harness", CODEX_MSG % (name, name))
    return feedback.block("block_wrong_harness", UNKNOWN_MSG % (name, declared))


if __name__ == "__main__":
    sys.exit(main())
