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

QUEUED = "Queued; `trace read <path>` reads each:"
MARKER = "[trimmed at L%d of %d — continue: trace read %s --lines %d:%d]"


def resend_user_rules(event):
    """Each Rule whole while it fits, then the next cut at a whole line with
    `trace read`'s own marker, then the rest named. Only a whole Rule is
    recorded, so one cut short is sent again where it applies."""
    recs = transcript.live_records(transcript.records(field(event, "transcript_path", "")))
    rules = []
    for path in transcript.user_rules_loaded(recs):
        try:
            with open(path, encoding="utf-8") as fh:
                rules.append((path, fh.read().splitlines()))
        except OSError:
            continue
    room = tracer.room(SOURCE)
    sections = []
    for index, (path, lines) in enumerate(rules):
        start = frontmatter.body_start(lines)
        whole = "Contents of %s:\n\n%s" % (path, "\n".join(lines[start:]).strip("\n"))
        used = feedback.width("\n\n".join(sections + [""]))
        if used + feedback.width(whole) + _queue_width(rules[index + 1:]) <= room:
            sections.append(whole)
            tracer.run(event, "docs", "prime", path)
            continue
        part = _cut(path, lines, start, room - used - _queue_width(rules[index:]))
        queued = rules[index + 1:] if part else rules[index:]
        sections += [part] if part else []
        sections.append(_queue(queued))
        break
    if sections:
        feedback.context(SOURCE, "SessionStart", "\n\n".join(sections))


def _cut(path, lines, start, room):
    """The Rule's whole lines from `start` that fit `room` beside the marker for
    the rest, or "" when not one does."""
    total = len(lines)
    frame = "Contents of %s (L%d-L%d of %d):\n\n\n\n" % (path, total, total, total)
    left = room - feedback.width(frame + MARKER % (total, total, path, total, total))
    shown = []
    for line in lines[start:]:
        if feedback.width(line) + 1 > left:
            break
        left -= feedback.width(line) + 1
        shown.append(line)
    if not shown:
        return ""
    last = start + len(shown)
    return "Contents of %s (L%d-L%d of %d):\n\n%s\n\n%s" % (
        path, start + 1, last, total, "\n".join(shown), MARKER % (last, total, path, last + 1, total))


def _queue(rules):
    return "\n".join([QUEUED] + ["- %s" % path for path, _ in rules])


def _queue_width(rules):
    return feedback.width(_queue(rules)) + 2 if rules else 0


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
