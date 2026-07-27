"""The `memory: none` declaration, read from an agent's definition file.

One declaration governs an agent on both harnesses, so both enforcement points
must agree on what the declaration looks like. They did not share code, and the
syntax was defined twice — the Claude gate (block_memory_access.py) and the
codex gate (codex_run.py) could silently drift apart. This module is the single
definition; each gate keeps only its own way of locating the file, because the
harnesses locate it differently (Claude resolves the name under the active
CLAUDE_CONFIG_DIR, codex under whichever roster supplied the instructions).

The two failure directions are not symmetric. A *missing* declaration must leave
Memory reachable: that is the contract, and every agent without the key relies on
it. An
*explicit* declaration must never quietly become permission, so anything that is
a real `memory: none` has to be read as one, and a definition that exists but
cannot be read is treated as a denial rather than assumed harmless.

Matching scripts/frontmatter.py is the point of the parsing here. That module is
this repository's frontmatter precedent and it unquotes a matching pair of
quotes, so `memory: "none"` and `memory: 'none'` are valid spellings of the same
declaration and are honoured as such. A trailing `# comment` is stripped too —
frontmatter.py does not do that, but reading `none # one-shot` as anything other
than a denial would be the exact failure this module exists to stop. Widening
what counts as a denial is safe; widening what counts as permission is not.

Scans the frontmatter block directly rather than importing frontmatter.py
itself: that module is a build-time dependency of sync.py and is never stowed to
~/.agents, where both gates run from.
"""

import re

# scripts/frontmatter.py's key pattern, so the two agree on what a key line is.
_KEY = re.compile(r"^([A-Za-z_][\w-]*):\s?(.*)$")


def denies_memory(path):
    """Whether the definition at `path` denies its agent Memory.

    True for an explicit `memory: none`, and for a definition that exists but
    cannot be read — an unreadable declaration is not evidence of permission.
    False when the file is absent or carries no `memory: none`, the undeclared
    case the contract leaves Memory reachable for.
    """
    try:
        with open(path, encoding="utf-8") as fh:
            text = fh.read()
    except (FileNotFoundError, NotADirectoryError):
        return False
    except (OSError, UnicodeDecodeError):
        return True
    return _declares_none(text)


def _declares_none(text):
    lines = text.splitlines()
    if not lines or lines[0].strip() != "---":
        return False
    for line in lines[1:]:
        if line.strip() == "---":
            return False
        match = _KEY.match(line)
        if match and match.group(1) == "memory":
            return _value(match.group(2)) == "none"
    return False


def _value(rest):
    rest = rest.strip()
    if len(rest) >= 2 and rest[0] in "'\"":
        # The scalar is what sits between the quotes; anything after the closing
        # quote (`"none"  # one-shot`) is not part of it.
        close = rest.find(rest[0], 1)
        if close > 0:
            return rest[1:close].strip().lower()
    return rest.split("#", 1)[0].strip().lower()
