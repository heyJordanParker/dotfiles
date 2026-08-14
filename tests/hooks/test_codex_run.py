"""Terminal, persistence, cancellation, concurrency, and process contracts for codex-run."""

import builtins
import json
import os
import subprocess
import sys
import threading
import time

from conftest import PY_HOOKS

sys.path.insert(0, os.path.join(PY_HOOKS, "lib"))

from lib import codex_run  # noqa: E402

_STUB = r'''#!/usr/bin/env python3
import json, os, sys

if len(sys.argv) < 2 or sys.argv[1] != "app-server":
    sys.exit("stub codex: unexpected argv %r" % (sys.argv[1:],))

script = json.loads(os.environ.get("STUB_SCRIPT") or "{}")
log = open(os.environ["STUB_LOG"], "a")
if script.get("stderr"):
    sys.stderr.write(script["stderr"])
    sys.stderr.flush()

thread = script.get("thread", "th_stub")


def send(obj):
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()


def notify(method, params):
    send({"method": method, "params": params})


for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    message = json.loads(line)
    log.write(line + "\n")
    log.flush()
    method, rid = message.get("method"), message.get("id")
    params = message.get("params") or {}
    if method == "initialize":
        send({"id": rid, "result": {"userAgent": "stub"}})
    elif method == "thread/start":
        send({"id": rid, "result": {"thread": {
            "id": thread, "path": script.get("rollout", "/tmp/rollout-stub.jsonl")}}})
    elif method == "thread/resume":
        thread = params.get("threadId") or thread
        send({"id": rid, "result": {"thread": {
            "id": thread, "path": script.get("rollout", "/tmp/rollout-stub.jsonl")}}})
    elif method == "turn/start":
        send({"id": rid, "result": {"turn": {"id": "turn_1", "status": "inProgress"}}})
        if script.get("hang"):
            continue
        notify("turn/started", {"threadId": thread, "turn": {
            "id": "turn_1", "status": "inProgress"}})
        for item in script.get("items", []):
            notify("item/started", {"threadId": thread, "turnId": "turn_1", "item": item})
            notify("item/completed", {"threadId": thread, "turnId": "turn_1", "item": item})
        if script.get("error"):
            notify("error", {"threadId": thread, "error": {"message": script["error"]}})
        notify("turn/completed", {"threadId": thread, "turn": {
            "id": "turn_1", "status": script.get("turn_status", "completed")}})
    elif method == "turn/interrupt":
        send({"id": rid, "result": {}})
        notify("turn/completed", {"threadId": thread, "turn": {
            "id": "turn_1", "status": "aborted"}})
'''


def _message(text):
    return {"type": "agentMessage", "phase": "final_answer", "text": text}


def _stub_codex(monkeypatch, tmp_path, **script):
    bin_directory = tmp_path / "bin"
    bin_directory.mkdir(exist_ok=True)
    stub = bin_directory / "codex"
    stub.write_text(_STUB)
    stub.chmod(0o755)
    log = tmp_path / "requests.jsonl"
    script.setdefault("items", [_message("the answer")])
    monkeypatch.setenv("PATH", "%s:%s" % (bin_directory, os.environ["PATH"]))
    monkeypatch.setenv("STUB_LOG", str(log))
    monkeypatch.setenv("STUB_SCRIPT", json.dumps(script))
    monkeypatch.setenv("CLAUDE_DATA_ROOT", str(tmp_path / "data"))
    monkeypatch.setattr(codex_run, "_resolve_output_dir", lambda: str(tmp_path))
    return log


def _requests(log):
    return [json.loads(line) for line in open(log).read().splitlines()]


def _sent(log, method):
    return [request["params"] for request in _requests(log)
            if request.get("method") == method][-1]


def _pin_agents(monkeypatch, tmp_path, names):
    agents = tmp_path / "agents"
    agents.mkdir(exist_ok=True)
    for name in names:
        (agents / (name + ".prompt.md")).write_text("instructions for %s" % name)
    monkeypatch.setattr(codex_run, "AGENTS_DIR", str(agents))
    monkeypatch.setenv("CLAUDE_CONFIG_DIR", str(tmp_path / "empty-root"))
    return agents


def _record(tmp_path):
    path = next(tmp_path.glob("codex-run-*.json"))
    return json.loads(path.read_text())


def _feed(tmp_path):
    return (tmp_path / codex_run._FEED).read_text().splitlines()


def _founding_run(monkeypatch, tmp_path):
    _pin_agents(monkeypatch, tmp_path, ["researcher"])
    _stub_codex(monkeypatch, tmp_path, thread="th_founded")
    assert codex_run.main(["@researcher", "start"]) == 0
    return _record(tmp_path)["job"]


def _driver(tmp_path, argv, agents, log, script, feed_interval=None):
    driver = tmp_path / ("driver-%d.py" % time.time_ns())
    driver.write_text(
        "import sys\n"
        "sys.path.insert(0, %r)\n"
        "sys.path.insert(0, %r)\n"
        "import codex_run\n"
        "codex_run.AGENTS_DIR = %r\n"
        "codex_run._resolve_output_dir = lambda: %r\n"
        "%s"
        "sys.exit(codex_run.main(%r))\n"
        % (os.path.join(PY_HOOKS, "lib"), PY_HOOKS, str(agents), str(tmp_path),
           "codex_run._FEED_INTERVAL = %r\n" % feed_interval
           if feed_interval is not None else "", argv))
    env = dict(os.environ,
               PATH="%s:%s" % (tmp_path / "bin", os.environ["PATH"]),
               CLAUDE_CONFIG_DIR=str(tmp_path / "empty-root"),
               CLAUDE_DATA_ROOT=str(tmp_path / "data"),
               STUB_LOG=str(log), STUB_SCRIPT=json.dumps(script))
    env.pop("CODEX_RUN_AGENT_FILE", None)
    return subprocess.Popen([sys.executable, str(driver)], env=env,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)


def _wait_for_record(tmp_path, status, seconds):
    deadline = time.time() + seconds
    while time.time() < deadline:
        for path in tmp_path.glob("codex-run-*.json"):
            try:
                record = json.loads(path.read_text())
            except (OSError, ValueError):
                continue
            if record.get("status") == status and record.get("turn"):
                return record
        time.sleep(0.02)
    raise AssertionError("no %s job record appeared under %s" % (status, tmp_path))


def test_answer_is_ok(monkeypatch, tmp_path, capsys):
    _pin_agents(monkeypatch, tmp_path, ["architect"])
    _stub_codex(monkeypatch, tmp_path)
    assert codex_run.main(["@architect", "do x"]) == 0
    out = capsys.readouterr().out
    assert "the answer" in out
    assert "status:  ok" in out


def test_no_answer_is_failure(monkeypatch, tmp_path, capsys):
    _pin_agents(monkeypatch, tmp_path, ["architect"])
    _stub_codex(monkeypatch, tmp_path, items=[])
    assert codex_run.main(["@architect", "do x"]) == 1
    assert "status:  failed" in capsys.readouterr().out
    assert _record(tmp_path)["error"] == "turn completed but produced no message"


def test_failed_turn_status_is_failure(monkeypatch, tmp_path, capsys):
    _pin_agents(monkeypatch, tmp_path, ["architect"])
    _stub_codex(monkeypatch, tmp_path, turn_status="failed")
    assert codex_run.main(["@architect", "do x"]) == 1
    assert "status:  failed" in capsys.readouterr().out


def test_error_notification_is_failure(monkeypatch, tmp_path, capsys):
    _pin_agents(monkeypatch, tmp_path, ["architect"])
    _stub_codex(monkeypatch, tmp_path, error="model request failed: 401 unauthorized")
    assert codex_run.main(["@architect", "do x"]) == 1
    out = capsys.readouterr().out
    assert "status:  failed" in out
    assert "401 unauthorized" in out


def test_failure_surfaces_stderr_on_stdout_and_disk(monkeypatch, tmp_path, capsys):
    _pin_agents(monkeypatch, tmp_path, ["architect"])
    _stub_codex(monkeypatch, tmp_path, items=[], stderr="codex: 401 unauthorized\n")
    codex_run.main(["@architect", "do x"])
    out = capsys.readouterr().out
    assert "401 unauthorized" in out
    disk = next(line.split(None, 1)[1].strip() for line in out.splitlines()
                if line.startswith("output:"))
    assert "401 unauthorized" in open(disk).read()


def test_reader_record_write_failure_ends_the_run(monkeypatch, tmp_path, capsys):
    _pin_agents(monkeypatch, tmp_path, ["architect"])
    _stub_codex(monkeypatch, tmp_path)
    real_replace = os.replace

    def fail_once(source, destination):
        if (threading.current_thread() is not threading.main_thread()
                and str(source).endswith(".json.writing")):
            raise OSError("record disk full")
        return real_replace(source, destination)

    monkeypatch.setattr(codex_run.os, "replace", fail_once)
    assert codex_run.main(["@architect", "do x"]) == 1
    assert "cannot write job record: record disk full" in capsys.readouterr().out
    assert _record(tmp_path)["status"] == "failed"


def test_event_file_creation_failure_is_recorded_and_stops_the_run(monkeypatch, tmp_path, capsys):
    _pin_agents(monkeypatch, tmp_path, ["architect"])
    _stub_codex(monkeypatch, tmp_path)
    real_open = builtins.open

    def fail_events(path, mode="r", *args, **kwargs):
        if str(path).endswith(".jsonl") and "w" in mode:
            raise OSError("read-only filesystem")
        return real_open(path, mode, *args, **kwargs)

    monkeypatch.setattr(builtins, "open", fail_events)
    assert codex_run.main(["@architect", "do x"]) == 1
    assert "cannot create event stream" in capsys.readouterr().out
    assert _record(tmp_path)["status"] == "failed"


def test_event_append_failure_marks_the_run_failed(monkeypatch, tmp_path, capsys):
    _pin_agents(monkeypatch, tmp_path, ["architect"])
    _stub_codex(monkeypatch, tmp_path)
    real_open = builtins.open

    class BrokenEvents:
        def __init__(self, file_handle):
            self.file_handle = file_handle

        def write(self, _):
            raise OSError("event disk full")

        def flush(self):
            self.file_handle.flush()

        def close(self):
            self.file_handle.close()

    def fail_append(path, mode="r", *args, **kwargs):
        file_handle = real_open(path, mode, *args, **kwargs)
        if str(path).endswith(".jsonl") and "w" in mode:
            return BrokenEvents(file_handle)
        return file_handle

    monkeypatch.setattr(builtins, "open", fail_append)
    assert codex_run.main(["@architect", "do x"]) == 1
    assert "cannot append event stream" in capsys.readouterr().out
    assert _record(tmp_path)["status"] == "failed"


def test_final_answer_write_failure_changes_ok_to_failed(monkeypatch, tmp_path, capsys):
    _pin_agents(monkeypatch, tmp_path, ["architect"])
    _stub_codex(monkeypatch, tmp_path)
    real_open = builtins.open

    def fail_answer(path, mode="r", *args, **kwargs):
        if str(path).endswith(".txt") and "w" in mode:
            raise OSError("answer disk full")
        return real_open(path, mode, *args, **kwargs)

    monkeypatch.setattr(builtins, "open", fail_answer)
    assert codex_run.main(["@architect", "do x"]) == 1
    assert "cannot write final answer" in capsys.readouterr().out
    assert _record(tmp_path)["status"] == "failed"


def test_ten_concurrent_short_jobs_stay_within_the_monitor_budget(monkeypatch, tmp_path):
    agents = _pin_agents(monkeypatch, tmp_path, ["architect"])
    log = _stub_codex(monkeypatch, tmp_path)
    script = json.loads(os.environ["STUB_SCRIPT"])
    started = time.time()
    interval = 0.02
    processes = [_driver(tmp_path, ["@architect", "short job"], agents, log, script,
                         feed_interval=interval) for _ in range(10)]
    try:
        for process in processes:
            out, err = process.communicate(timeout=90)
            assert process.returncode == 0, out + err
    finally:
        for process in processes:
            if process.poll() is None:
                process.kill()
    lines = _feed(tmp_path)
    elapsed = time.time() - started
    assert len(lines) <= codex_run._FEED_BURST + int(elapsed / interval)


def test_resume_reads_the_agent_off_the_record(monkeypatch, tmp_path, capsys):
    job = _founding_run(monkeypatch, tmp_path)
    capsys.readouterr()
    log = _stub_codex(monkeypatch, tmp_path, thread="th_founded")
    assert codex_run.main(["resume", job, "continue"]) == 0
    out = capsys.readouterr().out
    assert _sent(log, "thread/resume")["threadId"] == "th_founded"
    assert "agent:   researcher" in out


def test_cancel_interrupts_the_turn_and_the_run_ends(monkeypatch, tmp_path, capsys):
    agents = _pin_agents(monkeypatch, tmp_path, ["architect"])
    log = _stub_codex(monkeypatch, tmp_path, hang=True)
    process = _driver(tmp_path, ["@architect", "long task"], agents, log,
                      json.loads(os.environ["STUB_SCRIPT"]))
    try:
        record = _wait_for_record(tmp_path, "running", 30)
        assert codex_run.main(["cancel", record["job"]]) == 0
        assert process.wait(timeout=60) == 1
    finally:
        process.kill()
    assert "interrupting" in capsys.readouterr().out
    assert _record(tmp_path)["status"] == "cancelled"
    assert any(request.get("method") == "turn/interrupt" for request in _requests(log))


def test_cancelling_a_finished_job_is_a_no_op(monkeypatch, tmp_path, capsys):
    job = _founding_run(monkeypatch, tmp_path)
    capsys.readouterr()
    assert codex_run.main(["cancel", job]) == 0
    assert "already ok" in capsys.readouterr().out


def test_a_fresh_run_goes_end_to_end_through_the_real_launcher(tmp_path, monkeypatch):
    agents = tmp_path / "agents"
    agents.mkdir()
    (agents / "researcher.prompt.md").write_text("You are a researcher.\n")
    bin_directory = tmp_path / "bin"
    bin_directory.mkdir()
    (bin_directory / "codex").write_text(_STUB)
    (bin_directory / "codex").chmod(0o755)
    log = tmp_path / "requests.jsonl"
    script = {"thread": "th_e2e", "items": [_message("the answer")]}

    process = _driver(tmp_path, ["@researcher", "start"], agents, log, script)
    out, err = process.communicate(timeout=120)
    assert process.returncode == 0, out + err
    assert "the answer" in out and "Traceback" not in err
