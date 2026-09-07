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
`trace context <file>` shoulder without recording coverage. guard_trace blocks
raw reads in the same PreToolUse group, so only a successful `trace read`
records the source it actually delivers.

Silent fallback: any error path exits 0 with no output. The native tool runs.
The one exception is a per-file timeout, which says so in that file's place —
dropping it would leave a multi-file shoulder that reads as complete.
"""

import json
import os
import shutil
import subprocess
import sys
import time

from lib import feedback
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
    """The plain file targeted by a read-shaped shell segment."""
    if not parts:
        return ""
    cmd = os.path.basename(parts[0])
    if cmd not in READ_COMMANDS:
        return ""

    skip_value = False
    sed_has_program = False
    for tok in parts[1:]:
        if skip_value:
            skip_value = False
            if cmd == "sed":
                sed_has_program = True
            continue
        if tok.startswith("-"):
            if tok in ("-n", "--lines", "-c", "--bytes") and cmd in ("head", "tail"):
                skip_value = True
            elif tok in ("-e", "--expression", "-f", "--file") and cmd == "sed":
                skip_value = True
            continue
        if cmd == "sed" and not sed_has_program:
            sed_has_program = True
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


TRACE_TIMEOUT = 5

# What a file's shoulder says when trace ran out of time on it. A per-file call
# that overruns used to return "" like any other failure, and the file was then
# dropped from a multi-file shoulder — leaving a block that reads as the complete
# set for the match while silently missing entries, with nothing to tell the
# agent (or a test) which files never got looked at.
UNAVAILABLE = "[trace context unavailable: enrichment timed out]"
FAILED = "[trace context unavailable: trace failed]"


def run_trace(trace_bin, args, env, on_timeout="", timeout=TRACE_TIMEOUT):
    """stdout of `trace <args>`, fluff stripped, or "" on failure.

    A timeout answers `on_timeout`, so a caller that must account for every file
    can say so instead of losing it among the empty results.
    """
    try:
        out = subprocess.run([trace_bin, *args], capture_output=True, text=True,
                             timeout=timeout, env=env)
    except subprocess.TimeoutExpired:
        return on_timeout
    except Exception:
        return ""
    if out.returncode != 0:
        return ""
    return out.stdout.rstrip("\n")


def shoulder(trace_bin, path, env, offset=None, limit=None, record=True):
    """The full `trace context` shoulder for one file.

    `offset`/`limit` are the native Read tool's line-range parameters; when set
    they forward to `trace context` so it records which slice of the file the
    agent read (per-file read coverage). Absent → a whole-file read.

    `record` is False for an Edit/Write: the shoulder still renders (the agent
    gets the file's architectural context before changing it) but `--no-record`
    keeps the touch out of the read-coverage accumulator — an edit is not a read.
    """
    args = ["context", path]
    if offset is not None:
        args += ["--offset", str(offset)]
    if limit is not None:
        args += ["--limit", str(limit)]
    if not record:
        args.append("--no-record")
    return run_trace(trace_bin, args, env, on_timeout=UNAVAILABLE)


def rows(raw):
    """The `results` array of a trace document, or [] for anything else.

    Every `--json` command answers the one `{query, context, results, counts}`
    document, so one reader serves both match resolvers.
    """
    if not raw:
        return []
    try:
        out = json.loads(raw).get("results")
    except Exception:
        return []
    return out if isinstance(out, list) else []


def glob_matches(trace_bin, pattern, base, env, deadline=None):
    """Matched files anchored to the worktree that produced each find row."""
    timeout = TRACE_TIMEOUT if deadline is None else max(0, deadline - time.monotonic())
    if timeout == 0:
        return []
    raw = run_trace(trace_bin, ["find", pattern, base, "--json"], env, timeout=timeout)
    try:
        found_root = subprocess.run(
            ["git", "-C", base, "rev-parse", "--show-toplevel"],
            capture_output=True,
            text=True,
            timeout=min(TRACE_TIMEOUT, max(0, deadline - time.monotonic()))
            if deadline is not None else TRACE_TIMEOUT,
            env=env,
        )
        root = found_root.stdout.strip() if found_root.returncode == 0 else ""
    except Exception:
        root = ""
    root = root or os.path.realpath(base)
    files = []
    for row in rows(raw):
        path = row.get("path") if isinstance(row, dict) else None
        if not path:
            continue
        if os.path.isabs(path):
            files.append(path)
        else:
            files.append(os.path.join(root, path))
    return files


def grep_matches(trace_bin, pattern, path, env, deadline=None):
    """Distinct files containing a Grep match, order preserved."""
    timeout = TRACE_TIMEOUT if deadline is None else max(0, deadline - time.monotonic())
    if timeout == 0:
        return []
    raw = run_trace(
        trace_bin, ["grep", pattern, "--path", path, "--json"], env, timeout=timeout
    )
    files, seen = [], set()
    for hit in rows(raw):
        f = hit.get("file") if isinstance(hit, dict) else None
        if f and f not in seen:
            seen.add(f)
            files.append(f)
    return files


def enrich_matches(trace_bin, files, env, deadline=None):
    """Batch full no-record context without reducing successful-file coverage."""
    queue = list(dict.fromkeys(files))
    block, count = "", 0
    if deadline is None:
        deadline = time.monotonic() + BINDING["timeout"]

    while queue and count < MATCH_CAP:
        size = min(MATCH_CAP - count, len(queue))
        batch, queue = queue[:size], queue[size:]
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            block += "".join(f"{path}\n{UNAVAILABLE}\n" for path in batch)
            if queue:
                block += f"[trace context omitted: {len(queue)} additional matched files omitted]\n"
            return block

        raw = run_trace(
            trace_bin,
            ["context", *batch, "--no-record", "--json"],
            env,
            on_timeout=UNAVAILABLE,
            timeout=remaining,
        )
        if raw == UNAVAILABLE:
            block += "".join(f"{path}\n{UNAVAILABLE}\n" for path in batch)
            continue
        try:
            document = json.loads(raw)
            if set(document) != {"query", "context", "results", "counts"}:
                raise ValueError("incomplete trace document")
            result_rows = document["results"]
            if (
                not isinstance(document["query"], dict)
                or document["query"].get("paths") != batch
                or not isinstance(document["context"], dict)
                or not isinstance(document["counts"], dict)
                or not isinstance(result_rows, list)
                or len(result_rows) != len(batch)
            ):
                raise ValueError("trace results are not a list")
            associated = {
                row.get("file"): row
                for row in result_rows
                if isinstance(row, dict) and isinstance(row.get("file"), str)
            }
        except (TypeError, ValueError, json.JSONDecodeError):
            block += "".join(f"{path}\n{FAILED}\n" for path in batch)
            continue

        for path in batch:
            row = associated.get(path)
            content = row.get("content") if row else ""
            if isinstance(content, str) and content:
                rendered = content.rstrip("\n")
                block += f"{path}\n{rendered}\n"
                count += 1
            else:
                error = row.get("error") if row else None
                marker = f"[trace context unavailable: {error}]" if error else FAILED
                block += f"{path}\n{marker}\n"

    if queue:
        block += f"[trace context omitted: {len(queue)} additional matched files omitted]\n"
    return block


def main():
    # The sleeping-child probe completed in 28.30s with a 28s work budget;
    # two seconds left 1.70s for startup, cleanup and envelope emission.
    deadline = time.monotonic() + BINDING["timeout"] - 2
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
        # Only the native Read tool is a genuine read: it carries a line range
        # and records read coverage. Edit/Write get the same shoulder but with
        # --no-record (record=False) — an edit must not masquerade as a read.
        is_read = tool_name == "Read"
        offset = field(event, "tool_input.offset", None) if is_read else None
        limit = field(event, "tool_input.limit", None) if is_read else None
        output = shoulder(trace_bin, target, env, offset, limit, record=is_read)
    elif tool_name == "Bash":
        segs = segments(command_str(event))
        if not segs:
            return 0
        target = next((found for found in (read_target(s) for s in segs) if found), "")
        if not target:
            return 0
        output = shoulder(trace_bin, target, env, record=False)
    elif tool_name == "Glob":
        pattern = field(event, "tool_input.pattern", "")
        if not pattern:
            return 0
        base = field(event, "tool_input.path", "") or os.environ.get("PWD") or os.getcwd()
        matches = glob_matches(trace_bin, pattern, base, env, deadline)
        output = enrich_matches(trace_bin, matches, env, deadline)
    elif tool_name == "Grep":
        pattern = field(event, "tool_input.pattern", "")
        if not pattern:
            return 0
        path = field(event, "tool_input.path", "") or os.environ.get("PWD") or os.getcwd()
        matches = grep_matches(trace_bin, pattern, path, env, deadline)
        output = enrich_matches(trace_bin, matches, env, deadline)
    else:
        return 0

    if not output:
        return 0
    feedback.context("enrich_on_read", "PreToolUse", output)
    return 0


if __name__ == "__main__":
    sys.exit(main())
