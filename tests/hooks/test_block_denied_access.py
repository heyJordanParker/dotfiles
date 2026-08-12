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

@pytest.mark.parametrize("tool", ["Write", "Edit", "MultiEdit", "NotebookEdit", "apply_patch"])
def test_readonly_refuses_every_write_tool(tmp_path, tool):
    code, err = _run(_readonly(tmp_path), {"tool_name": tool, "tool_input": {}})
    assert code == 2
    assert "readonly: true" in err


def test_tools_without_a_write_tool_still_refuses_apply_patch(tmp_path):
    """The capability the `tools:` gate carried before this one absorbed it.

    Claude never offers the write tools to such an agent; codex has no per-tool
    allowlist and drops the field, so the refusal has to happen here or the same
    definition means two different agents.
    """
    path = _definition(tmp_path, "tester", "tools: Read, Grep, Glob, Bash")
    assert _run(path, {"tool_name": "apply_patch", "tool_input": {}})[0] == 2


@pytest.mark.parametrize("declared", ["tools: Read, Write, Bash", "tools: Edit",
                                      "tools: [Read, MultiEdit]"])
def test_an_agent_declaring_a_write_tool_keeps_it(tmp_path, declared):
    path = _definition(tmp_path, "ponytail", declared)
    assert _run(path, {"tool_name": "apply_patch", "tool_input": {}})[0] == 0


def test_an_agent_declaring_nothing_keeps_everything(tmp_path):
    path = _definition(tmp_path, "cto", "model: opus")
    assert _run(path, {"tool_name": "apply_patch", "tool_input": {}})[0] == 0
    assert _run(path, _bash("sed -i '' s/a/b/ file"))[0] == 0


# --- readonly: the shell -----------------------------------------------------

@pytest.mark.parametrize("command", [
    "trace grep foo",
    "rg 'def main' packages",
    "git diff --stat",
    "git log --oneline -20",
    "cat packages/agents/Claude.md",
    "ls -la packages/agents/hooks",
    "sed -n '1,20p' file.py",
    "curl -s https://example.com/api",
    "find . -name '*.py' | head -20",
    "honcho context jordan",
])
def test_readonly_keeps_the_commands_it_reads_with(tmp_path, command):
    code, err = _run(_readonly(tmp_path), _bash(command))
    assert code == 0, err


@pytest.mark.parametrize("command", [
    "sed -i '' s/a/b/ file.py",
    "cat a.py > b.py",
    "echo x >> notes.md",
    "curl -o out.json https://example.com",
    "git commit -m 'x'",
    "git checkout -- .",
    "cp a b",
    "mv a b",
    "mkdir -p new",
    "tee out.txt",
    "python3 -c \"open('f','w').write('x')\"",
    "npm install",
    "uv run python script.py",
    "npx tsx build.ts",
])
def test_readonly_refuses_every_way_to_change_the_tree(tmp_path, command):
    code, err = _run(_readonly(tmp_path), _bash(command))
    assert code == 2, command
    assert "readonly: true" in err


@pytest.mark.parametrize("command", [
    "pytest tests/hooks",
    "uv run pytest tests/hooks",
    "npm test",
    "npm run typecheck",
    "ruff check .",
    "tsc --noEmit",
])
def test_readonly_runs_the_checks_that_only_report(tmp_path, command):
    """Reporting whether the suite passes is reading, not writing — a read-only agent
    that cannot run the checks cannot answer the question it was dispatched for."""
    code, err = _run(_readonly(tmp_path), _bash(command))
    assert code == 0, err


@pytest.mark.parametrize("command", [
    " sed -i '' s/a/b/ file.py",
    "echo hi\nsed -i '' s/a/b/ file.py",
    "env FOO=1 sed -i '' s/a/b/ file.py",
    "FOO=1 sed -i '' s/a/b/ file.py",
    "bash -c 'sed -i \"\" s/a/b/ file.py'",
    "sh -lc 'rm -rf build'",
    "/usr/bin/sed -i '' s/a/b/ file.py",
    "./write.sh",
    "trace grep foo; rm file.py",
    "trace grep foo && python3 script.py",
])
def test_readonly_refuses_the_shapes_a_string_match_missed(tmp_path, command):
    """Every one of these ran on a definition that said the agent never writes."""
    assert _run(_readonly(tmp_path), _bash(command))[0] == 2, command


def _codex_shell(cmd):
    """codex hands a shell call over as a list, which command_str joins."""
    return {"tool_name": "shell_command",
            "tool_input": {"command": ["/bin/zsh", "-lc", cmd]}}


@pytest.mark.parametrize("cmd,expected", [
    ("trace grep foo", 0),
    ("curl -s https://example.com", 0),
    ("git diff --stat", 0),
    ("sed -i '' s/a/b/ f", 2),
    ("cp a b", 2),
    ("echo x > notes.md", 2),
    ("python3 -c 'print(1)'", 2),
])
def test_the_declaration_means_the_same_through_the_codex_wrapper(tmp_path, cmd, expected):
    """Every codex shell call arrives wrapped. Judging the wrapper took the whole
    shell off a read-only agent there; ignoring what it wraps gave the agent every
    write. Both were live defects — this is the shape that decides the harnesses
    agree."""
    assert _run(_readonly(tmp_path), _codex_shell(cmd))[0] == expected, cmd


def test_readonly_blocks_a_command_it_cannot_parse(tmp_path):
    """Unresolvable is a block, never a guess — the delete guard's contract."""
    code, err = _run(_readonly(tmp_path), _bash("sed -i 'unbalanced"))
    assert code == 2
    assert "cannot be read" in err


def test_the_refusal_names_the_command(tmp_path):
    """A block an agent can act on: it says which word it refused. Run as a real
    process, so the blocking exit code and the stderr envelope are proven too."""
    code, err = _run_process(_readonly(tmp_path), _bash("cp a b"))
    assert code == 2
    assert "`cp`" in err


# --- ssh: opt-in, and independent of readonly --------------------------------

@pytest.mark.parametrize("command", [
    "ssh prod 'uptime'",
    "scp file prod:/tmp/",
    "rsync -av . prod:/srv",
    "sftp prod",
    "env X=1 ssh prod",
    "bash -c 'ssh prod uptime'",
    "/usr/bin/ssh prod",
    "sudo -u jordan ssh prod",
    "cd /tmp && ssh prod",
])
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


def test_ssh_stays_denied_to_a_readonly_agent_that_did_not_declare_it(tmp_path):
    assert _run(_readonly(tmp_path), _bash("ssh prod"))[0] == 2


def test_a_readonly_agent_may_declare_ssh(tmp_path):
    """The two declarations are independent: read the machine, change nothing."""
    path = _definition(tmp_path, "auditor", "readonly: true", "ssh: enabled")
    assert _run(path, _bash("ssh prod 'tail -100 /var/log/app.log'"))[0] == 0
    assert _run(path, _bash("sed -i '' s/a/b/ f"))[0] == 2


def test_the_word_ssh_in_an_argument_is_not_a_remote_command(tmp_path):
    path = _definition(tmp_path, "ponytail", "model: opus")
    assert _run(path, _bash("rg ssh packages/ssh/config"))[0] == 0


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


def test_an_interactive_codex_session_is_not_gated(tmp_path):
    assert _run("", _bash("ssh prod"))[0] == 0


def test_an_absent_definition_grants_nothing_and_restricts_nothing(tmp_path):
    """No file is the undeclared case: it writes, and it has no machine."""
    absent = str(tmp_path / "gone.md")
    assert _run(absent, {"tool_name": "apply_patch", "tool_input": {}})[0] == 0
    assert _run(absent, _bash("ssh prod"))[0] == 2


def test_an_unreadable_definition_denies_the_restriction_and_the_grant(tmp_path):
    """A definition that cannot be read denies in both directions: the write it
    might have refused, and the machine it never granted."""
    unreadable = tmp_path / "broken.md"
    unreadable.write_bytes(b"---\nname: x\nreadonly: true\n---\n\xff\xfe")
    assert _run(str(unreadable), {"tool_name": "apply_patch", "tool_input": {}})[0] == 2


def test_an_unmapped_tool_is_ignored(tmp_path):
    assert _run(_readonly(tmp_path), {"tool_name": "WebFetch", "tool_input": {}})[0] == 0
