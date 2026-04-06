#!/bin/bash

# Classify user intent and inject contextual guidance
# Uses a separate Claude instance to classify the message
# Manages session state in /tmp/claude-session-state-{session_id}

set -uo pipefail
# NOTE: no set -e — this script must NEVER exit non-zero (which blocks the user's message)

cleanup() {
    pids=$(jobs -p 2>/dev/null)
    if [ -n "$pids" ]; then
        kill $pids 2>/dev/null
    fi
}
trap cleanup EXIT TERM INT HUP

# Guard against recursion
if [ "${CLAUDE_CLASSIFY_INTENT:-}" = "true" ]; then
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

# Skip system-generated messages (structural detection)
# XML-tagged: starts with <tag>, contains matching </tag>
if [[ "$PROMPT" == "<"* ]]; then
    TAG_REST="${PROMPT#<}"
    TAG_NAME="${TAG_REST%%[> ]*}"
    [[ -n "$TAG_NAME" && "$PROMPT" == *"</${TAG_NAME}>"* ]] && exit 0
fi
# Bracket-enclosed: entire message is a single [...] line (no content after)
if [[ "$PROMPT" == "["* ]]; then
    FIRST_LINE="${PROMPT%%$'\n'*}"
    [[ "$FIRST_LINE" == *"]" && "$PROMPT" == "$FIRST_LINE" ]] && exit 0
fi
# Raw text system messages (rare, no structural signal)
[[ "$PROMPT" == "This session is being continued"* ]] && exit 0
[[ "$PROMPT" == "Base directory for this skill:"* ]] && exit 0

TRANSCRIPT_PATH=$(echo "$EVENT" | jq -r '.transcript_path // ""')
STATE_FILE="/tmp/claude-session-state-${SESSION_ID}"

# Read existing session state (graceful if missing)
CURRENT_APPROACH="default"
CURRENT_TYPE="instructions"
CURRENT_NOTES="[]"
if [ -f "$STATE_FILE" ]; then
    CURRENT_APPROACH=$(jq -r '.approach // "default"' "$STATE_FILE" 2>/dev/null) || CURRENT_APPROACH="default"
    CURRENT_TYPE=$(jq -r '.type // "instructions"' "$STATE_FILE" 2>/dev/null) || CURRENT_TYPE="instructions"
    CURRENT_NOTES=$(jq -c '.notes // []' "$STATE_FILE" 2>/dev/null) || CURRENT_NOTES="[]"
fi

# Reset validation phase on new user message (new validation cycle)
if [ -f "$STATE_FILE" ]; then
    jq '.validation_phase = 0' "$STATE_FILE" > "${STATE_FILE}.tmp" 2>/dev/null && mv "${STATE_FILE}.tmp" "$STATE_FILE" 2>/dev/null || true
fi

# Format existing notes for classifier context
SESSION_NOTES_CONTEXT=""
NOTES_COUNT=$(echo "$CURRENT_NOTES" | jq 'length' 2>/dev/null) || NOTES_COUNT=0
if [ "$NOTES_COUNT" -gt 0 ]; then
    FORMATTED_NOTES=$(echo "$CURRENT_NOTES" | jq -r 'to_entries | map("- \(.value)") | join("\n")' 2>/dev/null) || FORMATTED_NOTES=""
    if [ -n "$FORMATTED_NOTES" ]; then
        SESSION_NOTES_CONTEXT="
Active session notes (behavioral corrections from this session — these shift how you classify):
${FORMATTED_NOTES}
Current approach: ${CURRENT_APPROACH}

---
"
    fi
fi

# Extract conversation context from transcript
CONVERSATION_CONTEXT=""
if [ -n "$TRANSCRIPT_PATH" ] && [ -f "$TRANSCRIPT_PATH" ]; then
    # Last 10 user messages with timestamps
    USER_MESSAGES=$(jq -r 'select(.type == "human") | "\(.timestamp // "?"): \(.message.content // "" | if type == "array" then map(select(.type == "text") | .text) | join(" ") else . end)"' "$TRANSCRIPT_PATH" 2>/dev/null | tail -n 10) || true

    # Last agent response with timestamp
    AGENT_RESPONSE=$(jq -r 'select(.type == "assistant") | "\(.timestamp // "?"): \(.message.content // "" | if type == "array" then map(select(.type == "text") | .text) | join(" ") else . end)"' "$TRANSCRIPT_PATH" 2>/dev/null | tail -n 1) || true

    if [ -n "$USER_MESSAGES" ] || [ -n "$AGENT_RESPONSE" ]; then
        CONVERSATION_CONTEXT="Recent user messages (for context on the user's direction and intent):
${USER_MESSAGES}

---
Last agent response (what the user is directly responding to):
${AGENT_RESPONSE}

---
"
    fi
fi

JSON_SCHEMA='{"type":"object","properties":{"type":{"type":"string","enum":["question","approval","instructions"]},"instructions":{"type":"array","items":{"type":"object","properties":{"text":{"type":"string"},"mode":{"type":"string","enum":["question","execute","correction"]}},"required":["text","mode"]}},"skills":{"type":"array","items":{"type":"string"}},"proposal_expected":{"type":"boolean"},"approach":{"type":"string","enum":["solo","default","team"]},"sequential":{"type":"boolean"},"finalize":{"type":"boolean"},"session_notes":{"type":"array","items":{"type":"string"}}},"required":["type"]}'

SYSTEM_PROMPT="You are a classifier. Extract instructions from user messages. Manage session state. Output structured JSON only."

EVALUATION_PROMPT="Classify this user message and extract any specific instructions, requirements, or constraints.

${SESSION_NOTES_CONTEXT}${CONVERSATION_CONTEXT}Current message to classify:
$PROMPT

Rules:
1. A message that makes sense as a response to the last AI output IS a response to it — classify based on what it's responding to, not its surface form. A single word after an options list is a selection (approval or instructions). A short reply after a proposal is feedback (instructions).
2. Emotional language is emphasis, not a separate category. Strip the emotion, classify the underlying intent.
3. When uncertain, classify as \"instructions\" with \"proposal_expected\": true — the default mode of operation is proposal, not execution. Only set \"proposal_expected\" to false when the user's intent to execute is clear from context.
4. Set \"proposal_expected\" to true when the conversation history shows a proposal is in progress or expected (user asked for options, prior message was a proposal, user is giving feedback on a proposal), OR when the user's intent is ambiguous. Default to true.
5. Set \"sequential\" to true when the user's instructions have explicit ordering (\"after that\", \"then\", \"finally\", numbered steps with dependencies). Default to false.
6. All user instructions contain subtleties and nuance — preserve ordering language, execution context, autonomy cues, and boundary conditions verbatim. Never flatten, summarize, or strip nuance from extracted instructions. Each instruction is an object with \"text\" (the instruction) and \"mode\" (one of: \"execute\" for approved actions, \"question\" for things the user wants answered, \"correction\" for feedback on previous analysis that doesn't require new action).
7. Only include a skill in \"skills\" if the user is invoking it — telling the agent to use or execute it NOW. If the skill is discussed, referenced, or mentioned as context, do not include it. \"use /commit\" → include. \"the /subagents skill handles this\" → exclude. When in doubt, include it — a false positive is cheaper than a false negative.
8. Set \"approach\" based on user signals. \"solo\" when user wants the agent to work alone (\"do this yourself\", \"don't spawn agents\", \"read it yourself\"). \"team\" when user wants persistent teammates (\"get a team\", \"dispatch agents\", uses /team). \"default\" when neither signal is present. Only change from the current approach (${CURRENT_APPROACH}) on clear intent shift.
9. Set \"finalize\" to true when the user is asking for a commit, to wrap up, ship it, or finalize work. Otherwise false.
10. Add to \"session_notes\" ONLY when this message reveals a surprise — the agent did something illogical that confused the user, the user explicitly forbids something with always/never language, the user corrects the same behavior twice, or the user's emotional state (frustration, anger, exasperation) indicates the agent surprised them. Maximum 10 notes total (including existing). If adding would exceed 10, drop the least critical existing note. Return the FULL list of notes (existing + new) in session_notes, or an empty array if no changes.
11. Corrections to previous analysis (\"no, X is actually Y\", \"that's not the issue\", \"this is a non-issue\", \"this is fine\") maintain the current type (${CURRENT_TYPE}). The user is providing feedback within the current mode, not changing direction. Return the previous type unchanged.

If the message is a question with no actionable instructions, return: {\"type\": \"question\", \"instructions\": [], \"skills\": [], \"proposal_expected\": false, \"approach\": \"${CURRENT_APPROACH}\", \"sequential\": false, \"finalize\": false, \"session_notes\": []}
If the message is a short approval (e.g. 'yes', 'ok', 'do it'), return: {\"type\": \"approval\", \"instructions\": [], \"skills\": [], \"proposal_expected\": false, \"approach\": \"${CURRENT_APPROACH}\", \"sequential\": false, \"finalize\": false, \"session_notes\": []}
Otherwise return: {\"type\": \"instructions\", \"instructions\": [{\"text\": \"instruction text\", \"mode\": \"execute|question|correction\"}], \"skills\": [\"/detected-skills-being-invoked\"], \"proposal_expected\": true/false, \"approach\": \"solo/default/team\", \"sequential\": true/false, \"finalize\": true/false, \"session_notes\": [\"full updated list if changes, empty if no changes\"]}

Detect /skill references (slash followed by a name, e.g. /commit, /review, /ask) — only include if being invoked, not discussed."

# Run classifier — if ANYTHING fails, pass through silently
RESULT=""
if CLAUDE_RESPONSE=$(CLAUDE_CLASSIFY_INTENT=true timeout 30 claude -p \
    --model opus \
    --effort low \
    --output-format json \
    --json-schema "$JSON_SCHEMA" \
    --system-prompt "$SYSTEM_PROMPT" \
    --setting-sources "" \
    --disallowedTools '*' 2>/dev/null <<< "$EVALUATION_PROMPT"); then
    RESULT=$(echo "$CLAUDE_RESPONSE" | jq '.structured_output // empty' 2>/dev/null) || true
fi

# If classification failed for any reason, pass through with no context
if [ -z "$RESULT" ] || [ "$RESULT" = "null" ]; then
    exit 0
fi

MSG_TYPE=$(echo "$RESULT" | jq -r '.type // "instructions"') || MSG_TYPE="instructions"

# Extract instructions grouped by mode
EXECUTE_INSTRUCTIONS=$(echo "$RESULT" | jq -r '[.instructions // [] | .[] | select(.mode == "execute")] | to_entries | map("\(.key + 1). \(.value.text)") | join("\n")' 2>/dev/null) || EXECUTE_INSTRUCTIONS=""
QUESTION_INSTRUCTIONS=$(echo "$RESULT" | jq -r '[.instructions // [] | .[] | select(.mode == "question")] | to_entries | map("\(.key + 1). \(.value.text)") | join("\n")' 2>/dev/null) || QUESTION_INSTRUCTIONS=""
CORRECTION_INSTRUCTIONS=$(echo "$RESULT" | jq -r '[.instructions // [] | .[] | select(.mode == "correction")] | to_entries | map("\(.key + 1). \(.value.text)") | join("\n")' 2>/dev/null) || CORRECTION_INSTRUCTIONS=""

# All instructions flattened (for backwards compat)
INSTRUCTIONS=$(echo "$RESULT" | jq -r '.instructions // [] | to_entries | map("\(.key + 1). \(.value.text // .value)") | join("\n")' 2>/dev/null) || INSTRUCTIONS=""

# Compute dominant mode: count per mode, most items wins, tie-break to more restrictive
EXECUTE_COUNT=$(echo "$RESULT" | jq '[.instructions // [] | .[] | select(.mode == "execute")] | length' 2>/dev/null) || EXECUTE_COUNT=0
QUESTION_COUNT=$(echo "$RESULT" | jq '[.instructions // [] | .[] | select(.mode == "question")] | length' 2>/dev/null) || QUESTION_COUNT=0
CORRECTION_COUNT=$(echo "$RESULT" | jq '[.instructions // [] | .[] | select(.mode == "correction")] | length' 2>/dev/null) || CORRECTION_COUNT=0

# Dominant mode: most items wins. Tie-break rounds to more restrictive (question > correction > execute)
DOMINANT_MODE="instructions"
MAX_COUNT=$EXECUTE_COUNT
if [ "$QUESTION_COUNT" -ge "$MAX_COUNT" ] 2>/dev/null; then
    DOMINANT_MODE="question"
    MAX_COUNT=$QUESTION_COUNT
fi
if [ "$CORRECTION_COUNT" -gt "$MAX_COUNT" ] 2>/dev/null; then
    DOMINANT_MODE="correction"
fi
# Corrections maintain previous type
if [ "$DOMINANT_MODE" = "correction" ]; then
    MSG_TYPE="$CURRENT_TYPE"
elif [ "$DOMINANT_MODE" = "question" ]; then
    MSG_TYPE="question"
fi

SKILLS=$(echo "$RESULT" | jq -r '.skills // [] | join(", ")' 2>/dev/null) || SKILLS=""
PROPOSAL_EXPECTED=$(echo "$RESULT" | jq -r '.proposal_expected // true' 2>/dev/null) || PROPOSAL_EXPECTED="true"
APPROACH=$(echo "$RESULT" | jq -r '.approach // "default"' 2>/dev/null) || APPROACH="default"
SEQUENTIAL=$(echo "$RESULT" | jq -r '.sequential // false' 2>/dev/null) || SEQUENTIAL="false"
FINALIZE=$(echo "$RESULT" | jq -r '.finalize // false' 2>/dev/null) || FINALIZE="false"
NEW_NOTES=$(echo "$RESULT" | jq -c '.session_notes // []' 2>/dev/null) || NEW_NOTES="[]"
NEW_NOTES_COUNT=$(echo "$NEW_NOTES" | jq 'length' 2>/dev/null) || NEW_NOTES_COUNT=0

# Update session state file
SAVE_NOTES="$CURRENT_NOTES"
if [ "$NEW_NOTES_COUNT" -gt 0 ]; then
    SAVE_NOTES="$NEW_NOTES"
fi
jq -n \
    --arg approach "$APPROACH" \
    --arg type "$MSG_TYPE" \
    --argjson finalize "$FINALIZE" \
    --argjson notes "$SAVE_NOTES" \
    '{approach: $approach, type: $type, finalize: $finalize, notes: $notes, validation_phase: 0}' \
    > "$STATE_FILE" 2>/dev/null || true

# Build context based on message type
# Mixed messages (multiple modes) always use grouped format
HAS_MIXED_MODES=false
MODE_COUNT=0
[ -n "$EXECUTE_INSTRUCTIONS" ] && MODE_COUNT=$((MODE_COUNT + 1))
[ -n "$QUESTION_INSTRUCTIONS" ] && MODE_COUNT=$((MODE_COUNT + 1))
[ -n "$CORRECTION_INSTRUCTIONS" ] && MODE_COUNT=$((MODE_COUNT + 1))
[ "$MODE_COUNT" -gt 1 ] && HAS_MIXED_MODES=true

if [ "$HAS_MIXED_MODES" = true ] || [ "$MSG_TYPE" = "instructions" ]; then
    # Grouped format for mixed messages and pure instructions
    CONTEXT=""
    if [ -n "$EXECUTE_INSTRUCTIONS" ]; then
        CONTEXT="Instructions from user:\n${EXECUTE_INSTRUCTIONS}"
    fi
    if [ -n "$QUESTION_INSTRUCTIONS" ]; then
        [ -n "$CONTEXT" ] && CONTEXT="${CONTEXT}\n\n"
        CONTEXT="${CONTEXT}Questions from user (answer these, do not act on them):\n${QUESTION_INSTRUCTIONS}"
    fi
    if [ -n "$CORRECTION_INSTRUCTIONS" ]; then
        [ -n "$CONTEXT" ] && CONTEXT="${CONTEXT}\n\n"
        CONTEXT="${CONTEXT}Corrections from user (acknowledge, do not change direction):\n${CORRECTION_INSTRUCTIONS}"
    fi
    # Fallback if no grouped instructions extracted
    if [ -z "$CONTEXT" ]; then
        CONTEXT="Instructions from user:\n${INSTRUCTIONS}"
    fi
    if [ -n "$SKILLS" ] && [ "$SKILLS" != "" ]; then
        CONTEXT="${CONTEXT}\n\nSkills to execute: ${SKILLS}"
    fi
    CONTEXT="${CONTEXT}\n\nOpen your response with a conversational restatement of these instructions — in your own words, proving you understood. Follow the restatement examples in Claude.md. Do not take any action before restating. Execute detected /skills immediately after restating."
elif [ "$MSG_TYPE" = "question" ]; then
    CONTEXT="This is a question. Answer it, do not act on it.\n\nOpen your response with a conversational restatement of the question — in your own words, proving you understood. Follow the restatement examples in Claude.md. Do not take any action before restating.\n\nWhen presenting options or answering questions, use /pcc skill: architecturally distinct options, each with pros, cons, and confidence percentage. For yes/no questions, present the case for both sides. No hedging — state confidence as a percentage."
elif [ "$MSG_TYPE" = "approval" ]; then
    CONTEXT="Approval. Start work on what was just discussed.\n\nOpen your response with a conversational restatement of what was discussed — in your own words, proving you understood. Follow the restatement examples in Claude.md. Do not take any action before restating."
fi

# Sequential execution context
if [ "$SEQUENTIAL" = "true" ]; then
    CONTEXT="${CONTEXT}\n\nThese steps are strictly sequential — launch each only after the previous completes. Do not parallelize."
fi

# Proposal context
if [ "$PROPOSAL_EXPECTED" = "true" ]; then
    CONTEXT="${CONTEXT}\n\nPresent a full, complete proposal before executing anything — do not make the user piece together context from prior messages.\n\nWhen presenting options in proposals, use /pcc skill: architecturally distinct options, each with pros, cons, and confidence percentage. Explore both sides of every tradeoff.\n\nBefore proposing, identify every element you're uncertain about and research each one — read full files, not grep fragments. Never propose from general knowledge when the code can answer definitively.\n\nIn proposals, never:\n- Hedge (\"may\", \"probably\", \"likely\", \"might\") — if you'd hedge, you haven't read enough code yet\n- Echo requirements back as proposals — include concrete HOW (mechanisms, code paths, data flow), not reworded WHAT\n- Present options below 80% confidence — low confidence means research more, not label and ship"
fi

# Finalize context
if [ "$FINALIZE" = "true" ]; then
    CONTEXT="${CONTEXT}\n\nFinalize authorized. After completing the commit/finalize as instructed, review session notes and suggest which should become permanent — in global/project Claude.md, skills, agents, rules, or commands as appropriate. Present suggestions only, do not act on them."
fi

# Standing rules (always injected for instructions)
if [ "$MSG_TYPE" = "instructions" ]; then
    CONTEXT="${CONTEXT}\n\nAny architectural changes to any plan are a hard blocker — require user approval before proceeding.\n\nBefore acting, verify you have sufficient context — read relevant files, check existing patterns, and research unknowns. Do not rush to implement."
fi

ESCAPED_CONTEXT=$(echo -e "$CONTEXT" | jq -Rs .) || { exit 0; }

printf '{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":%s}}\n' "$ESCAPED_CONTEXT"

exit 0
