"""The Claude Bash gate for trackable codex-run launches."""

import importlib
import io
import json
import sys

import pytest
from lib.codex_run import _ONE_JOB


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
        ('codex-run @ponytail "x" &', "shell-backgrounded"),
        ('codex-run @ponytail - > /tmp/x.log 2>&1 &', "shell-backgrounded"),
        ('codex-run resume job "msg"', "foreground"),
    ],
)
def test_blocks_untracked_runs(monkeypatch, capsys, command, message):
    assert _run(monkeypatch, command) == 2
    output = capsys.readouterr().err
    assert message in output
    assert "run_in_background: true" in output


def test_allows_a_harness_backgrounded_run(monkeypatch):
    assert _run(monkeypatch, 'codex-run @ponytail "x"', run_in_background=True) == 0


@pytest.mark.parametrize(
    "command",
    ["codex-run status", "codex-run watch"]
    + ["codex-run %s job" % name for name in _ONE_JOB],
)
def test_allows_foreground_read_back_commands(monkeypatch, command):
    assert _run(monkeypatch, command) == 0


def test_allows_a_subagent_foreground_run(monkeypatch):
    assert _run(monkeypatch, 'codex-run @ponytail "x"', is_subagent=True) == 0


def test_allows_a_mention_without_a_run(monkeypatch):
    assert _run(monkeypatch, "echo codex-run") == 0
