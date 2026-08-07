#!/usr/bin/env python3
"""Refuse codex-run launches the Claude harness cannot track."""

import os
import sys

from lib import feedback
from lib.codex_run import _ONE_JOB
from lib.command import command_head, tokenize
from lib.event import command_str, field, is_subagent, read_event

BINDING = {
    "events": {"PreToolUse": ["Bash"]},
    "timeout": 5,
    "harness": "claude",
    "roots": "all",
}

_WRAPPER_WORDS = {"time", "exec", "command"}
_READ_BACK = set(_ONE_JOB) | {"status", "watch"}

FOREGROUND_MSG = """BLOCKED: codex-run is foreground.

The harness cannot track a foreground codex-run, so it has no footer entry or
completion notification.

Set `run_in_background: true` on the Bash call instead."""

SHELL_BACKGROUND_MSG = """BLOCKED: codex-run is shell-backgrounded with `&`.

The shell hides that run from the harness, so it has no footer entry or completion
notification.

Remove `&` and set `run_in_background: true` on the Bash call instead."""


def main():
    event = read_event()
    if is_subagent(event):
        return 0

    tokens = tokenize(command_str(event))
    if tokens is None:
        return 0

    words = []
    for token in tokens + [";"]:
        if token and all(char in ";|&()\n" for char in token):
            segment = words
            words = []
            while segment and command_head(segment) in _WRAPPER_WORDS:
                segment = segment[1:]
            if command_head(segment) != "codex-run":
                continue

            command_index = next(
                index for index, word in enumerate(segment)
                if os.path.basename(word.strip("\"'")) == "codex-run"
            )
            action = segment[command_index + 1] if len(segment) > command_index + 1 else ""
            if action in _READ_BACK or (action != "resume" and not action.startswith("@")):
                continue
            if token == "&":
                return feedback.block("enforce_background_codex_run", SHELL_BACKGROUND_MSG)
            if field(event, "tool_input.run_in_background", None) is not True:
                return feedback.block("enforce_background_codex_run", FOREGROUND_MSG)
            continue
        words.append(token)
    return 0


if __name__ == "__main__":
    sys.exit(main())
