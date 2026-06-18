"""Coverage for the background guard on codex-run.

The guard forces codex-run Bash calls into the background: a real foreground
invocation is blocked (exit 2) unless run_in_background is set, a backgrounded one
passes (exit 0), and a mere mention of the wrapper as an argument (inspecting it,
listing it) is never blocked. It must recognize the invocation in command
position in every shell form it takes — bare, behind a path, behind a separator,
and wrapped in quotes — so a quoted path cannot bypass it.

Each case runs the hook as a subprocess and asserts the exit code.
"""

import json
import os
import subprocess

from conftest import PY_HOOKS

HOOK = os.path.join(PY_HOOKS, "enforce_background_codex_run.py")


def _run(command, background=False):
    payload = {"tool_name": "Bash", "tool_input": {"command": command}}
    if background:
        payload["tool_input"]["run_in_background"] = True
    return subprocess.run(
        ["python3", HOOK], input=json.dumps(payload),
        text=True, capture_output=True,
    ).returncode


# --- foreground invocations are blocked, in every command-position form -----------

def test_blocks_bare_foreground():
    assert _run('codex-run @architect "review this"') == 2


def test_blocks_path_foreground():
    assert _run('~/.local/bin/codex-run @architect "x"') == 2


def test_blocks_after_separator():
    assert _run('cd /repo && codex-run @architect "x"') == 2


def test_blocks_quoted_bare():
    # The bypass: quoting the command path slipped past the old guard.
    assert _run('"codex-run" @architect "x"') == 2


def test_blocks_quoted_path():
    assert _run('"$HOME/.local/bin/codex-run" @architect "x"') == 2


def test_blocks_single_quoted_bare():
    assert _run("'codex-run' @architect 'x'") == 2


def test_blocks_single_quoted_path():
    assert _run("'$HOME/.local/bin/codex-run' @architect 'x'") == 2


def test_blocks_env_assignment_prefix():
    # The bypass an env-assignment prefix opened: FOO=1 codex-run … is a real run.
    assert _run('FOO=1 codex-run @architect "x"') == 2


def test_blocks_env_command_prefix():
    assert _run('env X=1 codex-run @architect "x"') == 2


def test_blocks_time_prefix():
    # `time codex-run …` is a foreground run wrapped in the time keyword.
    assert _run('time codex-run @architect "x"') == 2


def test_blocks_exec_prefix():
    # `exec codex-run …` runs the wrapper foreground, replacing the shell.
    assert _run('exec codex-run @architect "x"') == 2


def test_blocks_command_prefix():
    # `command codex-run …` runs the wrapper foreground via the command builtin.
    assert _run('command codex-run @architect "x"') == 2


def test_blocks_newline_separated():
    # A newline between commands is a real separator, like `;` — the codex-run
    # after it must still be seen, not collapsed into the prior segment.
    assert _run('cd /repo\ncodex-run @architect "x"') == 2


# --- backgrounded invocations pass ------------------------------------------------

def test_allows_backgrounded_foreground():
    assert _run('codex-run @architect "x"', background=True) == 0


def test_allows_backgrounded_quoted():
    assert _run('"codex-run" @architect "x"', background=True) == 0


def test_allows_backgrounded_exec_prefix():
    assert _run('exec codex-run @architect "x"', background=True) == 0


def test_allows_backgrounded_command_prefix():
    assert _run('command codex-run @architect "x"', background=True) == 0


def test_allows_backgrounded_newline_separated():
    assert _run('cd /repo\ncodex-run @architect "x"', background=True) == 0


# --- a mention as an argument is never a run, quoted or not -----------------------

def test_allows_mention_which():
    assert _run("which codex-run") == 0


def test_allows_mention_ls_path():
    assert _run("ls packages/bin/codex-run") == 0


def test_allows_mention_ls_quoted_path():
    assert _run('ls "packages/bin/codex-run"') == 0


def test_allows_mention_cat_quoted():
    assert _run('cat "packages/bin/codex-run"') == 0


def test_allows_mention_echo_arg():
    assert _run("echo codex-run") == 0


def test_allows_mention_in_quoted_string():
    # The wrapper name inside an unrelated quoted string is a mention, not a run —
    # the false-positive the raw-string matcher tripped on.
    assert _run('echo "run codex-run in the background"') == 0
    assert _run("git commit -m 'document codex-run usage'") == 0


def test_allows_exec_command_mention():
    # `exec`/`command` as an argument (not in command position) is a mention, and
    # the wrapper-word peel must not fire on codex-run sitting in argument position.
    assert _run("echo exec codex-run") == 0
    assert _run("echo command codex-run") == 0


def test_allows_ordinary_bash():
    assert _run("git log") == 0
