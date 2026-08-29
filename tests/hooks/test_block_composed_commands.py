"""The atomic-execution gate. What counts as one step is `lib/command`'s contract;
this covers only that the gate refuses and allows through it."""

import importlib
import io
import json
import sys


PATCH = (
    "*** Begin Patch\n"
    "*** Update File: app/x.py\n"
    "@@\n"
    "-a = 1\n"
    "+a = 2\n"
    "*** End Patch\n"
)


def _run(monkeypatch, command, tool_name="Bash"):
    payload = {"tool_name": tool_name, "tool_input": {"command": command}}
    monkeypatch.setattr(sys, "stdin", io.StringIO(json.dumps(payload)))
    return importlib.import_module("block_composed_commands").main()


def test_a_chained_call_is_refused_with_its_operator(monkeypatch, capsys):
    assert _run(monkeypatch, "cd /tmp && uv run pytest -q") == 2
    assert "`&&`" in capsys.readouterr().err


def test_a_single_command_runs(monkeypatch):
    assert _run(monkeypatch, "uv run pytest -q") == 0


def test_a_codex_patch_is_not_read_as_a_composed_command(monkeypatch):
    assert _run(monkeypatch, PATCH, tool_name="apply_patch") == 0


def test_the_same_text_as_a_shell_call_is_still_refused(monkeypatch):
    assert _run(monkeypatch, PATCH) == 2
