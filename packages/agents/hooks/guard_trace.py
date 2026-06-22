#!/usr/bin/env python3
"""Force code reading/searching through `trace`; stop the agent filtering trace output.

Two blocks, both reading the command through lib.command's quote-aware parser so
quoted text (a commit message, an echo argument) is never mistaken for command
structure:
  A. a real `trace` command piped into a text-trimmer or redirected into a repo file.
  B. a raw cat/grep/find/sed/etc. command run against a path that exists in the repo.
A plain, unpiped, unredirected `trace` always passes.
"""

import os
import sys

from lib.command import command_head, segments, tokenize
from lib.event import command_str, field, read_event

BINDING = {
    "events": {"PreToolUse": ["Bash"]},
    "timeout": 5,
    "harness": "all",
}

TRIMMERS = {"grep", "egrep", "fgrep", "rg", "sed", "awk", "head", "tail",
            "cut", "sort", "uniq", "wc", "column", "fold", "tr", "jq"}
RAW_TOOLS = {"cat", "grep", "egrep", "fgrep", "rg", "find", "sed", "awk", "head", "tail"}
_DEVICES = {"/dev/null", "/dev/stdout", "/dev/stderr", "/dev/tty"}
_OP_CHARS = set(";|&()<>\n")

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


def _optype(token):
    """Classify a token: 'pipe' (| or |&), 'sep' (; & && || ( ) newline), 'redir'
    (carries < or >, stays part of its command), or None for an ordinary word."""
    if not token or any(c not in _OP_CHARS for c in token):
        return None
    if "<" in token or ">" in token:
        return "redir"
    if token in ("|", "|&"):
        return "pipe"
    return "sep"


def _is_file_redirect(token):
    """A `>`/`>>`/`&>` to a filename (the target is the next word). Excludes the
    fd-dup forms `>&N` (e.g. `2>&1`), whose target is a descriptor, not a file."""
    return ">" in token and ">&" not in token


def _trace_piped_or_redirected(command, cwd):
    toks = tokenize(command)
    if toks is None:
        return False
    pieces, words = [], []
    for t in toks:
        op = _optype(t)
        if op in ("pipe", "sep"):
            pieces.append((words, op))
            words = []
        else:
            words.append(t)
    pieces.append((words, ""))

    for i, (words, op) in enumerate(pieces):
        if command_head(words) != "trace":
            continue
        for j, w in enumerate(words):
            if _is_file_redirect(w) and j + 1 < len(words):
                rp = resolve_path(words[j + 1], cwd)
                if rp and inside_repo(rp, cwd):
                    return True
        if op == "pipe" and i + 1 < len(pieces):
            if command_head(pieces[i + 1][0]) in TRIMMERS:
                return True
    return False


def _raw_read_in_repo(command, cwd):
    segs = segments(command)
    if segs is None:
        return False
    for words in segs:
        if command_head(words) not in RAW_TOOLS:
            continue
        for tok in words:
            rp = resolve_path(tok, cwd)
            if rp and os.path.exists(rp) and inside_repo(rp, cwd):
                return True
    return False


def main():
    event = read_event()
    command = command_str(event)
    if not command:
        return 0
    cwd = field(event, "cwd", "") or os.getcwd()
    if _trace_piped_or_redirected(command, cwd) or _raw_read_in_repo(command, cwd):
        return block()
    return 0


if __name__ == "__main__":
    sys.exit(main())
