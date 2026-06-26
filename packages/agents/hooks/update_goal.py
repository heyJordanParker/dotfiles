#!/usr/bin/env python3
"""Maintain the session's goal, requirements, and boundaries (LLM, every turn).

On every UserPromptSubmit this reads what the user said, recalls cross-session
memory, and updates the session's goal (one paragraph), requirements (what the
work must do), and boundaries (what it must never do) on the spine — translating
only what the user expressed, never inventing or padding. Each list is hard-capped
at 10 in code. It then hands the main agent the current goal/requirements/
boundaries read back from the spine, the skills the user invoked this turn, a
one-line take on what the user is doing this turn relative to the goal (the hook's
inference, framed as such), and an optional one-line note when one would prevent
confusion.

This is the renamed intent classifier: the model call moved here, pointed at the
goal instead of mode-switching. The user drives modes by hand through typed
commands, handled deterministically by classify_intent.py.

Any infrastructure failure (recursion guard, missing tool, parse error, LLM
unavailable) returns 0 and never blocks the user's prompt.
"""

import json
import os
import re
import subprocess
import sys

from lib import transcript
from lib.event import field, read_event
from lib.model_call import run_model
from lib.session_state import load_state, merge_state

BINDING = {
    "events": {"UserPromptSubmit": []},
    "timeout": 70,
    "harness": "all",
}

LIST_CAP = 10
MEMORY_TIMEOUT = 8


def emit_context(text):
    sys.stdout.write(json.dumps({
        "hookSpecificOutput": {
            "hookEventName": "UserPromptSubmit",
            "additionalContext": text,
        }
    }) + "\n")


# --- structural system-message detection --------------------------------------

def is_system_prompt(prompt):
    if prompt.startswith("<"):
        tag_rest = prompt[1:]
        tag_name = re.split(r"[> ]", tag_rest, maxsplit=1)[0]
        if tag_name and ("</%s>" % tag_name) in prompt:
            return True
    if prompt.startswith("["):
        first_line = prompt.split("\n", 1)[0]
        if first_line.endswith("]") and prompt == first_line:
            return True
    if prompt.startswith("This session is being continued"):
        return True
    if prompt.startswith("Base directory for this skill:"):
        return True
    return False


# --- cross-session memory recall (iai-mcp) -------------------------------------

def recall_memory(session_id):
    """The cached session-start recall payload from the iai-mcp daemon, or "".

    Best-effort: a missing CLI, non-zero exit, or timeout yields no memory rather
    than blocking the turn."""
    cli = os.environ.get("IAI_MCP_SESSION_RECALL_CLI") or os.path.expanduser("~/.local/bin/iai-mcp")
    if not os.path.exists(cli):
        return ""
    try:
        r = subprocess.run([cli, "session-start", "--session-id", session_id],
                           capture_output=True, text=True, timeout=MEMORY_TIMEOUT)
    except Exception:
        return ""
    return r.stdout.strip() if r.returncode == 0 else ""


# --- LLM ------------------------------------------------------------------------

SYSTEM_PROMPT = ("You maintain a session's goal, requirements, and boundaries. "
                 "Output structured JSON only.")

JSON_SCHEMA = ('{"type":"object","properties":'
               '{"goal":{"type":"string"},'
               '"requirements":{"type":"array","items":{"type":"string"}},'
               '"boundaries":{"type":"array","items":{"type":"string"}},'
               '"skills":{"type":"array","items":{"type":"string"}},'
               '"take":{"type":"string"},'
               '"note":{"type":"string"}},'
               '"required":["goal","requirements","boundaries","take"]}')


def _block(label, items):
    return "%s:\n%s" % (label, "\n".join("- %s" % i for i in items)) if items else "%s: (none)" % label


def evaluation_prompt(prompt, goal, requirements, boundaries, memory, conversation):
    standing = "Current goal:\n%s\n\n%s\n\n%s\n" % (
        goal or "(none)", _block("Current requirements", requirements),
        _block("Current boundaries", boundaries))
    memory_block = ("Cross-session memory (background context about the user and project):\n%s\n\n---\n"
                    % memory) if memory else ""
    return (
'You maintain the session\'s GOAL, REQUIREMENTS, and BOUNDARIES from what the user says.\n'
'\n'
'%s'
'%s%s\n'
'The user\'s latest message:\n%s\n'
'\n'
'Update the three lists to reflect what the user has expressed, and output them in full.\n'
'\n'
'- goal: one paragraph naming the session\'s OVERARCHING objective — the big-picture outcome the individual tasks serve, never the task in front of you. It is stable: carry the standing goal forward unchanged and treat the latest message as work toward it. Rewrite the goal only when the user fundamentally redirects the whole session, which is rare. State it at the big-picture altitude.\n'
'- requirements: what the work MUST do — each phrased to be specific and testable (concrete enough to check whether it is met), e.g. "supports Google SSO", never vague like "make it good". A vague wish the user states but does not make concrete ("make it better", "more modern", "cleaner", "nicer") is NOT a testable requirement: do not restate it as one and do not invent the specifics it lacks — leave it out entirely.\n'
'- boundaries: what the work must NEVER do — each specific and testable, e.g. "never stores passwords in plaintext", never vague like "do not break things". Honor the user\'s own framing: when the user states a constraint as a prohibition ("must never", "don\'t", "no more than", "under no circumstances", "never"), it is a BOUNDARY, not a requirement. Do not flip a prohibition into a positive requirement to make it sound like a feature — keep it where the user put it.\n'
'- skills: every /skill the user invokes in THIS message (telling you to use or run it now). '
'Preserve the leading slash. Empty when none.\n'
'- take: a one-line take on what the user is doing with their latest message in relation to the goal.\n'
'- note: OPTIONAL. Include a single short note ONLY when it would save the main agent from a real '
'confusion this turn. Never include a note to fill the field. Omit it on almost every turn.\n'
'\n'
'Hard rules — follow exactly:\n'
'- Translate only what the user expressed. Never invent an item the user did not state.\n'
'- One stated rule is one item. When the user expresses a single constraint as one rule — even across a compound sentence with several clauses joined by "and", "or", a dash, or a restatement of the same idea — capture it as ONE requirement or ONE boundary. Do not split its clauses into separate items, and do not add a paraphrase of the same rule as a second item. Example: "reject the whole file if any row is malformed — never accept a file with a single bad row" is one boundary, not two.\n'
'- Never pad a list to look complete, and never add an item so a list reaches a nicer count. '
'Two real requirements stay two. An empty list stays empty.\n'
'- A requirement or boundary is a STANDING property of the finished work — something the result must always do, or must never do. An instruction to perform an action once and move on is NOT one, even though the user said it: using a skill ("use /cc"), running, cloning, tracing, reading, opening, checking, or doing a step are execute-and-forget — they get done and they are over, so they are never requirements or boundaries. Capture only the lasting constraints the work must keep satisfying, never the steps taken to get there. They must also directly serve the overarching goal; a tactical step for one task or a one-off correction stays out. Example: from "use /cc, clone the repo, trace how it works, and only pursue 25%%+ wins", the single requirement is the 25%%+ constraint — use/clone/trace are execute-and-forget and stay out.\n'
'- Curate the lists every turn. For EACH standing requirement and boundary, ask: does its subject matter belong to the goal as stated right now? If it names a feature, surface, or concern the goal is not about, DROP it — it is stale and must not appear in your output. A requirement only survives because it still serves the current goal, never because it was there before. Example: under a "payments system" goal, a requirement about search-page load time is unrelated and must be dropped.\n'
'- When the user abandons, replaces, or declines an approach or feature ("scrap X", "drop X", "we are doing Y instead", "I don\'t want X", "no X", "not X — just Y"), X simply does NOT enter the lists (and leaves if it was already there). Declining or removing X does not create a "never X" / "does not include X" boundary — the user removed a concern or passed on a feature, they did not add a prohibition. A boundary exists only when the user states an active rule the work must obey ("must never", "under no circumstances", "no more than"), not when they merely decline a feature. Capture only what the user does want, naming it (the replacement or the wanted feature Y), never the declined or discarded X.\n'
'- A feature the user defers to a hypothetical future ("if we ever add X later", "someday", "down the road", "for now just Y", "don\'t build X yet") is NOT a current requirement, even when concrete and testable. Capture only what the user wants built now; a deferred maybe-later is never a requirement, and never a "never X" boundary.\n'
'- Then add only what the latest message states that serves the goal. Output is the surviving items plus the new ones — nothing else.\n'
'- Specific and testable is about wording only — phrase each item so it can be checked, but never add a requirement or boundary the user did not express to satisfy it.\n'
'- Zero creativity. You are translating intent, not improving it.\n'
        ) % (standing, memory_block, conversation, prompt)


# --- message to the main agent --------------------------------------------------

OPENING_DIRECTIVE = (
    "Read the architect literally and follow exactly — their words are the spec, and where "
    "your paraphrase diverges from them, their words win. A request to propose, design, or "
    "find is answered with a proposal, not an edit.\n\n"
    "Every turn you MUST open your reply with both of the following, in order — neither is "
    "optional:\n"
    "1. Restate what the architect asked — in their voice, vocabulary, and order, following "
    "their intent. Keep every requirement, constraint, count, and listed item they stated, "
    "and add nothing they did not. One line when the ask is single; an enumerated list when "
    "the architect stated several requirements.\n"
    "2. The goal below as a `# Goal` block: a `# Goal` heading, the goal in prose, then a `>` "
    "blockquote for why it matters.\n"
    "Then start the work."
)


def build_message(state, result):
    parts = []
    goal = state.get("goal")
    if goal:
        parts.append("Session goal:\n%s" % goal)
    skills = result.get("skills")
    if isinstance(skills, list) and skills:
        parts.append("Skills to execute: " + ", ".join(str(s) for s in skills))
    note = result.get("note")
    if isinstance(note, str) and note.strip():
        parts.append(note.strip())
    take = result.get("take")
    if isinstance(take, str) and take.strip():
        parts.append("Goal-tracker's read of this turn (the hook's inference, "
                     "not the architect's words): " + take.strip().splitlines()[0].strip())
    if not parts:
        return ""
    body = "\n\n".join(parts)
    return (OPENING_DIRECTIVE + "\n\n" + body) if goal else body


def main():
    # Guard against recursion: the model call runs a nested harness process.
    if os.environ.get("CLAUDE_SESSION_HOOK") == "true":
        return 0

    event = read_event()

    session_id = field(event, "session_id", "")
    if not session_id or session_id.startswith("agent-"):
        return 0

    prompt = field(event, "prompt", "")
    if not prompt:
        return 0

    if is_system_prompt(prompt):
        return 0

    transcript_path = field(event, "transcript_path", "")
    state = load_state(session_id)
    goal = state.get("goal")
    requirements = state.get("requirements") or []
    boundaries = state.get("boundaries") or []

    memory = recall_memory(session_id)
    conversation = transcript.conversation_context(transcript_path)
    eval_prompt = evaluation_prompt(prompt, goal, requirements, boundaries, memory, conversation)

    result = run_model(system_prompt=SYSTEM_PROMPT, user_prompt=eval_prompt,
                       schema=JSON_SCHEMA, session_id=session_id, hook="update_goal")
    if not result:
        return 0

    update = {}
    new_goal = result.get("goal")
    if isinstance(new_goal, str) and new_goal.strip():
        update["goal"] = new_goal.strip()
    new_requirements = result.get("requirements")
    if isinstance(new_requirements, list):
        update["requirements"] = [str(x) for x in new_requirements][:LIST_CAP]
    new_boundaries = result.get("boundaries")
    if isinstance(new_boundaries, list):
        update["boundaries"] = [str(x) for x in new_boundaries][:LIST_CAP]
    if update:
        merge_state(session_id, update)

    # The message reflects the spine, read back after the write — not raw LLM text.
    state = load_state(session_id)
    message = build_message(state, result)
    if message:
        emit_context(message)
    return 0


if __name__ == "__main__":
    sys.exit(main())
