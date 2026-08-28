#!/usr/bin/env python3
"""Force code reading/searching through `trace`; stop the agent filtering trace output.

Four blocks, all reading the command through lib.command's quote-aware parser so
quoted text (a commit message, an echo argument) is never mistaken for command
structure, and through `script_lines`/`all_segments` so a command carried inside
`zsh -lc '…'`, an ssh line, or a `find -exec` runs the same gate as one typed
directly:
  A. a real `trace` command piped into a text-trimmer or redirected into a repo file.
  B. a raw cat/grep/find/sed/etc. command run against a path that exists in the repo,
     or a raw ls/tree listing that reaches the repo (no path argument lists cwd).
  C. a git subcommand that reads repository code and has an exact trace replacement.
  D. an unbounded search piped into sort/uniq, which holds the whole stream in memory.
A plain, unpiped, unredirected `trace` always passes.

A path behind a shell variable stays invisible here: the hook is handed the command
line, not the shell's environment, and refusing a token it cannot resolve would
refuse work on paths outside the repo.
"""

import glob as globlib
import os
import sys

from lib import feedback
from lib.command import (all_segments, command_head, git_subcommand,
                         head_and_args, script_lines, tokenize)
from lib.event import command_str, field, read_event

BINDING = {
    "events": {"PreToolUse": ["Bash"]},
    "timeout": 5,
    "harness": "all",
}

TRIMMERS = {"grep", "egrep", "fgrep", "rg", "sed", "awk", "head", "tail",
            "cut", "sort", "uniq", "wc", "column", "fold", "tr", "jq"}
RAW_TOOLS = {"cat", "grep", "egrep", "fgrep", "rg", "find", "sed", "awk", "head", "tail"}
# Listing tools default to cwd, so they are in-repo even with no path argument.
LIST_TOOLS = {"ls", "tree"}
_DEVICES = {"/dev/null", "/dev/stdout", "/dev/stderr", "/dev/tty"}
_OP_CHARS = set(";|&()<>\n")

# A search whose output is not bounded holds its whole stream in sort's memory.
# `rg -o` over the transcript corpus emitted 5.26 GB and sort peaked at 4.56x its
# input, so four concurrent censuses became four 10 GB processes. The bound is what
# makes the pipeline safe, not how large the tree is.
MEMORY_SINKS = {"sort", "uniq"}
SEARCHERS = {"rg", "grep", "egrep", "fgrep", "ag", "find", "fd"}
BOUNDING_FLAGS = ("-c", "--count", "--count-matches", "-m", "--max-count",
                  "-l", "--files-with-matches")

BLOCK_MSG = """BLOCKED: don't filter trace or hand-roll code reads.

trace returns scoped code intelligence — callers, complexity, nearest
Claude.md + rules, git activity. Piping it through grep/head/sed/awk/jq,
or using raw grep/find/sed/cat on repo files, throws that away.

Re-run the trace command with no pipe and no redirect; read all of it:
  grep -r / rg         -> trace grep <pattern> [-l <lang>] [--path <dir>]
  cat / head / sed -n  -> trace read <file> [<method>|--lines L1:L2]
  find                 -> trace find <pattern> [<base>]
  ls                   -> trace list <dir>
  tree                 -> trace tree <dir>
  tail / grep on a log -> trace logs <pattern> [--path <dir>] [--since <when>]
For partial output, use the in-binary filter — never a pipe:
  trace ... | jq '<expr>'  -> trace ... --json --filter '<expr>'"""

GIT_MSG = """BLOCKED: `git %s` reads repository code with no context.

trace answers the same question and carries callers, complexity, git
lifecycle, and the nearest Claude.md with it:
  %s

git keeps every question only it answers: status, branch, tag, rev-parse,
reflog, stash, merge-base, describe, ls-files, plain diff, log -p, and log -G."""

SORT_MSG = """BLOCKED: an unbounded search piped into sort holds the whole stream in memory.

`rg -o` over a large tree emits gigabytes, and sort costs about 4.5x its input
in resident memory. Bound the search instead:
  rg -c <pattern> <path>                     counts, materializing nothing
  rg --max-count <n> <pattern> <path>
  rg <pattern> <path> | head -200 | sort"""


def block(message):
    return feedback.block("guard_trace", message)


def resolve_paths(token, cwd):
    """Every path a token names, with globs expanded.

    A glob is the shape that made this guard fail open: the token resolved to
    nothing, and the read was then allowed on a tool the guard bans outright, so
    `cat src/*.ts` passed. Expanding names the files the command opens. A `$`, a
    backtick, or a brace still resolves to nothing, because what those stand for
    is not in the command line.
    """
    token = token.strip()
    for quote in ('"', "'"):
        if token.startswith(quote) and token.endswith(quote):
            token = token[1:-1]
    if not token or token.startswith("-"):
        return []
    if any(c in token for c in "$`({"):
        return []
    if token[:1] in "=><":
        return []
    if token.startswith("~"):
        token = os.path.expanduser("~") + token[1:]
    if not token.startswith("/"):
        token = os.path.join(cwd, token)
    if any(c in token for c in "*?["):
        return [os.path.normpath(p) for p in globlib.glob(token, recursive=True)]
    return [os.path.normpath(token)]


def inside_repo(p, cwd):
    if p in _DEVICES:
        return False
    # The artifact homes are prose, not code: a report, a plan, a shaping doc, or
    # a draft is read whole with cat, and `trace read` answers a code question it
    # is not being asked. docs/agents/ was the one home missing here, so an agent
    # reading a subagent's report.md or its own think.md was sent to trace.
    if ("/docs/shaping/" in p or "/docs/plans/" in p or "/docs/agents/" in p
            or "/.claude/shaping/" in p or "/.claude/plans/" in p
            or "/.tracer-cache/" in p):
        return False
    return p == cwd or p.startswith(cwd + "/")


def _reads_repo_file(token, cwd):
    return any(os.path.exists(p) and inside_repo(p, cwd)
               for p in resolve_paths(token, cwd))


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


def _pieces(line):
    """One line's segments as `(words, operator)`, keeping which segment feeds which.

    `all_segments` answers what a line runs and drops the pipes. The two rules that
    read a pipeline — a trace output being trimmed, a search being sorted — need
    that adjacency, so they read lines through here instead.
    """
    toks = tokenize(line)
    if toks is None:
        return None
    pieces, words = [], []
    for t in toks:
        op = _optype(t)
        if op in ("pipe", "sep"):
            pieces.append((words, op))
            words = []
        else:
            words.append(t)
    pieces.append((words, ""))
    return pieces


def _trace_piped_or_redirected(command, cwd):
    lines = script_lines(command)
    if lines is None:
        return False
    for line in lines:
        pieces = _pieces(line)
        if pieces is None:
            continue
        for i, (words, op) in enumerate(pieces):
            if command_head(words) != "trace":
                continue
            for j, w in enumerate(words):
                if _is_file_redirect(w) and j + 1 < len(words):
                    if any(inside_repo(p, cwd)
                           for p in resolve_paths(words[j + 1], cwd)):
                        return True
            if op == "pipe" and i + 1 < len(pieces):
                if command_head(pieces[i + 1][0]) in TRIMMERS:
                    return True
    return False


def _raw_read_in_repo(command, cwd):
    segs = all_segments(command)
    if segs is None:
        return False
    for words in segs:
        head = command_head(words)
        if head in LIST_TOOLS:
            args = [t for t in words[1:] if t and not t.lstrip("\"'").startswith("-")]
            # No path argument means the tool lists cwd — the repo itself. An
            # argument that resolves nowhere stays allowed unless some argument
            # lands inside the repo.
            if not args or any(_reads_repo_file(tok, cwd) for tok in args):
                return True
        if head not in RAW_TOOLS:
            continue
        for tok in words:
            if _reads_repo_file(tok, cwd):
                return True
    return False


def _git_read_refusal(words, cwd):
    """The trace command that replaces this git segment, or "" when git is the answer.

    Only the forms trace already answers exactly are named. A form trace cannot
    return — a patch, a regex pickaxe, a search at a ref, a stash — stays raw git,
    because banning it would block the work rather than route it.
    """
    subcommand = git_subcommand(words)
    if not subcommand:
        return ""
    args = words[words.index(subcommand) + 1:]
    separator = args.index("--") if "--" in args else len(args)
    flags = [a for a in args[:separator] if a.startswith("-")]
    positional = [a for a in args[:separator] if not a.startswith("-")]

    if subcommand in ("blame", "annotate"):
        return "trace blame <file> [<symbol>] [--lines L1:L2]"
    if subcommand == "grep":
        # `git grep <pattern> <rev>` searches a commit and trace grep searches the
        # worktree, so a second positional leaves git the only answer.
        return "" if len(positional) > 1 \
            else "trace grep <pattern> [-l <lang>] [--path <dir>]"
    if subcommand == "show":
        return "trace read <path> --at <ref>" if any(":" in a for a in positional) else ""
    if subcommand == "cat-file":
        return "trace read <path> --at <ref>" if "-p" in flags else ""
    if subcommand == "log":
        if any(f.startswith("-G") or f in ("-p", "--patch") for f in flags):
            return ""
        if any(f.startswith("-S") for f in flags):
            return "trace history --contains <pattern>"
        if any(f.startswith("-L") for f in flags):
            return "trace history <file> <symbol>"
        paths = positional + args[separator + 1:]
        if paths and all(_reads_repo_file(p, cwd) for p in paths):
            return "trace history <file>"
        return ""
    if subcommand == "diff":
        return "trace diff [--base <ref>] [--symbols]" if "--name-status" in flags else ""
    return ""


def _git_read_in_repo(command, cwd):
    segs = all_segments(command)
    if segs is None:
        return None
    for words in segs:
        replacement = _git_read_refusal(words, cwd)
        if replacement:
            return git_subcommand(words), replacement
    return None


def _unbounded_sort(command):
    lines = script_lines(command)
    if lines is None:
        return False
    for line in lines:
        pieces = _pieces(line)
        if pieces is None:
            continue
        for i, (words, op) in enumerate(pieces):
            if op != "pipe" or i + 1 >= len(pieces):
                continue
            if command_head(pieces[i + 1][0]) not in MEMORY_SINKS:
                continue
            head, args = head_and_args(words)
            if head not in SEARCHERS:
                continue
            if any(a == f or a.startswith(f + "=")
                   for a in args for f in BOUNDING_FLAGS):
                continue
            return True
    return False


def main():
    event = read_event()
    command = command_str(event)
    if not command:
        return 0
    cwd = field(event, "cwd", "") or os.getcwd()
    if _trace_piped_or_redirected(command, cwd) or _raw_read_in_repo(command, cwd):
        return block(BLOCK_MSG)
    git_read = _git_read_in_repo(command, cwd)
    if git_read:
        return block(GIT_MSG % git_read)
    if _unbounded_sort(command):
        return block(SORT_MSG)
    return 0


if __name__ == "__main__":
    sys.exit(main())
