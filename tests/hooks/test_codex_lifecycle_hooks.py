"""Lifecycle hooks use codex records, not Claude's generic background-task list."""

import io
import json
import os
import subprocess
import sys

import rewake_codex_failure
from lib import codex_run, session_state


def _session(monkeypatch, tmp_path, session_id):
    monkeypatch.setenv("CLAUDE_DATA_ROOT", str(tmp_path / "data"))
    assert session_state.cmd_start([session_id]) == 0
    return session_state._session_dir(session_id)


def _record(directory, job, pid, status="running", server_pid=None, ended_at=None):
    record = {
        "job": job, "agent": "architect", "status": status, "phase": "thinking",
        "thread": "thread", "turn": "turn", "pid": pid, "server_pid": server_pid,
        "session": os.path.basename(directory), "started_at": 1, "updated_at": 1,
        "ended_at": ended_at,
    }
    record["record"] = os.path.join(directory, job + ".json")
    codex_run._save_record(record)
    return record


def _event(monkeypatch, session_id):
    monkeypatch.setattr(sys, "stdin", io.StringIO(json.dumps({"session_id": session_id})))


def _sleeper():
    return subprocess.Popen([sys.executable, "-c", "import time; time.sleep(60)"],
                            start_new_session=True)


def _gone(proc):
    proc.wait(timeout=5)
    return proc.poll() is not None


def test_failure_rewake_only_exits_two_for_one_unannounced_failure(monkeypatch, tmp_path, capsys):
    directory = _session(monkeypatch, tmp_path, "session-a")
    session_state.merge_state("session-a", {"current_turn_start": 100})
    _record(directory, "codex-run-failed", 0, status="failed", ended_at=101)
    _record(directory, "codex-run-ok", 0, status="ok")
    _event(monkeypatch, "session-a")
    assert rewake_codex_failure.main() == 2
    assert "codex-run-failed failed" in capsys.readouterr().err
    _event(monkeypatch, "session-a")
    assert rewake_codex_failure.main() == 0
    assert rewake_codex_failure.BINDING["asyncRewake"] is True




def test_failure_rewake_permits_without_failures(monkeypatch, tmp_path):
    _session(monkeypatch, tmp_path, "session-a")
    session_state.merge_state("session-a", {"current_turn_start": 100})
    _event(monkeypatch, "session-a")
    assert rewake_codex_failure.main() == 0








