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
# through a harness's own Claude-compat aliasing, so a harness renaming a tool is
# corrected here and nowhere else. Claude emits the left names; codex emits its native names
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
    "NotebookEdit": "write",
    "apply_patch": "write",
    "Agent": "agent",
    "spawn_agent": "agent",
    # Claude's other routes to a running agent: now, on a schedule, or on another
    # machine. Each starts one, so each answers to whatever gates a spawn.
    "Workflow": "agent",
    "CronCreate": "agent",
    "RemoteTrigger": "agent",
    "AskUserQuestion": "ask",
    "request_user_input": "ask",
}


def canonical_tool(event):
    """The canonical tool name for this event, or '' when the tool is unmapped."""
    return _CANONICAL_TOOL.get(field(event, "tool_name", ""), "")


def is_subagent(event):
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


def agent_name(event):
    """Which agent this payload belongs to, or "" when none names one.

    Memory is stored per agent, so every write and every read needs the running
    agent's name. Claude puts it on the payload as `agent_type` — on a subagent
    event, and on the main thread of a session started with `--agent`, correct
    for both. Codex names it nowhere in the payload, so the run's own definition
    path answers instead, the same variable the codex-side gates read.

    The environment is not consulted on Claude: `CLAUDE_CODE_AGENT` inside a
    subagent still holds the dispatching agent's name, verified live from a
    `code-reviewer` dispatch that read back `cto`.
    """
    named = field(event, "agent_type", "")
    if named:
        return named
    path = os.environ.get("CODEX_RUN_AGENT_FILE", "")
    return os.path.basename(path)[:-3] if path.endswith(".md") else ""


def _is_codex_rollout(path):
    return os.path.basename(path).startswith("rollout-") or "/.codex/" in path


def stopping_transcript(event):
    """The stopping agent's own transcript, or "" when this stop is not an end.

    Claude's main-session Stop fires after every assistant turn, so acting there
    hits the architect's live session between his instructions. The two real
    ends are a SubagentStop — a one-shot subagent finishing, whose payload
    carries its own transcript — and a codex Stop, which ends the whole run and
    is told apart by its `turn_id` and its `.codex` rollout transcript.

    A SubagentStop never falls back to `transcript_path`: that is the parent's
    transcript, and acting on it treats the parent's live work as the stopping
    agent's.
    """
    path = field(event, "transcript_path", "")
    if field(event, "hook_event_name", "") == "SubagentStop":
        return field(event, "agent_transcript_path", "")
    if field(event, "turn_id", "") or _is_codex_rollout(path):
        return path
    return ""


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
