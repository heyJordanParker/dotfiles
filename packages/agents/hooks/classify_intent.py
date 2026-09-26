#!/usr/bin/env python3
"""Apply typed commands to session state and inject their directives.

On every UserPromptSubmit, typed mode-commands map straight to control state
  (/propose /execute force STATE; /orchestrate /build /interview force MODE, with
  /orchestrate and /build also entering the executing state; /commit forces a
  commit), persisted to session state and echoed as the matching skill-load
  directive when the command was typed or the state or mode changed.

State and mode stay manual. Any infrastructure failure returns 0 and never
blocks the prompt.
"""

import os
import re
import sys

from lib import feedback, frontmatter, transcript
from lib.event import field, read_event
from lib.session_mode import is_dispatched, resolve
from lib.session_state import load_state, merge_state

BINDING = {
    "events": {"UserPromptSubmit": []},
    "timeout": 60,
    "harness": "all",
    "standalone": True,
}

def emit_context(text):
    feedback.context("classify_intent", "UserPromptSubmit", text)


# Machine-authored text reaching UserPromptSubmit — a task notification, an
# injected block, a stop-gate's own feedback replayed back, a relayed message from
# another session. Each carries whatever text it quotes, so a /propose inside one
# must never write state. `transcript.harness_authored` owns the test; the
# uploader gates on the same call, so one definition governs what counts as the
# architect speaking.
is_system_prompt = transcript.harness_authored


# --- typed mode-commands (deterministic) ---------------------------------------

_FORCED_STATE = {"/propose": "propose", "/execute": "execute"}
_FORCED_MODE = {"/orchestrate": "orchestrate", "/build": "build", "/interview": "interview"}

# The architect types his commands anywhere, phrased naturally: "sure, /execute &
# /commit after", or mid-sentence on a later line. So every slash token counts, on
# any line at any position, minus backticked and double-quoted spans, which are
# discussion about a command rather than a mode switch.
_ANY_TOKEN = re.compile(r"(?:^|\s)(/[a-z][a-z0-9-]*)")

_FENCED_SPAN = re.compile(r"```.*?```", re.DOTALL)
# Inline delimiters pair only within one line, so a stray unpaired backtick or
# quote can never swallow a command typed on a later line.
_INLINE_SPAN = re.compile(r"`[^`\n]*`|\"[^\"\n]*\"")

# Spans blank to a non-whitespace filler: blanking to spaces would manufacture a
# whitespace-preceded token out of a glued path ("see`x`/execute", "foo"/propose).
_SPAN_FILLER = "#"


def _fill(match):
    return "".join("\n" if ch == "\n" else _SPAN_FILLER for ch in match.group(0))


def blank_spans(prompt):
    return _INLINE_SPAN.sub(_fill, _FENCED_SPAN.sub(_fill, prompt))


def leading_commands(prompt):
    return _ANY_TOKEN.findall(prompt)


def forced_commands(prompt):
    forced_state = ""
    forced_mode = ""
    forced_commit = False
    # Last typed command wins, so a corrected mode later in the message holds.
    for token in leading_commands(prompt):
        if token in _FORCED_STATE:
            forced_state = _FORCED_STATE[token]
        elif token in _FORCED_MODE:
            forced_mode = _FORCED_MODE[token]
        elif token == "/commit":
            forced_commit = True
    return forced_state, forced_mode, forced_commit


def directive(forced_state, forced_mode, governing_mode=None):
    """The skill-load directives for one turn's control axes.

    Called on a typed command, and after a compaction drops the skills a live
    session is still gated by: by reload_stale_skills on Claude and by
    inject_mode_skills on codex.

    The mode line names `governing_mode` — what lib.session_mode.resolve answers
    for this event after the write landed — so the skill the agent uses and the
    mode the gates enforce are read off one policy. Announcing the typed word
    instead let a turn that typed only /execute name the previous mode's skill.
    """
    if governing_mode is None:
        governing_mode = forced_mode
    out = ""
    if forced_state == "execute":
        out = ("This is an executing-state turn. Use /execute now and work under its "
               "contract: implement the approved work, and the moment it needs an "
               "architectural change, stop and escalate to the architect.")
    elif forced_state == "propose":
        out = ("This is a proposing-state turn. Use /propose now and produce the "
               "proposal under its contract.")
    if forced_mode == "interview":
        out = ("This is an interview turn. Use /interview now and interview the "
               "architect under its contract.")
    mode_line = {
        "orchestrate": "Use /orchestrate now.",
        "build": "Use /build now.",
    }.get(governing_mode, "")
    if mode_line:
        out = (out + "\n\n" + mode_line) if out else mode_line
    return out


COMMIT_DIRECTIVE = "Skills to execute: /commit"

WRITE_FAILED_NOTICE = ("The typed command could not be applied: the session state "
                       "write failed. The mode is unchanged. Tell the architect "
                       "before doing anything else.")


# --- typed skills (deterministic) ----------------------------------------------

SKILLS_DIR = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "skills")
_COMMANDS_DIR = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "commands")

# The mode/commit commands above carry richer state/mode directives; the
# general scan skips them so it never double-handles one.
_SPECIAL_SKILLS = {"/propose", "/execute", "/interview", "/orchestrate", "/build", "/commit"}

# A skill token counts only when its slash follows start-or-whitespace, so a name
# embedded in a path (.../skills/architecture) never matches.
_SKILL_TOKEN = re.compile(r"(?:^|\s)/([a-z][a-z0-9-]*)")


def _available_skills():
    names = set()
    for directory, suffix in ((SKILLS_DIR, None), (_COMMANDS_DIR, ".md")):
        try:
            entries = os.listdir(directory)
        except OSError:
            continue
        for entry in entries:
            if suffix is None:
                if os.path.isdir(os.path.join(directory, entry)):
                    names.add(entry)
            elif entry.endswith(suffix):
                names.add(entry[:-len(suffix)])
    return names


def _skill_disables_model_invocation(name):
    """True when skills/<name>/SKILL.md sets `disable-model-invocation: true`.

    Such a skill reaches context only via the harness slash-dispatch when the user
    types `/<name>`; the Skill tool refuses it. The directive routes the model
    through the Skill tool, so for these skills it dead-ends on a failing call —
    skip it. Commands (no SKILL.md) never carry the flag, so they're unaffected.
    """
    try:
        with open(os.path.join(SKILLS_DIR, name, "SKILL.md"), encoding="utf-8") as fh:
            declared = frontmatter.declared(fh.read(), "disable-model-invocation")
    except OSError:
        return False
    return (declared or "").lower() == "true"


def typed_skills(prompt):
    available = _available_skills()
    found = []
    seen = set()
    for match in _SKILL_TOKEN.finditer(prompt):
        token = "/" + match.group(1)
        if token in _SPECIAL_SKILLS or match.group(1) not in available or token in seen:
            continue
        seen.add(token)
        if _skill_disables_model_invocation(match.group(1)):
            continue
        found.append(token)
    return found


def skills_directive(skills):
    """The order that puts the agent back on a Skill, whatever asked for it.

    The Skill is named the way anyone names one, `/<name>`, because that is how
    every Prompt in this repository refers to a Skill and the agent needs no other
    handle. A second use answers `instructions unchanged` rather than the text,
    which is the harness saying the Process is still in the conversation: what the
    order buys is the agent going back to it, not the bytes arriving twice.

    An order only. Naming who typed it, or how far the session has run since, hands
    the agent a fact where an instruction belongs. reload_stale_skills emits this
    same sentence, so one wording covers both callers.
    """
    contract = "its contract governs" if len(skills) == 1 else "their contracts govern"
    return ("Use %s now, before anything else, so %s this turn."
            % (", ".join(skills), contract))


def draft_directive():
    """The order to draft before replying, when THINK_BEFORE_TALKING is on.

    /present carries the same instruction, and a measured `cld -p` run showed the
    agent skipping it: a single-shot turn answers straight from thinking. The
    directive arrives with the turn instead. Empty while the flag is unset, so the
    turn's injected context is what it was before this existed.
    """
    if os.environ.get("THINK_BEFORE_TALKING") != "1":
        return ""
    return ("### Write think.md before the reply\n"
            "Write this turn's findings and the decisions you have settled to "
            "docs/agents/<NNN>-<task-slug>/think.md in the run's Evidence "
            "directory, then write the reply from that file. Replace the whole "
            "file so it holds this turn only, never appended to an earlier "
            "turn's. This file is your own working document, so writing it is "
            "never the acting a question turn withholds. It records what you "
            "did and settled, never what you would do: a decision you write "
            "there is one you carry out this turn.")


def main():
    # Guard against recursion: the model call runs a nested harness process.
    if os.environ.get("CLAUDE_SESSION_HOOK") == "true":
        return 0

    event = read_event()

    session_id = field(event, "session_id", "")
    # A dispatched agent gets its task from its dispatcher, not from a typed
    # command, so it is skipped. The architect's hand-managed teammates are not
    # dispatched — they carry an agentId and nothing else — and skipping those
    # dropped every mode command they typed, leaving session state on the mode the
    # roster declared while the statusline showed what he asked for.
    if not session_id or is_dispatched(event):
        return 0

    prompt = field(event, "prompt", "")
    if not prompt:
        return 0

    if is_system_prompt(prompt):
        return 0

    # One blanking feeds both deterministic scans, so a quoted sentence can never
    # count as a mode command for one scan and a typed skill for the other.
    scanned = blank_spans(prompt)
    forced_state, forced_mode, forced_commit = forced_commands(scanned)

    # Ensure the session exists and apply any typed mode-commands. The commit
    # authorization is written on every human turn, not only when granted, so it
    # expires with the turn that typed /commit instead of latching for the session.
    # A <task-notification> wake-up is a system prompt and returns above, so async
    # work inside the granting turn keeps it; the next human turn revokes it.
    update = {"commit_requested": forced_commit}
    if forced_mode:
        update["mode"] = forced_mode
        update["mode_typed"] = True
    # /orchestrate and /build name how the architect wants the work done, which is
    # already the approval to do it — "orchestrate agents to do this /execute" was
    # him typing the second half every time. A /propose typed in the same message
    # is him asking for the proposal instead, and it wins. /interview is left out:
    # it is the mode that produces no work.
    if forced_mode in ("orchestrate", "build") and not forced_state:
        forced_state = "execute"
    if forced_state:
        update["state"] = forced_state
    stored = merge_state(session_id, update)

    # Deterministic context: the skill-load directives for the session's governing
    # state and mode, sent on a typed command and whenever the pair differs from the
    # one last announced, so the session's first turn loads the default propose Skill
    # and an unchanged session hears nothing. Announcing a mode the write never stored
    # leaves the agent working under one mode while the gates enforce the other, so
    # the directive rides only on a confirmed write.
    if stored:
        governing_mode = resolve(event, session_id)
        state = load_state(session_id)
        governing_state = forced_state or state.get("state") or "propose"
        # An interview session produces questions, not state work, so only a typed
        # state command names a state Skill there.
        if governing_mode == "interview" and not forced_state:
            governing_state = ""
        announced = [governing_state, governing_mode]
        context = ""
        if forced_state or forced_mode or state.get("announced") != announced:
            context = directive(governing_state, forced_mode, governing_mode)
            merge_state(session_id, {"announced": announced})
        if forced_commit:
            context = (context + "\n\n" + COMMIT_DIRECTIVE) if context else COMMIT_DIRECTIVE
    else:
        context = WRITE_FAILED_NOTICE if (forced_state or forced_mode or forced_commit) else ""

    # The draft order, when the flag is on.
    draft = draft_directive()
    if draft:
        context = (context + "\n\n" + draft) if context else draft

    if context:
        emit_context(context)
    return 0


if __name__ == "__main__":
    sys.exit(main())
