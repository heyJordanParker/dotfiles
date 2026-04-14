#!/bin/bash
# Test suite for classify-intent.sh
# Tests state transitions, context injection, and skip conditions
# Mocks LLM calls via PATH override — tests the shell logic, not the LLM
#
# Usage:
#   ./classify-intent-test.sh                          # test the deployed script
#   ./classify-intent-test.sh /path/to/script.sh       # test a specific script

SCRIPT="${1:-$(dirname "$0")/classify-intent.sh}"
[[ ! -f "$SCRIPT" ]] && { echo "Script not found: $SCRIPT"; exit 1; }

PASS=0 FAIL=0 TEST_NUM=0

# ============================================================================
# Mock setup — override claude and timeout in PATH
# ============================================================================

MOCK_DIR=$(mktemp -d)

cat > "$MOCK_DIR/claude" << 'MOCK'
#!/bin/bash
cat > /dev/null
echo "$MOCK_CLAUDE_RESPONSE"
MOCK
chmod +x "$MOCK_DIR/claude"

cat > "$MOCK_DIR/timeout" << 'MOCK'
#!/bin/bash
shift
"$@"
MOCK
chmod +x "$MOCK_DIR/timeout"

export PATH="$MOCK_DIR:$PATH"

# ============================================================================
# Helpers
# ============================================================================

next_session() {
    TEST_NUM=$((TEST_NUM + 1))
    echo "test-intent-$$-${TEST_NUM}"
}

setup_state() {
    local session_id="$1" json="$2"
    echo "$json" > "/tmp/claude-session-state-${session_id}"
}

mock() {
    local intent="${1:-instructions}" approach_change="${2:-no_change}"
    local commit_requested="${3:-false}" instructions="${4:-[]}" skills="${5:-[]}"
    local sequential="${6:-false}" notes="${7:-[]}" agents="${8:-[]}"
    jq -n \
        --arg i "$intent" --arg ac "$approach_change" \
        --argjson cr "$commit_requested" --argjson ins "$instructions" --argjson sk "$skills" \
        --argjson sq "$sequential" --argjson n "$notes" --argjson ag "$agents" \
        '{structured_output:{intent:$i,approach_change:$ac,commit_requested:$cr,instructions:$ins,skills:$sk,sequential:$sq,session_notes:$n,recommended_agents:$ag}}'
}

run() {
    local session_id="$1" prompt="$2"
    jq -n --arg s "$session_id" --arg p "$prompt" \
        '{session_id:$s, prompt:$p, transcript_path:""}' \
    | bash "$SCRIPT" 2>/dev/null
}

get_state() {
    local session_id="$1" field="$2"
    jq -r "if .$field == null then \"\" else .$field end" "/tmp/claude-session-state-${session_id}" 2>/dev/null
}

get_context() {
    echo "$1" | jq -r '.hookSpecificOutput.additionalContext // empty' 2>/dev/null
}

# ============================================================================
# Assertions
# ============================================================================

assert_state() {
    local label="$1" session_id="$2" field="$3" expected="$4"
    local actual=$(get_state "$session_id" "$field")
    if [[ "$actual" == "$expected" ]]; then
        printf "  PASS  %s\n" "$label"; PASS=$((PASS+1))
    else
        printf "  FAIL  %s (state.%s = '%s', want '%s')\n" "$label" "$field" "$actual" "$expected"
        FAIL=$((FAIL+1))
    fi
}

has() {
    local label="$1" output="$2" needle="$3"
    if get_context "$output" | grep -qF "$needle"; then
        printf "  PASS  %s\n" "$label"; PASS=$((PASS+1))
    else
        printf "  FAIL  %s (output missing '%s')\n" "$label" "$needle"; FAIL=$((FAIL+1))
    fi
}

excludes() {
    local label="$1" output="$2" needle="$3"
    if get_context "$output" | grep -qF "$needle"; then
        printf "  FAIL  %s (output should NOT contain '%s')\n" "$label" "$needle"; FAIL=$((FAIL+1))
    else
        printf "  PASS  %s\n" "$label"; PASS=$((PASS+1))
    fi
}

no_output() {
    local label="$1" output="$2"
    if [[ -z "$output" ]]; then
        printf "  PASS  %s\n" "$label"; PASS=$((PASS+1))
    else
        printf "  FAIL  %s (expected no output)\n" "$label"; FAIL=$((FAIL+1))
    fi
}

has_output() {
    local label="$1" output="$2"
    if [[ -n "$output" ]]; then
        printf "  PASS  %s\n" "$label"; PASS=$((PASS+1))
    else
        printf "  FAIL  %s (expected output, got nothing)\n" "$label"; FAIL=$((FAIL+1))
    fi
}

# ============================================================================
# Cleanup
# ============================================================================

cleanup() {
    rm -rf "$MOCK_DIR"
    rm -f /tmp/claude-session-state-test-intent-$$-*
}
trap cleanup EXIT

# ============================================================================
# Tests
# ============================================================================

echo "================================================================"
echo "  INTENT CLASSIFIER TEST SUITE"
echo "================================================================"

DEFAULT_STATE='{"state":"executing","intent":"instructions","approach":"subagents","notes":[],"commit_requested":false,"validation_phase":0}'
PROPOSING_STATE='{"state":"proposing","intent":"proposal_request","approach":"subagents","notes":[],"commit_requested":false,"validation_phase":0}'

# --- 1. Skip conditions ---
echo ""
echo "── 1. Skip Conditions ──"

output=$(jq -n '{session_id:"agent-abc123",prompt:"hello",transcript_path:""}' | bash "$SCRIPT" 2>/dev/null)
no_output "skips agent sessions" "$output"

SID=$(next_session)
output=$(jq -n --arg s "$SID" '{session_id:$s,prompt:"",transcript_path:""}' | bash "$SCRIPT" 2>/dev/null)
no_output "skips empty prompt" "$output"

SID=$(next_session)
output=$(run "$SID" '<system-reminder>some content</system-reminder>')
no_output "skips XML system messages" "$output"

SID=$(next_session)
output=$(run "$SID" '[some system message]')
no_output "skips single-line bracket messages" "$output"

SID=$(next_session)
output=$(run "$SID" 'This session is being continued from a previous conversation')
no_output "skips session continuation" "$output"

SID=$(next_session)
output=$(run "$SID" 'Base directory for this skill: /foo/bar')
no_output "skips skill base directory" "$output"

SID=$(next_session)
export MOCK_CLAUDE_RESPONSE=$(mock "instructions" "no_change" false '[{"text":"do thing","mode":"execute"}]')
output=$(run "$SID" $'[line one]\nmore content')
has_output "multi-line bracket is NOT skipped" "$output"

# --- 2. Pure approval ---
echo ""
echo "── 2. Pure Approval ──"

SID=$(next_session)
setup_state "$SID" "$PROPOSING_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "approval")
output=$(run "$SID" "go ahead")
assert_state  "intent becomes approval"           "$SID" "intent" "approval"
assert_state  "state becomes executing"            "$SID" "state" "executing"
has           "injects start-work"                 "$output" "Approval. Start work"
excludes      "no proposal block"                  "$output" "Present a full, complete proposal"

SID=$(next_session)
setup_state "$SID" "$PROPOSING_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "approval")
output=$(run "$SID" "yes")
assert_state  "'yes' is approval"                  "$SID" "intent" "approval"
assert_state  "'yes' transitions to executing"     "$SID" "state" "executing"

SID=$(next_session)
setup_state "$SID" "$PROPOSING_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "approval")
output=$(run "$SID" "do it")
assert_state  "'do it' is approval"                "$SID" "intent" "approval"

# --- 3. Pure question ---
echo ""
echo "── 3. Pure Question ──"

SID=$(next_session)
setup_state "$SID" "$PROPOSING_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "question")
output=$(run "$SID" "why does the auth middleware work this way?")
assert_state  "intent becomes question"            "$SID" "intent" "question"
assert_state  "state stays proposing"              "$SID" "state" "proposing"
has           "injects question context"            "$output" "This is a question. Answer it"
has           "has restatement instruction"          "$output" "conversational restatement"

SID=$(next_session)
setup_state "$SID" "$DEFAULT_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "question")
output=$(run "$SID" "what does this function do?")
assert_state  "question during executing: state stays" "$SID" "state" "executing"

# --- 4. Instructions ---
echo ""
echo "── 4. Instructions ──"

SID=$(next_session)
setup_state "$SID" "$DEFAULT_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "instructions" "no_change" false '[{"text":"fix the permission check in MediaController","mode":"execute"}]')
output=$(run "$SID" "fix the permission check in MediaController")
assert_state  "intent becomes instructions"        "$SID" "intent" "instructions"
assert_state  "state becomes executing"            "$SID" "state" "executing"
has           "has execution standing rules"        "$output" "Research the codebase before editing"
excludes      "no proposal block"                   "$output" "Present a full, complete proposal"
has           "extracts instruction text"            "$output" "fix the permission check"

# Instructions from proposing state → stays proposing (refines the proposal)
SID=$(next_session)
setup_state "$SID" "$PROPOSING_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "instructions" "no_change" false '[{"text":"also add error handling","mode":"execute"}]')
output=$(run "$SID" "also add error handling")
assert_state  "instructions during proposing stay proposing" "$SID" "state" "proposing"

# Instructions from executing state → stays executing
SID=$(next_session)
setup_state "$SID" "$DEFAULT_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "instructions" "no_change" false '[{"text":"fix another bug","mode":"execute"}]')
output=$(run "$SID" "fix another bug")
assert_state  "instructions during executing stay executing" "$SID" "state" "executing"

# --- 5. Proposal request ---
echo ""
echo "── 5. Proposal Request ──"

SID=$(next_session)
setup_state "$SID" "$DEFAULT_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "proposal_request" "no_change" false '[{"text":"analyze the auth bug and propose a fix","mode":"execute"}]')
output=$(run "$SID" "analyze the auth bug and propose a fix")
assert_state  "intent becomes proposal_request"    "$SID" "intent" "proposal_request"
assert_state  "state becomes proposing"            "$SID" "state" "proposing"
has           "has proposal block"                  "$output" "Present a full, complete proposal"
has           "has unresolved questions rule"        "$output" "re-surface every unanswered question"
excludes      "no execution standing rules"         "$output" "Research the codebase before editing"

# --- 6. Approval + instructions (THE FIX) ---
echo ""
echo "── 6. Approval + Instructions ──"

SID=$(next_session)
setup_state "$SID" "$PROPOSING_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "approval" "no_change" false '[{"text":"also fix the related issue","mode":"execute"}]')
output=$(run "$SID" "go ahead, also fix the related issue")
assert_state  "intent is approval"                 "$SID" "intent" "approval"
assert_state  "state transitions to executing"     "$SID" "state" "executing"
has           "has start-work"                     "$output" "Approval. Start work"
has           "has additional scope"                "$output" "Additional scope"
has           "has execution rules"                 "$output" "Research the codebase before editing"

SID=$(next_session)
setup_state "$SID" "$PROPOSING_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "approval" "no_change" false '[{"text":"fix both issues","mode":"execute"}]')
output=$(run "$SID" "go ahead
also fix the related issue as well")
assert_state  "multiline: state transitions to executing" "$SID" "state" "executing"
has           "multiline: has additional scope"     "$output" "Additional scope"

# --- 7. Corrections ---
echo ""
echo "── 7. Corrections ──"

SID=$(next_session)
setup_state "$SID" "$PROPOSING_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "correction" "no_change" false '[{"text":"use isWritableBy not canEditContents","mode":"correction"}]')
output=$(run "$SID" "that's wrong, use isWritableBy not canEditContents")
assert_state  "correction: intent is correction"    "$SID" "intent" "correction"
assert_state  "correction: state stays proposing"   "$SID" "state" "proposing"
has           "correction: complete response rule"  "$output" "complete response in the same format"
has           "correction: unresolved questions"    "$output" "unresolved questions"

SID=$(next_session)
setup_state "$SID" "$DEFAULT_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "correction" "no_change" false '[{"text":"the file is in app/Shared","mode":"correction"}]')
output=$(run "$SID" "no, the file is in app/Shared not app/Admin")
assert_state  "correction: state stays executing"   "$SID" "state" "executing"

# --- 8. State transition table (comprehensive) ---
echo ""
echo "── 8. State Transition Table ──"

# proposing + approval → executing
SID=$(next_session)
setup_state "$SID" "$PROPOSING_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "approval")
run "$SID" "yes" > /dev/null
assert_state  "proposing + approval → executing"   "$SID" "state" "executing"

# proposing + instructions → stays proposing (instructions refine the proposal)
SID=$(next_session)
setup_state "$SID" "$PROPOSING_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "instructions" "no_change" false '[{"text":"fix it","mode":"execute"}]')
run "$SID" "fix it" > /dev/null
assert_state  "proposing + instructions → proposing" "$SID" "state" "proposing"

# proposing + proposal_request → proposing
SID=$(next_session)
setup_state "$SID" "$PROPOSING_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "proposal_request" "no_change" false '[{"text":"try a different approach","mode":"execute"}]')
run "$SID" "propose a different approach" > /dev/null
assert_state  "proposing + proposal_request → proposing" "$SID" "state" "proposing"

# proposing + question → proposing
SID=$(next_session)
setup_state "$SID" "$PROPOSING_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "question")
run "$SID" "why this approach?" > /dev/null
assert_state  "proposing + question → proposing"   "$SID" "state" "proposing"

# proposing + correction → proposing
SID=$(next_session)
setup_state "$SID" "$PROPOSING_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "correction" "no_change" false '[{"text":"wrong","mode":"correction"}]')
run "$SID" "that's wrong" > /dev/null
assert_state  "proposing + correction → proposing" "$SID" "state" "proposing"

# executing + approval → executing
SID=$(next_session)
setup_state "$SID" "$DEFAULT_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "approval")
run "$SID" "yes" > /dev/null
assert_state  "executing + approval → executing"   "$SID" "state" "executing"

# executing + instructions → executing
SID=$(next_session)
setup_state "$SID" "$DEFAULT_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "instructions" "no_change" false '[{"text":"fix it","mode":"execute"}]')
run "$SID" "fix it" > /dev/null
assert_state  "executing + instructions → executing" "$SID" "state" "executing"

# executing + proposal_request → proposing
SID=$(next_session)
setup_state "$SID" "$DEFAULT_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "proposal_request" "no_change" false '[{"text":"analyze this","mode":"execute"}]')
run "$SID" "analyze this" > /dev/null
assert_state  "executing + proposal_request → proposing" "$SID" "state" "proposing"

# executing + question → executing
SID=$(next_session)
setup_state "$SID" "$DEFAULT_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "question")
run "$SID" "what does this do?" > /dev/null
assert_state  "executing + question → executing"   "$SID" "state" "executing"

# executing + correction → executing
SID=$(next_session)
setup_state "$SID" "$DEFAULT_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "correction" "no_change" false '[{"text":"wrong","mode":"correction"}]')
run "$SID" "that's wrong" > /dev/null
assert_state  "executing + correction → executing" "$SID" "state" "executing"

# executing + correction with execute → auto
SID=$(next_session)
setup_state "$SID" "$DEFAULT_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "correction" "no_change" false '[{"text":"wrong","mode":"correction"},{"text":"fix it","mode":"execute"}]')
run "$SID" "wrong, fix it" > /dev/null
assert_state  "executing + correction+exec → auto" "$SID" "state" "auto"

# proposing + correction with execute → auto
SID=$(next_session)
setup_state "$SID" "$PROPOSING_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "correction" "no_change" false '[{"text":"wrong","mode":"correction"},{"text":"fix typo","mode":"execute"}]')
run "$SID" "wrong, fix the typo" > /dev/null
assert_state  "proposing + correction+exec → auto" "$SID" "state" "auto"

# --- 9. Approach transitions ---
echo ""
echo "── 9. Approach Transitions ──"

SID=$(next_session)
setup_state "$SID" "$DEFAULT_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "instructions" "solo" false '[{"text":"read it yourself","mode":"execute"}]')
output=$(run "$SID" "go solo, read it yourself")
assert_state  "approach set to solo"               "$SID" "approach" "solo"
has           "notifies approach change"            "$output" "approach changed"

SID=$(next_session)
setup_state "$SID" '{"state":"executing","intent":"instructions","approach":"solo","notes":[],"commit_requested":false,"validation_phase":0}'
export MOCK_CLAUDE_RESPONSE=$(mock "instructions" "subagents" false '[{"text":"use agents","mode":"execute"}]')
output=$(run "$SID" "exit solo mode")
assert_state  "approach back to subagents"         "$SID" "approach" "subagents"

SID=$(next_session)
setup_state "$SID" "$DEFAULT_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "instructions" "team" false '[{"text":"dispatch a team","mode":"execute"}]')
output=$(run "$SID" "get a team on this")
assert_state  "approach set to team"               "$SID" "approach" "team"

# no_change preserves current approach
SID=$(next_session)
setup_state "$SID" "$DEFAULT_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "instructions" "no_change" false '[{"text":"do thing","mode":"execute"}]')
output=$(run "$SID" "fix the bug")
assert_state  "no_change preserves approach"       "$SID" "approach" "subagents"
excludes      "no approach notification"            "$output" "approach changed"

# no_change preserves solo (the approach drift fix)
SID=$(next_session)
setup_state "$SID" '{"state":"proposing","intent":"proposal_request","approach":"solo","notes":[],"commit_requested":false,"validation_phase":0}'
export MOCK_CLAUDE_RESPONSE=$(mock "correction" "no_change" false '[{"text":"use X instead","mode":"correction"}]')
output=$(run "$SID" "that's wrong, use X instead")
assert_state  "no_change preserves solo on correction" "$SID" "approach" "solo"

# --- 10. Commit Requested ---
echo ""
echo "── 10. Commit Requested ──"

SID=$(next_session)
setup_state "$SID" "$DEFAULT_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "instructions" "no_change" true '[{"text":"commit the changes","mode":"execute"}]')
output=$(run "$SID" "commit this")
assert_state  "commit_requested=true"                      "$SID" "commit_requested" "true"
has           "commit notification"               "$output" "commit authorized"
has           "commit context loads skill"         "$output" "/commit-message"

SID=$(next_session)
setup_state "$SID" "$DEFAULT_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "instructions" "no_change" false '[{"text":"do thing","mode":"execute"}]')
output=$(run "$SID" "fix the bug")
assert_state  "commit_requested=false by default"          "$SID" "commit_requested" "false"
excludes      "no commit context"                 "$output" "Commit authorized"

# --- 11. Sequential ---
echo ""
echo "── 11. Sequential Flag ──"

SID=$(next_session)
setup_state "$SID" "$DEFAULT_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "instructions" "no_change" false '[{"text":"first read","mode":"execute"},{"text":"then fix","mode":"execute"}]' "[]" true)
output=$(run "$SID" "first read the file, then fix the bug")
has           "sequential context injected"         "$output" "strictly sequential"

SID=$(next_session)
setup_state "$SID" "$DEFAULT_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "instructions" "no_change" false '[{"text":"fix it","mode":"execute"}]' "[]" false)
output=$(run "$SID" "fix the bug")
excludes      "no sequential without flag"          "$output" "strictly sequential"

# --- 12. Skills ---
echo ""
echo "── 12. Skills Detection ──"

SID=$(next_session)
setup_state "$SID" "$DEFAULT_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "instructions" "no_change" false '[{"text":"commit","mode":"execute"}]' '["/commit"]')
output=$(run "$SID" "/commit")
has           "skill in output"                     "$output" "/commit"

SID=$(next_session)
setup_state "$SID" "$DEFAULT_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "instructions" "no_change" false '[{"text":"fix it","mode":"execute"}]' "[]")
output=$(run "$SID" "fix the bug")
excludes      "no skills when none detected"        "$output" "Skills to execute"

# --- 13. Session notes ---
echo ""
echo "── 13. Session Notes ──"

SID=$(next_session)
setup_state "$SID" "$DEFAULT_STATE"
NOTES='["Agent spawned agents in solo mode"]'
export MOCK_CLAUDE_RESPONSE=$(mock "instructions" "no_change" false '[{"text":"fix it","mode":"execute"}]' "[]" false "$NOTES")
output=$(run "$SID" "I said solo, why did you spawn agents?")
NOTES_LEN=$(jq '.notes | length' "/tmp/claude-session-state-${SID}" 2>/dev/null)
if [[ "$NOTES_LEN" == "1" ]]; then
    printf "  PASS  notes saved (1 note)\n"; PASS=$((PASS+1))
else
    printf "  FAIL  notes saved (got %s notes, want 1)\n" "$NOTES_LEN"; FAIL=$((FAIL+1))
fi

SID=$(next_session)
setup_state "$SID" '{"state":"executing","intent":"instructions","approach":"subagents","notes":["existing note"],"commit_requested":false,"validation_phase":0}'
export MOCK_CLAUDE_RESPONSE=$(mock "instructions" "no_change" false '[{"text":"fix it","mode":"execute"}]' "[]" false "[]")
output=$(run "$SID" "fix it")
NOTES_LEN=$(jq '.notes | length' "/tmp/claude-session-state-${SID}" 2>/dev/null)
if [[ "$NOTES_LEN" == "1" ]]; then
    printf "  PASS  empty notes preserve existing\n"; PASS=$((PASS+1))
else
    printf "  FAIL  empty notes preserve existing (got %s notes, want 1)\n" "$NOTES_LEN"; FAIL=$((FAIL+1))
fi

# --- 14. Recommended agents ---
echo ""
echo "── 14. Recommended Agents ──"

SID=$(next_session)
setup_state "$SID" "$DEFAULT_STATE"
AGENTS='[{"agent":"debugger","reason":"bug investigation"}]'
export MOCK_CLAUDE_RESPONSE=$(mock "instructions" "no_change" false '[{"text":"fix the broken endpoint","mode":"execute"}]' "[]" false "[]" "$AGENTS")
output=$(run "$SID" "this endpoint is broken")
has           "recommended agent in output"         "$output" "@debugger"

SID=$(next_session)
setup_state "$SID" "$DEFAULT_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "instructions" "no_change" false '[{"text":"fix it","mode":"execute"}]' "[]" false "[]" "[]")
output=$(run "$SID" "fix it")
excludes      "no agents when none recommended"     "$output" "Recommended agents"

# --- 15. State notifications ---
echo ""
echo "── 15. State Change Notifications ──"

SID=$(next_session)
setup_state "$SID" "$DEFAULT_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "proposal_request" "no_change" false '[{"text":"analyze","mode":"execute"}]')
output=$(run "$SID" "analyze this")
has           "state change notified"               "$output" "state changed from"

SID=$(next_session)
setup_state "$SID" "$DEFAULT_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "instructions" "no_change" false '[{"text":"fix it","mode":"execute"}]')
output=$(run "$SID" "fix it")
excludes      "no notification when state unchanged" "$output" "state changed"

SID=$(next_session)
setup_state "$SID" "$DEFAULT_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "question")
output=$(run "$SID" "why?")
has           "intent change notified"              "$output" "intent changed"

# --- 16. Context injection by state ---
echo ""
echo "── 16. Context Injection by State ──"

# Shared rules appear in all states
SID=$(next_session)
setup_state "$SID" "$DEFAULT_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "proposal_request" "no_change" false '[{"text":"propose a fix","mode":"execute"}]')
output=$(run "$SID" "propose a fix")
has           "proposing: has proposal block"        "$output" "Present a full, complete proposal"
has           "proposing: has shared scope rule"     "$output" "Never change the scope"
has           "proposing: has shared decision rule"  "$output" "decision maker"
excludes      "proposing: no execution rules"        "$output" "Research the codebase before editing"

# Executing state injects execution rules + shared
SID=$(next_session)
setup_state "$SID" "$DEFAULT_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "instructions" "no_change" false '[{"text":"fix it","mode":"execute"}]')
output=$(run "$SID" "fix it")
has           "executing: has execution rules"       "$output" "Research the codebase before editing"
has           "executing: has shared scope rule"     "$output" "Never change the scope"
excludes      "executing: no proposal block"         "$output" "Present a full, complete proposal"

# Subagent rules only in subagents/team approach
SID=$(next_session)
setup_state "$SID" "$DEFAULT_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "instructions" "no_change" false '[{"text":"fix it","mode":"execute"}]')
output=$(run "$SID" "fix it")
has           "subagents approach: has subagent rules" "$output" "WHY and WHAT only"

SID=$(next_session)
setup_state "$SID" '{"state":"executing","intent":"instructions","approach":"solo","notes":[],"commit_requested":false,"validation_phase":0}'
export MOCK_CLAUDE_RESPONSE=$(mock "instructions" "no_change" false '[{"text":"fix it","mode":"execute"}]')
output=$(run "$SID" "fix it")
excludes      "solo approach: no subagent rules"     "$output" "WHY and WHAT only"

# Question doesn't inject state-specific rules beyond question context
SID=$(next_session)
setup_state "$SID" "$PROPOSING_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "question")
output=$(run "$SID" "why this approach?")
has           "question: has question context"       "$output" "This is a question. Answer it"
excludes      "question in proposing: no proposal block" "$output" "Present a full, complete proposal"

SID=$(next_session)
setup_state "$SID" "$DEFAULT_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "question")
output=$(run "$SID" "what is this?")
excludes      "question in executing: no exec rules"  "$output" "Research the codebase before editing"

# Approval injects start-work + execution rules
SID=$(next_session)
setup_state "$SID" "$PROPOSING_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "approval")
output=$(run "$SID" "go ahead")
has           "approval: has start-work"             "$output" "Approval. Start work"
has           "approval: has execution rules"        "$output" "Research the codebase before editing"

# --- 17. Correction context ---
echo ""
echo "── 17. Correction Context ──"

SID=$(next_session)
setup_state "$SID" "$PROPOSING_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "correction" "no_change" false '[{"text":"use Y instead","mode":"correction"}]')
output=$(run "$SID" "that's wrong, use Y instead")
has           "correction: complete response"        "$output" "complete response in the same format"
has           "correction: unresolved questions"     "$output" "unresolved questions"
has           "correction in proposing: proposal block" "$output" "Present a full, complete proposal"

SID=$(next_session)
setup_state "$SID" "$DEFAULT_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "correction" "no_change" false '[{"text":"use Y instead","mode":"correction"}]')
output=$(run "$SID" "that's wrong, use Y instead")
has           "correction in executing: exec rules"  "$output" "Research the codebase before editing"

# --- 18. Validation phase reset ---
echo ""
echo "── 18. Validation Phase ──"

SID=$(next_session)
setup_state "$SID" '{"state":"executing","intent":"instructions","approach":"subagents","notes":[],"commit_requested":false,"validation_phase":3}'
export MOCK_CLAUDE_RESPONSE=$(mock "instructions" "no_change" false '[{"text":"do thing","mode":"execute"}]')
run "$SID" "do the thing" > /dev/null
assert_state  "validation_phase reset to 0"        "$SID" "validation_phase" "0"

# --- 19. Fresh session ---
echo ""
echo "── 19. Fresh Session (no state file) ──"

SID=$(next_session)
export MOCK_CLAUDE_RESPONSE=$(mock "instructions" "no_change" false '[{"text":"fix this","mode":"execute"}]')
output=$(run "$SID" "fix this bug")
assert_state  "creates state file"                 "$SID" "intent" "instructions"
assert_state  "defaults to executing"              "$SID" "state" "executing"
assert_state  "defaults approach"                  "$SID" "approach" "subagents"
has_output    "produces output"                    "$output"

# Fresh session with proposal request
SID=$(next_session)
export MOCK_CLAUDE_RESPONSE=$(mock "proposal_request" "no_change" false '[{"text":"analyze this","mode":"execute"}]')
output=$(run "$SID" "analyze this bug and propose a fix")
assert_state  "fresh proposal_request → proposing" "$SID" "state" "proposing"

# --- 20. Classifier failure ---
echo ""
echo "── 20. Classifier Failure (graceful) ──"

SID=$(next_session)
setup_state "$SID" "$DEFAULT_STATE"
export MOCK_CLAUDE_RESPONSE=""
output=$(run "$SID" "fix the bug")
no_output     "empty response passes through" "$output"

SID=$(next_session)
setup_state "$SID" "$DEFAULT_STATE"
export MOCK_CLAUDE_RESPONSE="not json at all"
output=$(run "$SID" "fix the bug")
no_output     "invalid json passes through" "$output"

SID=$(next_session)
setup_state "$SID" "$DEFAULT_STATE"
export MOCK_CLAUDE_RESPONSE='{"structured_output":null}'
output=$(run "$SID" "fix the bug")
no_output     "null structured_output passes through" "$output"

# --- 21. Null field handling ---
echo ""
echo "── 21. Null Field Handling ──"

SID=$(next_session)
setup_state "$SID" "$DEFAULT_STATE"
# Missing commit_requested → defaults to false
export MOCK_CLAUDE_RESPONSE='{"structured_output":{"intent":"instructions","instructions":[{"text":"do thing","mode":"execute"}],"approach_change":"no_change","skills":[],"sequential":false,"session_notes":[],"recommended_agents":[]}}'
output=$(run "$SID" "do thing")
assert_state  "null commit_requested defaults to false" "$SID" "commit_requested" "false"

# --- 22. Edit blocker integration ---
echo ""
echo "── 22. Edit Blocker Integration ──"

EDIT_BLOCKER="${SCRIPT%/*}/block-edits-during-proposal.sh"
[[ ! -f "$EDIT_BLOCKER" ]] && EDIT_BLOCKER="/tmp/block-edits-during-proposal-v2.sh"
if [[ -f "$EDIT_BLOCKER" ]]; then
    # Proposing state blocks edits
    SID=$(next_session)
    setup_state "$SID" "$PROPOSING_STATE"
    blocker_exit=0
    echo "{\"session_id\":\"$SID\",\"tool_input\":{\"file_path\":\"/tmp/test.js\"}}" | bash "$EDIT_BLOCKER" 2>/dev/null || blocker_exit=$?
    if [[ "$blocker_exit" -eq 2 ]]; then
        printf "  PASS  proposing state blocks edits\n"; PASS=$((PASS+1))
    else
        printf "  FAIL  proposing state blocks edits (exit=%d, want 2)\n" "$blocker_exit"; FAIL=$((FAIL+1))
    fi

    # Executing state allows edits
    SID=$(next_session)
    setup_state "$SID" "$DEFAULT_STATE"
    blocker_exit=0
    echo "{\"session_id\":\"$SID\",\"tool_input\":{\"file_path\":\"/tmp/test.js\"}}" | bash "$EDIT_BLOCKER" 2>/dev/null || blocker_exit=$?
    if [[ "$blocker_exit" -eq 0 ]]; then
        printf "  PASS  executing state allows edits\n"; PASS=$((PASS+1))
    else
        printf "  FAIL  executing state allows edits (exit=%d, want 0)\n" "$blocker_exit"; FAIL=$((FAIL+1))
    fi

    # No state file allows edits
    SID=$(next_session)
    blocker_exit=0
    echo "{\"session_id\":\"$SID\",\"tool_input\":{\"file_path\":\"/tmp/test.js\"}}" | bash "$EDIT_BLOCKER" 2>/dev/null || blocker_exit=$?
    if [[ "$blocker_exit" -eq 0 ]]; then
        printf "  PASS  no state file allows edits\n"; PASS=$((PASS+1))
    else
        printf "  FAIL  no state file allows edits (exit=%d, want 0)\n" "$blocker_exit"; FAIL=$((FAIL+1))
    fi

    # Planning artifacts bypass blocker even in proposing
    SID=$(next_session)
    setup_state "$SID" "$PROPOSING_STATE"
    blocker_exit=0
    echo "{\"session_id\":\"$SID\",\"tool_input\":{\"file_path\":\"/home/user/.claude/shaping/plan.md\"}}" | bash "$EDIT_BLOCKER" 2>/dev/null || blocker_exit=$?
    if [[ "$blocker_exit" -eq 0 ]]; then
        printf "  PASS  planning artifacts bypass blocker\n"; PASS=$((PASS+1))
    else
        printf "  FAIL  planning artifacts bypass blocker (exit=%d, want 0)\n" "$blocker_exit"; FAIL=$((FAIL+1))
    fi

    # Full round-trip: classifier sets proposing → blocker blocks
    SID=$(next_session)
    export MOCK_CLAUDE_RESPONSE=$(mock "proposal_request" "no_change" false '[{"text":"analyze this","mode":"execute"}]')
    run "$SID" "analyze this and propose a fix" > /dev/null
    blocker_exit=0
    echo "{\"session_id\":\"$SID\",\"tool_input\":{\"file_path\":\"/tmp/test.js\"}}" | bash "$EDIT_BLOCKER" 2>/dev/null || blocker_exit=$?
    if [[ "$blocker_exit" -eq 2 ]]; then
        printf "  PASS  round-trip: proposal_request → blocker fires\n"; PASS=$((PASS+1))
    else
        printf "  FAIL  round-trip: proposal_request → blocker fires (exit=%d, want 2)\n" "$blocker_exit"; FAIL=$((FAIL+1))
    fi

    # Full round-trip: approval clears proposing → blocker allows
    SID=$(next_session)
    setup_state "$SID" "$PROPOSING_STATE"
    export MOCK_CLAUDE_RESPONSE=$(mock "approval")
    run "$SID" "go ahead" > /dev/null
    blocker_exit=0
    echo "{\"session_id\":\"$SID\",\"tool_input\":{\"file_path\":\"/tmp/test.js\"}}" | bash "$EDIT_BLOCKER" 2>/dev/null || blocker_exit=$?
    if [[ "$blocker_exit" -eq 0 ]]; then
        printf "  PASS  round-trip: approval → blocker allows\n"; PASS=$((PASS+1))
    else
        printf "  FAIL  round-trip: approval → blocker allows (exit=%d, want 0)\n" "$blocker_exit"; FAIL=$((FAIL+1))
    fi
else
    printf "  SKIP  edit blocker not found at %s\n" "$EDIT_BLOCKER"
fi

# --- 23. Correction with forward-looking language ---
echo ""
echo "── 23. Correction vs Instructions Edge Cases ──"

# Correction with forward-looking language should stay correction (state unchanged)
SID=$(next_session)
setup_state "$SID" "$PROPOSING_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "correction" "no_change" false '[{"text":"use X and then deploy it","mode":"correction"}]')
output=$(run "$SID" "no, just use X and then deploy it")
assert_state  "correction+forward language: state stays proposing" "$SID" "state" "proposing"
assert_state  "correction+forward language: intent is correction" "$SID" "intent" "correction"

# Instructions during proposing stay in proposing (refine the proposal)
SID=$(next_session)
setup_state "$SID" "$PROPOSING_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "instructions" "no_change" false '[{"text":"add a new endpoint for users","mode":"execute"}]')
output=$(run "$SID" "add a new endpoint for users")
assert_state  "instructions during proposing: stays proposing" "$SID" "state" "proposing"

# --- 24. Escape sequence handling ---
echo ""
echo "── 24. Auto State (Mixed Intents) ──"

# Correction with execute instructions → auto
SID=$(next_session)
setup_state "$SID" "$PROPOSING_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "correction" "no_change" false '[{"text":"use X instead","mode":"correction"},{"text":"fix the typo in line 12","mode":"execute"}]')
output=$(run "$SID" "that's wrong, use X instead. Also fix the typo in line 12 right now")
assert_state  "correction+execute → auto"          "$SID" "state" "auto"
has           "auto: has mixed intent context"      "$output" "mixed intents"
has           "auto: has correction context"        "$output" "corrected your previous output"

# Correction without execute → stays current state
SID=$(next_session)
setup_state "$SID" "$PROPOSING_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "correction" "no_change" false '[{"text":"use X instead","mode":"correction"}]')
output=$(run "$SID" "that's wrong, use X instead")
assert_state  "correction without execute → stays" "$SID" "state" "proposing"

# Question with execute instructions → auto
SID=$(next_session)
setup_state "$SID" "$PROPOSING_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "question" "no_change" false '[{"text":"why this approach","mode":"question"},{"text":"also run the tests","mode":"execute"}]')
output=$(run "$SID" "why this approach? also run the tests")
assert_state  "question+execute → auto"            "$SID" "state" "auto"

# Question without execute → stays current state
SID=$(next_session)
setup_state "$SID" "$PROPOSING_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "question")
output=$(run "$SID" "why this approach?")
assert_state  "question without execute → stays"   "$SID" "state" "proposing"

# Auto state allows edits (edit blocker integration)
if [[ -f "$EDIT_BLOCKER" ]]; then
    SID=$(next_session)
    setup_state "$SID" '{"state":"auto","intent":"correction","approach":"subagents","notes":[],"commit_requested":false,"validation_phase":0}'
    blocker_exit=0
    echo "{\"session_id\":\"$SID\",\"tool_input\":{\"file_path\":\"/tmp/test.js\"}}" | bash "$EDIT_BLOCKER" 2>/dev/null || blocker_exit=$?
    if [[ "$blocker_exit" -eq 0 ]]; then
        printf "  PASS  auto state allows edits\n"; PASS=$((PASS+1))
    else
        printf "  FAIL  auto state allows edits (exit=%d, want 0)\n" "$blocker_exit"; FAIL=$((FAIL+1))
    fi
fi

# Round-trip: correction+execute during proposing → auto → blocker allows
SID=$(next_session)
setup_state "$SID" "$PROPOSING_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "correction" "no_change" false '[{"text":"wrong","mode":"correction"},{"text":"fix typo","mode":"execute"}]')
run "$SID" "wrong, also fix the typo" > /dev/null
if [[ -f "$EDIT_BLOCKER" ]]; then
    blocker_exit=0
    echo "{\"session_id\":\"$SID\",\"tool_input\":{\"file_path\":\"/tmp/test.js\"}}" | bash "$EDIT_BLOCKER" 2>/dev/null || blocker_exit=$?
    if [[ "$blocker_exit" -eq 0 ]]; then
        printf "  PASS  round-trip: correction+execute → auto → edits allowed\n"; PASS=$((PASS+1))
    else
        printf "  FAIL  round-trip: correction+execute → auto → edits allowed (exit=%d, want 0)\n" "$blocker_exit"; FAIL=$((FAIL+1))
    fi
fi

echo ""
echo "── 25. Escape Sequence Handling ──"

SID=$(next_session)
setup_state "$SID" "$DEFAULT_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "instructions" "no_change" false '[{"text":"fix the \\n newline handling","mode":"execute"}]')
output=$(run "$SID" 'fix the \n newline handling')
has           "literal backslash-n preserved"       "$output" "newline handling"

# --- 26. commit_requested reset on next message ---
echo ""
echo "── 26. Commit Requested Reset ──"

SID=$(next_session)
setup_state "$SID" '{"state":"executing","intent":"instructions","approach":"subagents","notes":[],"commit_requested":true,"validation_phase":0}'
export MOCK_CLAUDE_RESPONSE=$(mock "instructions" "no_change" false '[{"text":"fix the bug","mode":"execute"}]')
run "$SID" "fix the bug" > /dev/null
assert_state  "commit_requested resets on next message" "$SID" "commit_requested" "false"

# --- 27. Combined approach + state change ---
echo ""
echo "── 27. Combined Approach + State Change ──"

SID=$(next_session)
setup_state "$SID" "$DEFAULT_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "proposal_request" "solo" false '[{"text":"propose a fix solo","mode":"execute"}]')
run "$SID" "go solo and propose a fix" > /dev/null
assert_state  "approach+state: approach=solo"      "$SID" "approach" "solo"
assert_state  "approach+state: state=proposing"    "$SID" "state" "proposing"

# --- 28. Team approach gets subagent rules ---
echo ""
echo "── 28. Team Approach Subagent Rules ──"

SID=$(next_session)
setup_state "$SID" '{"state":"executing","intent":"instructions","approach":"team","notes":[],"commit_requested":false,"validation_phase":0}'
export MOCK_CLAUDE_RESPONSE=$(mock "instructions" "no_change" false '[{"text":"fix it","mode":"execute"}]')
output=$(run "$SID" "fix it")
has           "team approach: has subagent rules"   "$output" "WHY and WHAT only"

# --- 29. Question + execute: no "Don't edit" contradiction ---
echo ""
echo "── 29. Question + Execute No Contradiction ──"

SID=$(next_session)
setup_state "$SID" "$DEFAULT_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "question" "no_change" false '[{"text":"why this","mode":"question"},{"text":"fix that","mode":"execute"}]')
output=$(run "$SID" "why this? also fix that")
excludes      "mixed question: no 'dont edit'"     "$output" "Don't edit the code"
has           "mixed question: has action items"    "$output" "action items"

SID=$(next_session)
setup_state "$SID" "$DEFAULT_STATE"
export MOCK_CLAUDE_RESPONSE=$(mock "question" "no_change" false '[{"text":"why this","mode":"question"}]')
output=$(run "$SID" "why this?")
has           "pure question: has 'dont edit'"      "$output" "Don't edit the code"

# ============================================================================
# Summary
# ============================================================================

echo ""
echo "================================================================"
printf "  TOTAL: %d passed, %d failed out of %d\n" "$PASS" "$FAIL" "$((PASS+FAIL))"
echo "================================================================"
[[ $FAIL -gt 0 ]] && exit 1
exit 0
