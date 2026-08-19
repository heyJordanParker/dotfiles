#!/usr/bin/env python3
"""Record one session-state change per hook event, driving the session_state store.

Replaces the seven-plus-one inline jq blocks that used to live in settings.json,
one per wired event. Claude Code hands this hook the event JSON on stdin; the hook
reads it once, routes on hook_event_name (and tool name where the contract needs
it), and drives the existing store to record the matching change.

Recording contract (one per event, mirroring the retired jq wiring):

  SessionStart    -> start    (plain session id, passes the transcript path through)
  UserPromptSubmit-> prompt   (plain session id, prompt TEXT on the store's stdin)
                     and, when the prompt is a completed <task-notification>,
                     archive that subagent's tracer log

The store is driven in-process (its cmd_* functions imported directly). The one
subprocess is the tracer-log archive, reused from archive_subagent_log.py, only on
the rare completed-task-notification path. The hook never blocks: it returns 0 on
every path, including malformed/empty payloads and a missing session id.
"""

import io
import re
import sys

import archive_subagent_log
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

# <task-id>…</task-id> and <status>…</status> from a subagent-completion prompt,
# matching the retired jq block's `[a-f0-9]*` / `[a-z]*` character classes.
_TASK_ID = re.compile(r"<task-id>([a-f0-9]*)</task-id>")
_STATUS = re.compile(r"<status>([a-z]*)</status>")


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


def _handle_task_notification(event, prompt):
    """A completed <task-notification> means a subagent finished: archive its
    tracer log. Non-completed or non-notification prompts do nothing. Mirrors the
    retired jq block -- plain session id for the archive's parent arg, bare task id
    (no agent- prefix) for its agent arg."""
    if not prompt.startswith("<task-notification>"):
        return
    status_match = _STATUS.search(prompt)
    task_match = _TASK_ID.search(prompt)
    status = status_match.group(1) if status_match else ""
    task_id = task_match.group(1) if task_match else ""
    if status != "completed" or not task_id:
        return
    session_id = field(event, "session_id", "")
    if session_id:
        archive_subagent_log.main([session_id, task_id])


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
        _handle_task_notification(event, prompt)
        return 0

    return 0


if __name__ == "__main__":
    sys.exit(main())
