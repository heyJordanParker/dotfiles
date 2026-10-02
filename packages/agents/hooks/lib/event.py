"""Hook event payload helpers.

Claude Code hands a hook its event as JSON on stdin. These helpers read the
payload once and pull nested fields with a string default.
"""

import glob
import json
import os
import re
import sys

from lib import agent_memory


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
    """The shell command as a string, or "" when the event is not a shell call.

    Claude sends `tool_input.command` as a string. Codex's shell tool may send it
    as a list (e.g. ["/bin/zsh", "-lc", "git reset --hard"]); a list is joined so
    command-pattern matching works either way. Anything else yields "".

    Codex puts a whole apply_patch body under the same `command` key
    (`apply_patch.rs:508`). Every command guard parses what this returns as
    shell, so a patch's own text was read as commands and a `+` line adding a
    newline refused the write. A tool `_CANONICAL_TOOL` knows to be something
    other than a shell therefore carries no command here.

    A tool it does not know — an unmapped MCP or function tool, or a payload
    with no `tool_name` at all — still answers with whatever `command` holds.
    Reading nothing there would pass every command guard on a line they cannot
    identify, and the standing treatment for a line a guard cannot identify is
    to judge it, never to wave it through.
    """
    if canonical_tool(event) not in ("", "shell"):
        return ""
    c = field(event, "tool_input.command", "")
    if isinstance(c, list):
        return " ".join(str(x) for x in c)
    return c if isinstance(c, str) else ""


# The header naming the file a codex patch hunk touches. `*** Move to:` names a
# rename's destination and is deliberately absent: the hunk's own file is what a
# hook loads rules for, validates, or records.
_PATCH_FILE = re.compile(r"^\*\*\* (?:Update|Add|Delete) File: (.+)$", re.M)


def patch_text(event):
    """The apply_patch body a write event carries, or "".

    Codex serializes it under `command` (`apply_patch.rs:508`). The `input`,
    `patch`, and `changes` spellings are read too: hooks here were written
    against them, and no version is on record sending them, so they stay until
    one is proven absent rather than being removed on this reading alone.
    """
    if canonical_tool(event) not in ("", "write"):
        return ""
    for key in ("command", "input", "patch", "changes"):
        text = field(event, "tool_input." + key, "")
        if isinstance(text, str) and "*** " in text:
            return text
    return ""


def patch_target(event):
    """The file a write event touches, however its harness names it.

    Claude carries `file_path`; codex carries a patch whose first hunk header
    names the file. A patch this cannot parse answers "", which every caller
    treats as "no target known" rather than as "no file touched".
    """
    path = field(event, "tool_input.file_path", "")
    if path:
        return path
    match = _PATCH_FILE.search(patch_text(event))
    return match.group(1).strip() if match else ""


# Our canonical tool names, keyed on the tool_name each harness emits on a tool
# event. This table is the single owner of the translation — we never route
# through a harness's own Claude-compat aliasing, so a harness renaming a tool is
# corrected here and nowhere else. Claude emits the left names; codex emits its native names
# (shell_command/apply_patch/request_user_input) plus, for the shell tool, the
# compat-aliased "Bash" — all map here. codex's spawn tool is the one exception,
# recognized by name in `canonical_tool`.
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
    # Claude's other routes to a new agent: a Workflow now, a cloud routine on
    # another machine. CronCreate is absent: it re-prompts this same session.
    "Workflow": "agent",
    "RemoteTrigger": "agent",
    "AskUserQuestion": "ask",
    "request_user_input": "ask",
}


def canonical_tool(event):
    """The canonical tool name for this event, or '' when the tool is unmapped."""
    name = field(event, "tool_name", "")
    # codex names a namespaced tool namespace+name with no separator, and the
    # multi-agent namespace is configurable, so its spawn tool is known by the
    # name it ends with.
    if name.endswith("spawn_agent"):
        return "agent"
    return _CANONICAL_TOOL.get(name, "")


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
    for both. Codex names it the same way for an agent it spawned, as the role it
    spawned under.

    The environment is not consulted on Claude: `CLAUDE_CODE_AGENT` inside a
    subagent still holds the dispatching agent's name, verified live from a
    `code-reviewer` dispatch that read back `cto`.

    A Claude Subagent started with a name reports that name as `agent_type`, so
    its agent comes from the record Claude writes beside its transcript.
    """
    named = field(event, "agent_type", "")
    if named:
        return _started_as(event, named) or named
    return ""


def _started_as(event, named):
    """The agent Claude's start record names for a Subagent started with a name, or "".

    Claude writes `agent-<id>.meta.json` beside each Subagent transcript under the
    parent session's `subagents/` folder. A Subagent started with a name has an id
    that embeds it, `a<name>-<hash>`, and its record carries the agent it runs as
    in `customAgentType`. A Subagent started by type has no such field.
    """
    if os.path.isfile(agent_memory.definition_path(named)):
        return ""
    record = field(event, "agent_transcript_path", "")
    if record.endswith(".jsonl"):
        paths = [record[:-len(".jsonl")] + ".meta.json"]
    else:
        parent = field(event, "transcript_path", "")
        if not parent.endswith(".jsonl"):
            return ""
        folder = os.path.join(parent[:-len(".jsonl")], "subagents")
        agent_id = field(event, "agent_id", "")
        paths = ([os.path.join(folder, "agent-%s.meta.json" % agent_id)] if agent_id
                 else glob.glob(os.path.join(folder, "agent-a%s-*.meta.json" % glob.escape(named))))
    for path in paths:
        try:
            with open(path, encoding="utf-8") as fh:
                started = json.load(fh).get("customAgentType")
        except (OSError, ValueError, AttributeError):
            continue
        if isinstance(started, str) and started:
            return started
    return ""


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
