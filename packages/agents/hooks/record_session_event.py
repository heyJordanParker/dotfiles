#!/usr/bin/env python3
"""Record one session-state change per hook event, driving the session_state spine.

Replaces the seven-plus-one inline jq blocks that used to live in settings.json,
one per wired event. Claude Code hands this hook the event JSON on stdin; the hook
reads it once, routes on hook_event_name (and tool name where the contract needs
it), and drives the existing spine to record the matching change.

Recording contract (one per event, mirroring the retired jq wiring):

  SessionStart    -> start    (plain session id, passes the transcript path through)
  UserPromptSubmit-> prompt   (plain session id, prompt TEXT on the spine's stdin)
                     and, when the prompt is a completed <task-notification>,
                     stopped agent-<task-id> + archive that subagent's tracer log
  Stop            -> stopped  (plain session id)
  PostToolUse     -> tool-used(agent-rewritten session id)
  PreToolUse Read -> read     (agent-rewritten session id, file path; skip if none)
  PreToolUse Skill-> skill    (agent-rewritten session id, skill name; skip if none)
  PostCompact     -> compacted(plain session id)

The agent-id rewrite (per-tool-call recordings + the task-notification path): when
the event carries agent_id, record against "agent-<agent_id>" so a subagent's
events nest under its parent. No session id at all -> do nothing. start / prompt /
stopped / compacted record against the plain session id, matching the old wiring.

The spine is driven in-process (its cmd_* functions imported directly) so the hot
path -- PostToolUse, fired once per tool call -- spawns no subprocess. The one
subprocess is the tracer-log archive, reused from archive_subagent_log.py, only on
the rare completed-task-notification path. The hook never blocks: it returns 0 on
every path, including malformed/empty payloads and a missing session id.
"""

import io
import re
import sys

import archive_subagent_log
from lib.event import field, read_event
from lib.session_state import (
    cmd_compacted,
    cmd_prompt,
    cmd_read,
    cmd_skill,
    cmd_start,
    cmd_stopped,
    cmd_tool_used,
    merge_state,
)

BINDING = {
    "events": {
        "SessionStart": [],
        "UserPromptSubmit": [],
        "Stop": [],
        "PostCompact": [],
        "PostToolUse": ["*"],
        "PreToolUse": ["Read", "Skill"],
    },
    "harness": "all",
}

# <task-id>…</task-id> and <status>…</status> from a subagent-completion prompt,
# matching the retired jq block's `[a-f0-9]*` / `[a-z]*` character classes.
_TASK_ID = re.compile(r"<task-id>([a-f0-9]*)</task-id>")
_STATUS = re.compile(r"<status>([a-z]*)</status>")


def _rewritten_session(event):
    """The per-tool-call recording target: agent-<agent_id> when an agent id is
    present (nest under the parent), else the plain session id. Empty string means
    no session id at all -> the caller does nothing."""
    agent_id = field(event, "agent_id", "")
    if agent_id:
        return "agent-%s" % agent_id
    return field(event, "session_id", "")


def _record_prompt(session_id, prompt):
    """Drive the spine's prompt command, which reads the prompt off stdin (it
    filters system-injected prompts and rotates the turn timers). Feed the text in
    the same way the spine tests do -- swap sys.stdin for the duration of the call."""
    saved = sys.stdin
    sys.stdin = io.StringIO(prompt)
    try:
        cmd_prompt([session_id])
    finally:
        sys.stdin = saved


def _handle_task_notification(event, prompt):
    """A completed <task-notification> means a subagent finished: record its stop
    and archive its tracer log. Non-completed or non-notification prompts do
    nothing. Mirrors the retired jq block -- plain session id for the archive's
    parent arg, bare task id (no agent- prefix) for its agent arg."""
    if not prompt.startswith("<task-notification>"):
        return
    status_match = _STATUS.search(prompt)
    task_match = _TASK_ID.search(prompt)
    status = status_match.group(1) if status_match else ""
    task_id = task_match.group(1) if task_match else ""
    if status != "completed" or not task_id:
        return
    cmd_stopped(["agent-%s" % task_id])
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
        return 0

    if hook_event == "UserPromptSubmit":
        prompt = field(event, "prompt", "")
        session_id = field(event, "session_id", "")
        if session_id:
            _record_prompt(session_id, prompt)
        _handle_task_notification(event, prompt)
        return 0

    if hook_event == "Stop":
        session_id = field(event, "session_id", "")
        if session_id:
            cmd_stopped([session_id])
        return 0

    if hook_event == "PostCompact":
        session_id = field(event, "session_id", "")
        if session_id:
            cmd_compacted([session_id])
        return 0

    if hook_event == "PostToolUse":
        session_id = _rewritten_session(event)
        if session_id:
            cmd_tool_used([session_id])
        return 0

    if hook_event == "PreToolUse":
        session_id = _rewritten_session(event)
        if not session_id:
            return 0
        tool = field(event, "tool_name", "")
        if tool == "Read":
            file_path = field(event, "tool_input.file_path", "")
            if file_path:
                cmd_read([session_id, file_path])
        elif tool == "Skill":
            skill = (
                field(event, "tool_input.skill", "")
                or field(event, "tool_input.skill_name", "")
                or field(event, "tool_input.name", "")
            )
            if skill:
                cmd_skill([session_id, skill])
                if skill == "interview":
                    merge_state(field(event, "session_id", ""), {"state": "interview"})
        return 0

    return 0


if __name__ == "__main__":
    sys.exit(main())
