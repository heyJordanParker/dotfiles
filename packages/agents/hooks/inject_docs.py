#!/usr/bin/env python3
"""Ensure a `trace` command's target has its project docs in context.

When the agent runs a path-taking `trace <subcmd> <path>` shell command, this
loads that path's project docs via `trace docs` and injects them as a
hookSpecificOutput.additionalContext envelope. Blocks the command (exit 2) if
`trace docs` fails, so the agent never traces without project-docs context.
Both harnesses run trace, so both run this. Never crashes.
"""

import json
import os
import shutil
import subprocess
import sys

from lib.event import field, read_event

BINDING = {
    "events": {"PreToolUse": ["Bash"]},
    "harness": "all",
    "timeout": 15,
}

SOURCE = "inject_docs"
PATH_TAKING = {
    "read", "info", "list", "tree", "structure", "grep",
    "struct", "find", "glob", "blame", "history", "diff",
}


def _resolve(target, cwd):
    target = target.strip().strip('"').strip("'")
    if not target:
        return ""
    if target.startswith("~"):
        target = os.path.expanduser("~") + target[1:]
    if not target.startswith("/"):
        target = os.path.join(cwd, target)
    return os.path.normpath(target)


def _trace_docs(target, triggering_tool, env, triggering_command=None):
    args = ["trace", "docs", target, "--source", SOURCE, "--triggering-tool", triggering_tool]
    if triggering_command is not None:
        args += ["--triggering-command", triggering_command]
    args += ["--json"]
    try:
        r = subprocess.run(args, capture_output=True, text=True, timeout=10, env=env)
        return r.returncode, r.stdout, r.stderr
    except Exception:
        return 1, "", ""


def _doc_count(response):
    try:
        return json.loads(response).get("doc_count", 0) or 0
    except Exception:
        return 0


def _emit_json(response, event_name="PreToolUse"):
    print(json.dumps(
        {"hookSpecificOutput": {"hookEventName": event_name, "additionalContext": response}},
        separators=(",", ":"), ensure_ascii=False,
    ))


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

    # Bash `trace` command — docs for the trace target; block on failure.
    command = field(event, "tool_input.command", "")
    if not command:
        return 0
    toks = command.split()
    subcmd, pathtok = "", ""
    for i, t in enumerate(toks):
        if os.path.basename(t) == "trace":
            subcmd = toks[i + 1] if i + 1 < len(toks) else ""
            for j in range(i + 2, len(toks)):
                if not toks[j].startswith("-"):
                    pathtok = toks[j]
                    break
            break
    if subcmd not in PATH_TAKING:
        return 0
    rp = _resolve(pathtok, cwd) if pathtok else ""
    target = rp if (rp and os.path.exists(rp)) else cwd
    rc, out, err = _trace_docs(target, "Bash", env, command)
    if rc != 0:
        sys.stderr.write(
            "BLOCKED: project-docs load failed for: %s\n\n"
            "`trace docs \"%s\" ...` exited %d. The trace command is\n"
            "blocked so the agent does not run it without project-docs context.\n\n"
            "Underlying error:\n%s\n" % (target, target, rc, err)
        )
        return 2
    if not _doc_count(out):
        return 0
    _emit_json(out)
    return 0


if __name__ == "__main__":
    sys.exit(main())
