"""Contract for the access declarations (block_denied_access.py).

`readonly: true` and `ssh: enabled` are ours, not the harness's, and one
definition governs an agent on both. The tests pin the boundary in both
directions: what each declaration must refuse — including every shell shape that
used to walk around a string match — and everything it must leave alone, because
a gate that denies more than the declaration does breaks agents rather than
protecting them.
"""

import contextlib
import io
import json
import os
import subprocess
import sys

import block_denied_access
import pytest
from conftest import PY_HOOKS

HOOK = os.path.join(PY_HOOKS, "block_denied_access.py")
AGENT_FILE_VAR = "CODEX_RUN_AGENT_FILE"


def _definition(tmp_path, name, *lines):
    body = "---\nname: %s\n%s\n---\n\nA frame.\n" % (name, "\n".join(lines))
    path = tmp_path / ("%s.md" % name)
    path.write_text(body)
    return str(path)


@contextlib.contextmanager
def _environment(**values):
    """The variables main() reads, set for the call and restored after it."""
    before = {k: os.environ.get(k) for k in values}
    for k, v in values.items():
        os.environ.pop(k, None) if v is None else os.environ.__setitem__(k, v)
    try:
        yield
    finally:
        for k, v in before.items():
            os.environ.pop(k, None) if v is None else os.environ.__setitem__(k, v)


def _call(payload, **environment):
    """main() in this process: it reads stdin and the environment, nothing else."""
    stderr = io.StringIO()
    stdin, sys.stdin = sys.stdin, io.StringIO(json.dumps(payload))
    try:
        with contextlib.redirect_stderr(stderr), _environment(**environment):
            code = block_denied_access.main()
    finally:
        sys.stdin = stdin
    return code, stderr.getvalue()


def _run(definition_path, payload):
    """Run the gate as a codex run, whose launcher exports the definition path."""
    return _call(payload, **{AGENT_FILE_VAR: definition_path or None})


def _run_process(definition_path, payload):
    """The same call as a real process, for the exit code and the stderr envelope."""
    env = dict(os.environ)
    env.pop(AGENT_FILE_VAR, None)
    if definition_path:
        env[AGENT_FILE_VAR] = definition_path
    proc = subprocess.run([sys.executable, HOOK], input=json.dumps(payload),
                          capture_output=True, text=True, env=env)
    return proc.returncode, proc.stderr


def _bash(cmd):
    return {"tool_name": "Bash", "tool_input": {"command": cmd}}


def _readonly(tmp_path, name="explorer"):
    return _definition(tmp_path, name, "tools: Read, Grep, Glob, Bash", "readonly: true")


# --- readonly: the write tools ----------------------------------------------

@pytest.mark.parametrize("tool", ["Write"])
def test_readonly_refuses_every_write_tool(tmp_path, tool):
    code, err = _run(_readonly(tmp_path), {"tool_name": tool, "tool_input": {}})
    assert code == 2
    assert "readonly: true" in err




@pytest.mark.parametrize("declared", ["tools: Read, Write, Bash"])
def test_an_agent_declaring_a_write_tool_keeps_it(tmp_path, declared):
    path = _definition(tmp_path, "ponytail", declared)
    assert _run(path, {"tool_name": "apply_patch", "tool_input": {}})[0] == 0




# --- readonly: the shell -----------------------------------------------------

@pytest.mark.parametrize("command", ["trace grep foo"])
def test_readonly_keeps_the_commands_it_reads_with(tmp_path, command):
    code, err = _run(_readonly(tmp_path), _bash(command))
    assert code == 0, err


@pytest.mark.parametrize("command", ["sed -i '' s/a/b/ file.py"])
def test_readonly_refuses_every_way_to_change_the_tree(tmp_path, command):
    code, err = _run(_readonly(tmp_path), _bash(command))
    assert code == 2, command
    assert "readonly: true" in err






def _codex_shell(cmd):
    """codex hands a shell call over as a list, which command_str joins."""
    return {"tool_name": "shell_command",
            "tool_input": {"command": ["/bin/zsh", "-lc", cmd]}}








# --- ssh: opt-in, and independent of readonly --------------------------------

@pytest.mark.parametrize("command", ["ssh prod 'uptime'"])
def test_an_agent_that_did_not_declare_ssh_cannot_reach_a_machine(tmp_path, command):
    path = _definition(tmp_path, "ponytail", "model: opus")
    code, err = _run(path, _bash(command))
    assert code == 2, command
    assert "ssh: enabled" in err


def test_an_agent_declaring_ssh_reaches_the_machine(tmp_path):
    """One of the two cases run as a real process: the allowing exit code is what
    the harness reads to let the call through."""
    path = _definition(tmp_path, "deployer", "ssh: enabled")
    assert _run_process(path, _bash("ssh prod 'uptime'"))[0] == 0








# --- who the declarations govern ---------------------------------------------

def test_a_claude_subagent_is_gated_by_its_own_definition(tmp_path):
    """The Claude half: no exported path, the name on the payload resolves the
    definition under the active config root."""
    root = tmp_path / "root"
    (root / "agents").mkdir(parents=True)
    (root / "agents" / "explorer.md").write_text(
        "---\nname: explorer\nreadonly: true\n---\n\nA frame.\n")
    code, _err = _call({"agent_id": "sub-1", "agent_type": "explorer", **_bash("cp a b")},
                       CLAUDE_CONFIG_DIR=str(root), **{AGENT_FILE_VAR: None})
    assert code == 2


def test_the_architects_own_session_is_never_gated(tmp_path):
    """`--agent cto` puts agent_type on the main thread with no agent_id. His
    shell is not a one-shot execution and keeps everything."""
    code, _err = _call({"agent_type": "explorer", **_bash("ssh prod")},
                       **{AGENT_FILE_VAR: None})
    assert code == 0








