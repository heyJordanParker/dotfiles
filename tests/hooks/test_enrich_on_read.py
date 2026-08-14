"""Focused contracts for the tracer enrichment hook."""

import io
import json
import os
import re
import subprocess
import sys

import enrich_on_read
import pytest
from conftest import PY_HOOKS

PY = os.path.join(PY_HOOKS, "enrich_on_read.py")

_TRACE_STUB = r'''#!/usr/bin/env python3
import json, os, sys

args = sys.argv[1:]
if args and args[0] == "glob":
    print(json.dumps({"matches": json.loads(os.environ.get("STUB_TRACE_MATCHES", "[]"))}))
elif args and args[0] == "context":
    print("[git: stub shoulder]")
'''


def _stub_trace(monkeypatch, tmp_path, matches=None):
    bin_directory = tmp_path / "bin"
    bin_directory.mkdir(exist_ok=True)
    trace = bin_directory / "trace"
    trace.write_text(_TRACE_STUB)
    trace.chmod(0o755)
    monkeypatch.setenv("PATH", "%s:%s" % (bin_directory, os.environ["PATH"]))
    monkeypatch.setenv("STUB_TRACE_MATCHES", json.dumps(matches or []))


def _run(payload):
    result = subprocess.run(
        ["python3", PY], input=json.dumps(payload), text=True, capture_output=True)
    return result.returncode, result.stdout.strip(), result.stderr.strip()


def _context(stdout):
    if not stdout:
        return ""
    return json.loads(stdout)["hookSpecificOutput"]["additionalContext"]


def _enriched_files(context):
    return set(re.findall(r"^(/\S+)$", context, re.M))


def _shoulder_count(context):
    return len(re.findall(r"^\[git:", context, re.M))


def _accounted(context):
    return _shoulder_count(context) + context.count("[trace context unavailable")


def _enrich(payload):
    return_code, out, err = _run(payload)
    return return_code, _context(out), err


def test_single_file_read_emits_one_headerless_shoulder(monkeypatch, tmp_path):
    _stub_trace(monkeypatch, tmp_path)
    return_code, context, _ = _enrich({
        "tool_name": "Read",
        "tool_input": {"file_path": "/repo/event.py"},
        "session_id": "single-read",
        "agent_id": "a",
    })
    assert return_code == 0
    assert _shoulder_count(context) == 1
    assert _enriched_files(context) == set()


def test_glob_enriches_each_matched_file(monkeypatch, tmp_path):
    matches = ["event.py", "feedback.py"]
    _stub_trace(monkeypatch, tmp_path, matches)
    return_code, context, _ = _enrich({
        "tool_name": "Glob",
        "tool_input": {"pattern": "*.py", "path": str(tmp_path)},
        "session_id": "glob",
        "agent_id": "a",
    })
    expected = {str(tmp_path / match) for match in matches}
    assert return_code == 0
    assert _enriched_files(context) == expected
    assert _shoulder_count(context) == len(expected)


def test_a_file_whose_enrichment_times_out_is_still_accounted_for(monkeypatch, capfd):
    deadlines = []

    def timing_out_context(argv, **kwargs):
        if argv[1] == "glob":
            return subprocess.CompletedProcess(
                argv, 0, '{"matches": ["event.py", "feedback.py"]}', "")
        deadlines.append(kwargs.get("timeout"))
        raise subprocess.TimeoutExpired(argv, kwargs.get("timeout"))

    monkeypatch.setattr(enrich_on_read, "resolve_trace_bin", lambda: "trace")
    monkeypatch.setattr(enrich_on_read.subprocess, "run", timing_out_context)
    monkeypatch.setattr(sys, "stdin", io.StringIO(json.dumps({
        "tool_name": "Glob",
        "tool_input": {"pattern": "*.py", "path": "/repo"},
        "session_id": "glob-timeout",
        "agent_id": "a",
    })))

    assert enrich_on_read.main() == 0
    context = _context(capfd.readouterr().out.strip())
    assert _enriched_files(context) == {"/repo/event.py", "/repo/feedback.py"}
    assert _accounted(context) == 2
    assert deadlines == [enrich_on_read.TRACE_TIMEOUT] * 2


def test_match_cap_bounds_enriched_files(monkeypatch, tmp_path):
    _stub_trace(monkeypatch, tmp_path, ["file-%d.py" % index for index in range(25)])
    _, context, _ = _enrich({
        "tool_name": "Glob",
        "tool_input": {"pattern": "*.py", "path": str(tmp_path)},
        "session_id": "cap",
        "agent_id": "a",
    })
    assert len(_enriched_files(context)) == enrich_on_read.MATCH_CAP


def test_codex_bash_read_branch_fires(monkeypatch, tmp_path):
    _stub_trace(monkeypatch, tmp_path)
    return_code, context, _ = _enrich({
        "tool_name": "Bash",
        "tool_input": {"command": "cat /repo/event.py"},
        "session_id": "codex-read",
        "agent_id": "a",
    })
    assert return_code == 0
    assert _shoulder_count(context) == 1


@pytest.mark.parametrize("payload", [
    {"tool_name": "Read", "tool_input": {}},
    {"tool_name": "Glob", "tool_input": {}},
    {"tool_name": "Grep", "tool_input": {}},
    {"tool_name": "WebFetch", "tool_input": {}},
])
def test_degenerate_input_exits_zero_without_output(monkeypatch, tmp_path, payload):
    _stub_trace(monkeypatch, tmp_path)
    return_code, out, _ = _run({**payload, "session_id": "fallback"})
    assert return_code == 0
    assert out == ""
