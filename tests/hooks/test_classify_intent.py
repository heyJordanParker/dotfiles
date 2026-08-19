"""Behavioral tests for classify_intent.py — intent → contract injection + notes.

The LLM call is stubbed, so these pin the deterministic shell around it: the
per-intent contract emitted as additionalContext, the question-with-action
variant, the sequential directive, session-notes maintenance (capped at 10), the
proposing-turn re-injection of standing notes, and composition with a typed
mode-command. The structural skips and the LLM-down typed-command fallback are
covered in test_local_llm_fallbacks.
"""

import classify_intent
import pytest
from lib.session_state import load_state


@pytest.fixture
def state_root(tmp_path, monkeypatch):
    root = tmp_path / "claude"
    monkeypatch.setenv("CLAUDE_DATA_ROOT", str(root))
    monkeypatch.delenv("CLAUDE_SESSION_HOOK", raising=False)
    # The hook resolves the governing session through owner_session, which prefers the
    # environment — the harness running pytest would otherwise lend these cases its own
    # session, and the payload's `ci1` would never be the session read back.
    for var in ("AGENT_SESSION_ID", "CODEX_THREAD_ID", "CLAUDE_CODE_SESSION_ID"):
        monkeypatch.delenv(var, raising=False)
    return root


def _run(monkeypatch, payload, model_result):
    monkeypatch.setattr(classify_intent, "run_model", lambda *a, **k: model_result)
    monkeypatch.setattr(classify_intent, "read_event", lambda: payload)
    captured = {}
    monkeypatch.setattr(classify_intent, "emit_context",
                        lambda text: captured.setdefault("text", text))
    rc = classify_intent.main()
    return rc, captured.get("text")


def test_question_emits_answer_contract(monkeypatch, state_root):
    rc, text = _run(monkeypatch,
                    {"session_id": "ci1", "prompt": "why does X work this way?"},
                    {"intent": "question"})
    assert rc == 0
    assert "This is a question. Answer it" in text
    assert "Answer with specific facts, not gestures at them" in text
    assert "execute the action items" not in text






def test_plain_action_emits_only_standing_reminders(monkeypatch, state_root):
    rc, text = _run(monkeypatch,
                    {"session_id": "ci1", "prompt": "add a guard to the parser"},
                    {"intent": "action"})
    assert rc == 0
    assert "The architect's call governs" in text
    assert "Ground every claim" in text
    assert "This is a question" not in text












@pytest.mark.parametrize("command,mode", [("/orchestrate", "orchestrate"),
                                          ("/build", "build"),
                                          ("/interview", "interview")])
def test_a_typed_mode_command_writes_the_mode_axis(monkeypatch, state_root, command, mode):
    """Mode is the axis the architect types. Interview used to be a state written when
    the interview SKILL was invoked; it is a mode set by typing the command now."""
    _run(monkeypatch, {"session_id": "ci1", "prompt": "%s the parser" % command},
         {"intent": "action"})
    state = load_state("ci1")
    assert state["mode"] == mode
    assert state["mode_typed"] is True






def test_typed_interview_skips_the_model_call(monkeypatch, state_root):
    """Interview turns the LLM hooks off for speed — only the deterministic directive
    rides, and the standing reminders that ride every other turn do not."""
    _, text = _run(monkeypatch, {"session_id": "ci1", "prompt": "/interview the parser"},
                   {"intent": "action"})
    assert "Use /interview now" in text
    assert "The architect's call governs" not in text
































def _stub_skills(monkeypatch, names):
    monkeypatch.setattr(classify_intent, "_available_skills", lambda: set(names))


def test_typed_skill_emits_reload_directive(monkeypatch, state_root):
    _stub_skills(monkeypatch, ["naming", "pcc"])
    _, text = _run(monkeypatch,
                   {"session_id": "ci1", "prompt": "give me names /naming"},
                   {"intent": "action"})
    # The Skill named the way anyone names one, and never a report of who asked.
    # reload_stale_skills emits this same sentence.
    assert "Use /naming now, before anything else" in text
    assert "The architect typed" not in text
















# The two real messages that exposed the positional recognition, taken verbatim
# from the session transcript (records 1670 and 1969 of
# 933f5da9-a2c3-474c-afb2-3b5ba98c64ec.jsonl).
INCIDENT_BATCH_APPROVAL = 'Ready to execute — already approved by you; one go confirms the batch\n\n1. Delete the never-fired model-call logging in session_state.py and model_call.py\n2. Delete packages/codex/rules/default.rules\n3. Delete debug/SKILL.md\'s superpowers plugin citations, its five unreached reference files, and its unattributed timing numbers\n4. Delete Architecture.md\'s clause renaming files that never existed\n5. Fix cto.md\'s stale "hardened for Opus 4.8" description line\n6. Make the diff-block form /pcc\'s single Template; /propose and proposals.md point at it; their divergent Templates and the two false "the cto Prompt covers this" references die\n7. Fold .claude/rules/prompts.md into global prompt-writing.md, keeping the full instruction set (load /cc, read Domain.md and the Architecture, place blocks per allowance, trace every term)\n8. Delete validate_subagent_prompt.py (measured: 20.8% of 903 dispatches blocked, ~27% precision, 96% of the worst prompts passed, 60s model call per dispatch)\n\n\nokay /execute those\n\nOption 1: Delete the ritual now\ndo that one too\n\n\non the rest:\nCall 1: the two Stop judges (babysitter.py, validate_completion.py)\nThis is stupid; instead of thinking delete yes/no – we should be thinking how can we IMPROVE the harness. You lack research on those, you completely ignore their content, and you assume they\'re all fluff while OBVIOUSLY I wrote those with agents to solve actual problems. The fact that they don\'t solve them WELL doesn\'t mean the problems weren\'t or aren\'t there.\n\nCall 2: the session-state tool\nSTOP WITH THE FUCKING CLI VS MCP TOOL. It\'s fucking cli as I said. DO WE NEED THE TOOL? HOW DO WE ARCHITECTURE IT? WHAT DO WE ADD?? THERE ARE SO MANY CRITICAL CHOICES HERE & YOU\'RE STUCK ON THE LITERALLY ONLY INCONSQEUNTIAL ONE THAT WILL NOT CHANGE THE QUALITY OF THE HARNESs BY ONE FUCKING PERCENT.\n\nCall 4: the always-on rules layer (the article\'s core claim)\nthis is also a retarded choice – it\'s literally "do we do stupid & YOLO it" or "do we put some marginal effort to make this not shit" – HOW do we trim those WHAT do we fucking trim? what problem are you solving & how is the harness getting fucking better? WHAT is the fucking proposal here? what the fuck does "trim" mean? the first 30 lines? be fucking specific & do the fukcing work.\n\nCall 5: classify_intent.py\'s model half\nwhat does the model half do? do we need it? how do we compensate if we delete it? does anything regress? what the fuck are you deciding? what the fuck are my inputs? I KNOW WE CAN EITHER KEEP OR DELETE FUCKING FILES. THAT\'S NOT FUCKING USEFUL.\n\ndon\'t act on the options I commented on. YOur thinking is clearly retarded there. Iterate until I\'m happy that you GET the problems and you weren\'t a lazy shit.'

INCIDENT_ENTER_EXECUTE = 'oh, enter /execute for the execution\n\n- unverified whether permission prompts function under your bypassPermissions default; codex has no plan mode, so cross-harness mode inheritance dies; your typed /propose becomes a harness UI mode\ncodex has planning mode\n\nbut, you\'re hititng a good point – I very much want to keep my MODES, that\'s something I use and want to use\nso delete everything is out\nkeep everything is a noop option (& thus stupid because I\'m discussing this which clearly means action is needed) so it\'s out too\n\nthe question is how muc hwe reduce\nexplore that & give me the options; be specific – i can\'t review shit like "keep the spine" or "full reduction"; those are unclear'


def test_incident_batch_approval_enters_executing(monkeypatch, state_root):
    # "okay /execute those" sits mid-message after a numbered list whose item 6
    # mentions /propose; /execute is typed later, so last-wins holds executing.
    _, text = _run(monkeypatch,
                   {"session_id": "ci1", "prompt": INCIDENT_BATCH_APPROVAL},
                   {"intent": "action"})
    assert load_state("ci1")["state"] == "execute"
    assert "This is an executing-state turn" in text


def test_incident_enter_execute_is_overridden_by_a_stray_propose(monkeypatch, state_root):
    # The architect typed /execute on line 1, then quoted a pasted finding that
    # contains a bare, unquoted "/propose" further down. Under "any position,
    # last wins" that stray token is indistinguishable from a typed command, so
    # the message resolves to proposing against the architect's intent. No rule
    # in the current contract can separate the two.
    _run(monkeypatch, {"session_id": "ci1", "prompt": INCIDENT_ENTER_EXECUTE},
         {"intent": "action"})
    assert load_state("ci1")["state"] == "propose"
