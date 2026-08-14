"""The Claude Bash gate for trackable codex-run launches."""

import importlib
import io
import json
import sys

import pytest


def _run(monkeypatch, command, run_in_background=None, is_subagent=False):
    payload = {"tool_name": "Bash", "tool_input": {"command": command}}
    if run_in_background is not None:
        payload["tool_input"]["run_in_background"] = run_in_background
    if is_subagent:
        payload["is_sidechain"] = True
    monkeypatch.setattr(sys, "stdin", io.StringIO(json.dumps(payload)))
    return importlib.import_module("enforce_background_codex_run").main()


@pytest.mark.parametrize(
    "command, message",
    [
        ('codex-run @ponytail "x"', "foreground"),
    ],
)
def test_blocks_untracked_runs(monkeypatch, capsys, command, message):
    assert _run(monkeypatch, command) == 2
    output = capsys.readouterr().err
    assert message in output
    assert "run_in_background: true" in output


def test_allows_a_harness_backgrounded_run(monkeypatch):
    assert _run(monkeypatch, 'codex-run @ponytail "x"', run_in_background=True) == 0





