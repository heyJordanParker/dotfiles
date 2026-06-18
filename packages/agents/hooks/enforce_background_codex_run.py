#!/usr/bin/env python3
"""Force codex-run Bash calls into the background.

A foreground codex-run hangs the orchestration on a slow codex turn, so a codex-run
command is blocked unless run_in_background is set.

It governs only the codex-run wrapper, never raw `codex exec` — the wrapper is the
sanctioned interface and the only command this guard recognizes.
"""

import sys

from lib.command import command_head, segments
from lib.event import command_str, field, read_event

BINDING = {
    "events": {"PreToolUse": ["Bash"]},
    "timeout": 5,
    "harness": "all",
}

MSG = """BLOCKED: codex-run must run in the background.

A foreground codex-run blocks your orchestration on the codex turn.
Set run_in_background: true on the Bash call."""

# Shell words that wrap and run a following command in the foreground; the real
# command is the next token, so they're peeled off before the codex-run check.
_WRAPPER_WORDS = {"time", "exec", "command"}


def _invokes_codex_run(command):
    """True when a command segment actually runs the codex-run wrapper.

    The decision is made from the real command token of each segment — past
    leading `VAR=val` / `env X=1` prefixes, any `time`/`exec`/`command` wrapper
    word, and any path or quoting — exactly the way the other command guards parse
    a command (see command.command_head). So `codex-run …`, `FOO=1 codex-run …`,
    `env X=1 codex-run …`, `time codex-run …`, `exec codex-run …`,
    `command codex-run …`, a path-qualified or quoted form, and an invocation after
    a separator (including a newline) all count; a mere mention as an argument
    (`which codex-run`, `ls bin/codex-run`, the name inside an unrelated quoted
    string) does not, because the wrapper isn't the segment's command token there.
    Structural, not raw-string matching, so a new ordinary invocation form can't
    slip past it."""
    segs = segments(command)
    if segs is None:
        return False  # unparseable — never choke, never false-block
    for seg in segs:
        # `time`/`exec`/`command` are wrapper words that run the real command in
        # the foreground; unwrap them so the wrapped codex-run is still seen.
        while seg and command_head(seg) in _WRAPPER_WORDS:
            seg = seg[1:]
        if command_head(seg) == "codex-run":
            return True
    return False


def main():
    event = read_event()
    command = command_str(event)
    if not _invokes_codex_run(command):
        return 0
    rib = field(event, "tool_input.run_in_background", False)
    rib_str = "true" if rib is True else ("false" if (rib is False or rib is None) else str(rib))
    if rib_str != "true":
        sys.stderr.write(MSG + "\n")
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
