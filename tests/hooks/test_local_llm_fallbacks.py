"""Behavioral gate for the local-LLM hooks when no model verdict comes back.

Covers classify_intent.py, babysitter.py, and validate_plan_quality.py with the
local LLM stubbed to fail (local_llm fixture), so each hook takes its
deterministic fallback branch. Each case asserts the Python hook's own exit code,
stdout (JSON-normalized), stderr, and the control fields in session state
(<CLAUDE_DATA_ROOT>/sessions/<session_id>/state.json) directly.

These hooks all funnel their model call through lib.model_call.run_model, which
returns None when the local LLM yields no answer. The assertions therefore pin
the deterministic behavior that holds without a model verdict: the typed
mode-command fallback (classify_intent), the one pre-LLM gate and the
allow-on-failure default (babysitter), and the empty-plan / allow-on-failure
default (validate_plan_quality).
"""

import io
import json
import os
import shutil
import subprocess
import sys

import babysitter
import classify_intent
import pytest
import validate_plan_quality
from conftest import PY_HOOKS

_MODULES = {"classify_intent.py": classify_intent,
            "babysitter.py": babysitter,
            "validate_plan_quality.py": validate_plan_quality}

# Cases that stay a real `python3 <hook>` run, one emitting and one silent per
# hook, because the harness reads the exit code and the stdout envelope off the
# process rather than off a return value.
PROCESS_CASES = {"ci_propose", "ci_xml_skip", "vc_forwarded", "vc_benign",
                 "vpq_empty", "vpq_plan_fail"}


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


def _call(hook, payload, monkeypatch, capsys):
    monkeypatch.setattr(sys, "stdin", io.StringIO(payload))
    rc = _MODULES[hook].main()
    captured = capsys.readouterr()
    return rc, captured.out, captured.err


# Executing-state control store: the state the stop gate reads to decide
# whether a deliverable-shaped turn warrants the LLM gate it can't reach offline.
S = {"state": "execute", "commit_requested": False}

# Forced-command fallback strings classify_intent emits when the LLM is down but
# a typed /propose | /execute | /subagents | /commit is present (fallback_context()
# and COMMIT_DIRECTIVE).
PROPOSE_FALLBACK = ("This is a proposing-state turn. Use /propose now and produce the "
                    "proposal under its contract.")
EXECUTE_FALLBACK = ("This is an executing-state turn. Use /execute now and work under its "
                    "contract: implement the approved work, and the moment it needs an "
                    "architectural change, stop and escalate with /pcc.")
ORCHESTRATE_FALLBACK = "Use /orchestrate now."
BUILD_FALLBACK = "Use /build now."
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
# its (offline-unreachable) classifier runs: session_state's _default_main_state.
CI_DEFAULT_STATE = {"mode": "build", "state": "propose", "commit_requested": False,
                    "goal": None, "notes": []}

# name, hook, payload, session_id (None → agent- skip), pre-state,
# expected (rc, stdout, stderr, post-state)
CASES = [
    ("ci_propose", "classify_intent.py",
     {"prompt": "/propose", "transcript_path": ""}, "p_ci1", None,
     # The mode line names the mode the session governs under, not the word typed:
     # the skill the agent loads and the mode the gates enforce come off one policy.
     (0, _ctx_standing(PROPOSE_FALLBACK + "\n\n" + BUILD_FALLBACK), "",
      {**CI_DEFAULT_STATE, "state": "propose"})),
    ("ci_xml_skip", "classify_intent.py",
     {"prompt": "<task-notification>x</task-notification>", "transcript_path": ""}, "p_ci1", None,
     (0, "", "", None)),
    # Forwarded-recommendation wording is a fact for the judge, not a verdict of its
    # own, so with the model unreachable the gate says nothing rather than ruling on
    # a substring.
    ("vc_forwarded", "babysitter.py",
     {"last_assistant_message": "The subagent recommends option B.", "transcript_path": ""}, "p_vc1",
     dict(S),
     (0, "", "", dict(S))),
    ("vc_benign", "babysitter.py",
     {"last_assistant_message": "All done.", "transcript_path": ""}, "p_vc1", dict(S),
     (0, "", "", dict(S))),
    # A dispatch is named by the sidechain marker, not by the session id: a Claude
    # subagent's payload carries the parent's UUID, so the id cannot tell them apart.
    ("vc_agent_skip", "babysitter.py",
     {"last_assistant_message": "shall i proceed", "transcript_path": "",
      "isSidechain": True}, None, None,
     (0, "", "", None)),
    ("vpq_plan_fail", "validate_plan_quality.py",
     {"tool_input": {"plan": "# Plan"}}, "p_vpq1", None,
     (0, "", "", None)),
    ("vpq_agent_skip", "validate_plan_quality.py",
     {"tool_input": {"plan": "# Plan"}, "isSidechain": True}, None, None,
     (0, "", "", None)),
]


@pytest.fixture
def state_root(tmp_path, monkeypatch):
    """Point session state at a per-test data root so seeding and reading the session
    record never touches the real ~/.claude. _run copies os.environ into the
    subprocess env, so the setenv reaches the hook under test."""
    root = tmp_path / "claude"
    monkeypatch.setenv("CLAUDE_DATA_ROOT", str(root))
    return root


@pytest.mark.parametrize("name,hook,payload,sid,pre,expected", CASES, ids=[c[0] for c in CASES])
def test_fallback_behavior(name, hook, payload, sid, pre, expected, local_llm, state_root,
                           monkeypatch, capsys):
    session_id = sid if sid is not None else "agent-skip"
    body = json.dumps({"session_id": session_id, **payload})

    if sid is not None:
        _reset_state(state_root, sid, pre)
    if name in PROCESS_CASES:
        rc, out, err = _run(hook, body, dict(os.environ))
    else:
        rc, out, err = _call(hook, body, monkeypatch, capsys)
    state = _read_state(state_root, sid) if sid is not None else None

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


def test_stop_gate_reaches_eval_without_crash(local_llm, state_root, write_transcript,
                                              monkeypatch, capsys):
    """The eval-prompt builder must not reference a removed state field. A turn that
    edits files gathers the read-log facts and reaches the full builder; with the LLM
    down the hook returns 0 — it must build that prompt without crashing."""
    sid = "vc_eval"
    _reset_state(state_root, sid, dict(S))

    def edit(i):
        return {"type": "assistant", "message": {"content": [
            {"type": "tool_use", "name": "Edit", "id": str(i), "input": {"file_path": "f%d" % i}}]}}

    tpath = write_transcript([{"type": "user", "message": {"content": "go"}},
                              edit(1), edit(2), edit(3)])
    body = json.dumps({"session_id": sid, "last_assistant_message": "All good.",
                       "transcript_path": tpath})
    rc, out, err = _call("babysitter.py", body, monkeypatch, capsys)
    assert rc == 0, f"babysitter crashed reaching the eval builder: {err}"
