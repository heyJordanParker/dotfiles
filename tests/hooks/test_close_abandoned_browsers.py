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


def test_codex_stop_is_told_apart_by_its_payload(monkeypatch, write_transcript, closed):
    path = write_transcript([_codex_function_call("agent-browser --session probe open a.com")])
    assert _codex_stop(monkeypatch, path) == 0
    assert ["agent-browser", "--session", "probe", "close"] in closed


def test_codex_stop_is_told_apart_by_its_rollout_path(monkeypatch, write_transcript, closed):
    path = write_transcript([_codex_exec("agent-browser --session probe open a.com")],
                            "rollout-2026-07-28T10-00-00-019fa872.jsonl")
    assert _fire(monkeypatch, {"hook_event_name": "Stop", "transcript_path": path}) == 0
    assert ["agent-browser", "--session", "probe", "close"] in closed


def test_subagent_stop_reads_the_subagents_own_transcript(monkeypatch, write_transcript, closed):
    """SubagentStop carries the parent's transcript_path too; the subagent's own
    work is the one that has to be cleaned up."""
    mine = write_transcript([_bash("agent-browser --session probe open example.com")], "mine.jsonl")
    assert _subagent_stop(monkeypatch, mine) == 0
    assert ["agent-browser", "--session", "probe", "close"] in closed


def test_no_transcript_path_does_nothing(monkeypatch, closed):
    assert _fire(monkeypatch, {"hook_event_name": "SubagentStop"}) == 0
    assert closed == []


def test_subagent_stop_without_its_own_transcript_never_reads_the_parents(
        monkeypatch, write_transcript, closed):
    """Falling back to `transcript_path` cleans up from the parent's transcript
    and closes the parent's live sessions."""
    parent = write_transcript([_bash("agent-browser --session parent open example.com")])
    assert _fire(monkeypatch, {"hook_event_name": "SubagentStop", "agent_id": "a1",
                               "transcript_path": parent}) == 0
    assert closed == []


def test_unreadable_transcript_path_does_nothing(monkeypatch, tmp_path, closed):
    assert _subagent_stop(monkeypatch, str(tmp_path / "gone.jsonl")) == 0
    assert closed == []


# --- ownership ---------------------------------------------------------------

def test_unclosed_named_session_is_closed(monkeypatch, write_transcript, closed):
    path = write_transcript([_bash("agent-browser --session probe open example.com")])
    assert _subagent_stop(monkeypatch, path) == 0
    assert ["agent-browser", "--session", "probe", "close"] in closed


@pytest.mark.parametrize("command", [
    "agent-browser open example.com",
    "agent-browser snapshot",
    "AGENT_BROWSER_SESSION=default agent-browser open example.com",
])
def test_the_shared_default_session_is_never_closed(monkeypatch, write_transcript, closed, command):
    """`default` is where siblings collide, so it is never attributable to one
    agent; its leaks are agent-browser's idle timeout's job."""
    path = write_transcript([_bash(command)])
    assert _subagent_stop(monkeypatch, path) == 0
    assert closed == []


def test_closed_session_is_left_alone(monkeypatch, write_transcript, closed):
    path = write_transcript([
        _bash("agent-browser --session probe open example.com"),
        _bash("agent-browser --session probe close"),
    ])
    assert _subagent_stop(monkeypatch, path) == 0
    assert closed == []


@pytest.mark.parametrize("command", [
    "agent-browser --session probe snapshot",
    "agent-browser --session probe get title",
    "agent-browser --session probe click @e2",
    "agent-browser --session probe eval 'document.title'",
    "agent-browser --session probe screenshot",
    "agent-browser --session probe connect 9222",
    "agent-browser --session probe goto example.com",
])
def test_reaching_a_browser_claims_the_named_session(monkeypatch, write_transcript, closed, command):
    """Verified against 0.33.1: these all report browserLaunched true on a fresh
    session, so an open-only claim would leak every one of them."""
    path = write_transcript([_bash(command)])
    assert _subagent_stop(monkeypatch, path) == 0
    assert ["agent-browser", "--session", "probe", "close"] in closed


@pytest.mark.parametrize("command", [
    "agent-browser --session probe skills get core --full",
    "agent-browser --session probe session list",
    "agent-browser --session probe profiles",
    "agent-browser --session probe read example.com",
    "agent-browser --session probe help",
    "agent-browser --session probe version",
    "agent-browser --session probe install",
    "agent-browser --session probe dashboard start",
])
def test_a_subcommand_that_never_reaches_a_browser_claims_nothing(
        monkeypatch, write_transcript, closed, command):
    path = write_transcript([_bash(command)])
    assert _subagent_stop(monkeypatch, path) == 0
    assert closed == []


def test_quoted_example_commands_in_output_are_ignored(monkeypatch, write_transcript, closed):
    """agent-browser's help quotes example commands; those arrive as tool results,
    not tool calls, and must not be read as sessions the agent opened."""
    path = write_transcript([{"type": "user", "message": {"role": "user", "content": [
        {"type": "tool_result", "tool_use_id": "t1",
         "content": "Examples:\n  agent-browser --session demo open example.com"}]}}])
    assert _subagent_stop(monkeypatch, path) == 0
    assert closed == []


# --- execution position ------------------------------------------------------

@pytest.mark.parametrize("command", [
    "echo agent-browser --session sibling open x",
    "grep agent-browser --session sibling open x",
    "printf '%s\\n' 'agent-browser --session sibling open x'",
    "cat > /tmp/note.sh <<'EOF'\nagent-browser --session sibling open x\nEOF",
    "git commit -m 'agent-browser --session sibling open x'",
])
def test_the_command_as_data_claims_nothing(monkeypatch, write_transcript, closed, command):
    """A sibling's `sibling` session must survive its name appearing in echoed,
    grepped, or heredoc'd text — the executable has to be in executed position."""
    path = write_transcript([_bash(command)])
    assert _subagent_stop(monkeypatch, path) == 0
    assert closed == []


@pytest.mark.parametrize("command", [
    "bash -s <<'SH'\nagent-browser --session probe open example.com\nSH",
    "bash <<'SH'\nagent-browser --session probe snapshot\nSH",
])
def test_a_heredoc_a_shell_reads_on_stdin_is_the_script(
        monkeypatch, write_transcript, closed, command):
    """`bash -s <<'SH'` runs its body, so the body is executed position — unlike a
    `cat > file` heredoc, which only writes it."""
    path = write_transcript([_bash(command)])
    assert _subagent_stop(monkeypatch, path) == 0
    assert ["agent-browser", "--session", "probe", "close"] in closed


@pytest.mark.parametrize("command", [
    "env bash -lc 'agent-browser --session probe open example.com'",
    "sudo bash -lc 'agent-browser --session probe open example.com'",
    "timeout 60 bash -lc 'agent-browser --session probe open example.com'",
    "env FOO=1 timeout 60 zsh -lc 'agent-browser --session probe snapshot'",
])
def test_a_prefix_around_a_wrapper_is_unwrapped(monkeypatch, write_transcript, closed, command):
    path = write_transcript([_bash(command)])
    assert _subagent_stop(monkeypatch, path) == 0
    assert ["agent-browser", "--session", "probe", "close"] in closed


# --- flag parsing ------------------------------------------------------------

@pytest.mark.parametrize("command", [
    "agent-browser --timeout 5 close",
    "agent-browser --verbose close",
    "agent-browser --json close",
])
def test_a_close_never_registers_as_an_open(monkeypatch, write_transcript, closed, command):
    """An unknown value-flag used to donate its value as the subcommand, reading
    `--timeout 5 close` as an open of a session called `5`."""
    path = write_transcript([_bash(command)])
    assert _subagent_stop(monkeypatch, path) == 0
    assert closed == []


def test_global_value_flags_do_not_shadow_the_subcommand(monkeypatch, write_transcript, closed):
    path = write_transcript([
        _bash("agent-browser --color-scheme dark --profile Default open example.com"),
        _bash("agent-browser --idle-timeout 10s --engine chrome close"),
    ])
    assert _subagent_stop(monkeypatch, path) == 0
    assert closed == []


def test_restore_takes_its_optional_name_without_eating_the_subcommand(monkeypatch, write_transcript, closed):
    path = write_transcript([_bash("agent-browser --session probe --restore open example.com")])
    assert _subagent_stop(monkeypatch, path) == 0
    assert ["agent-browser", "--session", "probe", "close"] in closed


# --- codex rollout shapes ----------------------------------------------------

def test_codex_exec_javascript_shape_is_read(monkeypatch, write_transcript, closed):
    path = write_transcript([_codex_exec("agent-browser --session probe open example.com")])
    assert _codex_stop(monkeypatch, path) == 0
    assert ["agent-browser", "--session", "probe", "close"] in closed


def test_codex_exec_reads_every_command_in_one_call(monkeypatch, write_transcript, closed):
    path = write_transcript([_codex_exec("agent-browser --session one open a.com",
                                         "agent-browser --session two open b.com")])
    assert _codex_stop(monkeypatch, path) == 0
    assert ["agent-browser", "--session", "one", "close"] in closed
    assert ["agent-browser", "--session", "two", "close"] in closed


def _codex_exec_raw(js):
    """A codex `exec` call carrying arbitrary JavaScript as its input."""
    return {"timestamp": "2026-07-28T11:19:12.058Z", "type": "response_item", "payload": {
        "type": "custom_tool_call", "id": "ctc_1", "status": "completed",
        "call_id": "call_1", "name": "exec", "input": js}}


@pytest.mark.parametrize("js", [
    'if (false) await tools.exec_command({cmd:"agent-browser --session sibling open a"})',
    'for (const x of []) await tools.exec_command({cmd:"agent-browser --session sibling open a"})',
    'ok && await tools.exec_command({cmd:"agent-browser --session sibling open a"})',
    'const spec = {cmd:"agent-browser --session sibling open a"};',
    'function later() { tools.exec_command({cmd:"agent-browser --session sibling open a"}) }',
    'const later = () => tools.exec_command({cmd:"agent-browser --session sibling open a"});',
])
def test_javascript_that_may_not_have_run_claims_nothing(
        monkeypatch, write_transcript, closed, js):
    """The hook does not evaluate JavaScript, so a guarded call and a bare object
    literal are both ambiguous — skipping them leaks, closing them is a wrong close."""
    path = write_transcript([_codex_exec_raw(js)])
    assert _codex_stop(monkeypatch, path) == 0
    assert closed == []


@pytest.mark.parametrize("js", [
    "await tools.exec_command({cmd:'agent-browser --session probe open a'})",
    "await tools.exec_command({cmd:`agent-browser --session probe open a`})",
    "const c = 'agent-browser --session probe open a';"
    " await tools.exec_command({cmd:c})",
])
def test_a_cmd_that_is_not_a_double_quoted_literal_is_skipped(
        monkeypatch, write_transcript, closed, js):
    """Reading these would mean evaluating JavaScript; skipping them leaks the
    session to agent-browser's idle timeout instead of guessing."""
    path = write_transcript([_codex_exec_raw(js)])
    assert _codex_stop(monkeypatch, path) == 0
    assert closed == []


def test_codex_zsh_argv_list_keeps_the_command(monkeypatch, write_transcript, closed):
    path = write_transcript([_codex_local_shell("agent-browser --session probe open a.com")])
    assert _codex_stop(monkeypatch, path) == 0
    assert ["agent-browser", "--session", "probe", "close"] in closed


# --- prefix commands ---------------------------------------------------------

@pytest.mark.parametrize("command", [
    "timeout 60 agent-browser --session probe open example.com",
    "sudo agent-browser --session probe open example.com",
    "bunx agent-browser --session probe open example.com",
    "npx agent-browser --session probe open example.com",
    "bash -lc 'agent-browser --session probe open example.com'",
    "/opt/homebrew/bin/agent-browser --session probe open example.com",
])
def test_prefixed_invocations_are_visible(monkeypatch, write_transcript, closed, command):
    path = write_transcript([_bash(command)])
    assert _subagent_stop(monkeypatch, path) == 0
    assert ["agent-browser", "--session", "probe", "close"] in closed


# --- chronology --------------------------------------------------------------

def test_close_all_does_not_abort_the_rest_of_the_scan(monkeypatch, write_transcript, closed):
    path = write_transcript([
        _bash("agent-browser --session a open example.com"),
        _bash("agent-browser close --all"),
        _bash("agent-browser --session b open example.com"),
    ])
    assert _subagent_stop(monkeypatch, path) == 0
    assert ["agent-browser", "--session", "b", "close"] in closed
    assert ["agent-browser", "--session", "a", "close"] not in closed


def test_close_all_is_scoped_to_its_own_namespace(monkeypatch, write_transcript, closed):
    """A namespaced `close --all` closes only that namespace's sessions, so
    clearing every namespace's replay state leaks the bare-namespace one."""
    path = write_transcript([
        _bash("agent-browser --session probe open example.com"),
        _bash("agent-browser --namespace child --session probe open example.com"),
        _bash("agent-browser --namespace child close --all"),
    ])
    assert _subagent_stop(monkeypatch, path) == 0
    assert ["agent-browser", "--session", "probe", "close"] in closed
    assert ["agent-browser", "--namespace", "child", "--session", "probe",
            "close"] not in closed


def test_a_session_first_seen_via_close_is_still_ordered(monkeypatch, write_transcript, closed):
    """`close p` before any open used to leave `p` out of the replay order, so the
    later open was tracked but never returned."""
    path = write_transcript([
        _bash("agent-browser --session p close"),
        _bash("agent-browser --session p open example.com"),
    ])
    assert _subagent_stop(monkeypatch, path) == 0
    assert ["agent-browser", "--session", "p", "close"] in closed


def test_reopening_after_a_close_leaves_it_open(monkeypatch, write_transcript, closed):
    path = write_transcript([
        _bash("agent-browser --session p open example.com"),
        _bash("agent-browser --session p close"),
        _bash("agent-browser --session p open example.com"),
    ])
    assert _subagent_stop(monkeypatch, path) == 0
    assert ["agent-browser", "--session", "p", "close"] in closed


def test_chained_commands_are_read_per_segment(monkeypatch, write_transcript, closed):
    path = write_transcript([_bash(
        "agent-browser --session probe open example.com && agent-browser --session probe snapshot")])
    assert _subagent_stop(monkeypatch, path) == 0
    assert closed.count(["agent-browser", "--session", "probe", "close"]) == 1


# --- session identity --------------------------------------------------------

def test_environment_prefix_names_the_session(monkeypatch, write_transcript, closed):
    path = write_transcript([_bash("AGENT_BROWSER_SESSION=probe agent-browser open example.com")])
    assert _subagent_stop(monkeypatch, path) == 0
    assert ["agent-browser", "--session", "probe", "close"] in closed


def test_an_exported_session_carries_forward(monkeypatch, write_transcript, closed):
    """The export and the invocation land in separate transcript commands, so the
    later `open` is `probe`, not `default` — and `default` would never be closed."""
    path = write_transcript([
        _bash("export AGENT_BROWSER_SESSION=probe"),
        _bash("agent-browser open example.com"),
    ])
    assert _subagent_stop(monkeypatch, path) == 0
    assert ["agent-browser", "--session", "probe", "close"] in closed


def test_an_exported_namespace_carries_forward(monkeypatch, write_transcript, closed):
    path = write_transcript([
        _bash("export AGENT_BROWSER_NAMESPACE=child"),
        _bash("agent-browser --session probe open example.com"),
    ])
    assert _subagent_stop(monkeypatch, path) == 0
    assert ["agent-browser", "--namespace", "child", "--session", "probe", "close"] in closed


def test_an_explicit_flag_beats_the_carried_export(monkeypatch, write_transcript, closed):
    path = write_transcript([
        _bash("export AGENT_BROWSER_SESSION=probe"),
        _bash("agent-browser --session other open example.com"),
    ])
    assert _subagent_stop(monkeypatch, path) == 0
    assert ["agent-browser", "--session", "other", "close"] in closed
    assert ["agent-browser", "--session", "probe", "close"] not in closed


def test_namespace_survives_to_the_close(monkeypatch, write_transcript, closed):
    path = write_transcript([
        _bash("agent-browser --namespace child --session probe open example.com")])
    assert _subagent_stop(monkeypatch, path) == 0
    assert ["agent-browser", "--namespace", "child", "--session", "probe", "close"] in closed


def test_a_namespaced_session_is_not_the_bare_one(monkeypatch, write_transcript, closed):
    path = write_transcript([
        _bash("agent-browser --namespace child --session probe open example.com"),
        _bash("agent-browser --session probe close"),
    ])
    assert _subagent_stop(monkeypatch, path) == 0
    assert ["agent-browser", "--namespace", "child", "--session", "probe", "close"] in closed


@pytest.mark.parametrize("command", [
    'agent-browser --session "$SESSION" open example.com',
    "agent-browser --session $SESSION open example.com",
    'agent-browser --namespace "$NS" --session probe open example.com',
])
def test_an_unresolvable_session_name_is_skipped(monkeypatch, write_transcript, closed, command):
    """A literal `$SESSION` is not a session; probing it talks to the wrong thing."""
    path = write_transcript([_bash(command)])
    assert _subagent_stop(monkeypatch, path) == 0
    assert closed == []


# --- close behavior and failure ----------------------------------------------

def test_close_is_unconditional_and_never_probes(monkeypatch, write_transcript, closed):
    """`close` on a dead session is a clean no-op, so an abandoned session is
    closed directly — a liveness probe would only spend an extra subprocess per
    session to skip no-op closes."""
    path = write_transcript([_bash("agent-browser --session probe open example.com")])
    assert _subagent_stop(monkeypatch, path) == 0
    assert ["agent-browser", "--session", "probe", "close"] in closed
    assert not any("info" in argv for argv in closed)


def test_close_failure_neither_raises_nor_blocks(monkeypatch, write_transcript):
    def _explode(argv):
        raise OSError("agent-browser: command not found")

    monkeypatch.setattr(hook, "_run", _explode)
    path = write_transcript([_bash("agent-browser --session probe open example.com")])
    assert _subagent_stop(monkeypatch, path) == 0


def test_many_sessions_fit_the_timeout(monkeypatch, write_transcript, closed):
    """Per-session close is bounded and the sessions run concurrently, so a
    multi-session cleanup is not killed midway by the BINDING timeout."""
    opens = [_bash("agent-browser --session s%d open example.com" % i) for i in range(24)]
    path = write_transcript(opens)
    assert _subagent_stop(monkeypatch, path) == 0
    for i in range(24):
        assert ["agent-browser", "--session", "s%d" % i, "close"] in closed
    assert hook.BINDING["timeout"] >= hook._BUDGET
    assert hook._BUDGET >= 24 / hook._WORKERS * hook._SUBPROCESS_TIMEOUT
