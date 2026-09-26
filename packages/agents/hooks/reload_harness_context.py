#!/usr/bin/env python3
"""Keep tracer's record of Claude Code's loaded docs true.

Claude Code reports each doc it loads while the session runs — a nested
Claude.md, a rule matching a path, an include — through `InstructionsLoaded`,
and each one is recorded (`trace docs prime <file>`), so tracer never sends a
doc the agent already holds. Compaction and a clear drop what was loaded, so
the record is forgotten before them (`trace docs reset`).

After a compaction Claude Code puts its session-start docs back without
reporting them, so every SessionStart records them itself: the Claude.md chain
(`trace docs prime`) and the working directory's docs (`trace docs <cwd>
--json`, which records every doc it returns) — the same set inject_rules
sends codex, recorded here instead of sent.

After a compaction Claude Code reloads project docs as files are read, but never
a user Rule the session loaded before it. So the compaction's SessionStart sends
those user Rules back from disk, whole, and records them.

codex reports no per-file load; inject_rules keeps its record.
"""

import os
import sys

from lib import feedback, frontmatter, tracer, transcript
from lib.event import field, read_event

BINDING = {
    "events": {
        "InstructionsLoaded": [],
        "PreCompact": [],
        "SessionStart": [],
    },
    "harness": "claude",
    "timeout": 20,
    "standalone": True,
}

SOURCE = "reload_harness_context"

UNSENT = "### Read these Rules whole\nThey did not fit in this message: %s"


def resend_user_rules(event):
    recs = transcript.live_records(transcript.records(field(event, "transcript_path", "")))
    room = tracer.room(SOURCE)
    sections, unsent = [], []
    for path in transcript.user_rules_loaded(recs):
        try:
            with open(path, encoding="utf-8") as fh:
                _, body = frontmatter.parse(fh.read())
        except OSError:
            continue
        section = "Contents of %s:\n\n%s" % (path, body)
        if len("\n\n".join(sections + [section])) > room:
            unsent.append(path)
            continue
        sections.append(section)
        tracer.run(event, "docs", "prime", path)
    if unsent:
        sections.append(UNSENT % ", ".join(unsent))
    if sections:
        feedback.context(SOURCE, "SessionStart", "\n\n".join(sections))


def main():
    if not tracer.available():
        return 0
    event = read_event()
    event_name = field(event, "hook_event_name", "")
    if event_name == "InstructionsLoaded":
        path = field(event, "file_path", "")
        if path:
            tracer.run(event, "docs", "prime", path)
        return 0
    if event_name == "PreCompact":
        tracer.run(event, "docs", "reset", "--source", SOURCE)
        return 0
    tracer.session_start(
        event, SOURCE,
        lambda: tracer.run(event, "docs", field(event, "cwd", "") or os.getcwd(), "--json", "--source", SOURCE),
    )
    if field(event, "source", "") == "compact":
        resend_user_rules(event)
    return 0


if __name__ == "__main__":
    sys.exit(main())
