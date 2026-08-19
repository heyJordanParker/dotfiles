"""The proposing/executing guards honor the launching (governing) session.

A codex run launched by Claude is a child of the launching Claude session and
must obey that session's proposing/executing mode — proposing means the codex
run is read-only too. The launching session's id is inherited in the environment
(CLAUDE_CODE_SESSION_ID); the run carries its own, different session id in the
hook payload, with no classified mode of its own.

Both guards resolve the governing session through lib.event.owner_session, which
reads os.environ at call time. The cases pin the governing session to proposing in
session state and give the run its own distinct session id, seeded to the opposite
state — and assert the edit is blocked anyway, because the governing session
governs. `monkeypatch.setenv` plus a direct `main()` call proves exactly what a
spawned process proves, so the proposal guard runs in-process here. The commit
guard keeps its spawn, because it is the only case in the suite that pins the exit
code the harness reads off block_unauthorized_commits.

The plain-Claude case (governing == own) is covered by test_proposal_guard_redirects
and test_local_llm_fallbacks; here the point is the divergence a codex run introduces.
"""

import io
import json
import os
import subprocess
import sys

import block_writes
import pytest
from conftest import PY_HOOKS

COMMIT_GUARD = os.path.join(PY_HOOKS, "block_unauthorized_commits.py")
REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))

GOVERNING_SID = "launching_claude_session"
RUN_OWN_SID = "codex_run_own_session"


@pytest.fixture
def governing_proposing(tmp_path, monkeypatch):
    """Pin the launching (governing) session to proposing in session state, and export
    its id as CLAUDE_CODE_SESSION_ID — the way a codex run inherits its launcher's
    identity.

    The run's own session is seeded to the OPPOSITE state — execute, with a
    commit authorized — so resolving the run's own session (the bug) would allow
    the edit and the commit. Only resolving the governing session blocks them. A
    test that left the run's own session unseeded could not tell the fix from the
    bug, because an unseeded session defaults to proposing / unauthorized and would
    block under either resolver."""
    monkeypatch.setenv("CLAUDE_DATA_ROOT", str(tmp_path))
    monkeypatch.setenv("CLAUDE_CODE_SESSION_ID", GOVERNING_SID)
    # The inherited codex-run marker would bypass the governing-session path.
    monkeypatch.delenv("CODEX_RUN_AGENT_FILE", raising=False)

    governing_dir = tmp_path / "sessions" / GOVERNING_SID
    governing_dir.mkdir(parents=True)
    (governing_dir / "state.json").write_text(
        json.dumps({"state": "propose", "commit_requested": False})
    )

    run_dir = tmp_path / "sessions" / RUN_OWN_SID
    run_dir.mkdir(parents=True)
    (run_dir / "state.json").write_text(
        json.dumps({"state": "execute", "commit_requested": True})
    )


def _payload(tool_input):
    """The run's own (non-governing) session id, the way a codex run sends it."""
    return json.dumps({
        "session_id": RUN_OWN_SID,
        "cwd": REPO,
        "tool_input": tool_input,
    })


def _call(main, tool_input, monkeypatch):
    monkeypatch.setattr(sys, "stdin", io.StringIO(_payload(tool_input)))
    return main()


def test_proposal_guard_blocks_codex_edit_under_governing_proposing(
        governing_proposing, monkeypatch):
    """A codex run editing a repo file is blocked because the launching session is
    proposing — even though the run's own session is seeded executing, which the
    pre-fix resolver would have consulted and allowed."""
    assert _call(block_writes.main,
                 {"file_path": os.path.join(REPO, "note.txt")}, monkeypatch) == 2


def test_proposal_guard_blocks_codex_redirect_under_governing_proposing(
        governing_proposing, monkeypatch):
    """Same for a Bash redirect into a repo file from the codex run."""
    assert _call(block_writes.main,
                 {"command": "echo x > note.txt"}, monkeypatch) == 2


def test_commit_guard_blocks_codex_commit_under_governing_session(governing_proposing):
    """The launching session never authorized a commit (commit_requested False), so
    a git commit from the codex run is blocked — even though the run's own session
    is seeded commit_requested True, which the pre-fix resolver would have allowed.

    Spawned, so the exit code the harness reads is the one asserted."""
    assert subprocess.run(
        ["python3", COMMIT_GUARD], input=_payload({"command": "git commit -m x"}),
        text=True, capture_output=True, cwd=REPO,
    ).returncode == 2
