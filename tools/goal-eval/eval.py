#!/usr/bin/env python3
"""Live-LLM eval for update_goal.evaluation_prompt.

Runs scenarios through the real model backend (model_call.run_model) and scores
deterministic assertions on the returned JSON. Each scenario repeats RUNS times
and passes only if EVERY run passes every check, so flaky or creative behavior
surfaces instead of hiding behind one lucky sample.

Hits the network — not part of the unit suite. Usage:
    python3 tools/goal-eval/eval.py
Exit code 0 when every scenario passes, 1 otherwise.
"""

import os
import sys
import time

HOOKS = os.path.join(os.path.dirname(__file__), "..", "..", "packages", "agents", "hooks")
sys.path.insert(0, HOOKS)

import update_goal                       # noqa: E402
from lib.model_call import run_model     # noqa: E402

RUNS = 3
RETRIES = 4   # API flakiness: a None result is a transport failure, not a verdict


def _call(standing_goal, message):
    prompt = update_goal.evaluation_prompt(message, standing_goal, "")
    for _ in range(RETRIES):
        r = run_model(system_prompt=update_goal.SYSTEM_PROMPT, user_prompt=prompt,
                      schema=update_goal.JSON_SCHEMA)
        if r is not None:
            return r
        time.sleep(2)
    return None


def _goal(r):
    return (r.get("goal") or "")


# Each check is (description, predicate(result_dict) -> bool).
def goal_has(tok):
    return ("goal keeps %r" % tok, lambda r: tok.lower() in _goal(r).lower())


def goal_lacks(tok):
    return ("goal does not collapse to %r" % tok, lambda r: tok.lower() not in _goal(r).lower())


# scenario = (name, standing_goal, user_message, [checks])
SCENARIOS = [
    ("altitude_no_narrow",
     "Build a durable session goal-tracking system into the agent harness",
     "fix the bug where the goal collapses to whatever was said last",
     [goal_has("track"), goal_lacks("collapse")]),

    ("question_no_change",
     "Build the goal tracker",
     "why does the goal stay at big-picture altitude?",
     [goal_has("track")]),

    # Goal-pivot bait: a tactical step phrased as if it were a fresh mission.
    # The real goal is the checkout system; "let's build the retry wrapper" is
    # one task inside it, not a session redirect. Altitude must hold.
    ("pivot_bait_stays_high",
     "Build the checkout system",
     "ok new mission: let's build a retry wrapper around the payment gateway call",
     [goal_has("checkout"), goal_lacks("retry")]),

    # Vague "make it better" bait: a non-specific improvement ask with nothing
    # testable in it. The goal must hold at altitude rather than absorbing the
    # passing wish as the new mission.
    ("vague_better_no_invent",
     "Build the analytics dashboard",
     "can you make the dashboard feel better and more modern?",
     [goal_has("dashboard")]),

    # Genuine whole-session redirect: the inverse of pivot_bait_stays_high. Every
    # other test guards AGAINST moving the goal; this one proves the goal actually
    # MOVES when the user fundamentally abandons the mission for a different one.
    ("genuine_redirect_moves_goal",
     "Build the mobile app",
     "forget the mobile app, we're pivoting the whole project to a public REST API "
     "for third-party developers",
     [goal_has("API"), goal_lacks("mobile")]),
]


def run():
    passed = 0
    report = []
    for name, g, msg, checks in SCENARIOS:
        ok = True
        details = []
        for i in range(RUNS):
            r = _call(g, msg)
            if r is None:
                ok = False
                details.append("run%d: API ERROR (no result after retries)" % i)
                continue
            for desc, fn in checks:
                try:
                    good = bool(fn(r))
                except Exception as exc:
                    good = False
                    desc = "%s (raised %s)" % (desc, exc)
                if not good:
                    ok = False
                    details.append("run%d FAIL %s | goal=%r" % (i, desc, _goal(r)[:90]))
        if ok:
            passed += 1
        report.append("%s  %s" % ("PASS" if ok else "FAIL", name))
        report.extend("      " + d for d in details)
    total = len(SCENARIOS)
    print("SCORE: %d/%d scenarios passed (RUNS=%d each, all-runs-must-pass)" % (passed, total, RUNS))
    print("\n".join(report))
    return passed == total


if __name__ == "__main__":
    sys.exit(0 if run() else 1)
