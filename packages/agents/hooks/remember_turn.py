#!/usr/bin/env python3
"""Store one completed turn in Honcho memory."""

import sys
import time

from lib import agent_memory, honcho, transcript
from lib.event import field, read_event

BINDING = {
    "events": {"Stop": [], "SubagentStop": []},
    "timeout": 10,
    "harness": "all",
    "standalone": True,
    "roots": "all",
}

REREADS = 3
REREAD_DELAY = 0.4

# The entrypoint Claude Code records for `claude -p`. No workflow here runs Claude
# headless, so these are an Agent's scripted probes, and their answers ("my system
# prompt does not contain HERON") were being remembered as lessons.
HEADLESS = "sdk-cli"


def stopping_transcript(event):
    """The stopping agent's own transcript."""
    if field(event, "hook_event_name", "") == "SubagentStop":
        return field(event, "agent_transcript_path", "")
    return field(event, "transcript_path", "")


def flushed_turn(event):
    """The stopping transcript's records and the turn's replies, once the final reply is on disk."""
    final = field(event, "last_assistant_message", "").strip()
    for attempt in range(REREADS):
        recs = transcript.records(stopping_transcript(event))
        replies = transcript.agent_replies(recs)
        if final in replies:
            return recs, replies
        if attempt < REREADS - 1:
            time.sleep(REREAD_DELAY)
    return recs, "\n\n".join(text for text in (replies, final) if text)


def main():
    event = read_event()
    cfg = honcho.config()
    if not honcho.enabled(cfg):
        return 0
    agent = honcho.memory_agent(event)
    if agent and agent_memory.denies_memory(agent_memory.definition_path(agent)):
        agent = ""
    recs, replies = flushed_turn(event)
    if any(rec.get("entrypoint") == HEADLESS for rec in recs):
        return 0
    honcho.store(cfg, event, recs, agent, transcript.architect_messages(recs), replies if agent else "")
    return 0


if __name__ == "__main__":
    sys.exit(main())
