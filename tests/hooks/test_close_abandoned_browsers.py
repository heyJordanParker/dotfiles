"""The stop-time browser cleanup: which stops clean up, which sessions get
closed, and never blocking."""

import io
import json
import sys

import close_abandoned_browsers as hook
import pytest


def _bash(command):
    """A Claude assistant record running one Bash command."""
    return {"type": "assistant", "message": {"role": "assistant", "content": [
        {"type": "tool_use", "id": "t1", "name": "Bash", "input": {"command": command}}]}}


def _codex_function_call(command):
    """The older codex rollout shape: function_call/exec_command with a JSON
    arguments string."""
    return {"timestamp": "2026-07-28T10:00:00.000Z", "type": "response_item", "payload": {
        "type": "function_call", "id": "fc_1", "name": "exec_command",
        "arguments": json.dumps({"cmd": command, "yield_time_ms": 10000}),
        "call_id": "call_1"}}


def _codex_exec(*commands):
    """The shape current codex rollouts actually record: a custom_tool_call named
    `exec` whose input is JavaScript awaiting tools.exec_command({cmd: ...}) —
    unquoted key, JSON-escaped value, possibly several per call."""
    calls = ",\n".join(
        'tools.exec_command({cmd:%s,workdir:"/Users/jordan/dotfiles",yield_time_ms:10000})'
        % json.dumps(c) for c in commands)
    return {"timestamp": "2026-07-28T11:19:12.058Z", "type": "response_item", "payload": {
        "type": "custom_tool_call", "id": "ctc_1", "status": "completed",
        "call_id": "call_1", "name": "exec",
        "input": "const ps=[\n%s\n];\nfor (const p of ps) text((await p).output);\n" % calls}}


def _codex_local_shell(command):
    """A local_shell_call carrying the command as a `/bin/zsh -lc` argv list."""
    return {"timestamp": "2026-07-28T10:00:00.000Z", "type": "response_item", "payload": {
        "type": "local_shell_call", "id": "lsc_1", "call_id": "call_1",
        "action": {"type": "exec", "command": ["/bin/zsh", "-lc", command]}}}


@pytest.fixture
def closed(monkeypatch):
    """Record the argv of every agent-browser run."""
    calls = []

    def _run(argv):
        calls.append(argv)
        return ""

    monkeypatch.setattr(hook, "_run", _run)
    return calls


def _fire(monkeypatch, event):
    monkeypatch.setattr(sys, "stdin", io.StringIO(json.dumps(event)))
    return hook.main()


def _subagent_stop(monkeypatch, path):
    return _fire(monkeypatch, {"hook_event_name": "SubagentStop", "agent_id": "a1",
                               "agent_type": "ponytail", "agent_transcript_path": path,
                               "transcript_path": "/parent/transcript.jsonl"})


def _codex_stop(monkeypatch, path):
    return _fire(monkeypatch, {"hook_event_name": "Stop", "transcript_path": path,
                               "turn_id": "019fa872-f904-7bc1-afd4-ccc735ac0a12",
                               "model": "gpt-5.4-codex"})


# --- which stops clean up ----------------------------------------------------

def test_claude_main_session_stop_closes_nothing(monkeypatch, write_transcript, closed):
    """Claude's Stop fires after every assistant turn, so cleaning up there would
    close the browser between the architect's instructions."""
    path = write_transcript([_bash("agent-browser --session probe open example.com")])
    assert _fire(monkeypatch, {"hook_event_name": "Stop", "transcript_path": path}) == 0
    assert closed == []






def test_subagent_stop_reads_the_subagents_own_transcript(monkeypatch, write_transcript, closed):
    """SubagentStop carries the parent's transcript_path too; the subagent's own
    work is the one that has to be cleaned up."""
    mine = write_transcript([_bash("agent-browser --session probe open example.com")], "mine.jsonl")
    assert _subagent_stop(monkeypatch, mine) == 0
    assert ["agent-browser", "--session", "probe", "close"] in closed








# --- ownership ---------------------------------------------------------------





@pytest.mark.parametrize("command", [
    "agent-browser --session probe snapshot",
    "agent-browser --session probe --restore snapshot",
    "agent-browser --session probe --restore saved snapshot",
])
def test_reaching_a_browser_claims_the_named_session(monkeypatch, write_transcript, closed, command):
    """Verified against 0.33.1: these all report browserLaunched true on a fresh
    session, so an open-only claim would leak every one of them. `--restore` takes
    its name only when the next token is not a subcommand, which 0.33.1 decides
    the same way: `--restore snapshot` snapshots, `--restore saved snapshot`
    restores `saved` and snapshots."""
    path = write_transcript([_bash(command)])
    assert _subagent_stop(monkeypatch, path) == 0
    assert ["agent-browser", "--session", "probe", "close"] in closed


@pytest.mark.parametrize("command", [
    "agent-browser --session probe skills get core --full",
    "agent-browser --session probe --restore saved read example.com",
])
def test_a_subcommand_that_never_reaches_a_browser_claims_nothing(
        monkeypatch, write_transcript, closed, command):
    path = write_transcript([_bash(command)])
    assert _subagent_stop(monkeypatch, path) == 0
    assert closed == []




# --- execution position ------------------------------------------------------







# --- codex rollout shapes ----------------------------------------------------





def _codex_exec_raw(js):
    """A codex `exec` call carrying arbitrary JavaScript as its input."""
    return {"timestamp": "2026-07-28T11:19:12.058Z", "type": "response_item", "payload": {
        "type": "custom_tool_call", "id": "ctc_1", "status": "completed",
        "call_id": "call_1", "name": "exec", "input": js}}








# --- prefix commands ---------------------------------------------------------



# --- chronology --------------------------------------------------------------











# --- session identity --------------------------------------------------------















# --- close behavior and failure ----------------------------------------------





