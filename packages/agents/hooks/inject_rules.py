#!/usr/bin/env python3
"""Keep codex's project rules in context, and tracer's record of them true.

codex loads the repo-root → cwd Claude.md chain once at session start and
nothing else: no nested Claude.md, no rules. Claude Code loads those itself and
reports each load to reload_harness_context, so this is codex-only.

- SessionStart: after a clear, forget what was loaded; record the chain codex
  loaded itself (`trace docs prime`), then send the working directory's
  remaining docs — its rules — so they arrive once.
- PreCompact: forget what was loaded, since compaction drops it.
- A file touch (Read/Write/Edit/apply_patch): send the touched file's docs not
  yet in context.

One process per event, so the reset, the prime and the send run in order.
Best-effort: never blocks, never crashes.
"""

import os
import sys

from lib import feedback, tracer
from lib.event import field, patch_target, read_event

BINDING = {
    "events": {
        "SessionStart": [],
        "PreCompact": [],
        "PreToolUse": ["Read", "Write", "Edit", "apply_patch"],
    },
    "harness": "codex",
    "timeout": 15,
    "standalone": True,
    "additionalContextLimit": 0,
}

SOURCE = "inject_rules"


def _send(event, target, event_name, tool):
    rc, text, _ = tracer.docs(event, target, SOURCE, tool)
    if rc == 0 and text.strip():
        feedback.context(SOURCE, event_name, text.strip())


def main():
    if not tracer.available():
        return 0
    event = read_event()
    cwd = field(event, "cwd", "") or os.getcwd()
    event_name = field(event, "hook_event_name", "")
    tool_name = field(event, "tool_name", "")

    if event_name == "PreCompact":
        tracer.run(event, "docs", "reset", "--source", SOURCE)
        return 0

    if event_name == "SessionStart":
        tracer.session_start(event, SOURCE, lambda: _send(event, cwd, "SessionStart", "SessionStart"))
        return 0

    target = ""
    if tool_name == "Read":
        target = field(event, "tool_input.file_path", "") or field(event, "tool_input.path", "")
    elif tool_name == "apply_patch":
        target = patch_target(event)
    elif tool_name in ("Write", "Edit"):
        target = field(event, "tool_input.file_path", "")
    path = tracer.resolve(target, cwd) if target else ""
    if path and os.path.exists(path):
        _send(event, path, "PreToolUse", tool_name or "PreToolUse")
    return 0


if __name__ == "__main__":
    sys.exit(main())
