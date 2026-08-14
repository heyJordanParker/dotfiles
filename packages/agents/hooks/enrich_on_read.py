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
The one exception is a per-file timeout, which says so in that file's place —
dropping it would leave a multi-file shoulder that reads as complete.
"""

import json
import os
import re
import shutil
import subprocess
import sys

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

# A `sed -n 'A,Bp'` line-range print: the address range the agent actually saw.
SED_RANGE = re.compile(r"^(\d+),(\d+)p$")
# A `head -n N` / `head -N` line count: the agent saw lines 1..N.
HEAD_COUNT = re.compile(r"^-n?(\d+)$")

# Cap on matched-file enrichment for the multi-file tools (Glob/Grep). The full
# `trace context` shoulder costs a git + graph lookup per file; an unbounded loop
# over a wide match set would blow the hook timeout and yield nothing. The cap
# keeps the shoulder useful while staying well inside the timeout.
MATCH_CAP = 20


def read_target(parts):
    """The file a read-shaped shell segment targets and the span it shows.

    parts is one tokenized segment (command + args). Returns
    `(path, offset, limit)` — the first plain path argument plus the 1-based
    inclusive line span the command renders, so a partial read records only the
    portion the agent saw, not the whole file:
      - `cat`/`bat`/`less`/`more`/`view` → `(path, None, None)` (whole file)
      - `head -n N` / `head -N`          → `(path, 1, N)`
      - `sed -n 'A,Bp'`                  → `(path, A, B - A + 1)`
    Returns `("", None, None)` when the segment isn't a read of a plain repo path
    (flags/globs/option values), or when the span is unknowable (`tail` reads the
    last N lines, whose start depends on the file length the hook can't see — a
    whole-file record would over-count and a positioned guess would be fiction, so
    `tail` is skipped rather than mis-recorded).
    """
    none = ("", None, None)
    if not parts:
        return none
    cmd = os.path.basename(parts[0])
    if cmd not in READ_COMMANDS:
        return none

    offset, limit = None, None
    want_count = False  # the previous token was a bare `-n` expecting its value
    for tok in parts[1:]:
        if want_count:
            want_count = False
            if cmd == "head" and tok.isdigit():
                offset, limit = 1, int(tok)
                continue
        if tok.startswith("-"):
            if cmd == "head":
                m = HEAD_COUNT.match(tok)
                if m:
                    offset, limit = 1, int(m.group(1))
                elif tok == "-n":
                    want_count = True  # count is the next token (`head -n 5`)
            continue
        if cmd == "sed":
            m = SED_RANGE.match(tok)
            if m:
                a, b = int(m.group(1)), int(m.group(2))
                if b >= a:
                    offset, limit = a, b - a + 1
                continue
        if any(c in tok for c in "$`*?[]{}"):
            return none
        # `tail` shows the last N lines — the shown span's start is unknowable
        # without the file length, so it must not be recorded as a read.
        if cmd == "tail":
            return none
        return (tok, offset, limit)
    return none


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


def run_trace(trace_bin, args, env, on_timeout=""):
    """stdout of `trace <args>`, fluff stripped, or "" on failure.

    A timeout answers `on_timeout`, so a caller that must account for every file
    can say so instead of losing it among the empty results.
    """
    try:
        out = subprocess.run([trace_bin, *args], capture_output=True, text=True,
                             timeout=TRACE_TIMEOUT, env=env)
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

    `record=False`: a Glob listing or Grep match surfaces a file's path (and one
    matching line) but never its content, so the shoulder renders without
    recording a read — a match must not inflate the matched file's read coverage.
    """
    block, count = "", 0
    for f in files:
        if count >= MATCH_CAP:
            break
        line = shoulder(trace_bin, f, env, record=False)
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
        target, offset, limit = next(
            (found for found in (read_target(s) for s in segs) if found[0]),
            ("", None, None),
        )
        if not target:
            return 0
        output = shoulder(trace_bin, target, env, offset, limit)
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
    feedback.context("enrich_on_read", "PreToolUse", output)
    return 0


if __name__ == "__main__":
    sys.exit(main())
