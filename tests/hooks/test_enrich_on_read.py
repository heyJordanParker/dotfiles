"""Focused contracts for the tracer enrichment hook."""

import io
import json
import os
import re
import subprocess
import sys
import time

import enrich_on_read
import pytest
from conftest import PY_HOOKS

PY = os.path.join(PY_HOOKS, "enrich_on_read.py")
GUARD = os.path.join(PY_HOOKS, "guard_trace.py")

# Answers in the one `{query, context, results, counts}` document every trace
# command emits, so a hook that reads the wrong key fails here instead of
# silently enriching nothing in a live session.
_TRACE_STUB = r'''#!/usr/bin/env python3
import json, os, sys

args = sys.argv[1:]
matches = json.loads(os.environ.get("STUB_TRACE_MATCHES", "[]"))
if args and args[0] == "find":
    print(json.dumps({"results": [{"path": m, "kind": "file"} for m in matches]}))
elif args and args[0] == "grep":
    print(json.dumps({"results": [{"file": m, "line": 1} for m in matches]}))
elif args and args[0] == "context":
    calls_path = os.environ.get("STUB_TRACE_CALLS")
    if calls_path:
        with open(calls_path, "a") as calls:
            calls.write(json.dumps(args) + "\n")
    paths = [arg for arg in args[1:] if arg not in ("--no-record", "--json")]
    if "--json" in args:
        unavailable = set(json.loads(os.environ.get("STUB_TRACE_UNAVAILABLE", "[]")))
        results = []
        for path in paths:
            if path in unavailable:
                results.append({"file": path, "content": "", "error": "file not found"})
            else:
                results.append({"file": path, "content": "[git: stub shoulder]", "error": None})
        print(json.dumps({
            "query": {"paths": paths},
            "context": {},
            "results": results,
            "counts": {"files": len(paths), "unavailable": len(unavailable.intersection(paths))},
        }))
    elif len(paths) == 1:
        print("[git: stub shoulder]")
    else:
        print("\n".join("%s\n[git: stub shoulder]" % path for path in paths))
'''


def _stub_trace(monkeypatch, tmp_path, matches=None):
    bin_directory = tmp_path / "bin"
    bin_directory.mkdir(exist_ok=True)
    trace = bin_directory / "trace"
    trace.write_text(_TRACE_STUB)
    trace.chmod(0o755)
    monkeypatch.delenv("TRACE_BIN", raising=False)
    monkeypatch.setenv("PATH", "%s:%s" % (bin_directory, os.environ["PATH"]))
    monkeypatch.setenv("STUB_TRACE_MATCHES", json.dumps(matches or []))
    calls = tmp_path / "trace-calls.jsonl"
    monkeypatch.setenv("STUB_TRACE_CALLS", str(calls))
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


@pytest.mark.skipif(not os.environ.get("TRACE_BIN"), reason="requires explicit measured trace binary")
@pytest.mark.parametrize("linked", [False, True])
def test_glob_from_nested_cwd_anchors_find_rows_at_the_worktree_root(tmp_path, linked):
    repository = tmp_path / "repository"
    requested = repository / "requested"
    cwd = repository / "nested" / "cwd"
    requested.mkdir(parents=True)
    cwd.mkdir(parents=True)
    expected = {requested / "first.py", requested / "second.py"}
    for path in expected:
        path.write_text("value = True\n")
    subprocess.run(["git", "init", "-q"], cwd=repository, check=True)
    subprocess.run(["git", "add", "."], cwd=repository, check=True)
    subprocess.run(
        ["git", "-c", "user.name=Test", "-c", "user.email=test@example.com",
         "commit", "-qm", "fixture"], cwd=repository, check=True)
    if linked:
        checkout = tmp_path / "checkout"
        subprocess.run(["git", "worktree", "add", "--detach", str(checkout)],
                       cwd=repository, check=True, capture_output=True)
        requested = checkout / "requested"
        cwd = checkout / "nested" / "cwd"
        cwd.mkdir(parents=True)
        expected = {requested / "first.py", requested / "second.py"}
    bin_directory = tmp_path / "bin"
    bin_directory.mkdir()
    (bin_directory / "trace").symlink_to(os.environ["TRACE_BIN"])
    env = os.environ.copy()
    env.pop("TRACE_BIN")
    env["PATH"] = "%s:%s" % (bin_directory, env["PATH"])
    env["PWD"] = str(cwd)

    code, out, error = _run({
        "tool_name": "Glob",
        "tool_input": {"pattern": "*.py", "path": str(requested)},
        "session_id": "nested-glob-%s" % tmp_path,
        "agent_id": "integration",
    }, env=env, cwd=cwd)

    assert (code, error) == (0, "")
    context = _context(out)
    assert _enriched_files(context) == {str(path) for path in expected}
    assert _shoulder_count(context) == len(expected)


def test_glob_retains_absolute_match_rows(monkeypatch, tmp_path):
    target = tmp_path / "event.py"
    _stub_trace(monkeypatch, tmp_path, [str(target)])
    return_code, context, _ = _enrich({
        "tool_name": "Glob",
        "tool_input": {"pattern": "*.py", "path": str(tmp_path)},
        "session_id": "absolute-glob",
        "agent_id": "a",
    })

    assert return_code == 0
    assert _enriched_files(context) == {str(target)}


def test_grep_enriches_each_matched_file(monkeypatch, tmp_path):
    matches = ["/repo/event.py", "/repo/feedback.py", "/repo/event.py"]
    _stub_trace(monkeypatch, tmp_path, matches)
    return_code, context, _ = _enrich({
        "tool_name": "Grep",
        "tool_input": {"pattern": "def ", "path": str(tmp_path)},
        "session_id": "grep",
        "agent_id": "a",
    })
    assert return_code == 0
    assert _enriched_files(context) == {"/repo/event.py", "/repo/feedback.py"}
    assert _shoulder_count(context) == 2


def test_a_file_whose_enrichment_times_out_is_still_accounted_for(monkeypatch, capfd):
    deadlines = []

    def timing_out_context(argv, **kwargs):
        if argv[1] == "find":
            return subprocess.CompletedProcess(
                argv, 0,
                '{"results": [{"path": "event.py"}, {"path": "feedback.py"}]}', "")
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
    assert deadlines == [enrich_on_read.TRACE_TIMEOUT,
                         pytest.approx(enrich_on_read.BINDING["timeout"] - 2, abs=0.1)]


def test_match_cap_bounds_enriched_files(monkeypatch, tmp_path):
    _stub_trace(monkeypatch, tmp_path, ["file-%d.py" % index for index in range(25)])
    _, context, _ = _enrich({
        "tool_name": "Glob",
        "tool_input": {"pattern": "*.py", "path": str(tmp_path)},
        "session_id": "cap",
        "agent_id": "a",
    })
    assert len(_enriched_files(context)) == enrich_on_read.MATCH_CAP
    assert "5 additional matched files omitted" in context


def test_match_resolution_spends_the_same_overall_deadline(monkeypatch, capfd):
    timeouts = []
    ticks = iter([0, 5, 6, 7])

    def traced(_trace_bin, args, *_args, **kwargs):
        timeouts.append(kwargs["timeout"])
        if args[0] == "find":
            return '{"results":[{"path":"event.py"}]}'
        return json.dumps({
            "query": {"paths": ["/repo/event.py"]}, "context": {},
            "results": [{"file": "/repo/event.py", "content": "[git: shoulder]", "error": None}],
            "counts": {"files": 1, "unavailable": 0},
        })

    monkeypatch.setattr(enrich_on_read, "resolve_trace_bin", lambda: "trace")
    monkeypatch.setattr(enrich_on_read.time, "monotonic", lambda: next(ticks))
    monkeypatch.setattr(enrich_on_read, "run_trace", traced)
    monkeypatch.setattr(sys, "stdin", io.StringIO(json.dumps({
        "tool_name": "Glob",
        "tool_input": {"pattern": "*.py", "path": "/repo"},
        "session_id": "deadline",
    })))

    assert enrich_on_read.main() == 0
    assert timeouts == [pytest.approx(23), pytest.approx(21)]
    assert _shoulder_count(_context(capfd.readouterr().out.strip())) == 1


def test_failed_early_batch_member_does_not_spend_a_successful_file_slot(monkeypatch):
    missing = "/repo/missing.py"
    successful = ["/repo/file-%d.py" % index for index in range(enrich_on_read.MATCH_CAP)]
    calls = []

    def context(_trace_bin, args, *_args, **_kwargs):
        calls.append(args)
        paths = args[1:-2]
        return json.dumps({
            "query": {"paths": paths},
            "context": {},
            "results": [
                {"file": path, "content": "" if path == missing else "[git: stub shoulder]",
                 "error": "file not found" if path == missing else None}
                for path in paths
            ],
            "counts": {"files": len(paths), "unavailable": int(missing in paths)},
        })

    monkeypatch.setattr(enrich_on_read, "run_trace", context)

    output = enrich_on_read.enrich_matches("trace", [missing, *successful], {})

    assert set(successful).issubset(_enriched_files(output))
    assert "%s\n[trace context unavailable: file not found]" % missing in output
    assert calls[0] == ["context", missing, *successful[:-1], "--no-record", "--json"]


def test_failures_across_batches_leave_later_successful_files_eligible(monkeypatch):
    first_missing = "/repo/missing-first.py"
    second_missing = "/repo/missing-second.py"
    successful = ["/repo/file-%d.py" % index for index in range(enrich_on_read.MATCH_CAP)]
    calls = []

    def context(_trace_bin, args, *_args, **_kwargs):
        calls.append(args)
        paths = args[1:-2]
        missing = {first_missing, second_missing}
        return json.dumps({
            "query": {"paths": paths},
            "context": {},
            "results": [
                {"file": path, "content": "" if path in missing else "[git: stub shoulder]",
                 "error": "file not found" if path in missing else None}
                for path in paths
            ],
            "counts": {"files": len(paths), "unavailable": len(missing.intersection(paths))},
        })

    monkeypatch.setattr(enrich_on_read, "run_trace", context)

    output = enrich_on_read.enrich_matches(
        "trace",
        [first_missing, *successful[:-1], second_missing, successful[-1]],
        {},
    )

    assert set(successful).issubset(_enriched_files(output))
    assert "%s\n[trace context unavailable: file not found]" % first_missing in output
    assert "%s\n[trace context unavailable: file not found]" % second_missing in output
    assert any(second_missing in call for call in calls)


def test_deadline_accounts_for_every_unfinished_batch_path(monkeypatch):
    paths = ["/repo/file-%d.py" % index for index in range(2)]
    ticks = iter([0, enrich_on_read.BINDING["timeout"] + 1])
    calls = []

    monkeypatch.setattr(enrich_on_read.time, "monotonic", lambda: next(ticks))
    monkeypatch.setattr(enrich_on_read, "run_trace", lambda *_args, **_kwargs: calls.append(1))

    output = enrich_on_read.enrich_matches("trace", paths, {})

    assert calls == []
    assert _enriched_files(output) == set(paths)
    assert _accounted(output) == len(paths)


def test_real_sleeping_trace_emits_timeout_accounting_before_outer_deadline(tmp_path):
    bin_directory = tmp_path / "bin"
    bin_directory.mkdir()
    pid_file = tmp_path / "trace.pid"
    trace = bin_directory / "trace"
    trace.write_text("""#!/usr/bin/env python3
import json, os, sys, time
if sys.argv[1] == "find":
    print(json.dumps({"results": [{"path": "file.py"}]}))
else:
    open(os.environ["SLEEP_TRACE_PID"], "w").write(str(os.getpid()))
    time.sleep(60)
""")
    trace.chmod(0o755)
    env = os.environ.copy()
    env["PATH"] = "%s:%s" % (bin_directory, env["PATH"])
    env["PWD"] = str(tmp_path)
    env["SLEEP_TRACE_PID"] = str(pid_file)
    started = time.monotonic()

    result = subprocess.run(
        ["python3", PY],
        input=json.dumps({
            "tool_name": "Glob",
            "tool_input": {"pattern": "*.py", "path": str(tmp_path)},
            "session_id": "sleeping-trace",
            "agent_id": "integration",
        }),
        text=True,
        capture_output=True,
        cwd=tmp_path,
        env=env,
        timeout=enrich_on_read.BINDING["timeout"],
    )

    elapsed = time.monotonic() - started
    assert result.returncode == 0
    assert "[trace context unavailable: enrichment timed out]" in _context(result.stdout)
    assert elapsed < enrich_on_read.BINDING["timeout"]
    pid = int(pid_file.read_text())
    with pytest.raises(ProcessLookupError):
        os.kill(pid, 0)


def test_glob_batches_the_full_context_surface_in_one_no_record_call(monkeypatch, tmp_path):
    matches = ["file-%d.py" % index for index in range(enrich_on_read.MATCH_CAP)]
    calls = _stub_trace(monkeypatch, tmp_path, matches)

    return_code, context, _ = _enrich({
        "tool_name": "Glob",
        "tool_input": {"pattern": "*.py", "path": str(tmp_path)},
        "session_id": "batch",
        "agent_id": "a",
    })

    expected = [str(tmp_path / match) for match in matches]
    assert return_code == 0
    assert _enriched_files(context) == set(expected)
    assert _trace_calls(calls) == [["context", *expected, "--no-record", "--json"]]


def test_batch_json_failures_do_not_spend_successful_file_allowance(monkeypatch, tmp_path):
    missing = ["/repo/missing-first.py", "/repo/missing-second.py"]
    successful = ["/repo/file-%d.py" % index for index in range(enrich_on_read.MATCH_CAP)]
    calls = _stub_trace(monkeypatch, tmp_path)
    monkeypatch.setenv("STUB_TRACE_UNAVAILABLE", json.dumps(missing))

    output = enrich_on_read.enrich_matches(
        "trace", [missing[0], *successful[:-1], missing[1], successful[-1]], os.environ.copy()
    )

    assert set(successful).issubset(_enriched_files(output))
    assert output.count("[trace context unavailable: file not found]") == len(missing)
    assert _trace_calls(calls) == [
        ["context", missing[0], *successful[:-1], "--no-record", "--json"],
        ["context", missing[1], "--no-record", "--json"],
        ["context", successful[-1], "--no-record", "--json"],
    ]


@pytest.mark.skipif(not os.environ.get("TRACE_BIN"), reason="requires explicit measured trace binary")
def test_refused_shell_read_enriches_without_recording_coverage(tmp_path):
    fixture = tmp_path / "fixture"
    fixture.mkdir()
    target = fixture / "read-target.py"
    target.write_text("value = True\n")
    trace_target = fixture / "trace-target.py"
    trace_target.write_text("trace_value = True\n")
    native_target = fixture / "native-target.py"
    native_target.write_text("native_value = True\n")
    (fixture / "Claude.md").write_text("# Fixture\n")
    subprocess.run(["git", "init", "-q"], cwd=fixture, check=True)
    subprocess.run(["git", "add", "."], cwd=fixture, check=True)
    subprocess.run(
        ["git", "-c", "user.name=Test", "-c", "user.email=test@example.com",
         "commit", "-qm", "fixture"], cwd=fixture, check=True)

    bin_directory = tmp_path / "bin"
    bin_directory.mkdir()
    (bin_directory / "trace").symlink_to(os.environ["TRACE_BIN"])
    env = os.environ.copy()
    env.pop("TRACE_BIN")
    for inherited in ("AGENT_SESSION_ID", "CLAUDE_CODE_SESSION_ID", "CODEX_THREAD_ID",
                      "TRACER_AGENT_ID"):
        env.pop(inherited, None)
    env["PATH"] = "%s:%s" % (bin_directory, env["PATH"])
    env["PWD"] = str(fixture)
    payload = {
        "tool_name": "Bash",
        "tool_input": {"command": "cat read-target.py"},
        "cwd": str(fixture),
        "session_id": "refused-read-%s" % tmp_path,
        "agent_id": "integration",
    }
    identity = {
        **env,
        "AGENT_SESSION_ID": payload["session_id"],
        "TRACER_AGENT_ID": payload["agent_id"],
    }

    guard = subprocess.run(
        ["python3", GUARD], input=json.dumps(payload), text=True, capture_output=True,
        cwd=fixture, env=env)
    enrich_code, enrich_out, enrich_error = _run(payload, env=env, cwd=fixture)
    status = json.loads(subprocess.run(
        ["trace", "docs", "status", "--json"], cwd=fixture, env=identity,
        text=True, capture_output=True, check=True).stdout)

    assert guard.returncode == 2
    assert "BLOCKED: don't filter trace" in guard.stderr
    assert (enrich_code, enrich_error) == (0, "")
    context = _context(enrich_out)
    assert "presence:" in context
    assert "loc:" in context
    assert "ccn:" in context
    assert not any(
        entry["path"] == str(target) and entry["read_fraction"] > 0
        for entry in status["results"]["loaded"]
    )

    subprocess.run(
        ["trace", "read", str(trace_target), "--lines", "1:1"], cwd=fixture,
        env=identity, text=True, capture_output=True, check=True)
    native_code, _, native_error = _run({
        "tool_name": "Read",
        "tool_input": {"file_path": str(native_target), "offset": 1, "limit": 1},
        "session_id": payload["session_id"],
        "agent_id": payload["agent_id"],
    }, env=env, cwd=fixture)
    positive_status = json.loads(subprocess.run(
        ["trace", "docs", "status", "--json"], cwd=fixture, env=identity,
        text=True, capture_output=True, check=True).stdout)
    fractions = {
        entry["path"]: entry["read_fraction"]
        for entry in positive_status["results"]["loaded"]
    }
    assert (native_code, native_error) == (0, "")
    assert fractions[str(trace_target)] == 1.0
    assert fractions[str(native_target)] == 1.0


@pytest.mark.skipif(not os.environ.get("TRACE_BIN"), reason="requires explicit measured trace binary")
def test_real_binary_batches_twenty_full_context_shoulders_without_recording_reads(tmp_path):
    fixture = tmp_path / "fixture"
    fixture.mkdir()
    (fixture / "Claude.md").write_text("# Fixture\n\nTwenty known Python files.\n")
    control = fixture / "control"
    control.mkdir()
    recorded = control / "recorded.txt"
    recorded.write_text("recorded = True\n")
    for index in range(enrich_on_read.MATCH_CAP):
        (fixture / ("file-%02d.py" % index)).write_text("def value_%02d():\n    return %d\n" % (index, index))
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
    env["PWD"] = str(fixture)
    payload = {
        "tool_name": "Glob",
        "tool_input": {"pattern": "*.py", "path": str(fixture)},
        "session_id": "real-batch-hook-%s" % tmp_path,
        "agent_id": "integration",
    }
    identity = {
        **env,
        "AGENT_SESSION_ID": payload["session_id"],
        "TRACER_AGENT_ID": payload["agent_id"],
    }

    subprocess.run(["trace", "context", str(recorded)], cwd=fixture, env=identity,
                   text=True, capture_output=True, check=True)
    before = subprocess.run(
        ["trace", "docs", "status", "--json"], cwd=fixture, env=identity,
        text=True, capture_output=True, check=True).stdout
    first_code, first_out, first_error = _run(payload, env=env, cwd=fixture)
    repeated_code, repeated_out, repeated_error = _run(payload, env=env, cwd=fixture)
    first_context, repeated_context = _context(first_out.strip()), _context(repeated_out.strip())
    after = subprocess.run(
        ["trace", "docs", "status", "--json"], cwd=fixture, env=identity,
        text=True, capture_output=True, check=True).stdout

    assert (first_code, first_error, repeated_code, repeated_error) == (0, "", 0, "")
    expected_files = {str(fixture / ("file-%02d.py" % index)) for index in range(enrich_on_read.MATCH_CAP)}
    first_files = _enriched_files(first_context)
    repeated_files = _enriched_files(repeated_context)
    assert first_files == expected_files
    assert repeated_files == first_files
    assert _shoulder_count(first_context) == enrich_on_read.MATCH_CAP
    assert _shoulder_count(repeated_context) == enrich_on_read.MATCH_CAP
    assert first_context.count("<enrich_on_read_agent>") == 1
    assert repeated_context.count("<enrich_on_read_agent>") == 1
    for field_name in ("presence:", "loc:", "ccn:", "[docs:", "[symbols:"):
        assert first_context.count(field_name) == enrich_on_read.MATCH_CAP
        assert repeated_context.count(field_name) == enrich_on_read.MATCH_CAP
    assert first_context.count("not loaded: Claude.md") == enrich_on_read.MATCH_CAP
    assert repeated_context.count("not loaded: Claude.md") == enrich_on_read.MATCH_CAP
    assert first_context.count("[dir ") == 1
    assert repeated_context.count("[dir ") == 0
    before_loaded = json.loads(before)["results"]["loaded"]
    after_loaded = json.loads(after)["results"]["loaded"]
    recorded_entry = next(entry for entry in before_loaded if entry["path"] == str(recorded))
    recorded_after = next(entry for entry in after_loaded if entry["path"] == str(recorded))
    assert recorded_entry["read_fraction"] == 1.0
    assert recorded_after["read_fraction"] == recorded_entry["read_fraction"]
    assert not any(entry["path"] in expected_files and entry["read_fraction"] > 0 for entry in after_loaded)


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


@pytest.mark.parametrize("command", [
    "head -n 5 /repo/event.py",
    "sed -n 2,4p /repo/event.py",
    "sed -n s/x/y/ /repo/event.py",
    "sed -e s/x/y/ /repo/event.py",
    "tail -n 5 /repo/event.py",
])
def test_codex_bash_reads_emit_no_record_context_without_fabricated_spans(
    monkeypatch, tmp_path, command
):
    calls = _stub_trace(monkeypatch, tmp_path)
    return_code, context, _ = _enrich({
        "tool_name": "Bash",
        "tool_input": {"command": command},
        "session_id": "codex-read-no-record",
        "agent_id": "a",
    })

    assert return_code == 0
    assert _shoulder_count(context) == 1
    assert _trace_calls(calls) == [["context", "/repo/event.py", "--no-record"]]


@pytest.mark.parametrize("parts", [
    ["echo", "/repo/event.py"],
    ["cat", "*.py"],
    ["head", "-n"],
])
def test_shell_read_target_rejects_non_reads_globs_and_incomplete_options(parts):
    assert enrich_on_read.read_target(parts) == ""


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
