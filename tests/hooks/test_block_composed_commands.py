"""The atomic-execution gate. What counts as one step is `lib/command`'s contract;
this covers only that the gate refuses and allows through it."""

import importlib
import io
import json
import sys


def _run(monkeypatch, command):
    payload = {"tool_name": "Bash", "tool_input": {"command": command}}
    monkeypatch.setattr(sys, "stdin", io.StringIO(json.dumps(payload)))
    return importlib.import_module("block_composed_commands").main()


def test_a_chained_call_is_refused_with_its_operator(monkeypatch, capsys):
    assert _run(monkeypatch, "cd /tmp && uv run pytest -q") == 2
    assert "`&&`" in capsys.readouterr().err


def test_a_single_command_runs(monkeypatch):
    assert _run(monkeypatch, "uv run pytest -q") == 0
