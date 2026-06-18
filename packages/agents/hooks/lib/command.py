"""Shell command parsing helpers shared across guard hooks."""

import os
import re
import shlex

# A leading `VAR=val` environment assignment on a command segment.
_ASSIGN = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*=")


def git_normalize(command):
    """Strip git global flags that can evade detection (-C/-c <x>, --git-dir=, --work-tree=)."""
    n = re.sub(r"git\s+-[Cc]\s+\S+", "git", command)
    n = re.sub(r"git\s+--git-dir=\S+", "git", n)
    n = re.sub(r"git\s+--work-tree=\S+", "git", n)
    return n


# Shell metacharacters shlex surfaces as standalone tokens. The default
# punctuation set is `();<>|&`; we append `\n` so a newline between commands is a
# token too — without it shlex eats newline as whitespace and collapses
# `a\ncodex-run` into one segment, hiding the second command from the guards.
_PUNCTUATION = "();<>|&\n"


def tokenize(command):
    """Quote-aware shell tokenizer.

    Control operators (`;` `|` `&` `<` `>` `(` `)` and runs like `&&` `||` `>>`)
    come back as their own tokens; a newline between commands does too, surfacing
    as a `\n` token (it stays whitespace inside quotes, so a quoted newline is not
    split out). A metacharacter inside a quoted argument stays part of that
    argument's token. Returns the token list, or None when the command can't be
    parsed (e.g. unbalanced quotes) — callers treat None as "leave it alone" so a
    malformed command is never a crash and never a block.
    """
    try:
        lex = shlex.shlex(command, posix=True, punctuation_chars=_PUNCTUATION)
        lex.whitespace_split = True
        lex.whitespace = lex.whitespace.replace("\n", "")
        return list(lex)
    except ValueError:
        return None


# Characters that, alone or in a run, make a token a command separator. Redirects
# (`<` `>`) are deliberately excluded — a redirect is part of its command, not a
# segment boundary — so a token is a separator only when every char is one of
# these. This matches `;`, `|`, `&`, `&&`, `||`, `|&`, `(`, `)`, a `\n`, and any
# coalesced run shlex emits (`\n\n`, `;\n`), while never matching `>`/`>>`/`>&`.
_SEPARATOR_CHARS = set(";|&()\n")


def _is_separator(token):
    return bool(token) and all(c in _SEPARATOR_CHARS for c in token)


def segments(command):
    """Tokenize and split into command segments at control operators.

    Returns a list of non-empty token-lists, or None when the command can't be parsed.
    """
    toks = tokenize(command)
    if toks is None:
        return None
    segs, cur = [], []
    for t in toks:
        if _is_separator(t):
            segs.append(cur)
            cur = []
        else:
            cur.append(t)
    segs.append(cur)
    return [s for s in segs if s]


def command_head(words):
    """The executable a segment runs, past leading `VAR=val` assignments and an
    `env` prefix.

    `words` is one tokenized segment (command + args). Returns the basename of the
    command token, so `FOO=1 codex-run …`, `env X=1 codex-run …`, and
    `/path/to/codex-run …` all reduce to `codex-run`. Empty string for an empty
    segment. This is the structural "what command is this segment running" check
    the command guards share, so a guard decides from the real command token
    rather than the raw command string.
    """
    i = 0
    while i < len(words) and _ASSIGN.match(words[i]):
        i += 1
    if i < len(words) and os.path.basename(words[i].strip("\"'")) == "env":
        i += 1
        while i < len(words) and _ASSIGN.match(words[i]):
            i += 1
    return os.path.basename(words[i].strip("\"'")) if i < len(words) else ""
