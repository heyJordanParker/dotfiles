#!/usr/bin/env python3
"""Store an agent's own replies in Honcho memory, under that agent's peer.

The counterpart of remember_architect_message.py: that one stores what the
architect said, this one stores what he was answered. Both write paths live
beside each other so one module owns who a message belongs to.

Every agent is its own peer, so a subagent's replies land in its own collection
rather than the orchestrator's. That is the point of recording them: what
ponytail said is how Honcho builds its picture of ponytail, and filing it under
one shared peer named for the harness mixed every agent's output into one pile
nobody could read back. A subagent stop carries its own transcript, which is
what gets read — the payload's `transcript_path` is the parent's.

A turn with no agent name behind it is not stored. Memory is per agent here, and
a nameless collection is the pile this hook exists to stop. An agent declaring
`memory: none` is not stored either: a collection built out of its output is a
memory of that agent, which is the thing the declaration denies.
"""

import sys

from lib import agent_memory, honcho, transcript
from lib.event import agent_name, field, read_event

BINDING = {
    "events": {"Stop": [], "SubagentStop": []},
    "timeout": 10,
    "harness": "all",
    "roots": "all",
}


def stopping_transcript(event):
    """The stopping agent's own transcript."""
    if field(event, "hook_event_name", "") == "SubagentStop":
        return field(event, "agent_transcript_path", "")
    return field(event, "transcript_path", "")


def main():
    event = read_event()

    cfg = honcho.config()
    if not honcho.enabled(cfg):
        return 0

    agent = agent_name(event)
    if not agent or agent_memory.denies_memory(agent_memory.definition_path(agent)):
        return 0

    text = transcript.agent_replies(transcript.records(stopping_transcript(event)))
    if not text.strip():
        return 0

    honcho.post(
        cfg,
        honcho.session_name(field(event, "cwd", "")),
        agent,
        text,
        metadata={"instance_id": field(event, "session_id", "")},
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
