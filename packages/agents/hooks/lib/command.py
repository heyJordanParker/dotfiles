"""Shell command parsing helpers shared across guard hooks."""

import os
import re
import shlex

# A leading `VAR=val` environment assignment on a command segment.
_ASSIGN = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*=")


# Git global options that consume a separate value token, verified against git
# itself (`--exec-path status` prints and consumes nothing). Every other global
# option is a single token — joined forms (`--git-dir=/x`, `-cuser.name=x`) and
# bare flags (`--no-pager`, `-p`, `--bare`) — and skips as a dash-word.
_GIT_VALUE_OPTS = frozenset(("-C", "-c", "--git-dir", "--work-tree", "--namespace"))


def _past_git_options(args):
    """Index of the first token after git's global options: the subcommand."""
    i = 0
    while i < len(args):
        if args[i] in _GIT_VALUE_OPTS:
            i += 2
        elif args[i].startswith("-"):
            i += 1
        else:
            break
    return i


def git_normalize(command):
    """Rewrite each git segment without its global options, so a guard's
    `git <subcommand>` pattern reads the real subcommand however the call was
    spelled — `git -C repo commit`, `git --no-pager reset`. Grammar, not a flag
    inventory: 6427d59 landed past the inventory this replaced. Redirect tokens
    stay bare and other words re-quote, so a caller reading redirects or
    invocations off the result reads them unchanged. An unparseable line
    returns as it came, the guards' standing treatment for one.
    """
    segs = segments(command)
    if segs is None:
        return command
    out = []
    for words in segs:
        head, args = head_and_args(words)
        if head == "git":
            words = ["git"] + args[_past_git_options(args):]
        out.append(" ".join(w if is_redirect(w) else shlex.quote(w) for w in words))
    return " ; ".join(out)


def git_subcommand(words):
    """The subcommand a git segment runs, past every global option.

    `words` is one tokenized segment. "" when the segment does not run git or
    names no subcommand. `git -C repo commit`, `git --no-pager commit`, and
    `sudo git -c a=b commit` all answer "commit".
    """
    head, args = head_and_args(words)
    if head != "git":
        return ""
    i = _past_git_options(args)
    return args[i] if i < len(args) else ""


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


def is_separator(token):
    return bool(token) and all(c in _SEPARATOR_CHARS for c in token)


def _split(command):
    """Command segments, each with whether its input comes from the one before it.

    A guard reading what a segment runs needs the pipe: `python3 --version` prints a
    version and `curl x | python3` runs whatever arrived, and the two segments are
    otherwise identical.
    """
    toks = tokenize(command)
    if toks is None:
        return None
    out, cur, piped = [], [], False
    for t in toks:
        if is_separator(t):
            if cur:
                out.append((cur, piped))
            cur, piped = [], "|" in t
        else:
            cur.append(t)
    if cur:
        out.append((cur, piped))
    return out


def segments(command):
    """Tokenize and split into command segments at control operators.

    Returns a list of non-empty token-lists, or None when the command can't be parsed.
    """
    split = _split(command)
    return None if split is None else [words for words, _ in split]


# Words whose own argument is another command. The real command follows, past
# that word's flags. Arities differ (`sudo -u jordan ssh` takes a value, `nohup`
# takes none), so a guard cannot know which token is the command — `invocations`
# answers with every candidate instead of picking one.
_PREFIX = frozenset((
    "env", "sudo", "doas", "command", "exec", "nohup",
    "time", "timeout", "nice", "ionice", "stdbuf", "xargs",
    # Shell keywords sit in the same position and hide the command exactly as a
    # prefix word does: `for f in *; do codex-run @x y; done` tokenizes to a
    # segment whose first word is `do`, and every guard read `do` as the command.
    "do", "then", "else", "elif", "if", "while", "until",
))

# Shells whose `-c` argument is a whole command line of its own.
_SHELL = frozenset(("bash", "sh", "zsh", "dash", "ksh"))

# Words that run a program named in their own argument, without being a shell:
# `source`/`.` run a file in the current shell and `eval` runs a string.
_CODE_RUNNERS = frozenset(("source", ".", "eval"))

# Words that give a command a new name. What that name runs is decided in the line
# and used after it, so following it would mean interpreting the shell.
_NAME_DEFINERS = frozenset(("alias", "function"))

# awk runs a program of its own, and two of its forms reach the shell from inside
# it. Ordinary text shaping — `awk '{print $1}' file` — reaches nothing.
_AWK = frozenset(("awk", "gawk", "mawk"))
_AWK_SHELL = ("system(", '| "', "|\"")

# The predicates whose argument is a command `find` runs per match.
_FIND_EXEC = ("-exec", "-execdir", "-ok", "-okdir")
_FIND_EXEC_PUNCTUATION = ("{}", ";", "\\;", "+")

# Commands whose trailing arguments are a command line run on another machine.
# Most real work legitimately rides ssh, so a guard judges what the line actually
# runs there rather than refusing the transport: `ssh host 'git status'` reads and
# `ssh host 'rm -rf x'` writes, and only the second is a write.
_REMOTE_SHELL = frozenset(("ssh", "mosh", "rsh"))

# A bare count or duration sitting between a prefix word and its command
# (`timeout 600 cmd`, `nice -n 10 cmd`).
_NUMERIC = re.compile(r"^\d+(?:\.\d+)?[smhd]?$")

# `bash -c` reaching `bash -c` reaching `bash -c`: real commands never nest this
# far, and the bound is what stops a crafted string from recursing forever.
_MAX_DEPTH = 4

# Interpreters that run code the command line does not contain: inline through
# `-c`/`-e`, from a script file named as an argument, or off stdin. `python3 -c "…"`
# and `sh /tmp/x.sh` are one bypass each for every guard that reads the line, and
# both were demonstrated against the spawn gate. The list is the one block_writes
# enforced privately. `bun`, `npm` and the other script runners stay out: `bun run
# build` names a script in a manifest, and `readonly_refusal` already resolves those.
RUNTIMES = frozenset((
    "python", "python3", "python2", "node", "deno", "perl", "ruby",
))

# The long flags that carry a program as their own argument. The short ones are
# read letter by letter below, because `python3 -c'code'` glues the program to the
# flag and an equality test never sees it.
_INLINE_LONG = frozenset(("--command", "--eval"))
_INLINE_LETTERS = "cem"

# The trees we wrote ourselves. A script here is our own tooling — `sync.py` is the
# repository's maintenance entry point — so running it is running us. Anywhere else
# the file is someone else's program and the line stops being readable.
OUR_TREE = ("/dotfiles/", "/.agents/", "/.claude/")


def is_ours(path):
    return any(fragment in path + "/" for fragment in OUR_TREE)


def _resolved(path, base):
    path = path.strip().strip("\"'")
    if path.startswith("~"):
        path = os.path.expanduser("~") + path[1:]
    if not path.startswith("/"):
        path = os.path.join(base, path)
    return os.path.realpath(path)


def _runner_target(words):
    """The command a runner word runs, as one `(head, args)` pair, or None.

    `uv run python -c '…'` and `npx tsx x.ts` put the real interpreter behind a
    runner. The runners are the same set `readonly_refusal` resolves through, so
    one list answers both questions.
    """
    i = _past_assignments(words, 0)
    while i < len(words) and _basename(words[i]) in _PREFIX:
        i = _past_prefix_args(words, i + 1)
    if i >= len(words) or _basename(words[i]) not in _RUNNERS:
        return None
    rest = [w for w in words[i + 1:] if not _ASSIGN.match(w)]
    if rest and rest[0] == "run":
        rest = rest[1:]
    while rest and rest[0].startswith("-"):
        rest = rest[1:]
    return (_basename(rest[0]), rest[1:]) if rest else None


def _inline_flag(args):
    """The flag carrying a program inside this argument list, or "".

    A single-dash argument is read letter by letter: `-c'print(1)'` arrives as one
    token with the program glued on, and `zsh -lc …` bundles the flag with others.
    A `--` argument is matched whole, so `--check` is not read as `-c`.
    """
    for arg in args:
        if arg in _INLINE_LONG:
            return arg
        if arg.startswith("-") and not arg.startswith("--") \
                and any(c in arg[1:] for c in _INLINE_LETTERS):
            return arg
    return ""


def hides_execution(words, piped=False):
    """What this segment runs that the line itself does not carry, or "".

    A shell's `-c` string is carried, and `_shell_script` reads it, so that shell is
    not hiding anything. What is hidden is inline code behind a flag, a script file,
    or a program piped in. A script file is the one readable case, and only when it
    is our own tooling — the trust boundary the write gate already used for the same
    question.

    A runtime that names no program at all is running its own flags: `python3
    --version` prints a version and hides nothing. It only hides a program when one
    is piped into it, which is what `piped` carries.
    """
    candidates = _expand(words, 0)
    runner = _runner_target(words)
    if runner:
        candidates = candidates + [runner]
    base = os.getcwd()
    for head, args in candidates:
        # A head the shell expands — `$SHELL -c …`, `` `codex-run …` `` — names a
        # command that is not in the line. A bare `$` is the artifact `$(…)` leaves
        # behind, and the command inside those parentheses is a segment of its own,
        # so it stays readable and is not refused here.
        if "`" in head or (head.startswith("$") and len(head) > 1):
            return "`%s` names a command this guard cannot resolve." % head
        if head in _NAME_DEFINERS:
            return "`%s` gives a command a name this guard cannot follow." % head
        if head in _AWK:
            program = next((a for a in args if not a.startswith("-")), "")
            if any(shape in program for shape in _AWK_SHELL):
                return "`%s` reaches the shell from inside its own program." % head
            continue
        if head not in _SHELL and head not in RUNTIMES and head not in _CODE_RUNNERS:
            continue
        if head in _SHELL and _shell_script(words) is not None:
            continue
        flag = _inline_flag(args)
        if flag:
            return "`%s %s` runs code that is not in this command line." % (head, flag)
        script = next((a for a in args if not a.startswith("-")), "")
        if not script:
            if piped:
                return "`%s` runs whatever is piped into it." % head
            continue
        path = _resolved(script, base)
        if not (os.path.isfile(path) and is_ours(path)):
            return "`%s %s` runs a program this guard cannot read." % (head, script)
    return ""


def _basename(word):
    return os.path.basename(word.strip("\"'"))


def _past_assignments(words, i):
    while i < len(words) and _ASSIGN.match(words[i]):
        i += 1
    return i


def _past_prefix_args(words, i):
    """Past one prefix word's own arguments: assignments, flags, and bare numbers."""
    while i < len(words) and (
        _ASSIGN.match(words[i]) or words[i].startswith("-") or _NUMERIC.match(words[i])
    ):
        i += 1
    return i


def head_and_args(words):
    """The executable a segment runs and the arguments it hands that executable.

    `words` is one tokenized segment (command + args). The head is the basename of
    the command token, so `FOO=1 codex-run …`, `env X=1 codex-run …`,
    `/path/to/codex-run …`, and `timeout 600 codex-run …` all reduce to
    `codex-run`. `("", [])` for an empty segment.

    A prefix word whose flag takes a value (`sudo -u jordan ssh`) resolves to the
    value, not the command — `invocations` is the reader for a guard that must not
    miss the command behind one.
    """
    i = _past_assignments(words, 0)
    while i < len(words) and _basename(words[i]) in _PREFIX:
        i = _past_prefix_args(words, i + 1)
    return (_basename(words[i]), words[i + 1:]) if i < len(words) else ("", [])


def command_head(words):
    """The executable a segment runs. This is the structural "what command is this
    segment running" check the command guards share, so a guard decides from the
    real command token rather than the raw command string."""
    return head_and_args(words)[0]


def redirects_output(command):
    """Whether the command line sends output into a file (`>`, `>>`, `2>`, `&>`).

    Every read-only command writes the moment its output is redirected, so this is
    the check that keeps `cat a > b` from passing as a read. It reads the shell
    scripts inside the line too, because `bash -c "cat a > b"` writes the same
    file. A redirect surfaces as a token of redirect characters alone; a quoted
    argument that merely contains one (`rg "a > b"`) stays inside its own word and
    is not one. None when the command cannot be parsed.
    """
    lines = _lines(command, 0)
    if lines is None:
        return None
    return any(">" in t and all(c in ">&" for c in t)
               for line, _ in lines for t in tokenize(line))


def script_lines(command):
    """This command line and every shell script nested inside it, as strings.

    `all_segments` flattens the nesting away, which loses which segment feeds
    which. A guard reading a pipeline — what sits on the left of a `|` — needs
    the lines back, so it can `_split` each one itself and keep that adjacency
    inside `zsh -lc '…'` exactly as it has it on the outer line.

    None when the line cannot be parsed, which every guard here treats as a
    block rather than a guess.
    """
    lines = _lines(command, 0)
    return None if lines is None else [line for line, _ in lines]


def all_segments(command):
    """Every command segment of this line and of every script nested inside it.

    `segments` reads the outer line alone, which is what a guard wants when it is
    judging shell syntax. A guard judging what the line DOES wants this instead: a
    shell's `-c` string and a command sent over ssh both run, so `bash -c 'rm x'` and
    `ssh prod 'rm -rf /srv'` surface their `rm` here and stay invisible to `segments`.

    None when the line cannot be parsed, which every guard here treats as a block
    rather than a guess.
    """
    lines = _lines(command, 0)
    if lines is None:
        return None
    found = []
    for line, _ in lines:
        found.extend(segments(line))
    return found


def invocations(command):
    """Every command the string runs, as `(head, arguments)` pairs.

    One pair per segment, of the outer line and of every shell script inside it,
    plus every candidate a prefix word's unknown flag arity leaves open. A guard
    reading these sees `ssh` in `sudo -u jordan ssh prod`, in `bash -c "ssh prod"`,
    and in `env X=1 /usr/bin/ssh prod` alike, which a match against the raw string
    does not. Extra candidates cost a guard nothing; a missed command costs it
    everything, so ambiguity resolves toward more.

    None when the string cannot be parsed — an unbalanced quote leaves the real
    command unknowable, and every guard here treats unknowable as a block rather
    than a guess.
    """
    lines = _lines(command, 0)
    if lines is None:
        return None
    found = []
    for line, _ in lines:
        for words in segments(line):
            found.extend(_expand(words, 0))
    return found


def _lines(command, depth, remote=False):
    """This command line and every shell script nested inside it, as
    `(line, remote)` pairs.

    A shell around a `-c` string is transport, not a command: codex hands every
    shell call over as `["/bin/zsh", "-lc", "<command>"]`, so a guard that judged
    the wrapper would refuse a read-only agent its whole shell on that harness
    while allowing the same command on Claude. The script is what runs, so it
    becomes a line of its own and the wrapper stops being a command.

    `remote` marks a line that runs on another machine — a line `_remote_scripts`
    lifted out of an ssh, and everything nested inside it. The write guards judge
    every line alike, but composition is judged per machine, and the tag is what
    lets `composition_refusal` tell the two apart.

    A segment that hands its work to something outside the string — inline runtime
    code, or a script file that is not our own tooling — leaves the real command
    unknowable, so it answers None exactly as an unbalanced quote does. Every guard
    reading through here already treats None as a refusal.
    """
    segs = _split(command)
    if segs is None:
        return None
    lines = [(command, remote)]
    if depth >= _MAX_DEPTH:
        return lines
    for words, piped in segs:
        if hides_execution(words, piped):
            return None
        script = _shell_script(words)
        nested_lines = [(s, remote) for s in ([script] if script is not None else [])] \
            + [(s, True) for s in _remote_scripts(words)] \
            + [(s, remote) for s in _exec_scripts(words)]
        for inner, inner_remote in nested_lines:
            nested = _lines(inner, depth + 1, inner_remote)
            if nested is None:
                return None
            lines.extend(nested)
    return lines


def _exec_scripts(words):
    """The command line `find` runs per match, or nothing.

    `-exec` puts a whole command after it, which is a line of its own exactly as a
    shell's `-c` string is. Without this, `find . -exec codex-run @x y \\;` reads as
    a `find`, and the spawn it carries is invisible to every guard.
    """
    if command_head(words) not in ("find", "fd"):
        return []
    for i, word in enumerate(words):
        if word in _FIND_EXEC:
            rest = [w for w in words[i + 1:] if w not in _FIND_EXEC_PUNCTUATION]
            return [" ".join(rest)] if rest else []
    return []


def _remote_scripts(words):
    """Every command line this segment might run on another machine.

    The host sits between the transport and its command, and ssh's flag arity is not
    knowable here — `ssh -p 22 prod 'rm x'` puts two words before the command,
    `ssh prod 'rm x'` puts one. Picking a boundary would let the shape it guessed
    wrong through, so every trailing position comes back as a candidate instead, the
    way `_prefix_candidates` resolves the same ambiguity. Extra candidates cost a
    guard nothing; a missed command costs it everything.

    `ssh host` alone opens a session with no command in the string to read, so it
    yields nothing and the transport stays the only thing a guard sees.
    """
    if command_head(words) not in _REMOTE_SHELL:
        return []
    i = _past_assignments(words, 0)
    while i < len(words) and _basename(words[i]) in _PREFIX:
        i = _past_prefix_args(words, i + 1)
    return [" ".join(words[j:]) for j in range(i + 2, len(words) + 1)
            if not words[j - 1].startswith("-")]


def _shell_script(words):
    """The command line a shell's `-c` carries in this segment, or None.

    Everything after the flag is the script, not the next word alone. Claude sends
    the script as one quoted word and the join is lossless; codex sends the shell
    call as a list that `lib/event.command_str` joins with spaces, which flattens
    that quoting away. Taking the remainder reads both, and the Claude shape it
    over-reads (`bash -c 'a b' trailing`) resolves toward more commands, never
    fewer.
    """
    head = command_head(words)
    if head not in _SHELL:
        return None
    for i, word in enumerate(words):
        if word.startswith("-") and not word.startswith("--") and "c" in word:
            rest = words[i + 1:]
            # Only the shell's own remaining flags are skipped. Dropping every
            # flag would drop the script's own (`zsh -lc sed -i '' s/a/b/ f`),
            # and the flag is what makes that command a write.
            while rest and rest[0].startswith("-"):
                rest = rest[1:]
            if not rest:
                return None
            # One token is Claude's shape — the whole script in one quoted word —
            # and its content is the line, lossless. More tokens is the codex
            # join, where posix tokenizing already consumed the quotes: a bare
            # re-join turned `ssh prod 'cd /srv && ls'` into a local `&&`, so
            # quoting is restored per token. Operator tokens stay bare so a
            # nested redirect still reads as one.
            if len(rest) == 1:
                return rest[0]
            return " ".join(w if w and all(c in _PUNCTUATION for c in w)
                            else shlex.quote(w) for w in rest)
    return None


def _expand(words, depth):
    i = _past_assignments(words, 0)
    if i >= len(words):
        return []
    head, rest = _basename(words[i]), words[i + 1:]
    if head in _SHELL and _shell_script(words) is not None:
        return []
    found = [(head, rest)]
    if head in _PREFIX and depth < _MAX_DEPTH:
        return found + _prefix_candidates(rest, depth)
    return found


# --- read-only classification ------------------------------------------------
#
# One classifier answers "does this command change anything", for the `readonly:`
# lever and for the orchestrator gate alike. It is an allowlist: a denylist of
# writing commands fails open on the first shape nobody listed, and every bypass is
# a one-liner. Anything not listed is refused, and that shows up as a block naming
# the command rather than as silent damage.

# Commands that inspect and leave the repository and the machine as they found them.
READERS = frozenset((
    # this roster's own readers
    "trace", "honcho", "agent-browser",
    # search and listing
    "rg", "grep", "egrep", "fgrep", "ag", "ack", "find", "fd", "ls", "tree",
    # file inspection
    "cat", "bat", "head", "tail", "wc", "nl", "file", "stat", "du", "df",
    "diff", "comm", "strings", "od", "xxd",
    # text shaping into the answer
    "sort", "uniq", "cut", "tr", "column", "jq", "yq", "sed", "awk",
    "basename", "dirname", "realpath", "readlink", "pwd", "echo", "printf",
    # the shell's own facts
    "date", "which", "type", "uname", "whoami", "hostname", "id", "env",
    "git", "curl",
    # process and status polling
    "ps", "lsof", "uptime", "sw_vers", "sysctl",
))

# Commands that check the tree without changing it: the suite, the linters, the
# type checkers. An orchestrator has to be able to validate what its subagents
# built, and a read-only agent has to be able to say whether the suite passes, so
# both reach these through the same list. Each one is a checker, never a runtime:
# `python3`, `node`, and `make` run whatever they are handed and stay out.
VALIDATORS = frozenset((
    "pytest", "phpunit", "pest", "playwright", "vitest", "jest", "mocha",
    "ruff", "mypy", "eslint", "tsc", "shellcheck", "stylelint", "phpstan",
))

# Runners whose job is to run one of the above. The argument decides, so the runner
# is resolved to the command it actually runs rather than granted outright.
_RUNNERS = frozenset(("uv", "uvx", "npx", "bunx", "poetry", "pipenv", "pdm", "rye"))

# Package managers that run a named script. Only the checking scripts are reachable.
_SCRIPT_RUNNERS = frozenset(("npm", "pnpm", "yarn", "bun"))
_CHECK_SCRIPTS = frozenset(("test", "tests", "lint", "typecheck", "check", "format:check"))

# The readers with a writing mode, and what turns it on. `wget` and `tee` have no
# read-only mode worth the flag table and are simply absent above.
_WRITE_FLAGS = {
    "sed": ("-i", "--in-place"),
    "curl": ("-o", "-O", "--output", "--remote-name", "--create-dirs"),
}

# git subcommands that only report. Everything else — `add`, `commit`, `config`,
# `branch`, `tag`, `remote`, `checkout` — changes the repository or its settings.
_GIT_READERS = frozenset((
    "status", "log", "diff", "show", "blame", "annotate", "shortlog", "reflog",
    "rev-parse", "rev-list", "ls-files", "ls-tree", "cat-file", "describe",
    "merge-base", "name-rev", "whatchanged", "grep", "count-objects", "version",
))

# Commands that reach another machine. The `ssh:` lever answers for these, never
# the read-only one.
REMOTE = frozenset(("ssh", "scp", "sftp", "rsync", "mosh", "telnet", "ssh-copy-id"))


def writing_flag(head, args):
    """The flag that turns this reader into a writer, or "" when none is present."""
    for flag in _WRITE_FLAGS.get(head, ()):
        if any(arg == flag or arg.startswith(flag + "=") for arg in args):
            return flag
    return ""


def readonly_refusal(head, args):
    """Why this command changes something, or "" when it only reads or checks.

    The allowlist half of the question — whether the command is one a reader runs
    at all. `mutation_targets` answers the other half, which paths it changes.
    """
    if head in REMOTE:
        return ""          # the ssh declaration answers for these, not this one
    if head in VALIDATORS:
        return ""
    if head in _RUNNERS:
        inner = next((a for a in args if not a.startswith("-")), "")
        if inner == "run":
            rest = [a for a in args[args.index("run") + 1:] if not a.startswith("-")]
            inner = rest[0] if rest else ""
        if _basename(inner) in VALIDATORS:
            return ""
        return "`%s %s` runs whatever it is handed." % (head, inner) if inner \
            else "`%s` needs a checking command here." % head
    if head in _SCRIPT_RUNNERS:
        positional = [a for a in args if not a.startswith("-")]
        script = positional[1] if positional[:1] == ["run"] and len(positional) > 1 \
            else (positional[0] if positional else "")
        if script in _CHECK_SCRIPTS:
            return ""
        return "`%s %s` is not a checking script." % (head, script) if script \
            else "`%s` needs a checking script here." % head
    if head not in READERS:
        return "`%s` is not one of the commands a read-only agent runs." % head
    flag = writing_flag(head, args)
    if flag:
        return "`%s %s` writes." % (head, flag)
    if head == "git":
        subcommand = next((a for a in args if not a.startswith("-")), "")
        if subcommand not in _GIT_READERS:
            return "`git %s` changes the repository." % subcommand if subcommand \
                else "`git` needs a reporting subcommand here."
    return ""


# --- mutation targets --------------------------------------------------------
#
# The other half of "does this command change anything": `readonly_refusal` answers
# whether the command is one a reader runs at all, and this answers which paths a
# command changes, which is what a guard needs to judge WHERE the change lands.

# Commands whose every path argument is a file they create, delete, move, or
# rewrite — so each one is a mutation target. `mv` is here, not in _DEST_MUTATORS,
# because it also removes its source path.
_ALL_ARG_MUTATORS = frozenset((
    "rm", "rmdir", "unlink", "shred", "mkdir", "mkfifo", "mknod",
    "touch", "truncate", "mv",
))
# Commands that read their sources and mutate only the last (destination) path.
_DEST_MUTATORS = frozenset(("cp", "install", "ln"))
# Commands whose first path argument is a mode/owner spec, not a file; the rest
# are the files they mutate.
_OWNER_MUTATORS = frozenset(("chmod", "chown", "chgrp"))


def is_redirect(token):
    """Whether the token is a shell redirect operator rather than a word."""
    return ">" in token and token != "" and token.strip("0123456789&<>") == ""


def _is_fd_reference(token):
    """A redirect target that names a file descriptor, not a file: `2>&1`, `>&2`, `>&-`."""
    return token == "-" or token.isdigit()


def _positional_paths(words):
    """Non-flag path arguments of a segment, past the command head, with shell
    redirect operators and their targets excluded — redirects are gathered
    separately. A flag's separate value (`-s 0`) is not distinguishable from a
    path here, so a stray value can read as a target; over-detection only ever
    blocks a mutation, never lets one through."""
    out, skip = [], False
    for t in words[1:]:
        if skip:
            skip = False
            continue
        if is_redirect(t):
            skip = True
            continue
        if t.startswith("-"):
            continue
        out.append(t)
    return out


def mutation_targets(words):
    """Paths a single command segment would create, write, move, or delete."""
    i = _past_assignments(words, 0)
    words = words[i:]
    if not words:
        return []
    head = os.path.basename(words[0])
    targets = []
    for j, t in enumerate(words):
        if is_redirect(t) and j + 1 < len(words) and not _is_fd_reference(words[j + 1]):
            targets.append(words[j + 1])
    if "tee" in words:
        for t in words[words.index("tee") + 1:]:
            if not t.startswith("-"):
                targets.append(t)
    for t in words:
        if t.startswith("of="):
            targets.append(t[3:])
    positional = _positional_paths(words)
    if head == "sed" and "-i" in words:
        if positional:
            targets.append(positional[-1])
    elif head in _ALL_ARG_MUTATORS:
        targets.extend(positional)
    elif head in _DEST_MUTATORS:
        if positional:
            targets.append(positional[-1])
    elif head in _OWNER_MUTATORS:
        targets.extend(positional[1:])
    return targets


# --- atomic execution --------------------------------------------------------
#
# One Bash call, one step. Composition is how several steps ride in as one, and it
# costs on every axis a guard cares about: the reader has to resolve deeper, a
# failure anywhere re-sends the whole payload, and the model writing it has to hold
# the whole sequence at once. Each shape refused here has a plainer replacement —
# a second call, or the tool that already owns the job — so nothing becomes
# unreachable.

# The keywords that wrap a command in a repetition. Their bodies are separated
# anyway, so this is what names the shape in the refusal rather than what finds it.
_LOOP_WORDS = frozenset(("for", "while", "until", "select", "do", "done"))

# A heredoc and the terminator that closes it. Matching the terminator is what
# separates the operator from prose about it: `git commit -m "use << here"` has no
# closing line, so it is text. A heredoc inside `"$(…)"` is still one, which is why
# this reads the raw line rather than the token list.
_HEREDOC = re.compile(r"<<-?\s*(['\"]?)(\w+)\1[^\n]*\n(?:.*\n)?\s*\2\s*$", re.S | re.M)

# The one prefix word whose whole purpose is applying a command to a batch. Every
# other prefix resolves to the command behind it; this one is the shape itself.
_BATCHER = "xargs"


def _composition_operator(token):
    """The operator that ends one command and starts another, or "".

    A lone `|` is not one: its consumer reads what the producer wrote, so the pair
    is a single step. A token of parentheses alone is what `$(…)` leaves behind,
    and the command inside is a segment the reader already resolves on its own.
    """
    if not is_separator(token) or token == "|" or all(c in "()" for c in token):
        return ""
    return token.replace("\n", "a newline")


def composition_refusal(command):
    """Why this command line is more than one step, or "" when it is one.

    Reads the line and every shell script nested inside it, so a chain inside
    `zsh -lc` answers exactly as a chain typed directly — which is the shape codex
    sends every command in.

    Composition is judged per machine: a line that runs over ssh is exempt,
    because a remote session's state dies with the call, so a chain there cannot
    be split into calls the way a local one can — the exported variable the next
    step needs is gone. The write guards still read remote lines through
    `all_segments` and `invocations`, and a remote string this reader cannot
    parse still refuses as None here.
    """
    lines = _lines(command, 0)
    if lines is None:
        return ("this line hides what it runs, or cannot be parsed. Inline code, a "
                "script that is not our own tooling, and an unbalanced quote each "
                "leave the real command unknowable")
    for line, remote in lines:
        if remote:
            continue
        if _HEREDOC.search(line):
            return ("a heredoc carries a whole file inside a command. Write the file "
                    "with the Write tool, and pass its path")
        tokens = tokenize(line)
        if tokens is None:
            return "this line cannot be parsed, so what it runs is unknowable"
        for words, piped in _split(line):
            if _basename(words[0]) in _LOOP_WORDS:
                return ("a loop repeats a command inside one call. Run the command "
                        "once per item, one call each")
            if _exec_scripts(words):
                return ("`find -exec` runs a command per match inside one call. Find "
                        "the matches, then act on them one at a time")
            if not piped:
                continue
            if _basename(words[0]) == _BATCHER:
                return ("`xargs` applies a command to a batch. Act on one target per "
                        "call instead")
            reason = readonly_refusal(*head_and_args(words))
            if reason:
                return ("a pipeline that ends in a change is a program, not a read: "
                        "%s" % reason)
        for token in tokens:
            operator = _composition_operator(token)
            if operator:
                return "`%s` joins two commands. Send them as two calls" % operator
    return ""


def _prefix_candidates(words, depth):
    """Every command a prefix word might be running.

    With no flag in front of it the command is unambiguous, so only it comes back.
    A flag first (`sudo -u jordan ssh prod`) may or may not have eaten the next
    word, so every later word comes back as a candidate as well.
    """
    ambiguous, start = False, None
    for i, word in enumerate(words):
        if _ASSIGN.match(word) or _NUMERIC.match(word) or is_separator(word):
            continue
        if word.startswith("-"):
            ambiguous = True
            continue
        start = i
        break
    if start is None:
        return []
    found = _expand(words[start:], depth + 1) or []
    if not ambiguous:
        return found
    return found + [
        (_basename(word), words[i + 1:])
        for i, word in enumerate(words[start + 1:], start + 1)
        if not (_ASSIGN.match(word) or _NUMERIC.match(word)
                or is_separator(word) or word.startswith("-"))
    ]
