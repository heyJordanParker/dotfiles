#!/usr/bin/env python3
"""PreToolUse Read/Edit/Write/Glob/Grep/Bash: inject the tracer shoulder as additionalContext.

Enriches the native file-touch tools with the tracer signal they don't carry:
Read/Edit/Write get one `trace context <file>` for the target; Glob/Grep resolve
their matched files and emit one full `trace context` shoulder per matched file
(capped) — the rich lifecycle/complexity/graph shoulder, not the thin per-line
`--details` / grep header.

Codex's CLI has no native Read/Glob/Grep tool — it reads files only through its
shell tool, which arrives here as tool_name "Bash" with the command in
tool_input.command. So a Codex `cat <file>` would carry no shoulder without the
Bash branch. When the shell command is a read of a repo file, emit the same
`trace context <file>` shoulder. guard_trace blocks the raw read in the same
PreToolUse group, but Codex still delivers this additionalContext to the model
(the two compose: guard steers to `trace read`, enrich supplies the shoulder).

Silent fallback: any error path exits 0 with no output. The native tool runs.
"""

import json
import os
import shutil
import subprocess
import sys

from lib.command import segments
from lib.event import command_str, field, read_event

BINDING = {
    "events": {"PreToolUse": ["Read", "Glob", "Grep", "Edit", "Write"]},
    "timeout": 30,
    "harness": "all",
}

# Read-shaped shell commands that take a file path and that guard_trace forces
# onto `trace read`. Matching this set means the shoulder fires exactly when a
# Codex shell read of a repo file would otherwise pass uninstrumented.
READ_COMMANDS = {"cat", "head", "tail", "sed", "less", "more", "view", "bat"}

# Cap on matched-file enrichment for the multi-file tools (Glob/Grep). The full
# `trace context` shoulder costs a git + graph lookup per file; an unbounded loop
# over a wide match set would blow the hook timeout and yield nothing. The cap
# keeps the shoulder useful while staying well inside the timeout.
MATCH_CAP = 20


def read_target(parts):
    """The file a read-shaped shell segment targets, or "" if it isn't one.

    parts is one tokenized segment (command + args). Returns the first plain
    path argument of a read command; flags, option values, and globs yield "".
    """
    if not parts or os.path.basename(parts[0]) not in READ_COMMANDS:
        return ""
    for tok in parts[1:]:
        if tok.startswith("-"):
            continue
        if any(c in tok for c in "$`*?[]{}"):
            return ""
        return tok
    return ""


def resolve_trace_bin():
    """The trace binary: `trace` on PATH, else the plugin launcher, else "" """
    trace_bin = shutil.which("trace")
    if not trace_bin:
        plugin_root = os.environ.get("CLAUDE_PLUGIN_ROOT") or os.path.join(
            os.path.expanduser("~"), ".claude/plugins/talents/talent-tree/packages/claude"
        )
        trace_bin = os.path.join(plugin_root, "bin", "trace")
    if os.path.isfile(trace_bin) and os.access(trace_bin, os.X_OK):
        return trace_bin
    return ""


def run_trace(trace_bin, args, env):
    """stdout of `trace <args>`, fluff stripped, or "" on any failure/timeout."""
    try:
        out = subprocess.run([trace_bin, *args], capture_output=True, text=True, timeout=5, env=env)
    except Exception:
        return ""
    if out.returncode != 0:
        return ""
    return out.stdout.rstrip("\n")


def shoulder(trace_bin, path, env):
    """The full `trace context` shoulder for one file."""
    return run_trace(trace_bin, ["context", path], env)


def glob_matches(trace_bin, pattern, base, env):
    """Matched files for a Glob, each prefixed with <base> so it resolves.

    `trace glob` returns matches relative to <base>; prepend it so each path
    resolves for the per-file `trace context` shoulder.
    """
    raw = run_trace(trace_bin, ["glob", pattern, base, "--json"], env)
    if not raw:
        return []
    try:
        matches = json.loads(raw).get("matches", []) or []
    except Exception:
        return []
    return [os.path.join(base, m) for m in matches if m]


def grep_matches(trace_bin, pattern, path, env):
    """Distinct files containing a Grep match, order preserved."""
    raw = run_trace(trace_bin, ["grep", pattern, "--path", path, "--json"], env)
    if not raw:
        return []
    try:
        hits = json.loads(raw).get("matches", []) or []
    except Exception:
        return []
    files, seen = [], set()
    for hit in hits:
        f = hit.get("file") if isinstance(hit, dict) else None
        if f and f not in seen:
            seen.add(f)
            files.append(f)
    return files


def enrich_matches(trace_bin, files, env):
    """One `trace context` shoulder per matched file, capped at MATCH_CAP.

    Each kept file contributes "<file>\\n<shoulder>\\n"; files whose shoulder
    comes back empty are skipped and don't count toward the cap.
    """
    block, count = "", 0
    for f in files:
        if count >= MATCH_CAP:
            break
        line = shoulder(trace_bin, f, env)
        if not line:
            continue
        block += f"{f}\n{line}\n"
        count += 1
    return block


def main():
    event = read_event()
    tool_name = field(event, "tool_name", "")

    # Hand trace the run's own session via AGENT_SESSION_ID — the harness-neutral
    # carrier trace resolves first — on a local copy only, never mutating
    # os.environ; CLAUDE_CODE_SESSION_ID stays as the launcher set it so
    # owner_session can resolve the governing mode on a nested codex run.
    env = dict(os.environ)
    session_id = field(event, "session_id", "")
    agent_id = field(event, "agent_id", "")
    if session_id:
        env["AGENT_SESSION_ID"] = session_id
    if agent_id:
        env["TRACER_AGENT_ID"] = agent_id

    trace_bin = resolve_trace_bin()
    if not trace_bin:
        return 0

    if tool_name in ("Read", "Edit", "Write"):
        target = field(event, "tool_input.file_path", "")
        if not target:
            return 0
        output = shoulder(trace_bin, target, env)
    elif tool_name == "Bash":
        segs = segments(command_str(event))
        if not segs:
            return 0
        target = next((t for t in (read_target(s) for s in segs) if t), "")
        if not target:
            return 0
        output = shoulder(trace_bin, target, env)
    elif tool_name == "Glob":
        pattern = field(event, "tool_input.pattern", "")
        if not pattern:
            return 0
        base = field(event, "tool_input.path", "") or os.environ.get("PWD") or os.getcwd()
        output = enrich_matches(trace_bin, glob_matches(trace_bin, pattern, base, env), env)
    elif tool_name == "Grep":
        pattern = field(event, "tool_input.pattern", "")
        if not pattern:
            return 0
        path = field(event, "tool_input.path", "") or os.environ.get("PWD") or os.getcwd()
        output = enrich_matches(trace_bin, grep_matches(trace_bin, pattern, path, env), env)
    else:
        return 0

    if not output:
        return 0
    print(json.dumps(
        {"hookSpecificOutput": {"hookEventName": "PreToolUse", "additionalContext": output}},
        separators=(",", ":"), ensure_ascii=False,
    ))
    return 0


if __name__ == "__main__":
    sys.exit(main())
