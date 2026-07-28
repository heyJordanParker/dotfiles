#!/usr/bin/env python3
"""Close the agent-browser sessions a stopping agent opened and never closed.

Log analysis showed most leaked sessions came from normal, successful
completion: the agent finishes without running `agent-browser close` and the
daemon idles until its timeout, or forever in the exempt modes. So the harness
cleans up instead of the agent. On a real end this reads the stopping agent's
own transcript, replays every agent-browser invocation in order, keeps the
sessions whose final state is open, and closes the ones agent-browser still
reports live.

Wrong-close outranks leak. A leaked session falls to agent-browser's own idle
timeout; a session closed out from under a sibling agent destroys live work.
Two principles follow, and every rule below is one of them applied:

1. Never close what the transcript did not provably open. A session is claimed
   only from an agent-browser call in executed command position — the resolved
   executable after environment assignments, `env`, prefixes (`sudo`, `timeout
   60`, `nice`), package runners (`npx`, `bunx`), and shell wrappers (`bash -lc
   '…'`), unwrapped recursively. Text that merely contains the command — an
   `echo`, a `grep`, a heredoc body written into a file — is data, never a
   claim; a heredoc body a shell reads on stdin is the script that ran, so it
   is parsed. Anything that cannot be read this way is skipped, which leaks
   rather than closes.

2. Never auto-close the shared `default` session. Only a named session is
   attributable to one agent. `default` is where siblings collide, so its leaks
   are the idle timeout's job, even when this transcript opened it.

Ownership is by browser reach, not by `open`. Verified against agent-browser
0.33.1: `snapshot`, `eval`, `click`, `get`, and `screenshot` on a fresh session
report `browserLaunched` true, so they start the browser just as `open` does,
while `open` against an already-live session is reuse. So every subcommand
except the ones that never touch a browser (`skills`, `session`, `profiles`,
`install`, `help`, `version`, `read`) claims the named session it targets.

Codex records its shell calls as JavaScript. This hook does not evaluate
JavaScript: only a `tools.exec_command({cmd:"<double-quoted literal>"})` call in
unguarded statement position counts as executed. A call behind `if`, `for`,
`while`, `&&`, or `?`, a call sitting after any `function` or `=>` in the input,
a single-quoted or template-literal or variable `cmd:`, and a bare object
literal with no call are all ambiguous and skipped.

No model call, no session-state read or write, no blocking: every path returns 0,
and a close that fails is left alone rather than surfaced into the turn.
"""

import concurrent.futures
import json
import os
import re
import subprocess
import sys
import time

from lib.command import segments
from lib.event import field, read_event
from lib.transcript import blocks, records

BINDING = {
    "events": {"Stop": [], "SubagentStop": []},
    "timeout": 60,
    "harness": "all",
    "roots": "all",
}

DEFAULT_SESSION = "default"

# The shell tool under each harness's own name. `exec` is codex's current one: a
# custom tool whose input is JavaScript calling tools.exec_command.
_SHELL_TOOLS = ("Bash", "shell", "shell_command", "exec_command", "exec")

# Codex records a tool call as a rollout `response_item` payload, not as a
# content block on a message.
_CODEX_CALLS = ("function_call", "custom_tool_call", "local_shell_call")

# A `zsh -lc <script>` wrapper carries the real command in its last word, both as
# a codex command list and as a Bash argument.
_SHELL_WRAPPERS = ("sh", "bash", "zsh", "dash", "ksh")
_SHELL_COMMAND_FLAG = re.compile(r"^-[a-z]*c$")

# Commands that run another command: they are stepped over to reach the real
# executable, so `timeout 60 bash -lc 'agent-browser …'` is read.
_PREFIXES = ("sudo", "doas", "nice", "nohup", "stdbuf", "command", "time",
             "timeout", "setsid", "ionice", "env")
_PREFIX_VALUE_FLAGS = ("-u", "-g", "-U", "-C", "-n", "-k", "-s", "-o", "-i", "-e")
# `timeout` takes a duration positional before the command it runs.
_PREFIX_POSITIONALS = {"timeout": 1}

_RUNNER_ONE = ("npx", "bunx", "pnpx", "uvx")
_RUNNER_TWO = (("npm", "exec"), ("pnpm", "dlx"), ("bun", "x"), ("yarn", "dlx"))

# agent-browser's global flags that consume the token after them. Only the global
# ones can shadow the subcommand, because a subcommand's own flags follow it.
# Parsing against the real set is what keeps `--timeout 5 close` — `--timeout` is
# not an agent-browser flag at all — from reading as an open of `5`.
_VALUE_FLAGS = (
    "--session", "--namespace", "--executable-path", "--extension", "--init-script",
    "--enable", "--args", "--user-agent", "--proxy", "--proxy-bypass", "--provider",
    "-p", "--device", "--screenshot-dir", "--screenshot-quality", "--screenshot-format",
    "--cdp", "--color-scheme", "--download-path", "--max-output", "--allowed-domains",
    "--action-policy", "--confirm-actions", "--engine", "--idle-timeout", "--model",
    "--config", "--profile", "--restore-save", "--restore-check-url",
    "--restore-check-text", "--restore-check-fn", "--session-name", "--state", "--headers",
)

# Flags whose value is optional: they consume the next token only when it is a
# boolean literal.
_OPTIONAL_BOOL_FLAGS = ("--headed", "--hide-scrollbars")

_CLOSE = "close"

# The subcommands that never reach a browser, so they claim nothing. Everything
# else launches or drives one on the session it targets.
_BROWSERLESS = ("skills", "session", "profiles", "install", "help", "version", "read",
                "dashboard")

# A session or namespace naming an unexpanded variable or a substitution is not a
# real name — probing it would talk to a session called `$VAR`.
_UNRESOLVED = ("$", "`")

_ENV_SESSION = "AGENT_BROWSER_SESSION"
_ENV_NAMESPACE = "AGENT_BROWSER_NAMESPACE"

_ASSIGNMENT = re.compile(r"^([A-Za-z_][A-Za-z0-9_]*)=(.*)$", re.S)

# A heredoc and its body. Whether the body is a script or data depends on what
# consumes it: `bash -s <<'SH'` runs it, `cat > file <<'EOF'` writes it.
_HEREDOC = re.compile(
    r"<<-?\s*(['\"]?)(\w+)\1[^\n]*\n(?P<body>.*?)^\s*\2\s*$", re.S | re.M)

# A `tools.exec_command({ … cmd:"…" … })` call, capturing the text before it back
# to the nearest statement or list boundary so a guarded call can be rejected.
_EXEC_CALL = re.compile(
    r"([^;{}\n]*)tools\.exec_command\(\s*\{(?P<body>[^{}]*)\}", re.S)
_CMD_LITERAL = re.compile(r'(?:^|[\s,{])"?cmd"?\s*:\s*(?P<value>"(?:[^"\\]|\\.)*")')
# A call reached through control flow may never have run; JavaScript is not
# evaluated here, so it is ambiguous and skipped.
_GUARDED = re.compile(r"(?:\b(?:if|else|for|while|switch|case|catch|do)\b|&&|\|\||\?)")
# A call inside a function or arrow body runs only if something calls it, which
# is exactly the question JavaScript evaluation would answer. Once a definition
# appears anywhere ahead of a call, every call after it is ambiguous.
_DEFERRED = re.compile(r"\bfunction\b|=>")

_SUBPROCESS_TIMEOUT = 5
_WORKERS = 8
# Leaves the BINDING timeout headroom to reap the pool after the last close.
_BUDGET = 45


def _as_command(value):
    """A command list flattened to its string; a shell wrapper to its script."""
    if isinstance(value, list):
        words = [str(x) for x in value]
        if len(words) >= 3 and os.path.basename(words[0]) in _SHELL_WRAPPERS and words[1].startswith("-"):
            return words[-1]
        return " ".join(words)
    return value if isinstance(value, str) else ""


def _js_commands(text):
    """Every command an `exec` JavaScript input provably ran.

    One `exec` call can start several tools.exec_command calls, so this yields
    all of them rather than the first.
    """
    out = []
    for match in _EXEC_CALL.finditer(text):
        if _GUARDED.search(match.group(1)) or _DEFERRED.search(text[:match.end(1)]):
            continue
        literal = _CMD_LITERAL.search(match.group("body"))
        if not literal:
            continue
        try:
            value = json.loads(literal.group("value"))
        except Exception:
            continue
        if value:
            out.append(value)
    return out


def _codex_commands(payload):
    """The shell commands one codex rollout tool-call payload ran."""
    kind = payload.get("type")
    if kind == "local_shell_call":
        action = payload.get("action")
        command = _as_command(action.get("command")) if isinstance(action, dict) else ""
        return [command] if command else []
    if kind not in _CODEX_CALLS or payload.get("name") not in _SHELL_TOOLS:
        return []
    raw = payload.get("arguments") or payload.get("input") or ""
    if isinstance(raw, str):
        try:
            raw = json.loads(raw)
        except Exception:
            return _js_commands(raw)
    if not isinstance(raw, dict):
        return []
    command = _as_command(raw.get("cmd") or raw.get("command"))
    return [command] if command else []


def shell_commands(recs):
    """Every shell command string the transcript's tool calls ran, in order.

    Only tool calls — never raw transcript text. agent-browser's own help and
    skill output quote example commands, and those land in the transcript as
    tool *results*; reading them as calls would close sessions nobody opened.
    """
    out = []
    for record in recs:
        for block in blocks(record, "tool_use"):
            if block.get("name") not in _SHELL_TOOLS:
                continue
            inp = block.get("input") or {}
            command = _as_command(inp.get("command") or inp.get("cmd"))
            if command:
                out.append(command)
        payload = record.get("payload")
        if isinstance(payload, dict):
            out.extend(_codex_commands(payload))
    return out


def _base(token):
    return os.path.basename(token.strip("\"'"))


def _executed(words):
    """(assignments, the words of the command this segment actually executes).

    Steps over leading `VAR=val`, `env`, run-another-command prefixes, and
    package runners — in any order and repeatedly — so the executable is decided
    by position rather than by finding its name somewhere in the tokens.
    """
    assignments, index = [], 0
    while index < len(words):
        match = _ASSIGNMENT.match(words[index])
        if match:
            assignments.append((match.group(1), match.group(2)))
            index += 1
            continue
        head = _base(words[index])
        if head == "export":
            index += 1
            continue
        if head in _PREFIXES:
            index += 1
            while index < len(words) and words[index].startswith("-"):
                index += 2 if words[index] in _PREFIX_VALUE_FLAGS else 1
            for _ in range(_PREFIX_POSITIONALS.get(head, 0)):
                if index < len(words) and not _ASSIGNMENT.match(words[index]):
                    index += 1
            continue
        if head in _RUNNER_ONE:
            index += 1
            continue
        if index + 1 < len(words) and (head, words[index + 1]) in _RUNNER_TWO:
            index += 2
            continue
        break
    return assignments, words[index:]


def _environment(assignments):
    """(session, namespace) named by a segment's environment assignments."""
    session = namespace = ""
    for name, value in assignments:
        if name == _ENV_SESSION:
            session = value
        elif name == _ENV_NAMESPACE:
            namespace = value
    return session, namespace


def _read_invocation(words, session, namespace):
    """(session, namespace, subcommand) for one tokenized agent-browser call."""
    subcommand = ""
    skip = False
    for index, word in enumerate(words[1:], start=1):
        if skip:
            skip = False
            continue
        following = words[index + 1] if index + 1 < len(words) else ""
        if word.startswith("--session="):
            session = word.split("=", 1)[1]
        elif word.startswith("--namespace="):
            namespace = word.split("=", 1)[1]
        elif word in _VALUE_FLAGS:
            if word == "--session":
                session = following
            elif word == "--namespace":
                namespace = following
            skip = True
        elif word == "--restore":
            skip = bool(following) and not following.startswith("-") and following != _CLOSE
        elif word in _OPTIONAL_BOOL_FLAGS:
            skip = following.lower() in ("true", "false")
        elif not word.startswith("-") and not subcommand:
            subcommand = word
    return session, namespace, subcommand


def _heredoc_scripts(command):
    """Heredoc bodies that a shell ran, rather than ones written to a file.

    `bash -s <<'SH'` and `bash <<'SH'` feed the body to a shell on stdin, so the
    body is the script that executed. `cat > file <<'EOF'` writes it, and a `-c`
    shell already has its script on the command line, so both bodies stay data.
    """
    out = []
    for match in _HEREDOC.finditer(command):
        consumer = (segments(command[:match.start()].rsplit("\n", 1)[-1]) or [[]])[-1]
        _, executed = _executed(consumer)
        if not executed or _base(executed[0]) not in _SHELL_WRAPPERS:
            continue
        if any(_SHELL_COMMAND_FLAG.match(word) for word in executed[1:]):
            continue
        out.append(match.group("body"))
    return out


def _events(command, depth=0):
    """What one shell command did, in order, as replay events.

    `("export", session, namespace)` for an environment export that outlives the
    command, and `("call", session, namespace, subcommand, all_flag)` for each
    agent-browser call in executed position.
    """
    out = []
    if depth < 3:
        for script in _heredoc_scripts(command):
            out.extend(_events(script, depth + 1))
    for words in segments(_HEREDOC.sub(" ", command)) or []:
        assignments, executed = _executed(words)
        session, namespace = _environment(assignments)
        if not executed:
            if session or namespace:
                out.append(("export", session, namespace))
            continue
        head = _base(executed[0])
        if head in _SHELL_WRAPPERS and depth < 3:
            for index, word in enumerate(executed[1:], start=1):
                if _SHELL_COMMAND_FLAG.match(word):
                    out.extend(_events(" ".join(executed[index + 1:]), depth + 1))
                    break
            continue
        if head != "agent-browser":
            continue
        session, namespace, subcommand = _read_invocation(executed, session, namespace)
        out.append(("call", session, namespace, subcommand, "--all" in executed[1:]))
    return out


def abandoned_sessions(recs):
    """Named session keys left open at the end of the transcript, in first-seen order.

    Replayed in order, so the last invocation per session decides: `open a;
    close --all; open b` leaves `b`, and `close p; open p` leaves `p`. `default`
    is never claimed, so it is never returned.
    """
    state, order = {}, []
    exported_session = exported_namespace = ""
    for command in shell_commands(recs):
        for event in _events(command):
            if event[0] == "export":
                exported_session = event[1] or exported_session
                exported_namespace = event[2] or exported_namespace
                continue
            _, session, namespace, subcommand, all_sessions = event
            session = session or exported_session or DEFAULT_SESSION
            namespace = namespace or exported_namespace
            if any(c in session + namespace for c in _UNRESOLVED):
                continue
            key = (namespace, session)
            if subcommand == _CLOSE:
                if all_sessions:
                    for known in state:
                        if known[0] == namespace:
                            state[known] = False
                    continue
            elif not subcommand or subcommand in _BROWSERLESS or session == DEFAULT_SESSION:
                continue
            if key not in state:
                order.append(key)
            state[key] = subcommand != _CLOSE
    return [key for key in order if state.get(key)]


def _run(argv):
    try:
        result = subprocess.run(argv, capture_output=True, text=True,
                                timeout=_SUBPROCESS_TIMEOUT)
    except Exception:
        return ""
    return result.stdout or ""


def _argv(key, tail):
    namespace, name = key
    argv = ["agent-browser"]
    if namespace:
        argv += ["--namespace", namespace]
    if name != DEFAULT_SESSION:
        argv += ["--session", name]
    return argv + tail


def is_live(key):
    """Whether agent-browser's own records still show a browser open on `key`.

    `session list` is not the check: it reports every socket name it knows,
    including ones whose daemon has exited. `session info` is per session and
    carries both flags — `active` for the daemon, `runtime.browserLaunched` for a
    browser actually open on it, which is the leak. Probing an unknown session
    reports inactive and launches nothing.
    """
    try:
        data = json.loads(_run(_argv(key, ["session", "info", "--json"])))
    except Exception:
        return False
    if not isinstance(data, dict):
        return False
    info = data.get("data")
    if not isinstance(info, dict) or not info.get("active"):
        return False
    runtime = info.get("runtime")
    return isinstance(runtime, dict) and bool(runtime.get("browserLaunched"))


def close_session(key):
    _run(_argv(key, ["close"]))


def _close_if_live(key, deadline):
    if time.monotonic() < deadline and is_live(key):
        close_session(key)


def _is_codex_rollout(path):
    return os.path.basename(path).startswith("rollout-") or "/.codex/" in path


def stopping_transcript(event):
    """The stopping agent's own transcript, or "" when this stop is not an end.

    Claude's main-session Stop fires after every assistant turn, so cleaning up
    there closes the browser between the architect's instructions. The two real
    ends are a SubagentStop — a one-shot subagent finishing, whose payload
    carries its own transcript — and a codex Stop, which ends the whole run and
    is told apart by its `turn_id` and its `.codex` rollout transcript.

    A SubagentStop never falls back to `transcript_path`: that is the parent's
    transcript, and cleaning up from it closes the parent's live sessions.
    """
    path = field(event, "transcript_path", "")
    if field(event, "hook_event_name", "") == "SubagentStop":
        return field(event, "agent_transcript_path", "")
    if field(event, "turn_id", "") or _is_codex_rollout(path):
        return path
    return ""


def main():
    try:
        path = stopping_transcript(read_event())
        if not path:
            return 0
        abandoned = abandoned_sessions(records(path))
        if not abandoned:
            return 0
        deadline = time.monotonic() + _BUDGET
        with concurrent.futures.ThreadPoolExecutor(max_workers=_WORKERS) as pool:
            list(pool.map(lambda key: _close_if_live(key, deadline), abandoned))
    except Exception:
        return 0
    return 0


if __name__ == "__main__":
    sys.exit(main())
