"""Shared fixtures for the hook tests.

Runtime code is on sys.path via pyproject's `pythonpath = ["packages/agents/hooks", ...]`,
so `from lib import transcript` and `import block_git_revert` resolve with no install.
"""

import json
import os

import pytest

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
PY_HOOKS = os.path.join(REPO, "packages", "agents", "hooks")


@pytest.fixture
def write_transcript(tmp_path):
    """Write records (list of dicts) as a JSONL transcript; return its path."""
    def _write(records, name="transcript.jsonl"):
        # Compact, like Claude Code's real transcripts — the raw-line substring
        # gates (current_turn_lines, mutation/ExitPlanMode scans) depend on it.
        path = tmp_path / name
        path.write_text("\n".join(json.dumps(r, separators=(",", ":")) for r in records) + "\n")
        return str(path)
    return _write


@pytest.fixture
def local_llm(tmp_path, monkeypatch):
    """Force the LLM-gate hooks onto their deterministic graceful/fallback branch
    with no network and no real credentials. Two moves: pin MODEL_CALL_BACKEND to
    `claude` so the Python hooks resolve to the claude adapter (the default is the
    openai backend, which reads ~/.codex/auth.json and hits the network), then put
    a PATH up front where `claude` exits 1 and `timeout` just execs its command so
    the claude adapter takes the fallback. Returns the bin dir."""
    binv = tmp_path / "bin"
    binv.mkdir()
    claude = binv / "claude"
    claude.write_text("#!/bin/bash\nexit 1\n")
    claude.chmod(0o755)
    timeout = binv / "timeout"
    timeout.write_text('#!/bin/bash\nshift\nexec "$@"\n')
    timeout.chmod(0o755)
    monkeypatch.setenv("PATH", f"{binv}:{os.environ['PATH']}")
    monkeypatch.setenv("MODEL_CALL_BACKEND", "claude")
    monkeypatch.delenv("CLAUDE_SESSION_HOOK", raising=False)
    return binv
