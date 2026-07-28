"""Behavioral gate for the local-LLM hooks when no model verdict comes back.

Covers classify_intent.py, validate_completion.py, and validate_plan_quality.py
with the local LLM stubbed to fail (local_llm fixture), so each hook takes its
deterministic fallback branch. Each case asserts the Python hook's own exit code,
stdout (JSON-normalized), stderr, and the control fields on the spine
(<CLAUDE_DATA_ROOT>/sessions/<session_id>/state.json) directly.

These hooks all funnel their model call through lib.model_call.run_model, which
returns None when the local LLM yields no answer. The assertions therefore pin
the deterministic behavior that holds without a model verdict: the typed
mode-command fallback (classify_intent), the pre-LLM deterministic gates and the
allow-on-failure default (validate_completion), and the empty-plan / allow-on-
failure default (validate_plan_quality).
"""

import json
import os
import shutil
import subprocess

import pytest
from conftest import PY_HOOKS

import classify_intent
import validate_completion
from lib import feedback


def _session_dir(root, sid):
    return os.path.join(str(root), "sessions", sid)


def _state_path(root, sid):
    return os.path.join(_session_dir(root, sid), "state.json")


def _reset_state(root, sid, pre):
    shutil.rmtree(_session_dir(root, sid), ignore_errors=True)
    if pre is not None:
        os.makedirs(_session_dir(root, sid), exist_ok=True)
        with open(_state_path(root, sid), "w") as fh:
            json.dump(pre, fh)


def _read_state(root, sid):
    try:
        with open(_state_path(root, sid)) as fh:
            return json.load(fh)
    except (FileNotFoundError, ValueError):
        return None


def _norm(s):
    s = s.strip()
    try:
        return json.dumps(json.loads(s), sort_keys=True)
    except ValueError:
        return s


def _run(hook, payload, env):
    r = subprocess.run(["python3", os.path.join(PY_HOOKS, hook)],
                       input=payload, text=True, capture_output=True, env=env)
    return r.returncode, r.stdout, r.stderr


# Executing-state control store: the state validate_completion reads to decide
# whether a deliverable-shaped turn warrants the LLM gate it can't reach offline.
S = {"state": "executing", "commit_requested": False}

# Forced-command fallback strings classify_intent emits when the LLM is down but
# a typed /propose | /execute | /subagents | /commit is present (fallback_context()
# and COMMIT_DIRECTIVE).
PROPOSE_FALLBACK = ("This is a proposing-state turn. Load the /propose skill now and "
                    "produce the proposal under its contract.")
EXECUTE_FALLBACK = ("This is an executing-state turn. Load the /execute skill now and "
                    "work under its contract: implement the approved work, and the moment "
                    "it needs an architectural change, stop and escalate with /pcc.")
SUBAGENTS_FALLBACK = "Load the /subagents skill now."
COMMIT_FALLBACK = "Skills to execute: /commit"

# stdout each classify_intent case emits, as the additionalContext envelope.
def _ctx(text):
    wrapped = "<classify_intent_agent>\n%s\n</classify_intent_agent>" % text
    return json.dumps({"hookSpecificOutput": {
        "hookEventName": "UserPromptSubmit", "additionalContext": wrapped}}) + "\n"


# classify_intent appends the standing behavioral reminders to every non-skipped
# turn, so each emitting case carries them after its forced-command directive.
STANDING = classify_intent.STANDING_REMINDERS


def _ctx_standing(text):
    return _ctx(text + "\n\n" + STANDING) if text else _ctx(STANDING)


# Default control fields classify_intent writes for a non-skipped session before
# its (offline-unreachable) classifier runs: the spine's _default_main_state.
CI_DEFAULT_STATE = {"approach": "subagents", "state": "proposing", "commit_requested": False,
                    "goal": None, "notes": []}

# validate_completion's forwarded-recommendation gate raises a non-halting concern
# (exit 0, systemMessage on stdout via feedback.raise_concern), not a hard block.
# Built from the gate's own constant so the message lives in one place.
_VC_FWD_WRAP = feedback.wrap(
    "validate_completion", validate_completion.FORWARDED_BLOCK_MSG % "the subagent recommends")
_VC_FWD_OUT = json.dumps({
    "systemMessage": _VC_FWD_WRAP,
    "hookSpecificOutput": {"hookEventName": "Stop", "additionalContext": _VC_FWD_WRAP}})

# name, hook, payload, session_id (None → agent- skip), pre-state,
# expected (rc, stdout, stderr, post-state)
CASES = [
    # classify_intent — typed /propose with the LLM down: forced state wins via
    # the fallback path; proposing-state contract emitted, state persisted.
    ("ci_propose", "classify_intent.py",
     {"prompt": "/propose", "transcript_path": ""}, "p_ci1", None,
     (0, _ctx_standing(PROPOSE_FALLBACK), "",
      {**CI_DEFAULT_STATE, "state": "proposing"})),
    # Compound typed command: /execute forces executing state, /subagents forces
    # the subagents approach — both fallbacks compose, both mutations persist.
    ("ci_execute_subagents", "classify_intent.py",
     {"prompt": "okay /execute and /subagents this", "transcript_path": ""}, "p_ci1", None,
     (0, _ctx_standing(EXECUTE_FALLBACK + "\n\n" + SUBAGENTS_FALLBACK), "",
      {**CI_DEFAULT_STATE, "state": "executing", "approach": "subagents"})),
    # XML-tagged system message: structurally detected, hook no-ops before any
    # state file is touched.
    ("ci_xml_skip", "classify_intent.py",
     {"prompt": "<task-notification>x</task-notification>", "transcript_path": ""}, "p_ci1", None,
     (0, "", "", None)),
    # agent- session id: subagent prompts are skipped, no state written.
    ("ci_agent_skip", "classify_intent.py",
     {"prompt": "/propose", "transcript_path": ""}, None, None,
     (0, "", "", None)),
    # Typed /commit with the LLM down: forces commit_requested via the fallback
    # path; the /commit contract is emitted and the flag persists.
    ("ci_commit", "classify_intent.py",
     {"prompt": "/commit", "transcript_path": ""}, "p_ci1", None,
     (0, _ctx_standing(COMMIT_FALLBACK), "",
      {**CI_DEFAULT_STATE, "commit_requested": True})),
    # Compound typed command: /execute forces executing state, /commit forces the
    # commit — both fallbacks compose, both mutations persist.
    ("ci_execute_commit", "classify_intent.py",
     {"prompt": "/execute /commit", "transcript_path": ""}, "p_ci1", None,
     (0, _ctx_standing(EXECUTE_FALLBACK + "\n\n" + COMMIT_FALLBACK), "",
      {**CI_DEFAULT_STATE, "state": "executing", "commit_requested": True})),
    # A /commit-suffixed token (e.g. /commit-foo) must NOT trip the /commit
    # forced command — the hyphen boundary keeps a longer name distinct.
    ("ci_commit_message_no_force", "classify_intent.py",
     {"prompt": "/commit-message", "transcript_path": ""}, "p_ci1", None,
     (0, _ctx_standing(""), "", CI_DEFAULT_STATE)),
    # Plain prompt, no typed command, LLM down: nothing to emit, but the session
    # state file is initialized to defaults before the classifier is attempted.
    ("ci_plain", "classify_intent.py",
     {"prompt": "just some text", "transcript_path": ""}, "p_ci1", None,
     (0, _ctx_standing(""), "", CI_DEFAULT_STATE)),

    # Deterministic forwarded-recommendation gate: blocks (exit 2) with the
    # forwarded-block message; state untouched.
    ("vc_forwarded", "validate_completion.py",
     {"last_assistant_message": "The subagent recommends option B.", "transcript_path": ""}, "p_vc1",
     dict(S),
     (0, _VC_FWD_OUT, "", dict(S))),
    # The same phrase inside double quotes is stripped by _strip_markdown, so the
    # forwarded gate does NOT fire: allow, phase unchanged.
    ("vc_forwarded_quoted", "validate_completion.py",
     {"last_assistant_message": 'He said "the subagent recommends option B" earlier.', "transcript_path": ""}, "p_vc1",
     dict(S),
     (0, "", "", dict(S))),
    # Benign message, no permission phrase, no mutations, no deliverable: allow.
    ("vc_benign", "validate_completion.py",
     {"last_assistant_message": "All done.", "transcript_path": ""}, "p_vc1", dict(S),
     (0, "", "", dict(S))),
    # agent- session id: subagent stops are never gated.
    ("vc_agent_skip", "validate_completion.py",
     {"last_assistant_message": "shall i proceed", "transcript_path": ""}, None, None,
     (0, "", "", None)),

    # validate_plan_quality — empty plan: nothing to evaluate, allow before the
    # model call.
    ("vpq_empty", "validate_plan_quality.py",
     {"tool_input": {"plan": ""}}, "p_vpq1", None,
     (0, "", "", None)),
    # Non-empty plan with the LLM down: run_model returns None, hook allows
    # (exit 0) rather than blocking on its own brokenness.
    ("vpq_plan_fail", "validate_plan_quality.py",
     {"tool_input": {"plan": "# Plan"}}, "p_vpq1", None,
     (0, "", "", None)),
    # agent- session id: plan gate is skipped for subagents.
    ("vpq_agent_skip", "validate_plan_quality.py",
     {"tool_input": {"plan": "# Plan"}}, None, None,
     (0, "", "", None)),
]


@pytest.fixture
def spine_root(tmp_path, monkeypatch):
    """Point the spine at a per-test data root so seeding and reading the session
    record never touches the real ~/.claude. _run copies os.environ into the
    subprocess env, so the setenv reaches the hook under test."""
    root = tmp_path / "claude"
    monkeypatch.setenv("CLAUDE_DATA_ROOT", str(root))
    return root


@pytest.mark.parametrize("name,hook,payload,sid,pre,expected", CASES, ids=[c[0] for c in CASES])
def test_fallback_behavior(name, hook, payload, sid, pre, expected, local_llm, spine_root):
    env = dict(os.environ)
    session_id = sid if sid is not None else "agent-skip"
    body = json.dumps({"session_id": session_id, **payload})

    if sid is not None:
        _reset_state(spine_root, sid, pre)
    rc, out, err = _run(hook, body, env)
    state = _read_state(spine_root, sid) if sid is not None else None

    exp_rc, exp_out, exp_err, exp_state = expected
    assert rc == exp_rc, f"{name}: exit {rc}, expected {exp_rc}"
    assert _norm(out) == _norm(exp_out), f"{name}: stdout {out!r}, expected {exp_out!r}"
    assert err.strip() == exp_err.strip(), f"{name}: stderr {err!r}, expected {exp_err!r}"
    if exp_state is None:
        assert state is None, f"{name}: expected no session record, got {state}"
    else:
        for k, v in exp_state.items():
            assert state is not None and state.get(k) == v, \
                f"{name}: control field {k}={state.get(k) if state else None}, expected {v}"


def test_validate_completion_reaches_eval_without_crash(local_llm, spine_root, write_transcript):
    """The eval-prompt builder must not reference a removed state field. A turn with
    >=3 mutations bypasses the early-allow gate and reaches session_context; with the
    LLM down the hook returns 0 — it must build that context without crashing."""
    sid = "vc_eval"
    _reset_state(spine_root, sid, dict(S))

    def edit(i):
        return {"type": "assistant", "message": {"content": [
            {"type": "tool_use", "name": "Edit", "id": str(i), "input": {"file_path": "f%d" % i}}]}}

    tpath = write_transcript([{"type": "user", "message": {"content": "go"}},
                              edit(1), edit(2), edit(3)])
    body = json.dumps({"session_id": sid, "last_assistant_message": "All good.",
                       "transcript_path": tpath})
    rc, out, err = _run("validate_completion.py", body, dict(os.environ))
    assert rc == 0, f"validate_completion crashed reaching the eval builder: {err}"
