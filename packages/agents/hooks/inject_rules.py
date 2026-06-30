#!/usr/bin/env python3
"""Inject project rules (the nearest Claude.md) for Codex, which has no native
rule-file loading. Claude Code loads Claude.md (root and nested) itself, so this
is Codex-only.

Two surfaces, both emitting trace's docs as a hookSpecificOutput.additionalContext
envelope (Codex 0.137+ rejects a raw `trace docs` JSON object on SessionStart —
it must be wrapped): SessionStart loads the repo-root rules (with a `trace docs
reset` on clear/compact), and a file touch (Read/Write/Edit/apply_patch) loads
the touched file's rules. Best-effort: never blocks, never crashes.
"""

import json
import os
import re
import shutil
import subprocess
import sys

from lib import feedback
from lib.event import field, read_event

BINDING = {
    "events": {
        "SessionStart": [],
        "PreToolUse": ["Read", "Write", "Edit", "apply_patch"],
    },
    "harness": "codex",
    "timeout": 15,
}

SOURCE = "inject_rules"
_PATCH_FILE = re.compile(r"^\*\*\* (?:Update|Add|Delete) File: (.+)$", re.M)


def _resolve(target, cwd):
    target = target.strip().strip('"').strip("'")
    if not target:
        return ""
    if target.startswith("~"):
        target = os.path.expanduser("~") + target[1:]
    if not target.startswith("/"):
        target = os.path.join(cwd, target)
    return os.path.normpath(target)


def _trace_docs(target, triggering_tool, env):
    args = ["trace", "docs", target, "--source", SOURCE, "--triggering-tool", triggering_tool, "--json"]
    try:
        r = subprocess.run(args, capture_output=True, text=True, timeout=10, env=env)
        return r.returncode, r.stdout
    except Exception:
        return 1, ""


def _doc_count(response):
    try:
        return json.loads(response).get("doc_count", 0) or 0
    except Exception:
        return 0


def _emit_json(response, event_name="PreToolUse"):
    feedback.context("inject_rules", event_name, response)


def main():
    if not shutil.which("trace"):
        return 0
    event = read_event()
    cwd = field(event, "cwd", "") or os.getcwd()

    # Hand trace the run's own session for its session log via AGENT_SESSION_ID,
    # the harness-neutral carrier trace resolves first — on a local copy only,
    # never mutating os.environ. CLAUDE_CODE_SESSION_ID is left untouched: on a
    # codex run it carries the launching Claude session that owner_session reads
    # to resolve the governing proposing/executing mode, so clobbering it would
    # destroy the launcher identity the proposal/commit guards depend on.
    env = dict(os.environ)
    session_id = field(event, "session_id", "")
    agent_id = field(event, "agent_id", "")
    if session_id:
        env["AGENT_SESSION_ID"] = session_id
    if agent_id:
        env["TRACER_AGENT_ID"] = agent_id

    event_name = field(event, "hook_event_name", "")
    tool_name = field(event, "tool_name", "")

    # SessionStart — always-on rules for the repo root.
    if event_name == "SessionStart":
        if field(event, "source", "") in ("clear", "compact"):
            try:
                subprocess.run(["trace", "docs", "reset", "--source", SOURCE], capture_output=True, timeout=10, env=env)
            except Exception:
                pass
        rc, out = _trace_docs(cwd, event_name, env)
        if rc != 0 or not _doc_count(out):
            return 0
        _emit_json(out, "SessionStart")
        return 0

    # File tools (Codex apply_patch/Read, Write/Edit) — rules for the touched file.
    target = ""
    if tool_name == "Read":
        target = field(event, "tool_input.file_path", "") or field(event, "tool_input.path", "")
    elif tool_name == "apply_patch":
        patch = (field(event, "tool_input.input", "") or field(event, "tool_input.patch", "")
                 or field(event, "tool_input.changes", ""))
        m = _PATCH_FILE.search(patch)
        target = m.group(1) if m else ""
    elif tool_name in ("Write", "Edit"):
        target = field(event, "tool_input.file_path", "")
    if not target:
        return 0
    rp = _resolve(target, cwd)
    if not (rp and os.path.exists(rp)):
        return 0
    rc, out = _trace_docs(rp, tool_name or "PreToolUse", env)
    if rc != 0 or not _doc_count(out):
        return 0
    _emit_json(out)
    return 0


if __name__ == "__main__":
    sys.exit(main())
