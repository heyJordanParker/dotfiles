"""A search with no bound on its output is refused when piped into sort."""

import importlib
import io
import json
import sys


def _run(monkeypatch, command):
    payload = {"tool_name": "Bash", "tool_input": {"command": command}}
    monkeypatch.setattr(sys, "stdin", io.StringIO(json.dumps(payload)))
    return importlib.import_module("block_unbounded_sort").main()


def test_unbounded_search_into_sort_is_refused(monkeypatch, capsys):
    assert _run(monkeypatch, "rg -o pattern /Users/nobody/corpus | sort") == 2
    assert "rg -c" in capsys.readouterr().err


def test_counted_search_into_sort_passes(monkeypatch):
    assert _run(monkeypatch, "rg -c pattern /Users/nobody/corpus | sort") == 0


def test_search_bounded_by_head_before_sort_passes(monkeypatch):
    assert _run(monkeypatch, "rg -o pattern /Users/nobody/corpus | head -200 | sort") == 0
