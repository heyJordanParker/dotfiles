"""What one side of a turn sends to Honcho (lib/honcho.store), and what a turn reads back.

Who is in a conversation and which scope it joins decides every view a later
turn reads, so the requests themselves are the contract.
"""

import pytest

from lib import honcho

CFG = {"peerName": "jordan", "workspace": "w", "environmentUrl": "http://x"}


def _sent(monkeypatch, session_status):
    calls = []
    monkeypatch.setattr(honcho, "_call", lambda cfg, method, route, body=None, **_: (
        calls.append((method, route, body)) or (session_status if route == "sessions" else 200, {})))
    monkeypatch.setattr(honcho, "project_name", lambda cwd: "dotfiles")
    return calls


@pytest.fixture
def sent(monkeypatch):
    return _sent(monkeypatch, 200)


def test_a_turn_stores_the_architect_with_principles_before_the_agent_with_lessons(sent):
    event = {"session_id": "s1", "cwd": "/repo"}
    recs = [{"type": "assistant", "message": {"model": "claude-opus-5-5", "content": []}}]

    assert honcho.store(CFG, event, recs, "cto", ["do this", "then that"], "fixed it")

    (_, _, session), (_, _, members), (_, route, messages) = sent
    assert session == {"id": "s1", "metadata": {
        "repository": "dotfiles", "agent": "cto", "model": "claude-opus-5-5", "harness": "claude"}}
    assert members == {
        "jordan": {"observe_me": True, "observe_others": False},
        "cto-claude-opus-5-5": {"observe_me": True, "observe_others": False},
    }
    assert route == "sessions/s1/messages"
    assert messages["messages"] == [
        {"peer_id": "jordan", "content": "do this",
         "configuration": {"reasoning": {"custom_instructions": honcho.PRINCIPLES}}},
        {"peer_id": "jordan", "content": "then that",
         "configuration": {"reasoning": {"custom_instructions": honcho.PRINCIPLES}}},
        {"peer_id": "cto-claude-opus-5-5", "content": "fixed it",
         "configuration": {"reasoning": {"custom_instructions": honcho.LESSONS}}},
    ]


def test_the_first_message_before_any_reply_reaches_the_architect_alone(sent):
    assert honcho.store(CFG, {"session_id": "s1", "cwd": "/repo"}, [], "cto", ["hello"], "")

    (_, _, _), (_, _, members), (_, _, messages) = sent
    assert set(members) == {"jordan"}
    assert messages["messages"] == [{"peer_id": "jordan", "content": "hello",
                                     "configuration": {"reasoning": {"custom_instructions": honcho.PRINCIPLES}}}]


def test_a_subagent_is_its_own_conversation_without_the_architect(sent):
    event = {"session_id": "parent", "agent_id": "a1", "cwd": "/repo", "model": "gpt-5.5",
             "turn_id": "t"}

    assert honcho.store(CFG, event, [], "ponytail", [], "done")

    (_, _, session), (_, _, members), (_, route, _) = sent
    assert session["id"] == "a1" and session["metadata"]["harness"] == "codex"
    assert set(members) == {"ponytail-gpt-5-5"}
    assert route == "sessions/a1/messages"


def test_a_session_honcho_creates_joins_its_project_scope(monkeypatch):
    sent = _sent(monkeypatch, 201)

    assert honcho.open_session(CFG, "s1", {"jordan": {}}, {}, "dotfiles")

    assert [(method, route) for method, route, _ in sent] == [
        ("POST", "sessions"), ("POST", "sessions"), ("PUT", "sessions/s1/peers")]
    assert sent[0][2] == {"id": "s1", "metadata": {}}
    assert sent[1][2] == {"id": "s1", "scopes": ["dotfiles"]}


def test_a_session_honcho_already_holds_never_joins_a_scope_again(sent):
    assert honcho.open_session(CFG, "s1", {"jordan": {}}, {}, "dotfiles")

    assert [(method, route) for method, route, _ in sent] == [
        ("POST", "sessions"), ("PUT", "sessions/s1/peers")]
    assert "scopes" not in sent[0][2]


def _line(line_id, content, session, level="explicit", sources=()):
    return {"id": line_id, "content": content, "session_id": session, "level": level,
            "source_ids": list(sources), "created_at": "2026-09-30T12:00:00Z"}


def test_general_keeps_only_lines_that_hold_beyond_one_project(monkeypatch):
    found = [
        _line("d1", "checks the plan before the code", None, "inductive", ["e1", "e2"]),
        _line("d2", "the billing page is slow", None, "deductive", ["e3", "e4"]),
        _line("d3", "reads the schema first", None, "deductive", ["e5", "e6"]),
        _line("g1", "answers in short sentences", "general"),
    ]
    sources = [_line("e1", "", "a1"), _line("e2", "", "b1"), _line("e3", "", "a1"),
               _line("e4", "", "a2"), _line("e5", "", "a1"), _line("e6", "", "general")]
    sessions = [{"id": "a1", "metadata": {"repository": "alpha"}},
                {"id": "a2", "metadata": {"repository": "alpha"}},
                {"id": "b1", "metadata": {"repository": "beta"}},
                {"id": "general", "metadata": {}}]
    answers = {"conclusions/query": found, "conclusions/list": {"items": sources},
               "sessions/list": {"items": sessions}}
    monkeypatch.setattr(honcho, "_call", lambda cfg, method, route, **_: (200, answers[route]))

    assert honcho.general(CFG, "jordan", query="how does he plan") == [
        "[2026-09-30] checks the plan before the code",
        "[2026-09-30] answers in short sentences",
    ]


def test_a_turn_searches_the_project_scope_on_its_words(monkeypatch):
    sent = []
    monkeypatch.setattr(honcho, "_call", lambda cfg, method, route, body=None, **_: (
        sent.append((route, body)) or (200, [_line("p1", "checks the plan first", None)])))

    assert honcho.search(CFG, "jordan", "dotfiles", "how does he plan") == ["[2026-09-30] checks the plan first"]

    assert sent == [("conclusions/query", {
        "query": "how does he plan", "top_k": 10, "distance": 0.7,
        "filters": {"observer": "scope.dotfiles", "observed": "jordan"}})]


def test_a_session_start_reads_the_patterns_the_dream_drew_in_the_project(monkeypatch):
    sent = []
    monkeypatch.setattr(honcho, "_call", lambda cfg, method, route, body=None, **_: (
        sent.append((route, body)) or (200, {"items": [_line("i1", "checks the plan first", None, "inductive")]})))

    assert honcho.patterns(CFG, "jordan", "dotfiles") == ["[2026-09-30] checks the plan first"]

    assert sent == [("conclusions/list", {"filters": {
        "observer_id": "scope.dotfiles", "observed_id": "jordan", "level": "inductive"}})]


def test_a_session_start_reads_the_newest_deliberate_saves(monkeypatch):
    sent = []
    monkeypatch.setattr(honcho, "_call", lambda cfg, method, route, body=None, **_: (
        sent.append((route, body)) or (200, {"items": [_line("g1", "answers in short sentences", "general")]})))

    assert honcho.general(CFG, "jordan") == ["[2026-09-30] answers in short sentences"]

    assert sent == [("conclusions/list", {"filters": {
        "observer_id": "jordan", "observed_id": "jordan", "session_id": "general"}})]
