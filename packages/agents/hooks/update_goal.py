#!/usr/bin/env python3
"""Maintain the session's goal (LLM, every turn).

On every UserPromptSubmit this reads what the user said and maintains the
session's goal — one paragraph of session-spanning memory
of the user's intent, set when missing and updated only when the user's input
diverges — translating only what the user expressed, never inventing or padding.
It then hands the main agent the goal read back from the spine, a one-line take
on what the user is doing this turn relative to the goal (the hook's inference,
framed as such), and an optional one-line note when one would prevent confusion.

This is the renamed intent classifier: the model call moved here, pointed at the
goal instead of mode-switching. The user drives modes by hand through typed
commands, handled deterministically by classify_intent.py.

Any infrastructure failure (recursion guard, missing tool, parse error, LLM
unavailable) returns 0 and never blocks the user's prompt.
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
    "timeout": 70,
    "harness": "all",
}

def emit_context(text):
    feedback.context("update_goal", "UserPromptSubmit", text)


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


# --- LLM ------------------------------------------------------------------------

SYSTEM_PROMPT = "You maintain a session's goal. Output structured JSON only."

JSON_SCHEMA = ('{"type":"object","properties":'
               '{"goal":{"type":"string"},'
               '"take":{"type":"string"},'
               '"note":{"type":"string"}},'
               '"required":["goal","take"]}')


def evaluation_prompt(prompt, goal, conversation):
    standing = "Current goal:\n%s\n" % (goal or "(none)")
    return (
'You maintain the session\'s GOAL from what the user says.\n'
'\n'
'%s'
'%s\n'
'The user\'s latest message:\n%s\n'
'\n'
'- goal: one paragraph capturing what the USER wants done across the WHOLE session, held from the session\'s first message to now. It tracks the user\'s intent, never the agent\'s current deliverable and never the single task in front of you this turn. Maintain it as memory:\n'
'    • No standing goal yet: set it from what the user is asking for.\n'
'    • The user\'s latest message is another task toward the standing goal — a step, a fix, even one the user frames as a fresh start: return the standing goal UNCHANGED, word for word. A task toward the goal is not a change to the goal.\n'
'    • The user genuinely changes the goal, abandoning or replacing what the session is for: revise the goal to the new one and drop what the user abandoned. Do not keep the old goal alongside the new one.\n'
'  An off-topic or meta message — a question about your state, a digression, a request to explain something — is NOT a change to the goal. The goal stays put. State it at the session altitude.\n'
'- take: a one-line take on what the user is doing with their latest message in relation to the goal.\n'
'- note: OPTIONAL. Include a single short note ONLY when it would save the main agent from a real '
'confusion this turn. Never include a note to fill the field. Omit it on almost every turn.\n'
'\n'
'Hard rules — follow exactly:\n'
'- Translate only what the user expressed. Never invent intent the user did not state.\n'
'- Zero creativity. You are translating intent, not improving it.\n'
        ) % (standing, conversation, prompt)


# --- message to the main agent --------------------------------------------------

def build_message(state, result):
    parts = []
    goal = state.get("goal")
    if goal:
        parts.append("Session goal:\n%s" % goal)
    note = result.get("note")
    if isinstance(note, str) and note.strip():
        parts.append(note.strip())
    take = result.get("take")
    if isinstance(take, str) and take.strip():
        parts.append("Goal-tracker's read of this turn (the hook's inference, "
                     "not the architect's words): " + take.strip().splitlines()[0].strip())
    if not parts:
        return ""
    return "\n\n".join(parts)


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
    if state.get("state") == "interview":
        return 0
    goal = state.get("goal")

    conversation = transcript.conversation_context(transcript_path)
    eval_prompt = evaluation_prompt(prompt, goal, conversation)

    result = run_model(system_prompt=SYSTEM_PROMPT, user_prompt=eval_prompt,
                       schema=JSON_SCHEMA)
    if not result:
        return 0

    new_goal = result.get("goal")
    if isinstance(new_goal, str) and new_goal.strip():
        merge_state(session_id, {"goal": new_goal.strip()})

    # The message reflects the spine, read back after the write — not raw LLM text.
    state = load_state(session_id)
    message = build_message(state, result)
    if message:
        emit_context(message)
    return 0


if __name__ == "__main__":
    sys.exit(main())
