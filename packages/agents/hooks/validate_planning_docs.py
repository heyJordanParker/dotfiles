#!/usr/bin/env python3
"""Ban deferred work in planning documents (LLM check).

Planning docs are identified by path under docs/shaping/ (or the legacy
.claude/shaping/) or a shaping/modeling/slicing frontmatter marker; their content is checked for
deferral by `claude -p`. Any failure path allows. validate-planning-docs.sh is
the plugin-distributed shell copy of this source.
"""

import os
import re
import sys

from lib import feedback
from lib.event import field, read_event
from lib.model_call import run_model

BINDING = {
    "events": {"PreToolUse": ["Write", "Edit", "MultiEdit"]},
    "timeout": 60,
    "harness": "all",
}

JSON_SCHEMA = '{"type":"object","properties":{"ok":{"type":"boolean"},"reason":{"type":"string"}},"required":["ok"]}'
SYSTEM_PROMPT = "You are a planning document validator. Check if content defers any work. You only RAISE the concern in `reason`; you never prescribe a fix — the main agent owns that. Output structured JSON only."

_MARKER = re.compile(r"^(shaping|modeling|slicing): true", re.M)


def _eval_prompt(file_path, content):
    return (
        'Does this planning document content defer any work?\n\n'
        'Rules:\n'
        '- Every item in a planning document ships. If it doesn\'t ship, it doesn\'t go in the document\n'
        '- Deferral includes: items marked as "deferred", punted to a future phase, declared out of scope for now, or any form of "deferred" or "this ships later, not now"\n'
        '- Slice labels (V1, V2, V3) and phase labels (Phase 1, Phase 2) are implementation ordering — all slices ship. They are NOT deferral unless they explicitly state work is pushed out of scope or will be done later\n'
        '- Optional items ("Nice-to-have", "Undecided", "if time allows") are also banned — every item in a planning doc is committed work, not a maybe. If it\'s not in scope, it\'s not in the document\n'
        '- Presenting options ("Option A or B", "we could do X or Y") is also banned — the user decides options and scope before the plan is finalized. Plans contain decisions, not choices\n'
        '- For Edit operations, you only see the replacement text (new_string), not the full document. Judge only what you see — do not infer deferral from slice/phase labels alone\n\n'
        'File: %s\n'
        'Content:\n'
        '%s\n\n'
        'Return JSON:\n'
        '- No deferral: {"ok": true}\n'
        '- Deferral found: {"ok": false, "reason": "[what was deferred and where]"}'
    ) % (file_path, content)


def main():
    event = read_event()
    session_id = field(event, "session_id", "")
    if not session_id or session_id.startswith("agent-"):
        return 0
    tool_name = field(event, "tool_name", "")
    if tool_name == "Write":
        file_path = field(event, "tool_input.file_path", "")
        content = field(event, "tool_input.content", "")
    elif tool_name == "Edit":
        file_path = field(event, "tool_input.file_path", "")
        content = field(event, "tool_input.new_string", "")
    elif tool_name == "apply_patch":
        patch = (field(event, "tool_input.input", "") or field(event, "tool_input.patch", "")
                 or field(event, "tool_input.changes", ""))
        m = re.search(r"^\*\*\* (?:Update|Add|Delete) File: (.+)$", patch, re.M)
        file_path = m.group(1) if m else ""
        content = "\n".join(ln[1:] for ln in patch.splitlines()
                            if ln.startswith("+") and not ln.startswith("+++"))
    else:
        return 0

    if not file_path.endswith(".md"):
        return 0

    is_planning = "/docs/shaping/" in file_path or "/.claude/shaping/" in file_path
    if not is_planning:
        if os.path.isfile(file_path):
            try:
                with open(file_path, encoding="utf-8", errors="replace") as fh:
                    header = "".join(fh.readline() for _ in range(5))
            except Exception:
                header = ""
        else:
            header = "\n".join(content.split("\n")[:5])
        if _MARKER.search(header):
            is_planning = True

    if not is_planning:
        return 0

    result = run_model(system_prompt=SYSTEM_PROMPT, user_prompt=_eval_prompt(file_path, content),
                       schema=JSON_SCHEMA)
    if not result:
        return 0
    if result.get("ok") is False:
        reason = result.get("reason") or "the planning document may defer work"
        return feedback.raise_concern("validate_planning_docs", "PreToolUse", "Potential issue: %s" % reason)
    return 0


if __name__ == "__main__":
    sys.exit(main())
