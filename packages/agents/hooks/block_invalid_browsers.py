#!/usr/bin/env python3
"""Block hand-driven browsers so browser work goes through agent-browser, headless.

Agents reach for Playwright, Puppeteer, the raw Chrome binary, or a remote-debugging
attach instead of the one CLI whose process lifecycle is managed. Those runs orphan
Chrome processes on the architect's machine.

The guard decides on execution position, never on pattern-in-text: a segment blocks
when the thing being executed is a browser launch — the resolved executable after
shell reserved words (`then`, `do`, `{`), transparent prefixes (`sudo`, `env`,
`timeout`, `xargs`, `nohup`), and package runners (`npx`, `bunx`, `npm exec`,
`bun run`, `deno run`, `uv run`, `python -m`), an `open` that reaches a browser, or
an inline script payload handed to an interpreter. Text that merely names a browser
(a commit message, a note, a dependency install, a `grep`, `BROWSER=chrome npm run
test:e2e`) never blocks, and neither do suite runs (`playwright test`, `cypress run`,
`bun run test:e2e`), version probes (`chromium --version`) or process inspection
(`pkill`, `lsof`, `killall`, `which`).

Codex sends its shell command as a list joined into `/bin/zsh -lc <command>`, so every
segment head would otherwise be `zsh`; a shell wrapper is unwrapped and its inner
command classified. Command substitutions are classified as commands of their own,
so a launch inside backticks blocks whatever the outer head is.

This is a deterrent and a redirect at the command layer, not a sandbox. An agent that
wants around it can compute a launch at runtime, drive AppleScript, call
`webbrowser.open`, or write a script and run it in a later turn. Those are out of
scope on purpose; chasing them buys nothing and costs false positives. What is in
scope is every spelling an agent reaches for by habit.
"""

import os
import re
import sys

from lib import feedback
from lib.command import segments, tokenize
from lib.event import command_str, read_event

BINDING = {
    "events": {"PreToolUse": ["Bash"]},
    "timeout": 5,
    "harness": "all",
    "roots": "all",
}

MESSAGE = (
    "BLOCKED: browser driving goes through agent-browser, headless.\n"
    "\n"
    "Read the usage docs with `agent-browser skills get core`, then use the CLI:\n"
    "open, snapshot, click, fill, screenshot, close. Never launch Chrome,\n"
    "Playwright, Puppeteer, or any other browser yourself, and never pass --headed."
)

_ASSIGNMENT = re.compile(r"^([A-Za-z_][A-Za-z0-9_]*)=(.*)$", re.S)

_SHELLS = {"zsh", "bash", "sh", "dash", "ksh", "fish"}
# `-c`, and the login/interactive spellings codex uses (`-lc`, `-lic`, `-ic`).
_SHELL_COMMAND_FLAG = re.compile(r"^-[a-z]*c$")

# Shell grammar words that stand in front of the command actually run.
_RESERVED = {
    "if", "then", "elif", "else", "fi", "while", "until", "do", "done",
    "for", "case", "esac", "select", "function", "{", "}", "!", "[[", "]]",
}
# Wrappers that run their remaining words as a command of their own.
_PREFIXES = {
    "command", "builtin", "exec", "nohup", "sudo", "doas", "time", "setsid",
    "caffeinate", "stdbuf", "timeout", "xargs", "nice", "ionice", "unbuffer",
    "script", "strace", "dtruss",
}
# `timeout 60`, `nice -n 10` — a bare number belonging to the prefix, not a command.
_PREFIX_ARGUMENT = re.compile(r"^\d+(\.\d+)?[smhd]?$")
_PREFIX_FLAG_WITH_VALUE = {"-u", "--user", "-n", "-k", "--kill-after", "-s", "--signal", "-I", "-i"}

_RUNNER_ONE = {"npx", "bunx", "pnpx", "uvx"}
_RUNNER_TWO = {
    ("npm", "exec"), ("npm", "run"),
    ("pnpm", "exec"), ("pnpm", "run"), ("pnpm", "dlx"),
    ("yarn", "run"), ("yarn", "dlx"),
    ("bun", "run"), ("bun", "x"),
    ("deno", "run"),
    ("uv", "run"), ("poetry", "run"), ("pdm", "run"), ("rye", "run"),
}
_FLAG_WITH_VALUE = {
    "-p", "--package", "--with", "--with-editable", "--from", "--allow-run",
    "--project", "--python", "--directory", "--cwd", "--prefix", "--workspace",
    "-w", "--filter", "--index-url", "--node-modules-dir", "--config-file",
}
# Runner flags whose value is itself a command string (`npx -c "playwright open"`).
_COMMAND_FLAGS = {"-c", "--call"}
_PYTHONS = {"python", "python2", "python3", "py"}

# Interpreters that execute an inline script handed to them on the command line.
_INTERPRETERS = {"node", "deno", "bun", "ruby", "php", "perl", "osascript"} | _PYTHONS
_PAYLOAD_FLAGS = {"-e", "--eval", "-p", "--print", "-c", "-r", "--command"}

_DRIVERS = {"chromedriver", "geckodriver", "msedgedriver", "safaridriver", "operadriver"}
# Executable names. Deliberately narrower than the .app set: `arc`, `edge`, and
# `opera` are real non-browser commands, so they only count as an application name.
_BROWSER_EXECUTABLE = re.compile(
    r"^(google[ _-]?chrome(-stable)?|chrome|chromium(-browser)?|firefox(-bin)?"
    r"|safari|microsoft[ _-]?edge|msedge|brave(-browser)?|thorium|waterfox|librewolf)"
    r"([ _-](canary|beta|dev|developer[ _-]edition|nightly|stable))*$", re.I
)
# Application names `open` reaches, including the vendors whose bare name is also a
# common command, which is safe here because the position is already an app name.
_BROWSER_APPLICATION = re.compile(
    r"^(google[ _-]?chrome(-stable)?|chrome|chromium(-browser)?|firefox(-bin)?"
    r"|safari(-technology[ _-]preview)?|microsoft[ _-]?edge|msedge|edge|brave(-browser)?"
    r"|thorium|waterfox|librewolf|arc|opera(-gx)?|vivaldi|orion|zen(-browser)?|dia"
    r"|tor[ _-]browser|duckduckgo|min|sigmaos)"
    r"([ _-](canary|beta|dev|developer[ _-]edition|nightly|stable|browser))*$", re.I
)
_BROWSER_BUNDLE_ID = re.compile(
    r"^(com\.google\.chrome|com\.apple\.safari|org\.mozilla\.firefox"
    r"|com\.microsoft\.edgemac|com\.brave\.browser|org\.chromium)", re.I
)
_URL = re.compile(r"^(https?://|www\.)", re.I)
# A browser run purely to report itself opens nothing.
_INFORMATIONAL = {"--version", "-v", "-V", "--help", "-h", "--product-version", "help"}

_PLAYWRIGHT_LAUNCH = {"open", "codegen", "screenshot", "pdf", "launch-server", "cr", "ff", "wk"}
_CYPRESS_LAUNCH = {"open"}
_AGENT_BROWSER_OPENS = {"open"}
_AGENT_BROWSER_CONFIG = "AGENT_BROWSER_CONFIG"
_AGENT_BROWSER_HEADED = "AGENT_BROWSER_HEADED"

# Patterns that mean a browser launch when they appear in an executed script payload.
# Naming a stack is not enough — `console.log('selenium')` drives nothing — so every
# pattern is an import of a driver or a call that opens a browser.
_PAYLOAD_PATTERNS = [
    re.compile(p, re.I) for p in (
        r"chromium\.launch",
        r"firefox\.launch",
        r"webkit\.launch",
        r"launchPersistentContext",
        r"connectOverCDP",
        r"puppeteer\.launch",
        r"(require\s*\(|import\s+|from\s+)\s*['\"]?(node:)?"
        r"(@playwright/test|playwright[\w.]*|puppeteer(-core)?|selenium[\w.]*"
        r"|pyppeteer|cypress|webdriverio|splinter|helium)\b",
        r"\bwebdriver\.\w+\(",
        r"/(Google Chrome|Chromium|Firefox|Safari)\.app/",
        r"--remote-debugging-port",
        r"--dump-dom",
        r"--screenshot=",
        r"\b(sync|async)_playwright\b",
        r"\b(chromedriver|geckodriver)\b.{0,40}\b(Service|start|launch)\b",
    )
]

# `channel: 'chrome'` only means a real Chrome when it configures a launch.
_CHANNEL_CHROME = re.compile(r"channel\s*[:=]\s*['\"]?chrome", re.I)
_LAUNCH = re.compile(r"launch", re.I)

_HEREDOC = re.compile(r"<<-?\s*(['\"]?)(\w+)\1(.*?)^\s*\2\s*$", re.S | re.M)
_SINGLE_QUOTED = re.compile(r"'[^']*'")
_BACKTICK = re.compile(r"(?<!\\)`([^`]*)`")
_LOOKUPS = {"which", "command", "type", "whence", "whereis"}
_PLACEHOLDER = re.compile(r"^__substitution(\d+)__$")
_VARIABLE = re.compile(r"^\$\{?([A-Za-z_][A-Za-z0-9_]*)\}?$")


def _base(token):
    return os.path.basename(token.strip("\"'")).lower()


def _tool(token):
    t = token.strip("\"'")
    for prefix in ("npm:", "jsr:"):
        if t.lower().startswith(prefix):
            t = t[len(prefix):]
    t = os.path.basename(t).lower()
    # `playwright@latest`, `playwright@1.4` — a version tag, never a scope.
    t = re.sub(r"(?<=.)@[^/]*$", "", t)
    return t[:-4] if t.endswith(".exe") else t


def _application_name(token):
    name = _base(token)
    return name[:-4] if name.endswith(".app") else name


def _is_browser(token):
    name = _application_name(token)
    return name in _DRIVERS or bool(_BROWSER_EXECUTABLE.match(name))


def _is_browser_application(token):
    name = _application_name(token)
    return name in _DRIVERS or bool(_BROWSER_APPLICATION.match(name))


def _skip_prefix_arguments(words, i):
    while i < len(words):
        if words[i] in _PREFIX_FLAG_WITH_VALUE:
            i += 2
        elif words[i].startswith("-") or _PREFIX_ARGUMENT.match(words[i]):
            i += 1
        else:
            break
    return i


def _effective(words):
    """(env assignments, words of the command executed, nested command string)."""
    assignments, i = [], 0
    while i < len(words):
        match = _ASSIGNMENT.match(words[i])
        if not match:
            break
        assignments.append((match.group(1), match.group(2)))
        i += 1

    while i < len(words):
        head = _base(words[i])
        if head in _RESERVED:
            i += 1
            continue
        if head == "env" or head in _PREFIXES:
            i += 1
            i = _skip_prefix_arguments(words, i)
            while i < len(words) and (match := _ASSIGNMENT.match(words[i])):
                assignments.append((match.group(1), match.group(2)))
                i += 1
            continue
        if head in _RUNNER_ONE:
            i += 1
        elif i + 1 < len(words) and (head, words[i + 1]) in _RUNNER_TWO:
            i += 2
        elif head in _PYTHONS and i + 1 < len(words) and words[i + 1] == "-m":
            i += 2
        else:
            break
        while i < len(words) and words[i].startswith("-"):
            if words[i] in _COMMAND_FLAGS:
                return assignments, [], " ".join(words[i + 1:i + 2])
            i += 2 if words[i] in _FLAG_WITH_VALUE else 1
    return assignments, words[i:], ""


def _script_payload(words):
    """Everything after an inline-script flag, or '' when the run has no payload."""
    for i, word in enumerate(words[1:], start=1):
        if word in _PAYLOAD_FLAGS:
            return " ".join(words[i + 1:])
        flag, sep, value = word.partition("=")
        if sep and flag in _PAYLOAD_FLAGS:
            return " ".join([value] + words[i + 1:])
    return ""


def _payload_launches(payload):
    if _CHANNEL_CHROME.search(payload) and _LAUNCH.search(payload):
        return True
    return any(p.search(payload) for p in _PAYLOAD_PATTERNS)


def _headed(words):
    for i, word in enumerate(words):
        if word == "--headed":
            return i + 1 >= len(words) or words[i + 1] != "false"
        if word.startswith("--headed="):
            return word.split("=", 1)[1] != "false"
    return False


def _opens_browser(words):
    for i, word in enumerate(words[1:], start=1):
        stripped = word.strip("\"'")
        # A bare short name is only an application in `-a`/`-b` position: `open
        # dist/min` and `open -a Preview ~/docs/edge` name files, not browsers.
        if words[i - 1] in ("-a", "-b"):
            if _is_browser_application(stripped) or _BROWSER_BUNDLE_ID.match(stripped):
                return True
        elif _base(stripped).endswith(".app") and _is_browser_application(stripped):
            return True
    flagged = any(w in ("-a", "-b") for w in words)
    return not flagged and any(_URL.match(w.strip("\"'")) for w in words[1:])


def _launches(words, depth, resolved, browser_variables=()):
    assignments, executed, nested = _effective(words)
    environment = dict(assignments)
    if nested:
        return _launches_command(nested, depth + 1)
    if not executed:
        return False

    head = _base(executed[0])
    if head in _SHELLS:
        for i, word in enumerate(executed[1:], start=1):
            if _SHELL_COMMAND_FLAG.match(word):
                return _launches_command(" ".join(executed[i + 1:]), depth + 1)
        return False

    variable = _VARIABLE.match(executed[0].strip("\"'"))
    if variable and variable.group(1) in browser_variables:
        return True

    placeholder = _PLACEHOLDER.match(head)
    if placeholder:
        return resolved.get(int(placeholder.group(1)), False)

    tool = _tool(executed[0])
    sub = executed[1] if len(executed) > 1 else ""

    if _is_browser(executed[0]):
        return any(w not in _INFORMATIONAL for w in executed[1:]) or len(executed) == 1
    if tool == "open":
        return _opens_browser(executed)
    if tool == "agent-browser":
        opening = any(w in _AGENT_BROWSER_OPENS for w in executed[1:])
        headed = environment.get(_AGENT_BROWSER_HEADED, "false").strip("\"'") != "false"
        if opening and headed:
            return True
        # A config file can turn headed on, so an explicit config is unverifiable.
        return _headed(executed) or (
            opening and ("--config" in executed or _AGENT_BROWSER_CONFIG in environment)
        )
    if tool in ("playwright", "@playwright/test"):
        return sub in _PLAYWRIGHT_LAUNCH
    if tool == "cypress":
        return sub in _CYPRESS_LAUNCH
    if tool in ("puppeteer", "puppeteer-core"):
        return True
    if head in _INTERPRETERS:
        payload = _script_payload(executed)
        if payload:
            return _payload_launches(payload)
    return False


def _substitutions(text):
    """(start, end, body) for every command substitution the shell would run.

    `$(…)` and `<(…)`/`>(…)` process substitutions are matched on balanced parens,
    so a body holding its own parens or a nested substitution comes back whole.
    """
    found = []
    for opener in ("$(", "<(", ">("):
        i = 0
        while True:
            i = text.find(opener, i)
            if i < 0:
                break
            if i and text[i - 1] == "\\":
                i += 2
                continue
            depth, j = 0, i + 1
            while j < len(text):
                if text[j] == "(":
                    depth += 1
                elif text[j] == ")":
                    depth -= 1
                    if not depth:
                        break
                j += 1
            if j >= len(text):
                break
            found.append((i, j + 1, text[i + 2:j]))
            i = j + 1
    for match in _BACKTICK.finditer(text):
        found.append((match.start(), match.end(), match.group(1)))
    return sorted(found)


def _resolves_to_browser(body):
    """`$(which chromium)` in head position executes a browser."""
    words = tokenize(body) or []
    if len(words) > 1 and _base(words[0]) in _LOOKUPS:
        return any(_is_browser(w) for w in words[1:])
    return False


def _heredocs(command):
    """(owner words, body) per heredoc, and the command with quoted bodies removed.

    A quoted-delimiter body is never expanded, so it is text and comes out. An
    unquoted body stays in: the shell expands it, and a substitution written there
    runs like any other.
    """
    found, kept, last = [], [], 0
    for match in _HEREDOC.finditer(command):
        owner = re.split(r"[;&|\n]", command[last:match.start()])[-1]
        found.append((tokenize(owner) or [], match.group(3)))
        kept.append(command[last:match.start() if match.group(1) else match.end()])
        last = match.end()
    kept.append(command[last:])
    return found, " ".join(kept)


def _launches_command(command, depth=0):
    if depth > 4:
        return False
    heredocs, text = _heredocs(command)
    for words, body in heredocs:
        # `python3 - <<'PY'` runs the body whether or not the shell expands it.
        _, executed, _ = _effective(words)
        if executed and _base(executed[0]) in _INTERPRETERS and _payload_launches(body):
            return True

    # A single-quoted span is literal text, so a substitution inside it never runs.
    literal = _SINGLE_QUOTED.sub(lambda m: " " * len(m.group()), text)
    found = _substitutions(text)
    resolved = {}
    for index in reversed(range(len(found))):
        start, end, body = found[index]
        executed = literal[start] != " "
        if executed and _launches_command(body, depth + 1):
            return True
        resolved[index] = executed and _resolves_to_browser(body)
        text = text[:start] + " __substitution%d__ " % index + text[end:]

    segs = segments(text)
    if segs is None:
        # An unparseable command naming a browser is not waved through.
        return bool(re.search(r"chrom|firefox|safari|webkit|puppeteer|playwright"
                              r"|selenium|webdriver|cypress|edge|browser", command, re.I))
    # A variable holding a browser path is only a launch once it is executed:
    # `CHROME_BIN=… bun run test:e2e` configures a suite, `"$CHROME" --headless`
    # runs the browser.
    browser_variables = {
        name for seg in segs for name, value in _effective(seg)[0]
        if "/" in value.strip("\"'") and _is_browser(value)
    }
    return any(_launches(seg, depth, resolved, browser_variables) for seg in segs)


def main():
    if _launches_command(command_str(read_event())):
        return feedback.block("block_invalid_browsers", MESSAGE)
    return 0


if __name__ == "__main__":
    sys.exit(main())
