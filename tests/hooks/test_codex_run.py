"""Coverage for lib/codex_run.py — the app-server transport, the job record, and
the job surface built on it.

The runner drives `codex app-server` over JSON-RPC. These tests replace `codex`
with a PATH stub that speaks the same protocol and emits a scripted notification
stream, so the real transport code runs — handshake, request/response
correlation, notification handling, phase tracking, the record, the feed — with
no real codex and no network. The stub logs every request it received, which is
how a test asserts on what actually reached codex (the inline instructions, the
memory config, the model, the effort).

Output lands under tmp_path (the resolver is pinned there), never a real session
dir.
"""

import builtins
import json
import os
import subprocess
import sys
import threading
import time

import pytest
from conftest import PY_HOOKS

sys.path.insert(0, os.path.join(PY_HOOKS, "lib"))

from lib import agent_memory, codex_run  # noqa: E402

# --- a stub codex app-server --------------------------------------------------------

_STUB = r'''#!/usr/bin/env python3
"""A `codex app-server` that speaks the protocol and follows a scripted turn."""
import json, os, sys, time

if len(sys.argv) < 2 or sys.argv[1] != "app-server":
    sys.exit("stub codex: unexpected argv %r" % (sys.argv[1:],))

script = json.loads(os.environ.get("STUB_SCRIPT") or "{}")
log = open(os.environ["STUB_LOG"], "a")
if script.get("invalid_output"):
    sys.stdout.write(script["invalid_output"] + "\n")
    sys.stdout.flush()
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
        if script.get("resume_error"):
            send({"id": rid, "error": {"code": -32600, "message": script["resume_error"]}})
        else:
            thread = params.get("threadId") or thread
            send({"id": rid, "result": {"thread": {
                "id": thread, "path": script.get("rollout", "/tmp/rollout-stub.jsonl")}}})
    elif method == "turn/start":
        send({"id": rid, "result": {"turn": {"id": "turn_1", "status": "inProgress"}}})
        if script.get("exit_after_start"):
            sys.exit(0)
        if script.get("hang"):
            continue
        notify("turn/started", {"threadId": thread, "turn": {"id": "turn_1", "status": "inProgress"}})
        for usage in script.get("token_usage", []):
            notify("thread/tokenUsage/updated", {"threadId": thread, "tokenUsage": {"total": usage}})
        for item in script.get("items", []):
            notify("item/started", {"threadId": thread, "turnId": "turn_1", "item": item})
            for usage in item.get("token_usage", []):
                notify("thread/tokenUsage/updated", {"threadId": thread, "tokenUsage": {"total": usage}})
            time.sleep(item.get("delay", 0))
            notify("item/completed", {"threadId": thread, "turnId": "turn_1", "item": item})
        if script.get("hang_after_items"):
            continue
        if script.get("error"):
            notify("error", {"threadId": thread, "error": {"message": script["error"]}})
        notify("turn/completed", {"threadId": thread, "turn": {
            "id": "turn_1", "status": script.get("turn_status", "completed")}})
    elif method == "turn/interrupt":
        if script.get("interrupt_error"):
            send({"id": rid, "error": {"code": -32000, "message": script["interrupt_error"]}})
            continue
        send({"id": rid, "result": {}})
        notify("turn/completed", {"threadId": thread, "turn": {"id": "turn_1", "status": "aborted"}})
'''


def _message(text):
    return {"type": "agentMessage", "phase": "final_answer", "text": text}


def _stub_codex(monkeypatch, tmp_path, **script):
    """Put a protocol-speaking `codex` first on PATH, pin output under tmp_path, and
    return the path its request log is written to."""
    binv = tmp_path / "bin"
    binv.mkdir(exist_ok=True)
    stub = binv / "codex"
    stub.write_text(_STUB)
    stub.chmod(0o755)
    log = tmp_path / "requests.jsonl"
    script.setdefault("items", [_message("the answer")])
    monkeypatch.setenv("PATH", "%s:%s" % (binv, os.environ["PATH"]))
    monkeypatch.setenv("STUB_LOG", str(log))
    monkeypatch.setenv("STUB_SCRIPT", json.dumps(script))
    monkeypatch.setenv("CLAUDE_DATA_ROOT", str(tmp_path / "data"))
    monkeypatch.setattr(codex_run, "_resolve_output_dir", lambda: str(tmp_path))
    return log


def _requests(log):
    return [json.loads(line) for line in open(log).read().splitlines()]


def _sent(log, method):
    """The params of the most recent `method` request. The log is shared by every
    run in a test, so the last one is the run being asserted on."""
    return [r["params"] for r in _requests(log) if r.get("method") == method][-1]


def _pin_agents(monkeypatch, tmp_path, names, frontmatter=None):
    """Point the shared roster at a tmp dir holding <name>.prompt.md for each name.

    CLAUDE_CONFIG_DIR is pinned at an empty root so the active-root roster
    contributes nothing and the real ~/.claude never leaks into a test."""
    agents = tmp_path / "agents"
    agents.mkdir(exist_ok=True)
    for name in names:
        (agents / (name + ".prompt.md")).write_text("instructions for %s" % name)
        if frontmatter:
            (agents / (name + ".md")).write_text("---\nname: %s\n%s\n---\n\nbody\n"
                                                 % (name, frontmatter))
    monkeypatch.setattr(codex_run, "AGENTS_DIR", str(agents))
    monkeypatch.setenv("CLAUDE_CONFIG_DIR", str(tmp_path / "empty-root"))
    return agents


def _pin_profile(monkeypatch, tmp_path, names):
    """Give the active config root its own roster, as a profile has."""
    root = tmp_path / "profile-root"
    profile_agents = root / "agents"
    profile_agents.mkdir(parents=True)
    for name in names:
        (profile_agents / (name + ".prompt.md")).write_text("profile instructions for %s" % name)
    monkeypatch.setenv("CLAUDE_CONFIG_DIR", str(root))
    return profile_agents


def _record(tmp_path):
    path = next(p for p in tmp_path.glob("codex-run-*.json"))
    return json.loads(path.read_text())


def _feed(tmp_path):
    return (tmp_path / codex_run._FEED).read_text().splitlines()


# --- the run: failure detection and the stdout contract -----------------------------

def test_answer_is_ok(monkeypatch, tmp_path, capsys):
    _pin_agents(monkeypatch, tmp_path, ["architect"])
    _stub_codex(monkeypatch, tmp_path)
    rc = codex_run.main(["@architect", "do x"])
    out = capsys.readouterr().out
    assert rc == 0
    assert "the answer" in out
    assert "status:  ok" in out
    assert codex_run._TRAILER in out


def test_trailer_carries_the_job_id(monkeypatch, tmp_path, capsys):
    """The job id is what every other command takes, so the run has to name it."""
    _pin_agents(monkeypatch, tmp_path, ["architect"])
    _stub_codex(monkeypatch, tmp_path)
    codex_run.main(["@architect", "do x"])
    out = capsys.readouterr().out
    job = next(line.split(None, 1)[1].strip() for line in out.splitlines()
               if line.startswith("job:"))
    assert job == _record(tmp_path)["job"]


def test_no_answer_is_failure(monkeypatch, tmp_path, capsys):
    # A turn that completes but produced no agent message is not a success.
    _pin_agents(monkeypatch, tmp_path, ["architect"])
    _stub_codex(monkeypatch, tmp_path, items=[])
    rc = codex_run.main(["@architect", "do x"])
    assert rc == 1
    assert "status:  failed" in capsys.readouterr().out
    assert _record(tmp_path)["error"] == "turn completed but produced no message"


def test_failed_turn_status_is_failure(monkeypatch, tmp_path, capsys):
    _pin_agents(monkeypatch, tmp_path, ["architect"])
    _stub_codex(monkeypatch, tmp_path, turn_status="failed")
    rc = codex_run.main(["@architect", "do x"])
    assert rc == 1
    assert "status:  failed" in capsys.readouterr().out


def test_error_notification_is_failure(monkeypatch, tmp_path, capsys):
    """An `error` notification fails the run even when the turn reports completed —
    codex answering with an error and a message is not a usable result."""
    _pin_agents(monkeypatch, tmp_path, ["architect"])
    _stub_codex(monkeypatch, tmp_path, error="model request failed: 401 unauthorized")
    rc = codex_run.main(["@architect", "do x"])
    out = capsys.readouterr().out
    assert rc == 1
    assert "status:  failed" in out
    assert "401 unauthorized" in out


def test_failure_surfaces_stderr_on_stdout_and_disk(monkeypatch, tmp_path, capsys):
    """On a failed run with no answer, codex's stderr is the only diagnosis there
    is, so it becomes the result rather than a bare "[no answer]"."""
    _pin_agents(monkeypatch, tmp_path, ["architect"])
    _stub_codex(monkeypatch, tmp_path, items=[], stderr="codex: 401 unauthorized\n")
    codex_run.main(["@architect", "do x"])
    out = capsys.readouterr().out
    assert "401 unauthorized" in out
    disk = next(line.split(None, 1)[1].strip() for line in out.splitlines()
                if line.startswith("output:"))
    assert "401 unauthorized" in open(disk).read()


def test_a_codex_that_will_not_start_fails_loudly(monkeypatch, tmp_path, capsys):
    """No traceback out of a wrapper whose whole interface is its printed result."""
    _pin_agents(monkeypatch, tmp_path, ["architect"])
    _stub_codex(monkeypatch, tmp_path)
    monkeypatch.setenv("PATH", str(tmp_path / "nothing"))
    rc = codex_run.main(["@architect", "do x"])
    out = capsys.readouterr().out
    assert rc == 1
    assert "status:  failed" in out
    assert "Traceback" not in out


def test_a_record_write_failure_stops_before_creating_other_outputs(monkeypatch, tmp_path, capsys):
    _pin_agents(monkeypatch, tmp_path, ["architect"])
    _stub_codex(monkeypatch, tmp_path)
    real_open = builtins.open

    def fail_record(path, *args, **kwargs):
        if str(path).endswith(".json.writing"):
            raise OSError("disk full")
        return real_open(path, *args, **kwargs)

    monkeypatch.setattr(builtins, "open", fail_record)
    assert codex_run.main(["@architect", "do x"]) == 1
    out = capsys.readouterr().out
    assert "cannot write job record" in out and "disk full" in out
    assert not list(tmp_path.glob("codex-run-*"))


def test_reader_record_write_failure_ends_the_run(monkeypatch, tmp_path, capsys):
    """A save from _read must fail the job, not leave its waiter at the idle limit."""
    _pin_agents(monkeypatch, tmp_path, ["architect"])
    _stub_codex(monkeypatch, tmp_path)
    real_replace = os.replace

    def fail_once(source, destination):
        if (threading.current_thread() is not threading.main_thread()
                and str(source).endswith(".json.writing")):
            raise OSError("record disk full")
        return real_replace(source, destination)

    monkeypatch.setattr(codex_run.os, "replace", fail_once)
    monkeypatch.setattr(codex_run, "_IDLE_LIMIT", 60)
    started = time.time()
    assert codex_run.main(["@architect", "do x"]) == 1
    elapsed = time.time() - started
    out = capsys.readouterr().out
    assert elapsed < 5
    assert "cannot write job record: record disk full" in out
    assert _record(tmp_path)["status"] == "failed"
    assert _record(tmp_path)["error"] == "cannot write job record: record disk full"


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
    record = _record(tmp_path)
    assert record["status"] == "failed"
    assert "cannot create event stream" in record["error"]


def test_event_append_failure_marks_the_run_failed(monkeypatch, tmp_path, capsys):
    _pin_agents(monkeypatch, tmp_path, ["architect"])
    _stub_codex(monkeypatch, tmp_path)
    real_open = builtins.open

    class BrokenEvents:
        def __init__(self, fh):
            self.fh = fh

        def write(self, _):
            raise OSError("event disk full")

        def flush(self):
            self.fh.flush()

        def close(self):
            self.fh.close()

    def fail_append(path, mode="r", *args, **kwargs):
        fh = real_open(path, mode, *args, **kwargs)
        return BrokenEvents(fh) if str(path).endswith(".jsonl") and "w" in mode else fh

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
    out = capsys.readouterr().out
    assert "cannot write final answer" in out and "status:  failed" in out
    assert _record(tmp_path)["status"] == "failed"


def test_preparation_answer_write_failure_is_reported(monkeypatch, tmp_path, capsys):
    _pin_agents(monkeypatch, tmp_path, ["architect"], frontmatter="effort: nope")
    _stub_codex(monkeypatch, tmp_path)
    real_open = builtins.open

    def fail_answer(path, mode="r", *args, **kwargs):
        if str(path).endswith(".txt") and "w" in mode:
            raise OSError("answer disk full")
        return real_open(path, mode, *args, **kwargs)

    monkeypatch.setattr(builtins, "open", fail_answer)
    assert codex_run.main(["@architect", "do x"]) == 1
    assert "cannot write preparation failure answer" in capsys.readouterr().out


def test_feed_write_failure_marks_the_run_failed(monkeypatch, tmp_path, capsys):
    _pin_agents(monkeypatch, tmp_path, ["architect"])
    _stub_codex(monkeypatch, tmp_path)
    real_open = builtins.open

    def fail_feed(path, mode="r", *args, **kwargs):
        if str(path).endswith(codex_run._FEED) and "a" in mode:
            raise OSError("feed disk full")
        return real_open(path, mode, *args, **kwargs)

    monkeypatch.setattr(builtins, "open", fail_feed)
    assert codex_run.main(["@architect", "do x"]) == 1
    assert "cannot append lifecycle feed" in capsys.readouterr().out
    assert _record(tmp_path)["status"] == "failed"


def test_invalid_app_server_output_fails_immediately(monkeypatch, tmp_path, capsys):
    _pin_agents(monkeypatch, tmp_path, ["architect"])
    _stub_codex(monkeypatch, tmp_path, invalid_output="not json")
    assert codex_run.main(["@architect", "do x"]) == 1
    assert "invalid app-server output: not json" in capsys.readouterr().out


def test_a_wedged_codex_hits_the_idle_deadline(monkeypatch, tmp_path, capsys):
    """The defect the exec transport had no answer for: a turn that never ends and
    never says anything used to hang until the harness ceiling."""
    _pin_agents(monkeypatch, tmp_path, ["architect"])
    _stub_codex(monkeypatch, tmp_path, hang=True)
    monkeypatch.setattr(codex_run, "_IDLE_LIMIT", 0.5)
    monkeypatch.setattr(codex_run, "_POLL_INTERVAL", 0.01)
    started = time.time()
    rc = codex_run.main(["@architect", "do x"])
    out = capsys.readouterr().out
    assert rc == 1
    assert time.time() - started < 30
    assert "status:  failed" in out
    assert "sent nothing" in out


def test_an_app_server_that_exits_under_a_turn_fails_without_waiting_for_silence(monkeypatch, tmp_path, capsys):
    _pin_agents(monkeypatch, tmp_path, ["architect"])
    _stub_codex(monkeypatch, tmp_path, exit_after_start=True)
    monkeypatch.setattr(codex_run, "_IDLE_LIMIT", 60)
    monkeypatch.setattr(codex_run, "_POLL_INTERVAL", 0.01)
    started = time.time()
    assert codex_run.main(["@architect", "do x"]) == 1
    assert time.time() - started < 5
    assert "exited unexpectedly" in capsys.readouterr().out


def test_silence_after_a_tool_uses_the_separate_watchdog(monkeypatch, tmp_path, capsys):
    _pin_agents(monkeypatch, tmp_path, ["architect"])
    log = _stub_codex(monkeypatch, tmp_path, hang_after_items=True,
                      items=[{"type": "commandExecution", "command": "long test"}])
    monkeypatch.setattr(codex_run, "_POST_TOOL_IDLE_LIMIT", 0.3)
    monkeypatch.setattr(codex_run, "_IDLE_LIMIT", 60)
    monkeypatch.setattr(codex_run, "_POLL_INTERVAL", 0.01)
    assert codex_run.main(["@architect", "do x"]) == 1
    assert "after a tool result" in capsys.readouterr().out
    assert any(r.get("method") == "turn/interrupt" for r in _requests(log))


# --- what actually reaches codex ----------------------------------------------------

def test_instructions_ride_inline_on_thread_start(monkeypatch, tmp_path):
    """No file on disk for codex to read: the agent's prompt.md body is the
    baseInstructions value in the request itself."""
    _pin_agents(monkeypatch, tmp_path, ["architect"])
    log = _stub_codex(monkeypatch, tmp_path)
    assert codex_run.main(["@architect", "do x"]) == 0
    assert _sent(log, "thread/start")["baseInstructions"] == "instructions for architect"


def test_the_run_asks_for_full_access_and_no_hook_trust_gate(monkeypatch, tmp_path):
    """Our shared Python guards govern the run, not codex's sandbox, and our hooks
    are our own vetted sources."""
    _pin_agents(monkeypatch, tmp_path, ["architect"])
    log = _stub_codex(monkeypatch, tmp_path)
    codex_run.main(["@architect", "do x"])
    params = _sent(log, "thread/start")
    assert params["sandbox"] == "danger-full-access"
    assert params["approvalPolicy"] == "never"
    assert params["config"]["bypass_hook_trust"] is True


def test_memory_none_switches_off_codex_own_memories(monkeypatch, tmp_path):
    """codex's own [memories] needs no tool call at all — it injects a Memory
    section straight into the run — so a blank declaration has to switch it off
    here. Honcho is the other provider, gated at the `honcho` command instead."""
    _pin_agents(monkeypatch, tmp_path, ["explorer"], frontmatter="memory: none")
    log = _stub_codex(monkeypatch, tmp_path)
    codex_run.main(["@explorer", "look"])
    config = _sent(log, "thread/start")["config"]
    assert config["memories"] == {"use_memories": False, "generate_memories": False}


def test_an_undeclared_agent_keeps_its_memory(monkeypatch, tmp_path):
    """Omitting the key leaves Memory reachable — that is the contract every agent
    without the key relies on."""
    _pin_agents(monkeypatch, tmp_path, ["explorer"], frontmatter="model: opus")
    log = _stub_codex(monkeypatch, tmp_path)
    codex_run.main(["@explorer", "look"])
    assert "memories" not in _sent(log, "thread/start")["config"]


def test_declared_model_and_effort_reach_the_turn(monkeypatch, tmp_path, capsys):
    _pin_agents(monkeypatch, tmp_path, ["research-judge"],
                frontmatter="model: opus\ncodex-model: gpt-5.6-luna\neffort: high")
    log = _stub_codex(monkeypatch, tmp_path)
    assert codex_run.main(["@research-judge", "judge this"]) == 0
    assert _sent(log, "thread/start")["model"] == "gpt-5.6-luna"
    assert _sent(log, "turn/start")["model"] == "gpt-5.6-luna"
    assert _sent(log, "turn/start")["effort"] == "high"
    assert "model:   gpt-5.6-luna  (effort high)" in capsys.readouterr().out


def test_undeclared_agent_runs_the_defaults(monkeypatch, tmp_path):
    """Pinned to the literals, not the constants: asserting against `_EFFORT` would
    follow the production value wherever it moved and never fail."""
    _pin_agents(monkeypatch, tmp_path, ["ponytail"], frontmatter="model: opus")
    log = _stub_codex(monkeypatch, tmp_path)
    assert codex_run.main(["@ponytail", "do x"]) == 0
    assert _sent(log, "turn/start")["model"] == codex_run._MODEL
    assert _sent(log, "turn/start")["effort"] == "medium"


@pytest.mark.parametrize("declared", ["low", "medium", "high", "xhigh", "max"])
def test_every_level_claude_accepts_reaches_codex_unchanged(monkeypatch, tmp_path, declared):
    """The five levels `claude --effort` lists are the five codex takes, so each
    crosses untranslated. `xhigh` and `max` are separate tiers on both."""
    _pin_agents(monkeypatch, tmp_path, ["architect"], frontmatter="effort: %s" % declared)
    log = _stub_codex(monkeypatch, tmp_path)
    assert codex_run.main(["@architect", "review"]) == 0
    assert _sent(log, "turn/start")["effort"] == declared


def test_invocation_flags_override_the_agents_declarations(monkeypatch, tmp_path, capsys):
    """`--model`/`--effort` serve one invocation, sitting above the whole
    frontmatter chain — the codex counterpart of the Agent tool's opts."""
    _pin_agents(monkeypatch, tmp_path, ["architect"],
                frontmatter="codex-model: gpt-5.6-luna\ncodex-effort: low")
    log = _stub_codex(monkeypatch, tmp_path)
    assert codex_run.main(["@architect", "--model", "gpt-5.6-sol",
                           "--effort", "max", "one deep run"]) == 0
    assert _sent(log, "turn/start")["model"] == "gpt-5.6-sol"
    assert _sent(log, "turn/start")["effort"] == "max"
    assert "model:   gpt-5.6-sol  (effort max)" in capsys.readouterr().out


def test_an_unknown_flag_effort_stops_before_any_run(monkeypatch, tmp_path, capsys):
    _pin_agents(monkeypatch, tmp_path, ["architect"])
    _stub_codex(monkeypatch, tmp_path)
    assert codex_run.main(["@architect", "--effort", "highest", "do x"]) == 2
    out = capsys.readouterr().out
    assert "unknown effort" in out and "highest" in out
    assert not list(tmp_path.glob("codex-run-*"))


def test_a_flag_missing_its_value_is_a_usage_error(monkeypatch, tmp_path, capsys):
    _pin_agents(monkeypatch, tmp_path, ["architect"])
    _stub_codex(monkeypatch, tmp_path)
    assert codex_run.main(["@architect", "do x", "--model"]) == 2
    assert "--model needs a value" in capsys.readouterr().out


def test_codex_effort_overrides_effort_for_the_codex_run(monkeypatch, tmp_path):
    _pin_agents(monkeypatch, tmp_path, ["explorer"],
                frontmatter="effort: low\ncodex-effort: high")
    log = _stub_codex(monkeypatch, tmp_path)
    assert codex_run.main(["@explorer", "map this"]) == 0
    assert _sent(log, "turn/start")["effort"] == "high"


def test_blank_declarations_read_as_undeclared(monkeypatch, tmp_path):
    """`effort:` and `codex-model:` are the same line shape, so a key written with
    no value must resolve the same way for both."""
    _pin_agents(monkeypatch, tmp_path, ["ponytail"], frontmatter="effort:\ncodex-model:")
    assert codex_run._codex_effort("ponytail") == codex_run._EFFORT
    assert codex_run._codex_model("ponytail") == codex_run._MODEL


@pytest.mark.parametrize("declared", ["none", "3", "HIGH", "highest"])
def test_unknown_effort_fails_rather_than_defaulting(monkeypatch, tmp_path, capsys, declared):
    """The vocabulary is exactly what Claude's own key accepts, so a word outside it
    is a typo, not something to silently replace with the default."""
    _pin_agents(monkeypatch, tmp_path, ["ponytail"], frontmatter="effort: %s" % declared)
    _stub_codex(monkeypatch, tmp_path)
    rc = codex_run.main(["@ponytail", "do x"])
    out = capsys.readouterr().out
    assert rc == 1
    assert declared in out and "unknown effort" in out
    assert "Traceback" not in out


def test_declared_harness_claude_refuses_and_names_the_dispatch(monkeypatch, tmp_path, capsys):
    _pin_agents(monkeypatch, tmp_path, ["copy-chief"], frontmatter="harness: claude")
    _stub_codex(monkeypatch, tmp_path)
    rc = codex_run.main(["@copy-chief", "write it"])
    out = capsys.readouterr().out
    assert rc == 1
    assert "does not run here" in out
    assert 'subagent_type: "copy-chief"' in out


def test_unrecognized_harness_denies(monkeypatch, tmp_path, capsys):
    """An unrecognized declaration denies rather than permits, so a typo cannot
    quietly widen where an agent runs."""
    _pin_agents(monkeypatch, tmp_path, ["ponytail"], frontmatter="harness: Codex")
    _stub_codex(monkeypatch, tmp_path)
    assert codex_run.main(["@ponytail", "do x"]) == 1
    assert "which is not a harness" in capsys.readouterr().out


def test_the_definition_path_is_exported_to_codex(monkeypatch, tmp_path):
    """It rides in the app-server's environment, which is how a codex-side hook
    inside the run learns which agent it is gating."""
    agents = _pin_agents(monkeypatch, tmp_path, ["explorer"], frontmatter="tools: Bash")
    _stub_codex(monkeypatch, tmp_path)
    captured = {}
    real = codex_run.subprocess.Popen
    monkeypatch.setattr(codex_run.subprocess, "Popen",
                        lambda cmd, **kw: captured.update(kw.get("env") or {}) or real(cmd, **kw))
    assert codex_run.main(["@explorer", "look"]) == 0
    assert captured[agent_memory.AGENT_FILE_VAR] == str(agents / "explorer.md")


def test_an_outer_definition_path_does_not_leak_into_a_nested_run(monkeypatch, tmp_path):
    """A codex-run dispatched from inside another one inherits its parent's value.
    Gating the inner run as the outer agent is worse than not gating it at all, so
    the variable is written per run rather than left as found."""
    agents = _pin_agents(monkeypatch, tmp_path, ["explorer"], frontmatter="tools: Bash")
    _stub_codex(monkeypatch, tmp_path)
    monkeypatch.setenv(agent_memory.AGENT_FILE_VAR, "/roster/outer.md")
    captured = {}
    real = codex_run.subprocess.Popen
    monkeypatch.setattr(codex_run.subprocess, "Popen",
                        lambda cmd, **kw: captured.update(kw.get("env") or {}) or real(cmd, **kw))
    assert codex_run.main(["@explorer", "look"]) == 0
    assert captured[agent_memory.AGENT_FILE_VAR] == str(agents / "explorer.md")


# --- the job record -----------------------------------------------------------------

def test_the_record_holds_what_a_continuation_needs(monkeypatch, tmp_path):
    """Identity is recorded, not reconstructed: the agent that founded a thread is
    a field this runner wrote, where it used to be a guess read out of codex's
    session archive."""
    _pin_agents(monkeypatch, tmp_path, ["researcher"], frontmatter="codex-model: gpt-5.6-luna")
    _stub_codex(monkeypatch, tmp_path, thread="th_recorded",
                rollout="/tmp/rollout-recorded.jsonl")
    assert codex_run.main(["@researcher", "find out"]) == 0
    record = _record(tmp_path)
    assert record["agent"] == "researcher"
    assert record["thread"] == "th_recorded"
    assert record["rollout"] == "/tmp/rollout-recorded.jsonl"
    assert record["model"] == "gpt-5.6-luna"
    assert record["effort"] == "medium"
    assert record["status"] == "ok"
    assert record["pid"] == os.getpid()
    assert record["turn"] == "turn_1"
    assert os.path.isfile(record["answer"]) and os.path.isfile(record["events"])
    assert record["started_at"] and record["ended_at"]


def test_the_record_reaches_a_terminal_state_on_failure(monkeypatch, tmp_path):
    """A record left saying `running` means the runner itself died, so every exit
    path has to write the terminal state — the failing ones included."""
    _pin_agents(monkeypatch, tmp_path, ["architect"])
    _stub_codex(monkeypatch, tmp_path, items=[], turn_status="failed")
    assert codex_run.main(["@architect", "do x"]) == 1
    record = _record(tmp_path)
    assert record["status"] == "failed"
    assert record["ended_at"]


def test_the_record_shares_the_stem_with_the_answer_and_events(monkeypatch, tmp_path):
    _pin_agents(monkeypatch, tmp_path, ["architect"])
    _stub_codex(monkeypatch, tmp_path)
    codex_run.main(["@architect", "do x"])
    record = _record(tmp_path)
    stem = record["job"]
    assert os.path.basename(record["answer"]) == stem + ".txt"
    assert os.path.basename(record["events"]) == stem + ".jsonl"
    assert os.path.basename(record["record"]) == stem + ".json"


def test_the_event_stream_is_kept_whole(monkeypatch, tmp_path):
    _pin_agents(monkeypatch, tmp_path, ["architect"])
    _stub_codex(monkeypatch, tmp_path)
    codex_run.main(["@architect", "do x"])
    events = open(_record(tmp_path)["events"]).read().splitlines()
    methods = [json.loads(line).get("method") for line in events]
    assert "turn/started" in methods and "turn/completed" in methods


def test_the_record_tracks_live_command_activity_and_fresh_input_tokens(tmp_path, monkeypatch):
    agents = _pin_agents(monkeypatch, tmp_path, ["architect"])
    log = _stub_codex(monkeypatch, tmp_path)
    script = {
        "items": [
            {"type": "commandExecution", "id": "command-one",
             "command": "/bin/zsh -lc 'trace grep first'",
             "token_usage": [{"inputTokens": 100, "cachedInputTokens": 20}], "delay": 0.3},
            {"type": "commandExecution", "id": "command-two",
             "command": "/bin/zsh -lc 'trace grep second'",
             "token_usage": [{"inputTokens": 260, "cachedInputTokens": 80}], "delay": 0.3},
            _message("done"),
        ],
    }
    proc = _driver(tmp_path, ["@architect", "watch it"], agents, log, script)
    try:
        _wait_for_record_field(tmp_path, "activity", "trace grep first", 30)
        assert _wait_for_record_field(tmp_path, "fresh_input_tokens", 80, 30)
        _wait_for_record_field(tmp_path, "activity", "trace grep second", 30)
        assert _wait_for_record_field(tmp_path, "fresh_input_tokens", 180, 30)
        assert proc.wait(timeout=30) == 0
    finally:
        proc.kill()
    record = _record(tmp_path)
    assert record["activity"] == ""
    assert record["fresh_input_tokens"] == 180


def test_a_run_without_commands_keeps_activity_empty(monkeypatch, tmp_path):
    _pin_agents(monkeypatch, tmp_path, ["architect"])
    _stub_codex(monkeypatch, tmp_path, items=[_message("done")],
                token_usage=[{"inputTokens": 200, "cachedInputTokens": 150}])
    assert codex_run.main(["@architect", "answer only"]) == 0
    record = _record(tmp_path)
    assert record["activity"] == ""
    assert record["fresh_input_tokens"] == 50


# --- resume -------------------------------------------------------------------------

def _founding_run(monkeypatch, tmp_path, agent="researcher", **script):
    _pin_agents(monkeypatch, tmp_path, [agent])
    _stub_codex(monkeypatch, tmp_path, thread="th_founded", **script)
    assert codex_run.main(["@" + agent, "start"]) == 0
    return _record(tmp_path)["job"]


def test_resume_reads_the_agent_off_the_record(monkeypatch, tmp_path, capsys):
    """A resume takes a job and a message and nothing else, so the agent and the
    thread both come from the record rather than from codex's archive."""
    job = _founding_run(monkeypatch, tmp_path)
    capsys.readouterr()
    log = _stub_codex(monkeypatch, tmp_path, thread="th_founded")
    assert codex_run.main(["resume", job, "continue"]) == 0
    out = capsys.readouterr().out
    assert _sent(log, "thread/resume")["threadId"] == "th_founded"
    assert "agent:   researcher" in out
    assert "status:  ok" in out


def test_resume_runs_under_the_recovered_agents_instructions_and_settings(monkeypatch, tmp_path):
    """codex keeps none of them with the thread, so a continuation has to send them
    again or it answers as codex's global default agent."""
    _pin_agents(monkeypatch, tmp_path, ["researcher"],
                frontmatter="codex-model: gpt-5.6-luna\neffort: xhigh\nmemory: none")
    _stub_codex(monkeypatch, tmp_path, thread="th_founded")
    assert codex_run.main(["@researcher", "start"]) == 0
    job = _record(tmp_path)["job"]
    log = _stub_codex(monkeypatch, tmp_path, thread="th_founded")
    assert codex_run.main(["resume", job, "continue"]) == 0
    resumed = _sent(log, "thread/resume")
    assert resumed["baseInstructions"] == "instructions for researcher"
    assert resumed["model"] == "gpt-5.6-luna"
    assert resumed["config"]["memories"]["use_memories"] is False
    assert _sent(log, "turn/start")["effort"] == "xhigh"


def test_a_plain_resume_keeps_the_founding_runs_overrides(monkeypatch, tmp_path):
    """The record outranks the frontmatter on a resume, so a run founded with
    flags continues at the depth it was founded at."""
    _pin_agents(monkeypatch, tmp_path, ["researcher"], frontmatter="effort: low")
    _stub_codex(monkeypatch, tmp_path, thread="th_founded")
    assert codex_run.main(["@researcher", "--model", "gpt-5.6-sol",
                           "--effort", "max", "start"]) == 0
    job = _record(tmp_path)["job"]
    log = _stub_codex(monkeypatch, tmp_path, thread="th_founded")
    assert codex_run.main(["resume", job, "continue"]) == 0
    assert _sent(log, "turn/start")["model"] == "gpt-5.6-sol"
    assert _sent(log, "turn/start")["effort"] == "max"


def test_a_resume_flag_overrides_the_record_for_that_turn(monkeypatch, tmp_path):
    job = _founding_run(monkeypatch, tmp_path)
    log = _stub_codex(monkeypatch, tmp_path, thread="th_founded")
    assert codex_run.main(["resume", job, "--effort", "xhigh", "go deeper"]) == 0
    assert _sent(log, "turn/start")["effort"] == "xhigh"


def test_a_resume_is_its_own_job_pointing_back(monkeypatch, tmp_path):
    job = _founding_run(monkeypatch, tmp_path)
    _stub_codex(monkeypatch, tmp_path, thread="th_founded")
    assert codex_run.main(["resume", job, "continue"]) == 0
    # The job id carries a nanosecond stamp, so it orders two runs a fraction of a
    # second apart where the record's whole-second timestamp cannot.
    records = sorted((json.loads(p.read_text()) for p in tmp_path.glob("codex-run-*.json")),
                     key=lambda r: r["job"])
    assert len(records) == 2
    assert records[1]["resumed_from"] == job
    assert records[1]["resumed"] is True


def test_a_gone_thread_starts_fresh_and_says_so(monkeypatch, tmp_path, capsys):
    """A silent fresh thread under a resume command reads exactly like a
    continuation and is not one, so the run says plainly that it did not resume."""
    job = _founding_run(monkeypatch, tmp_path)
    capsys.readouterr()
    log = _stub_codex(monkeypatch, tmp_path, thread="th_fresh",
                      resume_error="no rollout found for thread id th_founded")
    rc = codex_run.main(["resume", job, "continue"])
    out = capsys.readouterr().out
    assert rc == 0
    assert "DID NOT RESUME" in out
    assert "no rollout found" in out
    assert "th_fresh" in out
    assert _sent(log, "thread/start")["baseInstructions"] == "instructions for researcher"
    assert _record_named(tmp_path, "th_fresh")["resumed"] is False


def _record_named(tmp_path, thread):
    return next(json.loads(p.read_text()) for p in tmp_path.glob("codex-run-*.json")
                if json.loads(p.read_text()).get("thread") == thread)


def test_resume_of_an_agent_that_left_the_roster_stops(monkeypatch, tmp_path, capsys):
    """Continuing as codex's global default instead of the founding agent is the
    defect this refusal removes."""
    job = _founding_run(monkeypatch, tmp_path)
    capsys.readouterr()
    _pin_agents(monkeypatch, tmp_path, ["architect"])
    (tmp_path / "agents" / "researcher.prompt.md").unlink()
    _stub_codex(monkeypatch, tmp_path)
    assert codex_run.main(["resume", job, "continue"]) == 1
    assert "not a runnable agent" in capsys.readouterr().out


def test_resume_of_an_unknown_job_stops(monkeypatch, tmp_path, capsys):
    _pin_agents(monkeypatch, tmp_path, ["researcher"])
    _stub_codex(monkeypatch, tmp_path)
    assert codex_run.main(["resume", "codex-run-nope", "continue"]) == 1
    assert "no job matching" in capsys.readouterr().out


# --- job ids ------------------------------------------------------------------------

def test_a_unique_prefix_names_a_job(monkeypatch, tmp_path, capsys):
    job = _founding_run(monkeypatch, tmp_path)
    capsys.readouterr()
    found, complaint = codex_run._find_job(job[:-4])
    assert complaint == ""
    assert found["job"] == job


def test_an_ambiguous_prefix_is_refused(monkeypatch, tmp_path, capsys):
    _founding_run(monkeypatch, tmp_path)
    _stub_codex(monkeypatch, tmp_path, thread="th_founded")
    codex_run.main(["@researcher", "again"])
    capsys.readouterr()
    found, complaint = codex_run._find_job("codex-run-")
    assert found is None
    assert "matches 2 jobs" in complaint


def test_a_prefix_ambiguous_with_a_sibling_session_is_refused(monkeypatch, tmp_path):
    job = _founding_run(monkeypatch, tmp_path)
    sibling = tmp_path / "data" / "sessions" / "session-b"
    sibling.mkdir(parents=True)
    record = _record(tmp_path)
    record.update(job="codex-run-sibling", record=str(sibling / "codex-run-sibling.json"))
    codex_run._save_record(record)
    found, complaint = codex_run._find_job("codex-run")
    assert found is None
    assert job in complaint and "codex-run-sibling" in complaint


# --- the lifecycle feed -------------------------------------------------------------

def test_the_feed_carries_lifecycle_only(monkeypatch, tmp_path, capsys):
    """The Monitor reading this feed allows ten lines then one every two seconds and
    dies after thirty seconds of continuous suppression. A single run emits sixty to
    a hundred protocol events, so a feed that forwarded them would kill the monitor
    inside the first turn."""
    _pin_agents(monkeypatch, tmp_path, ["architect"])
    noisy = [{"type": "commandExecution", "command": "pytest tests/hooks -k %d" % n,
              "exitCode": 0, "status": "completed"} for n in range(60)]
    _stub_codex(monkeypatch, tmp_path, items=noisy + [_message("done")])
    assert codex_run.main(["@architect", "do x"]) == 0
    lines = _feed(tmp_path)
    assert len(lines) == 2, lines
    states = [line.split()[1] for line in lines]
    assert states == ["started", "ok"]
    assert lines[-1].endswith("5 chars")
    assert all(len(line) <= codex_run._LINE_LIMIT for line in lines)


def test_terminal_feed_size_matches_the_last_answer_written_to_disk(monkeypatch, tmp_path):
    _pin_agents(monkeypatch, tmp_path, ["architect"])
    _stub_codex(monkeypatch, tmp_path, items=[_message("a" * 870), _message("stop")])
    assert codex_run.main(["@architect", "do x"]) == 0
    answer = _record(tmp_path)["answer"]
    assert open(answer, "rb").read() == b"stop\n"
    assert _feed(tmp_path)[-1].endswith("5 chars")


def test_a_feed_line_stays_inside_the_length_limit(monkeypatch, tmp_path):
    _pin_agents(monkeypatch, tmp_path, ["architect"])
    _stub_codex(monkeypatch, tmp_path)
    codex_run.main(["@architect", "x " * 4000])
    assert all(len(line) <= codex_run._LINE_LIMIT for line in _feed(tmp_path))


def test_a_failed_run_says_so_on_the_feed(monkeypatch, tmp_path):
    _pin_agents(monkeypatch, tmp_path, ["architect"])
    _stub_codex(monkeypatch, tmp_path, items=[], turn_status="failed")
    codex_run.main(["@architect", "do x"])
    lines = _feed(tmp_path)
    assert [line.split()[1] for line in lines] == ["started", "failed"]
    assert lines[-1].endswith("%d chars" % len(open(_record(tmp_path)["answer"], "rb").read()))


def test_parallel_runs_share_one_feed(monkeypatch, tmp_path, capsys):
    """One file per Claude session, appended by every run in it — `watch` is the
    session's view, not one job's."""
    _pin_agents(monkeypatch, tmp_path, ["architect"])
    _stub_codex(monkeypatch, tmp_path)
    codex_run.main(["@architect", "first"])
    codex_run.main(["@architect", "second"])
    capsys.readouterr()
    jobs = {line.split()[2] for line in _feed(tmp_path)}
    assert len(jobs) == 2


def test_watch_stops_when_its_feed_becomes_unreadable(monkeypatch, tmp_path, capsys):
    path = tmp_path / codex_run._FEED
    path.mkdir()
    monkeypatch.setattr(codex_run, "_resolve_output_dir", lambda: str(tmp_path))
    monkeypatch.setattr(codex_run.time, "sleep", lambda _: (_ for _ in ()).throw(
        AssertionError("watch continued after its unreadable feed")))

    assert codex_run.main(["watch"]) == 1
    out = capsys.readouterr().out
    assert "watching %s" % path in out
    assert "cannot read lifecycle feed" in out
    assert str(path) in out


def test_ten_concurrent_short_jobs_stay_within_the_monitor_budget(monkeypatch, tmp_path):
    agents = _pin_agents(monkeypatch, tmp_path, ["architect"])
    log = _stub_codex(monkeypatch, tmp_path)
    script = json.loads(os.environ["STUB_SCRIPT"])
    started = time.time()
    interval = 0.02
    processes = [_driver(tmp_path, ["@architect", "short job"], agents, log, script,
                         feed_interval=interval)
                 for _ in range(10)]
    try:
        for process in processes:
            out, err = process.communicate(timeout=90)
            assert process.returncode == 0, out + err
    finally:
        for process in processes:
            if process.poll() is None:
                process.kill()
    lines = _feed(tmp_path)
    assert codex_run._FEED_BURST == 10 and codex_run._FEED_INTERVAL == 2
    elapsed = time.time() - started
    assert len(lines) <= codex_run._FEED_BURST + int(elapsed / interval)


# --- the job surface ----------------------------------------------------------------

def test_status_lists_this_sessions_jobs(monkeypatch, tmp_path, capsys):
    job = _founding_run(monkeypatch, tmp_path)
    capsys.readouterr()
    assert codex_run.main(["status"]) == 0
    out = capsys.readouterr().out
    assert job in out
    assert "researcher" in out and "ok" in out


def test_status_reconciles_a_dead_runner_as_failed(monkeypatch, tmp_path, capsys):
    """A reader observes the dead process and writes the terminal truth."""
    _founding_run(monkeypatch, tmp_path)
    capsys.readouterr()
    path = next(p for p in tmp_path.glob("codex-run-*.json"))
    record = json.loads(path.read_text())
    record.update(status="running", pid=999999)
    path.write_text(json.dumps(record))
    codex_run.main(["status"])
    assert "failed" in capsys.readouterr().out
    assert _record(tmp_path)["error"] == "codex-run runner exited unexpectedly"


def test_status_reports_and_skips_a_corrupt_record(monkeypatch, tmp_path, capsys):
    _founding_run(monkeypatch, tmp_path)
    record = _record(tmp_path)
    corrupt = tmp_path / "codex-run-corrupt.json"
    corrupt.write_text('{"job":')
    capsys.readouterr()

    assert codex_run.main(["status"]) == 0
    streams = capsys.readouterr()
    assert record["job"] in streams.out
    # The diagnostic goes to stderr: the lifecycle hooks scan records on every
    # Stop and SessionEnd, and stdout there is the harness's channel, not ours.
    assert "cannot read job record" in streams.err
    assert str(corrupt) in streams.err
    assert "cannot read job record" not in streams.out


def test_killing_a_real_runner_reconciles_its_record_as_failed(tmp_path, monkeypatch, capsys):
    agents = _pin_agents(monkeypatch, tmp_path, ["architect"])
    log = _stub_codex(monkeypatch, tmp_path, hang=True)
    proc = _driver(tmp_path, ["@architect", "long task"], agents, log,
                   json.loads(os.environ["STUB_SCRIPT"]))
    record = _wait_for_record(tmp_path, "running", 30)
    try:
        proc.kill()
        proc.wait(timeout=10)
        assert codex_run.main(["status"]) == 0
    finally:
        codex_run._terminate_tree(record["server_pid"])
    saved = json.loads(next(tmp_path.glob("codex-run-*.json")).read_text())
    assert saved["status"] == "failed"
    assert saved["error"] == "codex-run runner exited unexpectedly"
    assert saved["ended_at"]


def test_result_prints_the_answer_and_the_trailer(monkeypatch, tmp_path, capsys):
    job = _founding_run(monkeypatch, tmp_path)
    capsys.readouterr()
    assert codex_run.main(["result", job]) == 0
    out = capsys.readouterr().out
    assert "the answer" in out
    assert "agent:   researcher" in out


def test_history_names_the_rollout_and_the_transcript(monkeypatch, tmp_path, capsys):
    (tmp_path / "transcript").write_text("/Users/x/.claude/projects/p/s.jsonl\n")
    job = _founding_run(monkeypatch, tmp_path, rollout="/tmp/rollout-hist.jsonl")
    capsys.readouterr()
    assert codex_run.main(["history", job]) == 0
    out = capsys.readouterr().out
    assert "/tmp/rollout-hist.jsonl" in out
    assert "/Users/x/.claude/projects/p/s.jsonl" in out


def test_log_renders_activity(monkeypatch, tmp_path, capsys):
    _pin_agents(monkeypatch, tmp_path, ["architect"])
    _stub_codex(monkeypatch, tmp_path, items=[
        {"type": "commandExecution", "command": "uv run pytest", "exitCode": 0},
        {"type": "fileChange", "changes": [{"path": "/repo/a.py"}]},
        _message("done")])
    codex_run.main(["@architect", "do x"])
    job = _record(tmp_path)["job"]
    capsys.readouterr()
    assert codex_run.main(["log", job]) == 0
    out = capsys.readouterr().out
    assert "command  uv run pytest (exit 0)" in out
    assert "edit     /repo/a.py" in out
    assert "turn completed" in out


def test_events_tail_narrows_the_raw_stream(monkeypatch, tmp_path, capsys):
    job = _founding_run(monkeypatch, tmp_path)
    capsys.readouterr()
    assert codex_run.main(["events", job, "--tail", "2"]) == 0
    lines = capsys.readouterr().out.splitlines()
    assert len(lines) == 2
    assert all(json.loads(line) for line in lines)


def test_a_command_needing_a_job_says_so(monkeypatch, tmp_path, capsys):
    assert codex_run.main(["result"]) == 2
    assert "needs a job id" in capsys.readouterr().out


# --- cancellation -------------------------------------------------------------------

def _driver(tmp_path, argv, agents, log, script, idle_limit=None, feed_interval=None,
            poll_interval=None):
    """A `codex-run` process with the roster, the output dir and the stub pinned
    from outside, so the real main() runs against the real argv."""
    driver = tmp_path / ("driver-%d.py" % time.time_ns())
    driver.write_text(
        "import sys\n"
        "sys.path.insert(0, %r)\n"
        "sys.path.insert(0, %r)\n"
        "import codex_run\n"
        "codex_run.AGENTS_DIR = %r\n"
            "codex_run._resolve_output_dir = lambda: %r\n"
            "%s%s%s"
        "sys.exit(codex_run.main(%r))\n"
        % (os.path.join(PY_HOOKS, "lib"), PY_HOOKS, str(agents), str(tmp_path),
           "codex_run._IDLE_LIMIT = %r\n" % idle_limit if idle_limit is not None else "",
           "codex_run._FEED_INTERVAL = %r\n" % feed_interval if feed_interval is not None else "",
           "codex_run._POLL_INTERVAL = %r\n" % poll_interval if poll_interval is not None else "", argv))
    env = dict(os.environ,
               PATH="%s:%s" % (tmp_path / "bin", os.environ["PATH"]),
               CLAUDE_CONFIG_DIR=str(tmp_path / "empty-root"),
               CLAUDE_DATA_ROOT=str(tmp_path / "data"),
               STUB_LOG=str(log), STUB_SCRIPT=json.dumps(script))
    env.pop("CODEX_RUN_AGENT_FILE", None)
    return subprocess.Popen([sys.executable, str(driver)], env=env,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)


def test_cancel_interrupts_the_turn_and_the_run_ends(monkeypatch, tmp_path, capsys):
    """Killing the process outright is what left half-applied patches behind. The
    interrupt goes over the live connection first, and only a runner that will not
    answer its own signal gets the process tree."""
    agents = _pin_agents(monkeypatch, tmp_path, ["architect"])
    log = _stub_codex(monkeypatch, tmp_path, hang=True)
    proc = _driver(tmp_path, ["@architect", "long task"], agents, log,
                   json.loads(os.environ["STUB_SCRIPT"]))
    try:
        record = _wait_for_record(tmp_path, "running", 30)
        assert codex_run.main(["cancel", record["job"]]) == 0
        assert proc.wait(timeout=60) == 1
    finally:
        proc.kill()
    out = capsys.readouterr().out
    assert "interrupting" in out
    assert json.loads(next(tmp_path.glob("codex-run-*.json")).read_text())["status"] == "cancelled"
    assert any(r.get("method") == "turn/interrupt" for r in _requests(log))
    assert [line.split()[1] for line in _feed(tmp_path)] == ["started", "cancelled"]


def test_a_real_silent_app_server_is_interrupted_and_leaves_no_process(tmp_path, monkeypatch):
    """The subprocess path proves the watchdog, terminal record, and teardown together."""
    agents = _pin_agents(monkeypatch, tmp_path, ["architect"])
    log = _stub_codex(monkeypatch, tmp_path, hang=True)
    proc = _driver(tmp_path, ["@architect", "long task"], agents, log,
                   json.loads(os.environ["STUB_SCRIPT"]), idle_limit=0.5, poll_interval=0.01)
    out, err = proc.communicate(timeout=45)
    assert proc.returncode == 1, out + err
    record = _record(tmp_path)
    assert record["status"] == "failed"
    assert "sent nothing" in open(record["answer"]).read()
    assert not codex_run._alive(record["server_pid"])
    assert any(r.get("method") == "turn/interrupt" for r in _requests(log))


def test_a_failed_interrupt_is_reported_before_the_server_is_terminated(tmp_path, monkeypatch):
    agents = _pin_agents(monkeypatch, tmp_path, ["architect"])
    log = _stub_codex(monkeypatch, tmp_path, hang=True,
                      interrupt_error="control channel stopped answering")
    proc = _driver(tmp_path, ["@architect", "long task"], agents, log,
                   json.loads(os.environ["STUB_SCRIPT"]), idle_limit=0.5, poll_interval=0.01)
    out, err = proc.communicate(timeout=45)

    assert proc.returncode == 1, out + err
    record = _record(tmp_path)
    assert "cannot interrupt turn: control channel stopped answering" in open(record["answer"]).read()
    assert not codex_run._alive(record["server_pid"])
    assert any(r.get("method") == "turn/interrupt" for r in _requests(log))


def test_cancelling_a_finished_job_is_a_no_op(monkeypatch, tmp_path, capsys):
    job = _founding_run(monkeypatch, tmp_path)
    capsys.readouterr()
    assert codex_run.main(["cancel", job]) == 0
    assert "already ok" in capsys.readouterr().out


def test_failed_forced_cancellation_does_not_claim_the_runner_was_killed(monkeypatch, tmp_path, capsys):
    record = {"job": "codex-run-cancel", "agent": "architect", "status": "running",
              "phase": "thinking", "pid": 123, "server_pid": 456,
              "record": str(tmp_path / "codex-run-cancel.json")}
    codex_run._save_record(record)
    monkeypatch.setattr(codex_run, "_find_job", lambda _: (record, ""))
    monkeypatch.setattr(codex_run, "_alive", lambda _: True)
    monkeypatch.setattr(codex_run, "_terminate_tree", lambda _: True)
    monkeypatch.setattr(codex_run.time, "sleep", lambda _: None)

    def kill(_, sig):
        if sig == codex_run.signal.SIGKILL:
            raise OSError("operation not permitted")

    monkeypatch.setattr(codex_run.os, "kill", kill)
    assert codex_run.main(["cancel", record["job"]]) == 1
    out = capsys.readouterr().out
    assert "cannot force-cancel" in out and "killed the codex process tree" not in out
    assert _record(tmp_path)["status"] == "failed"


def test_forced_cancellation_writes_a_terminal_record(monkeypatch, tmp_path, capsys):
    record = {"job": "codex-run-cancel", "agent": "architect", "status": "running",
              "phase": "thinking", "pid": 123, "server_pid": 456,
              "record": str(tmp_path / "codex-run-cancel.json")}
    codex_run._save_record(record)
    monkeypatch.setattr(codex_run, "_find_job", lambda _: (record, ""))
    monkeypatch.setattr(codex_run, "_alive", lambda _: True)
    monkeypatch.setattr(codex_run, "_terminate_tree", lambda _: True)
    monkeypatch.setattr(codex_run.time, "sleep", lambda _: None)
    monkeypatch.setattr(codex_run.os, "kill", lambda *_: None)
    assert codex_run.main(["cancel", record["job"]]) == 1
    assert _record(tmp_path)["status"] == "cancelled"


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


def _wait_for_record_field(tmp_path, field, value, seconds):
    deadline = time.time() + seconds
    while time.time() < deadline:
        for path in tmp_path.glob("codex-run-*.json"):
            try:
                record = json.loads(path.read_text())
            except (OSError, ValueError):
                continue
            if record.get(field) == value:
                return record
        time.sleep(0.02)
    raise AssertionError("no record with %s=%r appeared under %s" % (field, value, tmp_path))


# --- the whole path in one process, nothing monkeypatched ---------------------------

def test_a_run_and_its_resume_go_end_to_end_through_the_real_launcher(tmp_path, monkeypatch):
    """Every other test here drives main() in-process, so a crash in module import
    or argv handling passes the suite while every real run dies. This one runs the
    launcher as a subprocess twice — a founding run and a resume of its job — so
    any exception anywhere is a non-zero exit with a traceback."""
    agents = tmp_path / "agents"
    agents.mkdir()
    (agents / "researcher.prompt.md").write_text("You are a researcher.\n")
    (agents / "researcher.md").write_text("---\nname: researcher\n---\n\nbody\n")
    binv = tmp_path / "bin"
    binv.mkdir()
    (binv / "codex").write_text(_STUB)
    (binv / "codex").chmod(0o755)
    log = tmp_path / "requests.jsonl"
    script = {"thread": "th_e2e", "items": [_message("the answer")]}

    first = _driver(tmp_path, ["@researcher", "start"], agents, log, script)
    out, err = first.communicate(timeout=120)
    assert first.returncode == 0, out + err
    assert "the answer" in out and "Traceback" not in err
    job = next(line.split(None, 1)[1].strip() for line in out.splitlines()
               if line.startswith("job:"))

    second = _driver(tmp_path, ["resume", job, "continue"], agents, log, script)
    out, err = second.communicate(timeout=120)
    assert second.returncode == 0, out + err
    assert "agent:   researcher" in out and "Traceback" not in err
    assert _sent(log, "thread/resume")["threadId"] == "th_e2e"


def test_dash_prompt_reads_stdin(monkeypatch, tmp_path):
    """`-` reads the prompt from stdin, so shell quoting cannot mangle the run."""
    import io
    _pin_agents(monkeypatch, tmp_path, ["architect"])
    log = _stub_codex(monkeypatch, tmp_path)
    monkeypatch.setattr(sys, "stdin", io.StringIO("multi\nline 'quoted' $prompt"))
    assert codex_run.main(["@architect", "-"]) == 0
    assert _sent(log, "turn/start")["input"][0]["text"] == "multi\nline 'quoted' $prompt"


# --- @<agent> resolves only to the named-agent allowlist, never outside it ----------

def test_known_agent_resolves(monkeypatch, tmp_path):
    _pin_agents(monkeypatch, tmp_path, ["architect", "code-reviewer"])
    path = codex_run._resolve_agent("@architect")
    assert path is not None and path.endswith("architect.prompt.md")


def test_profile_only_agent_resolves(monkeypatch, tmp_path):
    """A profile is its own config root with its own roster. An agent that exists
    only there is a real agent while that profile is active, so it runs."""
    _pin_agents(monkeypatch, tmp_path, ["ponytail"])
    profile = _pin_profile(monkeypatch, tmp_path, ["copy-chief"])
    assert "copy-chief" in codex_run._available_agents()
    assert codex_run._resolve_agent("@copy-chief") == str(profile / "copy-chief.prompt.md")


def test_shared_agent_still_resolves_from_inside_a_profile(monkeypatch, tmp_path):
    """Entering a profile adds a roster rather than losing one."""
    shared = _pin_agents(monkeypatch, tmp_path, ["ponytail"])
    _pin_profile(monkeypatch, tmp_path, ["copy-chief"])
    assert codex_run._resolve_agent("@ponytail") == str(shared / "ponytail.prompt.md")


def test_name_in_two_rosters_resolves_to_the_active_root(monkeypatch, tmp_path):
    """The profile's copy governs while the profile is active, matching which
    definition governs on the Claude side."""
    _pin_agents(monkeypatch, tmp_path, ["researcher"])
    profile = _pin_profile(monkeypatch, tmp_path, ["researcher"])
    assert codex_run._resolve_agent("@researcher") == str(profile / "researcher.prompt.md")
    assert codex_run._available_agents().count("researcher") == 1


def test_declaration_comes_from_the_roster_that_supplied_the_instructions(monkeypatch, tmp_path):
    """Reading the instructions from one roster and the declaration from another
    would run a profile agent under the shared agent's memory posture."""
    shared = _pin_agents(monkeypatch, tmp_path, ["researcher"])
    (shared / "researcher.md").write_text("---\nname: researcher\nmemory: none\n---\n\nbody\n")
    profile = _pin_profile(monkeypatch, tmp_path, ["researcher"])
    (profile / "researcher.md").write_text("---\nname: researcher\nmemory: user\n---\n\nbody\n")
    assert codex_run._declares_blank_memory("researcher") is False


def test_codex_model_comes_from_the_roster_that_supplied_the_instructions(monkeypatch, tmp_path):
    shared = _pin_agents(monkeypatch, tmp_path, ["researcher"])
    (shared / "researcher.md").write_text(
        "---\nname: researcher\ncodex-model: gpt-5.6-sol\n---\n\nbody\n")
    profile = _pin_profile(monkeypatch, tmp_path, ["researcher"])
    (profile / "researcher.md").write_text(
        "---\nname: researcher\ncodex-model: gpt-5.6-luna\n---\n\nbody\n")
    assert codex_run._codex_model("researcher") == "gpt-5.6-luna"


def test_declared_model_is_not_rewritten(monkeypatch, tmp_path):
    """The value reaches codex as written — this module does not own that key's
    vocabulary, so it lowercases nothing."""
    _pin_agents(monkeypatch, tmp_path, ["research-judge"], frontmatter="codex-model: GPT-5.6-Luna")
    assert codex_run._codex_model("research-judge") == "GPT-5.6-Luna"


def test_default_root_and_shared_roster_collapse_to_one(monkeypatch, tmp_path):
    """~/.claude/agents is a symlink to the shared roster, so the two candidates are
    one directory and must not be walked twice."""
    shared = _pin_agents(monkeypatch, tmp_path, ["ponytail"])
    root = tmp_path / "default-root"
    root.mkdir()
    (root / "agents").symlink_to(shared)
    monkeypatch.setenv("CLAUDE_CONFIG_DIR", str(root))
    assert codex_run._available_agents() == ["ponytail"]
    assert len(codex_run._roster_dirs()) == 1


def test_path_traversal_agent_is_unknown(monkeypatch, tmp_path):
    # A name carrying path segments must not escape the named-agent set, even if a
    # prompt.md happens to exist at the traversed location.
    _pin_agents(monkeypatch, tmp_path, ["architect"])
    (tmp_path / "architect.prompt.md").write_text("attacker instructions")
    assert codex_run._resolve_agent("@../architect") is None
    assert codex_run._resolve_agent("@../../etc/passwd") is None


def test_unknown_agent_dispatch_lists_available(monkeypatch, tmp_path, capsys):
    _pin_agents(monkeypatch, tmp_path, ["architect", "code-reviewer"])
    rc = codex_run.main(["@../../something", "do x"])
    out = capsys.readouterr().out
    assert rc == 1
    assert "unknown agent" in out
    assert "architect" in out and "code-reviewer" in out


def test_unreadable_definition_still_denies_memory(tmp_path):
    """The two keys share a parse but not their answer to an unreadable file."""
    directory = tmp_path / "researcher.md"
    directory.mkdir()
    assert agent_memory.denies_memory(str(directory)) is True
    with pytest.raises(ValueError):
        agent_memory.declaration(str(directory), "memory")


def test_unreadable_definition_fails_the_run_rather_than_defaulting(monkeypatch, tmp_path):
    """A declaration that was written and could not be read is a broken agent."""
    agents = _pin_agents(monkeypatch, tmp_path, ["research-judge"])
    (agents / "research-judge.md").write_bytes(
        b"---\nname: research-judge\ncodex-model: gpt-\xff\xfe-luna\n---\n")
    with pytest.raises(UnicodeDecodeError):
        codex_run._codex_model("research-judge")


def test_unreadable_definition_creates_a_failed_record(monkeypatch, tmp_path, capsys):
    agents = _pin_agents(monkeypatch, tmp_path, ["research-judge"])
    (agents / "research-judge.md").write_bytes(b"---\nname: research-judge\ncodex-model: gpt-\xff\n---\n")
    _stub_codex(monkeypatch, tmp_path)
    assert codex_run.main(["@research-judge", "review"]) == 1
    assert "Traceback" not in capsys.readouterr().out
    record = _record(tmp_path)
    assert record["status"] == "failed"
    assert "cannot prepare agent" in record["error"]
