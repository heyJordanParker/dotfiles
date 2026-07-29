"""Spine coverage for lib/session_state.py — the per-session state store every
hook reads and writes through.

This guards the store's load-bearing properties in-process: concurrent writes,
corrupt/empty/missing-state healing, subagent nesting and resolution, and the
per-turn stop-gate counters.

Every test runs against a per-test data root (CLAUDE_DATA_ROOT / CLAUDE_PROJECTS_ROOT
under tmp_path) — nothing touches the real ~/.claude. Time is driven by monkeypatching
the module's clock function.
"""

import json
import os
import sys
import threading

import pytest
from conftest import PY_HOOKS

# The module under test imports as a bare `session_state` from its own lib dir;
# put that dir on the path so both styles resolve to the one module.
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
    """Drive session_state's clock deterministically. Returns a setter."""
    state = {"now": 1000}

    def _set(value):
        state["now"] = value

    monkeypatch.setattr(session_state, "_now", lambda: state["now"])
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
    assert st["commit_requested"] is False
    assert st["goal"] is None
    assert st["notes"] == []
    assert st["gate_blocks"] == {}
    assert st["current_turn_start"] is None
    assert st["schema_version"] == 1


def test_start_is_idempotent(root, clock):
    _run(["start", "m", "--transcript-path", "/foo/m.jsonl"])
    _run(["set", "m", "approach", "team"])
    _run(["start", "m", "--transcript-path", "/foo/m.jsonl"])
    assert session_state.cmd_get(["m", "approach"]) == 0  # exit only; value below
    st = _read(_state(root, "m"))
    assert st["approach"] == "team"  # re-start did not clobber the mutation


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
    # subagent state omits the main-only control + goal fields
    for omitted in ("approach", "state", "goal", "notes", "gate_blocks",
                    "commit_requested"):
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


def test_heal_preserves_the_prior_mode(root, clock):
    # A truncated write leaves unparseable JSON; healing to defaults would drop the
    # architect's typed /execute back to proposing and block their edits.
    path = _seed_session_dir(root, "mode")
    with open(path, "w") as fh:
        fh.write('{"session_id":"mode","role":"main","state":"executing","appro')
    assert _run(["set", "mode", "approach", "solo"]) == 0
    st = _read(path)
    assert st["state"] == "executing"
    assert "prior_state_lost" not in st


def test_heal_flags_an_unrecoverable_mode(root, clock):
    path = _seed_session_dir(root, "lost")
    with open(path, "w") as fh:
        fh.write("garbage not json")
    assert _run(["set", "lost", "approach", "solo"]) == 0
    st = _read(path)
    assert st["state"] == "proposing"
    assert st["prior_state_lost"] is True


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
    _run(["set", "s", "current_turn_start", "3"])
    st = _read(_state(root, "s"))
    assert st["commit_requested"] is True
    assert st["current_turn_start"] == 3


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
    assert _run(["merge", bad, '{"approach":"team"}']) == 1


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
    session_state.cmd_get(["g", "goal"])
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
# Concurrency (DoD: concurrent writers never corrupt the state file)
# ---------------------------------------------------------------------------

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
    clock(1100)
    _run(["start", "hp", "--transcript-path", "/foo/hp.jsonl"])
    _prompt("fix the bug in PaymentService", "hp", monkeypatch)
    assert _read(_state(root, "hp"))["current_turn_start"] == 1100


@pytest.mark.parametrize("injected", [
    "<task-notification>x</task-notification>",
    "[task-notification]",
    "This session is being continued from a previous conversation",
    "Base directory for this skill: /home/x/.claude/skills/pcc",
])
def test_system_injected_prompts_filtered(root, clock, monkeypatch, injected):
    _run(["start", "sys", "--transcript-path", "/foo/sys.jsonl"])
    _prompt(injected, "sys", monkeypatch)
    assert _read(_state(root, "sys"))["current_turn_start"] is None


def test_turn_rotation(root, clock, monkeypatch):
    clock(1000)
    _run(["start", "rot", "--transcript-path", "/foo/rot.jsonl"])
    clock(1100)
    _prompt("first", "rot", monkeypatch)
    assert _read(_state(root, "rot"))["current_turn_start"] == 1100
    clock(1200)
    _prompt("second", "rot", monkeypatch)
    assert _read(_state(root, "rot"))["current_turn_start"] == 1200


# ---------------------------------------------------------------------------
# end
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


# ---------------------------------------------------------------------------
# Stop-gate block counter — per-turn, per-gate; resets when the turn advances
# ---------------------------------------------------------------------------

def test_gate_block_count_zero_before_any_block(root, clock):
    _run(["start", "g", "--transcript-path", "/foo/g.jsonl"])
    assert session_state.gate_block_count("g", "validate_completion") == 0


def test_bump_gate_block_counts_within_a_turn(root, clock):
    _run(["start", "g", "--transcript-path", "/foo/g.jsonl"])
    session_state.merge_state("g", {"current_turn_start": 100})
    assert session_state.bump_gate_block("g", "validate_completion") == 1
    assert session_state.bump_gate_block("g", "validate_completion") == 2
    assert session_state.gate_block_count("g", "validate_completion") == 2


def test_gate_block_count_resets_when_turn_advances(root, clock):
    _run(["start", "g", "--transcript-path", "/foo/g.jsonl"])
    session_state.merge_state("g", {"current_turn_start": 100})
    session_state.bump_gate_block("g", "validate_completion")
    assert session_state.gate_block_count("g", "validate_completion") == 1
    session_state.merge_state("g", {"current_turn_start": 200})  # next human turn
    assert session_state.gate_block_count("g", "validate_completion") == 0


def test_gate_blocks_are_isolated_per_gate(root, clock):
    _run(["start", "g", "--transcript-path", "/foo/g.jsonl"])
    session_state.merge_state("g", {"current_turn_start": 100})
    session_state.bump_gate_block("g", "validate_completion")
    assert session_state.gate_block_count("g", "babysitter") == 0
    assert session_state.gate_block_count("g", "validate_completion") == 1
