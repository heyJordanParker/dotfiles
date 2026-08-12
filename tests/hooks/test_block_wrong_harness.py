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


def test_all_runs_here(config_root, monkeypatch):
    config_root("ponytail", BOTH)
    assert _run(monkeypatch, "ponytail") == 0


def test_undeclared_agent_runs_here(config_root, monkeypatch):
    """The key is optional: omitting it means `all`, which is every agent today."""
    config_root("cto", UNDECLARED)
    assert _run(monkeypatch, "cto") == 0


def test_agent_with_no_definition_runs_here(config_root, monkeypatch):
    assert _run(monkeypatch, "not-an-agent") == 0


def test_missing_subagent_type_passes(config_root, monkeypatch):
    assert _run(monkeypatch, "") == 0


@pytest.mark.parametrize("declared", ["Codex", "CODEX", "CLAUDE", "both", "kodex"])
def test_unrecognized_value_denies(config_root, monkeypatch, capsys, declared):
    """The comparison is exact. Lowering would widen permission — `harness: CODEX`
    would pass the codex allowlist — which is the direction agent_memory forbids.
    An unrecognized value therefore denies on both harnesses, so a typo makes an
    agent unreachable rather than unrestricted."""
    config_root("ponytail", "---\nname: %%s\nharness: %s\n---\n\nbody\n" % declared)
    assert _run(monkeypatch, "ponytail") == 2
    out = _feedback(capsys)
    assert declared in out and "not a harness" in out


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


def test_harness_line_in_the_body_does_not_count(config_root, monkeypatch):
    config_root("architect", "---\nname: %s\n---\n\nharness: codex\n")
    assert _run(monkeypatch, "architect") == 0


def test_unreadable_definition_does_not_bar_the_agent(tmp_path, monkeypatch):
    """Unlike memory, an unreadable definition permits. Refusing every dispatch on
    a transient read failure would take the whole roster down, and the capability
    being withheld is one Claude would otherwise have granted."""
    agents = tmp_path / "agents"
    agents.mkdir()
    (agents / "researcher.md").mkdir()  # present, unreadable as a file
    monkeypatch.setenv("CLAUDE_CONFIG_DIR", str(tmp_path))
    assert _run(monkeypatch, "researcher") == 0


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


def test_codex_runs_an_undeclared_agent(monkeypatch, tmp_path):
    """The key is optional on this side too — the roster's default is unchanged."""
    _pin(monkeypatch, tmp_path, "ponytail", UNDECLARED)
    _stub_codex(monkeypatch, tmp_path)
    assert codex_run.main(["@ponytail", "do x"]) == 0


@pytest.mark.parametrize("declared", ["Codex", "CLAUDE", "both", "kodex"])
def test_codex_refuses_an_unrecognized_value(monkeypatch, tmp_path, capsys, declared):
    """Same exact matching as the Claude side, so a typo denies on both. The two
    refusals differ because their fixes do: this one names the definition, because
    sending a broken declaration to Claude would only earn a second refusal."""
    _pin(monkeypatch, tmp_path, "ponytail",
         "---\nname: %%s\nharness: %s\n---\n\nbody\n" % declared)
    _stub_codex(monkeypatch, tmp_path)
    _no_codex(monkeypatch)
    assert codex_run.main(["@ponytail", "do x"]) == 1
    out = capsys.readouterr().out
    assert declared in out and "not a harness" in out
    assert "Fix the `harness:` line in ponytail.md" in out
    assert "Agent(subagent_type:" not in out


def test_codex_resume_refuses_a_now_claude_only_agent(monkeypatch, tmp_path, capsys):
    """The gate sits in _dispatch, which both the founding run and a resume funnel
    through, so a job founded before its agent moved to Claude refuses on
    continuation with the same message."""
    _pin(monkeypatch, tmp_path, "architect", UNDECLARED)
    _stub_codex(monkeypatch, tmp_path)
    assert codex_run.main(["@architect", "review"]) == 0
    job = json.loads(next(tmp_path.glob("codex-run-*.json")).read_text())["job"]
    capsys.readouterr()

    (tmp_path / "agents" / "architect.md").write_text(CLAUDE_ONLY % "architect")
    _no_codex(monkeypatch)
    assert codex_run.main(["resume", job, "continue"]) == 1
    assert "declares `harness: claude`" in capsys.readouterr().out
