#!/usr/bin/env python3
"""SessionStart on `compact`, codex only: name the session's mode and state Skills again.

Compaction replaces the conversation with a summary, and every Skill in it goes
with the conversation. The mode Skill and the state Skill are the two that must
not: they say how the session works, and the gates keep enforcing both axes off
session_state either way.

On Claude, reload_stale_skills owns this: it names every Skill in use after a
compaction, the mode and state Skills included, and it reads Claude's transcript
to do it. Codex has no PostToolBatch and no Claude transcript, so this hook keeps
the mode and state order there.

The directive text keeps its one home in classify_intent. This hook resolves which
Skills govern — `session_mode.resolve` for the mode and `session_mode.state` for
the state axis, the same two answers block_writes and block_spawning gate on — and
reuses it, so the Skill the Agent is sent back to can never be the one it is no
longer being gated by.

Only `compact` fires it. A `resume` restores the whole transcript intact, and a
`startup` or `clear` has no Skill to name again.
"""

import sys

from classify_intent import directive
from lib import feedback
from lib.event import field, read_event
from lib.session_mode import resolve, state

BINDING = {
    "events": {"SessionStart": ["compact"]},
    "harness": "codex",
}

PREAMBLE = ("### Use the Skills the compaction dropped\n"
            "The conversation was compacted and the Skills in it left with it. These "
            "Skills still govern this session.")


def main():
    event = read_event()
    if field(event, "source", "") != "compact":
        return 0
    session_id = field(event, "session_id", "")
    if not session_id:
        return 0
    # Both axes ride one call: `directive` reads the mode twice — as the interview
    # override and as the skill line — so passing it as both arguments is what
    # reaches an interview session, which carries no mode line of its own.
    mode = resolve(event, session_id)
    body = directive(state(event), mode, mode)
    if not body:
        return 0
    feedback.context("inject_mode_skills", "SessionStart", PREAMBLE + "\n\n" + body)
    return 0


if __name__ == "__main__":
    sys.exit(main())
