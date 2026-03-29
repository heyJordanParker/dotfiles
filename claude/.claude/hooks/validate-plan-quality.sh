#!/bin/bash

# Plan quality gate — validates plans before ExitPlanMode
# PreToolUse hook on ExitPlanMode
# Uses claude -p for LLM-based plan evaluation

set -uo pipefail
# NOTE: no set -e — graceful allow on any failure

# Read hook event data
INPUT=$(cat)

# Skip agent sessions
SESSION_ID=$(echo "$INPUT" | jq -r '.session_id // ""' 2>/dev/null) || exit 0
[[ -z "$SESSION_ID" || "$SESSION_ID" == agent-* ]] && exit 0

# Extract plan content
PLAN=$(echo "$INPUT" | jq -r '.tool_input.plan // ""' 2>/dev/null) || exit 0
[[ -z "$PLAN" ]] && exit 0

# Truncate for LLM evaluation
PLAN=$(echo "$PLAN" | head -c 15000 2>/dev/null) || exit 0

# LLM evaluation
JSON_SCHEMA='{"type":"object","properties":{"ok":{"type":"boolean"},"reason":{"type":"string"}},"required":["ok"]}'

SYSTEM_PROMPT="You are a plan quality gate. Evaluate plans against strict quality rules. Output structured JSON only."

EVAL_PROMPT="Evaluate this plan. Reject if ANY rule is violated.

Rules:

1. Missing requirements table: Must have | ID | Requirement | Status | with R0, R1, R2... rows. Every requirement must have a committed status (e.g., \"Core goal\", \"Must-have\", \"Implementing\"). Banned statuses: \"Nice-to-have\", \"Undecided\", \"Optional\", \"If time allows\". If the user hasn't decided on a requirement, the plan isn't ready — remove it. Requirements must be specific and testable.

2. Steps are vague: Every step must reference specific file paths, functions, or mechanisms. No hedging (\"probably\", \"likely\"). No unresolved decisions (\"TBD\", \"Option A or B\").

3. Missing requirements traceability: Every step must reference which requirements it fulfills (e.g. \"fulfills R1\"). No orphan steps, no uncovered requirements.

4. Claims \"no changes needed\" without verification evidence.

5. Offers instead of acting: \"Would you like me to...\", \"Shall I...\".

6. Raises problems without solutions: Blockers must include multiple researched options with pros/cons/confidence.

7. Missing annotated file tree: Must end with a file tree showing all files to be created/modified with annotations.

8. Missing or weak validation step: Plan must include a final validation step before user review that: (a) uses /subagents skill to dispatch independent testing subagents for each validation task — never self-validate (the implementing agent is biased), (b) subagent prompts must include the WHY (what user problem this solves), may include the WHAT (what changed) if critical for testing, and never include the HOW (implementation details) to avoid bias, (c) traces every code path touched by the plan to verify correctness, (d) validates code serves real user scenarios end-to-end, (e) uses browser testing via tester agent and /agent-browser skill when UI is involved, (f) exhausts every test category fitting the scope of the change (unit, integration, user flows, edge cases), (g) autonomously fixes all issues that don't significantly change the plan's architecture, (h) final step: stop and present results to user for manual verification and feedback (no commits).

9. Contains deferred work: Items marked as deferred, punted to a future phase, declared out of scope for now, or any form of \"deferred\" or \"this ships later, not now.\" Every item in a plan ships. If it doesn't ship, it doesn't go in the plan.

10. Contains optionality: Plans must not present choices (\"Option A or B\", \"we could do X or Y\"), optional items (\"Nice-to-have\", \"if time allows\"), or undecided scope. The user decides all options and scope before the plan is finalized. Plans contain decisions, not choices.

Plan:
${PLAN}

Return JSON:
- Pass: {\"ok\": true}
- Fail: {\"ok\": false, \"reason\": \"[Specific issue]. Fix: [What to do].\"}"

RESULT=""
if CLAUDE_RESPONSE=$(CLAUDE_CLASSIFY_INTENT=true timeout 120 claude -p \
    --model opus \
    --effort low \
    --output-format json \
    --json-schema "$JSON_SCHEMA" \
    --system-prompt "$SYSTEM_PROMPT" \
    --setting-sources "" \
    --disallowedTools '*' 2>/dev/null <<< "$EVAL_PROMPT"); then
    RESULT=$(echo "$CLAUDE_RESPONSE" | jq '.structured_output // empty' 2>/dev/null) || true
fi

# If classification failed, allow (graceful)
if [ -z "$RESULT" ] || [ "$RESULT" = "null" ]; then
    exit 0
fi

# Check result
OK=$(echo "$RESULT" | jq -r 'if .ok == false then "false" else "true" end' 2>/dev/null) || exit 0
if [ "$OK" = "false" ]; then
    REASON=$(echo "$RESULT" | jq -r '.reason // "Plan quality check failed"' 2>/dev/null) || REASON="Plan quality check failed"
    echo "BLOCKED: Plan rejected. ${REASON}" >&2
    exit 2
fi

exit 0
