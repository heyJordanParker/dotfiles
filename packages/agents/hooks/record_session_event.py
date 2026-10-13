#!/usr/bin/env python3
"""Record one session-state change per hook event, driving the session_state store.

Replaces the seven-plus-one inline jq blocks that used to live in settings.json,
one per wired event. Claude Code hands this hook the event JSON on stdin; the hook
reads it once, routes on hook_event_name (and tool name where the contract needs
it), and drives the existing store to record the matching change.

Recording contract (one per event, mirroring the retired jq wiring):

  SessionStart    -> start    (plain session id, passes the transcript path through)
  UserPromptSubmit-> prompt   (plain session id, prompt TEXT on the store's stdin)

The store is driven in-process (its cmd_* functions imported directly). The hook
never blocks: it returns 0 on every path, including malformed/empty payloads and a
missing session id.
"""

import io
import sys

from lib.event import field, read_event
from lib.session_mode import declared_mode
from lib.session_state import cmd_prompt, cmd_start, load_state, merge_state

BINDING = {
    "events": {
        "SessionStart": [],
        "UserPromptSubmit": [],
    },
    "harness": "all",
}

def _record_prompt(session_id, prompt):
    """Drive the store's prompt command, which reads the prompt off stdin (it
    filters system-injected prompts and rotates the turn timers). Feed the text in
    the same way the store's tests do -- swap sys.stdin for the duration of the call."""
    saved = sys.stdin
    sys.stdin = io.StringIO(prompt)
    try:
        cmd_prompt([session_id])
    finally:
        sys.stdin = saved


def main():
    event = read_event()
    hook_event = field(event, "hook_event_name", "")

    if hook_event == "SessionStart":
        session_id = field(event, "session_id", "")
        if session_id:
            cmd_start([session_id, "--transcript-path", field(event, "transcript_path", "")])
            if load_state(session_id).get("mode_typed") is not True:
                merge_state(session_id, {"mode": declared_mode(event), "mode_typed": False})
        return 0

    if hook_event == "UserPromptSubmit":
        prompt = field(event, "prompt", "")
        session_id = field(event, "session_id", "")
        if session_id:
            _record_prompt(session_id, prompt)
        return 0

    return 0


if __name__ == "__main__":
    sys.exit(main())
