"""Resolve the running event's mode and its allowed surfaces."""

import os

from lib import agent_memory
from lib.event import agent_name, field, owner_session
from lib.session_state import load_state

MODES = ("orchestrate", "build", "interview")
STATES = ("propose", "execute")

# A dispatched agent is one thing or the other: it orchestrates, or it executes.
POLICY = {
    "orchestrate": {"write": False, "spawn": True},
    "build": {"write": True, "spawn": False},
    "interview": {"write": False, "spawn": False},
}


def declared_mode(event):
    """The event agent's declaration, with build as the omission fallback.

    The fallback is the strict answer on both surfaces: build writes and never
    spawns, so an agent name with no readable definition behind it is refused the
    spawn — for a dispatch and for a teammate alike. Every roster agent declares
    `mode`, so the fallback only ever governs a name that is not ours.
    """
    path = agent_memory.definition_path(agent_name(event))
    if not path:
        return "build"
    try:
        value = agent_memory.declaration(path, "mode")
    except OSError:
        return "build"
    return value if value in MODES else "build"


def is_dispatched(event):
    """Whether an agent someone dispatched owns this event, rather than a session
    the architect drives himself.

    A sidechain marker is the only payload evidence of a dispatch. `agentId`
    alone is not: the architect's own hand-managed teammates are top-level
    sessions carrying one, and gating on it spawn-blocked his own orchestrators.
    Codex has no sidechain, so a codex-run agent is named by its exported
    definition path instead.
    """
    for key in ("isSidechain", "is_sidechain"):
        if field(event, key, ""):
            return True
    return bool(os.environ.get(agent_memory.AGENT_FILE_VAR))


def is_teammate(event):
    """Whether one of the architect's hand-managed agents owns this event.

    A teammate is a top-level session he started on an agent: it carries an
    `agentId` but no dispatch evidence. That is exactly the payload `is_dispatched`
    refuses, so the two predicates never both answer True.
    """
    if is_dispatched(event):
        return False
    return bool(field(event, "agentId", "") or field(event, "agent_id", ""))


def resolve(event, session_id=None):
    """The event's mode, with a typed main-session override when it applies.

    `session_id` names the session to read the typed mode off. A gate omits it and
    gets the governing session `owner_session` answers. A hook that already knows
    which session it writes — the prompt classifier — passes that one, so the mode
    it announces and the mode it stored are the same record.
    """
    declared = declared_mode(event)
    if is_dispatched(event):
        return declared
    session_id = session_id or owner_session(event)
    if not session_id:
        return declared
    mode = load_state(session_id).get("mode")
    return mode if mode in MODES else declared


def permits(event, surface):
    """Whether the running agent's declared mode permits one guarded surface.

    A dispatched agent is gated on both surfaces. A teammate is gated on the spawn
    surface alone, by the mode its own definition declares: spawning cascades usage
    through every agent it starts, so a teammate running a build agent is refused it
    exactly as a dispatched build agent is. Its writes stay on the state axis, where
    the architect's own sessions sit — proposing holds them back, executing lets them
    through — and a true main session, which names no agent at all, stays there on
    both surfaces.
    """
    if is_dispatched(event) or (surface == "spawn" and is_teammate(event)):
        return POLICY[declared_mode(event)][surface]
    return True


def state(event):
    """The session state axis, defaulting to propose for an unrecorded session.

    A dispatched agent is always executing: its task arrived already scoped and
    there is no proposal pending for it to hold up. Every other session reads the
    stage it recorded, the architect's hand-managed teammates included.
    """
    if is_dispatched(event):
        return "execute"
    value = load_state(owner_session(event)).get("state")
    return value if value in STATES else "propose"
