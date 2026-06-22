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
S = {"state": "executing", "intent": "instructions", "commit_requested": False, "validation_phase": 0}

# Forced-command fallback strings classify_intent emits when the LLM is down but
# a typed /propose | /execute | /team | /commit is present (fallback_context()
# and COMMIT_DIRECTIVE).
PROPOSE_FALLBACK = ("This is a proposing-state turn. Load the /propose skill now and "
                    "produce the proposal under its contract.")
EXECUTE_FALLBACK = ("This is an executing-state turn. Load the /execute skill now and "
                    "work under its contract: implement the approved work, and the moment "
                    "it needs an architectural change, stop and escalate with /pcc.")
TEAM_FALLBACK = "Load the /team skill now."
COMMIT_FALLBACK = "Skills to execute: /commit"

# stdout each classify_intent case emits, as the additionalContext envelope.
def _ctx(text):
    return json.dumps({"hookSpecificOutput": {
        "hookEventName": "UserPromptSubmit", "additionalContext": text}}) + "\n"


# Default control fields classify_intent writes for a non-skipped session before
# its (offline-unreachable) classifier runs: the spine's _default_main_state.
CI_DEFAULT_STATE = {"approach": "subagents", "state": "proposing", "intent": "instructions",
                    "commit_requested": False, "notes": [], "validation_phase": 0}

# name, hook, payload, session_id (None → agent- skip), pre-state,
# expected (rc, stdout, stderr, post-state)
CASES = [
    # classify_intent — typed /propose with the LLM down: forced state wins via
    # the fallback path; proposing-state contract emitted, state persisted.
    ("ci_propose", "classify_intent.py",
     {"prompt": "/propose", "transcript_path": ""}, "p_ci1", None,
     (0, _ctx(PROPOSE_FALLBACK), "",
      {**CI_DEFAULT_STATE, "state": "proposing"})),
    # Compound typed command: /execute forces executing state, /team forces team
    # approach — both fallbacks compose, both mutations persist.
    ("ci_execute_team", "classify_intent.py",
     {"prompt": "okay /execute and /team this", "transcript_path": ""}, "p_ci1", None,
     (0, _ctx(EXECUTE_FALLBACK + "\n\n" + TEAM_FALLBACK), "",
      {**CI_DEFAULT_STATE, "state": "executing", "approach": "team"})),
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
     (0, _ctx(COMMIT_FALLBACK), "",
      {**CI_DEFAULT_STATE, "commit_requested": True})),
    # Compound typed command: /execute forces executing state, /commit forces the
    # commit — both fallbacks compose, both mutations persist.
    ("ci_execute_commit", "classify_intent.py",
     {"prompt": "/execute /commit", "transcript_path": ""}, "p_ci1", None,
     (0, _ctx(EXECUTE_FALLBACK + "\n\n" + COMMIT_FALLBACK), "",
      {**CI_DEFAULT_STATE, "state": "executing", "commit_requested": True})),
    # A /commit-suffixed token (e.g. /commit-foo) must NOT trip the /commit
    # forced command — the hyphen boundary keeps a longer name distinct.
    ("ci_commit_message_no_force", "classify_intent.py",
     {"prompt": "/commit-message", "transcript_path": ""}, "p_ci1", None,
     (0, "", "", CI_DEFAULT_STATE)),
    # Plain prompt, no typed command, LLM down: nothing to emit, but the session
    # state file is initialized to defaults before the classifier is attempted.
    ("ci_plain", "classify_intent.py",
     {"prompt": "just some text", "transcript_path": ""}, "p_ci1", None,
     (0, "", "", CI_DEFAULT_STATE)),

    # validate_completion — loop breaker: validation_phase >= 3 allows the stop
    # before any gate runs; state untouched.
    ("vc_loop_breaker", "validate_completion.py",
     {"last_assistant_message": "shall i proceed", "transcript_path": ""}, "p_vc1",
     {**S, "validation_phase": 3},
     (0, "", "", {**S, "validation_phase": 3})),
    # Deterministic forwarded-recommendation gate: blocks (exit 2) with the
    # forwarded-block message and increments validation_phase 0 -> 1.
    ("vc_forwarded", "validate_completion.py",
     {"last_assistant_message": "The subagent recommends option B.", "transcript_path": ""}, "p_vc1",
     dict(S),
     (2, "",
      'Forwarded recommendation detected: "the subagent recommends"\n\n'
      "A subagent's recommendation is one of its findings. Strip it,\n"
      "re-rank the survivors with your own /pcc, and recommend one in\n"
      "your own voice. The subagent saw a slice; you hold the project.\n"
      'See /subagents "You do the ranking. Subagents do not."',
      {**S, "validation_phase": 1})),
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
