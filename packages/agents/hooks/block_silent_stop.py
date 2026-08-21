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

import time

from lib import feedback, transcript
from lib.event import field, read_event

BINDING = {
    "events": {"Stop": []},
    "harness": "claude",
}

# The reply's records can land on disk AFTER Stop fires — observed twice live on
# 2026-08-21, the text record 98ms behind the hook. One read cannot separate
# "ended on a tool call" from "text still in flight", so a would-block re-reads
# until the flush wins or the retries run out. A truly silent turn never grows
# text, so retrying costs it nothing but the delay.
REREADS = 3
REREAD_DELAY = 0.4


def _turn_allows(path):
    turn = transcript.current_turn(transcript.records(path))
    ran_tools = False
    last_stop_reason = ""
    for record in turn:
        if record.get("type") != "assistant":
            continue
        if transcript.text_of(record).strip():
            return True
        message = record.get("message") or {}
        last_stop_reason = message.get("stop_reason") or ""
        if transcript.blocks(record, "tool_use"):
            ran_tools = True
    # A last assistant record stopped by end_turn proves a final non-tool message
    # exists even when its text is not on disk yet.
    return not ran_tools or last_stop_reason == "end_turn"


def main():
    event = read_event()
    # A prior Stop block this turn already forced a rewrite; never loop.
    if field(event, "stop_hook_active", False):
        return 0
    # The harness hands the reply text directly (the field babysitter.py reads);
    # a non-empty reply is the whole answer, with no transcript flush to race.
    if field(event, "last_assistant_message", "").strip():
        return 0
    path = field(event, "transcript_path", "")
    for attempt in range(REREADS):
        if _turn_allows(path):
            return 0
        if attempt < REREADS - 1:
            time.sleep(REREAD_DELAY)
    return feedback.block(
        "block_silent_stop",
        "This turn ran tools and wrote no reply text, so the architect sees "
        "nothing. Write the reply now — result first — then end the turn.",
    )


if __name__ == "__main__":
    raise SystemExit(main())
