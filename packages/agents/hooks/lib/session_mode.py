"""Resolve the running event's mode and whether it may start another agent."""

from lib import agent_memory
from lib.event import agent_name, field, owner_session
from lib.session_state import load_state

MODES = ("orchestrate", "build", "interview")
STATES = ("propose", "execute")


def declared_by(path):
    """The mode the definition at `path` declares, with build as the omission fallback.

    The declaration is how the agent works in a session the architect starts on
    it. An agent name with no readable definition behind it builds, so it is
    refused the spawn.
    """
    if not path:
        return "build"
    try:
        value = agent_memory.declaration(path, "mode")
    except OSError:
        return "build"
    return value if value in MODES else "build"


def declared_mode(event):
    return declared_by(agent_memory.definition_path(agent_name(event)))


def is_dispatched(event):
    """Whether an agent someone dispatched owns this event, rather than a session
    the architect drives himself.

    Claude puts `agent_id` on a payload only inside a Subagent call, so it names a
    Claude dispatch. A session started with `--agent`, the architect's own
    teammate, carries `agent_type` without it and stays his.
    """
    return bool(field(event, "agent_id", ""))


def resolve(event, session_id=None):
    """The event's mode, with a typed main-session override when it applies.

    `session_id` names the session to read the typed mode off. A gate omits it and
    gets the governing session `owner_session` answers. A hook that already knows
    which session it writes — the prompt classifier — passes that one, so the mode
    it announces and the mode it stored are the same record.
    """
    declared = declared_mode(event)
    session_id = session_id or owner_session(event)
    if not session_id:
        return declared
    mode = load_state(session_id).get("mode")
    return mode if mode in MODES else declared


def may_spawn(event):
    """Whether this event may start another agent.

    A dispatched agent never does: every Subagent builds, whatever its definition
    declares, so the work it is handed is done where its dispatcher reads the result.

    A session is held to the mode `resolve` answers — the same mode the statusline
    shows and the same skill the agent was told to work under. A session that names
    an agent is held to that agent's declaration, and a session the architect typed
    a mode into is held to what he typed, because the mode is his instruction and
    the gate is what makes it one. A session that names no agent and typed no mode
    has no chosen mode to enforce, so it spawns.
    """
    if is_dispatched(event):
        return False
    if agent_name(event) or load_state(owner_session(event)).get("mode_typed"):
        return resolve(event) == "orchestrate"
    return True


def state(event):
    """The launching session's state axis, which every event answers to.

    A dispatched agent inherits its launcher's stage: an agent exempted from it
    wrote the repo while the architect was still weighing the proposal. An
    unrecorded session defaults to propose, except a dispatch with no recorded
    launcher behind it, whose task arrived already scoped.
    """
    value = load_state(owner_session(event)).get("state")
    if value in STATES:
        return value
    return "execute" if is_dispatched(event) else "propose"
