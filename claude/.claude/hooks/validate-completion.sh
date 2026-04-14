#!/bin/bash

# Validate agent work before allowing stop
# Two-layer gate: deterministic combo check → LLM validation
# Uses session state for phase tracking, transcript for context extraction
#
# Layer 1 (deterministic): ExitPlanMode + permission-seeking phrase = always block
# Layer 2 (LLM): fires when phrase detected OR 3+ file mutations
# Gracefully allows on any error

set -uo pipefail
# NOTE: no set -e — graceful allow on any failure

export CLAUDE_SESSION_HOOK=true

# Read hook event data
EVENT=$(cat)

# Skip agent sessions
SESSION_ID=$(echo "$EVENT" | jq -r '.session_id // ""' 2>/dev/null) || exit 0
[[ -z "$SESSION_ID" || "$SESSION_ID" == agent-* ]] && exit 0

TRANSCRIPT_PATH=$(echo "$EVENT" | jq -r '.transcript_path // ""' 2>/dev/null) || exit 0
LAST_ASSISTANT_MSG=$(echo "$EVENT" | jq -r '.last_assistant_message // ""' 2>/dev/null) || LAST_ASSISTANT_MSG=""

STATE_FILE="/tmp/claude-session-state-${SESSION_ID}"

# Read session state (graceful if missing)
CURRENT_STATE="executing"
CURRENT_INTENT="instructions"
COMMIT_REQUESTED="false"
VALIDATION_PHASE=0
if [ -f "$STATE_FILE" ]; then
    CURRENT_STATE=$(jq -r '.state // "executing"' "$STATE_FILE" 2>/dev/null) || CURRENT_STATE="executing"
    CURRENT_INTENT=$(jq -r '.intent // "instructions"' "$STATE_FILE" 2>/dev/null) || CURRENT_INTENT="instructions"
    COMMIT_REQUESTED=$(jq -r '.commit_requested // false' "$STATE_FILE" 2>/dev/null) || COMMIT_REQUESTED="false"
    VALIDATION_PHASE=$(jq -r '.validation_phase // 0' "$STATE_FILE" 2>/dev/null) || VALIDATION_PHASE=0
fi

# Infinite loop breaker — allow stop after 3 blocks
if [ "$VALIDATION_PHASE" -ge 3 ] 2>/dev/null; then
    exit 0
fi

# Helper: increment validation_phase in session state
increment_phase() {
    if [ -f "$STATE_FILE" ]; then
        jq '.validation_phase = (.validation_phase // 0) + 1' "$STATE_FILE" > "${STATE_FILE}.tmp" 2>/dev/null && \
            mv "${STATE_FILE}.tmp" "$STATE_FILE" 2>/dev/null || true
    fi
}

# =========================================================================
# Detection: permission-seeking phrases in last message
# =========================================================================

LAST_MSG_LOWER=$(echo "$LAST_ASSISTANT_MSG" | tr '[:upper:]' '[:lower:]')
HAS_PHRASE=false
for phrase in \
    "shall i proceed" \
    "shall i continue" \
    "want me to continue" \
    "let me continue in the next message" \
    "should i move on" \
    "ready to proceed" \
    "can i proceed" \
    "ready to move" \
    "want me to go ahead" \
    "should i proceed"; do
    if echo "$LAST_MSG_LOWER" | grep -qF "$phrase"; then
        HAS_PHRASE=true
        break
    fi
done

# =========================================================================
# Layer 1: Fast combo check — ExitPlanMode in current turn + phrase
# =========================================================================

HAS_RECENT_PLAN_EXIT=false
if [ -n "$TRANSCRIPT_PATH" ] && [ -f "$TRANSCRIPT_PATH" ]; then
    # Only check current turn (after last real user message, not entire transcript)
    CURRENT_TURN=$(tac "$TRANSCRIPT_PATH" 2>/dev/null | awk '/"type":"user"/ && !/tool_use_id/ {exit} {print}' 2>/dev/null) || CURRENT_TURN=""
    if echo "$CURRENT_TURN" | grep -q "ExitPlanMode" 2>/dev/null; then
        HAS_RECENT_PLAN_EXIT=true
    fi
fi

if [ "$HAS_PHRASE" = true ] && [ "$HAS_RECENT_PLAN_EXIT" = true ]; then
    increment_phase
    echo "The plan is approved. Continue executing — do not ask permission to start." >&2
    exit 2
fi

# =========================================================================
# Layer 2: LLM trigger gate — phrase match OR mutations >= 3
# =========================================================================

# Count mutations in current turn only (not entire session)
# Reuses CURRENT_TURN from the ExitPlanMode check above
MUTATIONS=0
if [ -n "$CURRENT_TURN" ]; then
    MUTATIONS=$(echo "$CURRENT_TURN" | grep -c '"name":"Edit"\|"name":"Write"\|"name":"MultiEdit"' 2>/dev/null) || MUTATIONS=0
fi

# No phrase AND low mutations → allow stop
if [ "$HAS_PHRASE" = false ] && [ "$MUTATIONS" -lt 3 ] 2>/dev/null; then
    exit 0
fi

# Commit requested → allow stop (wrapping up)
if [ "$COMMIT_REQUESTED" = "true" ]; then
    exit 0
fi

# =========================================================================
# Layer 2: LLM validation
# =========================================================================

# Extract plan content from transcript
PLAN_CONTENT=""
if [ -n "$TRANSCRIPT_PATH" ] && [ -f "$TRANSCRIPT_PATH" ]; then
    PLAN_CONTENT=$(grep "ExitPlanMode" "$TRANSCRIPT_PATH" 2>/dev/null \
        | jq -r 'select(.type == "assistant") | .message.content[]? | select(.type == "tool_use" and .name == "ExitPlanMode") | .input.plan' 2>/dev/null \
        | tail -1 | head -c 6000) || true

    # Fallback: plan file on disk
    if [ -z "$PLAN_CONTENT" ]; then
        SLUG=$(grep -m1 '"slug"' "$TRANSCRIPT_PATH" 2>/dev/null | jq -r '.slug // ""' 2>/dev/null) || SLUG=""
        if [ -n "$SLUG" ] && [ -f "$HOME/.claude/plans/${SLUG}.md" ]; then
            PLAN_CONTENT=$(head -c 6000 "$HOME/.claude/plans/${SLUG}.md" 2>/dev/null) || true
        fi
    fi
fi

# Extract recent user messages for context
RECENT_USER_MSGS=""
if [ -n "$TRANSCRIPT_PATH" ] && [ -f "$TRANSCRIPT_PATH" ]; then
    RECENT_USER_MSGS=$(jq -r 'select(.type == "user" and (.message.content | type != "array" or any(.[]; .type == "text" and .text != ""))) | .message.content // "" | if type == "array" then map(select(.type == "text") | .text) | join(" ") else . end' "$TRANSCRIPT_PATH" 2>/dev/null | tail -n 4 | head -c 4000) || true
fi

# Build LLM context
PLAN_CONTEXT=""
if [ -n "$PLAN_CONTENT" ]; then
    PLAN_CONTEXT="Plan for this session:
${PLAN_CONTENT}
---
"
fi

SESSION_CONTEXT="Session state: state=${CURRENT_STATE}, intent=${CURRENT_INTENT}, commit_requested=${COMMIT_REQUESTED}
ExitPlanMode in current turn: ${HAS_RECENT_PLAN_EXIT}
Permission-seeking phrase detected: ${HAS_PHRASE}
File mutations this session: ${MUTATIONS}
---
"

JSON_SCHEMA='{"type":"object","properties":{"allow":{"type":"boolean"},"reason":{"type":"string"},"instruction":{"type":"string"}},"required":["allow"]}'

SYSTEM_PROMPT="You are a completion validator. Decide whether the agent should be allowed to stop or forced to continue. Output structured JSON only."

EVAL_PROMPT="Evaluate whether this agent should be allowed to stop.

${PLAN_CONTEXT}${SESSION_CONTEXT}Recent user messages:
${RECENT_USER_MSGS}
---
Agent's last message before stopping:
${LAST_ASSISTANT_MSG}
---

BLOCK the stop if ANY of these are true:

1. PREMATURE STOP — the agent asks permission to continue work that is already approved. Signals: \"shall I proceed?\", \"want me to continue?\", \"ready to move?\", \"where should I start?\" after a plan was approved or instructions were given. The agent should execute, not ask.
   Exception: genuine architectural escalation (destructive operation, credential needed, scope-changing decision with real tradeoffs). If the question has only one reasonable answer, it's not a genuine escalation — it's hand-holding.

2. DEFERRAL OF IN-SCOPE WORK — the agent defers work that was part of the original task. Signals: \"as a follow-up\", \"in a future PR\", \"separate concern\", \"TODO\", \"out of scope\" for work that IS in scope based on the plan or user's instructions.
   Exception: work genuinely unrelated to the current task, OR actions the agent physically cannot perform (DNS changes, dashboard access, server SSH, credential rotation, starting services in a different environment). \"You'll need to\" is legitimate ONLY when the agent has no way to do it itself.

3. INCOMPLETE WORK — the agent completed some plan steps but not all. Check the plan (if provided) against what the agent claims to have done.
   Exception: the agent explicitly identifies remaining items AND explains why it stopped (genuine blocker, not \"this is a good stopping point\").

4. CONTEXT PRESSURE EXCUSE — the agent stops citing context window, message length, or \"manageable\" context as the reason, mid-task. The agent should continue executing until the work is done or a genuine blocker is hit.

ALLOW the stop if ANY of these are true:
- Work is genuinely complete — all plan items done, no deferred work
- Agent is asking a genuine architectural question with multiple viable options and real tradeoffs
- Agent is asking about a destructive operation (git force-push, DB drop, file deletion)
- Agent needs credentials, API keys, or access it doesn't have
- Agent is presenting analysis/options that were the requested deliverable (user asked for research, agent delivered research)
- Agent correctly scoped out genuinely unrelated work found during implementation
- User's last message was a question — agent answered it and stopped

Return JSON:
- Allow: {\"allow\": true}
- Block: {\"allow\": false, \"reason\": \"specific issue found\", \"instruction\": \"what the agent should do next\"}"

# Run LLM validator
RESULT=""
if CLAUDE_RESPONSE=$(CLAUDE_SESSION_HOOK=true timeout 60 claude -p \
    --model opus \
    --effort low \
    --output-format json \
    --json-schema "$JSON_SCHEMA" \
    --system-prompt "$SYSTEM_PROMPT" \
    --setting-sources "" \
    --disallowedTools '*' 2>/dev/null <<< "$EVAL_PROMPT"); then
    RESULT=$(echo "$CLAUDE_RESPONSE" | jq '.structured_output // empty' 2>/dev/null) || true
fi

# If LLM failed, allow stop (graceful degradation)
if [ -z "$RESULT" ] || [ "$RESULT" = "null" ]; then
    exit 0
fi

ALLOW=$(echo "$RESULT" | jq -r '.allow // true' 2>/dev/null) || ALLOW="true"

if [ "$ALLOW" = "false" ]; then
    REASON=$(echo "$RESULT" | jq -r '.reason // "Incomplete work detected"' 2>/dev/null) || REASON="Incomplete work detected"
    INSTRUCTION=$(echo "$RESULT" | jq -r '.instruction // "Review and complete remaining work"' 2>/dev/null) || INSTRUCTION="Review and complete remaining work"

    increment_phase

    cat >&2 <<EOF
${REASON}

${INSTRUCTION}
EOF
    exit 2
fi

exit 0
