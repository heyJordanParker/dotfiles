#!/bin/bash

# Validate agent work before allowing stop
# Two-phase gate: requirements checklist → scope-appropriate validation
# Uses session state for phase tracking, transcript for plan extraction

set -uo pipefail
# NOTE: no set -e — graceful allow on any failure

# Read hook event data
EVENT=$(cat)

# Skip agent sessions
SESSION_ID=$(echo "$EVENT" | jq -r '.session_id // ""' 2>/dev/null) || exit 0
[[ -z "$SESSION_ID" || "$SESSION_ID" == agent-* ]] && exit 0

TRANSCRIPT_PATH=$(echo "$EVENT" | jq -r '.transcript_path // ""' 2>/dev/null) || exit 0
CWD=$(echo "$EVENT" | jq -r '.cwd // ""' 2>/dev/null) || CWD=""
LAST_ASSISTANT_MSG=$(echo "$EVENT" | jq -r '.last_assistant_message // ""' 2>/dev/null) || LAST_ASSISTANT_MSG=""

STATE_FILE="/tmp/claude-session-state-${SESSION_ID}"

# Read session state (graceful if missing — allow stop)
VALIDATION_PHASE=0
CURRENT_APPROACH="default"
CURRENT_NOTES="[]"
FINALIZE="false"
if [ -f "$STATE_FILE" ]; then
    VALIDATION_PHASE=$(jq -r '.validation_phase // 0' "$STATE_FILE" 2>/dev/null) || VALIDATION_PHASE=0
    CURRENT_APPROACH=$(jq -r '.approach // "default"' "$STATE_FILE" 2>/dev/null) || CURRENT_APPROACH="default"
    CURRENT_NOTES=$(jq -c '.notes // []' "$STATE_FILE" 2>/dev/null) || CURRENT_NOTES="[]"
    FINALIZE=$(jq -r '.finalize // false' "$STATE_FILE" 2>/dev/null) || FINALIZE="false"
fi

# Phase 2 complete — allow stop
if [ "$VALIDATION_PHASE" -ge 2 ] 2>/dev/null; then
    exit 0
fi

# Complexity check: edits this turn + plan existence
HAS_PLAN=false
EDITS_THIS_TURN=0
if [ -n "$TRANSCRIPT_PATH" ] && [ -f "$TRANSCRIPT_PATH" ]; then
    # Count Write/Edit tool uses in current assistant turn (after last real user message)
    # Note: awk {exit} causes non-zero pipeline exit with pipefail, so capture output first
    CURRENT_TURN=$(tac "$TRANSCRIPT_PATH" | awk '/"type":"user"/ && !/tool_use_id/ {exit} {print}' 2>/dev/null) || true
    EDITS_THIS_TURN=$(echo "$CURRENT_TURN" | grep -c '"name":"Write"\|"name":"Edit"' 2>/dev/null) || EDITS_THIS_TURN=0

    # Check for plan in transcript
    if grep -q "ExitPlanMode" "$TRANSCRIPT_PATH" 2>/dev/null; then
        HAS_PLAN=true
    else
        # Fallback: check plan file on disk
        SLUG=$(grep -m1 '"slug"' "$TRANSCRIPT_PATH" 2>/dev/null | jq -r '.slug // ""' 2>/dev/null) || SLUG=""
        if [ -n "$SLUG" ] && [ -f "$HOME/.claude/plans/${SLUG}.md" ]; then
            HAS_PLAN=true
        fi
    fi
fi

# 0 file edits this turn — skip always (pure conversation)
if [ "$EDITS_THIS_TURN" -eq 0 ] 2>/dev/null; then
    exit 0
fi

# No plan — skip always (unplanned work doesn't need validation)
if [ "$HAS_PLAN" = "false" ]; then
    exit 0
fi

# Extract context from transcript
FIRST_USER_MSG=""
LAST_USER_MSGS=""
PLAN_CONTENT=""
if [ -n "$TRANSCRIPT_PATH" ] && [ -f "$TRANSCRIPT_PATH" ]; then
    # First user message (the original direction)
    FIRST_USER_MSG=$(jq -r 'select(.type == "human") | .message.content // "" | if type == "array" then map(select(.type == "text") | .text) | join(" ") else . end' "$TRANSCRIPT_PATH" 2>/dev/null | head -n 1 | head -c 2000) || true

    # Last 4 user messages
    LAST_USER_MSGS=$(jq -r 'select(.type == "human") | .message.content // "" | if type == "array" then map(select(.type == "text") | .text) | join(" ") else . end' "$TRANSCRIPT_PATH" 2>/dev/null | tail -n 4 | head -c 4000) || true

    # Extract last plan (if any)
    PLAN_CONTENT=$(grep "ExitPlanMode" "$TRANSCRIPT_PATH" 2>/dev/null \
        | jq -r 'select(.type == "assistant") | .message.content[]? | select(.type == "tool_use" and .name == "ExitPlanMode") | .input.plan' 2>/dev/null \
        | tail -1 | head -c 6000) || true

    # If no plan in JSONL, try plan file on disk
    if [ -z "$PLAN_CONTENT" ]; then
        SLUG=$(grep -m1 '"slug"' "$TRANSCRIPT_PATH" 2>/dev/null | jq -r '.slug // ""' 2>/dev/null) || SLUG=""
        if [ -n "$SLUG" ] && [ -f "$HOME/.claude/plans/${SLUG}.md" ]; then
            PLAN_CONTENT=$(head -c 6000 "$HOME/.claude/plans/${SLUG}.md" 2>/dev/null) || true
        fi
    fi
fi

# Build classifier prompt based on phase
PLAN_CONTEXT=""
if [ -n "$PLAN_CONTENT" ]; then
    PLAN_CONTEXT="
Plan for this session:
${PLAN_CONTENT}
---
"
fi

NOTES_CONTEXT=""
NOTES_COUNT=$(echo "$CURRENT_NOTES" | jq 'length' 2>/dev/null) || NOTES_COUNT=0
if [ "$NOTES_COUNT" -gt 0 ]; then
    FORMATTED_NOTES=$(echo "$CURRENT_NOTES" | jq -r 'to_entries | map("- \(.value)") | join("\n")' 2>/dev/null) || FORMATTED_NOTES=""
    NOTES_CONTEXT="
Session notes:
${FORMATTED_NOTES}
---
"
fi

if [ "$VALIDATION_PHASE" -eq 0 ] 2>/dev/null; then
    # Phase 0 → 1: Requirements checklist + lint/build

    JSON_SCHEMA='{"type":"object","properties":{"requirements":{"type":"array","items":{"type":"object","properties":{"id":{"type":"string"},"requirement":{"type":"string"},"status":{"type":"string","enum":["done","partial","missing","not_applicable"]},"notes":{"type":"string"}},"required":["id","requirement","status"]}},"plan_validations":{"type":"array","items":{"type":"object","properties":{"id":{"type":"string"},"validation":{"type":"string"},"status":{"type":"string","enum":["done","partial","missing","not_applicable"]},"notes":{"type":"string"}},"required":["id","validation","status"]}},"risk_level":{"type":"string","enum":["low","medium","high"]},"risk_reason":{"type":"string"}},"required":["requirements","risk_level"]}'

    SYSTEM_PROMPT="You are a completion validator. Analyze conversation context and extract requirements with completion status. Output structured JSON only."

    EVAL_PROMPT="Analyze this session and extract all user requirements with their completion status.

${PLAN_CONTEXT}${NOTES_CONTEXT}First user message (original direction):
${FIRST_USER_MSG}

Recent user messages:
${LAST_USER_MSGS}

Last agent message:
${LAST_ASSISTANT_MSG}

Tasks:
1. Extract every requirement the user stated (explicitly or implicitly). For each, assess if the agent's work completed it based on the last agent message and conversation flow.
2. If a plan exists, also extract the plan's validation requirements separately into \"plan_validations\". If no plan exists, return an empty array.
3. Assess overall risk level:
   - \"low\": typos, formatting, config changes, documentation
   - \"medium\": features within existing patterns, refactoring, adding hooks/skills
   - \"high\": database schema changes, money/payment logic, core UI deletion, authentication/authorization, data migration

Return JSON with requirements, plan_validations (if plan exists), risk_level, and risk_reason."

    # Run classifier
    RESULT=""
    if CLAUDE_RESPONSE=$(CLAUDE_CLASSIFY_INTENT=true timeout 45 claude -p \
        --model opus \
        --effort low \
        --output-format json \
        --json-schema "$JSON_SCHEMA" \
        --system-prompt "$SYSTEM_PROMPT" \
        --setting-sources "" \
        --disallowedTools '*' 2>/dev/null <<< "$EVAL_PROMPT"); then
        RESULT=$(echo "$CLAUDE_RESPONSE" | jq '.structured_output // empty' 2>/dev/null) || true
    fi

    # If classification failed, allow stop
    if [ -z "$RESULT" ] || [ "$RESULT" = "null" ]; then
        exit 0
    fi

    # Format requirements table
    REQ_TABLE=$(echo "$RESULT" | jq -r '.requirements // [] | map("| \(.id) | \(.requirement) | \(.status) | \(.notes // "-") |") | join("\n")' 2>/dev/null) || REQ_TABLE=""
    RISK_LEVEL=$(echo "$RESULT" | jq -r '.risk_level // "medium"' 2>/dev/null) || RISK_LEVEL="medium"
    RISK_REASON=$(echo "$RESULT" | jq -r '.risk_reason // ""' 2>/dev/null) || RISK_REASON=""

    # Format plan validations table (if any)
    PLAN_VAL_TABLE=""
    PLAN_VAL_COUNT=$(echo "$RESULT" | jq '.plan_validations // [] | length' 2>/dev/null) || PLAN_VAL_COUNT=0
    if [ "$PLAN_VAL_COUNT" -gt 0 ]; then
        PLAN_VAL_TABLE=$(echo "$RESULT" | jq -r '.plan_validations // [] | map("| \(.id) | \(.validation) | \(.status) | \(.notes // "-") |") | join("\n")' 2>/dev/null) || PLAN_VAL_TABLE=""
    fi

    # Update phase (create state file if it doesn't exist)
    if [ -f "$STATE_FILE" ]; then
        jq --arg risk "$RISK_LEVEL" '.validation_phase = 1 | .risk_level = $risk' "$STATE_FILE" > "${STATE_FILE}.tmp" 2>/dev/null && mv "${STATE_FILE}.tmp" "$STATE_FILE" 2>/dev/null || true
    else
        jq -n --arg risk "$RISK_LEVEL" '{approach: "default", finalize: false, notes: [], validation_phase: 1, risk_level: $risk}' > "$STATE_FILE" 2>/dev/null || true
    fi

    # Build block message
    BLOCK_MSG="Before stopping, verify your work.

1. Lint and build the project — fix any obvious issues found.

2. Requirements checklist:

| # | Requirement | Status | Notes |
|---|-------------|--------|-------|
${REQ_TABLE}"

    if [ -n "$PLAN_VAL_TABLE" ]; then
        BLOCK_MSG="${BLOCK_MSG}

3. Plan validation requirements:

| # | Validation | Status | Notes |
|---|------------|--------|-------|
${PLAN_VAL_TABLE}"
    fi

    BLOCK_MSG="${BLOCK_MSG}

Risk level: ${RISK_LEVEL} (${RISK_REASON})

Complete all items marked partial or missing, then stop again for deeper validation."

    echo "$BLOCK_MSG" >&2
    exit 2

elif [ "$VALIDATION_PHASE" -eq 1 ] 2>/dev/null; then
    # Phase 1 → 2: Scope-appropriate validation
    # 1-2 edits in plan = phase 1 was enough, skip phase 2
    if [ "$EDITS_THIS_TURN" -lt 3 ] 2>/dev/null; then
        jq '.validation_phase = 2' "$STATE_FILE" > "${STATE_FILE}.tmp" 2>/dev/null && mv "${STATE_FILE}.tmp" "$STATE_FILE" 2>/dev/null || true
        exit 0
    fi

    RISK_LEVEL=$(jq -r '.risk_level // "medium"' "$STATE_FILE" 2>/dev/null) || RISK_LEVEL="medium"

    # Update phase
    jq '.validation_phase = 2' "$STATE_FILE" > "${STATE_FILE}.tmp" 2>/dev/null && mv "${STATE_FILE}.tmp" "$STATE_FILE" 2>/dev/null || true

    case "$RISK_LEVEL" in
        low)
            # Low risk — no deeper validation needed
            exit 0
            ;;
        medium)
            cat >&2 <<'EOF'
Before stopping, run scope-appropriate validation:

1. Run relevant project tests (if they cover the changed functionality)
2. Dispatch an architect agent to review the changes for encapsulation, dependency direction, and regression risk

After validation passes, stop again.
EOF
            exit 2
            ;;
        high)
            VALIDATION_INSTRUCTIONS="Before stopping, run full validation for this high-risk change:"
            VALIDATION_INSTRUCTIONS="${VALIDATION_INSTRUCTIONS}

1. Run ALL relevant project tests"

            if [ "$CURRENT_APPROACH" = "solo" ]; then
                VALIDATION_INSTRUCTIONS="${VALIDATION_INSTRUCTIONS}
2. Review the changes yourself for encapsulation, dependency direction, and regression risk
3. Trace every user flow affected by the changes
4. Verify no data loss, no security regression, no breaking changes"
            else
                VALIDATION_INSTRUCTIONS="${VALIDATION_INSTRUCTIONS}
2. Dispatch an architect agent to review changes
3. Dispatch a tester agent to trace affected user flows
4. If UI changes are involved, dispatch a ux-tester agent
5. Verify no data loss, no security regression, no breaking changes"
            fi

            VALIDATION_INSTRUCTIONS="${VALIDATION_INSTRUCTIONS}

After all validation passes, stop again."

            echo "$VALIDATION_INSTRUCTIONS" >&2
            exit 2
            ;;
    esac
fi

# Fallback — allow stop
exit 0
