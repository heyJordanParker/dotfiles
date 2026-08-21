#!/usr/bin/env python3
"""Block a turn from ending on tool calls with no reply text (Stop).

A turn that runs tools and writes no text reaches the architect as an empty
reply — the ScheduleWakeup/SendMessage-ate-my-reply failure, reproduced on
demand on 2026-08-21. The prose rule ("end every turn on a plain-text reply")
sat in memory and in the rules when it happened, so the fence is deterministic:
any assistant text in the current turn allows; tool calls with none blocks.

Codex is excluded the way babysitter.py is: the check is built on Claude's
turn boundary, which a codex rollout does not carry.
"""

from lib import feedback, transcript
from lib.event import field, read_event

BINDING = {
    "events": {"Stop": []},
    "harness": "claude",
}


def main():
    event = read_event()
    # A prior Stop block this turn already forced a rewrite; never loop.
    if field(event, "stop_hook_active", False):
        return 0
    turn = transcript.current_turn(
        transcript.records(field(event, "transcript_path", "")))
    ran_tools = False
    for record in turn:
        if record.get("type") != "assistant":
            continue
        for block in transcript.blocks(record):
            kind = block.get("type")
            if kind == "text" and block.get("text", "").strip():
                return 0
            if kind == "tool_use":
                ran_tools = True
    if not ran_tools:
        return 0
    return feedback.block(
        "block_silent_stop",
        "This turn ran tools and wrote no reply text, so the architect sees "
        "nothing. Write the reply now — result first — then end the turn.",
    )


if __name__ == "__main__":
    raise SystemExit(main())
