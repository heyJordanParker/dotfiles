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
from lib.session_state import load_state, merge_state


@pytest.fixture
def spine_root(tmp_path, monkeypatch):
    root = tmp_path / "claude"
    monkeypatch.setenv("CLAUDE_DATA_ROOT", str(root))
    monkeypatch.delenv("CLAUDE_SESSION_HOOK", raising=False)
    return root


def _run(monkeypatch, payload, model_result):
    monkeypatch.setattr(classify_intent, "run_model", lambda *a, **k: model_result)
    monkeypatch.setattr(classify_intent, "read_event", lambda: payload)
    captured = {}
    monkeypatch.setattr(classify_intent, "emit_context",
                        lambda text: captured.setdefault("text", text))
    rc = classify_intent.main()
    return rc, captured.get("text")


def test_question_emits_answer_contract(monkeypatch, spine_root):
    rc, text = _run(monkeypatch,
                    {"session_id": "ci1", "prompt": "why does X work this way?"},
                    {"intent": "question"})
    assert rc == 0
    assert "This is a question. Answer it" in text
    assert "Answer with specific facts, not gestures at them" in text
    assert "execute the action items" not in text


def test_question_with_action_items(monkeypatch, spine_root):
    _, text = _run(monkeypatch,
                   {"session_id": "ci1", "prompt": "how does X work? also change Y"},
                   {"intent": "question", "has_action_items": True})
    assert "Answer the question first, then execute the action items" in text
    assert "Answer with specific facts, not gestures at them" in text


def test_correction_emits_same_format_contract(monkeypatch, spine_root):
    _, text = _run(monkeypatch,
                   {"session_id": "ci1", "prompt": "no, that's wrong, use Y"},
                   {"intent": "correction"})
    assert "re-deliver the whole response in the same format as the original" in text
    assert "never a prose diff" in text


def test_plain_action_emits_only_standing_reminders(monkeypatch, spine_root):
    rc, text = _run(monkeypatch,
                    {"session_id": "ci1", "prompt": "add a guard to the parser"},
                    {"intent": "action"})
    assert rc == 0
    assert "The architect's call governs" in text
    assert "Ground every claim" in text
    assert "This is a question" not in text


def test_sequential_directive_appended(monkeypatch, spine_root):
    _, text = _run(monkeypatch,
                   {"session_id": "ci1", "prompt": "do A, then B, finally C"},
                   {"intent": "action", "sequential": True})
    assert "strictly sequential" in text


def test_notes_persisted_and_capped(monkeypatch, spine_root):
    notes = ["note %d" % i for i in range(15)]
    _run(monkeypatch, {"session_id": "ci1", "prompt": "you broke the build again"},
         {"intent": "correction", "notes": notes})
    assert load_state("ci1")["notes"] == notes[:10]


def test_notes_reinjected_on_proposing_turn(monkeypatch, spine_root):
    # A fresh main session defaults to the proposing state.
    _, text = _run(monkeypatch,
                   {"session_id": "ci1", "prompt": "look into the auth flow"},
                   {"intent": "action", "notes": ["never touch the tmux config"]})
    assert "Past corrections from this session — do not re-violate:" in text
    assert "never touch the tmux config" in text


def test_notes_not_reinjected_off_proposing(monkeypatch, spine_root):
    merge_state("ci1", {"state": "executing"})
    _, text = _run(monkeypatch,
                   {"session_id": "ci1", "prompt": "implement the slice"},
                   {"intent": "action", "notes": ["never touch the tmux config"]})
    # Persisted, but not surfaced outside a proposing turn.
    assert load_state("ci1")["notes"] == ["never touch the tmux config"]
    assert "never touch the tmux config" not in text
    # The standing reminders still ride every turn.
    assert "The architect's call governs" in text


def test_typed_propose_composes_with_question_contract(monkeypatch, spine_root):
    _, text = _run(monkeypatch,
                   {"session_id": "ci1", "prompt": "/propose what about caching X?"},
                   {"intent": "question"})
    assert "This is a proposing-state turn. Load the /propose skill now" in text
    assert "This is a question. Answer it" in text


def test_conversational_commands_on_the_first_line(monkeypatch, spine_root):
    # The architect types his commands mid-sentence: "sure, /execute & /commit after".
    _, text = _run(monkeypatch,
                   {"session_id": "ci1", "prompt": "sure, /execute & /commit after"},
                   {"intent": "action"})
    assert load_state("ci1")["state"] == "executing"
    assert load_state("ci1")["commit_requested"] is True
    assert classify_intent.COMMIT_DIRECTIVE in text


@pytest.mark.parametrize("prompt", [
    "okay /execute those",
    "sure /execute everything",
    "first paragraph\nthen also /execute it",
    "a plain paragraph with no command at all\n\n/execute",
    "point one\n- and /execute the batch after that\n",
])
def test_command_counts_on_any_line_at_any_position(monkeypatch, spine_root, prompt):
    _run(monkeypatch, {"session_id": "ci1", "prompt": prompt}, {"intent": "action"})
    assert load_state("ci1")["state"] == "executing"


def test_three_fragments_set_together(monkeypatch, spine_root):
    _run(monkeypatch, {"session_id": "ci1", "prompt": "/propose /subagents /commit"},
         {"intent": "action"})
    state = load_state("ci1")
    assert state["state"] == "proposing"
    assert state["approach"] == "subagents"
    assert state["commit_requested"] is True


@pytest.mark.parametrize("prompt", ["he said `/propose` is broken",
                                    'he said "/propose" is broken'])
def test_quoted_command_never_writes(monkeypatch, spine_root, prompt):
    merge_state("ci1", {"state": "executing"})
    _run(monkeypatch, {"session_id": "ci1", "prompt": prompt}, {"intent": "action"})
    assert load_state("ci1")["state"] == "executing"


@pytest.mark.parametrize("prompt", [
    "use ` here\nand /execute now\nand `done`",
    'he said "hi\nthen /execute\nand "ok"',
])
def test_unpaired_delimiter_never_erases_a_later_line(monkeypatch, spine_root, prompt):
    # A stray backtick or quote blanks nothing past its own line, so the command
    # typed two lines down still counts.
    _run(monkeypatch, {"session_id": "ci1", "prompt": prompt}, {"intent": "action"})
    assert load_state("ci1")["state"] == "executing"


@pytest.mark.parametrize("prompt", ["see`x`/execute",
                                    'grep "foo"/propose',
                                    'http://x.com/"a"/commit'])
def test_span_glued_to_a_slash_never_manufactures_a_command(monkeypatch, spine_root, prompt):
    # Blanking a span to spaces would fabricate a whitespace-preceded token out of
    # a glued path or URL; the filler is non-whitespace, so nothing is written.
    merge_state("ci1", {"state": "interview"})
    _run(monkeypatch, {"session_id": "ci1", "prompt": prompt}, {"intent": "action"})
    state = load_state("ci1")
    assert state["state"] == "interview"
    assert state.get("commit_requested") is not True


def test_quoted_skill_mention_emits_no_reload_directive(monkeypatch, spine_root):
    # Both deterministic scans read the same blanked text, so a quoted sentence
    # never reaches the skill scan.
    _stub_skills(monkeypatch, ["naming"])
    _, text = _run(monkeypatch,
                   {"session_id": "ci1",
                    "prompt": "the doc says `always run /naming first` before naming"},
                   {"intent": "action"})
    assert "/naming" not in text
    assert classify_intent.typed_skills(
        classify_intent.blank_spans(
            "the doc says `always run /naming first` before naming")) == []


@pytest.mark.parametrize("prompt", [
    "```\n/execute inside a fence\n```",
    "```\nuse ` here\n/execute inside a fence\n```",
])
def test_fenced_block_never_writes(monkeypatch, spine_root, prompt):
    merge_state("ci1", {"state": "proposing"})
    _run(monkeypatch, {"session_id": "ci1", "prompt": prompt}, {"intent": "action"})
    assert load_state("ci1")["state"] == "proposing"


def test_last_typed_command_wins(monkeypatch, spine_root):
    _, text = _run(monkeypatch,
                   {"session_id": "ci1", "prompt": "/propose this\n\n/execute it instead"},
                   {"intent": "action"})
    assert load_state("ci1")["state"] == "executing"
    assert "This is an executing-state turn" in text


def test_stop_hook_feedback_never_writes_the_mode(monkeypatch, spine_root):
    merge_state("ci1", {"state": "executing"})
    rc, text = _run(monkeypatch,
                    {"session_id": "ci1",
                     "prompt": "Stop hook feedback:\n/propose the fix instead"},
                    {"intent": "action"})
    assert rc == 0
    assert text is None
    assert load_state("ci1")["state"] == "executing"


def test_relayed_session_message_never_writes_the_mode(monkeypatch, spine_root):
    merge_state("ci1", {"state": "executing"})
    rc, text = _run(monkeypatch,
                    {"session_id": "ci1",
                     "prompt": "Another Claude session sent a message:\n/propose it"},
                    {"intent": "action"})
    assert rc == 0
    assert text is None
    assert load_state("ci1")["state"] == "executing"


def test_sidechain_payload_never_writes_the_parent_mode(monkeypatch, spine_root):
    merge_state("ci1", {"state": "executing"})
    rc, text = _run(monkeypatch,
                    {"session_id": "ci1", "prompt": "/propose", "isSidechain": True},
                    {"intent": "action"})
    assert rc == 0
    assert text is None
    assert load_state("ci1")["state"] == "executing"


def test_failed_state_write_emits_no_mode_directive(monkeypatch, spine_root):
    # A stale lock directory left by a killed process makes every write fail; the
    # hook must not announce a mode the state never took.
    merge_state("ci1", {"state": "proposing"})
    lock = spine_root / "sessions" / "ci1" / "state.json.lock"
    lock.mkdir()
    _, text = _run(monkeypatch, {"session_id": "ci1", "prompt": "/execute it"},
                   {"intent": "action"})
    assert "executing-state turn" not in text
    assert classify_intent.WRITE_FAILED_NOTICE in text
    assert load_state("ci1")["state"] == "proposing"


def _stub_skills(monkeypatch, names):
    monkeypatch.setattr(classify_intent, "_available_skills", lambda: set(names))


def test_typed_skill_emits_reload_directive(monkeypatch, spine_root):
    _stub_skills(monkeypatch, ["naming", "pcc"])
    _, text = _run(monkeypatch,
                   {"session_id": "ci1", "prompt": "give me names /naming"},
                   {"intent": "action"})
    assert "The architect typed /naming this turn" in text
    assert "reload for a fresh copy" in text


def test_typed_skill_ignores_path_embedded_name(monkeypatch, spine_root):
    _stub_skills(monkeypatch, ["architecture"])
    _, text = _run(monkeypatch,
                   {"session_id": "ci1",
                    "prompt": "read packages/agents/skills/architecture/SKILL.md"},
                   {"intent": "action"})
    assert "/architecture" not in text


def test_typed_skill_skips_special_command(monkeypatch, spine_root):
    _stub_skills(monkeypatch, ["propose", "naming"])
    _, text = _run(monkeypatch,
                   {"session_id": "ci1", "prompt": "/propose the fix"},
                   {"intent": "action"})
    # /propose keeps its richer state directive, never the generic reload line.
    assert "This is a proposing-state turn" in text
    assert "The architect typed /propose" not in text


def test_typed_skills_multiple(monkeypatch, spine_root):
    _stub_skills(monkeypatch, ["naming", "pcc"])
    _, text = _run(monkeypatch,
                   {"session_id": "ci1", "prompt": "use /naming and /pcc"},
                   {"intent": "action"})
    assert "The architect typed /naming, /pcc this turn" in text
    assert "Load each now" in text


def test_typed_skill_skips_disable_model_invocation(monkeypatch, spine_root):
    # A disable-model-invocation skill loads via slash-dispatch, not the Skill
    # tool — so the reload directive (which routes through the Skill tool) is
    # never emitted for it.
    _stub_skills(monkeypatch, ["review"])
    monkeypatch.setattr(classify_intent, "_skill_disables_model_invocation",
                        lambda name: name == "review")
    _, text = _run(monkeypatch,
                   {"session_id": "ci1", "prompt": "run /review now"},
                   {"intent": "action"})
    assert "The architect typed" not in text
    assert "/review" not in text


def test_typed_skills_filter_only_disabled(monkeypatch, spine_root):
    # A disabled skill drops out; a normal skill typed alongside it still gets
    # the reload directive.
    _stub_skills(monkeypatch, ["review", "naming"])
    monkeypatch.setattr(classify_intent, "_skill_disables_model_invocation",
                        lambda name: name == "review")
    _, text = _run(monkeypatch,
                   {"session_id": "ci1", "prompt": "/review then /naming"},
                   {"intent": "action"})
    assert "The architect typed /naming this turn" in text
    assert "reload for a fresh copy" in text
    assert "/review" not in text


def test_disable_model_invocation_read_from_frontmatter(tmp_path, monkeypatch):
    # The flag is read from the skill's real SKILL.md frontmatter.
    skills = tmp_path / "skills"
    (skills / "gated").mkdir(parents=True)
    (skills / "gated" / "SKILL.md").write_text(
        "---\nname: gated\ndisable-model-invocation: true\n---\nbody\n")
    (skills / "open").mkdir()
    (skills / "open" / "SKILL.md").write_text(
        "---\nname: open\n---\nbody\n")
    monkeypatch.setattr(classify_intent, "_SKILLS_DIR", str(skills))
    assert classify_intent._skill_disables_model_invocation("gated") is True
    assert classify_intent._skill_disables_model_invocation("open") is False
    assert classify_intent._skill_disables_model_invocation("missing") is False


def test_standing_reminders_present_and_not_duplicated(monkeypatch, spine_root):
    _, text = _run(monkeypatch,
                   {"session_id": "ci1", "prompt": "why does X work this way?"},
                   {"intent": "question"})
    # All three reminders ride every turn.
    assert "The architect's call governs" in text
    assert "Ground every claim and recommendation in the code" in text
    assert "Don't cut research short" in text
    # The generic no-sycophant line moved to the standing block, not duplicated
    # back into the question contract.
    assert "Don't be a sycophant" not in text


# The two real messages that exposed the positional recognition, taken verbatim
# from the session transcript (records 1670 and 1969 of
# 933f5da9-a2c3-474c-afb2-3b5ba98c64ec.jsonl).
INCIDENT_BATCH_APPROVAL = 'Ready to execute — already approved by you; one go confirms the batch\n\n1. Delete the never-fired model-call logging in session_state.py and model_call.py\n2. Delete packages/codex/rules/default.rules\n3. Delete debug/SKILL.md\'s superpowers plugin citations, its five unreached reference files, and its unattributed timing numbers\n4. Delete Architecture.md\'s clause renaming files that never existed\n5. Fix cto.md\'s stale "hardened for Opus 4.8" description line\n6. Make the diff-block form /pcc\'s single Template; /propose and proposals.md point at it; their divergent Templates and the two false "the cto Prompt covers this" references die\n7. Fold .claude/rules/prompts.md into global prompt-writing.md, keeping the full instruction set (load /cc, read Domain.md and the Architecture, place blocks per allowance, trace every term)\n8. Delete validate_subagent_prompt.py (measured: 20.8% of 903 dispatches blocked, ~27% precision, 96% of the worst prompts passed, 60s model call per dispatch)\n\n\nokay /execute those\n\nOption 1: Delete the ritual now\ndo that one too\n\n\non the rest:\nCall 1: the two Stop judges (babysitter.py, validate_completion.py)\nThis is stupid; instead of thinking delete yes/no – we should be thinking how can we IMPROVE the harness. You lack research on those, you completely ignore their content, and you assume they\'re all fluff while OBVIOUSLY I wrote those with agents to solve actual problems. The fact that they don\'t solve them WELL doesn\'t mean the problems weren\'t or aren\'t there.\n\nCall 2: the session-state tool\nSTOP WITH THE FUCKING CLI VS MCP TOOL. It\'s fucking cli as I said. DO WE NEED THE TOOL? HOW DO WE ARCHITECTURE IT? WHAT DO WE ADD?? THERE ARE SO MANY CRITICAL CHOICES HERE & YOU\'RE STUCK ON THE LITERALLY ONLY INCONSQEUNTIAL ONE THAT WILL NOT CHANGE THE QUALITY OF THE HARNESs BY ONE FUCKING PERCENT.\n\nCall 4: the always-on rules layer (the article\'s core claim)\nthis is also a retarded choice – it\'s literally "do we do stupid & YOLO it" or "do we put some marginal effort to make this not shit" – HOW do we trim those WHAT do we fucking trim? what problem are you solving & how is the harness getting fucking better? WHAT is the fucking proposal here? what the fuck does "trim" mean? the first 30 lines? be fucking specific & do the fukcing work.\n\nCall 5: classify_intent.py\'s model half\nwhat does the model half do? do we need it? how do we compensate if we delete it? does anything regress? what the fuck are you deciding? what the fuck are my inputs? I KNOW WE CAN EITHER KEEP OR DELETE FUCKING FILES. THAT\'S NOT FUCKING USEFUL.\n\ndon\'t act on the options I commented on. YOur thinking is clearly retarded there. Iterate until I\'m happy that you GET the problems and you weren\'t a lazy shit.'

INCIDENT_ENTER_EXECUTE = 'oh, enter /execute for the execution\n\n- unverified whether permission prompts function under your bypassPermissions default; codex has no plan mode, so cross-harness mode inheritance dies; your typed /propose becomes a harness UI mode\ncodex has planning mode\n\nbut, you\'re hititng a good point – I very much want to keep my MODES, that\'s something I use and want to use\nso delete everything is out\nkeep everything is a noop option (& thus stupid because I\'m discussing this which clearly means action is needed) so it\'s out too\n\nthe question is how muc hwe reduce\nexplore that & give me the options; be specific – i can\'t review shit like "keep the spine" or "full reduction"; those are unclear'


def test_incident_batch_approval_enters_executing(monkeypatch, spine_root):
    # "okay /execute those" sits mid-message after a numbered list whose item 6
    # mentions /propose; /execute is typed later, so last-wins holds executing.
    _, text = _run(monkeypatch,
                   {"session_id": "ci1", "prompt": INCIDENT_BATCH_APPROVAL},
                   {"intent": "action"})
    assert load_state("ci1")["state"] == "executing"
    assert "This is an executing-state turn" in text


def test_incident_enter_execute_is_overridden_by_a_stray_propose(monkeypatch, spine_root):
    # The architect typed /execute on line 1, then quoted a pasted finding that
    # contains a bare, unquoted "/propose" further down. Under "any position,
    # last wins" that stray token is indistinguishable from a typed command, so
    # the message resolves to proposing against the architect's intent. No rule
    # in the current contract can separate the two.
    _run(monkeypatch, {"session_id": "ci1", "prompt": INCIDENT_ENTER_EXECUTE},
         {"intent": "action"})
    assert load_state("ci1")["state"] == "proposing"
