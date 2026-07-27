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
    rc = codex_run._dispatch(None, "do x", "architect", resume_id="th_1")
    out = capsys.readouterr().out
    assert rc == 1
    assert "status:  failed" in out


def test_failure_surfaces_stderr_on_stdout(monkeypatch, tmp_path, capsys):
    # A real failure: non-zero exit with an error on stderr and no answer.
    _stub_codex(monkeypatch, tmp_path, stdout=_NO_ANSWER_STREAM,
                stderr="codex: model request failed: 401 unauthorized", code=1)
    rc = codex_run._dispatch(None, "do x", "architect", resume_id="th_1")
    out = capsys.readouterr().out
    assert rc == 1
    assert "status:  failed" in out
    assert "401 unauthorized" in out


def test_failure_surfaces_stderr_on_disk(monkeypatch, tmp_path, capsys):
    _stub_codex(monkeypatch, tmp_path, stdout=_NO_ANSWER_STREAM,
                stderr="codex: model request failed: 401 unauthorized", code=1)
    codex_run._dispatch(None, "do x", "architect", resume_id="th_1")
    out = capsys.readouterr().out
    disk_path = next(ln.split(None, 1)[1].strip()
                     for ln in out.splitlines() if ln.startswith("output:"))
    assert "401 unauthorized" in open(disk_path).read()


def test_answer_is_ok(monkeypatch, tmp_path, capsys):
    _stub_codex(monkeypatch, tmp_path, stdout=_ANSWER_STREAM, code=0)
    rc = codex_run._dispatch(None, "do x", "architect", resume_id="th_1")
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
    rc = codex_run._dispatch(None, "do x", "architect", resume_id="th_1")
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
        target=lambda: result.update(rc=codex_run._dispatch(None, "do x", "architect", resume_id="th_1")))
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
    rc = codex_run._dispatch(None, "continue", "architect", resume_id="th_REQUESTED")
    out = capsys.readouterr().out
    assert rc == 1
    assert "status:  failed" in out
    assert "DID NOT RESUME" in out
    assert "th_FRESH" in out and "th_REQUESTED" in out


def test_resume_same_thread_is_ok(monkeypatch, tmp_path, capsys):
    _stub_codex(monkeypatch, tmp_path, stdout=_ANSWER_STREAM, code=0)
    rc = codex_run._dispatch(None, "continue", "architect", resume_id="th_1")
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
                        lambda path, prompt, agent, resume_id=None, blank_memory=False:
                        captured.update(prompt=prompt) or 0)
    monkeypatch.setattr(sys, "stdin", io.StringIO("multi\nline 'quoted' $prompt"))
    rc = codex_run.main(["@architect", "-"])
    assert rc == 0
    assert captured["prompt"] == "multi\nline 'quoted' $prompt"


# --- @<agent> resolves only to the named-agent allowlist, never outside it --------

def _pin_agents(monkeypatch, tmp_path, names):
    """Point the shared roster at a tmp dir holding <name>.prompt.md for each name.

    CLAUDE_CONFIG_DIR is pinned at an empty root so the active-root roster
    contributes nothing and the real ~/.claude never leaks into a test.
    """
    agents = tmp_path / "agents"
    agents.mkdir()
    for name in names:
        (agents / (name + ".prompt.md")).write_text("instructions for %s" % name)
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


def test_known_agent_resolves(monkeypatch, tmp_path):
    _pin_agents(monkeypatch, tmp_path, ["architect", "code-reviewer"])
    path = codex_run._resolve_agent("@architect")
    assert path is not None and path.endswith("architect.prompt.md")


def test_profile_only_agent_resolves(monkeypatch, tmp_path):
    """A profile is its own config root with its own roster. An agent that exists
    only there was unreachable on codex; it is a real agent while that profile is
    active, so it runs."""
    _pin_agents(monkeypatch, tmp_path, ["ponytail"])
    profile = _pin_profile(monkeypatch, tmp_path, ["copy-chief"])
    assert "copy-chief" in codex_run._available_agents()
    assert codex_run._resolve_agent("@copy-chief") == str(profile / "copy-chief.prompt.md")


def test_shared_agent_still_resolves_from_inside_a_profile(monkeypatch, tmp_path):
    """Entering a profile adds a roster rather than losing one — the shared
    roster stays reachable behind it."""
    shared = _pin_agents(monkeypatch, tmp_path, ["ponytail"])
    _pin_profile(monkeypatch, tmp_path, ["copy-chief"])
    assert codex_run._resolve_agent("@ponytail") == str(shared / "ponytail.prompt.md")


def test_name_in_two_rosters_resolves_to_the_active_root(monkeypatch, tmp_path):
    """The profile's copy governs while the profile is active, matching which
    definition governs on the Claude side. Silently running the shared one under
    a profile name is the failure this ordering prevents."""
    _pin_agents(monkeypatch, tmp_path, ["researcher"])
    profile = _pin_profile(monkeypatch, tmp_path, ["researcher"])
    assert codex_run._resolve_agent("@researcher") == str(profile / "researcher.prompt.md")
    assert codex_run._available_agents().count("researcher") == 1


def test_declaration_comes_from_the_roster_that_supplied_the_instructions(monkeypatch, tmp_path):
    """A profile's copy of an agent carries the profile's declaration. Reading the
    instructions from one roster and the declaration from another would run a
    profile agent under the shared agent's memory posture."""
    shared = _pin_agents(monkeypatch, tmp_path, ["researcher"])
    (shared / "researcher.md").write_text("---\nname: researcher\nmemory: none\n---\n\nbody\n")
    profile = _pin_profile(monkeypatch, tmp_path, ["researcher"])
    (profile / "researcher.md").write_text("---\nname: researcher\nmemory: user\n---\n\nbody\n")
    assert codex_run._declares_blank_memory("researcher") is False


def test_default_root_and_shared_roster_collapse_to_one(monkeypatch, tmp_path):
    """~/.claude/agents is a symlink to the shared roster, so the two candidates
    are one directory and must not be walked twice."""
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
    outside = tmp_path / "architect.prompt.md"  # one level up from AGENTS_DIR
    outside.write_text("attacker instructions")
    assert codex_run._resolve_agent("@../architect") is None
    assert codex_run._resolve_agent("@../../etc/passwd") is None


# --- a resume runs as the agent that founded the thread ---------------------------

def test_resume_runs_end_to_end_through_the_real_launcher(tmp_path, monkeypatch):
    """The whole resume path in one process, with nothing in it monkeypatched.

    Every other test here replaces `_run` or `_dispatch`, so a crash in the
    recovery path itself — a NameError on the marker constant, say — passes the
    entire suite while every real resume dies. This one runs the `codex-run`
    launcher as a subprocess against a stub codex, so the module is imported
    fresh, the argv is the real one, and any exception anywhere between the
    launcher and the answer is a non-zero exit with a traceback."""
    import json
    import subprocess

    agents = tmp_path / "agents"
    agents.mkdir()
    (agents / "researcher.prompt.md").write_text(
        "<!-- codex-run agent: researcher -->\n\nYou are a researcher.\n")
    (agents / "researcher.md").write_text("---\nname: researcher\n---\n\nbody\n")

    day = tmp_path / "codex" / "sessions" / "2026" / "07" / "27"
    day.mkdir(parents=True)
    (day / "rollout-2026-07-27T11-00-04-th_e2e.jsonl").write_text(json.dumps({
        "type": "session_meta",
        "payload": {"session_id": "th_e2e", "base_instructions": {
            "text": "<!-- codex-run agent: researcher -->\n\nYou are a researcher.\n"}},
    }) + "\n")

    binv = tmp_path / "bin"
    binv.mkdir()
    stub = binv / "codex"
    stub.write_text("#!/bin/bash\ncat <<'__OUT__'\n%s\n__OUT__\n"
                    % _ANSWER_STREAM.replace('"th_1"', '"th_e2e"'))
    stub.chmod(0o755)

    # Pin AGENTS_DIR, CODEX_HOME and the output dir from outside, then hand the
    # real main() the real argv.
    driver = tmp_path / "driver.py"
    driver.write_text(
        "import sys\n"
        "sys.path.insert(0, %r)\n"
        "sys.path.insert(0, %r)\n"
        "import codex_run\n"
        "codex_run.AGENTS_DIR = %r\n"
        "codex_run.CODEX_HOME = %r\n"
        "codex_run._resolve_output_dir = lambda: %r\n"
        "sys.exit(codex_run.main(['resume', 'th_e2e', 'continue']))\n"
        % (os.path.join(PY_HOOKS, "lib"), PY_HOOKS, str(agents),
           str(tmp_path / "codex"), str(tmp_path)))

    env = dict(os.environ, PATH="%s:%s" % (binv, os.environ["PATH"]))
    proc = subprocess.run([sys.executable, str(driver)], capture_output=True, text=True, env=env)
    assert proc.returncode == 0, proc.stdout + proc.stderr
    assert "the answer" in proc.stdout
    assert "agent:   researcher" in proc.stdout
    assert "Traceback" not in proc.stderr



def _pin_rollout(monkeypatch, tmp_path, session, instructions, archived=False):
    """Write a codex rollout for `session` whose session_meta carries
    `instructions`, and point the lookup at it."""
    import json
    if archived:
        day = tmp_path / "codex" / "archived_sessions"
    else:
        day = tmp_path / "codex" / "sessions" / "2026" / "07" / "27"
    day.mkdir(parents=True, exist_ok=True)
    rollout = day / ("rollout-2026-07-27T11-00-04-%s.jsonl" % session)
    rollout.write_text(json.dumps({
        "type": "session_meta",
        "payload": {"session_id": session, "base_instructions": {"text": instructions}},
    }) + "\n")
    monkeypatch.setattr(codex_run, "CODEX_HOME", str(tmp_path / "codex"))
    return rollout


def _no_codex(monkeypatch):
    monkeypatch.setattr(codex_run, "_run",
                        lambda cmd, events: (_ for _ in ()).throw(AssertionError("codex ran")))


def test_founding_agent_is_recovered_from_the_rollout(monkeypatch, tmp_path, capsys):
    """The wrapper works the identity out itself, from codex's own record of the
    founding run: a resume — which takes a session and a message and nothing else
    — runs under the instructions the thread was founded on."""
    _pin_agents(monkeypatch, tmp_path, ["researcher", "architect"])
    _pin_rollout(monkeypatch, tmp_path, "th_1", "instructions for researcher")
    cmds = []
    monkeypatch.setattr(codex_run, "_run",
                        lambda cmd, events: cmds.append(cmd) or (0, "answer", "th_1", False, ""))
    monkeypatch.setattr(codex_run, "_output_paths",
                        lambda: (str(tmp_path / "a.txt"), str(tmp_path / "e.jsonl")))

    assert codex_run._founding_agent("th_1") == "researcher"
    assert codex_run.main(["resume", "th_1", "continue"]) == 0
    instructions = "model_instructions_file=%s/agents/researcher.prompt.md" % tmp_path
    assert instructions in cmds[0]
    assert "agent:   researcher" in capsys.readouterr().out


def test_archived_thread_resolves(monkeypatch, tmp_path):
    """codex moves old threads to archived_sessions; the record is still its
    record, so recovery follows it there."""
    _pin_agents(monkeypatch, tmp_path, ["researcher"])
    _pin_rollout(monkeypatch, tmp_path, "th_old", "instructions for researcher", archived=True)
    assert codex_run._founding_agent("th_old") == "researcher"


def test_marker_names_the_founding_agent(monkeypatch, tmp_path, capsys):
    """A thread founded after the marker existed identifies its agent by the
    recorded name, not by its prose — the whole body can be reworded and the
    thread still resumes as the agent that founded it."""
    agents = _pin_agents(monkeypatch, tmp_path, ["researcher", "architect"])
    (agents / "researcher.prompt.md").write_text(
        "<!-- codex-run agent: researcher -->\n\nA completely rewritten body.\n")
    _pin_rollout(monkeypatch, tmp_path, "th_1",
                 "<!-- codex-run agent: researcher -->\n\nThe body it was founded on.\n")
    cmds = []
    monkeypatch.setattr(codex_run, "_run",
                        lambda cmd, events: cmds.append(cmd) or (0, "answer", "th_1", False, ""))
    monkeypatch.setattr(codex_run, "_output_paths",
                        lambda: (str(tmp_path / "a.txt"), str(tmp_path / "e.jsonl")))

    assert codex_run._founding_agent("th_1") == "researcher"
    assert codex_run.main(["resume", "th_1", "continue"]) == 0
    assert "model_instructions_file=%s/agents/researcher.prompt.md" % tmp_path in cmds[0]
    assert "agent:   researcher" in capsys.readouterr().out


def test_marker_naming_a_non_roster_agent_stops(monkeypatch, tmp_path, capsys):
    """The name still resolves through the roster allowlist, so a stale or
    tampered marker cannot point a run at anything outside the named set."""
    _pin_agents(monkeypatch, tmp_path, ["researcher"])
    _pin_rollout(monkeypatch, tmp_path, "th_1",
                 "<!-- codex-run agent: ../../attacker -->\n\nbody\n")
    _no_codex(monkeypatch)
    assert codex_run.main(["resume", "th_1", "continue"]) == 1
    assert "unidentifiable" in capsys.readouterr().out


def test_marker_below_the_first_line_does_not_claim_identity(monkeypatch, tmp_path):
    """Only the opening line carries identity — a marker quoted inside a body
    (an agent discussing this mechanism, say) is text, not a claim."""
    _pin_agents(monkeypatch, tmp_path, ["researcher"])
    _pin_rollout(monkeypatch, tmp_path, "th_1",
                 "codex's own default instructions\n\n<!-- codex-run agent: researcher -->\n")
    assert codex_run._founding_agent("th_1") is None


def test_generator_marker_is_read_by_the_resume_path(monkeypatch, tmp_path):
    """The writer and the reader are separate modules in separate runtime trees;
    this pins their spelling of the marker together."""
    import agents as agents_generator

    agents_dir = tmp_path / "agents"
    agents_dir.mkdir()
    (tmp_path / "skills").mkdir()
    (agents_dir / "researcher.md").write_text(
        "---\nname: researcher\ndescription: d\n---\n\nYou are a researcher.\n")
    agents_generator.generate(str(agents_dir))
    generated = (agents_dir / "researcher.prompt.md").read_text()

    assert codex_run._marked_agent(generated) == "researcher"
    assert codex_run._without_marker(generated) == "You are a researcher."


def test_regenerated_prompt_still_resolves_on_the_first_line(monkeypatch, tmp_path):
    """Regeneration rewrites the body and leaves the Frame opener alone, so a
    thread founded before its agent was last regenerated no longer matches
    verbatim. The first line still identifies it."""
    agents = _pin_agents(monkeypatch, tmp_path, ["researcher"])
    (agents / "researcher.prompt.md").write_text(
        "You are a researcher.\n\n## Principles\n\n- a regenerated body\n")
    _pin_rollout(monkeypatch, tmp_path, "th_1",
                 "You are a researcher.\n\n## Principles\n\n- the body it was founded on\n")
    assert codex_run._founding_agent("th_1") == "researcher"


def test_pre_marker_thread_resolves_against_a_marked_corpus(monkeypatch, tmp_path):
    """The 2,257 threads founded before the marker carry no name. The corpus now
    opens with one, so the fallback compares with it stripped — otherwise the
    marker's arrival would itself have moved the opener out from under every one
    of them."""
    agents = _pin_agents(monkeypatch, tmp_path, ["researcher"])
    (agents / "researcher.prompt.md").write_text(
        "<!-- codex-run agent: researcher -->\n\nYou are a researcher.\n\n- a regenerated body\n")
    _pin_rollout(monkeypatch, tmp_path, "th_old",
                 "You are a researcher.\n\n- the body it was founded on\n")
    assert codex_run._founding_agent("th_old") == "researcher"


def test_first_line_shared_by_two_agents_resolves_to_nothing(monkeypatch, tmp_path):
    """The fallback identifies only when it identifies uniquely — a first line two
    agents claim is not evidence of either."""
    agents = _pin_agents(monkeypatch, tmp_path, ["researcher", "explorer"])
    for name in ("researcher", "explorer"):
        (agents / (name + ".prompt.md")).write_text("Shared opener.\n\nbody for %s\n" % name)
    _pin_rollout(monkeypatch, tmp_path, "th_1", "Shared opener.\n\nsome other body\n")
    assert codex_run._founding_agent("th_1") is None


def test_resume_of_an_unidentifiable_thread_stops(monkeypatch, tmp_path, capsys):
    """Continuing as codex's global default instead of the founding agent is the
    defect being removed, so an unresolvable session fails instead of running."""
    _pin_agents(monkeypatch, tmp_path, ["researcher"])
    _pin_rollout(monkeypatch, tmp_path, "th_1", "instructions for researcher")
    _no_codex(monkeypatch)
    rc = codex_run.main(["resume", "th_UNKNOWN", "continue"])
    out = capsys.readouterr().out
    assert rc == 1
    assert "cannot resume th_UNKNOWN" in out


def test_resume_of_a_bare_codex_thread_stops(monkeypatch, tmp_path, capsys):
    """A thread founded outside this wrapper carries instructions no agent owns,
    so it is unidentifiable and the run stops."""
    _pin_agents(monkeypatch, tmp_path, ["researcher"])
    _pin_rollout(monkeypatch, tmp_path, "th_1", "codex's own default instructions")
    _no_codex(monkeypatch)
    assert codex_run.main(["resume", "th_1", "continue"]) == 1
    assert "unidentifiable" in capsys.readouterr().out


def test_unknown_agent_dispatch_lists_available(monkeypatch, tmp_path, capsys):
    _pin_agents(monkeypatch, tmp_path, ["architect", "code-reviewer"])
    rc = codex_run.main(["@../../something", "do x"])
    out = capsys.readouterr().out
    assert rc == 1
    assert "unknown agent" in out
    assert "architect" in out and "code-reviewer" in out
