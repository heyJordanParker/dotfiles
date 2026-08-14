"""The harness gate: one `harness` declaration, both harnesses, exact matching."""

import io
import json
import os
import sys

import block_wrong_harness
import pytest
from conftest import PY_HOOKS

sys.path.insert(0, os.path.join(PY_HOOKS, "lib"))

from lib import codex_run  # noqa: E402
from test_codex_run import _sent, _stub_codex  # noqa: E402

CODEX_ONLY = "---\nname: %s\nharness: codex\ncodex-model: gpt-5.6-terra\n---\n\nbody\n"
CLAUDE_ONLY = "---\nname: %s\nmodel: opus\nharness: claude\n---\n\nbody\n"
BOTH = "---\nname: %s\nmodel: opus\nharness: all\n---\n\nbody\n"
UNDECLARED = "---\nname: %s\nmodel: opus\n---\n\nbody\n"


@pytest.fixture
def config_root(tmp_path, monkeypatch):
    """A Claude config root with an agents/ dir; returns a writer for it."""
    agents = tmp_path / "agents"
    agents.mkdir()
    monkeypatch.setenv("CLAUDE_CONFIG_DIR", str(tmp_path))

    def write(name, template):
        (agents / (name + ".md")).write_text(template % name)
    return write


def _run(monkeypatch, subagent_type):
    event = {"hook_event_name": "PreToolUse", "tool_name": "Agent",
             "tool_input": {"subagent_type": subagent_type, "prompt": "do x"}}
    monkeypatch.setattr(sys, "stdin", io.StringIO(json.dumps(event)))
    return block_wrong_harness.main()


def _feedback(capsys):
    captured = capsys.readouterr()
    return captured.out + captured.err


def test_codex_only_agent_is_refused_here(config_root, monkeypatch, capsys):
    """The whole point: an agent whose model is a codex model has nothing to run
    on here, so the dispatch would silently use the config root's model."""
    config_root("bulk-rewriter", CODEX_ONLY)
    assert _run(monkeypatch, "bulk-rewriter") == 2
    out = _feedback(capsys)
    assert "declares `harness: codex`" in out
    assert "codex-run @bulk-rewriter" in out


def test_claude_only_agent_runs_here(config_root, monkeypatch):
    config_root("architect", CLAUDE_ONLY)
    assert _run(monkeypatch, "architect") == 0












def test_declaration_is_read_from_the_active_root(tmp_path, monkeypatch):
    """A profile is its own config root: the same name is whichever file that
    root holds, matching how the memory gate resolves a definition."""
    for root, template in (("default", BOTH), ("profile", CODEX_ONLY)):
        agents = tmp_path / root / "agents"
        agents.mkdir(parents=True)
        (agents / "researcher.md").write_text(template % "researcher")

    monkeypatch.setenv("CLAUDE_CONFIG_DIR", str(tmp_path / "default"))
    assert _run(monkeypatch, "researcher") == 0
    monkeypatch.setenv("CLAUDE_CONFIG_DIR", str(tmp_path / "profile"))
    assert _run(monkeypatch, "researcher") == 2






# --- the codex side reads the same declaration -------------------------------------

def _pin(monkeypatch, tmp_path, name, template):
    agents = tmp_path / "agents"
    agents.mkdir(exist_ok=True)
    (agents / (name + ".prompt.md")).write_text("instructions for %s" % name)
    (agents / (name + ".md")).write_text(template % name)
    monkeypatch.setattr(codex_run, "AGENTS_DIR", str(agents))
    monkeypatch.setenv("CLAUDE_CONFIG_DIR", str(tmp_path / "empty-root"))


def _no_codex(monkeypatch):
    """A refusal must never reach codex, so starting an app-server is an error."""
    monkeypatch.setattr(codex_run, "_Server",
                        lambda *a, **kw: (_ for _ in ()).throw(AssertionError("codex must not run")))


def test_codex_refuses_a_claude_only_agent(monkeypatch, tmp_path, capsys):
    """The mirror of the Claude gate, and it never reaches codex."""
    _pin(monkeypatch, tmp_path, "architect", CLAUDE_ONLY)
    _stub_codex(monkeypatch, tmp_path)
    _no_codex(monkeypatch)
    assert codex_run.main(["@architect", "review"]) == 1
    out = capsys.readouterr().out
    assert "declares `harness: claude`" in out
    assert 'Agent(subagent_type: "architect"' in out
    assert "Traceback" not in out


def test_codex_runs_a_codex_only_agent(monkeypatch, tmp_path):
    _pin(monkeypatch, tmp_path, "bulk-rewriter", CODEX_ONLY)
    log = _stub_codex(monkeypatch, tmp_path)
    assert codex_run.main(["@bulk-rewriter", "rewrite"]) == 0
    assert _sent(log, "turn/start")["model"] == "gpt-5.6-terra"






