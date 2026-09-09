#!/usr/bin/env python3
"""Put the agent back on a governing Skill it has run past, before every model request.

A Skill's steps hold at the start of a session and get looser as it runs. The text
is still in the conversation the whole time — the harness answers a second use with
`instructions unchanged` — so nothing has been lost. What decays is adherence, and
what fixes it is being told to use the Process again, which is what the architect
does by hand when he repeats the Skill.

The model is called after a prompt arrives or after a tool batch resolves, and the
hook runs on both, so a session the architect walked away from is measured the same
as one he types into: 70% of every transcript byte sits between two of his prompts,
and the turns in between are task notifications, SendMessage deliveries and tool
batches. A compaction gets its own order, because the harness restores a used Skill
only as background text, cut at 5,000 tokens: every Skill in use is named again so
its next use is a live invocation. Claude only: PostToolBatch is Claude's event and
the transcript shape is Claude's; inject_mode_skills keeps the compaction order on
codex.

Nothing here reads a list of what governs, because two libraries already answer it.
`transcript.skill_arrivals` says which Skills reached the conversation and where,
covering the ones the architect typed, the ones the agent used itself, and the ones
this hook ordered — an order counts, so an overdue Skill is ordered once per
distance, not once per request. `frontmatter.declared` says what each Skill asks
for, as `reload-every: 5 turns` in its own file. This hook owns one thing: the
distance between those two answers.

The window is a preset 100 turns, so a turn is one percent of it and nothing here
counts messages: a session where the architect walks away for twenty hours and one
where he types every minute spend a turn on the same volume of conversation.

A Skill with no `reload-every` is never named again by distance, so a Skill nobody
has tuned costs nothing. Nothing is stored per Skill either: using it writes a fresh
arrival, which is what resets the distance.
"""

import json
import os
import re
import sys

from classify_intent import (SKILLS_DIR, _skill_disables_model_invocation, blank_spans,
                             directive, forced_commands, skills_directive, typed_skills)
from lib import feedback, frontmatter, transcript
from lib.event import field, read_event
from lib.session_mode import is_dispatched, resolve, state

BINDING = {
    "events": {"UserPromptSubmit": [], "PostToolBatch": [], "SessionStart": ["compact"]},
    "harness": "claude",
}

# One turn of the preset 100-turn window: 10k tokens of conversation, counted as
# characters at the standard four per token, because the transcript carries text.
TURN_CHARS = 40000

COMPACT_PREAMBLE = ("### Use the Skills the compaction dropped\n"
                    "The conversation was compacted and the Skills in it left with it. These "
                    "Skills still govern this session.")

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
    for a Skill the agent can no longer see. At `SessionStart: compact` the new
    boundary is not in the file yet, so the whole window that just closed is what
    this returns — which is exactly the set of Skills in use.
    """
    cut = 0
    for i, record in enumerate(recs):
        if transcript.text_of(record).startswith(transcript.COMPACT_MARKER):
            cut = i + 1
    return recs[cut:]


def chars_after(recs):
    """chars_after[i]: characters of conversation after record i.

    Whole records, not their text: a turn's tool results and thinking occupy the
    same context the steps have to compete with, and measuring text alone read this
    session as three turns deep when it was past forty. Serialized once, however
    many Skills are measured, because this runs on every tool batch.
    """
    sizes = [len(json.dumps(r, separators=(",", ":"))) for r in recs]
    out = [0] * len(recs)
    total = 0
    for i in range(len(recs) - 1, -1, -1):
        out[i] = total
        total += sizes[i]
    return out


def named_this_turn(prompt):
    """The Skills classify_intent is already ordering on this same event.

    Both hooks fire on one UserPromptSubmit and the architect's typed /<name>
    expands into the conversation after it, so without this a Skill he just typed
    is ordered twice in one turn. A machine-authored prompt names nothing here:
    classify_intent returns on it before ordering anything.
    """
    if not prompt or transcript.harness_authored(prompt):
        return set()
    scanned = blank_spans(prompt)
    forced_state, forced_mode, _ = forced_commands(scanned)
    named = {token.lstrip("/") for token in typed_skills(scanned)}
    for forced in (forced_state, forced_mode):
        if forced:
            named.add(forced)
    return named


def overdue(recs, skip):
    after = chars_after(recs)
    out = []
    for name, index in transcript.skill_arrivals(recs).items():
        if name in skip:
            continue
        turns = reload_every(name)
        if turns and after[index] >= turns * TURN_CHARS:
            out.append("/" + name)
    return out


def in_use(recs):
    """The Skills the architect typed or the agent used in these records.

    An order of this hook's own is not a use: a Skill the agent stops obeying falls
    out at the next compaction instead of being carried forever. A Skill the agent
    cannot use through the Skill tool is left out, or the order dead-ends.
    """
    return [name for name in transcript.skill_arrivals(recs, orders=False)
            if not _skill_disables_model_invocation(name)]


def compact_order(event, session_id, recs):
    """Every Skill in use, named again in one block after a compaction.

    The mode and state Skills ride `classify_intent.directive`, read off session
    state the way the gates read it, so a mode Skill the agent never happened to
    use is still named, transcript or no transcript; every other in-use Skill
    follows in one order.
    """
    mode = resolve(event, session_id)
    governing_state = state(event)
    body = directive(governing_state, mode, mode)
    rest = [name for name in in_use(live_records(recs))
            if name not in (governing_state, mode)]
    parts = [part for part in (body, skills_directive(["/" + n for n in rest]) if rest else "")
             if part]
    if parts:
        feedback.context("reload_stale_skills", "SessionStart",
                         COMPACT_PREAMBLE + "\n\n" + "\n\n".join(parts))
    return 0


def main():
    event = read_event()

    # A dispatched agent gets its Skills at dispatch, and its Process is the one its
    # founding prompt named, not the session's.
    session_id = field(event, "session_id", "")
    if not session_id or is_dispatched(event):
        return 0

    hook = field(event, "hook_event_name", "")
    recs = transcript.records(field(event, "transcript_path", ""))

    if hook == "SessionStart":
        if field(event, "source", "") != "compact":
            return 0
        return compact_order(event, session_id, recs)

    # No readable transcript is no measurement, and unmeasured is not overdue.
    if not recs:
        return 0
    due = overdue(live_records(recs), named_this_turn(field(event, "prompt", "")))
    if due:
        feedback.context("reload_stale_skills", hook, skills_directive(due))
    return 0


if __name__ == "__main__":
    sys.exit(main())
