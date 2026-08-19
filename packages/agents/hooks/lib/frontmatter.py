"""Read the YAML frontmatter of a Prompt file, at build time and at hook time.

Every definition in this repository declares itself the same way: an agent file,
a SKILL.md, a rules file. Reading that block was written four times over — once
here as `parse` for the codex generator, once in `agent_memory` for the gates,
once in `classify_intent` for `disable-model-invocation`, once more for a Skill's
own `reload-every` — because this module used to live in `scripts/`, which is a
build-time path and is never stowed to ~/.agents where the hooks run from.

It lives beside the hooks now, and `scripts/sync.py` already puts this directory
on `sys.path`, so the generator reaches it too. One definition of what a key line
is, one definition of what a value is.

Stdlib only, like the hooks. We own every input (our own definition files), so
this handles the subset they use: top-level `key: value` scalars and `key: |` /
`>` block scalars. Keys nobody reads are skipped, not parsed.
"""

import re

_KEY = re.compile(r"^([A-Za-z_][\w-]*):\s?(.*)$")


def parse(text):
    """The frontmatter fields and the body below them, as (dict, str).

    ({}, text) when the file opens with no frontmatter block, so a caller that
    wants the body always gets one.
    """
    lines = text.splitlines()
    if not lines or lines[0].strip() != "---":
        return {}, text
    close = next((i for i in range(1, len(lines)) if lines[i].strip() == "---"), None)
    if close is None:
        return {}, text
    return _fields(lines[1:close]), "\n".join(lines[close + 1:]).strip("\n")


def declared(text, key):
    """The value `key` carries in the frontmatter block, or None when absent.

    The scanning half of `parse`, for a caller that wants one key off a file it
    holds as text: it stops at the closing marker and never parses what it was
    not asked for. The value comes back as written, because this does not know
    what its caller's values mean.

    A trailing `# comment` is stripped and a matching pair of quotes is removed,
    so `memory: "none"` and `memory: none  # one-shot` are the same declaration.
    Reading the second as anything but a denial is the failure `agent_memory`
    exists to stop, and the same widening is harmless everywhere else.
    """
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


def _fields(lines):
    out = {}
    i = 0
    while i < len(lines):
        match = _KEY.match(lines[i])
        if not match:
            i += 1
            continue
        key, rest = match.group(1), match.group(2).strip()
        if rest and rest[0] in "|>" and rest.strip("|>+-") == "":
            block, i = _block(lines, i + 1)
            out[key] = ("\n" if rest[0] == "|" else " ").join(block).strip()
        else:
            out[key] = _unquote(rest)
            i += 1
    return out


def _block(lines, i):
    block = []
    while i < len(lines) and not (lines[i].strip() and not lines[i][:1].isspace()):
        block.append(lines[i].strip())
        i += 1
    while block and not block[-1]:
        block.pop()
    return block, i


def _unquote(s):
    if len(s) >= 2 and s[0] == s[-1] and s[0] in "'\"":
        return s[1:-1]
    return s


def _value(rest):
    rest = rest.strip()
    if len(rest) >= 2 and rest[0] in "'\"":
        # The scalar is what sits between the quotes; anything after the closing
        # quote (`"none"  # one-shot`) is not part of it.
        close = rest.find(rest[0], 1)
        if close > 0:
            return rest[1:close].strip()
    return rest.split("#", 1)[0].strip()
