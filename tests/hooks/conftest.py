"""Shared fixtures for the hook tests.

Runtime code is on sys.path via pyproject's `pythonpath = ["packages/agents/hooks", ...]`,
so `from lib import transcript` and `import block_git_revert` resolve with no install.
"""

import json
import os

import pytest

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
PY_HOOKS = os.path.join(REPO, "packages", "agents", "hooks")


@pytest.fixture(autouse=True)
def clean_hook_environment(monkeypatch):
    """Keep hook subprocesses independent of the harness that ran pytest."""
    monkeypatch.delenv("CODEX_RUN_AGENT_FILE", raising=False)


@pytest.fixture
def write_transcript(tmp_path):
    """Write records (list of dicts) as a JSONL transcript; return its path."""
    def _write(records, name="transcript.jsonl"):
        # Compact, like Claude Code's real transcripts.
        path = tmp_path / name
        path.write_text("\n".join(json.dumps(r, separators=(",", ":")) for r in records) + "\n")
        return str(path)
    return _write
