"""Coverage for the memory namer's two harnesses.

`honcho remember` takes the agent from the environment, which a subagent shares
with whoever started it. On Claude the hook writes `--as <agent>` into the command
through `updatedInput`. codex discards that output unless it also carries
`permissionDecision: "allow"` — a grant that would approve the call over every gate
ordered behind this one — so a codex subagent is refused and told to name itself.

Live evidence for the codex half: a child spawned as `worker` under `cto` ran
`honcho remember` and answered `remembered for cto`.

The rewrite's own shapes — command position, a quoted mention, an already-named
call — are `rewritten` and `invoked`, asserted directly. Only the harness decision
lives here.
"""

import io
import json
import sys

import name_memory_caller

ALLOW, BLOCK = 0, 2

CLAUDE_SUBAGENT = {"agent_type": "code-reviewer"}
CODEX_SUBAGENT = {"agent_type": "worker", "turn_id": "turn-1"}
CODEX_OWN_THREAD = {"turn_id": "turn-1"}


def _run(payload, monkeypatch, command='honcho remember "x"'):
    monkeypatch.delenv("CODEX_RUN_AGENT_FILE", raising=False)
    body = json.dumps({"tool_name": "Bash", "tool_input": {"command": command},
                       **payload})
    monkeypatch.setattr(sys, "stdin", io.StringIO(body))
    return name_memory_caller.main()


def test_a_claude_subagent_is_named_in_the_command(monkeypatch, capsys):
    assert _run(CLAUDE_SUBAGENT, monkeypatch) == ALLOW
    written = json.loads(capsys.readouterr().out)
    assert written["hookSpecificOutput"]["updatedInput"]["command"] == (
        'honcho remember --as code-reviewer "x"')


def test_a_codex_subagent_is_refused_rather_than_rewritten(monkeypatch, capsys):
    """codex discards the rewrite, so the write would land under the parent."""
    assert _run(CODEX_SUBAGENT, monkeypatch) == BLOCK
    assert "--as worker" in capsys.readouterr().err


def test_a_codex_subagent_that_names_itself_runs(monkeypatch):
    assert _run(CODEX_SUBAGENT, monkeypatch,
                command='honcho remember --as worker "x"') == ALLOW


def test_a_codex_run_own_thread_is_left_alone(monkeypatch, capsys):
    """Its environment already names it, so there is nothing to fix."""
    monkeypatch.setenv("CODEX_RUN_AGENT_FILE", "/agents/cto.md")
    body = json.dumps({"tool_name": "Bash",
                       "tool_input": {"command": 'honcho remember "x"'},
                       **CODEX_OWN_THREAD})
    monkeypatch.setattr(sys, "stdin", io.StringIO(body))
    assert name_memory_caller.main() == ALLOW
    assert capsys.readouterr().out == ""
