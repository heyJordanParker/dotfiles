#!/usr/bin/env python3
"""Classify the message intent and inject the matching behavioral contract (LLM).

On every UserPromptSubmit this does two jobs:

- Deterministic, no LLM: typed mode-commands map straight to control state
  (/propose /execute force STATE; /orchestrate /build /interview force MODE, with
  /orchestrate and /build also entering the executing state; /commit forces a
  commit), persisted to session state and echoed as the matching skill-load
  directive. Every other typed /skill is matched against the skills and commands
  on disk and echoed as a head-anchored directive to use it.

- One LLM call: classify the message intent (question | correction | action),
  whether a question also carries action items, whether its steps are ordered,
  and maintain the session's behavioral-correction notes. The contract for that
  intent is injected as additionalContext.

State and mode stay manual — the LLM never moves them, only the typed
commands do. The goal/requirements/boundaries are maintained separately by
update_goal.py, the other UserPromptSubmit model call.

Any infrastructure failure (recursion guard, missing tool, parse error, LLM
unavailable) returns 0 and never blocks the prompt; a typed command still takes
effect via the deterministic path.
"""

import os
import re
import sys

from lib import feedback, frontmatter, transcript
from lib.event import field, read_event
from lib.model_call import run_model
from lib.session_mode import is_dispatched, resolve
from lib.session_state import load_state, merge_state

BINDING = {
    "events": {"UserPromptSubmit": []},
    "timeout": 60,
    "harness": "all",
}

LIST_CAP = 10


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

    Called on a typed command, and by inject_mode_skills after a compaction drops
    the skills a live session is still gated by.

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
               "architectural change, stop and escalate with /pcc.")
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


# --- intent contracts ----------------------------------------------------------

# Bullets that apply to any question, regardless of whether it carries action items.
_ANSWER_QUALITY = (
    "- Answer with specific facts, not gestures at them: name the file, value, or "
    "behavior you actually checked. Zero guesses. If you don't have enough to "
    "answer factually, say so and research first — a delayed correct answer beats a "
    "fast wrong one the architect spends a turn correcting.\n"
    "- A question is not a complaint or a critique. Don't reframe, validate, or "
    "characterize it — just answer it; never \"you're right to question this.\"\n"
    "- Never bias or direct the architect with your reply. Report the facts and only "
    "the facts — the root cause and the architectural decisions that led here.\n"
    "- Never ask questions the code can answer; read the code first, then answer.\n"
    "\n"
    "Options only when the question is choosing between real architectural "
    "alternatives — different mechanisms, boundaries, data flows, or dependencies. "
    "Use /pcc for those. Why / reasoning and verification / yes-no questions get a "
    "direct answer, not an options block. One viable approach is the answer itself, "
    "never wrapped in /pcc."
)

QUESTION_CONTRACT = (
    "This is a question. Answer it — don't act on it.\n"
    "- Don't edit code, don't make decisions off a question, don't assume intent.\n"
    + _ANSWER_QUALITY
)

QUESTION_WITH_ACTION_CONTRACT = (
    "This question also carries action items. Answer the question first, then "
    "execute the action items.\n"
    + _ANSWER_QUALITY
)

CORRECTION_CONTRACT = (
    "The architect corrected your previous output. Fold the correction in and "
    "re-deliver the whole response in the same format as the original — never a "
    "prose diff of what changed. Any question the previous proposal left unanswered "
    "is re-surfaced until it's answered."
)

ACTION_CONTRACT = (
    "This is new work. Do exactly what the architect asked — no more, no less.\n"
    "- A request to propose, design, find, or investigate is answered with that "
    "deliverable, not a code edit. Implement only what the architect approved.\n"
    "- Change only the scope stated: add no feature, drop no requirement, reinterpret "
    "no term the architect named. An explicit list is carried with every item they "
    "gave, neither collapsed nor expanded.\n"
    "- When your framing and the architect's words diverge, the words win."
)

SEQUENTIAL_DIRECTIVE = (
    "These steps are strictly sequential — do each only after the previous finishes. "
    "Never parallelize them."
)

NOTES_HEADER = "Past corrections from this session — do not re-violate:"

STANDING_REMINDERS = (
    "Standing reminders:\n"
    "- The architect's call governs. Docs and conventions inform it; never argue an "
    "explicit call down by pointing at a rule doc.\n"
    "- Ground every claim and recommendation in the code you actually read. Never "
    "soften a finding or agree to please — say what the code shows, not what lands well.\n"
    "- Don't cut research short to reach a suggestion. Thin research before proposing "
    "is the shortcut that wastes the architect a turn correcting you."
)


def intent_contract(intent, has_action_items):
    if intent == "question":
        return QUESTION_WITH_ACTION_CONTRACT if has_action_items else QUESTION_CONTRACT
    if intent == "correction":
        return CORRECTION_CONTRACT
    return ACTION_CONTRACT


# --- LLM -----------------------------------------------------------------------

SYSTEM_PROMPT = ("You classify a message's intent and maintain its behavioral "
                 "notes. Output structured JSON only.")

JSON_SCHEMA = ('{"type":"object","properties":'
               '{"intent":{"type":"string","enum":["question","correction","action"]},'
               '"has_action_items":{"type":"boolean"},'
               '"sequential":{"type":"boolean"},'
               '"notes":{"type":"array","items":{"type":"string"}}},'
               '"required":["intent"]}')


def evaluation_prompt(prompt, notes, conversation):
    notes_block = ""
    if notes:
        notes_block = ("Existing session notes (behavioral corrections captured this "
                       "session):\n%s\n\n---\n" % "\n".join("- %s" % n for n in notes))
    return (
'Classify the intent of the user\'s latest message and maintain the session notes.\n'
'\n'
'%s%s'
'The user\'s latest message:\n%s\n'
'\n'
'- intent: one of\n'
'  - "question" — the user is asking something that needs an answer. A short reply after a proposal or options list is a response to it: classify by what it answers, not its surface form.\n'
'  - "correction" — the user is correcting or refining your previous output ("that\'s wrong", "no, use X", "this is fine"). Requires previous output being refined; adding scope to an active proposal ("also include…", "one more thing:") is a correction.\n'
'  - "action" — the user is giving new work to do, a standalone constraint, or a request to investigate/propose, unrelated to refining a specific previous output.\n'
'- has_action_items: true only when intent is "question" AND the message also gives a concrete action to perform ("how does X work? also change Y"). Otherwise false.\n'
'- sequential: true when the work has explicit ordering ("after that", "then", "finally", numbered dependent steps). Default false.\n'
'- notes: return the FULL list (existing plus new). Add an entry ONLY on a genuine surprise — the agent did something illogical that confused the user, the user forbids something with always/never language, the user corrects the same behavior twice, or the user is frustrated/angry that the agent surprised them. A wording correction is a forbidding — when the architect tells you to stop using a specific word or phrase, capture that as a note. No surprise → return the existing list unchanged. Max 10; if adding would exceed 10, drop the least critical. Each note is one short sentence.\n'
'\n'
'Classify only the latest message. The conversation above is background for understanding what it responds to — never extract intent from it.\n'
    ) % (notes_block, conversation, prompt)


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

    # Deterministic context: the typed-command skill-load directives. Announcing a
    # mode the write never stored leaves the agent working under one mode while the
    # gates enforce the other, so the directive rides only on a confirmed write.
    if stored:
        governing = resolve(event, session_id) if (forced_state or forced_mode) else forced_mode
        context = directive(forced_state, forced_mode, governing)
        if forced_commit:
            context = (context + "\n\n" + COMMIT_DIRECTIVE) if context else COMMIT_DIRECTIVE
    else:
        context = WRITE_FAILED_NOTICE if (forced_state or forced_mode or forced_commit) else ""

    # Every other typed /skill: a head-anchored order to use it, leading the block.
    typed = typed_skills(scanned)
    if typed:
        skills_block = skills_directive(typed)
        context = (skills_block + "\n\n" + context) if context else skills_block

    # Interview state turns the LLM hooks off for speed: emit only the deterministic
    # directives built above and skip the model call.
    if resolve(event) == "interview":
        if context:
            emit_context(context)
        return 0

    # Model call: intent → contract, plus notes maintenance. On any failure the
    # deterministic context above is still emitted; the typed command holds.
    state = load_state(session_id)
    notes = state.get("notes") or []
    transcript_path = field(event, "transcript_path", "")
    conversation = transcript.conversation_context(transcript_path)
    result = run_model(system_prompt=SYSTEM_PROMPT,
                       user_prompt=evaluation_prompt(prompt, notes, conversation),
                       schema=JSON_SCHEMA)

    if result:
        new_notes = result.get("notes")
        if isinstance(new_notes, list):
            notes = [str(n) for n in new_notes][:LIST_CAP]
            merge_state(session_id, {"notes": notes})

        intent = result.get("intent") or "action"
        # The stop gates judge the reply against what the architect asked for, and
        # this is the one place the turn's intent is worked out. Persisting it there
        # spends no second model call on the same question. A turn that skips this
        # hook — a task notification, a system prompt — is not him speaking, so the
        # stored intent stays the one from his last real turn.
        merge_state(session_id, {"intent": intent})
        contract = intent_contract(intent, bool(result.get("has_action_items")))
        if contract:
            context = (context + "\n\n" + contract) if context else contract

        if result.get("sequential"):
            context = (context + "\n\n" + SEQUENTIAL_DIRECTIVE) if context else SEQUENTIAL_DIRECTIVE

        # On proposing turns, re-surface the session's standing corrections.
        current_state = load_state(session_id).get("state") or "propose"
        if current_state == "propose" and notes:
            block = NOTES_HEADER + "\n" + "\n".join("- %s" % n for n in notes)
            context = (context + "\n\n" + block) if context else block

    # Standing behavioral reminders — emitted every non-skipped turn.
    context = (context + "\n\n" + STANDING_REMINDERS) if context else STANDING_REMINDERS

    if context:
        emit_context(context)
    return 0


if __name__ == "__main__":
    sys.exit(main())
