"""Focused contracts for the tracer enrichment hook."""

import json
import os
import subprocess
import time

import enrich_on_read
import pytest
from conftest import PY_HOOKS
from lib import feedback, tracer

PY = os.path.join(PY_HOOKS, "enrich_on_read.py")
ROOM = str(tracer.room(enrich_on_read.SOURCE))

# Records every argv it receives and answers with what the test sets.
_TRACE_STUB = r'''#!/usr/bin/env python3
import json, os, sys

with open(os.environ["STUB_TRACE_CALLS"], "a") as calls:
    calls.write(json.dumps(sys.argv[1:]) + "\n")
sys.stdout.write(os.environ.get("STUB_TRACE_OUT", "[git: stub shoulder]\n"))
sys.exit(int(os.environ.get("STUB_TRACE_EXIT", "0")))
'''


def _stub_trace(monkeypatch, tmp_path, out=None, exit_code=0):
    bin_directory = tmp_path / "bin"
    bin_directory.mkdir(exist_ok=True)
    trace = bin_directory / "trace"
    trace.write_text(_TRACE_STUB)
    trace.chmod(0o755)
    monkeypatch.setenv("PATH", "%s:%s" % (bin_directory, os.environ["PATH"]))
    calls = tmp_path / "trace-calls.jsonl"
    monkeypatch.setenv("STUB_TRACE_CALLS", str(calls))
    monkeypatch.setenv("STUB_TRACE_EXIT", str(exit_code))
    if out is not None:
        monkeypatch.setenv("STUB_TRACE_OUT", out)
    return calls


def _trace_calls(calls):
    if not calls.exists():
        return []
    return [json.loads(line) for line in calls.read_text().splitlines()]


def _run(payload, env=None, cwd=None):
    result = subprocess.run(
        ["python3", PY], input=json.dumps(payload), text=True, capture_output=True,
        env=env, cwd=cwd)
    return result.returncode, result.stdout.strip(), result.stderr.strip()


def _context(stdout):
    if not stdout:
        return ""
    return json.loads(stdout)["hookSpecificOutput"]["additionalContext"]


def _enrich(payload):
    return_code, out, err = _run(payload)
    return return_code, _context(out), err


@pytest.mark.parametrize("tool_input, expected", [
    ({"file_path": "/repo/event.py"}, ["context", "/repo/event.py", "--budget", ROOM]),
    ({"file_path": "/repo/event.py", "offset": 10, "limit": 5},
     ["context", "/repo/event.py", "--budget", ROOM, "--offset", "10", "--limit", "5"]),
])
def test_read_runs_one_recorded_context_call(monkeypatch, tmp_path, tool_input, expected):
    calls = _stub_trace(monkeypatch, tmp_path)
    return_code, context, _ = _enrich({"tool_name": "Read", "tool_input": tool_input})

    assert return_code == 0
    assert "[git: stub shoulder]" in context
    assert _trace_calls(calls) == [expected]


@pytest.mark.parametrize("tool", ["Edit", "Write"])
def test_an_edit_is_not_recorded_as_a_read(monkeypatch, tmp_path, tool):
    calls = _stub_trace(monkeypatch, tmp_path)
    _enrich({"tool_name": tool, "tool_input": {"file_path": "/repo/event.py"}})

    assert _trace_calls(calls) == [["context", "/repo/event.py", "--budget", ROOM, "--no-record"]]


def test_an_edit_gets_only_the_lines_it_replaces(monkeypatch, tmp_path):
    target = tmp_path / "event.py"
    target.write_text("a = 1\n\ndef first():\n    return 1\n    # end\n\ndef second():\n    return 2\n")
    calls = _stub_trace(monkeypatch, tmp_path)
    _enrich({"tool_name": "Edit", "tool_input": {
        "file_path": str(target), "old_string": "def second():\n    return 2", "new_string": "",
    }})

    assert _trace_calls(calls) == [
        ["context", str(target), "--budget", ROOM, "--no-record", "--offset", "7", "--limit", "2"]]


def test_grep_passes_the_native_grep_settings_to_one_trace_grep(monkeypatch, tmp_path):
    calls = _stub_trace(monkeypatch, tmp_path)
    _enrich({"tool_name": "Grep", "tool_input": {
        "pattern": "-needle", "path": "/repo", "-i": True, "glob": "*.py",
        "type": "py", "multiline": True, "output_mode": "content",
    }})

    assert _trace_calls(calls) == [[
        "grep", "--budget", ROOM, "-i", "-g", "*.py", "-t", "py", "-U", "--", "-needle", "/repo",
    ]]


@pytest.mark.parametrize("path, searched", [(None, "."), ("src", "src"), ("/elsewhere", "/elsewhere")])
def test_grep_names_the_searched_path_relative_to_the_session_directory(monkeypatch, tmp_path, path, searched):
    calls = _stub_trace(monkeypatch, tmp_path)
    tool_input = {"pattern": "def "} if path is None else {"pattern": "def ", "path": (
        str(tmp_path / path) if path == "src" else path)}
    _enrich({"tool_name": "Grep", "tool_input": tool_input, "cwd": str(tmp_path)})

    assert _trace_calls(calls) == [["grep", "--budget", ROOM, "--", "def ", searched]]


def test_glob_runs_one_trace_find(monkeypatch, tmp_path):
    calls = _stub_trace(monkeypatch, tmp_path)
    _enrich({"tool_name": "Glob", "tool_input": {"pattern": "**/*.py", "path": "/repo"}})

    assert _trace_calls(calls) == [["find", "**/*.py", "/repo", "--budget", ROOM]]


@pytest.mark.parametrize("tool, tool_input", [
    ("Grep", {"pattern": "absent"}),
    ("Glob", {"pattern": "*.absent"}),
])
def test_no_matches_injects_nothing(monkeypatch, tmp_path, tool, tool_input):
    _stub_trace(monkeypatch, tmp_path, out="(no matches)\n")
    return_code, out, _ = _run({"tool_name": tool, "tool_input": tool_input, "cwd": str(tmp_path)})

    assert (return_code, out) == (0, "")


def test_failed_read_for_existing_target_reports_trace_failure(monkeypatch, tmp_path):
    target = tmp_path / "event.py"
    target.write_text("event = True\n")
    _stub_trace(monkeypatch, tmp_path, out="", exit_code=1)
    _, context, _ = _enrich({"tool_name": "Read", "tool_input": {"file_path": str(target)}})

    assert "%s\n[trace context unavailable: trace failed]" % target in context


def test_a_partial_answer_with_exit_two_is_still_injected(monkeypatch, tmp_path):
    """Exit 2 names a missing path; a new file's directory still arrives."""
    _stub_trace(monkeypatch, tmp_path, out="---\ndirectory:\n  path: src/\n---\n", exit_code=2)
    _, context, _ = _enrich({"tool_name": "Write", "tool_input": {"file_path": str(tmp_path / "src/new.py")}})

    assert "path: src/" in context


def test_failed_write_for_missing_target_emits_no_context(monkeypatch, tmp_path):
    _stub_trace(monkeypatch, tmp_path, out="", exit_code=1)
    return_code, out, _ = _run({"tool_name": "Write", "tool_input": {"file_path": str(tmp_path / "new.py")}})

    assert (return_code, out) == (0, "")


def test_real_sleeping_trace_reports_the_timeout_before_the_hook_deadline(tmp_path):
    target = tmp_path / "event.py"
    target.write_text("event = True\n")
    bin_directory = tmp_path / "bin"
    bin_directory.mkdir()
    pid_file = tmp_path / "trace.pid"
    trace = bin_directory / "trace"
    trace.write_text("""#!/usr/bin/env python3
import os, time
open(os.environ["SLEEP_TRACE_PID"], "w").write(str(os.getpid()))
time.sleep(60)
""")
    trace.chmod(0o755)
    env = os.environ.copy()
    env["PATH"] = "%s:%s" % (bin_directory, env["PATH"])
    env["SLEEP_TRACE_PID"] = str(pid_file)
    started = time.monotonic()

    result = subprocess.run(
        ["python3", PY],
        input=json.dumps({"tool_name": "Read", "tool_input": {"file_path": str(target)}}),
        text=True, capture_output=True, cwd=tmp_path, env=env,
        timeout=enrich_on_read.BINDING["timeout"],
    )

    assert result.returncode == 0
    assert "[trace context unavailable: enrichment timed out]" in _context(result.stdout)
    assert time.monotonic() - started < enrich_on_read.BINDING["timeout"]
    with pytest.raises(ProcessLookupError):
        os.kill(int(pid_file.read_text()), 0)


@pytest.mark.parametrize("payload", [
    {"tool_name": "Read", "tool_input": {}},
    {"tool_name": "Glob", "tool_input": {}},
    {"tool_name": "Grep", "tool_input": {}},
    {"tool_name": "Bash", "tool_input": {"command": "cat /repo/event.py"}},
    {"tool_name": "WebFetch", "tool_input": {}},
])
def test_degenerate_input_exits_zero_without_output(monkeypatch, tmp_path, payload):
    calls = _stub_trace(monkeypatch, tmp_path)
    return_code, out, _ = _run({**payload, "session_id": "fallback"})

    assert (return_code, out) == (0, "")
    assert _trace_calls(calls) == []


def _real_fixture(tmp_path, files):
    fixture = tmp_path / "fixture"
    fixture.mkdir()
    (fixture / "Claude.md").write_text("# Fixture\n")
    for name, text in files.items():
        path = fixture / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)
    subprocess.run(["git", "init", "-q"], cwd=fixture, check=True)
    subprocess.run(["git", "add", "."], cwd=fixture, check=True)
    subprocess.run(
        ["git", "-c", "user.name=Test", "-c", "user.email=test@example.com", "commit", "-qm", "fixture"],
        cwd=fixture, check=True)
    bin_directory = tmp_path / "bin"
    bin_directory.mkdir()
    (bin_directory / "trace").symlink_to(os.environ["TRACE_BIN"])
    env = os.environ.copy()
    env.pop("TRACE_BIN")
    for inherited in ("AGENT_SESSION_ID", "CLAUDE_CODE_SESSION_ID", "CODEX_THREAD_ID", "TRACER_AGENT_ID"):
        env.pop(inherited, None)
    env["PATH"] = "%s:%s" % (bin_directory, env["PATH"])
    return fixture, env


def _read_fractions(fixture, env, session_id):
    status = subprocess.run(
        ["trace", "docs", "status", "--json"], cwd=fixture,
        env={**env, "AGENT_SESSION_ID": session_id, "TRACER_AGENT_ID": "integration"},
        text=True, capture_output=True, check=True).stdout
    return {entry["path"]: entry["read_fraction"] for entry in json.loads(status)["results"]["loaded"]}


@pytest.mark.skipif(not os.environ.get("TRACE_BIN"), reason="requires explicit measured trace binary")
def test_real_binary_names_every_glob_and_grep_match_in_one_message(tmp_path):
    files = {"src/file-%02d.py" % index: "def value_%02d():\n    return %d\n" % (index, index)
             for index in range(60)}
    fixture, env = _real_fixture(tmp_path, files)
    session_id = "real-hook-%s" % tmp_path

    for tool_input, tool in (({"pattern": "src/*.py", "path": str(fixture)}, "Glob"),
                             ({"pattern": "return", "path": str(fixture)}, "Grep")):
        code, out, error = _run({
            "tool_name": tool, "tool_input": tool_input, "cwd": str(fixture),
            "session_id": session_id, "agent_id": "integration",
        }, env=env, cwd=fixture)
        context = _context(out)

        assert (code, error) == (0, "")
        assert len(context) <= feedback.CONTEXT_LIMIT
        assert all(name in context for name in files), tool

    assert not any(fraction > 0 for fraction in _read_fractions(fixture, env, session_id).values())


@pytest.mark.skipif(not os.environ.get("TRACE_BIN"), reason="requires explicit measured trace binary")
def test_real_binary_records_a_read_but_not_an_edit(tmp_path):
    fixture, env = _real_fixture(tmp_path, {
        "read.py": "read_value = True\n", "edited.py": "edited_value = True\n",
    })
    session_id = "real-record-%s" % tmp_path
    for tool, name in (("Read", "read.py"), ("Edit", "edited.py")):
        code, out, error = _run({
            "tool_name": tool, "tool_input": {"file_path": str(fixture / name)}, "cwd": str(fixture),
            "session_id": session_id, "agent_id": "integration",
        }, env=env, cwd=fixture)
        assert (code, error) == (0, "")
        assert "lines: 1" in _context(out)

    fractions = _read_fractions(fixture, env, session_id)
    assert fractions[str(fixture / "read.py")] == 1.0
    assert fractions.get(str(fixture / "edited.py"), 0) == 0
