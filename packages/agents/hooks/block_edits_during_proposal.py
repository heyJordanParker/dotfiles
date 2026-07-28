#!/usr/bin/env python3
"""Block file mutations while session state is "proposing".

Mutation targets are found by quote-aware tokenizing (see command.segments), so a
`>` only counts as a redirect when it's a real shell operator — never when it sits
inside a quoted argument. A read-only command like `gh api -f q='a > b'` therefore
isn't mistaken for a redirect into a repo file and false-blocked, and malformed
input never chokes the parser.
"""

import os
import re
import sys

from lib import agent_memory, feedback
from lib.command import command_head, git_normalize, segments
from lib.event import canonical_tool, field, owner_session, read_event
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

# Commands whose every path argument is a file they create, delete, move, or
# rewrite — so each one is a mutation target. `mv` is here, not in _DEST_MUTATORS,
# because it also removes its source path.
_ALL_ARG_MUTATORS = {"rm", "rmdir", "unlink", "shred", "mkdir", "mkfifo",
                     "mknod", "touch", "truncate", "mv"}
# Commands that read their sources and mutate only the last (destination) path.
_DEST_MUTATORS = {"cp", "install", "ln"}
# Commands whose first path argument is a mode/owner spec, not a file; the rest
# are the files they mutate.
_OWNER_MUTATORS = {"chmod", "chown", "chgrp"}
# git subcommands that mutate the working tree and are not already blocked
# unconditionally by block_git_revert (reset/checkout/restore/stash).
_GIT_TREE_MUTATORS = re.compile(r"git\s+(rm|clean|mv)\b")


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
    if "/docs/shaping/" in t or "/docs/plans/" in t or "/.claude/shaping/" in t or "/.claude/plans/" in t:
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


def _positional_paths(words):
    """Non-flag path arguments of a segment, past the command head, with shell
    redirect operators and their targets excluded — redirects are gathered
    separately. A flag's separate value (`-s 0`) is not distinguishable from a
    path here, so a stray value can read as a target; over-detection only ever
    blocks a mutation, never lets one through."""
    out, skip = [], False
    for t in words[1:]:
        if skip:
            skip = False
            continue
        if _is_redirect(t):
            skip = True
            continue
        if t.startswith("-"):
            continue
        out.append(t)
    return out


def _segment_targets(words):
    """Paths a single command segment would create, write, move, or delete."""
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
            if not t.startswith("-"):
                targets.append(t)
    for t in words:
        if t.startswith("of="):
            targets.append(t[3:])
    positional = _positional_paths(words)
    if head == "sed" and "-i" in words:
        if positional:
            targets.append(positional[-1])
    elif head in _ALL_ARG_MUTATORS:
        targets.extend(positional)
    elif head in _DEST_MUTATORS:
        if positional:
            targets.append(positional[-1])
    elif head in _OWNER_MUTATORS:
        targets.extend(positional[1:])
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
    feedback.context("block_edits_during_proposal", "PreToolUse", msg)


def block(msg=BLOCK_MSG):
    return feedback.block("block_edits_during_proposal", msg)


def main():
    event = read_event()
    # A codex-run agent is a dispatched executor, not a conversation with the
    # architect: its task arrived already scoped, and there is no proposal pending
    # for it to be holding up. The state it does carry is written by the intent
    # classifier reading its dispatch prompt, which defaults to proposing and would
    # otherwise refuse the agent every interpreter and every in-repo write for the
    # whole run.
    if os.environ.get(agent_memory.AGENT_FILE_VAR):
        return 0
    session_id = owner_session(event)
    if not session_id:
        return 0
    state = load_state(session_id).get("state") or "proposing"
    if state != "proposing":
        return 0

    # codex delivers an apply_patch as tool_input.command carrying the patch body,
    # so the shell parse below would read the diff's own text as commands — a `+`
    # line adding a redirect reads as a redirect. The write itself is what this
    # gate cares about, and it has no file_path to check it against.
    if canonical_tool(event) == "write" and not field(event, "tool_input.file_path", ""):
        return block()

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
    if _GIT_TREE_MUTATORS.search(git_normalize(command)):
        return block()
    for seg in segs:
        if command_head(seg) in _INTERPRETERS:
            return block(INTERP_MSG)
        for target in _segment_targets(seg):
            if not _allowed_target(target, cwd):
                return block()
    return 0


if __name__ == "__main__":
    sys.exit(main())
