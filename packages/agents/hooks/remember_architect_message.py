#!/usr/bin/env python3
"""Store what the architect actually said in Honcho memory.

The plugin uploaded every UserPromptSubmit payload as his speech, because the
channel was the only thing it looked at. Task notifications, hook-injected
blocks, skill loads and compaction notices all arrive on that channel, so the
server derived "jordan instructed…" from an agent's own recovery script and fed
it back as fact in later sessions.

Both harnesses already record who spoke, and `transcript.architect_message` reads
whichever one wrote the file: Claude's per-record `promptSource`, or codex's
per-thread `originator`. Only his own words are sent. The record is written before
this hook fires, so the prompt it fires on is already in the transcript.

A subagent turn carries the parent's transcript, where the last user record is
the architect's real message — uploading it again would duplicate his words under
someone else's turn, so those return early.
"""

import sys

from lib import honcho, transcript
from lib.event import field, is_subagent, read_event

BINDING = {
    "events": {"UserPromptSubmit": []},
    "timeout": 10,
    "harness": "all",
    "roots": "all",
}


def main():
    event = read_event()
    if is_subagent(event):
        return 0

    cfg = honcho.config()
    if not honcho.enabled(cfg):
        return 0

    recs = transcript.records(field(event, "transcript_path", ""))
    text = transcript.architect_message(recs)
    if not text.strip():
        return 0

    honcho.post(
        cfg,
        honcho.session_name(field(event, "cwd", "")),
        cfg.get("peerName"),
        text,
        metadata={"instance_id": field(event, "session_id", "")},
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
