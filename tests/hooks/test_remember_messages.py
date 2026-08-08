"""Behavior of the two Honcho write hooks and the provenance test beneath them.

remember_architect_message.py stores a turn's prompt as the architect only when the
harness says he typed it; remember_agent_message.py stores the turn's replies under
the running agent's own peer. Both are exercised against a fake config and a
captured `honcho.post`, so nothing here reaches the network.
"""

import json
import subprocess

import pytest
import remember_agent_message
import remember_architect_message
from lib import honcho, transcript

CFG = {
    "peerName": "jordan",
    "workspace": "parkerlabs",
    "endpoint": {"baseUrl": "https://example.invalid"},
    "sessions": {"/repo": "jordan-repo"},
}


def user(text, source=None, meta=False, extra=None):
    rec = {"type": "user", "message": {"content": text}, "timestamp": "2026-08-05T00:00:00Z"}
    if source:
        rec["promptSource"] = source
    if meta:
        rec["isMeta"] = True
    rec.update(extra or {})
    return rec


def assistant(text):
    return {"type": "assistant", "message": {"content": [{"type": "text", "text": text}]}}


def tool_result():
    return {"type": "user", "message": {"content": [{"type": "tool_result", "content": "ok"}]}}


@pytest.fixture
def captured(monkeypatch):
    """Replace the network call; collect (session, peer, text) per module."""
    sent = []

    def fake_post(cfg, session, peer, text, created_at=None, metadata=None):
        sent.append({"session": session, "peer": peer, "text": text, "metadata": metadata})
        return True

    for module in (remember_architect_message, remember_agent_message):
        monkeypatch.setattr(module.honcho, "config", lambda: CFG)
        monkeypatch.setattr(module.honcho, "post", fake_post)
    return sent


def fire(module, monkeypatch, capsys, event):
    monkeypatch.setattr("sys.stdin", _Stdin(json.dumps(event)))
    rc = module.main()
    capsys.readouterr()
    return rc


class _Stdin:
    def __init__(self, text):
        self._text = text

    def read(self):
        return self._text


# --- provenance ---------------------------------------------------------------

@pytest.mark.parametrize("source,expected", [
    ("typed", True),
    ("queued", True),
    ("system", False),
    (None, False),
    ("", False),
    ("TYPED", False),
    ("future_source_anthropic_adds", False),
])
def test_is_architect_is_an_allowlist(source, expected):
    assert transcript.is_architect(user("hi", source)) is expected


def test_latest_user_record_skips_tool_results():
    recs = [user("first", "typed"), assistant("a"), tool_result()]
    assert transcript.text_of(transcript.latest_user_record(recs)) == "first"


# --- the architect's messages --------------------------------------------------

def test_typed_prompt_is_stored_as_the_architect(captured, monkeypatch, capsys, write_transcript):
    path = write_transcript([user("fix the thing", "typed")])
    fire(remember_architect_message, monkeypatch, capsys,
         {"transcript_path": path, "cwd": "/repo", "session_id": "s1"})
    assert len(captured) == 1
    assert captured[0]["peer"] == "jordan"
    assert captured[0]["session"] == "repo"
    assert captured[0]["text"] == "fix the thing"


def test_task_notification_is_not_stored(captured, monkeypatch, capsys, write_transcript):
    path = write_transcript([
        user("fix the thing", "typed"),
        assistant("working"),
        user("<task-notification><status>completed</status></task-notification>", "system"),
    ])
    fire(remember_architect_message, monkeypatch, capsys,
         {"transcript_path": path, "cwd": "/repo", "session_id": "s1"})
    assert captured == []


def test_hook_injected_block_is_not_stored(captured, monkeypatch, capsys, write_transcript):
    path = write_transcript([user("<local-command-caveat>x</local-command-caveat>", meta=True)])
    fire(remember_architect_message, monkeypatch, capsys,
         {"transcript_path": path, "cwd": "/repo", "session_id": "s1"})
    assert captured == []


def test_bracketed_typed_message_is_stored(captured, monkeypatch, capsys, write_transcript):
    """An image paste starts with '[' — provenance keeps it, a prefix test lost it."""
    path = write_transcript([user("[Image #1] this pops up and steals focus", "typed")])
    fire(remember_architect_message, monkeypatch, capsys,
         {"transcript_path": path, "cwd": "/repo", "session_id": "s1"})
    assert len(captured) == 1
    assert captured[0]["text"].startswith("[Image #1]")


def test_subagent_turn_does_not_restore_the_parents_prompt(captured, monkeypatch, capsys,
                                                           write_transcript):
    path = write_transcript([user("fix the thing", "typed")])
    fire(remember_architect_message, monkeypatch, capsys,
         {"transcript_path": path, "cwd": "/repo", "session_id": "s1", "isSidechain": True})
    assert captured == []


def test_memory_disabled_stores_nothing(monkeypatch, capsys, write_transcript):
    sent = []
    monkeypatch.setattr(remember_architect_message.honcho, "config",
                        lambda: dict(CFG, enabled=False))
    monkeypatch.setattr(remember_architect_message.honcho, "post",
                        lambda *a, **k: sent.append(a) or True)
    path = write_transcript([user("fix the thing", "typed")])
    fire(remember_architect_message, monkeypatch, capsys,
         {"transcript_path": path, "cwd": "/repo", "session_id": "s1"})
    assert sent == []


# --- the agent's replies -------------------------------------------------------

def test_turn_replies_are_stored_under_the_running_agent(captured, monkeypatch, capsys,
                                                         write_transcript):
    path = write_transcript([
        user("fix the thing", "typed"),
        assistant("looking now"),
        assistant("fixed it"),
    ])
    fire(remember_agent_message, monkeypatch, capsys,
         {"transcript_path": path, "cwd": "/repo", "session_id": "s1", "agent_type": "cto"})
    assert len(captured) == 1
    assert captured[0]["peer"] == "cto"
    assert captured[0]["text"] == "looking now\n\nfixed it"


def test_a_subagent_writes_into_its_own_peer_from_its_own_transcript(
        captured, monkeypatch, capsys, write_transcript):
    """The reason subagents are recorded at all: ponytail's replies are how Honcho
    builds its picture of ponytail. The payload's `transcript_path` is the
    parent's, so a subagent stop reads the one it carries for itself."""
    parent = write_transcript([user("dispatch", "typed"), assistant("orchestrating")])
    own = write_transcript([user("brief", "typed"), assistant("ponytail's finding")])
    fire(remember_agent_message, monkeypatch, capsys,
         {"hook_event_name": "SubagentStop", "transcript_path": parent,
          "agent_transcript_path": own, "cwd": "/repo", "session_id": "s1",
          "agent_id": "a1", "agent_type": "ponytail"})
    assert len(captured) == 1
    assert captured[0]["peer"] == "ponytail"
    assert captured[0]["text"] == "ponytail's finding"


def test_a_turn_with_no_agent_behind_it_is_not_stored(captured, monkeypatch, capsys,
                                                      write_transcript):
    """Memory is per agent here; a nameless collection is the pile this replaced."""
    monkeypatch.delenv("CODEX_RUN_AGENT_FILE", raising=False)
    path = write_transcript([user("fix the thing", "typed"), assistant("done")])
    fire(remember_agent_message, monkeypatch, capsys,
         {"transcript_path": path, "cwd": "/repo", "session_id": "s1"})
    assert captured == []


def test_an_agent_declaring_no_memory_stores_nothing(captured, monkeypatch, capsys,
                                                     write_transcript, tmp_path):
    """A collection built out of its output is a memory of that agent, which is
    the thing the declaration denies."""
    monkeypatch.setenv("CLAUDE_CONFIG_DIR", str(tmp_path))
    (tmp_path / "agents").mkdir()
    (tmp_path / "agents" / "explorer.md").write_text("---\nname: explorer\nmemory: none\n---\n")
    path = write_transcript([user("map the callers", "typed"), assistant("here they are")])
    fire(remember_agent_message, monkeypatch, capsys,
         {"transcript_path": path, "cwd": "/repo", "session_id": "s1",
          "agent_id": "a1", "agent_type": "explorer"})
    assert captured == []


def test_replies_before_this_turn_are_not_restored(captured, monkeypatch, capsys, write_transcript):
    path = write_transcript([
        user("first", "typed"),
        assistant("old reply"),
        user("second", "typed"),
        assistant("new reply"),
    ])
    fire(remember_agent_message, monkeypatch, capsys,
         {"transcript_path": path, "cwd": "/repo", "session_id": "s1", "agent_type": "cto"})
    assert captured[0]["text"] == "new reply"


def test_silent_turn_stores_nothing(captured, monkeypatch, capsys, write_transcript):
    path = write_transcript([user("fix the thing", "typed")])
    fire(remember_agent_message, monkeypatch, capsys,
         {"transcript_path": path, "cwd": "/repo", "session_id": "s1", "agent_type": "cto"})
    assert captured == []


# --- codex rollouts ------------------------------------------------------------
#
# Codex names who started the thread once, in session_meta, instead of per
# message. A codex-run or exec thread is an agent; the desktop app is him.

def meta(originator, source="vscode"):
    return {"type": "session_meta",
            "payload": {"originator": originator, "source": source, "session_id": "x"}}


def codex_msg(role, text):
    kind = "input_text" if role == "user" else "output_text"
    return {"type": "response_item",
            "payload": {"role": role, "content": [{"type": kind, "text": text}]}}


def test_codex_run_thread_has_no_architect_in_it(captured, monkeypatch, capsys,
                                                 write_transcript):
    path = write_transcript([meta("codex-run"), codex_msg("user", "## Story do the thing")])
    fire(remember_architect_message, monkeypatch, capsys,
         {"transcript_path": path, "cwd": "/repo", "session_id": "c1"})
    assert captured == []


def test_codex_exec_thread_has_no_architect_in_it(captured, monkeypatch, capsys,
                                                  write_transcript):
    path = write_transcript([meta("codex_exec", "exec"), codex_msg("user", "scripted prompt")])
    fire(remember_architect_message, monkeypatch, capsys,
         {"transcript_path": path, "cwd": "/repo", "session_id": "c2"})
    assert captured == []


def test_codex_subagent_source_has_no_architect_in_it(captured, monkeypatch, capsys,
                                                      write_transcript):
    spawn = {"subagent": {"thread_spawn": {"parent_thread_id": "p", "depth": 1}}}
    path = write_transcript([
        {"type": "session_meta", "payload": {"originator": "codex_work_desktop", "source": spawn}},
        codex_msg("user", "agent brief"),
    ])
    fire(remember_architect_message, monkeypatch, capsys,
         {"transcript_path": path, "cwd": "/repo", "session_id": "c3"})
    assert captured == []


def test_codex_desktop_thread_is_the_architect(captured, monkeypatch, capsys, write_transcript):
    path = write_transcript([meta("codex_work_desktop"), codex_msg("user", "fix the thing")])
    fire(remember_architect_message, monkeypatch, capsys,
         {"transcript_path": path, "cwd": "/repo", "session_id": "c4"})
    assert len(captured) == 1
    assert captured[0]["peer"] == "jordan"
    assert captured[0]["text"] == "fix the thing"


def test_unknown_codex_originator_denies():
    recs = [meta("some_future_entry_point"), codex_msg("user", "hello")]
    assert transcript.architect_message(recs) == ""


def test_codex_injected_blocks_are_the_harness_not_the_architect(captured, monkeypatch,
                                                                 capsys, write_transcript):
    """Codex puts `<recommended_plugins>` and `<environment_context>` on the user
    role with no metadata at all, so shape is the only evidence there is."""
    path = write_transcript([
        meta("codex_work_desktop"),
        codex_msg("user", "<recommended_plugins>Airtable, Linear</recommended_plugins>"),
        codex_msg("user", "my orbstack is spinning like hell"),
        codex_msg("user", "<environment_context><current_date>2026-08-05</current_date>"
                          "</environment_context>"),
    ])
    fire(remember_architect_message, monkeypatch, capsys,
         {"transcript_path": path, "cwd": "/repo", "session_id": "c6"})
    assert len(captured) == 1
    assert captured[0]["text"] == "my orbstack is spinning like hell"


@pytest.mark.parametrize("text,expected", [
    ("<recommended_plugins>a</recommended_plugins>", True),
    ("<environment_context><current_date>x</current_date></environment_context>", True),
    ("<task-notification><status>done</status></task-notification>", True),
    ("[Honcho Memory for jordan]", True),
    ("Stop hook feedback: something", True),
    ("Base directory for this skill: /x", True),
    ("This session is being continued from a previous conversation", True),
    ("[Image #1] this pops up and steals focus", False),
    ("use <div> instead of <span>", False),
    ("fix the thing", False),
    ("", False),
])
def test_harness_authored_reads_shape(text, expected):
    assert transcript.harness_authored(text) is expected


def test_codex_agent_replies_are_stored_under_its_own_peer(captured, monkeypatch, capsys,
                                                           write_transcript, tmp_path):
    """codex names no agent in the payload, so the run's exported definition path
    is what says whose replies these are."""
    definition = tmp_path / "ponytail.md"
    definition.write_text("---\nname: ponytail\n---\n")
    monkeypatch.setenv("CODEX_RUN_AGENT_FILE", str(definition))
    path = write_transcript([
        meta("codex-run"),
        codex_msg("user", "brief"),
        codex_msg("assistant", "here is the answer"),
    ])
    fire(remember_agent_message, monkeypatch, capsys,
         {"transcript_path": path, "cwd": "/repo", "session_id": "c5"})
    assert len(captured) == 1
    assert captured[0]["peer"] == "ponytail"
    assert captured[0]["text"] == "here is the answer"


# --- one project, one memory ---------------------------------------------------

def _git(*args, cwd):
    subprocess.run(["git", *args], cwd=cwd, check=True, capture_output=True)


@pytest.fixture
def repo_with_worktree(tmp_path):
    """A repo with a linked worktree, both real enough for rev-parse."""
    root = tmp_path / "myproject"
    root.mkdir()
    _git("init", "-q", cwd=root)
    _git("config", "user.email", "t@t", cwd=root)
    _git("config", "user.name", "t", cwd=root)
    (root / "f.txt").write_text("x")
    _git("add", "f.txt", cwd=root)
    _git("commit", "-qm", "init", cwd=root)
    nested = root / "packages" / "deep"
    nested.mkdir(parents=True)
    tree = tmp_path / "elsewhere" / "design"
    tree.parent.mkdir()
    _git("worktree", "add", "-q", "-b", "design", str(tree), cwd=root)
    return root, nested, tree


def test_a_worktree_shares_the_project_memory(repo_with_worktree):
    root, _, tree = repo_with_worktree
    assert honcho.session_name(str(tree)) == honcho.session_name(str(root))
    assert honcho.session_name(str(tree)) == "myproject"


def test_a_subdirectory_shares_the_project_memory(repo_with_worktree):
    root, nested, _ = repo_with_worktree
    assert honcho.session_name(str(nested)) == honcho.session_name(str(root))


def test_a_directory_outside_any_repo_keeps_its_own_name(tmp_path):
    loose = tmp_path / "notarepo"
    loose.mkdir()
    assert honcho.session_name(str(loose)) == "notarepo"


def test_the_session_carries_no_peer_prefix(repo_with_worktree):
    """One repository is one session that every peer writes into, so a second
    human joins the project instead of forking a session of their own."""
    root, _, _ = repo_with_worktree
    assert honcho.session_name(str(root)) == "myproject"


def test_long_text_splits_under_the_message_cap():
    parts = honcho.chunks("word " * 12000)
    assert len(parts) > 1
    assert all(len(p) <= honcho.MAX_MESSAGE for p in parts)


def test_short_text_is_one_message():
    assert honcho.chunks("hello") == ["hello"]


def test_unreachable_endpoint_fails_silently():
    cfg = dict(CFG, endpoint={"baseUrl": "http://127.0.0.1:1"})
    assert honcho.post(cfg, "s", "jordan", "hi") is False


def test_missing_config_is_not_enabled():
    assert honcho.enabled({}) is False
