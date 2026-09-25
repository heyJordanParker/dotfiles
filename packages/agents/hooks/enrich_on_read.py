#!/usr/bin/env python3
"""PreToolUse Read/Edit/Write/Glob/Grep: the tracer context the native tool lacks.

One `trace` call per tool, sized to fit one hook message:

- Read/Edit/Write: `trace context <file>` — the file's facts and every
  declaration it holds; an Edit gets only the declarations around the lines it
  replaces. Only a Read records read coverage, with its line range; an edit is
  not a read.
- Grep: `trace grep` with the native Grep's own pattern, path, case, glob, type
  and multiline settings, so it matches the same files — each named with its
  facts and its matches grouped under their declarations.
- Glob: `trace find` for the same pattern — each matched file with its facts.

The native tool always runs. A failed call on a file that exists says so in
that file's place instead of disappearing.
"""

import os
import sys

from lib import feedback, tracer
from lib.event import field, read_event

BINDING = {
    "events": {"PreToolUse": ["Read", "Glob", "Grep", "Edit", "Write"]},
    "timeout": 15,
    "harness": "all",
    "standalone": True,
    "additionalContextLimit": 0,
}

SOURCE = "enrich_on_read"
NO_MATCHES = "(no matches)"


def _command(event, tool, cwd):
    """(trace arguments, the path they are about) for this tool call, or
    (None, "") when there is nothing to enrich."""
    room = str(tracer.room(SOURCE))
    if tool in ("Read", "Edit", "Write"):
        target = field(event, "tool_input.file_path", "")
        if not target:
            return None, ""
        args = ["context", target, "--budget", room]
        if tool == "Read":
            offset = field(event, "tool_input.offset", None)
            limit = field(event, "tool_input.limit", None)
            if offset is not None:
                args += ["--offset", str(offset)]
            if limit is not None:
                args += ["--limit", str(limit)]
        else:
            args.append("--no-record")
        if tool == "Edit":
            args += _edited_lines(tracer.resolve(target, cwd), field(event, "tool_input.old_string", ""))
        return args, target
    pattern = field(event, "tool_input.pattern", "")
    if not pattern:
        return None, ""
    path = field(event, "tool_input.path", "") or cwd
    if tool == "Glob":
        return ["find", pattern, path, "--budget", room], path
    if tool == "Grep":
        args = ["grep", "--budget", room]
        if field(event, "tool_input.-i", False):
            args.append("-i")
        if field(event, "tool_input.glob", ""):
            args += ["-g", field(event, "tool_input.glob", "")]
        if field(event, "tool_input.type", ""):
            args += ["-t", field(event, "tool_input.type", "")]
        if field(event, "tool_input.multiline", False):
            args.append("-U")
        # ripgrep names files as the path was given: relative to the session's
        # own directory, a name costs its repository-relative part only.
        searched = tracer.resolve(path, cwd)
        if searched == cwd or searched.startswith(cwd.rstrip("/") + "/"):
            searched = os.path.relpath(searched, cwd)
        # A pattern may start with `-`; after `--` it is never read as a flag.
        return args + ["--", pattern, searched], path
    return None, ""


def _edited_lines(path, old):
    try:
        with open(path, encoding="utf-8", errors="replace") as source:
            text = source.read()
    except OSError:
        return []
    at = text.find(old) if old else -1
    if at < 0:
        return []
    return ["--offset", str(text.count("\n", 0, at) + 1), "--limit", str(old.count("\n") + 1)]


def main():
    if not tracer.available():
        return 0
    event = read_event()
    cwd = field(event, "cwd", "") or os.getcwd()
    args, target = _command(event, field(event, "tool_name", ""), cwd)
    if args is None:
        return 0
    rc, out, err = tracer.run(event, *args)
    out = out.strip()
    # Exit 2 names a missing path and still answers for the rest — a new
    # file's directory, the other paths of a search.
    if rc in (0, 2) and out and out != NO_MATCHES:
        return feedback.context(SOURCE, "PreToolUse", out)
    path = tracer.resolve(target, cwd)
    if rc != 0 and os.path.isfile(path):
        reason = "enrichment timed out" if err == "timed out" else "trace failed"
        return feedback.context(SOURCE, "PreToolUse", f"{target}\n[trace context unavailable: {reason}]")
    return 0


if __name__ == "__main__":
    sys.exit(main())
