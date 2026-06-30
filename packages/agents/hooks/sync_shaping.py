#!/usr/bin/env python3
"""Shaping/modeling ripple-check reminder on .md writes.

sync-shaping.sh is the plugin-distributed shell copy of this source.
"""

import os
import sys

from lib import feedback
from lib.event import field, read_event

BINDING = {
    "events": {"PostToolUse": ["Write", "Edit"]},
    "timeout": 5,
    "harness": "all",
}

SHAPING_MSG = """Shaping ripple check:
- Changed Requirements? → update Fit Check + any Gaps, Open Questions by Part
- Changed Shape (A, B...) Parts? → update Fit Check + any Gaps, Open Questions by Part
- Changed Boundaries? → check if modeling artifacts + plan still respect them
- Updated a Mermaid diagram? → Affordance tables are the source of truth. Update tables FIRST, then render
- Changed Work Streams Detail? → update Work Streams Mermaid"""

MODELING_MSG = """Modeling sync check:
- Updated affordances.md? → Re-render the 3-section presentation (DB Schema, UX, Architecture). Keep affordances.md structure complete (all UI, Code, Data Store tables with full wiring)
- Updated the presentation? → Verify every affordance from affordances.md appears in exactly one section. Update affordances.md if new affordances were discovered during presentation
- Changed a model/type/architecture? → Update BOTH affordances.md tables AND the shaping doc's parts table
- Changed the shaping doc's parts? → Check if affordances.md + presentation need updating
- Added/removed a boundary in shaping doc? → Check if modeling artifacts respect it"""


def main():
    event = read_event()
    f = field(event, "tool_input.file_path", "")
    if not (f.endswith(".md") and os.path.isfile(f)):
        return 0
    try:
        with open(f, encoding="utf-8", errors="replace") as fh:
            header = []
            for _ in range(5):
                line = fh.readline()
                if not line:
                    break
                header.append(line.rstrip("\n"))
    except Exception:
        return 0
    if any(line.startswith("shaping: true") for line in header):
        return feedback.block("sync_shaping", SHAPING_MSG)
    if any(line.startswith("modeling: true") for line in header):
        return feedback.block("sync_shaping", MODELING_MSG)
    return 0


if __name__ == "__main__":
    sys.exit(main())
