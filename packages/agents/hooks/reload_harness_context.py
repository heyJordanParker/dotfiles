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

codex reports no per-file load; inject_rules keeps its record.
"""

import os
import sys

from lib import tracer
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
    return 0


if __name__ == "__main__":
    sys.exit(main())
