"""Parse the YAML frontmatter and body of a Markdown definition file.

Stdlib only, like the hooks. We own every input (our own agent files), so this
handles the subset they use: top-level `key: value` scalars and `key: |` / `>`
block scalars. Returns (fields, body); keys we don't read are skipped, not parsed.
"""

import re

_KEY = re.compile(r"^([A-Za-z_][\w-]*):\s?(.*)$")


def parse(text):
    lines = text.splitlines()
    if not lines or lines[0].strip() != "---":
        return {}, text
    close = next((i for i in range(1, len(lines)) if lines[i].strip() == "---"), None)
    if close is None:
        return {}, text
    return _fields(lines[1:close]), "\n".join(lines[close + 1:]).strip("\n")


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
