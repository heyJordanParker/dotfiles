"""The `trace` CLI as hooks run it: under the event's own session, and docs
sized to fit one hook message.

`trace` keys its session log by AGENT_SESSION_ID, the harness-neutral carrier it
resolves first. Hooks set it on a copy of the environment only:
CLAUDE_CODE_SESSION_ID stays as the launcher set it, because on a codex run it
names the launching Claude session whose mode owner_session resolves.
"""

import os
import shutil
import subprocess
from functools import lru_cache

from lib import feedback
from lib.event import field


@lru_cache
def binary():
    """`trace` on PATH, else the plugin's own launcher, else "" — a plugin
    consumer has the launcher without trace on PATH."""
    found = shutil.which("trace")
    if found:
        return found
    plugin_root = os.environ.get("CLAUDE_PLUGIN_ROOT") or os.path.join(
        os.path.expanduser("~"), ".claude/plugins/talents/talent-tree/packages/claude"
    )
    launcher = os.path.join(plugin_root, "bin", "trace")
    return launcher if os.path.isfile(launcher) and os.access(launcher, os.X_OK) else ""


def available():
    return bool(binary())


def room(name):
    """The characters a `name` hook message holds for trace's text."""
    return feedback.CONTEXT_LIMIT - len(feedback.wrap(name, ""))


def _session_env(event):
    env = dict(os.environ)
    session_id = field(event, "session_id", "")
    agent_id = field(event, "agent_id", "")
    if session_id:
        env["AGENT_SESSION_ID"] = session_id
    if agent_id:
        env["TRACER_AGENT_ID"] = agent_id
    return env


def run(event, *args, timeout=10):
    """`trace <args>` under the event's session, from the event's working
    directory: (exit code, stdout, stderr).

    trace keeps a session's record under the repository it runs in, so every
    call for one session runs in the session's own directory — otherwise one
    call records a doc where the next never looks."""
    cwd = field(event, "cwd", "") or None
    try:
        done = subprocess.run(
            [binary(), *args], capture_output=True, text=True, timeout=timeout,
            env=_session_env(event), cwd=cwd,
        )
        return done.returncode, done.stdout, done.stderr
    except subprocess.TimeoutExpired:
        return 1, "", "timed out"
    except Exception as error:
        return 1, "", str(error)


def session_start(event, source, send):
    start = field(event, "source", "")
    if start == "clear":
        run(event, "docs", "reset", "--source", source)
    run(event, "docs", "prime", "--reason", "post_compact" if start == "compact" else "session_start")
    send()


def resolve(target, cwd):
    """`target` as an absolute, normalized path, or "" for an empty one."""
    target = str(target).strip().strip('"').strip("'")
    if not target:
        return ""
    target = os.path.expanduser(target)
    if not os.path.isabs(target):
        target = os.path.join(cwd, target)
    return os.path.normpath(target)


def docs(event, targets, name, tool, command=None, skip=()):
    """The project docs for `targets` not yet in the agent's context, as Markdown
    sized to fit one `name` hook message: (exit code, text, stderr).

    `trace` sends each doc from its first unread line, nearest first, cuts the
    first one that does not fit at a whole line, and names the rest. A doc
    counts as loaded only once every line arrived, so one cut short continues
    on the next message. `skip` names docs the triggering command delivers
    itself."""
    args = ["docs", *targets, "--budget", str(room(name)), "--source", name, "--triggering-tool", tool]
    for path in skip:
        args += ["--skip", path]
    if command is not None:
        args += ["--triggering-command", command]
    return run(event, *args)
