"""Behavioral tests for reload_stale_skills.py — the distance gate on a Skill.

These pin what the architect can observe: a Skill used recently is left alone, a
Skill the session has run past its own `reload-every` is ordered again, a Skill the
agent reached for itself counts exactly as one the architect typed, the hook's own
order counts so an overdue Skill is ordered once per distance, a machine-authored
prompt and a tool batch are measured like a typed prompt, a compaction names every
Skill in use, a Skill with no key is never named by distance, and a Skill named on
this same turn is not ordered twice.
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


def _ordered(text, hook_name="PostToolBatch"):
    """This hook's own order, as the harness stores a hook's context."""
    return {"type": "attachment", "attachment": {
        "type": "hook_additional_context", "hookName": hook_name, "hookEvent": hook_name,
        "content": ["<reload_stale_skills_agent>\n%s\n</reload_stale_skills_agent>" % text]}}


def _run(monkeypatch, payload, budgets, mode="build", state="execute", user_only=()):
    monkeypatch.setattr(hook, "reload_every", lambda name: budgets.get(name, 0))
    monkeypatch.setattr(hook, "_skill_disables_model_invocation", lambda name: name in user_only)
    monkeypatch.setattr(hook, "resolve", lambda event, session_id=None: mode)
    monkeypatch.setattr(hook, "state", lambda event: state)
    monkeypatch.setattr(hook, "read_event", lambda: payload)
    captured = {}
    monkeypatch.setattr(hook.feedback, "context",
                        lambda name, event, body: captured.update(event=event, text=body))
    rc = hook.main()
    return rc, captured.get("text"), captured.get("event")


def _payload(transcript_path, prompt="carry on", hook_event="UserPromptSubmit", **extra):
    payload = {"session_id": "r1", "transcript_path": transcript_path,
               "hook_event_name": hook_event}
    if prompt is not None:
        payload["prompt"] = prompt
    payload.update(extra)
    return payload


def _batch(transcript_path):
    return _payload(transcript_path, prompt=None, hook_event="PostToolBatch",
                    tool_calls=[{"tool_name": "Bash"}])


def _compact(transcript_path):
    return _payload(transcript_path, prompt=None, hook_event="SessionStart", source="compact")


def test_a_recently_used_skill_is_left_alone(monkeypatch, write_transcript):
    path = write_transcript([_typed("5-whys"), _assistant("x" * (4 * TURN))])
    rc, text, _ = _run(monkeypatch, _payload(path), {"5-whys": 5})
    assert rc == 0
    assert text is None


def test_a_skill_past_its_distance_is_ordered_again(monkeypatch, write_transcript):
    path = write_transcript([_typed("5-whys"), _assistant("x" * (5 * TURN))])
    _, text, event = _run(monkeypatch, _payload(path), {"5-whys": 5})
    # The Skill is named the way anyone names one. The harness answering
    # "instructions unchanged" is not a failure: the order buys the agent going
    # back to the Process, not the text arriving twice.
    assert text.startswith("Use /5-whys now, before anything else")
    assert event == "UserPromptSubmit"


def test_a_tool_batch_is_measured_like_a_prompt(monkeypatch, write_transcript):
    """The model is called after a batch as it is after a prompt, and a session the
    architect walked away from is nothing but batches."""
    path = write_transcript([_used("execute"), _assistant("x" * (30 * TURN))])
    _, text, event = _run(monkeypatch, _batch(path), {"execute": 30})
    assert text.startswith("Use /execute now")
    assert event == "PostToolBatch"


@pytest.mark.parametrize("prompt", [
    "<task-notification>ran /5-whys on the failure</task-notification>",
    "Another Claude session sent a message:\n<cross-session-message>use /5-whys</cross-session-message>",
])
def test_a_machine_authored_prompt_is_measured(monkeypatch, write_transcript, prompt):
    """A task notification or a SendMessage delivery opens most turns of a long
    session; skipping them left the hook blind for the whole run. A Skill named
    inside one is not a typed Skill: classify_intent never classifies the prompt,
    so nothing else orders it this turn."""
    path = write_transcript([_typed("5-whys"), _assistant("x" * (5 * TURN))])
    _, text, _ = _run(monkeypatch, _payload(path, prompt), {"5-whys": 5})
    assert text.startswith("Use /5-whys now")


def test_a_skill_the_agent_used_itself_is_measured(monkeypatch, write_transcript):
    """The Skills that drift are the ones the agent reaches for on its own, and the
    architect never types those. The transcript records them either way."""
    path = write_transcript([_used("show-me"), _assistant("x" * (5 * TURN))])
    _, text, _ = _run(monkeypatch, _payload(path), {"show-me": 3})
    assert text.startswith("Use /show-me now")


def test_the_agents_own_use_resets_the_distance(monkeypatch, write_transcript):
    """Obeying the order has to count, or it stands forever and every turn carries it."""
    path = write_transcript([_typed("5-whys"), _assistant("x" * (5 * TURN)),
                             _used("5-whys"), _assistant("x" * TURN)])
    _, text, _ = _run(monkeypatch, _payload(path), {"5-whys": 5})
    assert text is None


def test_the_hooks_own_order_resets_the_distance(monkeypatch, write_transcript):
    """Before every model request, an overdue Skill would otherwise be ordered on
    every one of them until the agent obeyed."""
    path = write_transcript([_typed("5-whys"), _assistant("x" * (5 * TURN)),
                             _ordered("Use /5-whys now, before anything else, so its contract governs this turn."),
                             _assistant("x" * TURN)])
    _, text, _ = _run(monkeypatch, _batch(path), {"5-whys": 5})
    assert text is None


def test_an_ignored_order_is_repeated_after_another_distance(monkeypatch, write_transcript):
    path = write_transcript([_typed("5-whys"), _assistant("x" * (5 * TURN)),
                             _ordered("Use /5-whys now, before anything else, so its contract governs this turn."),
                             _assistant("x" * (5 * TURN))])
    _, text, _ = _run(monkeypatch, _batch(path), {"5-whys": 5})
    assert text.startswith("Use /5-whys now")


def test_each_skill_is_measured_against_its_own_number(monkeypatch, write_transcript):
    """One conversation, two Skills used together: only the tighter one is due."""
    path = write_transcript([_typed("5-whys"), _typed("delegate"),
                             _assistant("x" * (6 * TURN))])
    _, text, _ = _run(monkeypatch, _payload(path), {"5-whys": 5, "delegate": 20})
    assert "/5-whys" in text
    assert "/delegate" not in text


def test_a_quoted_or_commented_number_still_reads(monkeypatch, tmp_path):
    """The shared frontmatter reader unquotes and strips a trailing comment, so a
    Skill written either way is not silently read as never."""
    assert hook.frontmatter.declared('---\nreload-every: "5 turns"\n---\n', "reload-every") == "5 turns"
    assert hook.frontmatter.declared("---\nreload-every: 5 turns # tight\n---\n", "reload-every") == "5 turns"


def test_a_compaction_puts_every_skill_out_of_distance_reach(monkeypatch, write_transcript):
    """The arrival survives in the file and not in the conversation, so measuring
    across the boundary would report a Skill the agent cannot see."""
    path = write_transcript([_typed("delegate"),
                             _user("This session is being continued from a previous"),
                             _assistant("still here")])
    _, text, _ = _run(monkeypatch, _payload(path), {"delegate": 20})
    assert text is None


def test_a_compaction_names_every_skill_in_use(monkeypatch, write_transcript):
    """At SessionStart:compact the boundary is not written yet, so the whole window
    that just closed is the in-use set: the mode and state Skills off session state,
    then every Skill typed or used, whatever its reload-every."""
    path = write_transcript([_typed("naming"), _used("delegate"), _assistant("x" * TURN)])
    _, text, event = _run(monkeypatch, _compact(path), {}, mode="orchestrate", state="execute")
    assert event == "SessionStart"
    assert text.startswith("### Use the Skills the compaction dropped")
    assert "Use /execute now" in text
    assert "Use /orchestrate now." in text
    assert "Use /naming, /delegate now" in text


def test_a_compaction_names_the_mode_and_state_skills_once(monkeypatch, write_transcript):
    """The agent used /execute and /orchestrate itself, and session state names
    them too: one block, each named once."""
    path = write_transcript([_used("execute"), _used("orchestrate"), _used("delegate"),
                             _assistant("x" * TURN)])
    _, text, _ = _run(monkeypatch, _compact(path), {}, mode="orchestrate", state="execute")
    assert text.count("/execute") == 1
    assert text.count("/orchestrate") == 1
    assert text.count("/delegate") == 1


def test_a_compaction_order_counts_a_use_but_not_an_earlier_order(monkeypatch, write_transcript):
    """A Skill the agent stopped obeying is not carried from compaction to
    compaction on the strength of this hook's own earlier order."""
    path = write_transcript([_user("This session is being continued from a previous"),
                             _ordered("Use /naming now.", "SessionStart"),
                             _used("delegate"), _assistant("x" * TURN)])
    _, text, _ = _run(monkeypatch, _compact(path), {}, mode="build", state="execute")
    assert "/delegate" in text
    assert "/naming" not in text


def test_a_compaction_in_the_previous_window_is_not_in_use(monkeypatch, write_transcript):
    path = write_transcript([_typed("naming"),
                             _user("This session is being continued from a previous"),
                             _used("delegate"), _assistant("x" * TURN)])
    _, text, _ = _run(monkeypatch, _compact(path), {}, mode="build", state="execute")
    assert "/delegate" in text
    assert "/naming" not in text


def test_a_compaction_never_orders_a_skill_the_agent_cannot_use(monkeypatch, write_transcript):
    """A `disable-model-invocation` Skill the architect typed is in use, and an
    order to use it dead-ends on the Skill tool."""
    path = write_transcript([_typed("handoff"), _used("delegate"), _assistant("x" * TURN)])
    _, text, _ = _run(monkeypatch, _compact(path), {}, user_only=("handoff",))
    assert "/delegate" in text
    assert "/handoff" not in text


def test_a_compaction_in_interview_mode_names_the_interview_skill(monkeypatch, write_transcript):
    """An interview session produces questions, not state work, so the state Skill
    is not named there — the same answer classify_intent gives on a typed turn."""
    path = write_transcript([_used("naming"), _assistant("x" * TURN)])
    _, text, _ = _run(monkeypatch, _compact(path), {}, mode="interview", state="propose")
    assert "Use /interview now" in text
    assert "/propose" not in text
    assert "Use /naming now" in text


def test_a_compaction_with_no_transcript_still_names_the_mode_and_state(monkeypatch):
    """The gates keep enforcing both axes whether or not the file was flushed."""
    _, text, _ = _run(monkeypatch, _compact("/no/such/transcript.jsonl"), {},
                      mode="orchestrate", state="execute")
    assert "Use /execute now" in text
    assert "Use /orchestrate now." in text


def test_an_order_counts_only_what_it_ordered(monkeypatch, write_transcript):
    """The executing-state order says `escalate with /pcc`; that mention is not an
    arrival, or /pcc's distance would reset at every compaction."""
    path = write_transcript([_typed("pcc"), _assistant("x" * (5 * TURN)),
                             _ordered("Use /execute now and work under its contract: implement "
                                      "the approved work, and the moment it needs an architectural "
                                      "change, stop and escalate with /pcc.", "SessionStart"),
                             _assistant("x" * TURN)])
    _, text, _ = _run(monkeypatch, _batch(path), {"pcc": 5})
    assert text.startswith("Use /pcc now")


def test_a_session_start_that_is_not_a_compaction_orders_nothing(monkeypatch, write_transcript):
    path = write_transcript([_typed("naming"), _assistant("x" * TURN)])
    _, text, _ = _run(monkeypatch, _payload(path, prompt=None, hook_event="SessionStart",
                                            source="resume"), {})
    assert text is None


def test_a_skill_with_no_number_is_never_named_by_distance(monkeypatch, write_transcript):
    path = write_transcript([_typed("show-me"), _assistant("x" * (90 * TURN))])
    _, text, _ = _run(monkeypatch, _payload(path), {})
    assert text is None


def test_a_skill_named_this_turn_is_not_ordered_twice(monkeypatch, write_transcript):
    """classify_intent already emits the order for a typed Skill on this event, and
    the harness expands it after this hook runs."""
    path = write_transcript([_typed("5-whys"), _assistant("x" * (9 * TURN))])
    monkeypatch.setattr(hook, "typed_skills", lambda scanned: ["/5-whys"])
    _, text, _ = _run(monkeypatch, _payload(path, "run /5-whys on this"), {"5-whys": 5})
    assert text is None


def test_a_conversation_with_no_skill_emits_nothing(monkeypatch, write_transcript):
    path = write_transcript([_assistant("x" * (90 * TURN))])
    _, text, _ = _run(monkeypatch, _payload(path), {"5-whys": 5})
    assert text is None


def test_an_unreadable_transcript_orders_nothing(monkeypatch):
    """No records is no measurement, and unmeasured is not overdue."""
    _, text, _ = _run(monkeypatch, _payload("/no/such/transcript.jsonl"), {"5-whys": 5})
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
