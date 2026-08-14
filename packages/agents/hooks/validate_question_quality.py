#!/usr/bin/env python3
"""Question quality gate (LLM check).

Fires before AskUserQuestion; evaluates the question via `claude -p`.
Block on rejection (exit 2) so the reason reaches the agent as tool feedback
and it reworks the question in the same turn; any failure path allows
(exit 0). A prompt-type hook cannot do this: its deny halts the turn instead
of feeding back (observed on 2.1.x).
"""

import json
import sys

from lib import feedback
from lib.event import field, read_event
from lib.model_call import run_model
from lib.session_mode import is_dispatched

BINDING = {
    "events": {"PreToolUse": ["AskUserQuestion"]},
    "timeout": 90,
    "harness": "claude",
}

JSON_SCHEMA = '{"type":"object","properties":{"ok":{"type":"boolean"},"reason":{"type":"string"}},"required":["ok"]}'
SYSTEM_PROMPT = "You are a question quality gate. Evaluate questions to the user against strict quality rules. Output structured JSON only."


def _eval_prompt(tool_input):
    return (
        'Question quality gate. Evaluate this AskUserQuestion input.\n\n'
        'Reject if:\n\n'
        '1. Asks permission to act: "Can I proceed?", "Want me to continue?", "Shall I?", "Would you like me to...". Act or present information — don\'t ask to act.\n\n'
        '2. Hedges about unverified state: "probably", "likely", "I think", "should be", "might be". Verify first, then ask.\n\n'
        '3. Fewer than 4 distinct options. Yes/no and single-option approval ("does this work?") waste the tool. Present 4+ meaningfully different alternatives with tradeoffs.\n\n'
        '4. Dumps decisions without research: "What next?", "How should I handle this?". Research and present concrete options.\n\n'
        '5. Not self-contained: Requires reading external context — "as mentioned above", "the plan I showed", "as discussed". Include all necessary context inline.\n\n'
        '6. Prompts the user to respond: Flattery ("Great question"), directive closing ("Which one?", "Which do you prefer?"), or nudges ("Let me know", "Thoughts?", "Sound good?"). Present options and stop.\n\n'
        '7. Batches multiple independent decisions. One question = one decision.\n\n'
        'Question input:\n'
        '%s\n\n'
        'Return JSON:\n'
        '- Pass: {"ok": true}\n'
        '- Fail: {"ok": false, "reason": "[Specific issue]. Fix: [What to do]."}'
    ) % tool_input


def main():
    event = read_event()
    session_id = field(event, "session_id", "")
    if not session_id or is_dispatched(event):
        return 0
    tool_input = field(event, "tool_input", None)
    if not tool_input:
        return 0

    result = run_model(system_prompt=SYSTEM_PROMPT,
                       user_prompt=_eval_prompt(json.dumps(tool_input, ensure_ascii=False)),
                       schema=JSON_SCHEMA)
    if not result:
        return 0
    if result.get("ok") is False:
        reason = result.get("reason") or "the question may not meet the quality rules"
        return feedback.block("validate_question_quality",
                              "BLOCKED: question rejected. %s" % reason)
    return 0


if __name__ == "__main__":
    sys.exit(main())
