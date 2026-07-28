"""Contract for the codex-side `tools:` gate (block_undeclared_tools.py).

Claude honours `tools:` natively — a tool left off the list is never offered. On
codex the field is dropped, so the read-only agents ran with full write access.
This hook restores the declaration's meaning there, and the tests below pin the
boundary in both directions: what it must refuse, and everything it must leave
alone. A gate that denies more than the declaration does makes codex the stricter
harness, which is the same divergence pointing the other way.
"""

import os
import subprocess
import sys

import pytest

HOOK = os.path.join(os.path.dirname(__file__), "..", "..",
                    "packages", "agents", "hooks", "block_undeclared_tools.py")
AGENT_FILE_VAR = "CODEX_RUN_AGENT_FILE"


def _definition(tmp_path, name, tools_line):
    """An agent definition carrying (or omitting) a `tools:` declaration."""
    body = "---\nname: %s\n%s\n---\n\nA frame.\n" % (name, tools_line)
    path = tmp_path / ("%s.md" % name)
    path.write_text(body)
    return str(path)


def _run(definition_path, tool_name):
    env = dict(os.environ)
    if definition_path:
        env[AGENT_FILE_VAR] = definition_path
    else:
        env.pop(AGENT_FILE_VAR, None)
    proc = subprocess.run(
        [sys.executable, HOOK],
        input='{"tool_name":"%s","tool_input":{}}' % tool_name,
        capture_output=True, text=True, env=env,
    )
    return proc.returncode, proc.stderr


def test_read_only_agent_is_refused_the_write_tool(tmp_path):
    path = _definition(tmp_path, "explorer", "tools: Read, Grep, Glob, Bash")
    code, err = _run(path, "apply_patch")
    assert code == 2
    assert "explorer agent declares no write tool" in err
    assert "Read, Grep, Glob, Bash" in err   # the message names what it does have


@pytest.mark.parametrize("tool", ["shell_command", "exec_command", "Bash"])
def test_the_shell_stays_reachable(tmp_path, tool):
    """Every read-only agent declares Bash, so shell is granted on both harnesses.

    Denying it here would withhold a capability Claude gives, and `echo x > file`
    is reachable on Claude for exactly these agents.
    """
    path = _definition(tmp_path, "explorer", "tools: Read, Grep, Glob, Bash")
    assert _run(path, tool)[0] == 0


@pytest.mark.parametrize("declared", [
    "tools: Read, Write, Bash",
    "tools: Edit",
    "tools: Read, MultiEdit, Bash",
])
def test_an_agent_declaring_a_write_tool_keeps_it(tmp_path, declared):
    path = _definition(tmp_path, "ponytail", declared)
    assert _run(path, "apply_patch")[0] == 0


def test_an_agent_declaring_no_tools_key_keeps_everything(tmp_path):
    """No `tools:` line means every tool on Claude, so nothing is withheld here."""
    path = _definition(tmp_path, "cto", "model: opus")
    assert _run(path, "apply_patch")[0] == 0


def test_an_interactive_session_is_not_gated(tmp_path):
    """Only a codex-run sets the variable; a plain session declares nothing."""
    assert _run("", "apply_patch")[0] == 0


def test_an_unreadable_definition_does_not_deny(tmp_path):
    """The inverse of the memory contract, on purpose.

    Memory treats an unreadable definition as a denial because permission must
    never be assumed. Here the capability at stake is one Claude would have
    granted, so refusing it on a transient read failure would break a writing
    agent for no safety gain.
    """
    assert _run(str(tmp_path / "absent.md"), "apply_patch")[0] == 0


def test_a_bracketed_list_is_read(tmp_path):
    """`tools: [Read, Bash]` is the same declaration as the bare list."""
    path = _definition(tmp_path, "explorer", "tools: [Read, Bash]")
    assert _run(path, "apply_patch")[0] == 2


def test_an_unmapped_tool_is_ignored(tmp_path):
    path = _definition(tmp_path, "explorer", "tools: Read, Bash")
    assert _run(path, "WebFetch")[0] == 0
