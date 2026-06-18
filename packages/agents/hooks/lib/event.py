"""Hook event payload helpers.

Claude Code hands a hook its event as JSON on stdin. These helpers read the
payload once and pull nested fields with a string default.
"""

import json
import os
import sys


def read_event():
    try:
        return json.load(sys.stdin)
    except Exception:
        return {}


def field(event, dotted, default=""):
    cur = event
    for key in dotted.split("."):
        if isinstance(cur, dict) and key in cur:
            cur = cur[key]
        else:
            return default
    return default if cur is None else cur


def command_str(event):
    """The shell command as a string.

    Claude sends `tool_input.command` as a string. Codex's shell tool may send it
    as a list (e.g. ["/bin/zsh", "-lc", "git reset --hard"]); a list is joined so
    command-pattern matching works either way. Anything else yields "".
    """
    c = field(event, "tool_input.command", "")
    if isinstance(c, list):
        return " ".join(str(x) for x in c)
    return c if isinstance(c, str) else ""


# Our canonical tool names, keyed on the tool_name each harness emits on a tool
# event. This table is the single owner of the translation — we never route
# through a harness's own Claude-compat aliasing, so a harness changing its
# emitted name fails the test in test_event_canonical.py instead of silently
# dropping a match. Claude emits the left names; codex emits its native names
# (shell_command/apply_patch/spawn_agent/request_user_input) plus, for the shell
# tool, the compat-aliased "Bash" — all map here.
_CANONICAL_TOOL = {
    "Bash": "shell",
    "shell_command": "shell",
    "exec_command": "shell",
    "Read": "read",
    "Write": "write",
    "Edit": "write",
    "MultiEdit": "write",
    "apply_patch": "write",
    "Agent": "agent",
    "spawn_agent": "agent",
    "AskUserQuestion": "ask",
    "request_user_input": "ask",
}


def canonical_tool(event):
    """The canonical tool name for this event, or '' when the tool is unmapped."""
    return _CANONICAL_TOOL.get(field(event, "tool_name", ""), "")


def owner_session(event):
    """The id of the session whose proposing/executing mode governs this run.

    Claude main: itself. Claude subagent: the parent (the payload session_id
    already carries the parent UUID). Codex launched by Claude: the launching
    Claude session, inherited in the environment. Standalone run (no launcher):
    its own session — itself a main session the classifier gives a mode.

    The place hooks resolve the governing session through — the launcher's mode —
    never the env var directly. Distinct from session_state.own_session_id, which
    resolves a process's own session for output keying (and prefers the inner
    CODEX_THREAD_ID over the launcher's CLAUDE_CODE_SESSION_ID).
    """
    return os.environ.get("CLAUDE_CODE_SESSION_ID") or field(event, "session_id", "")
