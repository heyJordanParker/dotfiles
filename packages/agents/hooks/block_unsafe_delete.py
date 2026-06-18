#!/usr/bin/env python3
"""Block rm commands outside whitelisted directories.

The whitelist is resolved relative to the repo root, three levels up from this
file (packages/agents/hooks/). block-unsafe-delete.sh is the plugin-distributed
shell copy of this source.

rm is matched as a command word even when reached by a qualified path
(`/bin/rm`) or a `\` alias-bypass (`\rm`), since the word boundary allows an
optional `\` and optional `path/` prefix. A delete whose target the guard can't
statically resolve — fed through a pipe, run inside
a command substitution, or carried by an unexpanded shell variable — is blocked
rather than guessed at: the real path is unknown at guard time, so allowing it
would wave the delete through unchecked.
"""

import os
import re
import sys

from lib.event import command_str, field, read_event

BINDING = {
    "events": {"PreToolUse": ["Bash"]},
    "timeout": 5,
    "harness": "all",
}

_HOOK_DIR = os.path.dirname(os.path.realpath(__file__))
DOTFILES_DIR = os.path.normpath(os.path.join(_HOOK_DIR, "..", "..", ".."))
HOME = os.path.expanduser("~")

ALLOWED_PREFIXES = [
    DOTFILES_DIR,
    HOME + "/Developer",
    HOME + "/Downloads",
    HOME + "/Desktop",
    HOME + "/conductor",
    HOME + "/.claude",
    "/tmp",
]

# rm as a command word: an optional `\` alias-bypass and optional `path/` prefix
# (so `/bin/rm` and `\rm` both match), after start-of-string or a shell
# separator, followed by whitespace or end.
_RM = r"(^|[;&|\s$(`])\\?(\S*/)?rm(\s|$)"
# An unexpanded expansion in an operand — $var, ${x}, $(...), or a backtick.
_EXPANSION = re.compile(r"[$`]")
_GLOB = re.compile(r"[*?\[]")


def is_allowed(path):
    return any(path.startswith(prefix) for prefix in ALLOWED_PREFIXES)


def allowed_list():
    return ", ".join(p.replace(HOME, "~") for p in ALLOWED_PREFIXES)


def block(msg):
    sys.stderr.write(msg + "\n")
    return 2


def unresolvable():
    return block(
        "BLOCKED: rm with a target this guard can't resolve.\n\n"
        "This rm reaches its target through a pipe, a command substitution, or\n"
        "an unexpanded shell variable, so the guard can't tell what it deletes.\n\n"
        "Run rm against a literal path inside an allowed directory:\n"
        "%s\n"
        "Or delete the file manually." % allowed_list()
    )


def main():
    event = read_event()
    command = command_str(event)
    cwd = field(event, "cwd", "")

    if not re.search(_RM, command):
        return 0

    # Piped rm (xargs rm, etc.) — targets come from stdin, unknowable.
    if re.search(r"\|.*rm(\s|$)", command):
        return unresolvable()

    # rm inside a command substitution — $(rm ...) / `rm ...`, qualified path or
    # `\` alias-bypass too.
    if re.search(r"(\$\(|`)\\?(\S*/)?rm\s", command):
        return unresolvable()

    # rm with no operands — nothing to delete.
    if not re.search(r"rm\s+", command):
        return 0

    rest = re.sub(r".*rm\s+", "", command)
    operands = [tok for tok in rest.split() if tok and not tok.startswith("-")]
    if not operands:
        return 0

    # Any operand reached through an unexpanded expansion — can't resolve, block.
    if any(_EXPANSION.search(tok) for tok in operands):
        return unresolvable()

    # Glob operands — the matched set is fixed by cwd at runtime; gate on cwd.
    if any(_GLOB.search(tok) for tok in operands):
        if not is_allowed(cwd):
            return block(
                "BLOCKED: rm with glob pattern outside allowed directories.\n\n"
                "Allowed: %s\n"
                "Please delete these files manually." % allowed_list()
            )
        return 0

    for path in operands:
        if path.startswith("~"):
            path = HOME + path[1:]
        if not path.startswith("/"):
            path = cwd + "/" + path
        path = os.path.normpath(path)
        if not is_allowed(path):
            return block(
                "BLOCKED: Cannot delete '%s'\n\n"
                "Allowed: %s\n"
                "Please delete this file manually." % (path, allowed_list())
            )

    return 0


if __name__ == "__main__":
    sys.exit(main())
