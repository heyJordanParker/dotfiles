#!/usr/bin/env python3
"""Classify the message intent and inject the matching behavioral contract (LLM).

On every UserPromptSubmit this does two jobs:

- Deterministic, no LLM: typed mode-commands map straight to control state
  (/propose /execute force STATE; /solo /subagents force APPROACH; /commit
  forces a commit), persisted to the session spine and echoed as the matching
  skill-load directive. Every other typed /skill is matched against the skills
  and commands on disk and echoed as a head-anchored reload directive.

- One LLM call: classify the message intent (question | correction | action),
  whether a question also carries action items, whether its steps are ordered,
  and maintain the session's behavioral-correction notes. The contract for that
  intent is injected as additionalContext.

State and approach stay manual — the LLM never moves them, only the typed
commands do. The goal/requirements/boundaries are maintained separately by
update_goal.py, the other UserPromptSubmit model call.

Any infrastructure failure (recursion guard, missing tool, parse error, LLM
unavailable) returns 0 and never blocks the prompt; a typed command still takes
effect via the deterministic path.
"""

import os
import re
import sys

from lib import feedback, transcript
from lib.event import field, read_event
from lib.model_call import run_model
from lib.session_state import load_state, merge_state

BINDING = {
    "events": {"UserPromptSubmit": []},
    "timeout": 60,
    "harness": "all",
}

LIST_CAP = 10


def emit_context(text):
    feedback.context("classify_intent", "UserPromptSubmit", text)


def is_system_prompt(prompt):
    # XML-tagged: starts with <tag>, contains matching </tag>
    if prompt.startswith("<"):
        tag_rest = prompt[1:]
        tag_name = re.split(r"[> ]", tag_rest, maxsplit=1)[0]
        if tag_name and ("</%s>" % tag_name) in prompt:
            return True
    # Bracket-enclosed: entire message is a single [...] line
    if prompt.startswith("["):
        first_line = prompt.split("\n", 1)[0]
        if first_line.endswith("]") and prompt == first_line:
            return True
    if prompt.startswith("This session is being continued"):
        return True
    if prompt.startswith("Base directory for this skill:"):
        return True
    # Machine-authored replays: a stop-gate's own feedback and a relayed message
    # from another session both come back through UserPromptSubmit carrying
    # whatever text they quote, so a /propose inside them must never write state.
    if prompt.startswith("Stop hook feedback:"):
        return True
    if prompt.startswith("Another Claude session sent a message:"):
        return True
    return False


def is_subagent_prompt(event):
    """True when the payload belongs to a subagent turn.

    A Claude subagent's payload carries the parent's UUID in session_id, so the id
    can't tell them apart; the sidechain markers can. Both spellings are checked
    because the harness snake-cases its own payload keys and passes the transcript
    record's camelCase keys through unchanged.
    """
    for key in ("isSidechain", "is_sidechain", "agentId", "agent_id"):
        if field(event, key, ""):
            return True
    return False


# --- typed mode-commands (deterministic) ---------------------------------------

_FORCED_STATE = {"/propose": "proposing", "/execute": "executing", "/interview": "interview"}
_FORCED_APPROACH = {"/solo": "solo", "/subagents": "subagents"}

# A line's leading run of slash tokens: "/execute /commit fix the parser" types two
# commands, "okay /execute" types none. Anywhere past that run — mid-sentence,
# quoted, backticked, inside a pasted report or a numbered list — the token is
# discussion about the command, not the architect switching mode.
_LEADING_RUN = re.compile(r"^[ \t]*((?:/[a-z][a-z0-9-]*(?:[ \t]+|$))+)", re.M)


def leading_commands(prompt):
    found = []
    for match in _LEADING_RUN.finditer(prompt):
        found.extend(match.group(1).split())
    return found


def forced_commands(prompt):
    forced_state = ""
    forced_approach = ""
    forced_commit = False
    # Last typed command wins, so a corrected mode later in the message holds.
    for token in leading_commands(prompt):
        if token in _FORCED_STATE:
            forced_state = _FORCED_STATE[token]
        elif token in _FORCED_APPROACH:
            forced_approach = _FORCED_APPROACH[token]
        elif token == "/commit":
            forced_commit = True
    return forced_state, forced_approach, forced_commit


def directive(forced_state, forced_approach):
    out = ""
    if forced_state == "executing":
        out = ("This is an executing-state turn. Load the /execute skill now and "
               "work under its contract: implement the approved work, and the moment "
               "it needs an architectural change, stop and escalate with /pcc.")
    elif forced_state == "proposing":
        out = ("This is a proposing-state turn. Load the /propose skill now and "
               "produce the proposal under its contract.")
    elif forced_state == "interview":
        out = ("This is an interview turn. Load the interview skill now and interview "
               "the architect one question at a time.")
    approach_line = {
        "solo": "Load the /solo skill now.",
        "subagents": "Load the /subagents skill now.",
    }.get(forced_approach, "")
    if approach_line:
        out = (out + "\n\n" + approach_line) if out else approach_line
    return out


COMMIT_DIRECTIVE = "Skills to execute: /commit"

WRITE_FAILED_NOTICE = ("The typed command could not be applied: the session state "
                       "write failed. The mode is unchanged. Tell the architect "
                       "before doing anything else.")


# --- typed skills (deterministic) ----------------------------------------------

_SKILLS_DIR = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "skills")
_COMMANDS_DIR = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "commands")

# The mode/commit commands above carry richer state/approach directives; the
# general scan skips them so it never double-handles one.
_SPECIAL_SKILLS = {"/propose", "/execute", "/interview", "/solo", "/subagents", "/commit"}

# A skill token counts only when its slash follows start-or-whitespace, so a name
# embedded in a path (.../skills/architecture) never matches.
_SKILL_TOKEN = re.compile(r"(?:^|\s)/([a-z][a-z0-9-]*)")


def _available_skills():
    names = set()
    for directory, suffix in ((_SKILLS_DIR, None), (_COMMANDS_DIR, ".md")):
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

    Such a skill loads only via the harness slash-dispatch when the user types
    `/<name>`; the Skill tool refuses it. The reload directive routes the model
    through the Skill tool, so for these skills it dead-ends on a failing call —
    skip it. Commands (no SKILL.md) never carry the flag, so they're unaffected.
    """
    try:
        with open(os.path.join(_SKILLS_DIR, name, "SKILL.md"), encoding="utf-8") as fh:
            lines = fh.read().splitlines()
    except OSError:
        return False
    if not lines or lines[0].strip() != "---":
        return False
    for line in lines[1:]:
        if line.strip() == "---":
            return False
        match = re.match(r"disable-model-invocation:\s*(.+)$", line)
        if match:
            return match.group(1).strip().strip("'\"").lower() == "true"
    return False


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
    each = "it" if len(skills) == 1 else "each"
    return ("The architect typed %s this turn. Load %s now via the Skill tool, before "
            "anything else — even if already loaded earlier this session; reload for a "
            "fresh copy so its contract governs this turn." % (", ".join(skills), each))


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
    if not session_id or is_subagent_prompt(event):
        return 0

    prompt = field(event, "prompt", "")
    if not prompt:
        return 0

    if is_system_prompt(prompt):
        return 0

    forced_state, forced_approach, forced_commit = forced_commands(prompt)

    # Ensure the session exists and apply any typed mode-commands.
    update = {}
    if forced_state:
        update["state"] = forced_state
    if forced_approach:
        update["approach"] = forced_approach
    if forced_commit:
        update["commit_requested"] = True
    stored = merge_state(session_id, update)

    # Deterministic context: the typed-command skill-load directives. Announcing a
    # mode the write never stored leaves the agent working under one mode while the
    # gates enforce the other, so the directive rides only on a confirmed write.
    if stored:
        context = directive(forced_state, forced_approach)
        if forced_commit:
            context = (context + "\n\n" + COMMIT_DIRECTIVE) if context else COMMIT_DIRECTIVE
    else:
        context = WRITE_FAILED_NOTICE if update else ""

    # Every other typed /skill: a head-anchored reload directive, leading the block.
    typed = typed_skills(prompt)
    if typed:
        skills_block = skills_directive(typed)
        context = (skills_block + "\n\n" + context) if context else skills_block

    # Interview state turns the LLM hooks off for speed: emit only the deterministic
    # directives built above and skip the model call.
    if load_state(session_id).get("state") == "interview":
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
        contract = intent_contract(intent, bool(result.get("has_action_items")))
        if contract:
            context = (context + "\n\n" + contract) if context else contract

        if result.get("sequential"):
            context = (context + "\n\n" + SEQUENTIAL_DIRECTIVE) if context else SEQUENTIAL_DIRECTIVE

        # On proposing turns, re-surface the session's standing corrections.
        current_state = load_state(session_id).get("state") or "proposing"
        if current_state == "proposing" and notes:
            block = NOTES_HEADER + "\n" + "\n".join("- %s" % n for n in notes)
            context = (context + "\n\n" + block) if context else block

    # Standing behavioral reminders — emitted every non-skipped turn.
    context = (context + "\n\n" + STANDING_REMINDERS) if context else STANDING_REMINDERS

    if context:
        emit_context(context)
    return 0


if __name__ == "__main__":
    sys.exit(main())
