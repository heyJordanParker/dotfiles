"""Behavioral tests for reload_stale_skills.py — the distance gate on a Skill.

These pin what the architect can observe: a Skill used recently is left alone, a
Skill the session has run past its own `reload-every` is ordered again, a Skill the
agent reached for itself counts exactly as one the architect typed, a compaction
puts every Skill out of reach, a Skill with no key is never named, and a Skill
named on this same turn is not ordered twice.
"""

import pytest
import reload_stale_skills as hook

TURN = hook.TURN_CHARS


def _user(text):
    return {"type": "user", "message": {"content": text}}


def _assistant(text):
    return {"type": "assistant", "message": {"content": [{"type": "text", "text": text}]}}


def _typed(name):
    """The architect typing /<name>: the harness expands the Skill itself."""
    return _user("Base directory for this skill: /Users/x/.claude/skills/%s\n\n# %s"
                 % (name, name))


def _used(name):
    """The agent using the Skill, which the transcript keeps whatever the tool answers."""
    return {"type": "assistant", "message": {"content": [
        {"type": "tool_use", "name": "Skill", "input": {"skill": name}}]}}


def _run(monkeypatch, payload, budgets):
    monkeypatch.setattr(hook, "reload_every", lambda name: budgets.get(name, 0))
    monkeypatch.setattr(hook, "read_event", lambda: payload)
    captured = {}
    monkeypatch.setattr(hook.feedback, "context",
                        lambda name, event, body: captured.setdefault("text", body))
    rc = hook.main()
    return rc, captured.get("text")


def _payload(transcript_path, prompt="carry on"):
    return {"session_id": "r1", "prompt": prompt, "transcript_path": transcript_path}


def test_a_recently_used_skill_is_left_alone(monkeypatch, write_transcript):
    path = write_transcript([_typed("5-whys"), _assistant("x" * (4 * TURN))])
    rc, text = _run(monkeypatch, _payload(path), {"5-whys": 5})
    assert rc == 0
    assert text is None


def test_a_skill_past_its_distance_is_ordered_again(monkeypatch, write_transcript):
    path = write_transcript([_typed("5-whys"), _assistant("x" * (5 * TURN))])
    _, text = _run(monkeypatch, _payload(path), {"5-whys": 5})
    # The Skill is named the way anyone names one. The harness answering
    # "instructions unchanged" is not a failure: the order buys the agent going
    # back to the Process, not the text arriving twice.
    assert text.startswith("Use /5-whys now, before anything else")


def test_a_skill_the_agent_used_itself_is_measured(monkeypatch, write_transcript):
    """The Skills that drift are the ones the agent reaches for on its own, and the
    architect never types those. The transcript records them either way."""
    path = write_transcript([_used("show-me"), _assistant("x" * (5 * TURN))])
    _, text = _run(monkeypatch, _payload(path), {"show-me": 3})
    assert text.startswith("Use /show-me now")


def test_the_agents_own_use_resets_the_distance(monkeypatch, write_transcript):
    """Obeying the order has to count, or it stands forever and every turn carries it."""
    path = write_transcript([_typed("5-whys"), _assistant("x" * (5 * TURN)),
                             _used("5-whys"), _assistant("x" * TURN)])
    _, text = _run(monkeypatch, _payload(path), {"5-whys": 5})
    assert text is None


def test_each_skill_is_measured_against_its_own_number(monkeypatch, write_transcript):
    """One conversation, two Skills used together: only the tighter one is due."""
    path = write_transcript([_typed("5-whys"), _typed("delegate"),
                             _assistant("x" * (6 * TURN))])
    _, text = _run(monkeypatch, _payload(path), {"5-whys": 5, "delegate": 20})
    assert "/5-whys" in text
    assert "/delegate" not in text


def test_a_quoted_or_commented_number_still_reads(monkeypatch, tmp_path):
    """The shared frontmatter reader unquotes and strips a trailing comment, so a
    Skill written either way is not silently read as never."""
    assert hook.frontmatter.declared('---\nreload-every: "5 turns"\n---\n', "reload-every") == "5 turns"
    assert hook.frontmatter.declared("---\nreload-every: 5 turns # tight\n---\n", "reload-every") == "5 turns"


def test_a_compaction_puts_every_skill_out_of_reach(monkeypatch, write_transcript):
    """The arrival survives in the file and not in the conversation, so measuring
    across the boundary would report a Skill the agent cannot see."""
    path = write_transcript([_typed("delegate"),
                             _user("This session is being continued from a previous"),
                             _assistant("still here")])
    _, text = _run(monkeypatch, _payload(path), {"delegate": 20})
    assert text is None


def test_a_skill_with_no_number_is_never_named(monkeypatch, write_transcript):
    path = write_transcript([_typed("show-me"), _assistant("x" * (90 * TURN))])
    _, text = _run(monkeypatch, _payload(path), {})
    assert text is None


def test_a_skill_named_this_turn_is_not_ordered_twice(monkeypatch, write_transcript):
    """classify_intent already emits the order for a typed Skill on this event, and
    the harness expands it after this hook runs."""
    path = write_transcript([_typed("5-whys"), _assistant("x" * (9 * TURN))])
    monkeypatch.setattr(hook, "typed_skills", lambda scanned: ["/5-whys"])
    _, text = _run(monkeypatch, _payload(path, "run /5-whys on this"), {"5-whys": 5})
    assert text is None


def test_a_conversation_with_no_skill_emits_nothing(monkeypatch, write_transcript):
    path = write_transcript([_assistant("x" * (90 * TURN))])
    _, text = _run(monkeypatch, _payload(path), {"5-whys": 5})
    assert text is None


def test_an_unreadable_transcript_orders_nothing(monkeypatch):
    """No records is no measurement, and unmeasured is not overdue."""
    _, text = _run(monkeypatch, _payload("/no/such/transcript.jsonl"), {"5-whys": 5})
    assert text is None


def test_an_injected_block_is_not_a_turn_of_the_architect(monkeypatch, write_transcript):
    path = write_transcript([_typed("5-whys"), _assistant("x" * (90 * TURN))])
    _, text = _run(monkeypatch,
                   _payload(path, "<task-notification>done</task-notification>"),
                   {"5-whys": 5})
    assert text is None


def test_reload_every_reads_the_frontmatter(tmp_path, monkeypatch):
    """The real reader, against the real corpus: the numbers the architect set."""
    assert hook.reload_every("5-whys") == 5
    assert hook.reload_every("delegate") == 20
    assert hook.reload_every("orchestrate") == 20
    assert hook.reload_every("execute") == 30
    assert hook.reload_every("cc") == 20
    assert hook.reload_every("show-me") == 0
    assert hook.reload_every("no-such-skill") == 0
