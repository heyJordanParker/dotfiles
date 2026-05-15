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

# Ensure session state file exists (creates with defaults if missing)
[ ! -f "$STATE_FILE" ] && "$HOME/.claude/hooks/initialize-session-state.sh" "$SESSION_ID"

# Read existing session state
CURRENT_APPROACH=$(jq -r '.approach // "solo"' "$STATE_FILE" 2>/dev/null) || CURRENT_APPROACH="solo"
CURRENT_STATE=$(jq -r '.state // "proposing"' "$STATE_FILE" 2>/dev/null) || CURRENT_STATE="proposing"
CURRENT_INTENT=$(jq -r '.intent // "instructions"' "$STATE_FILE" 2>/dev/null) || CURRENT_INTENT="instructions"
CURRENT_NOTES=$(jq -c '.notes // []' "$STATE_FILE" 2>/dev/null) || CURRENT_NOTES="[]"

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

JSON_SCHEMA='{"type":"object","properties":{"intent":{"type":"string","enum":["approval","question","instructions","correction","proposal_request"]},"instructions":{"type":"array","items":{"type":"object","properties":{"text":{"type":"string"},"mode":{"type":"string","enum":["question","execute","correction"]}},"required":["text","mode"]}},"skills":{"type":"array","items":{"type":"string"}},"approach_change":{"type":"string","enum":["solo","subagents","team","no_change"]},"state_change":{"type":"string","enum":["proposing","executing","auto","no_change"]},"sequential":{"type":"boolean"},"commit_requested":{"type":"boolean"},"session_notes":{"type":"array","items":{"type":"string"}},"recommended_agents":{"type":"array","items":{"type":"object","properties":{"agent":{"type":"string"},"reason":{"type":"string"}},"required":["agent","reason"]}}},"required":["intent"]}'

SYSTEM_PROMPT="You are a classifier. Classify user intent. Extract instructions. Output structured JSON only."

EVALUATION_PROMPT="Classify this user message's INTENT and extract any specific instructions, requirements, or constraints.

${SESSION_NOTES_CONTEXT}${CONVERSATION_CONTEXT}Current message to classify:
$PROMPT

You are classifying INTENT only. The state machine transitions are handled separately — you do not decide the agent's state.

Intent types:
- \"approval\" — user is approving/accepting a specific proposal or plan that the agent just presented. Requires a preceding proposal to approve. Message can be pure (\"yes\", \"go ahead\") OR compound — an approval/acknowledgement signal followed by imperative scope directing execution of the just-proposed work (\"okay, update those using /cc\", \"perfect, commit this with /commit-message\"). In compound form, intent stays \"approval\" AND the imperative tail is extracted into instructions[] with mode: \"execute\" — the approval signal is primary, the imperative is scope attached to it, not a separate directive. \"just fix it\" or \"do it\" as a standalone directive without a preceding proposal → instructions, not approval. An approval lead word followed by a pivot to unrelated new work (\"okay, now let's do X instead\") → instructions — the lead word only carries approval weight when the remainder references the standing proposal.
- \"question\" — user is asking a question that requires an answer. No action should be taken.
- \"instructions\" — user is giving the agent new work to do with action language (\"fix\", \"add\", \"change\", \"implement\", \"update\", \"remove\", \"refactor\"). Also standalone constraints and boundaries (\"don't use third-party libraries\") when there is no previous agent output being refined.
- \"correction\" — user is correcting or giving feedback on previous agent output (\"that's wrong\", \"no, use X instead\", \"this is fine\", \"this is a non-issue\"). Requires previous agent output being refined — a standalone constraint like \"don't use third-party libraries\" without the agent having proposed or used them is instructions (a boundary), not correction.
- \"proposal_request\" — user is asking for analysis, investigation, review, or a proposal BEFORE execution (\"propose\", \"analyze\", \"what would we need\", \"design\", \"plan\", \"evaluate\", \"compare options\", \"investigate\", \"look into\", \"dig into\", \"figure out\", \"see what's going on\", \"check why\", \"double check\", \"verify\", \"review\", \"audit\", \"validate\").

Rules:
0. COMPOUND APPROVAL + IMPERATIVE SCOPE is the dominant approval shape — users rarely send a bare \"yes\"; they approve and add scope in one breath. When a message opens with an approval/acknowledgement lead word (\"okay\", \"yes\", \"sure\", \"yeah\", \"alright\", \"go ahead\", \"perfect\", \"great\", \"fine\", \"approved\", \"let's\", \"cool\", \"nice\") AND the remainder contains imperative verbs referring to work the agent just proposed or discussed, classify as \"approval\" — NOT \"instructions\". Extract the imperative remainder into instructions[] with mode: \"execute\". Both the approval signal AND the executable scope are preserved in the output. Failure mode to avoid: dropping the lead word as throat-clearing and classifying the remainder as pure \"instructions\" — this locks the session in propose-first mode and blocks the edits the user just approved.
1. A message that makes sense as a response to the last AI output IS a response to it — classify based on what it's responding to, not its surface form. A single word after an options list is a selection (approval). A short reply after a proposal is feedback (correction or approval).
2. Emotional language is emphasis, not a separate category. Strip the emotion, classify the underlying intent.
3. Corrections preserve the current direction. A message that corrects previous output while also containing forward-looking language (\"no, just use X and then deploy it\") is still a correction — the user is refining, not giving new independent instructions. Messages that add scope to an active proposal or discussion (\"one more thing:\", \"also include\", \"we also need\", \"don't forget about\") are corrections when the user is refining what should be proposed. Only classify as \"instructions\" when the message introduces genuinely new work unrelated to the previous output.
4. When uncertain between \"instructions\" and \"proposal_request\", prefer \"proposal_request\". The conservative default is to propose before executing.\n4b. When a correction also redirects to a fundamentally new direction (\"that won't work, propose something with Redis instead\", \"scrap this, try a different approach\"), classify as \"proposal_request\" — the user wants a fresh proposal, not a patched version of the rejected one.\n4c. When the user answers questions from a proposal (\"yes\", \"Option A\", \"obviously\"), classify as \"correction\" — the user is refining the proposal with their answers, not approving execution. Only classify as \"approval\" when the user explicitly signals the proposal is complete and ready for execution.
5. Set \"sequential\" to true when the user's instructions have explicit ordering (\"after that\", \"then\", \"finally\", numbered steps with dependencies). Default to false.
6. All user instructions contain subtleties and nuance — preserve ordering language, execution context, autonomy cues, and boundary conditions verbatim. Never flatten, summarize, or strip nuance from extracted instructions. Each instruction is an object with \"text\" (the instruction) and \"mode\" (one of: \"execute\" for actions, \"question\" for things the user wants answered, \"correction\" for feedback on previous analysis). When a message contains BOTH a question AND an action directive, extract both with their respective modes — never absorb an action into a question or vice versa. Example: \"how does X work? also change Y to Z\" → [{\"text\": \"how does X work\", \"mode\": \"question\"}, {\"text\": \"change Y to Z\", \"mode\": \"execute\"}].
6b. Questions in the user's message require direct answers. Extract every question as a separate mode: \"question\" instruction, even when embedded in corrections, emotional language, or rhetorical framing. \"What is this about?\" is a question. \"Do we have this elsewhere?\" is a research question. \"Did you read X?\" is a question requiring an honest answer. Do not dismiss questions as decorative or rhetorical.
7. Only include a skill in \"skills\" if the user is invoking it — telling the agent to use or execute it NOW. If the skill is discussed, referenced, or mentioned as context, do not include it. \"use /commit\" → include. \"the /subagents skill handles this\" → exclude. When in doubt, include it — a false positive is cheaper than a false negative.
8. Set \"approach_change\" to \"no_change\" unless the user signals an approach shift. Two trigger families:\n(a) Direct mode-change requests — always trigger transition. The user names the target mode using forms like \"enter X mode\", \"go X\" / \"go to X mode\", \"switch to X\", \"X mode\", where X is one of solo, subagents, team. Negation/inverse forms also count when they unambiguously name a target. Direct mode-change requests are a HARD OVERRIDE — they fire even when buried inside long messages, repeated for emphasis, or wrapped in frustration. The user's literal naming of the target mode is dispositive; absence of dispatch verbs does not block the transition.\n  - \"solo\" on: \"go solo\", \"enter solo mode\", \"solo mode\", \"switch to solo\", \"do this yourself\", \"don't spawn agents\", \"read it yourself\".\n  - \"subagents\" on: \"enter subagents mode\", \"go subagents\", \"go to subagents\", \"go to subagents mode\", \"switch to subagents\", \"subagents mode\", \"exit solo\", \"use agents\", \"use subagents\".\n  - \"team\" on: \"enter team mode\", \"go team\", \"go to team mode\", \"switch to team\", \"team mode\", \"get a team\", uses /team.\n(b) Dispatch-language signals — trigger ONLY when the new approach differs from current. \"subagents\" when the user asks to launch, spawn, dispatch, or use agents/subagents in this turn (e.g. \"spawn a subagent\", \"launch 3 agents in parallel\", \"have a @debugger investigate\", \"get an agent to do X\", \"1 subagent to research Y\"). Output \"no_change\" if current approach is already \"subagents\" or \"team\" — these phrases describe work within the existing approach, not a shift.\nSingle-action directives without dispatch language (\"run the tests\", \"fix the bug\", \"add a guard\", \"commit this\") are NOT approach signals. Mentioning a specific agent (e.g. \"get a @debugger\") is NOT a \"team\" signal — \"team\" requires explicit team language. It IS a \"subagents\" signal when current approach is \"solo\". Current approach: ${CURRENT_APPROACH}.
8b. Set \"state_change\" to \"no_change\" unless the user signals a state shift. Two trigger families:\n(a) Mode-name declarations — always trigger transition. \"executing\" on \"execute mode\", \"enter execute mode\", \"go into execute mode\". \"proposing\" on \"proposing mode\", \"go back to proposing\", \"propose first\". \"auto\" on \"auto mode\", \"mixed mode\".\n(b) Execution-intent signals — trigger \"executing\" when the user's phrasing rules out a propose-first interpretation: \"execute this\" / \"execute X\" / \"execute the plan\", \"just do it\" / \"just execute\" / \"skip the proposal\", \"implement this directly\" / \"build it now\" / \"ship it\" / \"do it now\", or execution paired with dispatch (\"spawn a subagent to implement X\", \"launch agents to fix Y\", \"get an agent to build Z\").\nConservative default: if the message could reasonably be read as a request to research, investigate, or propose, output \"no_change\" and let the state machine apply its propose-first bias. Only output \"executing\" when the phrasing explicitly demands action NOW.\nThis is a HARD OVERRIDE — output wins over intent classification and current state. Single-action directives without execution-intent language (\"fix the bug\", \"commit this\", \"deploy\") are NOT state signals — they're work instructions handled by the state machine. Current state: ${CURRENT_STATE}.
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
- \"okay, update those using /cc\" → {\"intent\": \"approval\", \"instructions\": [{\"text\": \"update those using /cc\", \"mode\": \"execute\"}], \"skills\": [\"/cc\"], ...}
- \"sure, and also update our /cc references to mention those conditionals\" → {\"intent\": \"approval\", \"instructions\": [{\"text\": \"also update our /cc references to mention those conditionals\", \"mode\": \"execute\"}], \"skills\": [\"/cc\"], ...}
- \"perfect, commit this with /commit-message\" → {\"intent\": \"approval\", \"instructions\": [{\"text\": \"commit this with /commit-message\", \"mode\": \"execute\"}], \"skills\": [\"/commit-message\"], ...}
- \"approved\\n\\nwhen done, commit with /commit-message and push\" → {\"intent\": \"approval\", \"instructions\": [{\"text\": \"when done, commit with /commit-message and push\", \"mode\": \"execute\"}], \"skills\": [\"/commit-message\"], ...}
- \"yeah, reread the /cc skill & then update\" → {\"intent\": \"approval\", \"instructions\": [{\"text\": \"reread the /cc skill & then update\", \"mode\": \"execute\"}], \"skills\": [\"/cc\"], ...}
- \"yes. make sure you don't touch the tmux infrastructure at all; we are NOT migrating\" → {\"intent\": \"approval\", \"instructions\": [{\"text\": \"make sure you don't touch the tmux infrastructure at all; we are NOT migrating\", \"mode\": \"execute\"}], ...}
- \"fine. implement & test this; I want to see it completely working after I'm back\" → {\"intent\": \"approval\", \"instructions\": [{\"text\": \"implement & test this; I want to see it completely working after I'm back\", \"mode\": \"execute\"}], ...}
- \"go ahead; use the bricks skill if you have bricks specific work... execute\" → {\"intent\": \"approval\", \"instructions\": [{\"text\": \"use the bricks skill if you have bricks specific work... execute\", \"mode\": \"execute\"}], ...}
- \"okay, now let's do X instead\" → {\"intent\": \"instructions\", ...}  (negative case — lead word does NOT carry approval when the remainder pivots to unrelated new work)
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
# Auto-invoke matching skill on approach transition (mirrors per-mode skills)
if [ "$APPROACH" != "$CURRENT_APPROACH" ]; then
    case "$APPROACH" in
        solo)      AUTO_SKILL="/solo" ;;
        subagents) AUTO_SKILL="/subagents" ;;
        team)      AUTO_SKILL="/team" ;;
        *)         AUTO_SKILL="" ;;
    esac
    if [ -n "$AUTO_SKILL" ] && [[ ! "$SKILLS" == *"$AUTO_SKILL"* ]]; then
        SKILLS="${SKILLS:+${SKILLS}, }${AUTO_SKILL}"
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
        # Mixed intent: question + execute instructions → auto (but not from proposing)
        if [ "$EXECUTE_COUNT" -gt 0 ] && [ "$CURRENT_STATE" != "proposing" ]; then
            NEW_STATE="auto"
        fi
        ;;
    correction)
        # Mixed intent: correction + execute instructions → auto (but not from proposing)
        if [ "$EXECUTE_COUNT" -gt 0 ] && [ "$CURRENT_STATE" != "proposing" ]; then
            NEW_STATE="auto"
        fi
        ;;
esac

# Hard override: explicit state declaration from user wins over state-machine derivation
STATE_CHANGE=$(echo "$RESULT" | jq -r '.state_change // "no_change"' 2>/dev/null) || STATE_CHANGE="no_change"
if [ "$STATE_CHANGE" != "no_change" ]; then
    NEW_STATE="$STATE_CHANGE"
fi

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
        CONTEXT="${CONTEXT}\n- Don't be a sycophant. No hedging. No \"you're right, the problem is…\" after a question. No reframing, validating, or characterizing the question — just answer it.\n- Don't guess what the user wants or means. Don't infer feedback from questions. A question is not a complaint or a critique — never respond with \"you're right to question this.\"\n- Don't exit plan mode.\n- Don't update the plan.\n- Never bias and direct the user with your reply. Objectively report the facts & only the facts.\n- Directly answer with the root cause and the architectural decisions that led here.\n- Never ask questions the code can answer — read the code first, then answer.\n- Zero-guess policy — every code assertion must be validated against the source before it is claimed. If the answer depends on what the code does, returns, contains, or causes, reading the relevant source is mandatory before answering. No statement about code without reading it. Pattern matching is not validation. The correct answer beats the quick answer — your instinct will be to answer fast; resist it. Never trade rigor for speed.\n- Focus – answer EXACTLY what was asked & provide the necessary context.\n- Stay consistent – Jordan's word is gospel; don't forget.\n\nWhen presenting options or answering questions, use /pcc skill: architecturally distinct options, each with pros, cons, and confidence percentage. For yes/no questions, present the case for both sides. No hedging — state confidence as a percentage.\n\nBut not every question warrants options. These question shapes take a direct answer — do NOT force /pcc:\n- Why / reasoning: \"why did we pick X over Y\", \"why do we need this abstraction\", \"why is there duplication between X and Y\"\n- Verification / yes-no: \"have we handled the null case for X\", \"are we already using library Y somewhere\", \"have we already solved this in module N\"\n\nUse /pcc only when the user is choosing between REAL architectural alternatives — fundamentally different mechanisms, boundaries, data flows, or dependencies. Micro-decisions (file placement, naming, single-call refactors, log message wording, mechanical tweaks) get a direct answer, not an options list.\n\n/pcc requires 2+ viable options. If you only have one viable approach, present it as the answer itself — no pros, no cons, no confidence percentage. A single option is not a recommendation; recommendations rank multiple options. Never wrap a lone option in the /pcc format.\n\nPros describe how the option solves the stated problem. Cons describe real costs or risks the option introduces. Forbidden in cons: cross-option references (\"more complex than Option Z\"), treating normal implementation cost as inherent badness (\"8-file edit\"), filler added to balance the format. If an option has no real con, say so.\n\nConfidence ranks rightness — how confident you are this option is the right call for the stated problem, accounting for compromises. Major compromises drag the score down. Options clustered within ~10% (88/90/92) mean you haven't actually differentiated them.\n\nInconsistent confidence scores or pros/cons that feel forced are a signal of lacking codebase research. Fix them by reading more code, not by adjusting numbers or reshuffling bullets. Never ship a /pcc with patched-over scores — go back and research until the differences are clear."
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
    CONTEXT="${CONTEXT}\n\nAny architectural changes to any plan are a hard blocker — require user approval before proceeding.\n\nNever change the scope of the user's requirements without approval. No adding features, removing requirements, reinterpreting terminology, creating files outside the stated scope, or hacking infrastructure as a workaround. If scope needs to change, state what and why, then wait for approval before proceeding.\n\nWhen the user gives feedback on a decision, evaluate the options and present findings — never conclude with a decision. The user is the decision maker.\n\nZero-guess policy — every code assertion must be validated against the source before it is claimed. No statement about what code does, calls, returns, contains, or causes without reading it first. Applies to reports, summaries, answers, and proposals — not just edits. Pattern matching is not validation. The correct answer beats the quick answer — never trade rigor for speed."

    CONTEXT="${CONTEXT}\n\nDELIVERABLE-TURN ENFORCEMENT:\n- The complete deliverable goes in THIS turn's FINAL user-visible text. No deferral, no \"will be regenerated next turn\", no \"for now here's X, I'll redo later\". If your work is off-brief or sub-quality, redo it now, before emitting — never ship acknowledged-bad output.\n- When the deliverable needs more research, more reads, more iterations, or more thinking to land correctly, invest that work now, in this same turn, before emitting. Extra effort in-turn always costs less than a follow-up turn. Bias to do the work, not ship-and-revise.\n- Do NOT place the deliverable in thinking blocks or in text emitted before tool calls — the render folds those zones and the user will not see them. The deliverable lives in the final assistant text, after the last tool call, in this same turn."

    if [ "$APPROACH" = "subagents" ] || [ "$APPROACH" = "team" ]; then
        CONTEXT="${CONTEXT}\n\nWhen dispatching subagents: communicate WHY and WHAT only — not HOW (unless the HOW is a unique finding the subagent won't realistically discover reading the code). Do not pre-research, pre-read files, or run commands to \"prepare\" for a subagent. Do not include information the subagent can find in the codebase. The value of subagents is fresh, unbiased context — over-instruction destroys this.\n\nWhen told to do something N times in parallel, run all N in parallel — never serialize. Use a single message with multiple Agent tool calls."
    fi
fi

if [ "$ACTION_INTENT" = true ] && [ "$NEW_STATE" = "proposing" ]; then
    CONTEXT="${CONTEXT}\n\n# Proposal-Mode Rules\n\nThese rules govern every change proposal, in any codebase, for any problem. Examples are illustrative only.\n\nThe quality bar is clean readable markdown at full width, plain headings, honest options. Match it. Every rule below removes a way proposals get worse.\n\n## Verification is your work and never appears\n\nCheck every claim against the actual source this turn — read the real files, whole files, not grep fragments. Grep finds the line; it does not ground the claim. Not remembered, not inferred. One extra full read costs less than one guessed claim. Checked against your transcript; an unverified proposal is void. A confident reconstruction of code you did not open this turn is the worst possible output: it reads as authoritative and it is a guess, and the user ends up doing the validation you skipped.\n\nThe first character of your response is \`#\`, the title. No \"I have what I need\", no \"the proposal follows\", no summary of what you read. The verification happened; its only trace is a correct proposal. A line of any kind before the title is a failure.\n\n## Who you are\n\nYou are the architect six months from now, alone, debugging at 11pm. You wrote this; the wiring has faded; past-you owes you clarity.\n\n## Who the architect is and what they care about\n\nThe architect knows the domain. They do not know this change yet, and they will not open the codebase to learn it. They are reviewing an architecture decision, not reading a tutorial.\n\nThey care about: what our code does, which of our components own which responsibility, where our boundaries move, and whether a third-party dependency is the right choice for the job. They do not care how the third-party library works internally. How a framework boots, which framework file requires which, the framework's internal call sequence — that is your knowledge for getting the proposal right. It never appears in the proposal. Not in the Why, not in a step, not in a choice. If a sentence explains the mechanism of third-party code, delete it. State only what our code does and what we depend on the third party to do — never how the third party does it.\n\nWrite at the level of someone who knows the domain. Do not explain basics. Do not narrate a sequence of framework events. State the architectural change: which component now owns what, which dependency relationship inverts, what contract that creates.\n\n## What is and is not a decision\n\nA decision is a point where the brief left a real open choice and picking one option makes the work under the other wrong.\n\nThe brief's own mandate is never a decision. If the brief says to make a change, that change is the work, not a question. Never reframe an instruction as something to decide.\n\nA decision is an architectural alternative the architect is choosing between — fundamentally different mechanisms, boundaries, data flows, or dependencies. It is not every finding, gap, error, or bug you surface. When the architect asked for an investigation, an audit, gaps, or errors, deliver the findings: each with its place and its impact, and nothing else. Never manufacture options for items the architect asked you to surface. When the architect already picked an option and asks to refine one part of it, apply the refinement and nothing else — never regenerate variants of the part they kept.\n\nDo not pre-announce decisions. The Why never lists \"the decisions are X and Y\". A decision appears once, where it is made, and nowhere else — not foreshadowed, not summarized, not repeated. Stating it twice is a defect.\n\n## Never propose a regression\n\nA regression is the user losing something they can do, or our system losing something it can do. Never present one as a tradeoff, an option, an acceptable cost, or a parenthetical inside an option. No confidence number buys it back.\n\nWhen a path appears to require breaking a capability, the path is wrong. The only response is more research — read more callers, more extension points, more of the existing code that is there for a reason — until you find the path that keeps every capability. A proposal that contains a regression misaligns every piece of work that follows it. It is void however well it reads.\n\n## Unanswered questions never disappear\n\nEvery choice you put to the architect that they did not answer reappears, in full, in every later version of the proposal, until they answer it. Never drop a question because the proposal moved on. Never assume an answer to keep going. An unanswered question silently removed is the same failure as a guess written as fact.\n\nEmit a question only when a real external-context gap exists — an environment, prerequisite, constraint, or scope boundary the code cannot answer — and state what flips in the proposal under each answer. Never invent an assumption to fill the slot; an assumption tail fabricates context and rots across every later turn. Never rephrase an option-pick as a question. Never ask a motivation probe, an open-ended \"thoughts?\", an obvious confirmation, or anything that points at something the architect cannot recall. If no real gap exists, write \"No open questions.\"\n\n## No metaphor. No jargon. No hype. No importance-in-prose.\n\nEvery word names the actual thing. Banned analogy-words: \"cutover\", \"fork\", \"harvest\", \"leverage\", \"surface\" (verb), \"bridge\", \"glue\", \"wire up\", \"hang off\", \"ride on\", \"load-bearing\", \"safety net\". Say the action.\n\nImportance is carried by order, never stated. The first slice and the first step in it are the most important. Never write \"the genuinely hard part\", \"the key one\", \"the biggest lever\".\n\n## The shape\n\nThree parts, in order. No other top-level sections. No \"verified facts\", no \"flagged claims\", no list of decisions.\n\n### Why\n\nThree to five sentences. The architectural change: what our code does after this that it did not before, which dependency relationship changes, what new contract or boundary that creates, and what is genuinely difficult about it (stated as the difficulty, in domain terms, not as framework mechanism). No third-party internals. No list of upcoming decisions.\n\n### The plan\n\nThe change is one or more slices. A slice is a coherent piece of the architecture — a responsibility that moves, a boundary that forms, a capability that changes owner. Slices decompose the change the way the architecture decomposes, not the way the framework's boot sequence runs.\n\nA slice heading is the plain name of what the slice does to our architecture. It never contains the word \"Slice\". It is never numbered.\n\nEach slice holds its own steps, numbered from 1 within that slice. \`Step 1\`, \`Step 2\` restart in the next slice. There is no global step count across the proposal.\n\nA step is one short full-width paragraph: what our code change is and the effect on our system. Readable prose, not a labelled block, not a collapsed bullet, not a narrow column.\n\nAt most one file tree per slice, and only when the slice touches enough of our files that prose alone is ambiguous. Many slices need no tree. Never a tree per step. Never a tree per file. A tree lists only our files the slice creates or changes, with a short role note and \`<- (NEW)\` for new ones:\n\n\`\`\`\napp/Tenant/SomeArea/\n├── Thing.php*        <- (NEW) one-line role\n└── Other.php         <- what changes in it\n\`\`\`\n\nOrder slices, and steps within them, so each one's context was delivered by the ones before. Never forward-reference a later step.\n\nNo step exists only to undo or correct a previous step. If a step would do something a later step walks back, the plan is wrong — do it correctly once, in the right place. \"Do X everywhere, then remove X where it was not needed\" is two steps doing one job badly; state the rule for where X belongs and apply it once.\n\n### Choices, in place\n\nA choice appears once, inside the step where the decision is made, where the reader reaches it. The heading is the plain question — no \"Decide:\", no \"Fork\", no \"Choice:\" prefix. Then one or two sentences of what is at stake, including the concrete cost not visible until after an option is built — in domain terms, about our code, never framework mechanism. Then the options:\n\n\`\`\`\nShould the held registrations live on the existing service or a new class?\n\nThe reader who has to find where this behavior lives opens whichever this\npicks. The existing service is already large and unrelated to this concern.\n\n**Option A — on the existing service.** What it is, concretely, in our code.\n\n- pro: how it solves the stated problem\n- con: the concrete cost it adds, the one not foreseen until it bites\n- confidence: 55%\n\n**Option B — a new single-purpose class.** What it is, concretely.\n\n- pro: ...\n- con: ...\n- confidence: 78%\n\`\`\`\n\nOption name and one-line description on the heading line. Pros, cons, confidence as separate \`-\` bullets. A con states a real cost the option adds — never a cross-reference to another option, never normal implementation effort dressed up as a flaw, never filler to balance the format. If an option has no real con, say so.\n\nConfidences differ by more than 10 points. Equal-ish confidence means the analysis is unfinished — read more code, do not adjust the numbers. Forced pros, forced cons, or clustered confidences are the signal that you have not read enough; fix them by reading, never by renumbering. No recommendation, no pick, no \"later steps assume A\".\n\nA slice with no decision has no choice block and says nothing about it. Never announce the absence of a choice. Never point at a choice made in another slice — a choice appears once, where it is made.\n\n## Readability\n\nFull width. Short sentences, one idea each. Blank line between ideas. No paragraph over three sentences. Visible whitespace.\n\n## One response, complete\n\nDeliver the entire proposal in one response. No progressive disclosure — the architect corrects direction, they do not discover it one piece at a time. When the response is a proposal, the whole current proposal is in it; the architect never scrolls back or rebuilds state from an earlier message. As it evolves, prune resolved and superseded sections and re-emit the live proposal — never an \"everything else unchanged\" handoff.\n\n## Options are figured out before they are written\n\nRun every option through the requirements yourself before you write a word of it. An option that fails a requirement is dropped, or stated as rejected with the reason already worked out. Never discover a flaw mid-sentence. If you catch yourself writing \"actually\", \"wait\", \"hmm\", or \"X does not actually Y\" inside the proposal, that option was not figured out — delete it and rewrite with clean, pre-validated options. The architect receives conclusions, never your scratch work.\n\n## No hedging, no echo\n\nNever hedge — \"may\", \"probably\", \"likely\", \"might\" mean you have not read enough; go read. Never hand the requirements back reworded as the plan — state the concrete change, the code paths, the data flow.\n\n## How it ends\n\nEnds at the last confidence number or the last step's last sentence. No closing sentence, no summary of the plan's safety."
elif [ "$ACTION_INTENT" = true ] && [ "$NEW_STATE" = "executing" ]; then
    CONTEXT="${CONTEXT}\n\nResearch the codebase before editing. Never change code you haven't read.\n\nBefore acting, verify you have sufficient context — read relevant files, check existing patterns, and research unknowns. Do not rush to implement.\n\nPreserve every user-facing and system capability. Do more research, never the regression."
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
RESTATEMENT="Open your response with a conversational restatement of ${RESTATEMENT_TARGET} — in your own words, preserving every explicit requirement, constraint, count, and boundary the user stated verbatim, without adding any the user did not state. Follow the restatement examples in Claude.md. Do not take any action before restating."
if [ "$INTENT" = "instructions" ] || [ "$INTENT" = "proposal_request" ]; then
    RESTATEMENT="${RESTATEMENT} Execute detected /skills immediately after restating."
fi
CONTEXT="${RESTATEMENT}\n\n${CONTEXT}"

ESCAPED_CONTEXT=$(printf '%b' "$CONTEXT" | jq -Rs .) || { exit 0; }

printf '{"hookSpecificOutput":{"hookEventName":"UserPromptSubmit","additionalContext":%s}}\n' "$ESCAPED_CONTEXT"

exit 0
