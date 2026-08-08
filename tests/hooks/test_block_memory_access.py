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
from test_codex_run import _sent, _stub_codex  # noqa: E402

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


def _run(monkeypatch, agent_type, command="honcho context jordan",
         agent_id="a1d6344fe2b8d8332"):
    """A subagent's PreToolUse payload by default: both fields present, which is
    the shape observed from a real Agent-tool dispatch. `agent_id=None` is the
    main thread, which carries `agent_type` alone when started with --agent."""
    event = {"hook_event_name": "PreToolUse", "tool_name": "Bash",
             "tool_input": {"command": command}}
    if agent_type is not None:
        event["agent_type"] = agent_type
    if agent_id is not None:
        event["agent_id"] = agent_id
    monkeypatch.setattr(sys, "stdin", io.StringIO(json.dumps(event)))
    return block_memory_access.main()


def test_only_the_honcho_command_is_gated(config_root, monkeypatch):
    """The gate reads the command, so a blank-declared agent still runs every
    other command — and a mention of the word in prose is not a memory call."""
    config_root("researcher", BLANK)
    assert _run(monkeypatch, "researcher", command="git status") == 0
    assert _run(monkeypatch, "researcher", command="echo honcho-is-off") == 0
    assert _run(monkeypatch, "researcher", command="honcho remember r 'x'") == 2


def test_the_command_is_gated_through_a_path_or_a_chain(config_root, monkeypatch):
    config_root("researcher", BLANK)
    assert _run(monkeypatch, "researcher", command="~/bin/honcho list") == 2
    assert _run(monkeypatch, "researcher", command="cd /tmp && honcho list") == 2


def test_codex_run_is_gated_by_its_exported_definition(tmp_path, monkeypatch):
    """codex names no agent in the payload, so the gate reads the definition path
    its launcher exported — the same variable the codex runner sets."""
    definition = tmp_path / "researcher.md"
    definition.write_text(BLANK % "researcher")
    monkeypatch.setenv("CODEX_RUN_AGENT_FILE", str(definition))
    assert _run(monkeypatch, None, agent_id=None) == 2

    definition.write_text(UNDECLARED % "researcher")
    assert _run(monkeypatch, None, agent_id=None) == 0


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


def _memory_config(monkeypatch, tmp_path, argv):
    """What the run's thread request asked codex for, under a stub app-server."""
    log = _stub_codex(monkeypatch, tmp_path)
    assert codex_run.main(argv) == 0
    method = "thread/resume" if argv[0] == "resume" else "thread/start"
    return _sent(log, method)["config"]


def test_codex_blank_agent_loses_its_own_memories(monkeypatch, tmp_path):
    """codex's own [memories] needs no tool call at all — it injects a Memory
    section straight into the run — so the declaration has to switch it off in
    the thread config. Honcho is the other provider and is gated at the command."""
    _pin(monkeypatch, tmp_path, "researcher", BLANK)
    config = _memory_config(monkeypatch, tmp_path, ["@researcher", "task"])
    assert config["memories"] == {"use_memories": False, "generate_memories": False}


def test_codex_undeclared_agent_keeps_memory(monkeypatch, tmp_path):
    _pin(monkeypatch, tmp_path, "architect", UNDECLARED)
    config = _memory_config(monkeypatch, tmp_path, ["@architect", "task"])
    assert "memories" not in config


def test_blank_memory_keeps_the_docs_server(monkeypatch, tmp_path):
    """The memory config touches no server registry, so every agent declaring
    `memory: none` keeps its other servers — the researcher among them, whose job
    is library docs."""
    _pin(monkeypatch, tmp_path, "researcher", BLANK)
    config = _memory_config(monkeypatch, tmp_path, ["@researcher", "task"])
    assert "context7" in config["mcp_servers"]


def test_codex_resume_applies_the_founding_agents_declaration(monkeypatch, tmp_path):
    """The declaration follows the thread across a resume. The founding agent is
    read off the job record, so a thread founded blank stays blank on continuation
    and a thread founded by an agent that keeps memory keeps it."""
    _pin(monkeypatch, tmp_path, "researcher", BLANK)
    _pin(monkeypatch, tmp_path, "architect", UNDECLARED)
    jobs = {}
    for name in ("researcher", "architect"):
        _stub_codex(monkeypatch, tmp_path, thread="th_" + name)
        assert codex_run.main(["@" + name, "task"]) == 0
        jobs[name] = next(json.loads(p.read_text()) for p in tmp_path.glob("codex-run-*.json")
                          if json.loads(p.read_text())["agent"] == name)["job"]

    blank = _memory_config(monkeypatch, tmp_path, ["resume", jobs["researcher"], "continue"])
    keeps = _memory_config(monkeypatch, tmp_path, ["resume", jobs["architect"], "continue"])
    assert blank["memories"]["use_memories"] is False
    assert "memories" not in keeps


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
    config = _memory_config(monkeypatch, tmp_path, ["@researcher", "task"])
    assert config["memories"]["use_memories"] is False
