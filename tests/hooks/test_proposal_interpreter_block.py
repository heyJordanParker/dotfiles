"""Coverage for the proposing-state interpreter-execution rule.

While a proposal is expected, block_writes allows an interpreter
to run an existing script in our tree, but blocks inline code and every other
script. It leaves the named tools (codex, codex-run, trace, git, uv/pytest, npm)
running, and never blocks *writing* a script outside that tree: it allows the
write and emits a heads-up that execution is blocked.

Each case calls the hook's main() against an isolated proposing-state spine and
asserts the return code (2 = blocked, 0 = allowed). The guard resolves its session
from os.environ at call time, so a seeded environment and a direct call prove what
a spawn proves; the spawned exit code is pinned once in
test_proposal_guard_redirects.
"""

import io
import json
import os
import sys

import block_writes
import pytest

GOVERNING_SID = "test_interp_block"
RUN_OWN_SID = "test_interp_block_run_own"
REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))


@pytest.fixture
def proposing(tmp_path, monkeypatch):
    """Pin the governing session to propose on the spine, the way the guard reads it.

    The guard resolves the session through lib.event.owner_session, which prefers
    CLAUDE_CODE_SESSION_ID over the payload's session_id, so the id has to be in the
    environment or the seeded record is never read. The run's own session — the id in
    the payload — is seeded to the OPPOSITE state, execute, so a resolver that read the
    payload would run every interpreter and fail these cases. An unseeded session
    defaults to propose and would block under either resolver, proving nothing."""
    monkeypatch.setenv("CLAUDE_DATA_ROOT", str(tmp_path))
    monkeypatch.setenv("CLAUDE_CODE_SESSION_ID", GOVERNING_SID)
    governing = tmp_path / "sessions" / GOVERNING_SID
    governing.mkdir(parents=True)
    (governing / "state.json").write_text(json.dumps({"state": "propose"}))
    run_own = tmp_path / "sessions" / RUN_OWN_SID
    run_own.mkdir(parents=True)
    (run_own / "state.json").write_text(json.dumps({"state": "execute"}))


def _run(monkeypatch, tool_input, cwd=REPO):
    monkeypatch.delenv("CODEX_RUN_AGENT_FILE", raising=False)
    payload = json.dumps({"session_id": RUN_OWN_SID, "cwd": cwd, "tool_input": tool_input})
    monkeypatch.setattr(sys, "stdin", io.StringIO(payload))
    return block_writes.main()


# Interpreter executions that are inline, outside our tree, or do not name a real
# script in our tree must block (exit 2).
INTERP_BLOCKED = [
    'python3 -c "x=1"',
]

INTERP_ALLOWED = [
    "python3 scripts/sync.py",
]

# Named tools — including the codex flow via codex-run — must run (exit 0)
TOOLS_ALLOWED = [
    'codex-run @architect "review this"',
    "codex exec -s read-only 'x'",
    "trace grep foo",
    "git log",
    "uv run pytest tests/hooks/",
    "npm run build",
]


@pytest.mark.parametrize("cmd", INTERP_BLOCKED)
def test_interpreter_execution_blocked(proposing, monkeypatch, cmd):
    assert _run(monkeypatch, {"command": cmd}) == 2


@pytest.mark.parametrize("cmd", INTERP_ALLOWED)
def test_existing_tree_script_is_allowed(proposing, monkeypatch, cmd):
    assert _run(monkeypatch, {"command": cmd}) == 0










# --- a dispatched codex agent is not the architect's conversation -------------

def _run_as(monkeypatch, tool_input, agent_file, tool_name="Bash"):
    if agent_file:
        monkeypatch.setenv("CODEX_RUN_AGENT_FILE", agent_file)
    else:
        monkeypatch.delenv("CODEX_RUN_AGENT_FILE", raising=False)
    payload = json.dumps({"session_id": RUN_OWN_SID, "cwd": REPO,
                          "tool_name": tool_name, "tool_input": tool_input})
    monkeypatch.setattr(sys, "stdin", io.StringIO(payload))
    return block_writes.main()


AGENT = os.path.join(REPO, "packages", "agents", "agents", "ponytail.md")






_PATCH = ("*** Begin Patch\n*** Update File: docs/x.md\n@@\n"
          "+echo hi > packages/out.txt\n*** End Patch")


