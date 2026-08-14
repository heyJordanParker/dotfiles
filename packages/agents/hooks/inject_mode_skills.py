#!/usr/bin/env python3
"""SessionStart on `compact`: put the session's mode and state Skills back.

Compaction replaces the conversation with a summary, and every Skill loaded into
it goes with the conversation. The mode Skill and the state Skill are the two
that must not: they say how the session works, and the gates keep enforcing both
axes off session_state either way.

Nothing else was reloading them. `classify_intent.directive` emits the load
directive only on a turn that types /orchestrate, /build, /interview, /propose or
/execute — every other turn resolves an empty mode and emits nothing — so the
Skills entered context once per typed command and never came back. The
descriptions of `orchestrate`, `build`, `propose` and `execute` already name this
reload.

The directive text keeps its one home in classify_intent. This hook resolves
which Skills govern — `session_mode.resolve` for the mode and
`session_mode.state` for the state axis, the same two answers block_writes and
block_spawning gate on — and reuses it, so the Skill the Agent reloads can never
be the one it is no longer being gated by.

Only `compact` fires it. A `resume` restores the whole transcript with its Skill
loads intact, and a `startup` or `clear` has no Skill to put back.
"""

import sys

from classify_intent import directive
from lib import feedback
from lib.event import field, read_event
from lib.session_mode import resolve, state

BINDING = {
    "events": {"SessionStart": ["compact"]},
    "harness": "all",
}

PREAMBLE = ("### Reload the Skills the compaction dropped\n"
            "The conversation was compacted and the Skills loaded in it left context. "
            "These Skills still govern this session.")


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
