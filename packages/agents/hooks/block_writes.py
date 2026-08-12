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

from lib import feedback
from lib.command import all_segments, command_head, git_normalize, mutation_targets
from lib.event import canonical_tool, field, read_event
from lib.session_mode import is_dispatched, permits, state

BINDING = {
    "events": {
        "PreToolUse": ["Bash", "Write", "Edit", "MultiEdit", "NotebookEdit"],
    },
    "harness": "all",
    "timeout": 5,
}

BLOCK_MSG = """BLOCKED: A proposal is expected — do not edit code.

Update your proposal based on the user's feedback and present it again.
Only edit code after the user approves.

To write a plan, shaping doc, or Evidence, use docs/plans/, docs/shaping/,
docs/agents/, or /tmp/ — this gate does not apply there. Never relocate a
file elsewhere to dodge the gate."""

MODE_MSG = """BLOCKED: this mode does not write files.

Reading, searching, and read-only commands all still run. To change a file, dispatch
a subagent to make the edit, or switch the session's mode."""

_DEVICES = {"/dev/null", "/dev/stdout", "/dev/stderr", "/dev/tty"}
_ASSIGN = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*=")
_OUR_TREE = ("/dotfiles/", "/.agents/", "/.claude/")

# git subcommands that mutate the working tree and are not already blocked
# unconditionally by block_git_revert (reset/checkout/restore/stash).
_GIT_TREE_MUTATORS = re.compile(r"git\s+(rm|clean|mv)\b")


def _is_our_tree(path):
    return any(fragment in path + "/" for fragment in _OUR_TREE)


def _allowed_target(t, cwd):
    """True = allowed, False = blocked (a mutation that lands in our tree)."""
    t = t.strip().strip('"').strip("'")
    if t in _DEVICES:
        return True
    if t.startswith("~"):
        t = os.path.expanduser("~") + t[1:]
    if not t.startswith("/"):
        t = os.path.join(cwd, t)
    t = os.path.normpath(t)
    # t + "/" so the directory itself is allowed, not only paths inside it —
    # `mkdir -p docs/agents` must pass, not just writes to files under it.
    probe = t + "/"
    for d in ("/docs/shaping/", "/docs/plans/", "/docs/agents/",
              "/.claude/shaping/", "/.claude/plans/"):
        if d in probe:
            return True
    if t == "/tmp" or t.startswith("/tmp/"):
        return True
    if t == "/private/tmp" or t.startswith("/private/tmp/"):
        return True
    if t == cwd or t.startswith(cwd + "/") or _is_our_tree(t):
        return False
    return True


INTERP_MSG = """BLOCKED: running an interpreter is disabled while a proposal is expected.

Writing files is fine (including under /tmp), and scripts already in the repo still
run — but a script from outside it, or code passed inline, is blocked until the
proposal is approved. Run it after approval, or ask the user to run it manually.
Codex review still works via codex-run."""

SCRIPT_WARNING = ("Heads up — a proposal is expected: you can write this script, but "
                  "executing it (python/node/bash/…) is blocked until the proposal is approved.")

_INTERPRETERS = {"python", "python3", "python2", "node", "deno",
                 "perl", "ruby", "bash", "sh", "zsh"}
_SCRIPT_EXTS = (".py", ".sh", ".bash", ".zsh", ".js", ".mjs", ".cjs", ".ts", ".rb", ".pl")


def _is_script(path):
    return path.strip().strip("\"'").endswith(_SCRIPT_EXTS)


def _allowed_interpreter(words, cwd):
    i = 0
    while i < len(words) and _ASSIGN.match(words[i]):
        i += 1
    if i < len(words) and os.path.basename(words[i].strip("\"'")) == "env":
        i += 1
        while i < len(words) and _ASSIGN.match(words[i]):
            i += 1
    i += 1
    if any(t == "-c" or t == "-e" or t == "--command" or t == "--eval" for t in words[i:]):
        return False
    for t in words[i:]:
        if t.startswith("-"):
            continue
        if t.startswith("~"):
            t = os.path.expanduser("~") + t[1:]
        if not t.startswith("/"):
            t = os.path.join(cwd, t)
        t = os.path.realpath(t)
        return os.path.isfile(t) and _is_our_tree(t)
    return False


def warn(msg):
    feedback.context("block_writes", "PreToolUse", msg)


def block(msg=BLOCK_MSG):
    return feedback.block("block_writes", msg)


def main():
    event = read_event()
    # A dispatched orchestrator is forbidden MUTATIONS, not Bash: the same target
    # parser the proposing arm uses decides, so `pytest`, `ls`, `echo`, and
    # `git status` all run — an orchestrator has to be able to validate what its
    # subagents built — and only a command that changes the tree is refused.
    banned = not permits(event, "write")
    # The proposing arm governs every session the architect drives himself, his
    # hand-managed teammates included. A dispatched agent's task arrived already
    # scoped and there is no proposal pending for it to hold up, so it alone is
    # exempt — and what counts as dispatched is session_mode's one definition.
    proposing = not is_dispatched(event) and state(event) == "propose"
    if not proposing and not banned:
        return 0
    refusal = MODE_MSG if banned else BLOCK_MSG

    cwd = field(event, "cwd", "") or os.getcwd()
    file_path = field(event, "tool_input.file_path", "")
    command = field(event, "tool_input.command", "")

    if canonical_tool(event) == "write":
        if banned:
            return block(MODE_MSG)
        # codex delivers an apply_patch as tool_input.command carrying the patch body,
        # so the shell parse below would read the diff's own text as commands — a `+`
        # line adding a redirect reads as a redirect. The write itself is what this
        # gate cares about, and it has no file_path to check it against.
        if not file_path:
            return block()

    if file_path:
        if not _allowed_target(file_path, cwd):
            return block(refusal)
        if _is_script(file_path):
            warn(SCRIPT_WARNING)
        return 0

    if not command:
        return 0

    segs = all_segments(command)
    if segs is None:
        return 0  # unparseable — never choke, never false-block a read
    if _GIT_TREE_MUTATORS.search(git_normalize(command)):
        return block(refusal)
    for seg in segs:
        if proposing and command_head(seg) in _INTERPRETERS:
            if not _allowed_interpreter(seg, cwd):
                return block(INTERP_MSG)
        for target in mutation_targets(seg):
            if not _allowed_target(target, cwd):
                return block(refusal)
    return 0


if __name__ == "__main__":
    sys.exit(main())
