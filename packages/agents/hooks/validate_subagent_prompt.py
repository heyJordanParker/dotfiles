#!/usr/bin/env python3
"""Subagent prompt gate (LLM check).

Fires before an Agent dispatch; evaluates the subagent prompt for
over-instruction via `claude -p`. Block on rejection (exit 2) so the reason
reaches the orchestrator as tool feedback and it retries in the same turn;
any failure path allows (exit 0). A prompt-type hook cannot do this: its deny
halts the turn instead of feeding back (observed on 2.1.x).
"""

import sys

from lib import feedback
from lib.event import field, read_event
from lib.model_call import run_model

BINDING = {
    "events": {"PreToolUse": ["Agent"]},
    "timeout": 60,
    "harness": "claude",
}

JSON_SCHEMA = '{"type":"object","properties":{"ok":{"type":"boolean"},"reason":{"type":"string"}},"required":["ok"]}'
SYSTEM_PROMPT = "You are a subagent dispatch gate. Evaluate subagent prompts for over-instruction. Output structured JSON only."


def _eval_prompt(prompt):
    return (
        'Evaluate this subagent dispatch for over-instruction.\n\n'
        'Block if ANY of these are true:\n'
        '1. Contains pre-researched file contents (quoted code blocks with file paths, command output dumps, or content that looks copied from a file read)\n'
        '2. Specifies implementation steps — line numbers to change, specific code to write, step-by-step HOW instructions (exception: mechanical tasks like bulk renames are fine)\n'
        '3. Scoped too narrowly — asks the agent to review a single method or line instead of the module/feature/service boundary\n'
        '4. Prescribes the agent\'s investigation or implementation path — tells it which files to read in what order, which functions to trace, or what conclusions to reach, instead of letting it explore from the goal\n\n'
        'Do NOT block if:\n'
        '- The prompt communicates WHY (user story, business context, motivation) and WHAT (goal, deliverable, acceptance criteria)\n'
        '- It describes scope — which areas are involved, what changed and why, or what the goal is. This is goal-setting, not pre-research\n'
        '- It includes an annotated file tree (paths with brief annotations — this is navigation, not pre-research)\n'
        '- It includes a unique finding the subagent won\'t realistically discover (e.g., a runtime behavior observed in logs, a user-reported symptom, a constraint from a conversation with the user)\n'
        '- It uses Story/Business/Goal/DoD structure\n\n'
        'Subagent prompt:\n'
        '%s\n\n'
        'Return JSON:\n'
        '- Pass: {"ok": true}\n'
        '- Fail: {"ok": false, "reason": "[What\'s wrong]. Fix: [How to fix the prompt]."}'
    ) % prompt


def main():
    event = read_event()
    session_id = field(event, "session_id", "")
    if not session_id or session_id.startswith("agent-"):
        return 0
    prompt = field(event, "tool_input.prompt", "")
    if not prompt:
        return 0

    result = run_model(system_prompt=SYSTEM_PROMPT, user_prompt=_eval_prompt(prompt),
                       schema=JSON_SCHEMA, session_id=session_id, hook="validate_subagent_prompt")
    if not result:
        return 0
    if result.get("ok") is False:
        reason = result.get("reason") or "the subagent prompt over-instructs"
        return feedback.block("validate_subagent_prompt",
                              "BLOCKED: subagent prompt rejected. %s" % reason)
    return 0


if __name__ == "__main__":
    sys.exit(main())
