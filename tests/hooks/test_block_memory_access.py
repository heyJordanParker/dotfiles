"""The per-agent memory gate: one `memory: none` declaration, both harnesses."""

import io
import json
import os
import sys

import block_memory_access
import pytest
from conftest import PY_HOOKS

sys.path.insert(0, os.path.join(PY_HOOKS, "lib"))

from lib import codex_run  # noqa: E402
from test_codex_run import _pin_rollout  # noqa: E402

BLANK = "---\nname: %s\nmodel: opus\nmemory: none\n---\n\nbody\n"
DECLARED_ON = "---\nname: %s\nmodel: opus\nmemory: user\n---\n\nbody\n"
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


def _run(monkeypatch, agent_type, tool="mcp__plugin_honcho_honcho__search",
         agent_id="a1d6344fe2b8d8332"):
    """A subagent's PreToolUse payload by default: both fields present, which is
    the shape observed from a real Agent-tool dispatch. `agent_id=None` is the
    main thread, which carries `agent_type` alone when started with --agent."""
    event = {"hook_event_name": "PreToolUse", "tool_name": tool, "tool_input": {}}
    if agent_type is not None:
        event["agent_type"] = agent_type
    if agent_id is not None:
        event["agent_id"] = agent_id
    monkeypatch.setattr(sys, "stdin", io.StringIO(json.dumps(event)))
    return block_memory_access.main()


def test_declared_blank_agent_is_refused(config_root, monkeypatch, capsys):
    config_root("researcher", BLANK)
    assert _run(monkeypatch, "researcher") == 2
    assert "researcher agent declares `memory: none`" in capsys.readouterr().err


def test_agent_declaring_memory_on_reaches_memory(config_root, monkeypatch):
    config_root("architect", DECLARED_ON)
    assert _run(monkeypatch, "architect") == 0


def test_undeclared_agent_reaches_memory(config_root, monkeypatch):
    config_root("cto", UNDECLARED)
    assert _run(monkeypatch, "cto") == 0


def test_unknown_agent_reaches_memory(config_root, monkeypatch):
    # No definition file under this root: undeclared, so today's behaviour holds.
    assert _run(monkeypatch, "not-an-agent") == 0


def test_missing_agent_type_reaches_memory(config_root, monkeypatch):
    config_root("researcher", BLANK)
    assert _run(monkeypatch, None) == 0


def test_main_session_started_with_the_agent_keeps_memory(config_root, monkeypatch):
    """The pair that defines the gate. `agent_type` alone is the main thread of a
    session started with `--agent` — the architect's shell alias starts every
    session that way, so keying the presence test on it would take memory off his
    own conversation the moment he launched one as a blank-declaring agent.
    Observed live: a `--agent blankprobe` main session carries
    `{"agent_id": null, "agent_type": "blankprobe"}`."""
    config_root("researcher", BLANK)
    assert _run(monkeypatch, "researcher", agent_id=None) == 0


def test_subagent_of_that_same_agent_is_refused(config_root, monkeypatch, capsys):
    """The other half: the same declaration, the same name, inside a subagent —
    `agent_id` present — is refused. Observed live: an Agent-tool dispatch of the
    same agent carries `{"agent_id": "a1d6…", "agent_type": "blankprobe"}`."""
    config_root("researcher", BLANK)
    assert _run(monkeypatch, "researcher") == 2
    assert "researcher agent declares `memory: none`" in capsys.readouterr().err


def test_declaration_is_read_from_the_active_root(tmp_path, monkeypatch):
    """A profile is its own config root: the same name is whichever file that
    root holds, so the profile's copy of an agent governs inside the profile."""
    for root, template in (("default", DECLARED_ON), ("profile", BLANK)):
        agents = tmp_path / root / "agents"
        agents.mkdir(parents=True)
        (agents / "researcher.md").write_text(template % "researcher")

    monkeypatch.setenv("CLAUDE_CONFIG_DIR", str(tmp_path / "default"))
    assert _run(monkeypatch, "researcher") == 0
    monkeypatch.setenv("CLAUDE_CONFIG_DIR", str(tmp_path / "profile"))
    assert _run(monkeypatch, "researcher") == 2


def test_memory_line_in_the_body_does_not_count(config_root, monkeypatch):
    # Only the frontmatter declares; the same words in the prose do not.
    config_root("architect", "---\nname: %s\n---\n\nmemory: none\n")
    assert _run(monkeypatch, "architect") == 0


# --- the codex route reads the same declaration ------------------------------------

def _pin(monkeypatch, tmp_path, name, template):
    agents = tmp_path / "agents"
    agents.mkdir(exist_ok=True)
    (agents / (name + ".prompt.md")).write_text("instructions for %s" % name)
    (agents / (name + ".md")).write_text(template % name)
    monkeypatch.setattr(codex_run, "AGENTS_DIR", str(agents))
    # codex-run searches the active config root's roster first. Pin it at an
    # empty root so the real ~/.claude never supplies an agent of the same name.
    monkeypatch.setenv("CLAUDE_CONFIG_DIR", str(tmp_path / "empty-root"))


def test_codex_blank_agent_disables_the_memory_server(monkeypatch, tmp_path):
    _pin(monkeypatch, tmp_path, "researcher", BLANK)
    seen = {}
    monkeypatch.setattr(codex_run, "_dispatch",
                        lambda path, prompt, agent, resume_id=None, blank_memory=False:
                        seen.update(blank=blank_memory) or 0)
    assert codex_run.main(["@researcher", "task"]) == 0
    assert seen["blank"] is True


def test_codex_undeclared_agent_keeps_memory(monkeypatch, tmp_path):
    _pin(monkeypatch, tmp_path, "architect", UNDECLARED)
    seen = {}
    monkeypatch.setattr(codex_run, "_dispatch",
                        lambda path, prompt, agent, resume_id=None, blank_memory=False:
                        seen.update(blank=blank_memory) or 0)
    assert codex_run.main(["@architect", "task"]) == 0
    assert seen["blank"] is False


def test_codex_blank_agent_run_carries_the_disable_flag(monkeypatch, tmp_path):
    """The flag reaches the actual codex argv, not just _dispatch's signature."""
    cmds = []
    monkeypatch.setattr(codex_run, "_run",
                        lambda cmd, events, definition_path='': cmds.append(cmd) or (0, "answer", "th_1", False, ""))
    monkeypatch.setattr(codex_run, "_output_paths",
                        lambda: (str(tmp_path / "a.txt"), str(tmp_path / "e.jsonl")))
    codex_run._dispatch(str(tmp_path / "p.md"), "task", "architect", blank_memory=True)
    codex_run._dispatch(str(tmp_path / "p.md"), "task", "architect", blank_memory=False)
    assert "mcp_servers.honcho.enabled=false" in cmds[0]
    assert "mcp_servers.honcho.enabled=false" not in cmds[1]


def test_codex_blank_agent_run_disables_both_providers(monkeypatch, tmp_path):
    """codex has two memory providers and a blank-declared run must lose both:
    the honcho MCP server, and codex's own [memories], which needs no tool call
    because it injects a Memory section straight into the run."""
    cmds = []
    monkeypatch.setattr(codex_run, "_run",
                        lambda cmd, events, definition_path='': cmds.append(cmd) or (0, "answer", "th_1", False, ""))
    monkeypatch.setattr(codex_run, "_output_paths",
                        lambda: (str(tmp_path / "a.txt"), str(tmp_path / "e.jsonl")))
    codex_run._dispatch(str(tmp_path / "p.md"), "task", "architect", blank_memory=True)
    codex_run._dispatch(str(tmp_path / "p.md"), "task", "architect", blank_memory=False)
    for flag in ("mcp_servers.honcho.enabled=false",
                 "memories.use_memories=false",
                 "memories.generate_memories=false"):
        assert flag in cmds[0]
        assert flag not in cmds[1]


def test_codex_resume_applies_the_founding_agents_declaration(monkeypatch, tmp_path):
    """The declaration follows the thread across a resume. The founding agent is
    recovered from codex's own record, so a thread founded blank stays blank on
    continuation and a thread founded by an agent that keeps memory keeps it."""
    _pin(monkeypatch, tmp_path, "researcher", BLANK)
    _pin(monkeypatch, tmp_path, "architect", UNDECLARED)
    _pin_rollout(monkeypatch, tmp_path, "th_blank", "instructions for researcher")
    _pin_rollout(monkeypatch, tmp_path, "th_keeps", "instructions for architect")
    cmds = []
    monkeypatch.setattr(codex_run, "_run",
                        lambda cmd, events, definition_path='': cmds.append(cmd) or (0, "answer", cmd[3], False, ""))
    monkeypatch.setattr(codex_run, "_output_paths",
                        lambda: (str(tmp_path / "a.txt"), str(tmp_path / "e.jsonl")))
    assert codex_run.main(["resume", "th_blank", "continue"]) == 0
    assert codex_run.main(["resume", "th_keeps", "continue"]) == 0
    for flag in ("mcp_servers.honcho.enabled=false",
                 "memories.use_memories=false",
                 "memories.generate_memories=false"):
        assert flag in cmds[0]
        assert flag not in cmds[1]


# --- the declaration fails closed, never open --------------------------------------

@pytest.mark.parametrize("declaration", [
    "memory: none",
    'memory: "none"',      # quoted scalars are valid under scripts/frontmatter.py
    "memory: 'none'",
    "memory: none # one-shot",
    'memory: "none"  # one-shot',
    "memory: none   ",
    "memory: NONE",
])
def test_every_valid_spelling_of_the_denial_denies(tmp_path, monkeypatch, declaration):
    agents = tmp_path / "agents"
    agents.mkdir()
    (agents / "researcher.md").write_text(
        "---\nname: researcher\n%s\nmodel: opus\n---\n\nbody\n" % declaration)
    monkeypatch.setenv("CLAUDE_CONFIG_DIR", str(tmp_path))
    assert _run(monkeypatch, "researcher") == 2


def test_unreadable_definition_denies(tmp_path, monkeypatch):
    """A definition that exists but cannot be read is not evidence of
    permission. A *missing* one is — that is the undeclared contract."""
    agents = tmp_path / "agents"
    agents.mkdir()
    (agents / "researcher.md").mkdir()  # present, unreadable as a file
    monkeypatch.setenv("CLAUDE_CONFIG_DIR", str(tmp_path))
    assert _run(monkeypatch, "researcher") == 2


def test_codex_reads_the_same_spellings(monkeypatch, tmp_path):
    """Both harnesses go through one parser, so a spelling that denies on Claude
    denies on codex too."""
    _pin(monkeypatch, tmp_path, "researcher",
         "---\nname: %s\nmemory: 'none'  # one-shot\n---\n\nbody\n")
    seen = {}
    monkeypatch.setattr(codex_run, "_dispatch",
                        lambda path, prompt, agent, resume_id=None, blank_memory=False:
                        seen.update(blank=blank_memory) or 0)
    assert codex_run.main(["@researcher", "task"]) == 0
    assert seen["blank"] is True
