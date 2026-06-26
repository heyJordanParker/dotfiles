"""Coverage for lib/model_call.py — the shared backend selector the four
classifier/validator hooks call, plus its optional per-call measurement log.

The tests exercise behavior directly. No real network and no real
credentials are touched: every adapter is replaced with an in-process stub, and
the measurement log writes under a per-test data root (CLAUDE_DATA_ROOT under
tmp_path), never the real ~/.claude.
"""

import json
import os
import sys

import pytest
from conftest import PY_HOOKS

# model_call._record does a bare `import session_state`; put the lib dir on the
# path so that resolves to the same module the spine tests use.
sys.path.insert(0, os.path.join(PY_HOOKS, "lib"))

from lib import model_call  # noqa: E402


@pytest.fixture
def root(tmp_path, monkeypatch):
    data_root = tmp_path / "claude"
    monkeypatch.setenv("CLAUDE_DATA_ROOT", str(data_root))
    monkeypatch.delenv("MODEL_CALL_BACKEND", raising=False)
    monkeypatch.delenv("CLAUDE_PROJECTS_ROOT", raising=False)
    for var in ("AGENT_SESSION_ID", "CODEX_THREAD_ID", "CLAUDE_CODE_SESSION_ID"):
        monkeypatch.delenv(var, raising=False)
    return data_root


def _stub_adapter(monkeypatch, result, name="claude"):
    """Replace one backend adapter with a stub returning a fixed result tuple, so
    no subprocess or network call ever happens. Tests pass backend=name to
    run_model so resolution lands on the stub regardless of the default."""
    monkeypatch.setitem(model_call._ADAPTERS, name, lambda effort, sp, up, sc, raw: result)


# ---------------------------------------------------------------------------
# Backend resolution order: explicit arg > env > default claude (DoD)
# ---------------------------------------------------------------------------

def test_backend_resolution_order(root, monkeypatch):
    assert model_call._resolve_backend(None) == "openai"        # default
    monkeypatch.setenv("MODEL_CALL_BACKEND", "claude")
    assert model_call._resolve_backend(None) == "claude"        # env over default
    assert model_call._resolve_backend("local") == "local"      # explicit over env


def test_run_model_dispatches_to_resolved_backend(root, monkeypatch):
    seen = {}

    def stub(effort, sp, up, sc, raw):
        seen["called"] = True
        return ({"ok": 1}, "model-x", raw, 10, 5)

    monkeypatch.setitem(model_call._ADAPTERS, "openai", stub)
    monkeypatch.setenv("MODEL_CALL_BACKEND", "openai")
    out = model_call.run_model(system_prompt="sys", user_prompt="user", schema={"k": "v"})
    assert out == {"ok": 1}
    assert seen["called"] is True


# ---------------------------------------------------------------------------
# A failed call returns nothing (DoD)
# ---------------------------------------------------------------------------

def test_failed_call_returns_none(root, monkeypatch):
    _stub_adapter(monkeypatch, (None, "opus", False, None, None))
    assert model_call.run_model(system_prompt="sys", user_prompt="user", schema={},
                                backend="claude") is None


def test_failed_call_without_measure_writes_nothing(root, monkeypatch):
    _stub_adapter(monkeypatch, (None, "opus", False, None, None))
    model_call.run_model(system_prompt="sys", user_prompt="user", schema={},
                         backend="claude", session_id="sess-1")
    # measure defaults off → no model_calls.jsonl anywhere
    assert not os.path.isdir(os.path.join(str(root), "sessions"))


# ---------------------------------------------------------------------------
# The shape of one recorded measurement event (DoD)
# ---------------------------------------------------------------------------

def test_measure_records_one_event(root, monkeypatch):
    _stub_adapter(monkeypatch, ({"x": 1}, "opus", False, 120, 30))
    out = model_call.run_model(system_prompt="sys", user_prompt="user", schema={"k": "v"},
                               backend="claude", measure=True, session_id="sess-1", hook="classify")
    assert out == {"x": 1}
    log = os.path.join(str(root), "sessions", "sess-1", "model_calls.jsonl")
    with open(log) as fh:
        lines = fh.readlines()
    assert len(lines) == 1
    event = json.loads(lines[0])
    assert event["hook"] == "classify"
    assert event["backend"] == "claude"
    assert event["model"] == "opus"
    assert event["raw"] is False
    assert event["input_tokens"] == 120
    assert event["output_tokens"] == 30
    assert event["outcome"] == "ok"
    assert isinstance(event["latency"], (int, float))
    assert isinstance(event["ts"], str)


def test_measure_records_parse_fail_on_failed_call(root, monkeypatch):
    _stub_adapter(monkeypatch, (None, "opus", False, None, None))
    model_call.run_model(system_prompt="sys", user_prompt="user", schema={}, backend="claude",
                         measure=True, session_id="sess-1", hook="classify")
    log = os.path.join(str(root), "sessions", "sess-1", "model_calls.jsonl")
    with open(log) as fh:
        event = json.loads(fh.readline())
    assert event["outcome"] == "parse_fail"
    assert event["input_tokens"] is None
    assert event["output_tokens"] is None


def test_measure_without_session_id_records_nothing(root, monkeypatch):
    _stub_adapter(monkeypatch, ({"x": 1}, "opus", False, 1, 1))
    model_call.run_model(system_prompt="sys", user_prompt="user", schema={},
                         backend="claude", measure=True, session_id="")
    assert not os.path.isdir(os.path.join(str(root), "sessions"))


def test_measure_rejects_invalid_session_id(root, monkeypatch):
    _stub_adapter(monkeypatch, ({"x": 1}, "opus", False, 1, 1))
    model_call.run_model(system_prompt="sys", user_prompt="user", schema={},
                         backend="claude", measure=True, session_id="../escape")
    assert not os.path.isdir(os.path.join(str(root), "sessions", "..", "escape"))


def test_raw_flag_threads_into_event(root, monkeypatch):
    captured = {}

    def stub(effort, sp, up, sc, raw):
        captured["raw"] = raw
        return ({"x": 1}, "opus", raw, 1, 1)

    monkeypatch.setitem(model_call._ADAPTERS, "claude", stub)
    model_call.run_model(system_prompt="sys", user_prompt="user", schema={}, backend="claude",
                         raw=True, measure=True, session_id="sess-1")
    assert captured["raw"] is True
    log = os.path.join(str(root), "sessions", "sess-1", "model_calls.jsonl")
    with open(log) as fh:
        assert json.loads(fh.readline())["raw"] is True


# ---------------------------------------------------------------------------
# Pure helpers: token normalization + JSON extraction (no network)
# ---------------------------------------------------------------------------

def test_claude_tokens_sums_cache_fields():
    usage = {"input_tokens": 100, "cache_read_input_tokens": 50,
             "cache_creation_input_tokens": 10, "output_tokens": 7}
    inp, out = model_call._claude_tokens(usage)
    assert inp == 160   # fresh + both cache portions
    assert out == 7


def test_claude_tokens_none_usage():
    assert model_call._claude_tokens(None) == (None, None)


def test_openai_tokens_does_not_double_count_cache():
    usage = {"input_tokens": 200, "cached_tokens": 80, "output_tokens": 9}
    inp, out = model_call._openai_tokens(usage)
    assert inp == 200   # cached is a subset already inside input_tokens
    assert out == 9


def test_extract_json_pulls_object_from_prose():
    assert model_call._extract_json('go {"intent":"x"} done') == {"intent": "x"}
    assert model_call._extract_json("no json here") is None
    assert model_call._extract_json("") is None


def test_shape_line_serializes_dict_schema():
    line = model_call._shape_line({"intent": "string"})
    assert '{"intent":"string"}' in line
    assert "one line of minified JSON" in line


# ---------------------------------------------------------------------------
# local backend is explicitly unsupported, not silent
# ---------------------------------------------------------------------------

def test_local_backend_raises_explicit_error(root):
    with pytest.raises(NotImplementedError) as exc:
        model_call._call_local("none", "sys", "user", {}, False)
    assert "review-prompt" in str(exc.value)


# ---------------------------------------------------------------------------
# Effort: threaded to the adapter, translated per provider, validated
# ---------------------------------------------------------------------------

def test_effort_threads_to_adapter(root, monkeypatch):
    captured = {}

    def stub(effort, sp, up, sc, raw):
        captured["effort"] = effort
        return ({"x": 1}, "opus", raw, 1, 1)

    monkeypatch.setitem(model_call._ADAPTERS, "claude", stub)
    model_call.run_model("high", system_prompt="sys", user_prompt="user", schema={},
                         backend="claude")
    assert captured["effort"] == "high"


def test_effort_defaults_to_none(root, monkeypatch):
    captured = {}

    def stub(effort, sp, up, sc, raw):
        captured["effort"] = effort
        return ({"x": 1}, "opus", raw, 1, 1)

    monkeypatch.setitem(model_call._ADAPTERS, "claude", stub)
    model_call.run_model(system_prompt="sys", user_prompt="user", schema={}, backend="claude")
    assert captured["effort"] == "none"


def test_unknown_effort_raises(root):
    with pytest.raises(ValueError):
        model_call.run_model("ultra", system_prompt="sys", user_prompt="user", schema={})


def test_effort_maps_cover_every_level():
    for level in model_call.EFFORT_LEVELS:
        assert level in model_call._OPENAI_EFFORT
        assert level in model_call._CLAUDE_EFFORT
    # our floor has no OpenAI-less translation; claude has no "none", clamps to low
    assert model_call._OPENAI_EFFORT["none"] == "none"
    assert model_call._CLAUDE_EFFORT["none"] == "low"
    assert model_call._OPENAI_EFFORT["max"] == "xhigh"
    assert model_call._CLAUDE_EFFORT["max"] == "max"
