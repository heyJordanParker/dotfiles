#!/usr/bin/env python3
"""Block assignment to zsh's tied search-path parameters.

In zsh `path`, `cdpath`, `fpath`, and `manpath` are array parameters tied to
$PATH, $CDPATH, $FPATH, and $MANPATH. Assigning a plain string to one of them
replaces the whole array, so `path="$HOME/x"` collapses the command search path
to a single bogus entry and breaks every later command in that shell.

Only a bare assignment clobbers the shell. A temporary `path=x cmd` prefix is
scoped to that one command in zsh and is left alone — the guard fires only on a
segment whose entire content is assignments (no command follows).
"""

import re
import sys

from lib import feedback
from lib.command import command_head, segments
from lib.event import command_str, read_event

BINDING = {
    "events": {"PreToolUse": ["Bash"]},
    "timeout": 5,
    "harness": "all",
}

TIED = {"path", "cdpath", "fpath", "manpath"}
_ASSIGN_NAME = re.compile(r"^([A-Za-z_][A-Za-z0-9_]*)=")


def block(name):
    return feedback.block(
        "block_path_assignment",
        "BLOCKED: assigning to `%s` corrupts the shell.\n\n"
        "In zsh `%s` is tied to the $%s array; assigning a plain string to it\n"
        "replaces the array and breaks command resolution for the rest of the\n"
        "shell. Rename the variable (e.g. `target_path`)." % (name, name, name.upper())
    )


def main():
    event = read_event()
    command = command_str(event)

    segs = segments(command)
    if segs is None:
        return 0

    for seg in segs:
        # A command after the assignments means a scoped temporary assignment in
        # zsh — left alone. Only an all-assignments segment clobbers the shell.
        if not seg or command_head(seg) != "":
            continue
        for tok in seg:
            m = _ASSIGN_NAME.match(tok)
            if m and m.group(1) in TIED:
                return block(m.group(1))

    return 0


if __name__ == "__main__":
    sys.exit(main())
