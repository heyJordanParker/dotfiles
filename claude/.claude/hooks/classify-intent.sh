#!/bin/bash

# Classify user intent and inject contextual guidance
# Uses a separate Claude instance to classify the message
# Manages session state in /tmp/claude-session-state-{session_id}
#
# State machine: LLM classifies INTENT, bash transitions STATE deterministically
# Intent: approval | question | instructions | correction | proposal_request
# State: proposing | executing | auto (mixed intents — no edit restrictions)
# Approach: solo | subagents | team (LLM outputs mutations, bash applies)

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
if [ "${CLAUDE_SESSION_HOOK:-}" = "true" ]; then
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
CURRENT_APPROACH="subagents"
CURRENT_STATE="proposing"
CURRENT_INTENT="instructions"
CURRENT_NOTES="[]"
if [ -f "$STATE_FILE" ]; then
    CURRENT_APPROACH=$(jq -r '.approach // "subagents"' "$STATE_FILE" 2>/dev/null) || CURRENT_APPROACH="subagents"
    CURRENT_STATE=$(jq -r '.state // "proposing"' "$STATE_FILE" 2>/dev/null) || CURRENT_STATE="proposing"
    CURRENT_INTENT=$(jq -r '.intent // "instructions"' "$STATE_FILE" 2>/dev/null) || CURRENT_INTENT="instructions"
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
Current state: ${CURRENT_STATE}

---
"
    fi
fi

# Extract conversation context from transcript
CONVERSATION_CONTEXT=""
if [ -n "$TRANSCRIPT_PATH" ] && [ -f "$TRANSCRIPT_PATH" ]; then
    RECENT_TURNS=""
    AGENT_RESPONSE=""

    # Build conversation stream: human user messages (U|) and agent text blocks (A|)
    # Filters system noise: skill expansions (<...), task notifications ([...), command outputs, continuations
    CONV_STREAM=$(jq -r '
        if .type == "user" and (.message.content | type == "string")
           and (.message.content | (startswith("<") or startswith("[") or startswith("Base directory") or startswith("This session is being continued")) | not)
        then "U|\(.message.content | gsub("\n"; " ") | .[:200])"
        elif .type == "assistant" and (.message.content | type == "array") and (.message.content | any(.[]; .type == "text"))
        then "A|\(.message.content | map(select(.type == "text") | .text) | join(" ") | gsub("\n"; " ") | .[:300])"
        else empty
        end
    ' "$TRANSCRIPT_PATH" 2>/dev/null) || true

    if [ -n "$CONV_STREAM" ]; then
        # Find turn boundary: line number of the last human user message
        # grep -a forces text mode (transcripts can contain non-ASCII content)
        TURN_LINE=$(echo "$CONV_STREAM" | grep -an '^U|' | tail -n 1 | cut -d: -f1)

        if [ -n "$TURN_LINE" ] && [ "$TURN_LINE" -gt 0 ] 2>/dev/null; then
            # Agent's last response: text blocks after turn boundary, up to 5, bounded by turn
            AGENT_RESPONSE=$(echo "$CONV_STREAM" | tail -n +"$((TURN_LINE + 1))" | grep -a '^A|' | sed 's/^A|//' | tail -n 5) || true

            # Recent conversation: interleaved turns before current turn, last 8 entries
            if [ "$TURN_LINE" -gt 1 ]; then
                RECENT_TURNS=$(echo "$CONV_STREAM" | head -n "$((TURN_LINE - 1))" | tail -n 8 | sed 's/^U|/[User] /; s/^A|/[Agent] /') || true
            fi
        else
            # No turn boundary found — fall back to last 5 agent text blocks
            AGENT_RESPONSE=$(echo "$CONV_STREAM" | grep -a '^A|' | sed 's/^A|//' | tail -n 5) || true
        fi
    fi

    if [ -n "$RECENT_TURNS" ] || [ -n "$AGENT_RESPONSE" ]; then
        CONTEXT_PARTS=""
        if [ -n "$RECENT_TURNS" ]; then
            CONTEXT_PARTS="Recent conversation (background — do NOT extract instructions from this section):
${RECENT_TURNS}

---
"
        fi
        if [ -n "$AGENT_RESPONSE" ]; then
            CONTEXT_PARTS="${CONTEXT_PARTS}Agent's last response (what the user is directly responding to):
${AGENT_RESPONSE}

---
"
        fi
        CONVERSATION_CONTEXT="$CONTEXT_PARTS"
    fi
fi

JSON_SCHEMA='{"type":"object","properties":{"intent":{"type":"string","enum":["approval","question","instructions","correction","proposal_request"]},"instructions":{"type":"array","items":{"type":"object","properties":{"text":{"type":"string"},"mode":{"type":"string","enum":["question","execute","correction"]}},"required":["text","mode"]}},"skills":{"type":"array","items":{"type":"string"}},"approach_change":{"type":"string","enum":["solo","subagents","team","no_change"]},"sequential":{"type":"boolean"},"commit_requested":{"type":"boolean"},"session_notes":{"type":"array","items":{"type":"string"}},"recommended_agents":{"type":"array","items":{"type":"object","properties":{"agent":{"type":"string"},"reason":{"type":"string"}},"required":["agent","reason"]}}},"required":["intent"]}'

SYSTEM_PROMPT="You are a classifier. Classify user intent. Extract instructions. Output structured JSON only."

EVALUATION_PROMPT="Classify this user message's INTENT and extract any specific instructions, requirements, or constraints.

${SESSION_NOTES_CONTEXT}${CONVERSATION_CONTEXT}Current message to classify:
$PROMPT

You are classifying INTENT only. The state machine transitions are handled separately — you do not decide the agent's state.

Intent types:
- \"approval\" — user is approving/accepting a specific proposal or plan that the agent just presented. Requires a preceding proposal to approve. Includes approval with additional instructions (\"go ahead, also fix X\"). \"just fix it\" or \"do it\" as a standalone directive without a preceding proposal → instructions, not approval.
- \"question\" — user is asking a question that requires an answer. No action should be taken.
- \"instructions\" — user is giving the agent new work to do with action language (\"fix\", \"add\", \"change\", \"implement\", \"update\", \"remove\", \"refactor\"). Also standalone constraints and boundaries (\"don't use third-party libraries\") when there is no previous agent output being refined.
- \"correction\" — user is correcting or giving feedback on previous agent output (\"that's wrong\", \"no, use X instead\", \"this is fine\", \"this is a non-issue\"). Requires previous agent output being refined — a standalone constraint like \"don't use third-party libraries\" without the agent having proposed or used them is instructions (a boundary), not correction.
- \"proposal_request\" — user is asking for analysis, investigation, review, or a proposal BEFORE execution (\"propose\", \"analyze\", \"what would we need\", \"design\", \"plan\", \"evaluate\", \"compare options\", \"investigate\", \"look into\", \"dig into\", \"figure out\", \"see what's going on\", \"check why\", \"double check\", \"verify\", \"review\", \"audit\", \"validate\").

Rules:
1. A message that makes sense as a response to the last AI output IS a response to it — classify based on what it's responding to, not its surface form. A single word after an options list is a selection (approval). A short reply after a proposal is feedback (correction or approval).
2. Emotional language is emphasis, not a separate category. Strip the emotion, classify the underlying intent.
3. Corrections preserve the current direction. A message that corrects previous output while also containing forward-looking language (\"no, just use X and then deploy it\") is still a correction — the user is refining, not giving new independent instructions. Messages that add scope to an active proposal or discussion (\"one more thing:\", \"also include\", \"we also need\", \"don't forget about\") are corrections when the user is refining what should be proposed. Only classify as \"instructions\" when the message introduces genuinely new work unrelated to the previous output.
4. When uncertain between \"instructions\" and \"proposal_request\", prefer \"proposal_request\". The conservative default is to propose before executing.\n4b. When a correction also redirects to a fundamentally new direction (\"that won't work, propose something with Redis instead\", \"scrap this, try a different approach\"), classify as \"proposal_request\" — the user wants a fresh proposal, not a patched version of the rejected one.\n4c. When the user answers questions from a proposal (\"yes\", \"Option A\", \"obviously\"), classify as \"correction\" — the user is refining the proposal with their answers, not approving execution. Only classify as \"approval\" when the user explicitly signals the proposal is complete and ready for execution.
5. Set \"sequential\" to true when the user's instructions have explicit ordering (\"after that\", \"then\", \"finally\", numbered steps with dependencies). Default to false.
6. All user instructions contain subtleties and nuance — preserve ordering language, execution context, autonomy cues, and boundary conditions verbatim. Never flatten, summarize, or strip nuance from extracted instructions. Each instruction is an object with \"text\" (the instruction) and \"mode\" (one of: \"execute\" for actions, \"question\" for things the user wants answered, \"correction\" for feedback on previous analysis). When a message contains BOTH a question AND an action directive, extract both with their respective modes — never absorb an action into a question or vice versa. Example: \"how does X work? also change Y to Z\" → [{\"text\": \"how does X work\", \"mode\": \"question\"}, {\"text\": \"change Y to Z\", \"mode\": \"execute\"}].
6b. Questions in the user's message require direct answers. Extract every question as a separate mode: \"question\" instruction, even when embedded in corrections, emotional language, or rhetorical framing. \"What is this about?\" is a question. \"Do we have this elsewhere?\" is a research question. \"Did you read X?\" is a question requiring an honest answer. Do not dismiss questions as decorative or rhetorical.
7. Only include a skill in \"skills\" if the user is invoking it — telling the agent to use or execute it NOW. If the skill is discussed, referenced, or mentioned as context, do not include it. \"use /commit\" → include. \"the /subagents skill handles this\" → exclude. When in doubt, include it — a false positive is cheaper than a false negative.
8. Set \"approach_change\" to \"no_change\" unless the user explicitly signals a shift. \"solo\" when user wants the agent to work alone (\"do this yourself\", \"don't spawn agents\", \"read it yourself\"). \"subagents\" when user wants to exit solo mode or use agents without persistent teams (\"use agents\", \"exit solo\"). \"team\" when user wants persistent teammates (\"get a team\", uses /team). Only output a value other than \"no_change\" on clear intent shift — direct mode-change requests (\"enter solo mode\", \"go solo\", \"switch to team\", \"exit solo\"). Single-action directives (\"run the tests\", \"fix the bug\", \"add a guard\", \"commit this\") are NOT approach signals. Mentioning a specific agent (\"get a @debugger\") is NOT a team signal — it's a dispatch instruction within the current approach. Current approach: ${CURRENT_APPROACH}.
9. Set \"commit_requested\" to true when the user explicitly asks for a git commit — \"commit this\", \"/commit\", \"create a commit\". Approving work, applying changes, deploying, replacing files, shipping — none of these are commit requests. If the user doesn't say \"commit\", commit_requested is false.
10. Add to \"session_notes\" ONLY when this message reveals a surprise — the agent did something illogical that confused the user, the user explicitly forbids something with always/never language, the user corrects the same behavior twice, or the user's emotional state (frustration, anger, exasperation) indicates the agent surprised them. Maximum 10 notes total (including existing). If adding would exceed 10, drop the least critical existing note. Return the FULL list of notes (existing + new) in session_notes, or an empty array if no changes.
11. Set \"recommended_agents\" when the user's intent clearly matches a specialized agent. Only include agents that clearly match — empty array when no match is obvious. Multiple recommendations are fine when the task spans domains.
12. The conversation history is BACKGROUND CONTEXT for understanding what the user is responding to. Extract instructions ONLY from the current message. If the current message narrows, changes, or contradicts earlier messages, follow the current message — it represents the user's latest position. Never synthesize instructions by combining multiple older messages.

Agent routing — recommend when user intent matches:
- Claude.md, skills, hooks, plugins, context engineering, documentation → context-engineer
- Architecture, system design, encapsulation, dependency direction → architect
- Bugs, test failures, errors, stack traces, \"doesn't work\" → debugger
- Code quality, diff review, PR review → code-reviewer
- UI components, CSS, styling, layouts, visual design → designer
- Frontend features, React, user flows, UX implementation → frontend-engineer
- Backend features, API, database, services → backend-engineer
- UX testing, \"test the flow\", browser testing → ux-tester
- Feature verification, API testing, \"does this work\" → tester
- External docs, library research, \"how does X work\" → researcher

Examples:
- \"yes\" → {\"intent\": \"approval\", ...}
- \"go ahead, also fix the related issue\" → {\"intent\": \"approval\", \"instructions\": [{\"text\": \"also fix the related issue\", \"mode\": \"execute\"}], ...}
- \"why does X work this way?\" → {\"intent\": \"question\", ...}
- \"fix the bug in MediaController\" → {\"intent\": \"instructions\", \"instructions\": [{\"text\": \"fix the bug in MediaController\", \"mode\": \"execute\"}], ...}
- \"that's wrong, use Y instead\" → {\"intent\": \"correction\", \"instructions\": [{\"text\": \"use Y instead\", \"mode\": \"correction\"}], ...}
- \"propose a fix for the auth bug\" → {\"intent\": \"proposal_request\", \"instructions\": [{\"text\": \"propose a fix for the auth bug\", \"mode\": \"execute\"}], ...}
- \"analyze this and tell me what's wrong\" → {\"intent\": \"proposal_request\", \"instructions\": [{\"text\": \"analyze this and tell me what's wrong\", \"mode\": \"execute\"}], ...}
- \"investigate why the tests are failing\" → {\"intent\": \"proposal_request\", ...}
- \"double check your work\" → {\"intent\": \"proposal_request\", ...}
- \"just fix it\" → {\"intent\": \"instructions\", \"commit_requested\": false, ...}
- \"deploy this to production\" → {\"intent\": \"instructions\", \"commit_requested\": false, ...}
- \"commit this\" → {\"intent\": \"instructions\", \"commit_requested\": true, ...}
- \"how does X work? also change Y to Z\" → {\"intent\": \"question\", \"instructions\": [{\"text\": \"how does X work\", \"mode\": \"question\"}, {\"text\": \"change Y to Z\", \"mode\": \"execute\"}], ...}

Detect /skill references (slash followed by a name, e.g. /commit, /review, /ask) — only include if being invoked, not discussed. Preserve the leading slash in skill names (\"/commit\" not \"commit\"). A bare slash-command as the entire message is always an invocation."

# Run classifier — if ANYTHING fails, pass through silently
RESULT=""
if CLAUDE_RESPONSE=$(CLAUDE_SESSION_HOOK=true timeout 30 claude -p \
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

INTENT=$(echo "$RESULT" | jq -r '.intent // "instructions"') || INTENT="instructions"

# Extract instructions grouped by mode
EXECUTE_INSTRUCTIONS=$(echo "$RESULT" | jq -r '[.instructions // [] | .[] | select(.mode == "execute")] | to_entries | map("\(.key + 1). \(.value.text)") | join("\n")' 2>/dev/null) || EXECUTE_INSTRUCTIONS=""
QUESTION_INSTRUCTIONS=$(echo "$RESULT" | jq -r '[.instructions // [] | .[] | select(.mode == "question")] | to_entries | map("\(.key + 1). \(.value.text)") | join("\n")' 2>/dev/null) || QUESTION_INSTRUCTIONS=""
CORRECTION_INSTRUCTIONS=$(echo "$RESULT" | jq -r '[.instructions // [] | .[] | select(.mode == "correction")] | to_entries | map("\(.key + 1). \(.value.text)") | join("\n")' 2>/dev/null) || CORRECTION_INSTRUCTIONS=""

# All instructions flattened (for backwards compat)
INSTRUCTIONS=$(echo "$RESULT" | jq -r '.instructions // [] | to_entries | map("\(.key + 1). \(.value.text // .value)") | join("\n")' 2>/dev/null) || INSTRUCTIONS=""

SKILLS=$(echo "$RESULT" | jq -r '.skills // [] | join(", ")' 2>/dev/null) || SKILLS=""
APPROACH_CHANGE=$(echo "$RESULT" | jq -r '.approach_change // "no_change"' 2>/dev/null) || APPROACH_CHANGE="no_change"
# Apply approach mutation — LLM outputs transitions, bash applies
if [ "$APPROACH_CHANGE" != "no_change" ]; then
    APPROACH="$APPROACH_CHANGE"
else
    APPROACH="$CURRENT_APPROACH"
fi
# Auto-invoke /solo skill on approach transition to solo
if [ "$APPROACH" = "solo" ] && [ "$CURRENT_APPROACH" != "solo" ]; then
    if [[ ! "$SKILLS" == *"/solo"* ]]; then
        SKILLS="${SKILLS:+${SKILLS}, }/solo"
    fi
fi
SEQUENTIAL=$(echo "$RESULT" | jq -r 'if .sequential == null then false else .sequential end' 2>/dev/null) || SEQUENTIAL="false"
COMMIT_REQUESTED=$(echo "$RESULT" | jq -r 'if .commit_requested == null then false else .commit_requested end' 2>/dev/null) || COMMIT_REQUESTED="false"
NEW_NOTES=$(echo "$RESULT" | jq -c '.session_notes // []' 2>/dev/null) || NEW_NOTES="[]"
NEW_NOTES_COUNT=$(echo "$NEW_NOTES" | jq 'length' 2>/dev/null) || NEW_NOTES_COUNT=0
RECOMMENDED_AGENTS=$(echo "$RESULT" | jq -r '.recommended_agents // [] | map("- @\(.agent) — \(.reason)") | join("\n")' 2>/dev/null) || RECOMMENDED_AGENTS=""

# ==========================================================================
# Deterministic state transition — LLM classifies intent, bash decides state
# ==========================================================================

# Compute execute instruction count for mixed-intent detection
EXECUTE_COUNT=$(echo "$RESULT" | jq '[.instructions // [] | .[] | select(.mode == "execute")] | length' 2>/dev/null) || EXECUTE_COUNT=0

NEW_STATE="$CURRENT_STATE"
case "$INTENT" in
    proposal_request) NEW_STATE="proposing" ;;
    instructions)
        # Instructions during proposing refine the proposal — don't break out
        # Instructions during executing/auto stay executing
        if [ "$CURRENT_STATE" != "proposing" ]; then
            NEW_STATE="executing"
        fi
        ;;
    approval)         NEW_STATE="executing" ;;
    question)
        # Mixed intent: question + execute instructions → auto
        if [ "$EXECUTE_COUNT" -gt 0 ]; then
            NEW_STATE="auto"
        fi
        ;;
    correction)
        # Mixed intent: correction + execute instructions → auto
        if [ "$EXECUTE_COUNT" -gt 0 ]; then
            NEW_STATE="auto"
        fi
        ;;
esac

# Detect state transitions for agent notification
STATE_NOTIFICATIONS=""
[ "$APPROACH" != "$CURRENT_APPROACH" ] && \
    STATE_NOTIFICATIONS="${STATE_NOTIFICATIONS}Session state updated: approach changed from '${CURRENT_APPROACH}' to '${APPROACH}'.\n"
[ "$NEW_STATE" != "$CURRENT_STATE" ] && \
    STATE_NOTIFICATIONS="${STATE_NOTIFICATIONS}Session state updated: state changed from '${CURRENT_STATE}' to '${NEW_STATE}'.\n"
[ "$INTENT" != "$CURRENT_INTENT" ] && \
    STATE_NOTIFICATIONS="${STATE_NOTIFICATIONS}Session state updated: intent changed from '${CURRENT_INTENT}' to '${INTENT}'.\n"
[ "$COMMIT_REQUESTED" = "true" ] && \
    STATE_NOTIFICATIONS="${STATE_NOTIFICATIONS}Session state updated: commit authorized.\n"

# Update session state file
SAVE_NOTES="$CURRENT_NOTES"
if [ "$NEW_NOTES_COUNT" -gt 0 ]; then
    SAVE_NOTES="$NEW_NOTES"
fi
jq -n \
    --arg approach "$APPROACH" \
    --arg state "$NEW_STATE" \
    --arg intent "$INTENT" \
    --argjson commit_requested "$COMMIT_REQUESTED" \
    --argjson notes "$SAVE_NOTES" \
    '{approach: $approach, state: $state, intent: $intent, commit_requested: $commit_requested, notes: $notes, validation_phase: 0}' \
    > "$STATE_FILE" 2>/dev/null || true

# ==========================================================================
# Build context based on intent and state
# ==========================================================================

# Intent-specific context
CONTEXT=""
case "$INTENT" in
    approval)
        RESTATEMENT_TARGET="what was discussed"
        CONTEXT="Approval. Start work on what was just discussed."
        if [ -n "$EXECUTE_INSTRUCTIONS" ]; then
            CONTEXT="${CONTEXT}\n\nAdditional scope from user:\n${EXECUTE_INSTRUCTIONS}"
        fi
        ;;
    question)
        RESTATEMENT_TARGET="the question"
        if [ "$EXECUTE_COUNT" -gt 0 ]; then
            CONTEXT="This is a question with action items. Answer the question AND execute the action items."
        else
            CONTEXT="This is a question. Answer it.\n\n- Don't edit the code. Don't make decisions based on a question. Don't assume intent."
        fi
        CONTEXT="${CONTEXT}\n- Don't be a sycophant. No hedging. No \"you're right, the problem is…\" after a question. No reframing, validating, or characterizing the question — just answer it.\n- Don't guess what the user wants or means. Don't infer feedback from questions. A question is not a complaint or a critique — never respond with \"you're right to question this.\"\n- Don't exit plan mode.\n- Don't update the plan.\n- Never bias and direct the user with your reply. Objectively report the facts & only the facts.\n- Directly answer with the root cause and the architectural decisions that led here.\n- Never ask questions the code can answer — read the code first, then answer.\n- Focus – answer EXACTLY what was asked & provide the necessary context.\n- Stay consistent – Jordan's word is gospel; don't forget.\n\nWhen presenting options or answering questions, use /pcc skill: architecturally distinct options, each with pros, cons, and confidence percentage. For yes/no questions, present the case for both sides. No hedging — state confidence as a percentage."
        ;;
    correction)
        RESTATEMENT_TARGET="these corrections"
        CONTEXT=""
        if [ -n "$CORRECTION_INSTRUCTIONS" ]; then
            CONTEXT="Corrections from user (acknowledge, do not change direction):\n${CORRECTION_INSTRUCTIONS}"
        fi
        if [ -n "$EXECUTE_INSTRUCTIONS" ]; then
            [ -n "$CONTEXT" ] && CONTEXT="${CONTEXT}\n\n"
            CONTEXT="${CONTEXT}Instructions from user:\n${EXECUTE_INSTRUCTIONS}"
        fi
        if [ -n "$QUESTION_INSTRUCTIONS" ]; then
            [ -n "$CONTEXT" ] && CONTEXT="${CONTEXT}\n\n"
            CONTEXT="${CONTEXT}Questions from user (answer these, do not act on them):\n${QUESTION_INSTRUCTIONS}"
        fi
        [ -z "$CONTEXT" ] && CONTEXT="Corrections from user:\n${INSTRUCTIONS}"
        CONTEXT="${CONTEXT}\n\nThe user corrected your previous output. Incorporate the correction and deliver a complete response in the same format as the original — not prose diffs. Any unresolved questions from the previous proposal must be re-surfaced until answered."
        ;;
    instructions|proposal_request)
        if [ -n "$QUESTION_INSTRUCTIONS" ]; then
            RESTATEMENT_TARGET="the questions and instructions"
        else
            RESTATEMENT_TARGET="these instructions"
        fi
        # Grouped format: questions first (must be answered), corrections second (context), instructions last (action)
        CONTEXT=""
        if [ -n "$QUESTION_INSTRUCTIONS" ]; then
            CONTEXT="The user asked questions that must be answered. Answer them directly:\n${QUESTION_INSTRUCTIONS}"
        fi
        if [ -n "$CORRECTION_INSTRUCTIONS" ]; then
            [ -n "$CONTEXT" ] && CONTEXT="${CONTEXT}\n\n"
            CONTEXT="${CONTEXT}Corrections from user (acknowledge, do not change direction):\n${CORRECTION_INSTRUCTIONS}"
        fi
        if [ -n "$EXECUTE_INSTRUCTIONS" ]; then
            [ -n "$CONTEXT" ] && CONTEXT="${CONTEXT}\n\n"
            CONTEXT="${CONTEXT}Instructions from user:\n${EXECUTE_INSTRUCTIONS}"
        fi
        # Fallback if no grouped instructions extracted
        if [ -z "$CONTEXT" ]; then
            CONTEXT="Instructions from user:\n${INSTRUCTIONS}"
        fi
        if [ -n "$SKILLS" ]; then
            CONTEXT="${CONTEXT}\n\nSkills to execute: ${SKILLS}"
        fi
        ;;
esac

# Sequential execution context
if [ "$SEQUENTIAL" = "true" ]; then
    CONTEXT="${CONTEXT}\n\nThese steps are strictly sequential — launch each only after the previous completes. Do not parallelize."
fi

# Shared rules and state-specific context (only for intents where the agent may take action)
ACTION_INTENT=false
case "$INTENT" in
    instructions|proposal_request|approval|correction) ACTION_INTENT=true ;;
    question) [ "$EXECUTE_COUNT" -gt 0 ] && ACTION_INTENT=true ;;
esac

if [ "$ACTION_INTENT" = true ]; then
    CONTEXT="${CONTEXT}\n\nAny architectural changes to any plan are a hard blocker — require user approval before proceeding.\n\nNever change the scope of the user's requirements without approval. No adding features, removing requirements, reinterpreting terminology, creating files outside the stated scope, or hacking infrastructure as a workaround. If scope needs to change, state what and why, then wait for approval before proceeding.\n\nWhen the user gives feedback on a decision, evaluate the options and present findings — never conclude with a decision. The user is the decision maker."

    if [ "$APPROACH" = "subagents" ] || [ "$APPROACH" = "team" ]; then
        CONTEXT="${CONTEXT}\n\nWhen dispatching subagents: communicate WHY and WHAT only — not HOW (unless the HOW is a unique finding the subagent won't realistically discover reading the code). Do not pre-research, pre-read files, or run commands to \"prepare\" for a subagent. Do not include information the subagent can find in the codebase. The value of subagents is fresh, unbiased context — over-instruction destroys this.\n\nWhen told to do something N times in parallel, run all N in parallel — never serialize. Use a single message with multiple Agent tool calls."
    fi
fi

if [ "$ACTION_INTENT" = true ] && [ "$NEW_STATE" = "proposing" ]; then
    CONTEXT="${CONTEXT}\n\nResearch the codebase before proposing. Never propose changes to code you haven't read.\n\nPresent a full, complete proposal before executing anything — do not make the user piece together context from prior messages.\n\nWhen presenting options in proposals, use /pcc skill: architecturally distinct options, each with pros, cons, and confidence percentage. Explore both sides of every tradeoff. Every decision point presents architecturally distinct options — never advocate for a single approach without alternatives.\n\nResearch every option and deliver a complete proposal in one response. Do not use progressive disclosure — the user is an architect who corrects direction, not a student who needs guided discovery.\n\nBefore proposing, identify every element you're uncertain about and research each one — read full files, not grep fragments. Never propose from general knowledge when the code can answer definitively.\n\nIn proposals, never:\n- Hedge (\"may\", \"probably\", \"likely\", \"might\") — if you'd hedge, you haven't read enough code yet\n- Echo requirements back as proposals — include concrete HOW (mechanisms, code paths, data flow), not reworded WHAT\n- Present options below 80% confidence — low confidence means research more, not label and ship\n\nYour proposal must address exactly the requirements stated — nothing added, nothing removed, nothing reframed. Present options within the stated scope, not options that change it. If you believe the scope should be different, say so explicitly as a separate decision point before proposing.\n\nInclude the full, complete proposal at the end of every response. The user should never need to scroll back or remember a prior message to see the current proposal. Proposals are appended to and updated — never shortened, summarized, or replaced with diffs.\n\nEnd every proposal with a numbered list of open questions for the user. If there are no open questions, state that explicitly.\n\nIf your previous proposal contained unresolved questions, re-surface every unanswered question in your response. Unresolved questions must appear in every agent response until the user answers them — never silently drop a question when revising a proposal."
elif [ "$ACTION_INTENT" = true ] && [ "$NEW_STATE" = "executing" ]; then
    CONTEXT="${CONTEXT}\n\nResearch the codebase before editing. Never change code you haven't read.\n\nBefore acting, verify you have sufficient context — read relevant files, check existing patterns, and research unknowns. Do not rush to implement."
elif [ "$ACTION_INTENT" = true ] && [ "$NEW_STATE" = "auto" ]; then
    CONTEXT="${CONTEXT}\n\nThis message contains mixed intents. Execute action items first, then answer questions. The user expects actions completed before discussion. Research the codebase before editing. Never change code you haven't read."
fi

# Commit context
if [ "$COMMIT_REQUESTED" = "true" ]; then
    CONTEXT="${CONTEXT}\n\nSkills to execute: /commit-message\n\nAfter completing the commit, review session notes and suggest which should become permanent — in global/project Claude.md, skills, agents, rules, or commands as appropriate. Present suggestions only, do not act on them."
fi

# Recommended agents (injected when classifier identifies matching specialists)
if [ -n "$RECOMMENDED_AGENTS" ]; then
    CONTEXT="${CONTEXT}\n\nRecommended agents for this task:\n${RECOMMENDED_AGENTS}"
fi

# Prepend state change notifications
[ -n "$STATE_NOTIFICATIONS" ] && \
    CONTEXT="State changes (applied by classifier, no action needed):\n${STATE_NOTIFICATIONS}\n${CONTEXT}"

# Head-anchor restatement instruction (must be first for primacy)
RESTATEMENT="Open your response with a conversational restatement of ${RESTATEMENT_TARGET} — in your own words, proving you understood. Follow the restatement examples in Claude.md. Do not take any action before restating."
if [ "$INTENT" = "instructions" ] || [ "$INTENT" = "proposal_request" ]; then
    RESTATEMENT="${RESTATEMENT} Execute detected /skills immediately after restating."
fi
CONTEXT="${RESTATEMENT}\n\n${CONTEXT}"

ESCAPED_CONTEXT=$(printf '%b' "$CONTEXT" | jq -Rs .) || { exit 0; }

printf '{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":%s}}\n' "$ESCAPED_CONTEXT"

exit 0
