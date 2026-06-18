#!/usr/bin/env python3
"""Force code reading/searching through `trace`; stop the agent filtering trace output.

Two blocks:
  A. `trace` piped into a text-trimmer or redirected into a repo file.
  B. raw cat/grep/find/sed/etc against a path that exists inside the repo.
A plain, unpiped, unredirected `trace` always passes.
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

TRIMMERS = "grep|egrep|fgrep|rg|sed|awk|head|tail|cut|sort|uniq|wc|column|fold|tr|jq"
RAW_TOOLS = {"cat", "grep", "egrep", "fgrep", "rg", "find", "sed", "awk", "head", "tail"}
_DEVICES = {"/dev/null", "/dev/stdout", "/dev/stderr", "/dev/tty"}
_ENV_PREFIX = re.compile(r"^(?:[A-Za-z_][A-Za-z0-9_]*=\S*\s+)+")

BLOCK_MSG = """BLOCKED: don't filter trace or hand-roll code reads.

trace returns scoped code intelligence — callers, complexity, nearest
Claude.md + rules, git activity. Piping it through grep/head/sed/awk/jq,
or using raw grep/find/sed/cat on repo files, throws that away.

Re-run the trace command with no pipe and no redirect; read all of it:
  grep -r / rg         -> trace grep <pattern> [-l <lang>]
  cat / head / sed -n  -> trace read <file> [<method>]
  find                 -> trace find <pattern> [<base>]
For partial output, use the in-binary filter — never a pipe:
  trace ... | jq '<expr>'  -> trace ... --json --filter '<expr>'"""


def block():
    sys.stderr.write(BLOCK_MSG + "\n")
    return 2


def resolve_path(t, cwd):
    t = t.strip()
    for q in ('"', "'"):
        if t.startswith(q) and t.endswith(q):
            t = t[1:-1]
    if not t or t.startswith("-"):
        return ""
    if any(c in t for c in "$`*?[({"):
        return ""
    if t[:1] in "=><":
        return ""
    if t.startswith("~"):
        t = os.path.expanduser("~") + t[1:]
    if not t.startswith("/"):
        t = os.path.join(cwd, t)
    return os.path.normpath(t)


def inside_repo(p, cwd):
    if p in _DEVICES:
        return False
    if "/.claude/shaping/" in p or "/.claude/plans/" in p or "/.tracer-cache/" in p:
        return False
    return p == cwd or p.startswith(cwd + "/")


def main():
    event = read_event()
    command = command_str(event)
    if not command:
        return 0
    cwd = field(event, "cwd", "") or os.getcwd()

    # --- A. trace piped into a trimmer / redirected into the repo ---
    if re.search(r"\btrace\b", command):
        after = re.sub(r"^.*\btrace\b", "", command)  # greedy: after the last `trace`
        if re.search(r"\|\s*(sudo\s+)?(" + TRIMMERS + r")\b", after):
            return block()
        for m in re.finditer(r"[0-9]?&?>{1,2}\s*([^\s|;&<>(){}`]+)", after):
            rp = resolve_path(m.group(1), cwd)
            if rp and inside_repo(rp, cwd):
                return block()

    # --- B. raw replaced tools against an in-repo path ---
    normalized = command.replace("||", "|").replace("&&", "&")
    for seg in re.split(r"[|;&]", normalized):
        seg = seg.lstrip()
        seg = _ENV_PREFIX.sub("", seg)
        parts = seg.split()
        if not parts:
            continue
        if os.path.basename(parts[0]) not in RAW_TOOLS:
            continue
        for tok in parts[1:]:
            rp = resolve_path(tok, cwd)
            if rp and os.path.exists(rp) and inside_repo(rp, cwd):
                return block()
    return 0


if __name__ == "__main__":
    sys.exit(main())
