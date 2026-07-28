"""Coverage for lib/model_call.py — the shared backend selector the
classifier/validator hooks call.

The tests exercise behavior directly. No real network and no real credentials are
touched: every adapter is replaced with an in-process stub.
"""

import os
import sys

import pytest
from conftest import PY_HOOKS

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
    """Replace one backend adapter with a stub returning a fixed parsed result, so
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
        return {"ok": 1}

    monkeypatch.setitem(model_call._ADAPTERS, "openai", stub)
    monkeypatch.setenv("MODEL_CALL_BACKEND", "openai")
    out = model_call.run_model(system_prompt="sys", user_prompt="user", schema={"k": "v"})
    assert out == {"ok": 1}
    assert seen["called"] is True


# ---------------------------------------------------------------------------
# A failed call returns nothing (DoD)
# ---------------------------------------------------------------------------

def test_failed_call_returns_none(root, monkeypatch):
    _stub_adapter(monkeypatch, None)
    assert model_call.run_model(system_prompt="sys", user_prompt="user", schema={},
                                backend="claude") is None


def test_raw_flag_threads_to_adapter(root, monkeypatch):
    captured = {}

    def stub(effort, sp, up, sc, raw):
        captured["raw"] = raw
        return {"x": 1}

    monkeypatch.setitem(model_call._ADAPTERS, "claude", stub)
    model_call.run_model(system_prompt="sys", user_prompt="user", schema={},
                         backend="claude", raw=True)
    assert captured["raw"] is True


# ---------------------------------------------------------------------------
# Pure helpers: JSON extraction (no network)
# ---------------------------------------------------------------------------

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
        return {"x": 1}

    monkeypatch.setitem(model_call._ADAPTERS, "claude", stub)
    model_call.run_model("high", system_prompt="sys", user_prompt="user", schema={},
                         backend="claude")
    assert captured["effort"] == "high"


def test_effort_defaults_to_none(root, monkeypatch):
    captured = {}

    def stub(effort, sp, up, sc, raw):
        captured["effort"] = effort
        return {"x": 1}

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
