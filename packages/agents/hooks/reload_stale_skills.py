#!/usr/bin/env python3
"""UserPromptSubmit: put the agent back on a governing Skill it has run past.

A Skill's steps hold at the start of a session and get looser as it runs. The text
is still in the conversation the whole time — the harness answers a second use with
`instructions unchanged` — so nothing has been lost. What decays is adherence, and
what fixes it is being told to use the Process again, which is what the architect
does by hand when he repeats the Skill.

Nothing here reads a list of what governs, because two libraries already answer it.
`transcript.skill_arrivals` says which Skills reached the conversation and where,
covering the ones the architect typed and the ones the agent used itself.
`frontmatter.declared` says what each Skill asks for, as `reload-every: 5 turns` in
its own file. This hook owns one thing: the distance between those two answers.

The window is a preset 100 turns, so a turn is one percent of it and nothing here
counts messages: a session where the architect walks away for twenty hours and one
where he types every minute spend a turn on the same volume of conversation.

A Skill with no `reload-every` is never named again, so a Skill nobody has tuned
costs nothing. Nothing is stored per Skill either: using it writes a fresh arrival,
which is what resets the distance, and ignoring the order leaves it standing next
turn.
"""

import json
import os
import re
import sys

from classify_intent import (SKILLS_DIR, blank_spans, forced_commands,
                             skills_directive, typed_skills)
from lib import feedback, frontmatter, transcript
from lib.event import field, read_event
from lib.session_mode import is_dispatched

BINDING = {
    "events": {"UserPromptSubmit": []},
    "harness": "claude",
}

# One turn of the preset 100-turn window: 10k tokens of conversation, counted as
# characters at the standard four per token, because the transcript carries text.
TURN_CHARS = 40000

COMPACT_MARKER = "This session is being continued"

_LEADING_COUNT = re.compile(r"\d+")


def reload_every(name):
    """The Skill's distance in turns, or 0 when it is never named again.

    The value carries its own unit (`5 turns`) and only the number is read, so
    `never` and a missing key answer the same 0.
    """
    try:
        with open(os.path.join(SKILLS_DIR, name, "SKILL.md"), encoding="utf-8") as fh:
            declared = frontmatter.declared(fh.read(), "reload-every")
    except OSError:
        return 0
    count = _LEADING_COUNT.match(declared or "")
    return int(count.group(0)) if count else 0


def live_records(recs):
    """The records still in the conversation: everything after the last compaction.

    A compaction leaves the old records in the transcript file while dropping them
    from the conversation, so measuring across that boundary would find an arrival
    for a Skill the agent can no longer see.
    """
    cut = 0
    for i, record in enumerate(recs):
        if transcript.text_of(record).startswith(COMPACT_MARKER):
            cut = i + 1
    return recs[cut:]


def chars_since(recs, index):
    """Characters of conversation after `index`.

    Whole records, not their text: a turn's tool results and thinking occupy the
    same context the steps have to compete with, and measuring text alone read this
    session as three turns deep when it was past forty.
    """
    return sum(len(json.dumps(r, separators=(",", ":"))) for r in recs[index + 1:])


def named_this_turn(prompt):
    """The Skills classify_intent is already ordering on this same event.

    Both hooks fire on one UserPromptSubmit and the architect's typed /<name>
    expands into the conversation after it, so without this a Skill he just typed
    is ordered twice in one turn.
    """
    scanned = blank_spans(prompt)
    forced_state, forced_mode, _ = forced_commands(scanned)
    named = {token.lstrip("/") for token in typed_skills(scanned)}
    for forced in (forced_state, forced_mode):
        if forced:
            named.add(forced)
    return named


def overdue(recs, skip):
    out = []
    for name, index in transcript.skill_arrivals(recs).items():
        if name in skip:
            continue
        turns = reload_every(name)
        if turns and chars_since(recs, index) >= turns * TURN_CHARS:
            out.append("/" + name)
    return out


def main():
    event = read_event()

    # A dispatched agent gets its Skills at dispatch, and its Process is the one its
    # founding prompt named, not the session's.
    session_id = field(event, "session_id", "")
    if not session_id or is_dispatched(event):
        return 0

    prompt = field(event, "prompt", "")
    if not prompt or transcript.harness_authored(prompt):
        return 0

    # No readable transcript is no measurement, and unmeasured is not overdue.
    recs = transcript.records(field(event, "transcript_path", ""))
    if not recs:
        return 0

    due = overdue(live_records(recs), named_this_turn(prompt))
    if due:
        feedback.context("reload_stale_skills", "UserPromptSubmit", skills_directive(due))
    return 0


if __name__ == "__main__":
    sys.exit(main())
