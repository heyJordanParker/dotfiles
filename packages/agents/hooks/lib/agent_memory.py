"""The declarations an agent's definition file carries in its frontmatter.

`memory: none` is the one with two enforcement points; `codex-model` is read
here too because it comes off the same line shape in the same file, resolved
from the same roster, and defining that parse a second time is how the memory
syntax nearly drifted in the first place.

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
        text = _read(path)
    except (OSError, UnicodeDecodeError):
        return True
    if text is None:
        return False
    declared = _declared(text, "memory")
    # Lowering is done here, not in the parse: widening what counts as a denial
    # is safe, and this is the only key where it is.
    return declared is not None and declared.lower() == "none"


def declaration(path, key):
    """The frontmatter value the definition at `path` declares for `key`.

    None when the file is absent or carries no such key — both are the undeclared
    case, and the caller supplies its own default. The value comes back exactly
    as written, because this function does not know what its caller's values
    mean.

    A definition that exists but cannot be read raises rather than reading as
    undeclared: a declaration that was written and did not take effect is a
    broken agent, and silently running the default would hide it behind an
    output line that looks exactly like a correct run.

    Never read `memory` through here. That contract is that an unreadable
    definition denies, and this returns None for one — the permissive answer.
    `denies_memory` is the only correct reader of that key, so asking for it here
    is a mistake rather than a request.
    """
    if key == "memory":
        raise ValueError("read the memory declaration through denies_memory")
    text = _read(path)
    return None if text is None else _declared(text, key)


def _read(path):
    """The file's text, or None when it is absent. Other read failures raise."""
    try:
        with open(path, encoding="utf-8") as fh:
            return fh.read()
    except (FileNotFoundError, NotADirectoryError):
        return None


def _declared(text, key):
    """The value `key` carries in the frontmatter block, or None."""
    lines = text.splitlines()
    if not lines or lines[0].strip() != "---":
        return None
    for line in lines[1:]:
        if line.strip() == "---":
            return None
        match = _KEY.match(line)
        if match and match.group(1) == key:
            return _value(match.group(2))
    return None


def _value(rest):
    rest = rest.strip()
    if len(rest) >= 2 and rest[0] in "'\"":
        # The scalar is what sits between the quotes; anything after the closing
        # quote (`"none"  # one-shot`) is not part of it.
        close = rest.find(rest[0], 1)
        if close > 0:
            return rest[1:close].strip()
    return rest.split("#", 1)[0].strip()
