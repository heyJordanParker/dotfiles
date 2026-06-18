"""Spine coverage for lib/session_state.py — the per-session state store every
hook reads and writes through.

This guards the store's load-bearing properties in-process: atomic
concurrent increments, corrupt/empty/missing-state healing, subagent nesting and
resolution, the append-only event logs, and their truncation on compaction.

Every test runs against a per-test data root (CLAUDE_DATA_ROOT / CLAUDE_PROJECTS_ROOT
under tmp_path) — nothing touches the real ~/.claude. Time is driven by monkeypatching
the module's two clock functions.
"""

import json
import os
import sys
import threading

import pytest
from conftest import PY_HOOKS

# The module under test imports as a bare `session_state` from its own lib dir
# (model_call._record does `import session_state`); put that dir on the path so
# both styles resolve to the one module.
sys.path.insert(0, os.path.join(PY_HOOKS, "lib"))

import session_state  # noqa: E402


@pytest.fixture
def root(tmp_path, monkeypatch):
    """Isolate the store under tmp_path and silence stderr noise from expected
    error paths. Returns the data root."""
    data_root = tmp_path / "claude"
    projects_root = tmp_path / "projects"
    projects_root.mkdir(parents=True)
    monkeypatch.setenv("CLAUDE_DATA_ROOT", str(data_root))
    monkeypatch.setenv("CLAUDE_PROJECTS_ROOT", str(projects_root))
    for var in ("AGENT_SESSION_ID", "CODEX_THREAD_ID", "CLAUDE_CODE_SESSION_ID"):
        monkeypatch.delenv(var, raising=False)
    return data_root


@pytest.fixture
def clock(monkeypatch):
    """Drive session_state's two clock functions deterministically. Returns a
    setter; the ISO string is derived from the epoch so timestamps stay ordered."""
    state = {"now": 1000}

    def _set(value):
        state["now"] = value

    def _iso():
        return "1970-01-01T00:00:%02dZ" % (state["now"] % 60)

    monkeypatch.setattr(session_state, "_now", lambda: state["now"])
    monkeypatch.setattr(session_state, "_iso_now", _iso)
    return _set


# the CLI dispatch under test; short alias keeps call sites tight
_run = session_state.main


def _state(root, *parts):
    return os.path.join(str(root), "sessions", *parts, "state.json")


def _read(path):
    with open(path) as fh:
        return json.load(fh)


def _stage_transcript(root_projects, agent_id, parent_id, project="proj"):
    """Mirror the harness's stage_subagent_transcript: write the empty subagent
    transcript that _resolve_parent_id globs for."""
    path = os.path.join(str(root_projects), project, parent_id,
                        "subagents", agent_id + ".jsonl")
    os.makedirs(os.path.dirname(path), exist_ok=True)
    open(path, "w").close()
    return path


# ---------------------------------------------------------------------------
# Initialization + default schema
# ---------------------------------------------------------------------------

def test_start_creates_main_state_with_defaults(root, clock):
    assert _run(["start", "main-1", "--transcript-path", "/foo/main-1.jsonl"]) == 0
    st = _read(_state(root, "main-1"))
    assert st["role"] == "main"
    assert st["session_id"] == "main-1"
    assert st["parent_session_id"] is None
    assert st["approach"] == "subagents"
    assert st["state"] == "proposing"
    assert st["intent"] == "instructions"
    assert st["commit_requested"] is False
    assert st["notes"] == []
    assert st["validation_phase"] == 0
    assert st["pane"] is None
    assert st["tmux-pane"] is None
    assert st["schema_version"] == 1


def test_start_is_idempotent(root, clock):
    _run(["start", "m", "--transcript-path", "/foo/m.jsonl"])
    _run(["set", "m", "approach", "team"])
    _run(["start", "m", "--transcript-path", "/foo/m.jsonl"])
    assert session_state.cmd_get(["m", "approach"]) == 0  # exit only; value below
    st = _read(_state(root, "m"))
    assert st["approach"] == "team"  # re-start did not clobber the mutation


def test_start_records_session_start_once(root, clock):
    clock(4242)
    _run(["start", "m", "--transcript-path", "/foo/m.jsonl"])
    assert _read(_state(root, "m"))["session_start"] == 4242
    clock(9999)
    _run(["start", "m", "--transcript-path", "/foo/m.jsonl"])
    assert _read(_state(root, "m"))["session_start"] == 4242  # not overwritten


def test_start_without_transcript_is_main(root, clock):
    _run(["start", "m", "--transcript-path", ""])
    assert _read(_state(root, "m"))["role"] == "main"


# ---------------------------------------------------------------------------
# Subagent resolution + nesting (DoD: agent-<id> nests under parent; main does not)
# ---------------------------------------------------------------------------

def test_subagent_nests_under_parent(root, clock):
    _run(["start", "parent", "--transcript-path", "/p/parent/parent.jsonl"])
    _run(["start", "agent-xyz",
                        "--transcript-path", "/p/parent/subagents/agent-xyz.jsonl"])
    nested = _state(root, "parent", "subagents", "agent-xyz")
    assert os.path.isfile(nested)
    st = _read(nested)
    assert st["role"] == "subagent"
    assert st["session_id"] == "agent-xyz"
    assert st["parent_session_id"] == "parent"
    # subagent state omits the classifier-only fields
    for omitted in ("approach", "state", "intent", "notes",
                    "validation_phase", "commit_requested"):
        assert omitted not in st


def test_main_session_does_not_nest(root, clock):
    _run(["start", "main-1", "--transcript-path", "/foo/main-1.jsonl"])
    assert os.path.isfile(_state(root, "main-1"))  # flat, top-level
    assert not os.path.isdir(os.path.join(str(root), "sessions", "main-1", "subagents"))


def test_subagent_resolves_parent_via_staged_transcript(root, clock):
    """A lazy-create on an agent-* with no explicit --transcript-path resolves the
    parent by globbing the staged transcript under CLAUDE_PROJECTS_ROOT."""
    _run(["start", "lc-parent", "--transcript-path", "/p/lc-parent/lc-parent.jsonl"])
    _stage_transcript(root.parent / "projects", "agent-lcsub", "lc-parent")
    assert _run(["set", "agent-lcsub", "approach", "team"]) == 0
    nested = _state(root, "lc-parent", "subagents", "agent-lcsub")
    assert os.path.isfile(nested)
    st = _read(nested)
    assert st["role"] == "subagent"
    assert st["parent_session_id"] == "lc-parent"
    assert st["approach"] == "team"


def test_orphan_agent_without_parent_fails_loud(root, clock, capsys):
    rc = _run(["set", "agent-orphan", "approach", "team"])
    assert rc == 1
    assert "no resolvable parent" in capsys.readouterr().err
    assert not os.path.isdir(os.path.join(str(root), "sessions", "agent-orphan"))


def test_out_of_order_subagent_then_parent(root, clock):
    """Subagent starts before its parent: parent dir is created, parent state.json
    only materializes when the parent itself starts; the subagent survives that."""
    _run(["start", "agent-ooo",
                        "--transcript-path", "/p/main-300/subagents/agent-ooo.jsonl"])
    assert os.path.isfile(_state(root, "main-300", "subagents", "agent-ooo"))
    assert not os.path.isfile(_state(root, "main-300"))
    _run(["start", "main-300", "--transcript-path", "/p/main-300/main-300.jsonl"])
    assert os.path.isfile(_state(root, "main-300"))
    assert os.path.isfile(_state(root, "main-300", "subagents", "agent-ooo"))


def test_grandchild_subagent_rejected(root, clock):
    """Two /subagents/ segments → the parser rejects and the glob finds nothing,
    so init fails rather than fabricating a nested grandchild."""
    _run(["start", "gc-main", "--transcript-path", "/p/gc-main/gc-main.jsonl"])
    _run(["start", "agent-gc1",
          "--transcript-path", "/p/gc-main/subagents/agent-gc1.jsonl"])
    rc = _run(["start", "agent-gc2", "--transcript-path",
               "/p/gc-main/subagents/agent-gc1/subagents/agent-gc2.jsonl"])
    assert rc == 1


# ---------------------------------------------------------------------------
# Healing: corrupt / empty / missing state.json recovers to defaults (DoD)
# ---------------------------------------------------------------------------

def _seed_session_dir(root, sid):
    d = os.path.join(str(root), "sessions", sid)
    os.makedirs(d, exist_ok=True)
    return os.path.join(d, "state.json")


def test_set_heals_corrupt_state(root, clock):
    path = _seed_session_dir(root, "rc")
    with open(path, "w") as fh:
        fh.write("this is not json")
    assert _run(["set", "rc", "approach", "team"]) == 0
    st = _read(path)
    assert st["role"] == "main"
    assert st["session_id"] == "rc"
    assert st["approach"] == "team"


def test_merge_heals_corrupt_state(root, clock):
    path = _seed_session_dir(root, "rc2")
    with open(path, "w") as fh:
        fh.write("garbage")
    assert _run(["merge", "rc2", '{"approach":"team","intent":"approval"}']) == 0
    st = _read(path)
    assert st["approach"] == "team"
    assert st["intent"] == "approval"


def test_set_heals_empty_state(root, clock):
    path = _seed_session_dir(root, "empty")
    open(path, "w").close()
    assert _run(["set", "empty", "approach", "team"]) == 0
    st = _read(path)
    assert st["approach"] == "team"
    assert st["role"] == "main"


def test_set_heals_array_shaped_state(root, clock):
    path = _seed_session_dir(root, "arr")
    with open(path, "w") as fh:
        fh.write("[1,2,3]")
    assert _run(["set", "arr", "approach", "team"]) == 0
    st = _read(path)
    assert isinstance(st, dict)
    assert st["approach"] == "team"


def test_set_on_missing_state_lazy_creates(root, clock):
    assert _run(["set", "fresh", "approach", "team"]) == 0
    st = _read(_state(root, "fresh"))
    assert st["role"] == "main"
    assert st["approach"] == "team"


def test_start_heals_corrupt_state(root, clock):
    path = _seed_session_dir(root, "heal")
    with open(path, "w") as fh:
        fh.write("garbage not json")
    assert _run(["start", "heal", "--transcript-path", "/foo/heal.jsonl"]) == 0
    st = _read(path)
    assert st["role"] == "main"
    assert st["session_id"] == "heal"


def test_get_on_corrupt_state_is_soft(root, clock, capsys):
    path = _seed_session_dir(root, "soft")
    with open(path, "w") as fh:
        fh.write("not json")
    rc = _run(["get", "soft", "approach"])
    assert rc == 0
    assert capsys.readouterr().out == ""  # no value, no heal on a read


# ---------------------------------------------------------------------------
# set / merge type fidelity + field isolation
# ---------------------------------------------------------------------------

def test_set_parses_json_scalars(root, clock):
    _run(["start", "s", "--transcript-path", "/foo/s.jsonl"])
    _run(["set", "s", "commit_requested", "true"])
    _run(["set", "s", "validation_phase", "3"])
    st = _read(_state(root, "s"))
    assert st["commit_requested"] is True
    assert st["validation_phase"] == 3


def test_set_falls_back_to_string(root, clock):
    _run(["start", "s", "--transcript-path", "/foo/s.jsonl"])
    _run(["set", "s", "approach", "with multiple words"])
    assert _read(_state(root, "s"))["approach"] == "with multiple words"


def test_set_preserves_other_fields(root, clock):
    _run(["start", "s", "--transcript-path", "/foo/s.jsonl"])
    _run(["set", "s", "approach", "team"])
    assert _read(_state(root, "s"))["state"] == "proposing"


def test_merge_rejects_non_object(root, clock):
    _run(["start", "m", "--transcript-path", "/foo/m.jsonl"])
    for frag in ('"a-string"', "[1,2,3]", "42", "null", "true"):
        assert _run(["merge", "m", frag]) == 1
    st = _read(_state(root, "m"))
    assert st["approach"] == "subagents"  # untouched by the rejected merges


def test_merge_empty_object_succeeds(root, clock):
    _run(["start", "m", "--transcript-path", "/foo/m.jsonl"])
    assert _run(["merge", "m", "{}"]) == 0


# ---------------------------------------------------------------------------
# session_id validation / traversal guard
# ---------------------------------------------------------------------------

@pytest.mark.parametrize("bad", [
    "../foo", "..//foo", "/etc/passwd", "", "has space", "has;semi",
    "has$dollar", "has`tick", "has*glob", "has?q", "-leading-dash",
])
def test_invalid_session_ids_rejected(root, clock, bad):
    assert _run(["start", bad, "--transcript-path", "/foo/x.jsonl"]) == 1
    assert _run(["set", bad, "approach", "team"]) == 1
    assert _run(["get", bad, "approach"]) == 1
    assert _run(["read", bad, "/x"]) == 1


@pytest.mark.parametrize("good", [
    "12345", "x", "_underscore_first", "MIXED_Case-123",
    "0c76b915-3e91-442d-a033-9900ae991a75",
])
def test_valid_session_ids_accepted(root, clock, good):
    assert _run(["start", good, "--transcript-path", "/foo/x.jsonl"]) == 0


# ---------------------------------------------------------------------------
# get behaviors
# ---------------------------------------------------------------------------

def test_get_missing_session_is_soft(root, clock):
    assert _run(["get", "nope", "approach"]) == 0


def test_get_null_field_emits_nothing(root, clock, capsys):
    _run(["start", "g", "--transcript-path", "/foo/g.jsonl"])
    session_state.cmd_get(["g", "pane"])
    assert capsys.readouterr().out == ""


def test_get_path_modes(root, clock, capsys):
    session_state.cmd_get(["--path", "data-root"])
    assert capsys.readouterr().out.strip() == str(root)
    session_state.cmd_get(["--path", "sessions"])
    assert capsys.readouterr().out.strip() == os.path.join(str(root), "sessions")
    session_state.cmd_get(["--path", "shaping"])
    assert capsys.readouterr().out.strip() == os.path.join(str(root), "shaping")


def test_get_path_resolves_subagent(root, clock, capsys):
    _run(["start", "p", "--transcript-path", "/p/p/p.jsonl"])
    _run(["start", "agent-s", "--transcript-path", "/p/p/subagents/agent-s.jsonl"])
    session_state.cmd_get(["--path", "agent-s"])
    out = capsys.readouterr().out.strip()
    assert out == os.path.join(str(root), "sessions", "p", "subagents", "agent-s")


# ---------------------------------------------------------------------------
# Event logs: reads / skills append correctly, coexist, nest (DoD)
# ---------------------------------------------------------------------------

def _logfile(root, sid, name):
    return os.path.join(str(root), "sessions", sid, name)


def test_read_appends_jsonl(root, clock):
    _run(["start", "a", "--transcript-path", "/foo/a.jsonl"])
    _run(["read", "a", "/some/file.ts"])
    with open(_logfile(root, "a", "reads.jsonl")) as fh:
        entry = json.loads(fh.readline())
    assert entry["path"] == "/some/file.ts"
    assert isinstance(entry["ts"], str)


def test_skill_appends_jsonl_without_path_field(root, clock):
    _run(["start", "a", "--transcript-path", "/foo/a.jsonl"])
    _run(["skill", "a", "/cc"])
    with open(_logfile(root, "a", "skills.jsonl")) as fh:
        entry = json.loads(fh.readline())
    assert entry["skill"] == "/cc"
    assert "path" not in entry


def test_reads_and_skills_coexist(root, clock):
    _run(["start", "a", "--transcript-path", "/foo/a.jsonl"])
    _run(["read", "a", "/f1.ts"])
    _run(["skill", "a", "/cc"])
    _run(["read", "a", "/f2.ts"])
    _run(["skill", "a", "/pcc"])
    with open(_logfile(root, "a", "reads.jsonl")) as fh:
        assert len(fh.readlines()) == 2
    with open(_logfile(root, "a", "skills.jsonl")) as fh:
        assert len(fh.readlines()) == 2


def test_sequential_appends_accumulate(root, clock):
    _run(["start", "a", "--transcript-path", "/foo/a.jsonl"])
    for i in range(100):
        _run(["read", "a", "/path/%d" % i])
    with open(_logfile(root, "a", "reads.jsonl")) as fh:
        assert len(fh.readlines()) == 100


def test_subagent_log_is_isolated(root, clock):
    _run(["start", "p", "--transcript-path", "/p/p/p.jsonl"])
    _run(["start", "agent-s", "--transcript-path", "/p/p/subagents/agent-s.jsonl"])
    _run(["skill", "agent-s", "/subagents"])
    assert os.path.isfile(_logfile(root, "p/subagents/agent-s", "skills.jsonl"))
    assert not os.path.isfile(_logfile(root, "p", "skills.jsonl"))


# ---------------------------------------------------------------------------
# Concurrency (DoD: many concurrent increments all land, none lost)
# ---------------------------------------------------------------------------

def _spawn(fn, count):
    threads = [threading.Thread(target=fn) for _ in range(count)]
    for t in threads:
        t.start()
    for t in threads:
        t.join()


def test_concurrent_tool_used_loses_no_increment(root, clock):
    _run(["start", "tu", "--transcript-path", "/foo/tu.jsonl"])

    def bump():
        for _ in range(25):
            _run(["tool-used", "tu"])

    _spawn(bump, 4)
    assert _read(_state(root, "tu"))["tools_used"] == 100


def test_concurrent_appends_preserve_every_line(root, clock):
    _run(["start", "a", "--transcript-path", "/foo/a.jsonl"])

    def append():
        for i in range(50):
            _run(["read", "a", "/p/%d" % i])

    _spawn(append, 2)
    with open(_logfile(root, "a", "reads.jsonl")) as fh:
        lines = fh.readlines()
    assert len(lines) == 100
    for ln in lines:
        json.loads(ln)  # every line is valid JSON


def test_concurrent_set_keeps_state_valid(root, clock):
    _run(["start", "c", "--transcript-path", "/foo/c.jsonl"])

    def writer(field, prefix):
        def run():
            for i in range(25):
                _run(["set", "c", field, "%s-%d" % (prefix, i)])
        return run

    t1 = threading.Thread(target=writer("approach", "team"))
    t2 = threading.Thread(target=writer("intent", "appr"))
    t1.start()
    t2.start()
    t1.join()
    t2.join()
    st = _read(_state(root, "c"))
    assert st["approach"].startswith("team-")
    assert st["intent"].startswith("appr-")
    assert st["state"] == "proposing"  # untouched field preserved


# ---------------------------------------------------------------------------
# prompt: human-turn filtering + turn rotation
# ---------------------------------------------------------------------------

def _prompt(content, sid, monkeypatch):
    import io
    monkeypatch.setattr("sys.stdin", io.StringIO(content))
    return session_state.cmd_prompt([sid])


def test_human_prompt_opens_turn(root, clock, monkeypatch):
    _run(["start", "hp", "--transcript-path", "/foo/hp.jsonl"])
    _prompt("fix the bug in PaymentService", "hp", monkeypatch)
    assert _read(_state(root, "hp"))["human_turns"] == 1


@pytest.mark.parametrize("injected", [
    "<task-notification>x</task-notification>",
    "[task-notification]",
    "This session is being continued from a previous conversation",
    "Base directory for this skill: /home/x/.claude/skills/pcc",
])
def test_system_injected_prompts_filtered(root, clock, monkeypatch, injected):
    _run(["start", "sys", "--transcript-path", "/foo/sys.jsonl"])
    _prompt(injected, "sys", monkeypatch)
    st = _read(_state(root, "sys"))
    assert st["human_turns"] == 0
    assert st["current_turn_start"] is None


def test_turn_rotation(root, clock, monkeypatch):
    clock(1000)
    _run(["start", "rot", "--transcript-path", "/foo/rot.jsonl"])
    clock(1100)
    _prompt("first", "rot", monkeypatch)
    st = _read(_state(root, "rot"))
    assert st["human_turns"] == 1
    assert st["current_turn_start"] == 1100
    assert st["previous_turn_start"] is None
    clock(1200)
    _prompt("second", "rot", monkeypatch)
    st = _read(_state(root, "rot"))
    assert st["human_turns"] == 2
    assert st["current_turn_start"] == 1200
    assert st["previous_turn_start"] == 1100


# ---------------------------------------------------------------------------
# stats / is-long-running derived snapshot
# ---------------------------------------------------------------------------

def test_stats_snapshot(root, clock, monkeypatch, capsys):
    clock(10000)
    _run(["start", "st", "--transcript-path", "/foo/st.jsonl"])
    clock(10100)
    _prompt("first", "st", monkeypatch)
    clock(10150)
    session_state.cmd_stats(["st"])
    out = json.loads(capsys.readouterr().out)
    assert out["session_start"] == 10000
    assert out["session_duration"] == 150
    assert out["human_turns"] == 1
    assert out["current_turn_start"] == 10100
    assert out["current_turn_duration"] == 50
    assert out["previous_turn_start"] is None
    assert out["previous_turn_duration"] is None


def test_stats_missing_session_is_empty_object(root, clock, capsys):
    session_state.cmd_stats(["nope"])
    assert capsys.readouterr().out.strip() == "{}"


def test_is_long_running_thresholds(root, clock, monkeypatch):
    clock(0)
    _run(["start", "lr", "--transcript-path", "/foo/lr.jsonl"])
    clock(100)
    assert _run(["is-long-running", "lr"]) == 1  # below all
    for i in range(5):
        clock(100 + i)
        _prompt("p%d" % i, "lr", monkeypatch)
    assert _run(["is-long-running", "lr"]) == 0  # 5 turns crosses default
    assert _run(["is-long-running", "lr", "--turns", "10"]) == 1  # raised above 5


# ---------------------------------------------------------------------------
# Compaction truncates the event logs but not state.json (DoD)
# ---------------------------------------------------------------------------

def test_compacted_truncates_logs(root, clock):
    _run(["start", "cp", "--transcript-path", "/foo/cp.jsonl"])
    for i in range(5):
        _run(["read", "cp", "/path/%d" % i])
    for i in range(3):
        _run(["skill", "cp", "/s%d" % i])
    assert _run(["compacted", "cp"]) == 0
    assert os.path.getsize(_logfile(root, "cp", "reads.jsonl")) == 0
    assert os.path.getsize(_logfile(root, "cp", "skills.jsonl")) == 0


def test_compacted_preserves_state_counters(root, clock, monkeypatch):
    _run(["start", "cp", "--transcript-path", "/foo/cp.jsonl"])
    _prompt("first", "cp", monkeypatch)
    _run(["tool-used", "cp"])
    _run(["tool-used", "cp"])
    _run(["compacted", "cp"])
    st = _read(_state(root, "cp"))
    assert st["human_turns"] == 1
    assert st["tools_used"] == 2


def test_appends_after_compaction_start_fresh(root, clock):
    _run(["start", "cp", "--transcript-path", "/foo/cp.jsonl"])
    _run(["read", "cp", "/before"])
    _run(["compacted", "cp"])
    _run(["read", "cp", "/after-1"])
    _run(["read", "cp", "/after-2"])
    with open(_logfile(root, "cp", "reads.jsonl")) as fh:
        lines = fh.readlines()
    assert len(lines) == 2
    assert json.loads(lines[0])["path"] == "/after-1"


# ---------------------------------------------------------------------------
# end / list / find-by-pane
# ---------------------------------------------------------------------------

def test_end_removes_session_and_cascades(root, clock):
    _run(["start", "p", "--transcript-path", "/p/p/p.jsonl"])
    _run(["start", "agent-c", "--transcript-path", "/p/p/subagents/agent-c.jsonl"])
    _run(["end", "p"])
    assert not os.path.isdir(os.path.join(str(root), "sessions", "p"))


def test_end_subagent_keeps_parent(root, clock):
    _run(["start", "p", "--transcript-path", "/p/p/p.jsonl"])
    _run(["start", "agent-c", "--transcript-path", "/p/p/subagents/agent-c.jsonl"])
    _run(["end", "agent-c"])
    sessions = os.path.join(str(root), "sessions")
    assert not os.path.isdir(os.path.join(sessions, "p", "subagents", "agent-c"))
    assert os.path.isdir(os.path.join(sessions, "p"))


def test_end_missing_session_is_idempotent(root, clock):
    assert _run(["end", "nope"]) == 0


def test_list_excludes_subagents(root, clock, capsys):
    _run(["start", "m", "--transcript-path", "/p/m/m.jsonl"])
    _run(["start", "agent-s", "--transcript-path", "/p/m/subagents/agent-s.jsonl"])
    session_state.cmd_list([])
    assert capsys.readouterr().out.split() == ["m"]


def test_list_subagents(root, clock, capsys):
    _run(["start", "m", "--transcript-path", "/p/m/m.jsonl"])
    _run(["start", "agent-a", "--transcript-path", "/p/m/subagents/agent-a.jsonl"])
    _run(["start", "agent-b", "--transcript-path", "/p/m/subagents/agent-b.jsonl"])
    session_state.cmd_list(["--subagents", "m"])
    assert sorted(capsys.readouterr().out.split()) == ["agent-a", "agent-b"]


def test_find_by_pane(root, clock, capsys):
    _run(["start", "f", "--transcript-path", "/foo/f.jsonl"])
    _run(["set", "f", "pane", "zellij-A"])
    session_state.cmd_find_by_pane(["zellij-A"])
    assert capsys.readouterr().out.strip() == "f"


def test_find_by_pane_tmux_field_is_separate(root, clock, capsys):
    _run(["start", "f", "--transcript-path", "/foo/f.jsonl"])
    _run(["set", "f", "tmux-pane", "Coding:1:0"])
    session_state.cmd_find_by_pane(["Coding:1:0"])            # default reads .pane
    assert capsys.readouterr().out.strip() == ""
    session_state.cmd_find_by_pane(["--tmux", "Coding:1:0"])  # --tmux reads .tmux-pane
    assert capsys.readouterr().out.strip() == "f"


# ---------------------------------------------------------------------------
# dispatch
# ---------------------------------------------------------------------------

def test_unknown_command_exits_1(root, clock):
    assert _run(["bogus"]) == 1


def test_no_command_exits_1(root, clock):
    assert _run([]) == 1


# ---------------------------------------------------------------------------
# In-process accessors load_state / merge_state — the read/write path the
# guards use directly (cmd_merge delegation stays covered by the merge tests).
# ---------------------------------------------------------------------------

def test_load_state_returns_stored_fields(root, clock):
    _run(["start", "ls1", "--transcript-path", "/foo/ls1.jsonl"])
    _run(["set", "ls1", "approach", "team"])
    st = session_state.load_state("ls1")
    assert st["approach"] == "team"
    assert st["state"] == "proposing"
    assert st["session_id"] == "ls1"


def test_load_state_missing_session_is_empty(root, clock):
    assert session_state.load_state("nope") == {}


def test_load_state_invalid_id_is_empty(root, clock):
    assert session_state.load_state("../etc/passwd") == {}


def test_merge_state_applies_and_preserves(root, clock):
    _run(["start", "ms1", "--transcript-path", "/foo/ms1.jsonl"])
    assert session_state.merge_state("ms1", {"approach": "team", "intent": "approval"}) is True
    st = _read(_state(root, "ms1"))
    assert st["approach"] == "team"
    assert st["intent"] == "approval"
    assert st["state"] == "proposing"   # untouched control field preserved
    assert st["session_id"] == "ms1"    # identity/telemetry preserved


def test_merge_state_lazy_creates_session(root, clock):
    assert session_state.merge_state("ms2", {"state": "executing"}) is True
    st = _read(_state(root, "ms2"))
    assert st["state"] == "executing"
    assert st["role"] == "main"


def test_merge_state_invalid_id_returns_false_no_write(root, clock):
    assert session_state.merge_state("../bad", {"state": "executing"}) is False
    sessions = os.path.join(str(root), "sessions")
    assert not os.path.isdir(sessions) or os.listdir(sessions) == []


def test_merge_state_non_dict_fragment_returns_false(root, clock):
    _run(["start", "ms3", "--transcript-path", "/foo/ms3.jsonl"])
    assert session_state.merge_state("ms3", "not a dict") is False
    assert session_state.merge_state("ms3", None) is False
