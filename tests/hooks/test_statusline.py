"""The codex rows appended to Claude's statusline."""

import json
import os
import re
import shutil
import subprocess
from collections import Counter

import pytest
from conftest import REPO

SCRIPT = os.path.join(REPO, "packages", "claude", "statusline-command.sh")
SID = "statusline-session"
ANSI = re.compile(r"\x1b\[[0-9;]*m")
NOW = 10_000_000


@pytest.fixture
def statusline(tmp_path, monkeypatch):
    """Run the statusline against one isolated session directory."""
    root = tmp_path / "claude"
    session = root / "sessions" / SID
    session.mkdir(parents=True)
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()
    date = bin_dir / "date"
    date.write_text(f"#!/bin/bash\nprintf '%s\\n' {NOW}\n")
    date.chmod(0o755)
    monkeypatch.setenv("CLAUDE_DATA_ROOT", str(root))
    monkeypatch.setenv("PATH", f"{bin_dir}:{os.environ['PATH']}")

    def write(name, contents):
        (session / name).write_text(contents if isinstance(contents, str) else json.dumps(contents))

    def run(columns=None):
        env = dict(os.environ)
        env["CLAUDE_DATA_ROOT"] = str(root)
        env["PATH"] = f"{bin_dir}:{os.environ['PATH']}"
        if columns is not None:
            env["COLUMNS"] = str(columns)
        else:
            env.pop("COLUMNS", None)
        result = subprocess.run(
            ["bash", os.environ.get("STATUSLINE_COMMAND", SCRIPT)],
            input=json.dumps({
                "workspace": {"current_dir": str(tmp_path)},
                "model": {"display_name": None},
                "output_style": {"name": "default"},
                "context_window": {"current_usage": None},
                "session_id": SID,
            }),
            text=True,
            capture_output=True,
            env=env,
        )
        return result, [ANSI.sub("", line) for line in result.stdout.splitlines()[2:]]

    return write, run


def _job(**fields):
    return {
        "session": SID,
        "status": "running",
        "agent": "architect",
        "pid": os.getpid(),
        "started_at": NOW - 59,
        "updated_at": NOW,
        "activity": "reading architecture",
        **fields,
    }


@pytest.mark.parametrize("contents", ["", '{"session":', "not json"])
def test_unreadable_records_never_blank_claudes_statusline(statusline, contents):
    write, run = statusline
    write("codex-run-broken.json", contents)

    result, rows = run()

    assert result.returncode == 0
    assert rows == []


def test_no_running_jobs_adds_no_blank_codex_row(statusline):
    _, run = statusline

    result, rows = run()

    assert result.returncode == 0
    assert rows == []
    assert not result.stdout.endswith("\n\n")


def test_rows_fill_claudes_available_width_and_keep_activity_before_the_trailer(statusline):
    write, run = statusline
    write("codex-run-architect.json", _job(agent="architect", activity="reading the long architecture document"))
    write("codex-run-builder.json", _job(agent="builder", activity="implementing the narrow test case"))

    for columns in (42, 80):
        result, rows = run(columns)

        assert result.returncode == 0
        assert len(rows) == 2
        assert all(len(row) == columns - 4 for row in rows)
        for row in rows:
            before_trailer, trailer = row.rstrip().rsplit(" ", 1)
            assert trailer == "59s"
            assert before_trailer.rsplit("(codex)", 1)[1].strip()


@pytest.mark.parametrize("job", [_job(pid=999_999_999), _job(pid=None)])
def test_dead_or_pidless_runners_remain_visible_as_dead(statusline, job):
    write, run = statusline
    write("codex-run-dead.json", job)

    result, rows = run(80)

    assert result.returncode == 0
    assert len(rows) == 1
    assert "failed — runner dead" in rows[0]


def test_stale_jobs_show_their_idle_duration(statusline):
    write, run = statusline
    write("codex-run-idle.json", _job(updated_at=NOW - 90))

    result, rows = run(80)

    assert result.returncode == 0
    assert "idle 1m 30s" in rows[0]


def test_only_the_current_sessions_records_render(statusline):
    write, run = statusline
    write("codex-run-own.json", _job(agent="mine"))
    write("codex-run-foreign.json", _job(session="another-session", agent="not-mine"))

    result, rows = run(80)

    assert result.returncode == 0
    assert len(rows) == 1
    assert "mine (codex)" in rows[0]
    assert "not-mine" not in rows[0]


@pytest.mark.parametrize(("elapsed", "label"), [
    (59, "59s"), (1609, "26m 49s"), (4392, "1h 13m 12s"), (93780, "1d 2h 3m"),
])
def test_elapsed_matches_claudes_boundary_format(statusline, elapsed, label):
    write, run = statusline
    write("codex-run-elapsed.json", _job(started_at=NOW - elapsed))

    result, rows = run()

    assert result.returncode == 0
    assert rows[0].endswith(label)


def test_long_activity_truncates_from_its_end_inside_the_available_width(statusline):
    write, run = statusline
    write("codex-run-long.json", _job(agent="builder", activity="inspect architecture carefully without stopping"))

    result, rows = run(42)

    assert result.returncode == 0
    assert len(rows) == 1
    assert len(rows[0]) == 38
    assert "inspect…" in rows[0]
    assert "architecture" not in rows[0]


def test_spawn_count_stays_constant_across_record_history(statusline, tmp_path, monkeypatch):
    write, run = statusline
    spawn_log = tmp_path / "spawns.log"
    monkeypatch.setenv("STATUSLINE_SPAWN_LOG", str(spawn_log))
    for command in ("jq", "awk"):
        binary = shutil.which(command)
        wrapper = tmp_path / "bin" / command
        wrapper.write_text(
            f"#!/bin/bash\nprintf '%s\\n' {command} >> \"$STATUSLINE_SPAWN_LOG\"\nexec {binary} \"$@\"\n"
        )
        wrapper.chmod(0o755)

    for number in range(2):
        write(f"codex-run-live-{number}.json", _job(fresh_input_tokens=1_000))
    result, _ = run()
    assert result.returncode == 0
    two_records = Counter(spawn_log.read_text().splitlines())

    spawn_log.write_text("")
    for number in range(2, 200):
        write(f"codex-run-history-{number}.json", _job(status="completed"))
    result, _ = run()
    assert result.returncode == 0
    two_hundred_records = Counter(spawn_log.read_text().splitlines())

    assert two_records == two_hundred_records == {"jq": 7, "awk": 2}
