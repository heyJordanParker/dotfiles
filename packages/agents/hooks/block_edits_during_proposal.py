#!/usr/bin/env python3
"""Block file mutations while session state is "proposing".

Mutation targets are found by quote-aware tokenizing (see command.segments), so a
`>` only counts as a redirect when it's a real shell operator — never when it sits
inside a quoted argument. A read-only command like `gh api -f q='a > b'` therefore
isn't mistaken for a redirect into a repo file and false-blocked, and malformed
input never chokes the parser.
"""

import json
import os
import re
import sys

from lib.command import command_head, segments
from lib.event import field, owner_session, read_event
from lib.session_state import load_state

BINDING = {
    "events": {
        "PreToolUse": ["Bash", "Write", "Edit", "MultiEdit", "NotebookEdit"],
    },
    "harness": "all",
    "timeout": 5,
}

BLOCK_MSG = """BLOCKED: A proposal is expected — do not edit code.

Update your proposal based on the user's feedback and present it again.
Only edit code after the user approves."""

_DEVICES = {"/dev/null", "/dev/stdout", "/dev/stderr", "/dev/tty"}
_ASSIGN = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*=")


def _allowed_target(t, cwd):
    """True = allowed, False = blocked (a mutation that lands inside the repo)."""
    t = t.strip().strip('"').strip("'")
    if t in _DEVICES:
        return True
    if t.startswith("~"):
        t = os.path.expanduser("~") + t[1:]
    if not t.startswith("/"):
        t = os.path.join(cwd, t)
    t = os.path.normpath(t)
    if "/.claude/shaping/" in t or "/.claude/plans/" in t:
        return True
    if t == "/tmp" or t.startswith("/tmp/"):
        return True
    if t == "/private/tmp" or t.startswith("/private/tmp/"):
        return True
    if t == cwd or t.startswith(cwd + "/"):
        return False
    return True


def _is_redirect(tok):
    return ">" in tok and tok != "" and tok.strip("0123456789&<>") == ""


def _is_fd_reference(tok):
    """A redirect target that names a file descriptor, not a file: `2>&1`, `>&2`, `>&-`."""
    return tok == "-" or tok.isdigit()


def _segment_targets(words):
    """Paths a single command segment would write."""
    i = 0
    while i < len(words) and _ASSIGN.match(words[i]):
        i += 1
    words = words[i:]
    if not words:
        return []
    head = os.path.basename(words[0])
    targets = []
    for j, t in enumerate(words):
        if _is_redirect(t) and j + 1 < len(words) and not _is_fd_reference(words[j + 1]):
            targets.append(words[j + 1])
    if "tee" in words:
        for t in words[words.index("tee") + 1:]:
            if t.startswith("-"):
                continue
            targets.append(t)
    if head == "sed" and "-i" in words:
        nonflags = [t for t in words[1:] if not t.startswith("-")]
        if nonflags:
            targets.append(nonflags[-1])
    for t in words:
        if t.startswith("of="):
            targets.append(t[3:])
    if head in ("cp", "mv", "install"):
        nonflags = [t for t in words[1:] if not t.startswith("-")]
        if nonflags:
            targets.append(nonflags[-1])
    if head in ("touch", "truncate"):
        for t in words[1:]:
            if not t.startswith("-"):
                targets.append(t)
    return targets


INTERP_MSG = """BLOCKED: running an interpreter is disabled while a proposal is expected.

Writing files is fine (including under /tmp) — but executing python/node/bash/etc.,
inline or as a script, is blocked until the proposal is approved. Run it after
approval, or ask the user to run it manually. Codex review still works via codex-run."""

SCRIPT_WARNING = ("Heads up — a proposal is expected: you can write this script, but "
                  "executing it (python/node/bash/…) is blocked until the proposal is approved.")

_INTERPRETERS = {"python", "python3", "python2", "node", "deno",
                 "perl", "ruby", "bash", "sh", "zsh"}
_SCRIPT_EXTS = (".py", ".sh", ".bash", ".zsh", ".js", ".mjs", ".cjs", ".ts", ".rb", ".pl")


def _is_script(path):
    return path.strip().strip("\"'").endswith(_SCRIPT_EXTS)


def warn(msg):
    print(json.dumps({"hookSpecificOutput": {
        "hookEventName": "PreToolUse", "additionalContext": msg}}))


def block(msg=BLOCK_MSG):
    sys.stderr.write(msg + "\n")
    return 2


def main():
    event = read_event()
    session_id = owner_session(event)
    if not session_id:
        return 0
    state = load_state(session_id).get("state") or "proposing"
    if state != "proposing":
        return 0

    cwd = field(event, "cwd", "") or os.getcwd()
    file_path = field(event, "tool_input.file_path", "")
    command = field(event, "tool_input.command", "")

    if file_path:
        if not _allowed_target(file_path, cwd):
            return block()
        if _is_script(file_path):
            warn(SCRIPT_WARNING)
        return 0

    if not command:
        return 0

    segs = segments(command)
    if segs is None:
        return 0  # unparseable — never choke, never false-block a read
    for seg in segs:
        if command_head(seg) in _INTERPRETERS:
            return block(INTERP_MSG)
        for target in _segment_targets(seg):
            if not _allowed_target(target, cwd):
                return block()
    return 0


if __name__ == "__main__":
    sys.exit(main())
