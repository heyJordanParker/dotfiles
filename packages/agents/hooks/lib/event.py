"""Hook event payload helpers.

Claude Code hands a hook its event as JSON on stdin. These helpers read the
payload once and pull nested fields with a string default.
"""

import glob
import json
import os
import sys

from lib import agent_definition


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


def agent_name(event):
    """Which agent this payload belongs to, or "" when none names one.

    Every gate that reads an agent's definition needs the running agent's name.
    Claude puts it on the payload as `agent_type` — on a subagent
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
    if os.path.isfile(agent_definition.definition_path(named)):
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
