#!/usr/bin/env python3
"""Plan quality gate (LLM check).

Fires before ExitPlanMode; reads the plan from the payload and evaluates it
against ten strict quality rules via `claude -p`. Block on rejection (exit 2);
any failure path allows (exit 0). validate-plan-quality.sh is the
plugin-distributed shell copy of this source.
"""

import sys

from lib.event import field, read_event
from lib.model_call import run_model

BINDING = {
    "events": {"PreToolUse": ["ExitPlanMode"]},
    "timeout": 180,
    "harness": "claude",
}

JSON_SCHEMA = '{"type":"object","properties":{"ok":{"type":"boolean"},"reason":{"type":"string"}},"required":["ok"]}'
SYSTEM_PROMPT = "You are a plan quality gate. Evaluate plans against strict quality rules. Output structured JSON only."


def _eval_prompt(plan):
    return (
        'Evaluate this plan. Reject if ANY rule is violated.\n\n'
        'Rules:\n\n'
        '1. Missing requirements table: Must have | ID | Requirement | Status | with R0, R1, R2... rows. Every requirement must have a committed status (e.g., "Core goal", "Must-have", "Implementing"). Banned statuses: "Nice-to-have", "Undecided", "Optional", "If time allows". If the user hasn\'t decided on a requirement, the plan isn\'t ready — remove it. Requirements must be specific and testable.\n\n'
        '2. Steps are vague: Every step must reference specific file paths, functions, or mechanisms. No hedging ("probably", "likely"). No unresolved decisions ("TBD", "Option A or B").\n\n'
        '3. Missing requirements traceability: Every step must reference which requirements it fulfills (e.g. "fulfills R1"). No orphan steps, no uncovered requirements.\n\n'
        '4. Claims "no changes needed" without verification evidence.\n\n'
        '5. Offers instead of acting: "Would you like me to...", "Shall I...".\n\n'
        '6. Raises blockers without solutions: architecture blockers must include multiple researched options with pros/cons/confidence. Convention blockers: name the repo precedent that applies. Implementation blockers: state the fix the agent is taking.\n\n'
        '7. Missing annotated file tree: Must end with a file tree showing all files to be created/modified with annotations.\n\n'
        '8. Missing or weak validation step: Plan must include a final validation step before user review that: (a) uses /subagents skill to dispatch independent testing subagents for each validation task — never self-validate (the implementing agent is biased), (b) subagent prompts must include the WHY (what user problem this solves), may include the WHAT (what changed) if critical for testing, and never include the HOW (implementation details) to avoid bias, (c) traces every code path touched by the plan to verify correctness, (d) validates code serves real user scenarios end-to-end, (e) uses browser testing via tester agent and /agent-browser skill when UI is involved, (f) exhausts every test category fitting the scope of the change (unit, integration, user flows, edge cases), (g) autonomously fixes all issues that don\'t significantly change the plan\'s architecture, (h) final step: stop and present results to user for manual verification and feedback (no commits).\n\n'
        '9. Contains deferred work: Items marked as deferred, punted to a future phase, declared out of scope for now, or any form of "deferred" or "this ships later, not now." Every item in a plan ships. If it doesn\'t ship, it doesn\'t go in the plan.\n\n'
        '10. Contains optionality: Plans must not present choices ("Option A or B", "we could do X or Y"), optional items ("Nice-to-have", "if time allows"), or undecided scope. The user decides all options and scope before the plan is finalized. Plans contain decisions, not choices.\n\n'
        'Plan:\n'
        '%s\n\n'
        'Return JSON:\n'
        '- Pass: {"ok": true}\n'
        '- Fail: {"ok": false, "reason": "[Specific issue]. Fix: [What to do]."}'
    ) % plan


def main():
    event = read_event()
    session_id = field(event, "session_id", "")
    if not session_id or session_id.startswith("agent-"):
        return 0
    plan = field(event, "tool_input.plan", "")
    if not plan:
        return 0

    result = run_model(system_prompt=SYSTEM_PROMPT, user_prompt=_eval_prompt(plan),
                       schema=JSON_SCHEMA, session_id=session_id, hook="validate_plan_quality")
    if not result:
        return 0
    if result.get("ok") is False:
        reason = result.get("reason") or "Plan quality check failed"
        sys.stderr.write("BLOCKED: Plan rejected. %s\n" % reason)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
