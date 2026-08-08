"""Honcho context injection reads only what the architect typed."""

import json

import inject_honcho_memory
from lib import honcho

CFG = {
    "peerName": "jordan",
    "workspace": "parkerlabs",
    "endpoint": {"baseUrl": "https://example.invalid"},
}


class _Stdin:
    def __init__(self, text):
        self._text = text

    def read(self):
        return self._text


def _user(text, source):
    return {"type": "user", "promptSource": source, "message": {"content": text}}


def _fire(monkeypatch, capsys, write_transcript, records, event=None):
    transcript_path = write_transcript(records)
    payload = {"transcript_path": transcript_path, "session_id": "s1"}
    payload.update(event or {})
    monkeypatch.setattr("sys.stdin", _Stdin(json.dumps(payload)))
    assert inject_honcho_memory.main() == 0
    return capsys.readouterr().out


def _stub_context(monkeypatch, answers):
    calls = []

    def context(cfg, peer, query="", timeout=None):
        calls.append((peer, query))
        return answers.get(peer, [])

    monkeypatch.setattr(inject_honcho_memory.honcho, "config", lambda: CFG)
    monkeypatch.setattr(inject_honcho_memory.honcho, "remembered_context", context)
    monkeypatch.setattr(inject_honcho_memory.honcho, "card", lambda *a, **k: [])
    return calls


def test_injects_the_architects_collection_for_a_plain_session(monkeypatch, capsys,
                                                               write_transcript):
    calls = _stub_context(monkeypatch, {"jordan": ["Jordan prefers short lists"]})

    output = _fire(monkeypatch, capsys, write_transcript, [_user("find memory bugs", "typed")])

    assert calls == [("jordan", "find memory bugs")]
    assert json.loads(output) == {
        "hookSpecificOutput": {
            "hookEventName": "UserPromptSubmit",
            "additionalContext": "<inject_honcho_memory_agent>\n[Honcho Memory for jordan]: Jordan prefers short lists\n</inject_honcho_memory_agent>",
        },
    }


def test_a_named_agent_also_gets_its_own_collection(monkeypatch, capsys, write_transcript):
    """Two collections, disjoint by construction: nobody observes anybody, so each
    holds conclusions about its own peer's messages and nothing appears twice."""
    calls = _stub_context(monkeypatch, {"jordan": ["Jordan prefers short lists"],
                                        "cto": ["cto stops asking which file to start with"]})

    output = _fire(monkeypatch, capsys, write_transcript,
                   [_user("find memory bugs", "typed")], {"agent_type": "cto"})

    assert calls == [("jordan", "find memory bugs"), ("cto", "find memory bugs")]
    context = json.loads(output)["hookSpecificOutput"]["additionalContext"]
    assert "[Honcho Memory for jordan]: Jordan prefers short lists" in context
    assert "[Honcho Memory for cto]: cto stops asking which file to start with" in context


def test_a_subagent_reads_its_own_collection_on_its_brief(monkeypatch, capsys,
                                                          write_transcript, tmp_path):
    """The collection exists to be read by the agent it is about. Skipping
    subagents left every per-agent collection written and never read. The search
    text is the dispatch prompt, because that is what the turn is about."""
    calls = _stub_context(monkeypatch, {"jordan": ["Jordan prefers short lists"],
                                        "ponytail": ["ponytail reads the file first"]})
    monkeypatch.setenv("CLAUDE_CONFIG_DIR", str(tmp_path))
    (tmp_path / "agents").mkdir()
    (tmp_path / "agents" / "ponytail.md").write_text("---\nname: ponytail\n---\n")

    output = _fire(monkeypatch, capsys, write_transcript, [_user("unrelated", "typed")],
                   {"agent_id": "a1", "agent_type": "ponytail", "prompt": "fix the parser"})

    assert calls == [("jordan", "fix the parser"), ("ponytail", "fix the parser")]
    assert "[Honcho Memory for ponytail]" in output


def test_an_agent_declaring_no_memory_is_injected_nothing(monkeypatch, capsys,
                                                          write_transcript, tmp_path):
    """The declaration is about the agent, not about the tool it reaches memory
    through, so it holds on the way in as well as on the way out."""
    _stub_context(monkeypatch, {"jordan": ["Jordan prefers short lists"]})
    monkeypatch.setenv("CLAUDE_CONFIG_DIR", str(tmp_path))
    (tmp_path / "agents").mkdir()
    (tmp_path / "agents" / "explorer.md").write_text("---\nname: explorer\nmemory: none\n---\n")

    output = _fire(monkeypatch, capsys, write_transcript, [_user("unrelated", "typed")],
                   {"agent_id": "a1", "agent_type": "explorer", "prompt": "map the callers"})

    assert output == ""


def test_a_trivial_prompt_retrieves_nothing(monkeypatch, capsys, write_transcript):
    """An acknowledgement cannot inform a retrieval, and a slash command is about
    to be expanded by the harness. Searching on "ok" spent two network calls in
    front of the turn to match the word "ok"."""
    monkeypatch.setattr(inject_honcho_memory.honcho, "config", lambda: CFG)
    monkeypatch.setattr(inject_honcho_memory.honcho, "remembered_context",
                        lambda *a, **k: (_ for _ in ()).throw(AssertionError("retrieved")))

    for prompt in ("ok", "yes", "  thanks ", "/execute", "y", "do it"):
        assert _fire(monkeypatch, capsys, write_transcript, [_user(prompt, "typed")]) == ""

    # A real turn that merely starts with one of those words still retrieves.
    _stub_context(monkeypatch, {"jordan": ["Jordan prefers short lists"]})
    monkeypatch.setattr(inject_honcho_memory.honcho, "card", lambda *a, **k: [])
    assert _fire(monkeypatch, capsys, write_transcript,
                 [_user("ok, now fix the parser", "typed")]) != ""


def test_a_failed_retrieval_falls_back_and_an_empty_one_does_not(monkeypatch, tmp_path):
    """A turn whose retrieval fails used to start memory-blind with nothing to say
    so, and the last successful answer is stale but true. An answered "this peer
    has nothing" is a different thing: treating it as a failure replayed a stale
    set at a fresh agent forever, because the cache could never empty."""
    monkeypatch.setattr(honcho, "CACHE_DIR", str(tmp_path))
    monkeypatch.setattr(honcho, "context", lambda *a, **k: ["a live conclusion"])
    assert honcho.remembered_context(CFG, "jordan") == ["a live conclusion"]

    monkeypatch.setattr(honcho, "context", lambda *a, **k: None)
    assert honcho.remembered_context(CFG, "jordan") == ["a live conclusion"]

    monkeypatch.setattr(honcho, "context", lambda *a, **k: [])
    assert honcho.remembered_context(CFG, "jordan") == []
    monkeypatch.setattr(honcho, "context", lambda *a, **k: None)
    assert honcho.remembered_context(CFG, "jordan") == []

    assert honcho.remembered_context(CFG, "never-retrieved") == []


def test_the_profile_card_leads_the_architects_block(monkeypatch, capsys, write_transcript):
    """The card is the part that does not depend on the turn's words matching a
    conclusion, so a turn that matches nothing still opens with who he is."""
    _stub_context(monkeypatch, {"jordan": ["Jordan prefers short lists"]})
    monkeypatch.setattr(inject_honcho_memory.honcho, "card",
                        lambda cfg, peer, **k: ["IDENTITY: Name: jordan"] if peer == "jordan" else [])

    output = _fire(monkeypatch, capsys, write_transcript, [_user("find memory bugs", "typed")])

    context = json.loads(output)["hookSpecificOutput"]["additionalContext"]
    assert "[Honcho Memory for jordan]: IDENTITY: Name: jordan; Jordan prefers short lists" in context


def test_compaction_is_covered_from_the_far_side_and_precompact_stays_silent(monkeypatch,
                                                                             capsys,
                                                                             write_transcript):
    """Compaction replaces the conversation with a summary, so it is where memory
    is thinnest — and Claude fires SessionStart with `source: compact` right after,
    which is the channel that reaches that moment.

    PreCompact itself has no output variant in Claude's schema: the block was
    rejected wholesale, so the architect got a validation banner and the turn got
    no memory. A session still holding the old wiring must now stay quiet."""
    calls = _stub_context(monkeypatch, {"jordan": ["Jordan prefers short lists"]})
    monkeypatch.setattr(inject_honcho_memory.honcho, "card", lambda *a, **k: [])

    monkeypatch.setattr("sys.stdin", _Stdin(json.dumps(
        {"hook_event_name": "SessionStart", "source": "compact", "session_id": "s1"})))
    assert inject_honcho_memory.main() == 0
    assert calls == [("jordan", "")]
    emitted = json.loads(capsys.readouterr().out)["hookSpecificOutput"]
    assert emitted["hookEventName"] == "SessionStart"
    assert "[Honcho Memory for jordan]" in emitted["additionalContext"]

    assert "PreCompact" not in inject_honcho_memory.BINDING["events"]
    monkeypatch.setattr("sys.stdin", _Stdin(json.dumps(
        {"hook_event_name": "PreCompact", "session_id": "s1",
         "transcript_path": write_transcript([_user("find memory bugs", "typed")])})))
    assert inject_honcho_memory.main() == 0
    captured = capsys.readouterr()
    assert captured.out == ""
    assert "carries no additionalContext" in captured.err


def test_a_session_opens_with_both_collections_unsearched(monkeypatch, capsys):
    """SessionStart has no prompt, so the retrieval runs unsearched and each
    collection answers with what it holds most strongly. Without this a session
    starts blank and stays blank until a prompt happens to match something."""
    calls = _stub_context(monkeypatch, {"jordan": ["Jordan prefers short lists"],
                                        "cto": ["cto reads the file first"]})

    monkeypatch.setattr("sys.stdin", _Stdin(json.dumps(
        {"hook_event_name": "SessionStart", "session_id": "s1", "agent_type": "cto"})))
    assert inject_honcho_memory.main() == 0

    assert calls == [("jordan", ""), ("cto", "")]
    emitted = json.loads(capsys.readouterr().out)["hookSpecificOutput"]
    assert emitted["hookEventName"] == "SessionStart"
    assert "[Honcho Memory for jordan]" in emitted["additionalContext"]
    assert "[Honcho Memory for cto]" in emitted["additionalContext"]


def test_task_notification_does_not_retrieve_or_inject(monkeypatch, capsys, write_transcript):
    monkeypatch.setattr(inject_honcho_memory.honcho, "config", lambda: CFG)
    monkeypatch.setattr(inject_honcho_memory.honcho, "context",
                        lambda *args, **kwargs: (_ for _ in ()).throw(AssertionError("retrieved")))

    output = _fire(monkeypatch, capsys, write_transcript, [
        _user("fix the hook", "typed"),
        _user("<task-notification><status>completed</status></task-notification>", "system"),
    ])

    assert output == ""


def test_context_uses_the_context_endpoint_and_strips_representation_markers(monkeypatch):
    seen = {}
    monkeypatch.setattr(honcho, "_request",
                        lambda cfg, method, route, body=None, query=None, timeout=None:
                        seen.update(method=method, route=route, body=body, query=query) or {
                            "representation": "# Heading\n[2026-08-06] first\n- second\n\n"})

    assert honcho.context(CFG, "jordan", query="my actual words") == ["first", "second"]
    assert seen == {
        "method": "GET",
        "route": "peers/jordan/context",
        "body": None,
        "query": {
            "search_query": "my actual words",
            "search_max_distance": 0.6,
            "include_most_frequent": "true",
            "search_top_k": 10,
            "max_conclusions": 15,
        },
    }


def test_context_separates_a_peer_with_nothing_from_a_server_that_did_not_answer(monkeypatch):
    """Same [] for both made every failure look like an empty peer, which is what
    let a stale cache outlive the truth."""
    monkeypatch.setattr(honcho, "_request", lambda *a, **k: None)
    assert honcho.context(CFG, "jordan") is None

    monkeypatch.setattr(honcho, "_request", lambda *a, **k: {"representation": ""})
    assert honcho.context(CFG, "jordan") == []


def _dispatch(monkeypatch, capsys, agent, prompt="fix the parser"):
    monkeypatch.setattr("sys.stdin", _Stdin(json.dumps({
        "hook_event_name": "PreToolUse", "tool_name": "Agent", "session_id": "s1",
        "tool_input": {"subagent_type": agent, "prompt": prompt,
                       "description": "fix it"}})))
    assert inject_honcho_memory.main() == 0
    return capsys.readouterr().out


def test_a_claude_subagent_gets_its_memory_on_the_dispatch_itself(monkeypatch, capsys):
    """No prompt event fires inside a Claude subagent — a dispatched agent asked
    whether it saw a memory block answered NONE. The dispatch is a tool call, so
    the memory goes into the brief, which is the one text it is sure to read."""
    calls = _stub_context(monkeypatch, {"jordan": ["Jordan prefers short lists"],
                                        "ponytail": ["ponytail reads the file first"]})

    emitted = json.loads(_dispatch(monkeypatch, capsys, "ponytail"))["hookSpecificOutput"]

    assert calls == [("jordan", "fix the parser"), ("ponytail", "fix the parser")]
    assert emitted["hookEventName"] == "PreToolUse"
    assert "permissionDecision" not in emitted
    brief = emitted["updatedInput"]["prompt"]
    assert brief.endswith("\n\nfix the parser")
    assert "[Honcho Memory for ponytail]: ponytail reads the file first" in brief
    assert "<inject_honcho_memory_agent>" in brief
    # The rest of the call is handed back untouched.
    assert emitted["updatedInput"]["subagent_type"] == "ponytail"
    assert emitted["updatedInput"]["description"] == "fix it"


def test_the_dispatched_agent_owns_the_declaration_not_the_dispatcher(monkeypatch, capsys,
                                                                      tmp_path):
    """The peer is the agent about to run. Reading the dispatcher's declaration
    would hand a blank agent the memory of whoever dispatched it."""
    _stub_context(monkeypatch, {"jordan": ["Jordan prefers short lists"]})
    monkeypatch.setenv("CLAUDE_CONFIG_DIR", str(tmp_path))
    (tmp_path / "agents").mkdir()
    (tmp_path / "agents" / "researcher.md").write_text(
        "---\nname: researcher\nmemory: none\n---\n")

    assert _dispatch(monkeypatch, capsys, "researcher") == ""


def test_ask_separates_a_server_that_failed_from_an_answer_it_had_none_for(monkeypatch, capsys):
    """This endpoint reasons with a model and fails on its own: it answered 500 for
    every peer while card, context, search and the writes were all healthy. One
    message for both made that read as "memory knows nothing about him"."""
    monkeypatch.setattr(honcho, "config", lambda: CFG)

    monkeypatch.setattr(honcho, "_request", lambda *a, **k: None)
    assert honcho.ask(CFG, "jordan", "what does he prefer") is None
    assert honcho.main(["ask", "jordan", "what", "does", "he", "prefer"]) == 1
    assert "did not answer" in capsys.readouterr().err

    monkeypatch.setattr(honcho, "_request", lambda *a, **k: {"content": ""})
    assert honcho.ask(CFG, "jordan", "what does he prefer") == ""
    assert honcho.main(["ask", "jordan", "what", "does", "he", "prefer"]) == 1
    assert "knows nothing about jordan" in capsys.readouterr().err

    monkeypatch.setattr(honcho, "_request", lambda *a, **k: {"content": "He drops em dashes."})
    assert honcho.main(["ask", "jordan", "what", "does", "he", "prefer"]) == 0
    assert "He drops em dashes." in capsys.readouterr().out


def test_conclusion_management_uses_the_v3_api(monkeypatch):
    calls = []
    monkeypatch.setattr(honcho, "_request", lambda *args, **kwargs: calls.append((args, kwargs)) or [{"id": "one"}])

    assert honcho.conclusions(CFG, {"observer_id": "jordan"}) == [{"id": "one"}]
    assert honcho.create_conclusion(CFG, "jordan", "jordan", "a fact")
    assert honcho.delete_conclusion(CFG, "one")
    assert calls == [
        ((CFG, "POST", "conclusions/list"), {"body": {"filters": {"observer_id": "jordan"}}, "query": {"page": 1, "size": 50}}),
        ((CFG, "POST", "conclusions"), {"body": {"conclusions": [{"observer_id": "jordan", "observed_id": "jordan", "content": "a fact", "session_id": None}]}}),
        ((CFG, "DELETE", "conclusions/one"), {}),
    ]


def test_remembering_writes_into_the_agents_own_collection(monkeypatch, capsys):
    """Observer and observed are both the agent: the subject is its own behaviour
    and no second party's view differs. No session — the live API answers 404 for
    one the messages have not created, and the collection is what a read asks for."""
    seen = {}
    monkeypatch.setattr(honcho, "config", lambda: CFG)
    monkeypatch.setattr(honcho, "_request", lambda cfg, method, route, body=None, query=None:
                        seen.update(route=route, body=body) or {})
    monkeypatch.setenv("CLAUDE_CODE_AGENT", "ponytail")
    monkeypatch.delenv("CODEX_RUN_AGENT_FILE", raising=False)

    assert honcho.main(["remember", "give", "the", "whole", "hierarchy"]) == 0
    written = seen["body"]["conclusions"][0]
    assert seen["route"] == "conclusions"
    assert written["observer_id"] == written["observed_id"] == "ponytail"
    assert written["content"] == "give the whole hierarchy"
    assert written["session_id"] is None
    assert "remembered for ponytail" in capsys.readouterr().out


def test_the_command_line_names_the_agent_itself(monkeypatch, capsys, tmp_path):
    """One argument, the text. The name is the thing an agent would get wrong, so
    it is never asked for: a codex run's definition path answers it, and a Claude
    session answers with what it was started as."""
    written = []
    monkeypatch.setattr(honcho, "config", lambda: CFG)
    monkeypatch.setattr(honcho, "create_conclusion",
                        lambda cfg, observer, observed, text: written.append((observed, text)) or True)

    definition = tmp_path / "ponytail.md"
    definition.write_text("---\nname: ponytail\n---\n")
    monkeypatch.setenv("CODEX_RUN_AGENT_FILE", str(definition))
    monkeypatch.setenv("CLAUDE_CODE_AGENT", "cto")
    assert honcho.main(["remember", "give", "the", "whole", "hierarchy"]) == 0

    monkeypatch.delenv("CODEX_RUN_AGENT_FILE")
    assert honcho.main(["remember", "read the file first"]) == 0

    assert written == [("ponytail", "give the whole hierarchy"), ("cto", "read the file first")]
    assert "remembered for ponytail" in capsys.readouterr().out


def _name_hook(monkeypatch, capsys, command, agent="ponytail"):
    """Run the naming hook over one command; returns (exit code, replacement or "")."""
    import name_memory_caller

    monkeypatch.setattr("sys.stdin", _Stdin(json.dumps({
        "hook_event_name": "PreToolUse", "tool_name": "Bash", "session_id": "s1",
        "agent_id": "a1", "agent_type": agent, "tool_input": {"command": command}})))
    code = name_memory_caller.main()
    out = capsys.readouterr().out
    replacement = json.loads(out)["hookSpecificOutput"]["updatedInput"]["command"] if out else ""
    return code, replacement


def test_the_name_is_written_into_the_command_before_it_runs(monkeypatch, capsys):
    """The name goes into the call itself, so by the time the command runs there
    is nothing left to infer. The earlier design left the name in a note keyed by
    the text being remembered, which keys two parsers off one string — a `$VAR`
    the shell expands or a `&&` tail makes the keys differ, silently."""
    code, replaced = _name_hook(monkeypatch, capsys,
                                "honcho remember 'read the file first'")
    assert code == 0
    assert replaced == "honcho remember --as ponytail 'read the file first'"

    _, chained = _name_hook(monkeypatch, capsys,
                            "cd /tmp && honcho remember \"$FINDING\" && echo done")
    assert chained == "cd /tmp && honcho remember --as ponytail \"$FINDING\" && echo done"

    _, absolute = _name_hook(monkeypatch, capsys, "~/bin/honcho remember x")
    assert absolute == "~/bin/honcho remember --as ponytail x"


def test_nothing_else_is_rewritten(monkeypatch, capsys):
    """A read, a different command, the word inside a string, and a call that
    already names an agent are all left exactly as written."""
    for command in ("honcho context jordan", "git status",
                    "echo \"run honcho remember x\"",
                    "honcho remember --as explorer 'x'"):
        code, replaced = _name_hook(monkeypatch, capsys, command)
        assert (code, replaced) == (0, ""), command


def test_the_hook_outlives_every_retrieval_it_makes(monkeypatch):
    """Three retrievals run in front of a turn — the card and both collections —
    each capped at RETRIEVAL_TIMEOUT. A hook ceiling below their sum truncates a
    turn's memory at whichever one it reached, exactly when the server is slow
    and the cached fallback matters most."""
    assert inject_honcho_memory.RETRIEVAL_TIMEOUT < honcho.TIMEOUT
    assert inject_honcho_memory.BINDING["timeout"] > 3 * inject_honcho_memory.RETRIEVAL_TIMEOUT


def test_remember_needs_a_name_and_a_text_after_the_flag(monkeypatch, capsys):
    """`--as` with nothing after it would otherwise store the flag and its value
    as the thing being remembered."""
    monkeypatch.setattr(honcho, "config", lambda: CFG)
    monkeypatch.setattr(honcho, "create_conclusion",
                        lambda *a: (_ for _ in ()).throw(AssertionError("wrote anyway")))

    assert honcho.main(["remember", "--as"]) == 1
    assert honcho.main(["remember", "--as", "ponytail"]) == 1
    assert "honcho remember" in capsys.readouterr().err


def test_a_call_that_cannot_be_named_is_refused(monkeypatch, capsys):
    """An environment prefix puts the call out of command position, where the
    rewrite cannot reach it. Refusing names the `--as` form; the alternative is a
    write that silently lands in the dispatching agent's collection."""
    import name_memory_caller

    monkeypatch.setattr("sys.stdin", _Stdin(json.dumps({
        "hook_event_name": "PreToolUse", "tool_name": "Bash", "session_id": "s1",
        "agent_id": "a1", "agent_type": "ponytail",
        "tool_input": {"command": "FOO=1 honcho remember 'x'"}})))
    assert name_memory_caller.main() == 2
    assert "honcho remember --as ponytail" in capsys.readouterr().err


def test_an_explicit_agent_overrides_the_running_one(monkeypatch, capsys):
    """The optional half: `--as` writes into another agent's collection, which is
    the architect's call rather than something an agent has to get right."""
    written = []
    monkeypatch.setattr(honcho, "config", lambda: CFG)
    monkeypatch.setattr(honcho, "create_conclusion",
                        lambda cfg, observer, observed, text: written.append((observed, text)) or True)
    monkeypatch.setenv("CLAUDE_CODE_AGENT", "cto")
    monkeypatch.delenv("CODEX_RUN_AGENT_FILE", raising=False)

    assert honcho.main(["remember", "--as", "ponytail", "read", "the file first"]) == 0
    assert written == [("ponytail", "read the file first")]
    assert "remembered for ponytail" in capsys.readouterr().out


def test_the_command_line_refuses_to_guess_the_agent(monkeypatch, capsys):
    monkeypatch.setattr(honcho, "config", lambda: CFG)
    monkeypatch.setattr(honcho, "create_conclusion",
                        lambda *a: (_ for _ in ()).throw(AssertionError("wrote anyway")))
    monkeypatch.delenv("CODEX_RUN_AGENT_FILE", raising=False)
    monkeypatch.delenv("CLAUDE_CODE_AGENT", raising=False)

    assert honcho.main(["remember", "something"]) == 1
    assert "names which agent is running" in capsys.readouterr().err


def test_the_command_line_refuses_an_unknown_command(monkeypatch, capsys):
    monkeypatch.setattr(honcho, "config", lambda: CFG)
    assert honcho.main([]) == 1
    assert "honcho remember" in capsys.readouterr().err


def test_the_command_line_stops_when_memory_is_disabled(monkeypatch, capsys):
    monkeypatch.setattr(honcho, "config", lambda: dict(CFG, enabled=False))
    assert honcho.main(["context", "jordan"]) == 1
    assert "disabled" in capsys.readouterr().err


def test_empty_delete_response_is_a_success(monkeypatch):
    class Response:
        status = 204

        def read(self):
            return b""

        def __enter__(self):
            return self

        def __exit__(self, *args):
            return False

    monkeypatch.setattr(honcho.urllib.request, "urlopen", lambda *args, **kwargs: Response())

    assert honcho.delete_conclusion(CFG, "one")
