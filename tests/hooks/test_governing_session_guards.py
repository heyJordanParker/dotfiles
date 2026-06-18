"""The proposing/executing guards honor the launching (governing) session.

A codex run launched by Claude is a child of the launching Claude session and
must obey that session's proposing/executing mode — proposing means the codex
run is read-only too. The launching session's id is inherited in the environment
(CLAUDE_CODE_SESSION_ID); the run carries its own, different session id in the
hook payload, with no classified mode of its own.

Both guards resolve the governing session through lib.event.owner_session, which
prefers CLAUDE_CODE_SESSION_ID over the payload's own session_id. These cases run
each guard as a subprocess exactly as the harness does, with the governing
session pinned to proposing on the spine and the run's own session id distinct
and unseeded — and assert the edit is blocked anyway, because the governing
session governs.

The plain-Claude case (governing == own) is covered by test_proposal_guard_redirects
and test_local_llm_fallbacks; here the point is the divergence a codex run introduces.
"""

import json
import os
import subprocess

import pytest
from conftest import PY_HOOKS

PROPOSAL_GUARD = os.path.join(PY_HOOKS, "block_edits_during_proposal.py")
COMMIT_GUARD = os.path.join(PY_HOOKS, "block_unauthorized_commits.py")
REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))

GOVERNING_SID = "launching_claude_session"
RUN_OWN_SID = "codex_run_own_session"


@pytest.fixture
def governing_proposing(tmp_path, monkeypatch):
    """Pin the launching (governing) session to proposing on the spine, and export
    its id as CLAUDE_CODE_SESSION_ID — the way a codex run inherits its launcher's
    identity.

    The run's own session is seeded to the OPPOSITE state — executing, with a
    commit authorized — so resolving the run's own session (the bug) would allow
    the edit and the commit. Only resolving the governing session blocks them. A
    test that left the run's own session unseeded could not tell the fix from the
    bug, because an unseeded session defaults to proposing / unauthorized and would
    block under either resolver."""
    monkeypatch.setenv("CLAUDE_DATA_ROOT", str(tmp_path))
    monkeypatch.setenv("CLAUDE_CODE_SESSION_ID", GOVERNING_SID)

    governing_dir = tmp_path / "sessions" / GOVERNING_SID
    governing_dir.mkdir(parents=True)
    (governing_dir / "state.json").write_text(
        json.dumps({"state": "proposing", "commit_requested": False})
    )

    run_dir = tmp_path / "sessions" / RUN_OWN_SID
    run_dir.mkdir(parents=True)
    (run_dir / "state.json").write_text(
        json.dumps({"state": "executing", "commit_requested": True})
    )


def _run(hook, tool_input):
    """Run the guard with the run's own (non-governing) session id in the payload."""
    payload = json.dumps({
        "session_id": RUN_OWN_SID,
        "cwd": REPO,
        "tool_input": tool_input,
    })
    return subprocess.run(
        ["python3", hook], input=payload, text=True, capture_output=True, cwd=REPO
    ).returncode


def test_proposal_guard_blocks_codex_edit_under_governing_proposing(governing_proposing):
    """A codex run editing a repo file is blocked because the launching session is
    proposing — even though the run's own session is seeded executing, which the
    pre-fix resolver would have consulted and allowed."""
    assert _run(PROPOSAL_GUARD, {"file_path": os.path.join(REPO, "note.txt")}) == 2


def test_proposal_guard_blocks_codex_redirect_under_governing_proposing(governing_proposing):
    """Same for a Bash redirect into a repo file from the codex run."""
    assert _run(PROPOSAL_GUARD, {"command": "echo x > note.txt"}) == 2


def test_commit_guard_blocks_codex_commit_under_governing_session(governing_proposing):
    """The launching session never authorized a commit (commit_requested False), so
    a git commit from the codex run is blocked — even though the run's own session
    is seeded commit_requested True, which the pre-fix resolver would have allowed."""
    assert _run(COMMIT_GUARD, {"command": "git commit -m x"}) == 2
