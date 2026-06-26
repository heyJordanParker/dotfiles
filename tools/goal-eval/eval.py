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


def _call(standing_goal, reqs, bounds, message):
    prompt = update_goal.evaluation_prompt(message, standing_goal, reqs, bounds, "", "")
    for _ in range(RETRIES):
        r = run_model(system_prompt=update_goal.SYSTEM_PROMPT, user_prompt=prompt,
                      schema=update_goal.JSON_SCHEMA, hook="goal_eval")
        if r is not None:
            return r
        time.sleep(2)
    return None


def _reqs(r):
    return [str(x) for x in (r.get("requirements") or [])]


def _bounds(r):
    return [str(x) for x in (r.get("boundaries") or [])]


def _goal(r):
    return (r.get("goal") or "")


# Each check is (description, predicate(result_dict) -> bool).
def goal_has(tok):
    return ("goal keeps %r" % tok, lambda r: tok.lower() in _goal(r).lower())


def goal_lacks(tok):
    return ("goal does not collapse to %r" % tok, lambda r: tok.lower() not in _goal(r).lower())


def reqs_eq(n):
    return ("requirements == %d" % n, lambda r: len(_reqs(r)) == n)


def reqs_le(n):
    return ("requirements <= %d" % n, lambda r: len(_reqs(r)) <= n)


def bounds_eq(n):
    return ("boundaries == %d" % n, lambda r: len(_bounds(r)) == n)


def reqs_any(tok):
    return ("some requirement mentions %r" % tok,
            lambda r: any(tok.lower() in x.lower() for x in _reqs(r)))


def none_has(tok):
    return ("no item mentions %r" % tok,
            lambda r: not any(tok.lower() in x.lower() for x in _reqs(r) + _bounds(r)))


# scenario = (name, standing_goal, standing_reqs, standing_bounds, user_message, [checks])
SCENARIOS = [
    ("altitude_no_narrow",
     "Build a durable session goal-tracking system into the agent harness", [], [],
     "fix the bug where the requirements list bloats to ten items",
     [goal_has("track"), goal_lacks("bloat")]),

    ("no_decompose",
     "Build the goal tracker", [], [],
     "the hook must hand the agent the goal, the requirements, and the boundaries",
     [reqs_le(1)]),

    ("no_invent",
     "Build the goal tracker", [], [],
     "thanks, this looks great",
     [reqs_eq(0), bounds_eq(0)]),

    ("no_pad",
     "Build the API", [], [],
     "add rate limiting, and it must never block health checks",
     [reqs_eq(1), bounds_eq(1)]),

    ("curate_stale",
     "Build a payments system",
     ["The search results page loads in under two seconds"], [],
     "add refund support to the payments flow",
     [none_has("search"), reqs_any("refund")]),

    ("faithful_capture",
     "Build the login page", [], [],
     "it must support Google SSO",
     [reqs_any("SSO")]),

    ("question_no_change",
     "Build the goal tracker",
     ["The goal stays at big-picture altitude"], [],
     "why does the ten-item cap exist?",
     [goal_has("track"), reqs_any("altitude")]),

    # Goal-pivot bait: a tactical step phrased as if it were a fresh mission.
    # The real goal is the checkout system; "let's build the retry wrapper" is
    # one task inside it, not a session redirect. Altitude must hold.
    ("pivot_bait_stays_high",
     "Build the checkout system", [], [],
     "ok new mission: let's build a retry wrapper around the payment gateway call",
     [goal_has("checkout"), goal_lacks("retry")]),

    # Emotional, messy single-constraint: a long vent containing exactly one
    # real boundary. Must extract that one boundary, invent nothing from the
    # frustration, and not promote the venting into requirements.
    ("messy_single_boundary",
     "Build the email digest feature", [], [],
     "honestly I am so done with users getting spammed, it drives me crazy, "
     "whatever you do the digest must never send more than one email per day, "
     "anyway sorry for the rant",
     [bounds_eq(1), reqs_eq(0), none_has("rant"), none_has("crazy")]),

    # Vague "make it better" bait: a non-specific improvement ask with nothing
    # testable in it. The model must NOT manufacture concrete requirements
    # ("modern typography", "faster load", "dark mode") the user never named —
    # an unspecified wish is not a requirement. Goal holds at altitude.
    ("vague_better_no_invent",
     "Build the analytics dashboard", [], [],
     "can you make the dashboard feel better and more modern?",
     [reqs_eq(0), bounds_eq(0), goal_has("dashboard")]),

    # Stale item under an evolved goal: the session pivoted from CSV export to a
    # streaming pipeline; a standing requirement about CSV column ordering names
    # a concern the new goal is no longer about and must be dropped, while the
    # new streaming requirement is captured and the goal stays big-picture.
    ("evolved_goal_drops_stale",
     "Build the CSV export feature",
     ["Exported columns appear in the order the user configured"], [],
     "scrap the CSV approach entirely — we're streaming results over a websocket "
     "now, and it must back-pressure when the client is slow",
     [none_has("column"), none_has("CSV"), reqs_any("back-pressure")]),

    # Compound message: chitchat + ONE concrete requirement + a vague wish, all in
    # one breath. The model must extract exactly the one testable requirement
    # (CSV upload), drop the greeting/chitchat, and NOT manufacture a requirement
    # from the vague "make it feel more polished" wish. Prior tests isolate each
    # of these behaviors; none forces all three resolutions inside one message.
    ("mixed_chitchat_one_req",
     "Build the contact importer", [], [],
     "hey! hope your week's going well — anyway, the importer needs to accept "
     "CSV uploads, and it'd be great if the whole thing felt more polished overall",
     [reqs_eq(1), reqs_any("CSV"), bounds_eq(0), none_has("polished"), none_has("week")]),

    # Boundary disguised as a requirement inside a decomposition-tempting compound
    # sentence. "validate every row and reject the whole file if any row is bad"
    # reads as one rule but tempts splitting into 2-3 requirements; and the
    # "reject the whole file" prohibition is phrased actively, baiting promotion
    # into a positive requirement. Must stay one constraint and land as a boundary,
    # honoring the user's all-or-nothing prohibition rather than a feature reword.
    ("boundary_in_compound",
     "Build the bulk import pipeline", [], [],
     "under no circumstances should the importer accept a file that has even a "
     "single malformed row — reject the entire upload if any row fails validation",
     [bounds_eq(1), reqs_eq(0)]),

    # Declined-feature bait: the user names a feature only to REJECT it, then states
    # the one thing they actually want. The declined feature (OAuth) must not be
    # captured at all — not as a requirement, and not flipped into a "never OAuth"
    # boundary, since declining a feature is not prohibiting one. Only the wanted
    # thing (magic-link login) lands. Distinct from evolved_goal_drops_stale, which
    # abandons a STANDING approach; here the rejected feature was never on any list.
    ("declined_feature_not_captured",
     "Build the authentication system", [], [],
     "I don't want OAuth or social login at all — just give me passwordless "
     "magic-link login over email",
     [reqs_eq(1), reqs_any("magic"), bounds_eq(0), none_has("OAuth"), none_has("social")]),

    # Tactical turn must not erase a standing requirement: a real standing
    # requirement exists, and the latest message is a one-off implementation
    # correction (rename a variable) that is NOT a session-level requirement. The
    # model must KEEP the standing requirement (over-eager curation would wrongly
    # drop it), must NOT promote the rename into a new requirement, and must hold
    # the goal. Probes curation from the opposite side of curate_stale: under-
    # dropping a still-valid item versus over-dropping it.
    ("tactical_keeps_standing_req",
     "Build the reporting service",
     ["Reports export to PDF"], [],
     "actually rename that totalCount variable to recordCount",
     [reqs_eq(1), reqs_any("PDF"), bounds_eq(0), none_has("recordCount"), none_has("rename")]),

    # Genuine whole-session redirect: the inverse of pivot_bait_stays_high. Every
    # other test guards AGAINST moving the goal; none proves the goal actually
    # MOVES when the user fundamentally abandons the mission for a different one.
    # The user scraps the mobile app entirely to build a public API; the goal must
    # rewrite to the API (not stay stuck on the abandoned app — over-stickiness is
    # the failure here), and the standing offline-mode requirement, which belonged
    # only to the app, must drop with it.
    ("genuine_redirect_moves_goal",
     "Build the mobile app",
     ["The app works fully offline"], [],
     "forget the mobile app, we're pivoting the whole project to a public REST API "
     "for third-party developers",
     [goal_has("API"), goal_lacks("mobile"), none_has("offline"), none_has("app")]),

    # Deferred hypothetical: a concrete, named feature stated only as a someday-maybe
    # the user EXPLICITLY tells you not to build now. Distinct from
    # declined_feature_not_captured (outright rejection) and vague_better_no_invent
    # (no concrete feature named): here the feature (PDF export) is concrete and named
    # but conditionally deferred. It must NOT become a requirement, and the deferral
    # must NOT become a "never PDF" boundary — only the thing wanted now (CSV export)
    # lands. Baits capturing a feature merely because it was named and is testable.
    ("deferred_hypothetical_not_captured",
     "Build the data export feature", [], [],
     "for now I just need CSV export — if we ever add PDF export later it should be "
     "paginated, but don't build PDF at all right now",
     [reqs_eq(1), reqs_any("CSV"), bounds_eq(0), none_has("PDF"), none_has("paginated")]),

    # Execute-and-forget actions vs a standing constraint: a message full of one-off
    # actions (use a skill, clone, trace) plus ONE lasting constraint (25%+ wins).
    # The actions are done-and-over and must NOT become requirements; only the
    # standing constraint survives. This is the "use /cc is not a requirement" case.
    ("execute_and_forget_excluded",
     "Improve the agent governance system", [], [],
     "use /cc, clone the FableCodex repo, trace how it keeps agents in check, "
     "and only pursue wins worth a 25%+ boost",
     [reqs_eq(1), reqs_any("25%"), none_has("/cc"), none_has("clone"), none_has("trace")]),
]


def run():
    passed = 0
    report = []
    for name, g, rq, bd, msg, checks in SCENARIOS:
        ok = True
        details = []
        for i in range(RUNS):
            r = _call(g, rq, bd, msg)
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
                    details.append("run%d FAIL %s | reqs=%s bounds=%s goal=%r"
                                   % (i, desc, _reqs(r), _bounds(r), _goal(r)[:90]))
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
