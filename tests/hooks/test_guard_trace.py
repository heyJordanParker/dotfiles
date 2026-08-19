"""The trace-routing gate. Raw file reads and listings on repo paths are
refused toward the matching trace subcommand; paths outside the repo pass."""

import importlib
import io
import json
import sys


def _run(monkeypatch, command, cwd):
    payload = {
        "tool_name": "Bash",
        "tool_input": {"command": command},
        "cwd": cwd,
    }
    monkeypatch.setattr(sys, "stdin", io.StringIO(json.dumps(payload)))
    return importlib.import_module("guard_trace").main()


def test_bare_ls_is_refused_as_a_repo_listing(monkeypatch, capsys, tmp_path):
    assert _run(monkeypatch, "ls", str(tmp_path)) == 2
    assert "trace list" in capsys.readouterr().err


def test_ls_with_flags_only_is_refused(monkeypatch, tmp_path):
    assert _run(monkeypatch, "ls -la", str(tmp_path)) == 2


def test_ls_on_a_repo_directory_is_refused(monkeypatch, tmp_path):
    (tmp_path / "src").mkdir()
    assert _run(monkeypatch, "ls src", str(tmp_path)) == 2


def test_tree_is_refused_toward_trace_tree(monkeypatch, capsys, tmp_path):
    assert _run(monkeypatch, "tree", str(tmp_path)) == 2
    assert "trace tree" in capsys.readouterr().err


def test_ls_outside_the_repo_passes(monkeypatch, tmp_path, tmp_path_factory):
    outside = tmp_path_factory.mktemp("outside")
    assert _run(monkeypatch, f"ls {outside}", str(tmp_path)) == 0


def test_ls_on_an_unresolvable_outside_glob_passes(monkeypatch, tmp_path):
    assert _run(monkeypatch, "ls -d /somewhere/else/prefix-*", str(tmp_path)) == 0


def test_cat_on_a_repo_file_is_still_refused(monkeypatch, tmp_path):
    (tmp_path / "app.py").write_text("x = 1\n")
    assert _run(monkeypatch, "cat app.py", str(tmp_path)) == 2


def test_plain_trace_passes(monkeypatch, tmp_path):
    assert _run(monkeypatch, "trace grep pattern", str(tmp_path)) == 0
