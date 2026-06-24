#!/usr/bin/env python3
"""Map typed mode-commands to session control state (deterministic, no LLM).

On every UserPromptSubmit this reads the prompt for typed mode commands
(/propose /execute force STATE; /solo /subagents /team force APPROACH; /commit
forces a commit), persists them to the session spine (the control state the
proposal/commit/solo guards read), resets the per-turn validation phase, and
emits the matching skill-load contract as additionalContext.

No model call: the user drives modes by hand through the commands, so intent is
not classified. The goal/requirements/boundaries the session is working toward
are maintained separately by update_goal.py, which holds the one LLM call.

A structural system message or a subagent session is skipped untouched.
"""

import json
import re
import sys

from lib.event import field, read_event
from lib.session_state import merge_state

BINDING = {
    "events": {"UserPromptSubmit": []},
    "harness": "all",
}


def emit_context(text):
    sys.stdout.write(json.dumps({
        "hookSpecificOutput": {
            "hookEventName": "UserPromptSubmit",
            "additionalContext": text,
        }
    }) + "\n")


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
    return False


def _typed(prompt, token):
    """Bounded literal-token match: /propose matches, /proposed and /commit-x don't."""
    padded = " %s " % prompt
    pattern = r"(^|[^A-Za-z0-9])" + re.escape(token) + r"($|[^A-Za-z0-9-])"
    return re.search(pattern, padded) is not None


def forced_commands(prompt):
    forced_state = ""
    if _typed(prompt, "/propose"):
        forced_state = "proposing"
    elif _typed(prompt, "/execute"):
        forced_state = "executing"
    forced_approach = ""
    if _typed(prompt, "/solo"):
        forced_approach = "solo"
    elif _typed(prompt, "/subagents"):
        forced_approach = "subagents"
    elif _typed(prompt, "/team"):
        forced_approach = "team"
    forced_commit = _typed(prompt, "/commit")
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
    approach_line = {
        "solo": "Load the /solo skill now.",
        "subagents": "Load the /subagents skill now.",
        "team": "Load the /team skill now.",
    }.get(forced_approach, "")
    if approach_line:
        out = (out + "\n\n" + approach_line) if out else approach_line
    return out


COMMIT_DIRECTIVE = "Skills to execute: /commit"


def main():
    event = read_event()

    session_id = field(event, "session_id", "")
    if not session_id or session_id.startswith("agent-"):
        return 0

    prompt = field(event, "prompt", "")
    if not prompt:
        return 0

    if is_system_prompt(prompt):
        return 0

    forced_state, forced_approach, forced_commit = forced_commands(prompt)

    # Reset the per-turn validation phase; apply any typed mode mutations.
    update = {"validation_phase": 0}
    if forced_state:
        update["state"] = forced_state
    if forced_approach:
        update["approach"] = forced_approach
    if forced_commit:
        update["commit_requested"] = True
    merge_state(session_id, update)

    context = directive(forced_state, forced_approach)
    if forced_commit:
        context = (context + "\n\n" + COMMIT_DIRECTIVE) if context else COMMIT_DIRECTIVE
    if context:
        emit_context(context)
    return 0


if __name__ == "__main__":
    sys.exit(main())
