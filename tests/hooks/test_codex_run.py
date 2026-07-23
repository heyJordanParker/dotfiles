"""Coverage for lib/codex_run.py — the failure detection and error surfacing the
wrapper owns.

The runner shells out to `codex exec` and reads its event stream. These tests
replace `codex` with a PATH stub that emits a chosen event stream on stdout, a
chosen error on stderr, and a chosen exit code, so the runner's failure logic is
exercised with no real codex. Output lands under tmp_path (the resolver is pinned
there), never the real session dir.

The three behaviors under test:
- a run that exits zero but produced no final answer is a failure, not a success;
- a failing run's stderr is surfaced on stdout and persisted on disk, so it is
  diagnosable rather than a bare "[no answer]";
- a normal run with an answer is ok and exits zero.
"""

import os
import sys

from conftest import PY_HOOKS

sys.path.insert(0, os.path.join(PY_HOOKS, "lib"))

from lib import codex_run  # noqa: E402


def _stub_codex(monkeypatch, tmp_path, *, stdout="", stderr="", code=0):
    """Put a fake `codex` first on PATH that prints fixed stdout/stderr and exits
    with a fixed code, and pin output under tmp_path."""
    binv = tmp_path / "bin"
    binv.mkdir(exist_ok=True)
    fake = binv / "codex"
    # printf the streams verbatim, then exit the chosen code.
    fake.write_text(
        "#!/bin/bash\n"
        "cat <<'__OUT__'\n%s\n__OUT__\n"
        "cat <<'__ERR__' >&2\n%s\n__ERR__\n"
        "exit %d\n" % (stdout, stderr, code)
    )
    fake.chmod(0o755)
    monkeypatch.setenv("PATH", "%s:%s" % (binv, os.environ["PATH"]))
    monkeypatch.setattr(codex_run, "_resolve_output_dir", lambda: str(tmp_path))


_ANSWER_STREAM = (
    '{"type":"thread.started","thread_id":"th_1"}\n'
    '{"type":"item.completed","item":{"type":"agent_message","text":"the answer"}}\n'
    '{"type":"turn.completed"}'
)

_NO_ANSWER_STREAM = (
    '{"type":"thread.started","thread_id":"th_1"}\n'
    '{"type":"turn.completed"}'
)


def test_no_answer_zero_exit_is_failure(monkeypatch, tmp_path, capsys):
    # codex exits zero, emits no agent_message — not a success.
    _stub_codex(monkeypatch, tmp_path, stdout=_NO_ANSWER_STREAM, code=0)
    rc = codex_run._dispatch(None, "do x", resume_id="th_1")
    out = capsys.readouterr().out
    assert rc == 1
    assert "status:  failed" in out


def test_failure_surfaces_stderr_on_stdout(monkeypatch, tmp_path, capsys):
    # A real failure: non-zero exit with an error on stderr and no answer.
    _stub_codex(monkeypatch, tmp_path, stdout=_NO_ANSWER_STREAM,
                stderr="codex: model request failed: 401 unauthorized", code=1)
    rc = codex_run._dispatch(None, "do x", resume_id="th_1")
    out = capsys.readouterr().out
    assert rc == 1
    assert "status:  failed" in out
    assert "401 unauthorized" in out


def test_failure_surfaces_stderr_on_disk(monkeypatch, tmp_path, capsys):
    _stub_codex(monkeypatch, tmp_path, stdout=_NO_ANSWER_STREAM,
                stderr="codex: model request failed: 401 unauthorized", code=1)
    codex_run._dispatch(None, "do x", resume_id="th_1")
    out = capsys.readouterr().out
    disk_path = next(ln.split(None, 1)[1].strip()
                     for ln in out.splitlines() if ln.startswith("output:"))
    assert "401 unauthorized" in open(disk_path).read()


def test_answer_is_ok(monkeypatch, tmp_path, capsys):
    _stub_codex(monkeypatch, tmp_path, stdout=_ANSWER_STREAM, code=0)
    rc = codex_run._dispatch(None, "do x", resume_id="th_1")
    out = capsys.readouterr().out
    assert rc == 0
    assert "status:  ok" in out
    assert "the answer" in out


def test_turn_failed_event_is_failure(monkeypatch, tmp_path, capsys):
    # A turn that fails but exits zero still trips failure.
    stream = (
        '{"type":"thread.started","thread_id":"th_1"}\n'
        '{"type":"item.completed","item":{"type":"agent_message","text":"the answer"}}\n'
        '{"type":"turn.failed"}'
    )
    _stub_codex(monkeypatch, tmp_path, stdout=stream, code=0)
    rc = codex_run._dispatch(None, "do x", resume_id="th_1")
    assert rc == 1
    assert "status:  failed" in capsys.readouterr().out


def test_large_stderr_does_not_deadlock(monkeypatch, tmp_path):
    """A failing run that floods stderr before finishing stdout returns promptly —
    it does not hang. The stub writes far more than a pipe buffer holds to stderr
    *before* its final stdout line, so a runner that drained stdout to exhaustion
    before reading stderr would deadlock: codex blocks writing stderr, stops
    producing stdout, and the runner blocks on stdout that never comes. The run is
    executed in a worker thread with a hard join timeout so a regression surfaces
    as a failed assertion instead of hanging the suite."""
    import threading

    binv = tmp_path / "bin"
    binv.mkdir(exist_ok=True)
    fake = binv / "codex"
    # stderr flood first (each line >> pipe buffer in aggregate), THEN the stdout
    # event stream. This ordering is what makes an undrained stderr stall stdout.
    fake.write_text(
        "#!/bin/bash\n"
        "for i in $(seq 1 20000); do echo \"codex error padding line $i\" >&2; done\n"
        "echo 'FATAL: model request failed' >&2\n"
        "cat <<'__OUT__'\n%s\n__OUT__\n"
        "exit 1\n" % _NO_ANSWER_STREAM
    )
    fake.chmod(0o755)
    monkeypatch.setenv("PATH", "%s:%s" % (binv, os.environ["PATH"]))
    monkeypatch.setattr(codex_run, "_resolve_output_dir", lambda: str(tmp_path))

    result = {}
    worker = threading.Thread(
        target=lambda: result.update(rc=codex_run._dispatch(None, "do x", resume_id="th_1")))
    worker.start()
    worker.join(timeout=30)
    assert not worker.is_alive(), "runner deadlocked on a stderr flood — it did not return"
    assert result["rc"] == 1
    disk = next(p for p in tmp_path.glob("*.txt"))
    assert "FATAL: model request failed" in disk.read_text()


def test_resume_into_fresh_thread_is_failure(monkeypatch, tmp_path, capsys):
    # A resume that comes back under a different thread id did not resume — codex
    # started a fresh thread, so the run fails and the trailer says so loudly.
    stream = (
        '{"type":"thread.started","thread_id":"th_FRESH"}\n'
        '{"type":"item.completed","item":{"type":"agent_message","text":"the answer"}}\n'
        '{"type":"turn.completed"}'
    )
    _stub_codex(monkeypatch, tmp_path, stdout=stream, code=0)
    rc = codex_run._dispatch(None, "continue", resume_id="th_REQUESTED")
    out = capsys.readouterr().out
    assert rc == 1
    assert "status:  failed" in out
    assert "DID NOT RESUME" in out
    assert "th_FRESH" in out and "th_REQUESTED" in out


def test_resume_same_thread_is_ok(monkeypatch, tmp_path, capsys):
    _stub_codex(monkeypatch, tmp_path, stdout=_ANSWER_STREAM, code=0)
    rc = codex_run._dispatch(None, "continue", resume_id="th_1")
    out = capsys.readouterr().out
    assert rc == 0
    assert "DID NOT RESUME" not in out


def test_dash_prompt_reads_stdin(monkeypatch, tmp_path, capsys):
    # `-` reads the prompt from stdin, so shell quoting cannot mangle the run.
    import io
    _pin_agents(monkeypatch, tmp_path, ["architect"])
    _stub_codex(monkeypatch, tmp_path, stdout=_ANSWER_STREAM, code=0)
    captured = {}
    monkeypatch.setattr(codex_run, "_dispatch",
                        lambda path, prompt, resume_id=None: captured.update(prompt=prompt) or 0)
    monkeypatch.setattr(sys, "stdin", io.StringIO("multi\nline 'quoted' $prompt"))
    rc = codex_run.main(["@architect", "-"])
    assert rc == 0
    assert captured["prompt"] == "multi\nline 'quoted' $prompt"


# --- @<agent> resolves only to the named-agent allowlist, never outside it --------

def _pin_agents(monkeypatch, tmp_path, names):
    """Point AGENTS_DIR at a tmp dir holding <name>.prompt.md for each name."""
    agents = tmp_path / "agents"
    agents.mkdir()
    for name in names:
        (agents / (name + ".prompt.md")).write_text("instructions for %s" % name)
    monkeypatch.setattr(codex_run, "AGENTS_DIR", str(agents))
    return agents


def test_known_agent_resolves(monkeypatch, tmp_path):
    _pin_agents(monkeypatch, tmp_path, ["architect", "code-reviewer"])
    path = codex_run._resolve_agent("@architect")
    assert path is not None and path.endswith("architect.prompt.md")


def test_path_traversal_agent_is_unknown(monkeypatch, tmp_path):
    # A name carrying path segments must not escape the named-agent set, even if a
    # prompt.md happens to exist at the traversed location.
    _pin_agents(monkeypatch, tmp_path, ["architect"])
    outside = tmp_path / "architect.prompt.md"  # one level up from AGENTS_DIR
    outside.write_text("attacker instructions")
    assert codex_run._resolve_agent("@../architect") is None
    assert codex_run._resolve_agent("@../../etc/passwd") is None


def test_unknown_agent_dispatch_lists_available(monkeypatch, tmp_path, capsys):
    _pin_agents(monkeypatch, tmp_path, ["architect", "code-reviewer"])
    rc = codex_run.main(["@../../something", "do x"])
    out = capsys.readouterr().out
    assert rc == 1
    assert "unknown agent" in out
    assert "architect" in out and "code-reviewer" in out
