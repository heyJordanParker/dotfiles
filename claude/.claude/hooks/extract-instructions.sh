#!/bin/bash

# Extract instructions from user message and inject as context
# Uses a separate Claude instance to classify the message

set -euo pipefail

cleanup() {
    pids=$(jobs -p 2>/dev/null)
    if [ -n "$pids" ]; then
        kill $pids 2>/dev/null
    fi
}
trap cleanup EXIT TERM INT HUP

# Guard against recursion
if [ "${CLAUDE_EXTRACT_INSTRUCTIONS:-}" = "true" ]; then
    exit 0
fi

# Read hook event data
EVENT=$(cat)

# Skip agent sessions
SESSION_ID=$(echo "$EVENT" | jq -r '.session_id // ""')
[[ -z "$SESSION_ID" || "$SESSION_ID" == agent-* ]] && exit 0

# Extract user message and transcript path
PROMPT=$(echo "$EVENT" | jq -r '.prompt // ""')
[[ -z "$PROMPT" ]] && exit 0

TRANSCRIPT_PATH=$(echo "$EVENT" | jq -r '.transcript_path // ""')

# Extract conversation context from transcript
CONVERSATION_CONTEXT=""
if [ -n "$TRANSCRIPT_PATH" ] && [ -f "$TRANSCRIPT_PATH" ]; then
    # Last 10 user messages with timestamps
    USER_MESSAGES=$(jq -r 'select(.type == "human") | "\(.timestamp // "?"): \(.message.content // "" | if type == "array" then map(select(.type == "text") | .text) | join(" ") else . end)"' "$TRANSCRIPT_PATH" 2>/dev/null | tail -n 10 | head -c 8000)

    # Last agent response with timestamp
    AGENT_RESPONSE=$(jq -r 'select(.type == "assistant") | "\(.timestamp // "?"): \(.message.content // "" | if type == "array" then map(select(.type == "text") | .text) | join(" ") else . end)"' "$TRANSCRIPT_PATH" 2>/dev/null | tail -n 1 | head -c 4000)

    if [ -n "$USER_MESSAGES" ] || [ -n "$AGENT_RESPONSE" ]; then
        CONVERSATION_CONTEXT="Recent conversation context:

Recent user messages:
${USER_MESSAGES}

Last agent response:
${AGENT_RESPONSE}

---
"
    fi
fi

JSON_SCHEMA='{"type":"object","properties":{"type":{"type":"string","enum":["question","approval","instructions"]},"instructions":{"type":"array","items":{"type":"string"}},"skills":{"type":"array","items":{"type":"string"}}},"required":["type"]}'

SYSTEM_PROMPT="You are a classifier. Extract instructions from user messages. Output structured JSON only."

EVALUATION_PROMPT="Classify this user message and extract any specific instructions, requirements, or constraints.

${CONVERSATION_CONTEXT}Current message to classify:
$PROMPT

If the message is a question with no actionable instructions, return: {\"type\": \"question\", \"instructions\": [], \"skills\": []}
If the message is a short approval (e.g. 'yes', 'ok', 'do it'), return: {\"type\": \"approval\", \"instructions\": [], \"skills\": []}
Otherwise return: {\"type\": \"instructions\", \"instructions\": [\"each specific instruction\"], \"skills\": [\"/any-detected-skills\"]}

Detect /skill references (slash followed by a name, e.g. /commit, /review, /ask) and list them in skills."

CLAUDE_RESPONSE=$(CLAUDE_EXTRACT_INSTRUCTIONS=true timeout 30 claude -p \
    --model opus \
    --effort low \
    --output-format json \
    --json-schema "$JSON_SCHEMA" \
    --system-prompt "$SYSTEM_PROMPT" \
    --setting-sources "" \
    --disallowedTools '*' 2>/dev/null <<< "$EVALUATION_PROMPT")
EXIT_CODE=$?

if [ $EXIT_CODE -ne 0 ]; then
    exit 0
fi

RESULT=$(echo "$CLAUDE_RESPONSE" | jq '.structured_output // empty' 2>/dev/null)
if [ -z "$RESULT" ] || [ "$RESULT" = "null" ]; then
    exit 0
fi

MSG_TYPE=$(echo "$RESULT" | jq -r '.type // "instructions"')
INSTRUCTIONS=$(echo "$RESULT" | jq -r '.instructions // [] | to_entries | map("\(.key + 1). \(.value)") | join("\n")' 2>/dev/null)
SKILLS=$(echo "$RESULT" | jq -r '.skills // [] | join(", ")' 2>/dev/null)

case "$MSG_TYPE" in
    question)
        CONTEXT="This is a question. Answer it, do not act on it.\n\nOpen your response with a conversational restatement of the question — in your own words, proving you understood. Follow the restatement examples in Claude.md. Do not take any action before restating."
        ;;
    approval)
        CONTEXT="Approval. Start work on what was just discussed.\n\nOpen your response with a conversational restatement of what was discussed — in your own words, proving you understood. Follow the restatement examples in Claude.md. Do not take any action before restating."
        ;;
    *)
        CONTEXT="Instructions from user:\n${INSTRUCTIONS}"
        if [ -n "$SKILLS" ] && [ "$SKILLS" != "" ]; then
            CONTEXT="${CONTEXT}\n\nSkills to execute: ${SKILLS}"
        fi
        CONTEXT="${CONTEXT}\n\nOpen your response with a conversational restatement of these instructions — in your own words, proving you understood. Follow the restatement examples in Claude.md. Do not take any action before restating. Execute detected /skills immediately after restating."
        ;;
esac

ESCAPED_CONTEXT=$(echo -e "$CONTEXT" | jq -Rs .)

printf '{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":%s}}\n' "$ESCAPED_CONTEXT"

exit 0
