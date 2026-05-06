#!/bin/bash
# session-state-test.sh — exhaustive tests for session-state.sh
# Run: ./session-state-test.sh
# Output: per-test pass/fail, then totals.

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
HELPER="${SCRIPT_DIR}/session-state.sh"

if [ ! -f "$HELPER" ]; then
    echo "ERROR: $HELPER not found" >&2
    exit 1
fi

# ============================================================================
# Test framework
# ============================================================================
TESTS_RUN=0
TESTS_PASSED=0
TESTS_FAILED=0
TEST_ROOT=""
TEST_OUT=""
TEST_ERR=""
TEST_CODE=0

setup_test() {
    TEST_ROOT=$(mktemp -d -t session-state-test.XXXXXX)
    export CLAUDE_DATA_ROOT="$TEST_ROOT"
    # Isolate the projects-root glob so _resolve_parent_id doesn't see real
    # subagent transcripts under ~/.claude/projects during tests.
    export CLAUDE_PROJECTS_ROOT="$TEST_ROOT/projects"
    mkdir -p "$CLAUDE_PROJECTS_ROOT"
    unset CLAUDE_SESSION_ID
}

teardown_test() {
    [ -n "$TEST_ROOT" ] && rm -rf "$TEST_ROOT"
    TEST_ROOT=""
    unset CLAUDE_DATA_ROOT
    unset CLAUDE_PROJECTS_ROOT
    unset CLAUDE_SESSION_ID
}

# Stage a fake subagent transcript so _resolve_parent_id can find a parent.
# Echoes the transcript path on stdout.
stage_subagent_transcript() {
    local agent_id="$1"
    local parent_id="$2"
    local project="${3:-test-proj}"
    local path="$CLAUDE_PROJECTS_ROOT/$project/$parent_id/subagents/${agent_id}.jsonl"
    mkdir -p "$(dirname "$path")"
    : > "$path"
    echo "$path"
}

ok() {
    TESTS_RUN=$((TESTS_RUN + 1))
    TESTS_PASSED=$((TESTS_PASSED + 1))
    printf '  \033[32m✓\033[0m %s\n' "$1"
}

fail() {
    TESTS_RUN=$((TESTS_RUN + 1))
    TESTS_FAILED=$((TESTS_FAILED + 1))
    printf '  \033[31m✗\033[0m %s\n' "$1"
    [ -n "${2:-}" ] && printf '      \033[31m%s\033[0m\n' "$2"
}

assert_eq() {
    local actual="$1" expected="$2" msg="$3"
    if [ "$actual" = "$expected" ]; then
        ok "$msg"
    else
        fail "$msg" "expected='$expected' got='$actual'"
    fi
}

assert_exit() {
    local code="$1" expected="$2" msg="$3"
    if [ "$code" -eq "$expected" ]; then
        ok "$msg"
    else
        fail "$msg" "expected exit=$expected got=$code"
    fi
}

assert_match() {
    local actual="$1" pattern="$2" msg="$3"
    if [[ "$actual" =~ $pattern ]]; then
        ok "$msg"
    else
        fail "$msg" "value='$actual' did not match /$pattern/"
    fi
}

assert_file_exists() {
    if [ -f "$1" ]; then ok "$2"; else fail "$2" "file not found: $1"; fi
}

assert_dir_exists() {
    if [ -d "$1" ]; then ok "$2"; else fail "$2" "directory not found: $1"; fi
}

assert_not_exists() {
    if [ ! -e "$1" ]; then ok "$2"; else fail "$2" "should not exist: $1"; fi
}

section() {
    printf '\n\033[1m─── %s ───\033[0m\n' "$1"
}

ss() { bash "$HELPER" "$@"; }

# Capture stdout, stderr, exit code in TEST_OUT, TEST_ERR, TEST_CODE.
capture() {
    local tmp_out tmp_err
    tmp_out=$(mktemp); tmp_err=$(mktemp)
    "$@" >"$tmp_out" 2>"$tmp_err"
    TEST_CODE=$?
    TEST_OUT=$(cat "$tmp_out")
    TEST_ERR=$(cat "$tmp_err")
    rm -f "$tmp_out" "$tmp_err"
}

# ============================================================================
# PATH RESOLUTION
# ============================================================================
section "PATH RESOLUTION"

setup_test
unset CLAUDE_DATA_ROOT
out=$(ss get --path data-root)
assert_eq "$out" "$HOME/.claude" "path data-root with no override → \$HOME/.claude"
teardown_test

setup_test
out=$(ss get --path data-root)
assert_eq "$out" "$TEST_ROOT" "path data-root respects \$CLAUDE_DATA_ROOT"
teardown_test

setup_test
out=$(ss get --path sessions)
assert_eq "$out" "$TEST_ROOT/sessions" "path sessions → data-root/sessions"
teardown_test

setup_test
out=$(ss get --path shaping)
assert_eq "$out" "$TEST_ROOT/shaping" "path shaping → data-root/shaping"
teardown_test

setup_test
ss start "main-001" --transcript-path "/foo/main-001.jsonl"
out=$(ss get --path "main-001")
assert_eq "$out" "$TEST_ROOT/sessions/main-001" "path --session for main → sessions/<id>"
teardown_test

setup_test
ss start "main-002" --transcript-path "/foo/projects/p/main-002/main-002.jsonl"
ss start "agent-abc" --transcript-path "/foo/projects/p/main-002/subagents/agent-abc.jsonl"
out=$(ss get --path "agent-abc")
assert_eq "$out" "$TEST_ROOT/sessions/main-002/subagents/agent-abc" "path --session for subagent → nested path"
teardown_test

setup_test
capture ss get --path "nonexistent"
assert_exit "$TEST_CODE" 1 "path --session on missing session exits 1"
teardown_test

setup_test
ss start "main-003" --transcript-path "/foo/main-003.jsonl"
export CLAUDE_SESSION_ID="main-003"
out=$(ss get --path)
assert_eq "$out" "$TEST_ROOT/sessions/main-003" "path with no args uses \$CLAUDE_SESSION_ID"
teardown_test

setup_test
capture ss get --path
assert_exit "$TEST_CODE" 1 "path with no args and no \$CLAUDE_SESSION_ID exits 1"
teardown_test

setup_test
capture ss get --path bogus-target
assert_exit "$TEST_CODE" 1 "path with unknown target exits 1"
teardown_test

# ============================================================================
# INIT
# ============================================================================
section "INIT"

setup_test
ss start "main-100" --transcript-path "/foo/main-100.jsonl"
assert_file_exists "$TEST_ROOT/sessions/main-100/state.json" "init main creates state.json"
teardown_test

setup_test
ss start "main-101" --transcript-path "/foo/main-101.jsonl"
state="$TEST_ROOT/sessions/main-101/state.json"
assert_eq "$(jq -r .role "$state")"             "main"         "init main: role=main"
assert_eq "$(jq -r .session_id "$state")"       "main-101"     "init main: session_id"
assert_eq "$(jq -r .parent_session_id "$state")" "null"        "init main: parent_session_id=null"
assert_eq "$(jq -r .approach "$state")"         "solo"         "init main: approach=solo"
assert_eq "$(jq -r .state "$state")"            "proposing"    "init main: state=proposing"
assert_eq "$(jq -r .intent "$state")"           "instructions" "init main: intent=instructions"
assert_eq "$(jq -r .commit_requested "$state")" "false"        "init main: commit_requested=false"
assert_eq "$(jq -r '.notes | length' "$state")" "0"            "init main: notes=[]"
assert_eq "$(jq -r .validation_phase "$state")" "0"            "init main: validation_phase=0"
assert_eq "$(jq -r .pane "$state")"             "null"         "init main: pane=null"
assert_eq "$(jq -r '."tmux-pane"' "$state")"    "null"         "init main: tmux-pane=null"
assert_eq "$(jq -r .schema_version "$state")"   "1"            "init main: schema_version=1"
teardown_test

setup_test
ss start "main-200" --transcript-path "/foo/projects/p/main-200/main-200.jsonl"
ss start "agent-xyz" --transcript-path "/foo/projects/p/main-200/subagents/agent-xyz.jsonl"
assert_file_exists "$TEST_ROOT/sessions/main-200/subagents/agent-xyz/state.json" "subagent init creates nested state.json"
teardown_test

setup_test
ss start "main-201" --transcript-path "/foo/projects/p/main-201/main-201.jsonl"
ss start "agent-bbb" --transcript-path "/foo/projects/p/main-201/subagents/agent-bbb.jsonl"
state="$TEST_ROOT/sessions/main-201/subagents/agent-bbb/state.json"
assert_eq "$(jq -r .role "$state")"             "subagent"  "subagent: role=subagent"
assert_eq "$(jq -r .session_id "$state")"       "agent-bbb" "subagent: session_id"
assert_eq "$(jq -r .parent_session_id "$state")" "main-201" "subagent: parent_session_id"
assert_eq "$(jq -r 'has("approach") | tostring' "$state")"        "false" "subagent: omits approach"
assert_eq "$(jq -r 'has("state") | tostring' "$state")"           "false" "subagent: omits state"
assert_eq "$(jq -r 'has("intent") | tostring' "$state")"          "false" "subagent: omits intent"
assert_eq "$(jq -r 'has("notes") | tostring' "$state")"           "false" "subagent: omits notes"
assert_eq "$(jq -r 'has("validation_phase") | tostring' "$state")" "false" "subagent: omits validation_phase"
assert_eq "$(jq -r 'has("commit_requested") | tostring' "$state")" "false" "subagent: omits commit_requested"
assert_eq "$(jq -r 'has("pane") | tostring' "$state")"            "true"  "subagent: keeps pane"
assert_eq "$(jq -r 'has("tmux-pane") | tostring' "$state")"       "true"  "subagent: keeps tmux-pane"
assert_eq "$(jq -r .schema_version "$state")"                     "1"     "subagent: schema_version=1"
teardown_test

setup_test
ss start "agent-ooo" --transcript-path "/foo/projects/p/main-300/subagents/agent-ooo.jsonl"
assert_dir_exists  "$TEST_ROOT/sessions/main-300"                            "out-of-order: parent dir created"
assert_file_exists "$TEST_ROOT/sessions/main-300/subagents/agent-ooo/state.json" "out-of-order: subagent state created"
assert_not_exists  "$TEST_ROOT/sessions/main-300/state.json"                 "out-of-order: parent state.json not yet"
ss start "main-300" --transcript-path "/foo/projects/p/main-300/main-300.jsonl"
assert_file_exists "$TEST_ROOT/sessions/main-300/state.json"                 "out-of-order: parent init after subagent works"
assert_file_exists "$TEST_ROOT/sessions/main-300/subagents/agent-ooo/state.json" "out-of-order: subagent state survives parent init"
teardown_test

setup_test
ss start "main-400" --transcript-path "/foo/main-400.jsonl"
ss set "main-400" approach team
ss start "main-400" --transcript-path "/foo/main-400.jsonl"
out=$(ss get "main-400" approach)
assert_eq "$out" "team" "init is idempotent — does not overwrite existing state"
teardown_test

setup_test
ss start "main-500"
state="$TEST_ROOT/sessions/main-500/state.json"
assert_eq "$(jq -r .role "$state")" "main" "init without --transcript-path → main role"
teardown_test

setup_test
ss start "main-501" --transcript-path "/random/path.jsonl"
state="$TEST_ROOT/sessions/main-501/state.json"
assert_eq "$(jq -r .role "$state")" "main" "init with non-subagent transcript path → main role"
teardown_test

setup_test
ss start "main-502" --transcript-path ""
state="$TEST_ROOT/sessions/main-502/state.json"
assert_eq "$(jq -r .role "$state")" "main" "init with empty transcript path → main role"
teardown_test

setup_test
ss start "main-600" --transcript-path "/some/transcript.jsonl"
assert_file_exists "$TEST_ROOT/sessions/main-600/transcript" "init writes transcript file"
out=$(cat "$TEST_ROOT/sessions/main-600/transcript")
assert_eq "$out" "/some/transcript.jsonl" "transcript file contents = path"
teardown_test

setup_test
capture ss start
assert_exit "$TEST_CODE" 1 "init with no args exits 1"
teardown_test

# ============================================================================
# GET
# ============================================================================
section "GET"

setup_test
ss start "g-1" --transcript-path "/foo/g-1.jsonl"
out=$(ss get "g-1" approach)
assert_eq "$out" "solo" "get returns existing field default"
teardown_test

setup_test
capture ss get "missing-session" approach
assert_eq "$TEST_OUT" ""    "get on missing session: empty stdout"
assert_exit "$TEST_CODE" 0  "get on missing session: exit 0 (soft)"
teardown_test

setup_test
ss start "g-2" --transcript-path "/foo/g-2.jsonl"
out=$(ss get "g-2" not_a_real_field)
assert_eq "$out" "" "get on unknown field returns empty"
teardown_test

setup_test
ss start "g-3" --transcript-path "/foo/g-3.jsonl"
out=$(ss get "g-3" pane)
assert_eq "$out" "" "get on null field returns empty (// empty)"
teardown_test

setup_test
ss start "g-4" --transcript-path "/foo/projects/p/g-main/g-main.jsonl"
ss start "agent-g4" --transcript-path "/foo/projects/p/g-main/subagents/agent-g4.jsonl"
out=$(ss get "agent-g4" role)
assert_eq "$out" "subagent" "get works on subagent state"
teardown_test

setup_test
capture ss get
assert_exit "$TEST_CODE" 1 "get with no args exits 1"
capture ss get only-one-arg
assert_exit "$TEST_CODE" 1 "get with one arg exits 1"
teardown_test

# ============================================================================
# SET
# ============================================================================
section "SET"

setup_test
ss start "s-1" --transcript-path "/foo/s-1.jsonl"
ss set "s-1" approach team
out=$(ss get "s-1" approach)
assert_eq "$out" "team" "set then get round-trips"
teardown_test

setup_test
ss start "s-2" --transcript-path "/foo/s-2.jsonl"
ss set "s-2" approach team
out=$(ss get "s-2" state)
assert_eq "$out" "proposing" "set on one field preserves others"
teardown_test

setup_test
# Lazy-create on never-init'd UUID-shaped session — succeeds, lands flat
ss set "missing-session" approach team
assert_exit "$?" 0 "set on never-init'd UUID-shaped session: lazy-creates flat main"
assert_file_exists "$TEST_ROOT/sessions/missing-session/state.json" "set lazy-create: state.json materializes"
assert_eq "$(ss get missing-session role)"     "main" "set lazy-create: role=main"
assert_eq "$(ss get missing-session approach)" "team" "set lazy-create: value applied"
teardown_test

setup_test
# Lazy-create on never-init'd agent-* WITHOUT staged transcript → errors
capture ss set "agent-orphan" approach team
assert_exit "$TEST_CODE" 1 "set on agent-* without resolvable parent: exit 1"
echo "$TEST_ERR" | grep -q "no resolvable parent" && ok "set on orphan agent-*: clear error" || fail "orphan err msg" "got: $TEST_ERR"
teardown_test

setup_test
# Lazy-create on agent-* WITH staged transcript → nests under correct parent
ss start "lc-parent" --transcript-path "/foo/projects/p/lc-parent/lc-parent.jsonl"
stage_subagent_transcript "agent-lcsub" "lc-parent" >/dev/null
ss set "agent-lcsub" approach team
assert_exit "$?" 0 "set on agent-* with staged transcript: lazy-creates nested"
assert_file_exists "$TEST_ROOT/sessions/lc-parent/subagents/agent-lcsub/state.json" "set lazy-create subagent: nested location"
assert_eq "$(ss get agent-lcsub role)"              "subagent"  "set lazy-create subagent: role=subagent"
assert_eq "$(ss get agent-lcsub parent_session_id)" "lc-parent" "set lazy-create subagent: parent_session_id populated from glob"
assert_eq "$(ss get agent-lcsub approach)"          "team"      "set lazy-create subagent: value applied"
teardown_test

setup_test
ss start "s-3" --transcript-path "/foo/s-3.jsonl"
ss set "s-3" commit_requested true
state="$TEST_ROOT/sessions/s-3/state.json"
assert_eq "$(jq -r '.commit_requested | type' "$state")" "boolean" "set parses 'true' as JSON boolean"
assert_eq "$(jq -r '.commit_requested' "$state")"        "true"    "set 'true' value preserved"
teardown_test

setup_test
ss start "s-4" --transcript-path "/foo/s-4.jsonl"
ss set "s-4" validation_phase 3
state="$TEST_ROOT/sessions/s-4/state.json"
assert_eq "$(jq -r '.validation_phase | type' "$state")" "number" "set parses '3' as JSON number"
assert_eq "$(jq -r '.validation_phase' "$state")"        "3"      "set '3' value preserved"
teardown_test

setup_test
ss start "s-5" --transcript-path "/foo/s-5.jsonl"
ss set "s-5" approach "with multiple words"
out=$(ss get "s-5" approach)
assert_eq "$out" "with multiple words" "set handles strings with spaces"
teardown_test

setup_test
ss start "s-6" --transcript-path "/foo/s-6.jsonl"
ss set "s-6" approach 'has"quotes"'
out=$(ss get "s-6" approach)
assert_eq "$out" 'has"quotes"' "set handles strings with embedded quotes"
teardown_test

setup_test
ss start "s-7" --transcript-path "/foo/s-7.jsonl"
ss set "s-7" notes '["a","b","c"]'
state="$TEST_ROOT/sessions/s-7/state.json"
assert_eq "$(jq -r '.notes | type' "$state")"   "array" "set parses array as JSON array"
assert_eq "$(jq -r '.notes | length' "$state")" "3"     "set array preserves length"
assert_eq "$(jq -r '.notes[1]' "$state")"       "b"     "set array preserves element"
teardown_test

setup_test
ss start "s-conc" --transcript-path "/foo/s-conc.jsonl"
(for i in $(seq 1 25); do ss set "s-conc" approach "team-$i"; done) &
(for i in $(seq 1 25); do ss set "s-conc" intent "approval-$i"; done) &
wait
state="$TEST_ROOT/sessions/s-conc/state.json"
capture jq -e . "$state"
assert_exit "$TEST_CODE" 0 "concurrent set: JSON remains valid"
approach_val=$(jq -r .approach "$state")
intent_val=$(jq -r .intent "$state")
assert_match "$approach_val" '^team-[0-9]+$'      "concurrent set: approach landed as one of the values"
assert_match "$intent_val"   '^approval-[0-9]+$'  "concurrent set: intent landed as one of the values"
assert_eq "$(jq -r .state "$state")" "proposing" "concurrent set: untouched field preserved"
teardown_test

setup_test
capture ss set
assert_exit "$TEST_CODE" 1 "set with no args exits 1"
teardown_test

# ============================================================================
# MERGE
# ============================================================================
section "MERGE"

setup_test
ss start "m-1" --transcript-path "/foo/m-1.jsonl"
ss merge "m-1" '{"approach":"team","intent":"approval"}'
assert_eq "$(ss get m-1 approach)" "team"     "merge updates approach"
assert_eq "$(ss get m-1 intent)"   "approval" "merge updates intent"
teardown_test

setup_test
ss start "m-2" --transcript-path "/foo/m-2.jsonl"
ss merge "m-2" '{"approach":"team"}'
assert_eq "$(ss get m-2 state)"        "proposing" "merge preserves state"
assert_eq "$(ss get m-2 schema_version)" "1"       "merge preserves schema_version"
teardown_test

setup_test
ss start "m-3" --transcript-path "/foo/m-3.jsonl"
capture ss merge "m-3" 'this is not json'
assert_exit "$TEST_CODE" 1 "merge with invalid JSON exits 1"
teardown_test

setup_test
# Lazy-create on never-init'd UUID-shaped session — succeeds, lands flat
ss merge "missing" '{"approach":"team","intent":"approval"}'
assert_exit "$?" 0 "merge on never-init'd UUID-shaped session: lazy-creates flat main"
assert_eq "$(ss get missing role)"     "main"     "merge lazy-create: role=main"
assert_eq "$(ss get missing approach)" "team"     "merge lazy-create: approach applied"
assert_eq "$(ss get missing intent)"   "approval" "merge lazy-create: intent applied"
teardown_test

setup_test
capture ss merge "agent-orphan-merge" '{}'
assert_exit "$TEST_CODE" 1 "merge on agent-* without resolvable parent: exit 1"
teardown_test

setup_test
# Lazy-create via merge on agent-* with staged transcript
ss start "mp-parent" --transcript-path "/foo/projects/p/mp-parent/mp-parent.jsonl"
stage_subagent_transcript "agent-mpsub" "mp-parent" >/dev/null
ss merge "agent-mpsub" '{"pane":"zellij-X"}'
assert_exit "$?" 0 "merge on agent-* with staged transcript: lazy-creates nested"
assert_eq "$(ss get agent-mpsub role)"              "subagent"  "merge lazy-create: role=subagent"
assert_eq "$(ss get agent-mpsub parent_session_id)" "mp-parent" "merge lazy-create: parent populated"
assert_eq "$(ss get agent-mpsub pane)"              "zellij-X"  "merge lazy-create: fragment applied"
teardown_test

setup_test
ss start "m-4" --transcript-path "/foo/m-4.jsonl"
ss merge "m-4" '{"commit_requested":true,"validation_phase":2,"notes":["x","y"]}'
state="$TEST_ROOT/sessions/m-4/state.json"
assert_eq "$(jq -r '.commit_requested | type' "$state")"  "boolean" "merge preserves boolean type"
assert_eq "$(jq -r '.validation_phase | type' "$state")"  "number"  "merge preserves number type"
assert_eq "$(jq -r '.notes | type' "$state")"             "array"   "merge preserves array type"
assert_eq "$(jq -r '.notes | length' "$state")"           "2"       "merge preserves array length"
teardown_test

setup_test
capture ss merge
assert_exit "$TEST_CODE" 1 "merge with no args exits 1"
teardown_test

# ============================================================================
# APPEND
# ============================================================================
section "APPEND"

setup_test
ss start "a-1" --transcript-path "/foo/a-1.jsonl"
ss read "a-1" "/path/to/file.ts"
assert_file_exists "$TEST_ROOT/sessions/a-1/reads.jsonl" "append creates reads.jsonl"
teardown_test

setup_test
ss start "a-2" --transcript-path "/foo/a-2.jsonl"
ss read "a-2" "/some/file.ts"
line=$(head -1 "$TEST_ROOT/sessions/a-2/reads.jsonl")
assert_eq "$(echo "$line" | jq -r .path)"      "/some/file.ts" "append entry: .path correct"
assert_eq "$(echo "$line" | jq -r '.ts | type')" "string"      "append entry: .ts is a string"
teardown_test

setup_test
ss start "a-3" --transcript-path "/foo/a-3.jsonl"
for i in $(seq 1 100); do ss read "a-3" "/path/$i"; done
count=$(wc -l < "$TEST_ROOT/sessions/a-3/reads.jsonl" | tr -d ' ')
assert_eq "$count" "100" "100 sequential appends → 100 lines"
teardown_test

setup_test
ss start "a-4" --transcript-path "/foo/a-4.jsonl"
(for i in $(seq 1 50); do ss read "a-4" "/A/$i"; done) &
(for i in $(seq 1 50); do ss read "a-4" "/B/$i"; done) &
wait
count=$(wc -l < "$TEST_ROOT/sessions/a-4/reads.jsonl" | tr -d ' ')
assert_eq "$count" "100" "concurrent appends preserve all entries"
invalid=0
while IFS= read -r line; do
    echo "$line" | jq -e . >/dev/null 2>&1 || invalid=$((invalid + 1))
done < "$TEST_ROOT/sessions/a-4/reads.jsonl"
assert_eq "$invalid" "0" "concurrent appends: every line is valid JSON"
teardown_test

setup_test
ss read "a-5-uninit" "/some/file.ts"
assert_file_exists "$TEST_ROOT/sessions/a-5-uninit/reads.jsonl" "append on uninit session lazy-creates dir + file"
teardown_test

setup_test
ss start "a-7-main" --transcript-path "/foo/projects/p/a-7-main/a-7-main.jsonl"
ss start "agent-a7" --transcript-path "/foo/projects/p/a-7-main/subagents/agent-a7.jsonl"
ss read "agent-a7" "/sub/file.ts"
assert_file_exists "$TEST_ROOT/sessions/a-7-main/subagents/agent-a7/reads.jsonl" "read on subagent — writes nested reads.jsonl"
teardown_test

setup_test
capture ss read
assert_exit "$TEST_CODE" 1 "read with no args exits 1"
capture ss skill
assert_exit "$TEST_CODE" 1 "skill with no args exits 1"
teardown_test

# ----- skills kind -----
setup_test
ss start "as-1" --transcript-path "/foo/as-1.jsonl"
ss skill "as-1" "/cc"
assert_file_exists "$TEST_ROOT/sessions/as-1/skills.jsonl" "append skills creates skills.jsonl"
line=$(head -1 "$TEST_ROOT/sessions/as-1/skills.jsonl")
assert_eq "$(echo "$line" | jq -r .skill)"     "/cc"    "skills entry: .skill correct"
assert_eq "$(echo "$line" | jq -r '.ts | type')" "string" "skills entry: .ts is a string"
assert_eq "$(echo "$line" | jq -r 'has("path") | tostring')" "false" "skills entry: no .path field"
teardown_test

setup_test
ss start "as-2" --transcript-path "/foo/as-2.jsonl"
ss skill "as-2" "/cc"
ss skill "as-2" "/pcc"
ss skill "as-2" "/commit"
count=$(wc -l < "$TEST_ROOT/sessions/as-2/skills.jsonl" | tr -d ' ')
assert_eq "$count" "3" "3 skills appends → 3 lines"
assert_eq "$(sed -n 1p "$TEST_ROOT/sessions/as-2/skills.jsonl" | jq -r .skill)" "/cc"     "skills order: 1st"
assert_eq "$(sed -n 2p "$TEST_ROOT/sessions/as-2/skills.jsonl" | jq -r .skill)" "/pcc"    "skills order: 2nd"
assert_eq "$(sed -n 3p "$TEST_ROOT/sessions/as-2/skills.jsonl" | jq -r .skill)" "/commit" "skills order: 3rd"
teardown_test

setup_test
ss start "as-3" --transcript-path "/foo/as-3.jsonl"
ss read "as-3" "/file.ts"
ss skill "as-3" "/cc"
ss read "as-3" "/other.ts"
ss skill "as-3" "/pcc"
reads_count=$(wc -l < "$TEST_ROOT/sessions/as-3/reads.jsonl" | tr -d ' ')
skills_count=$(wc -l < "$TEST_ROOT/sessions/as-3/skills.jsonl" | tr -d ' ')
assert_eq "$reads_count"  "2" "reads + skills coexist: reads.jsonl has 2 entries"
assert_eq "$skills_count" "2" "reads + skills coexist: skills.jsonl has 2 entries"
teardown_test

setup_test
ss start "as-4" --transcript-path "/foo/as-4.jsonl"
(for i in $(seq 1 50); do ss skill "as-4" "/skill-A-$i"; done) &
(for i in $(seq 1 50); do ss skill "as-4" "/skill-B-$i"; done) &
wait
count=$(wc -l < "$TEST_ROOT/sessions/as-4/skills.jsonl" | tr -d ' ')
assert_eq "$count" "100" "concurrent skills appends preserve all 100 entries"
invalid=0
while IFS= read -r line; do
    echo "$line" | jq -e . >/dev/null 2>&1 || invalid=$((invalid + 1))
done < "$TEST_ROOT/sessions/as-4/skills.jsonl"
assert_eq "$invalid" "0" "concurrent skills appends: every line is valid JSON"
teardown_test

setup_test
ss start "as-main" --transcript-path "/foo/projects/p/as-main/as-main.jsonl"
ss start "agent-as" --transcript-path "/foo/projects/p/as-main/subagents/agent-as.jsonl"
ss skill "agent-as" "/subagents"
assert_file_exists "$TEST_ROOT/sessions/as-main/subagents/agent-as/skills.jsonl" "subagent skills append → nested skills.jsonl"
assert_not_exists "$TEST_ROOT/sessions/as-main/skills.jsonl" "subagent append does NOT touch parent's skills.jsonl"
teardown_test

setup_test
ss skill "as-uninit" "/cc"
assert_file_exists "$TEST_ROOT/sessions/as-uninit/skills.jsonl" "skills append on uninit session lazy-creates"
teardown_test

# ============================================================================
# FIND-BY-PANE
# ============================================================================
section "FIND-BY-PANE"

setup_test
ss start "f-1" --transcript-path "/foo/f-1.jsonl"
ss set "f-1" pane "zellij-pane-A"
out=$(ss find-by-pane "zellij-pane-A")
assert_eq "$out" "f-1" "find-by-pane (default) finds zellij pane"
teardown_test

setup_test
ss start "f-2" --transcript-path "/foo/f-2.jsonl"
ss set "f-2" tmux-pane "Coding:1:0"
out=$(ss find-by-pane --tmux "Coding:1:0")
assert_eq "$out" "f-2" "find-by-pane --tmux finds tmux pane"
teardown_test

setup_test
ss start "f-3" --transcript-path "/foo/f-3.jsonl"
ss set "f-3" tmux-pane "tmux-only"
out=$(ss find-by-pane "tmux-only")
assert_eq "$out" "" "find-by-pane (default) ignores .tmux-pane field"
teardown_test

setup_test
ss start "f-4" --transcript-path "/foo/f-4.jsonl"
ss set "f-4" pane "zellij-only"
out=$(ss find-by-pane --tmux "zellij-only")
assert_eq "$out" "" "find-by-pane --tmux ignores .pane field"
teardown_test

setup_test
ss start "f-5" --transcript-path "/foo/f-5.jsonl"
out=$(ss find-by-pane "no-such-pane")
assert_eq "$out" "" "find-by-pane: no match returns empty"
teardown_test

setup_test
out=$(ss find-by-pane "anything")
assert_eq "$out" "" "find-by-pane with zero sessions returns empty"
teardown_test

setup_test
ss start "f-main" --transcript-path "/foo/projects/p/f-main/f-main.jsonl"
ss start "agent-f" --transcript-path "/foo/projects/p/f-main/subagents/agent-f.jsonl"
ss set "agent-f" pane "subagent-pane"
out=$(ss find-by-pane "subagent-pane")
assert_eq "$out" "agent-f" "find-by-pane finds subagent's zellij pane"
teardown_test

setup_test
ss start "f-A" --transcript-path "/foo/f-A.jsonl"
ss start "f-B" --transcript-path "/foo/f-B.jsonl"
ss start "f-C" --transcript-path "/foo/f-C.jsonl"
ss set "f-B" pane "wanted-pane"
out=$(ss find-by-pane "wanted-pane")
assert_eq "$out" "f-B" "find-by-pane returns the right session among many"
teardown_test

setup_test
capture ss find-by-pane
assert_exit "$TEST_CODE" 1 "find-by-pane with no args exits 1"
capture ss find-by-pane --tmux
assert_exit "$TEST_CODE" 1 "find-by-pane --tmux with no pane_id exits 1"
teardown_test

# ============================================================================
# LIST
# ============================================================================
section "LIST"

setup_test
out=$(ss list)
assert_eq "$out" "" "list with no sessions returns empty"
teardown_test

setup_test
ss start "l-1" --transcript-path "/foo/l-1.jsonl"
ss start "l-2" --transcript-path "/foo/l-2.jsonl"
ss start "l-3" --transcript-path "/foo/l-3.jsonl"
out=$(ss list | sort | tr '\n' ' ')
assert_eq "$out" "l-1 l-2 l-3 " "list returns all main sessions"
teardown_test

setup_test
ss start "l-main" --transcript-path "/foo/projects/p/l-main/l-main.jsonl"
ss start "agent-l1" --transcript-path "/foo/projects/p/l-main/subagents/agent-l1.jsonl"
out=$(ss list | sort | tr '\n' ' ')
assert_eq "$out" "l-main " "list (no flag) excludes subagents"
teardown_test

setup_test
ss start "l-main2" --transcript-path "/foo/projects/p/l-main2/l-main2.jsonl"
ss start "agent-A" --transcript-path "/foo/projects/p/l-main2/subagents/agent-A.jsonl"
ss start "agent-B" --transcript-path "/foo/projects/p/l-main2/subagents/agent-B.jsonl"
out=$(ss list --subagents "l-main2" | sort | tr '\n' ' ')
assert_eq "$out" "agent-A agent-B " "list --subagents returns all children"
teardown_test

setup_test
ss start "l-no-subs" --transcript-path "/foo/l-no-subs.jsonl"
out=$(ss list --subagents "l-no-subs")
assert_eq "$out" "" "list --subagents on session w/ no children returns empty"
teardown_test

setup_test
out=$(ss list --subagents "no-such-session")
assert_eq "$out" "" "list --subagents on missing session returns empty"
teardown_test

setup_test
capture ss list --subagents
assert_exit "$TEST_CODE" 1 "list --subagents without parent_id exits 1"
teardown_test

# ============================================================================
# CLEANUP
# ============================================================================
section "CLEANUP"

setup_test
ss start "c-1" --transcript-path "/foo/c-1.jsonl"
ss end "c-1"
assert_not_exists "$TEST_ROOT/sessions/c-1" "cleanup removes main session dir"
teardown_test

setup_test
ss start "c-main" --transcript-path "/foo/projects/p/c-main/c-main.jsonl"
ss start "agent-c1" --transcript-path "/foo/projects/p/c-main/subagents/agent-c1.jsonl"
ss start "agent-c2" --transcript-path "/foo/projects/p/c-main/subagents/agent-c2.jsonl"
ss end "c-main"
assert_not_exists "$TEST_ROOT/sessions/c-main"                          "cleanup main: parent dir gone"
assert_not_exists "$TEST_ROOT/sessions/c-main/subagents/agent-c1"        "cleanup main: cascade removes agent-c1"
assert_not_exists "$TEST_ROOT/sessions/c-main/subagents/agent-c2"        "cleanup main: cascade removes agent-c2"
teardown_test

setup_test
ss start "c-main2" --transcript-path "/foo/projects/p/c-main2/c-main2.jsonl"
ss start "agent-c3" --transcript-path "/foo/projects/p/c-main2/subagents/agent-c3.jsonl"
ss end "agent-c3"
assert_not_exists "$TEST_ROOT/sessions/c-main2/subagents/agent-c3" "cleanup subagent: subagent dir gone"
assert_dir_exists "$TEST_ROOT/sessions/c-main2"                    "cleanup subagent: parent dir intact"
teardown_test

setup_test
capture ss end "nonexistent"
assert_exit "$TEST_CODE" 0 "cleanup on missing session exits 0 (idempotent)"
teardown_test

setup_test
capture ss end
assert_exit "$TEST_CODE" 1 "cleanup with no args exits 1"
teardown_test

# ============================================================================
# DISPATCH
# ============================================================================
section "DISPATCH"

setup_test
capture ss
assert_exit "$TEST_CODE" 1 "no command exits 1"
teardown_test

setup_test
capture ss bogus-command
assert_exit "$TEST_CODE" 1 "unknown command exits 1"
teardown_test

# ============================================================================
# REGRESSION — bugs caught in second-round audit (must stay green)
# ============================================================================
section "REGRESSION: jq null/false detection"

setup_test
ss start "rj-1" --transcript-path "/foo/rj-1.jsonl"
ss set "rj-1" approach "null"
state="$TEST_ROOT/sessions/rj-1/state.json"
assert_eq "$(jq -r '.approach | type' "$state")" "null"    "set 'null' parses as JSON null (not string)"
ss set "rj-1" commit_requested "false"
assert_eq "$(jq -r '.commit_requested | type' "$state")" "boolean" "set 'false' parses as JSON boolean"
assert_eq "$(jq -r '.commit_requested' "$state")" "false"          "set 'false' value preserved"
ss set "rj-1" commit_requested "true"
assert_eq "$(jq -r '.commit_requested | type' "$state")" "boolean" "set 'true' still parses as boolean (regression check)"
teardown_test

section "REGRESSION: corrupt state.json — get soft, set/merge refuse to clobber"

setup_test
mkdir -p "$TEST_ROOT/sessions/rc-1"
echo "this is not json" > "$TEST_ROOT/sessions/rc-1/state.json"
capture ss get "rc-1" approach
assert_exit "$TEST_CODE" 0 "get on corrupt state.json: exit 0 (soft)"
assert_eq "$TEST_OUT" ""   "get on corrupt state.json: empty stdout"
teardown_test

setup_test
mkdir -p "$TEST_ROOT/sessions/rc-2"
echo "this is not json" > "$TEST_ROOT/sessions/rc-2/state.json"
ss set "rc-2" approach team
assert_exit "$?" 0 "set on corrupt state.json: heals to defaults + applies (exit 0)"
state="$TEST_ROOT/sessions/rc-2/state.json"
jq -e . "$state" >/dev/null 2>&1 && ok "set on corrupt: file is valid JSON after heal" || fail "rc-2 heal" "still corrupt"
assert_eq "$(jq -r .approach "$state")"   "team" "set on corrupt: applied value persists"
assert_eq "$(jq -r .role "$state")"       "main" "set on corrupt: healed with main defaults"
assert_eq "$(jq -r .session_id "$state")" "rc-2" "set on corrupt: session_id populated"
teardown_test

setup_test
mkdir -p "$TEST_ROOT/sessions/rc-3"
echo "garbage" > "$TEST_ROOT/sessions/rc-3/state.json"
ss merge "rc-3" '{"approach":"team","intent":"approval"}'
assert_exit "$?" 0 "merge on corrupt state.json: heals to defaults + applies (exit 0)"
state="$TEST_ROOT/sessions/rc-3/state.json"
jq -e . "$state" >/dev/null 2>&1 && ok "merge on corrupt: file is valid JSON after heal" || fail "rc-3 heal" "still corrupt"
assert_eq "$(jq -r .approach "$state")" "team"     "merge on corrupt: approach applied"
assert_eq "$(jq -r .intent "$state")"   "approval" "merge on corrupt: intent applied"
teardown_test

section "REGRESSION: merge fragment must be JSON object"

setup_test
ss start "rm-1" --transcript-path "/foo/rm-1.jsonl"
state="$TEST_ROOT/sessions/rm-1/state.json"
capture ss merge "rm-1" '"just-a-string"'
assert_exit "$TEST_CODE" 1 "merge with JSON string: exit 1"
assert_eq "$(jq -r 'type' "$state")" "object" "merge non-object: state.json root remains object"
assert_eq "$(jq -r .approach "$state")" "solo" "merge non-object: existing fields preserved"
capture ss merge "rm-1" '[1,2,3]'
assert_exit "$TEST_CODE" 1 "merge with JSON array: exit 1"
capture ss merge "rm-1" '42'
assert_exit "$TEST_CODE" 1 "merge with JSON number: exit 1"
capture ss merge "rm-1" 'null'
assert_exit "$TEST_CODE" 1 "merge with JSON null: exit 1"
capture ss merge "rm-1" 'true'
assert_exit "$TEST_CODE" 1 "merge with JSON boolean: exit 1"
ss merge "rm-1" '{}'
assert_exit "$?" 0 "merge with empty object {}: exit 0 (still works)"
teardown_test

section "REGRESSION: session_id traversal blocked"

setup_test
mkdir -p "$TEST_ROOT/outside-target"
echo "sentinel" > "$TEST_ROOT/outside-target/sentinel"
capture ss start "../outside-target" --transcript-path "/foo/x.jsonl"
assert_exit "$TEST_CODE" 1 "init traversal session_id: exit 1"
assert_not_exists "$TEST_ROOT/outside-target/state.json" "init traversal: no state.json written outside"
assert_file_exists "$TEST_ROOT/outside-target/sentinel"  "init traversal: external file untouched"
teardown_test

setup_test
mkdir -p "$TEST_ROOT/outside-cleanup"
echo "ok" > "$TEST_ROOT/outside-cleanup/sentinel"
capture ss end "../outside-cleanup"
assert_exit "$TEST_CODE" 1 "cleanup traversal session_id: exit 1"
assert_file_exists "$TEST_ROOT/outside-cleanup/sentinel" "cleanup traversal: external dir survived"
teardown_test

setup_test
for bad in '../foo' '..//foo' '/etc/passwd' '' 'has space' 'has;semi' 'has$dollar' 'has`tick' 'has*glob' 'has?q' '-leading-dash' '--transcript-path'; do
    capture ss start "$bad" --transcript-path "/foo/x.jsonl"
    assert_exit "$TEST_CODE" 1 "init '$bad' rejected"
    capture ss end "$bad"
    assert_exit "$TEST_CODE" 1 "cleanup '$bad' rejected"
    capture ss get "$bad" approach
    assert_exit "$TEST_CODE" 1 "get '$bad' rejected"
    capture ss set "$bad" approach team
    assert_exit "$TEST_CODE" 1 "set '$bad' rejected"
    capture ss merge "$bad" '{}'
    assert_exit "$TEST_CODE" 1 "merge '$bad' rejected"
    capture ss read "$bad" /x
    assert_exit "$TEST_CODE" 1 "append '$bad' rejected"
    capture ss get --path "$bad"
    assert_exit "$TEST_CODE" 1 "path --session '$bad' rejected"
    capture ss list --subagents "$bad"
    assert_exit "$TEST_CODE" 1 "list --subagents '$bad' rejected"
done
teardown_test

setup_test
# agent-* needs a transcript path that carries parent info (or staged transcript)
ss start "p-for-hex" --transcript-path "/foo/p-for-hex.jsonl"
ss start "agent-a1655989b93d3e4bf" --transcript-path "/foo/projects/p/p-for-hex/subagents/agent-a1655989b93d3e4bf.jsonl"
assert_exit "$?" 0 "valid agent-{hex} session_id: accepted (with subagent transcript path)"
ss start "0c76b915-3e91-442d-a033-9900ae991a75" --transcript-path "/foo/x.jsonl"
assert_exit "$?" 0 "valid UUID session_id: accepted"
ss start "main_with_underscores" --transcript-path "/foo/x.jsonl"
assert_exit "$?" 0 "valid snake_case session_id: accepted"
teardown_test

setup_test
# Malformed transcript path with .. in the parent slot — _parse_parent_from_transcript
# rejects it (multi-/subagents/ count). agent-evil falls back to glob, which finds
# nothing under the test's CLAUDE_PROJECTS_ROOT, so init errors loud rather than
# treating as main. Either way: no traversal escape.
capture ss start "agent-evil" --transcript-path "/foo/projects/p/../subagents/agent-evil.jsonl"
assert_exit "$TEST_CODE" 1 "evil transcript: agent-* with no resolvable parent → exit 1 (no fallback)"
[ ! -d "$TEST_ROOT/sessions/../subagents" ] && ok "evil transcript: no traversal escape" || fail "evil transcript: traversal happened"
[ ! -d "$TEST_ROOT/sessions/agent-evil" ] && ok "evil transcript: no flat-main fallback created" || fail "evil transcript" "fallback dir exists"
teardown_test

section "REGRESSION: weird state.json shapes degrade gracefully"

setup_test
mkdir -p "$TEST_ROOT/sessions/ws-1"
echo '[1,2,3]' > "$TEST_ROOT/sessions/ws-1/state.json"
capture ss get "ws-1" approach
assert_exit "$TEST_CODE" 0 "get on array-shaped state.json: soft exit (read-only, no heal)"
assert_eq "$TEST_OUT" ""   "get on array-shaped state.json: empty stdout"
ss set "ws-1" approach team
assert_exit "$?" 0 "set on array-shaped state.json: heals to defaults + applies"
state="$TEST_ROOT/sessions/ws-1/state.json"
assert_eq "$(jq -r 'type' "$state")"      "object" "set on array: state.json now an object"
assert_eq "$(jq -r .approach "$state")"   "team"   "set on array: applied value persists"
teardown_test

setup_test
mkdir -p "$TEST_ROOT/sessions/ws-2"
: > "$TEST_ROOT/sessions/ws-2/state.json"
capture ss get "ws-2" approach
assert_exit "$TEST_CODE" 0 "get on empty state.json: soft exit (read-only, no heal)"
assert_eq "$TEST_OUT" ""   "get on empty state.json: empty stdout"
ss set "ws-2" approach team
assert_exit "$?" 0 "set on empty state.json: heals to defaults + applies"
state="$TEST_ROOT/sessions/ws-2/state.json"
[ -s "$state" ] && ok "set on empty: file no longer empty" || fail "ws-2 heal" "still empty"
assert_eq "$(jq -r .approach "$state")" "team" "set on empty: applied value persists"
teardown_test

setup_test
mkdir -p "$TEST_ROOT/sessions/ws-3/state.json"
capture ss get "ws-3" approach
assert_exit "$TEST_CODE" 0 "get when state.json is a directory: soft exit"
assert_eq "$TEST_OUT" ""   "get state.json-is-dir: empty stdout"
teardown_test

setup_test
mkdir -p "$TEST_ROOT/sessions/ws-4"
ln -s /nonexistent-target "$TEST_ROOT/sessions/ws-4/state.json"
capture ss get "ws-4" approach
assert_exit "$TEST_CODE" 0 "get on broken-symlink state.json: soft exit"
teardown_test

section "REGRESSION: unicode and large values"

setup_test
ss start "uni-1" --transcript-path "/foo/uni-1.jsonl"
ss set "uni-1" approach "🦊 fox"
assert_eq "$(ss get uni-1 approach)" "🦊 fox" "emoji values round-trip"
ss set "uni-1" approach "中文测试"
assert_eq "$(ss get uni-1 approach)" "中文测试" "CJK values round-trip"
teardown_test

setup_test
ss start "big-1" --transcript-path "/foo/big-1.jsonl"
big=$(head -c 10000 /dev/urandom | base64 | head -c 10000)
ss set "big-1" approach "$big"
out=$(ss get "big-1" approach)
[ "${#out}" -ge 9000 ] && ok "10KB string value round-trips (${#out} chars)" || fail "10KB string round-trip" "got ${#out} chars"
teardown_test

section "REGRESSION: concurrent init + concurrent cleanup"

setup_test
(ss start "ci-1" --transcript-path "/foo/ci-1.jsonl") &
(ss start "ci-1" --transcript-path "/foo/ci-1.jsonl") &
wait
state="$TEST_ROOT/sessions/ci-1/state.json"
assert_file_exists "$state" "concurrent init same id: state.json exists"
jq -e . "$state" >/dev/null 2>&1 && ok "concurrent init same id: state.json valid JSON" || fail "concurrent init" "state corrupt"
teardown_test

setup_test
ss start "csa-1" --transcript-path "/foo/csa-1.jsonl"
(for i in $(seq 1 30); do ss set "csa-1" approach "A-$i"; done) &
(for i in $(seq 1 30); do ss read "csa-1" "/p/$i"; done) &
wait
state="$TEST_ROOT/sessions/csa-1/state.json"
jq -e . "$state" >/dev/null 2>&1 && ok "concurrent set+append: state.json valid" || fail "set+append" "state corrupt"
reads_count=$(wc -l < "$TEST_ROOT/sessions/csa-1/reads.jsonl" | tr -d ' ')
assert_eq "$reads_count" "30" "concurrent set+append: 30 reads preserved"
teardown_test

setup_test
ss start "ccs-1" --transcript-path "/foo/ccs-1.jsonl"
(ss end "ccs-1") &
(ss set "ccs-1" approach team 2>/dev/null) &
wait
# After lazy-create, set may resurrect the dir if it ran after cleanup. Either
# outcome is non-corrupting: dir gone (cleanup won) OR dir present with valid
# state.json (set lazy-recreated). Verify no half-state.
if [ ! -d "$TEST_ROOT/sessions/ccs-1" ]; then
    ok "concurrent cleanup+set: cleanup won — dir cleaned"
else
    state="$TEST_ROOT/sessions/ccs-1/state.json"
    jq -e . "$state" >/dev/null 2>&1 && ok "concurrent cleanup+set: set lazy-recreated with valid JSON" || fail "cleanup race" "dir present but state corrupt"
fi
teardown_test

section "REGRESSION: --transcript-path requires a value (was infinite-loop)"

setup_test
out=$(timeout 5 bash "$HELPER" start "tp-1" --transcript-path 2>&1)
code=$?
assert_exit "$code" 1 "init --transcript-path with no value: exit 1 (no hang)"
echo "$out" | grep -q "requires a value" && ok "init --transcript-path: emits clear error" || fail "tp error msg" "got: $out"
teardown_test

section "REGRESSION: flag handling edge cases"

setup_test
ss start "fh-1" --transcript-path "/a.jsonl" --transcript-path "/b.jsonl"
assert_exit "$?" 0 "init with repeated --transcript-path: doesn't crash"
content=$(cat "$TEST_ROOT/sessions/fh-1/transcript")
case "$content" in /a.jsonl|/b.jsonl) ok "init repeated --transcript-path: one value picked ($content)";; *) fail "repeated tp" "got '$content'";; esac
teardown_test

setup_test
capture ss start "fh-2" --bogus-flag
assert_exit "$TEST_CODE" 1 "init with unknown flag: exit 1"
teardown_test

section "REGRESSION: subagent path resolution via CLAUDE_SESSION_ID"

setup_test
ss start "sp-main" --transcript-path "/foo/projects/p/sp-main/sp-main.jsonl"
ss start "agent-sp" --transcript-path "/foo/projects/p/sp-main/subagents/agent-sp.jsonl"
export CLAUDE_SESSION_ID="agent-sp"
out=$(ss get --path)
assert_eq "$out" "$TEST_ROOT/sessions/sp-main/subagents/agent-sp" "path with subagent CLAUDE_SESSION_ID resolves nested"
unset CLAUDE_SESSION_ID
teardown_test

section "REGRESSION: cleanup of symlinked session dir doesn't follow link"

setup_test
mkdir -p "$TEST_ROOT/elsewhere"
echo "external" > "$TEST_ROOT/elsewhere/sentinel"
mkdir -p "$TEST_ROOT/sessions"
ln -s "$TEST_ROOT/elsewhere" "$TEST_ROOT/sessions/sym-1"
echo '{"session_id":"sym-1","role":"main","approach":"team","schema_version":1}' > "$TEST_ROOT/sessions/sym-1/state.json"
assert_eq "$(ss get sym-1 approach)" "team" "get works through symlinked session dir"
ss end "sym-1"
assert_not_exists "$TEST_ROOT/sessions/sym-1" "cleanup removes symlink"
assert_dir_exists  "$TEST_ROOT/elsewhere"     "cleanup does NOT remove symlink target dir"
assert_file_exists "$TEST_ROOT/elsewhere/sentinel" "cleanup preserves files in symlink target"
teardown_test

section "REGRESSION: subagent isolation under concurrency"

setup_test
ss start "iso-main" --transcript-path "/foo/projects/p/iso-main/iso-main.jsonl"
ss start "agent-iso1" --transcript-path "/foo/projects/p/iso-main/subagents/agent-iso1.jsonl"
ss start "agent-iso2" --transcript-path "/foo/projects/p/iso-main/subagents/agent-iso2.jsonl"
(for i in $(seq 1 30); do ss read "agent-iso1" "/A/$i"; done) &
(for i in $(seq 1 30); do ss read "agent-iso2" "/B/$i"; done) &
(for i in $(seq 1 30); do ss set "iso-main" approach "M-$i"; done) &
wait
c1=$(wc -l < "$TEST_ROOT/sessions/iso-main/subagents/agent-iso1/reads.jsonl" | tr -d ' ')
c2=$(wc -l < "$TEST_ROOT/sessions/iso-main/subagents/agent-iso2/reads.jsonl" | tr -d ' ')
assert_eq "$c1" "30" "concurrent subagent isolation: agent-iso1 has 30 reads"
assert_eq "$c2" "30" "concurrent subagent isolation: agent-iso2 has 30 reads"
jq -e . "$TEST_ROOT/sessions/iso-main/state.json" >/dev/null 2>&1 && ok "subagent isolation: parent state.json valid" || fail "iso parent" "corrupt"
teardown_test

section "MULTI-AGENT: parent ↔ child relationships"

setup_test
ss start "rel-parent" --transcript-path "/foo/projects/p/rel-parent/rel-parent.jsonl"
ss start "agent-rel1" --transcript-path "/foo/projects/p/rel-parent/subagents/agent-rel1.jsonl"
ss start "agent-rel2" --transcript-path "/foo/projects/p/rel-parent/subagents/agent-rel2.jsonl"
parent_id_from_main=$(ss get "rel-parent" session_id)
parent_id_in_sub1=$(ss get "agent-rel1" parent_session_id)
parent_id_in_sub2=$(ss get "agent-rel2" parent_session_id)
assert_eq "$parent_id_in_sub1" "rel-parent"          "subagent 1: parent_session_id points to rel-parent"
assert_eq "$parent_id_in_sub2" "rel-parent"          "subagent 2: parent_session_id points to rel-parent"
assert_eq "$parent_id_in_sub1" "$parent_id_from_main" "subagent 1: parent_session_id == parent's session_id"
assert_eq "$(ss get agent-rel1 role)"       "subagent"    "subagent 1: role"
assert_eq "$(ss get agent-rel1 session_id)" "agent-rel1"  "subagent 1: own session_id"
assert_eq "$(ss get agent-rel2 role)"       "subagent"    "subagent 2: role"
assert_eq "$(ss get agent-rel2 session_id)" "agent-rel2"  "subagent 2: own session_id"
out=$(ss list --subagents "rel-parent" | sort | tr '\n' ' ')
assert_eq "$out" "agent-rel1 agent-rel2 " "list --subagents reflects parent ↔ children link"
teardown_test

setup_test
ss start "p-A" --transcript-path "/foo/projects/p/p-A/p-A.jsonl"
ss start "p-B" --transcript-path "/foo/projects/p/p-B/p-B.jsonl"
ss start "agent-A1" --transcript-path "/foo/projects/p/p-A/subagents/agent-A1.jsonl"
ss start "agent-B1" --transcript-path "/foo/projects/p/p-B/subagents/agent-B1.jsonl"
assert_eq "$(ss get agent-A1 parent_session_id)" "p-A" "agent-A1 → parent p-A"
assert_eq "$(ss get agent-B1 parent_session_id)" "p-B" "agent-B1 → parent p-B"
out_a=$(ss list --subagents "p-A" | sort | tr '\n' ' ')
out_b=$(ss list --subagents "p-B" | sort | tr '\n' ' ')
assert_eq "$out_a" "agent-A1 " "p-A's children list isolated"
assert_eq "$out_b" "agent-B1 " "p-B's children list isolated"
out=$(ss list | sort | tr '\n' ' ')
assert_eq "$out" "p-A p-B " "top-level list shows only mains across multiple parent trees"
teardown_test

setup_test
ss start "p-X" --transcript-path "/foo/projects/p/p-X/p-X.jsonl"
ss start "p-Y" --transcript-path "/foo/projects/p/p-Y/p-Y.jsonl"
ss start "agent-X1" --transcript-path "/foo/projects/p/p-X/subagents/agent-X1.jsonl"
ss start "agent-X2" --transcript-path "/foo/projects/p/p-X/subagents/agent-X2.jsonl"
ss start "agent-Y1" --transcript-path "/foo/projects/p/p-Y/subagents/agent-Y1.jsonl"
ss end "p-X"
assert_not_exists "$TEST_ROOT/sessions/p-X"             "cleanup p-X removes parent dir"
assert_not_exists "$TEST_ROOT/sessions/p-X/subagents/agent-X1" "cleanup p-X cascades to agent-X1"
assert_not_exists "$TEST_ROOT/sessions/p-X/subagents/agent-X2" "cleanup p-X cascades to agent-X2"
assert_dir_exists "$TEST_ROOT/sessions/p-Y"             "cleanup p-X leaves p-Y intact"
assert_dir_exists "$TEST_ROOT/sessions/p-Y/subagents/agent-Y1" "cleanup p-X leaves p-Y's subagents intact"
teardown_test

setup_test
ss start "p-Z" --transcript-path "/foo/projects/p/p-Z/p-Z.jsonl"
ss start "agent-Z1" --transcript-path "/foo/projects/p/p-Z/subagents/agent-Z1.jsonl"
ss start "agent-Z2" --transcript-path "/foo/projects/p/p-Z/subagents/agent-Z2.jsonl"
ss end "agent-Z1"
assert_not_exists "$TEST_ROOT/sessions/p-Z/subagents/agent-Z1" "cleanup subagent: subagent removed"
assert_dir_exists "$TEST_ROOT/sessions/p-Z/subagents/agent-Z2" "cleanup subagent: sibling intact"
assert_dir_exists "$TEST_ROOT/sessions/p-Z"                   "cleanup subagent: parent intact"
assert_eq "$(ss get p-Z approach)" "solo" "cleanup subagent: parent state.json still readable"
teardown_test

section "MULTI-AGENT: cross-session concurrency"

setup_test
for sid in m-a m-b m-c m-d; do
    ss start "$sid" --transcript-path "/foo/$sid.jsonl"
done
( for i in $(seq 1 30); do ss set    "m-a" approach "A-$i"; done ) &
( for i in $(seq 1 30); do ss read "m-b"    "/B/$i"; done ) &
( for i in $(seq 1 30); do ss skill "m-c"   "/C-$i"; done ) &
( for i in $(seq 1 30); do ss merge  "m-d" "{\"intent\":\"D-$i\"}"; done ) &
wait
for sid in m-a m-b m-c m-d; do
    state="$TEST_ROOT/sessions/$sid/state.json"
    jq -e . "$state" >/dev/null 2>&1 && ok "multi-main: $sid state.json valid" || fail "multi-main $sid" "corrupt"
done
assert_match "$(ss get m-a approach)" '^A-[0-9]+$' "m-a: approach landed"
assert_eq "$(ss get m-a intent)"      "instructions" "m-a: intent unchanged (cross-talk check)"
b_reads=$(wc -l < "$TEST_ROOT/sessions/m-b/reads.jsonl" | tr -d ' ')
assert_eq "$b_reads" "30" "m-b: reads.jsonl has 30 entries"
[ ! -f "$TEST_ROOT/sessions/m-b/skills.jsonl" ] && ok "m-b: no skills.jsonl (isolation)" || fail "m-b" "skills.jsonl appeared"
c_skills=$(wc -l < "$TEST_ROOT/sessions/m-c/skills.jsonl" | tr -d ' ')
assert_eq "$c_skills" "30" "m-c: skills.jsonl has 30 entries"
[ ! -f "$TEST_ROOT/sessions/m-c/reads.jsonl" ] && ok "m-c: no reads.jsonl (isolation)" || fail "m-c" "reads.jsonl appeared"
assert_match "$(ss get m-d intent)" '^D-[0-9]+$' "m-d: intent landed"
assert_eq "$(ss get m-d approach)" "solo" "m-d: approach unchanged (cross-talk check)"
teardown_test

setup_test
ss start "tree-A" --transcript-path "/foo/projects/p/tree-A/tree-A.jsonl"
ss start "tree-B" --transcript-path "/foo/projects/p/tree-B/tree-B.jsonl"
ss start "agent-A1" --transcript-path "/foo/projects/p/tree-A/subagents/agent-A1.jsonl"
ss start "agent-A2" --transcript-path "/foo/projects/p/tree-A/subagents/agent-A2.jsonl"
ss start "agent-B1" --transcript-path "/foo/projects/p/tree-B/subagents/agent-B1.jsonl"
ss start "agent-B2" --transcript-path "/foo/projects/p/tree-B/subagents/agent-B2.jsonl"
( for i in $(seq 1 25); do ss set    "tree-A"   approach "PA-$i"; done ) &
( for i in $(seq 1 25); do ss set    "tree-B"   approach "PB-$i"; done ) &
( for i in $(seq 1 25); do ss read "agent-A1"    "/A1/$i"; done ) &
( for i in $(seq 1 25); do ss skill "agent-A2"   "/A2-$i"; done ) &
( for i in $(seq 1 25); do ss read "agent-B1"    "/B1/$i"; done ) &
( for i in $(seq 1 25); do ss skill "agent-B2"   "/B2-$i"; done ) &
wait
for sid in tree-A tree-B agent-A1 agent-A2 agent-B1 agent-B2; do
    dir=$(ss get --path "$sid")
    jq -e . "$dir/state.json" >/dev/null 2>&1 && ok "cross-tree: $sid state.json valid" || fail "$sid" "corrupt"
done
assert_eq "$(wc -l < "$TEST_ROOT/sessions/tree-A/subagents/agent-A1/reads.jsonl"  | tr -d ' ')" "25" "agent-A1 reads: isolated count"
assert_eq "$(wc -l < "$TEST_ROOT/sessions/tree-A/subagents/agent-A2/skills.jsonl" | tr -d ' ')" "25" "agent-A2 skills: isolated count"
assert_eq "$(wc -l < "$TEST_ROOT/sessions/tree-B/subagents/agent-B1/reads.jsonl"  | tr -d ' ')" "25" "agent-B1 reads: isolated count"
assert_eq "$(wc -l < "$TEST_ROOT/sessions/tree-B/subagents/agent-B2/skills.jsonl" | tr -d ' ')" "25" "agent-B2 skills: isolated count"
assert_eq "$(ss get agent-A1 parent_session_id)" "tree-A" "post-concurrency: agent-A1 → tree-A"
assert_eq "$(ss get agent-B2 parent_session_id)" "tree-B" "post-concurrency: agent-B2 → tree-B"
crosstalk=$(grep -c '/B' "$TEST_ROOT/sessions/tree-A/subagents/agent-A1/reads.jsonl" || true)
assert_eq "$crosstalk" "0" "agent-A1 reads: no /B paths leaked across trees"
assert_match "$(ss get tree-A approach)" '^PA-[0-9]+$' "tree-A approach: PA-N"
assert_match "$(ss get tree-B approach)" '^PB-[0-9]+$' "tree-B approach: PB-N"
teardown_test

setup_test
( ss start "race-parent" --transcript-path "/foo/projects/p/race-parent/race-parent.jsonl" ) &
( ss start "agent-race1" --transcript-path "/foo/projects/p/race-parent/subagents/agent-race1.jsonl" ) &
( ss start "agent-race2" --transcript-path "/foo/projects/p/race-parent/subagents/agent-race2.jsonl" ) &
wait
assert_file_exists "$TEST_ROOT/sessions/race-parent/state.json"                      "race init: parent state.json"
assert_file_exists "$TEST_ROOT/sessions/race-parent/subagents/agent-race1/state.json" "race init: agent-race1 state.json"
assert_file_exists "$TEST_ROOT/sessions/race-parent/subagents/agent-race2/state.json" "race init: agent-race2 state.json"
assert_eq "$(ss get agent-race1 parent_session_id)" "race-parent" "race init: agent-race1 → race-parent"
assert_eq "$(ss get agent-race2 parent_session_id)" "race-parent" "race init: agent-race2 → race-parent"
assert_eq "$(ss get race-parent role)"              "main"        "race init: parent role=main"
teardown_test

setup_test
for sid in fp-1 fp-2 fp-3 fp-4 fp-5; do
    ss start "$sid" --transcript-path "/foo/$sid.jsonl"
    ss set "$sid" pane "pane-$sid"
done
( for i in $(seq 1 20); do ss find-by-pane "pane-fp-3" >/dev/null; done ) &
( for i in $(seq 1 20); do ss set "fp-1" pane "moved-$i" 2>/dev/null; done ) &
( for i in $(seq 1 20); do ss find-by-pane "pane-fp-5" >/dev/null; done ) &
wait
assert_eq "$(ss find-by-pane pane-fp-3)" "fp-3" "post-concurrency find: fp-3"
assert_eq "$(ss find-by-pane pane-fp-5)" "fp-5" "post-concurrency find: fp-5"
final_pane=$(ss get fp-1 pane)
assert_match "$final_pane" '^moved-[0-9]+$' "post-concurrency: fp-1 pane was relocated to moved-N"
teardown_test

setup_test
ss start "live-A" --transcript-path "/foo/live-A.jsonl"
ss start "live-B" --transcript-path "/foo/live-B.jsonl"
( for i in $(seq 1 30); do ss read "live-B" "/B/$i"; done ) &
( sleep 0.05; ss end "live-A" ) &
wait
assert_not_exists "$TEST_ROOT/sessions/live-A" "cleanup-vs-live: live-A gone"
b_reads=$(wc -l < "$TEST_ROOT/sessions/live-B/reads.jsonl" | tr -d ' ')
assert_eq "$b_reads" "30" "cleanup-vs-live: live-B got all 30 entries despite parallel cleanup"
teardown_test

setup_test
for sid in s-1 s-2 s-3 s-4 s-5; do
    ss start "$sid" --transcript-path "/foo/projects/p/$sid/$sid.jsonl"
    ss start "agent-${sid}-x" --transcript-path "/foo/projects/p/$sid/subagents/agent-${sid}-x.jsonl"
    ss start "agent-${sid}-y" --transcript-path "/foo/projects/p/$sid/subagents/agent-${sid}-y.jsonl"
done
for sid in s-1 s-2 s-3 s-4 s-5; do
    ( for i in $(seq 1 10); do ss read "$sid" "/main-$sid/$i"; done ) &
    ( for i in $(seq 1 10); do ss read "agent-${sid}-x" "/x-$sid/$i"; done ) &
    ( for i in $(seq 1 10); do ss skill "agent-${sid}-y" "/y-$sid-$i"; done ) &
done
wait
all_correct=1
for sid in s-1 s-2 s-3 s-4 s-5; do
    main_reads=$(wc -l < "$TEST_ROOT/sessions/$sid/reads.jsonl" 2>/dev/null | tr -d ' ')
    x_reads=$(wc -l < "$TEST_ROOT/sessions/$sid/subagents/agent-${sid}-x/reads.jsonl" 2>/dev/null | tr -d ' ')
    y_skills=$(wc -l < "$TEST_ROOT/sessions/$sid/subagents/agent-${sid}-y/skills.jsonl" 2>/dev/null | tr -d ' ')
    [ "$main_reads" = "10" ] && [ "$x_reads" = "10" ] && [ "$y_skills" = "10" ] || { all_correct=0; break; }
done
assert_eq "$all_correct" "1" "stress 15-session concurrent: all 15 jsonl files have exactly 10 entries each"
for sid in s-1 s-2 s-3 s-4 s-5; do
    assert_eq "$(ss get agent-${sid}-x parent_session_id)" "$sid" "stress: agent-${sid}-x → $sid"
    assert_eq "$(ss get agent-${sid}-y parent_session_id)" "$sid" "stress: agent-${sid}-y → $sid"
done
for sid in s-1 s-2 s-3 s-4 s-5; do
    out=$(ss list --subagents "$sid" | sort | tr '\n' ' ')
    assert_eq "$out" "agent-${sid}-x agent-${sid}-y " "stress: $sid lists exactly its own 2 subagents"
done
teardown_test

section "REGRESSION: grandchild subagents rejected (Claude Code doesn't nest)"

setup_test
ss start "gc-main" --transcript-path "/foo/projects/p/gc-main/gc-main.jsonl"
# Explicit --transcript-path on init carries the parent directly — no glob needed
ss start "agent-gc1" --transcript-path "/foo/projects/p/gc-main/subagents/agent-gc1.jsonl"
# Grandchild path — has TWO /subagents/ segments. Parser rejects (multi-segment),
# glob doesn't find a transcript for agent-gc2, so init errors loud.
capture ss start "agent-gc2" --transcript-path "/foo/projects/p/gc-main/subagents/agent-gc1/subagents/agent-gc2.jsonl"
assert_exit "$TEST_CODE" 1 "grandchild path: agent-gc2 errors (no resolvable parent)"
assert_not_exists "$TEST_ROOT/sessions/agent-gc2"          "grandchild path: no flat-main fallback dir"
assert_not_exists "$TEST_ROOT/sessions/gc-main/subagents/agent-gc1/subagents" "grandchild path: no fictional nested dir created"
teardown_test

setup_test
# Single-level subagent (1x /subagents/) still works
ss start "sl-main" --transcript-path "/foo/projects/p/sl-main/sl-main.jsonl"
ss start "agent-sl" --transcript-path "/foo/projects/p/sl-main/subagents/agent-sl.jsonl"
assert_eq "$(ss get agent-sl role)"              "subagent" "single-level subagent still works (regression check)"
assert_eq "$(ss get agent-sl parent_session_id)" "sl-main"  "single-level subagent: parent linkage intact"
teardown_test

section "REGRESSION: case-insensitive filesystem collision rejected"

setup_test
ss start "Case-A" --transcript-path "/foo/Case-A.jsonl"
capture ss start "case-a" --transcript-path "/foo/case-a.jsonl"
# On case-insensitive APFS this collides; on case-sensitive FS it's two separate dirs.
# Either way the helper should reject the second one to prevent silent state-sharing.
assert_exit "$TEST_CODE" 1 "case-collision rejected at init time (prevents silent state-sharing on macOS APFS)"
echo "$TEST_ERR" | grep -q "collides case-insensitively" && ok "case-collision: emits clear error" || fail "case-collision error msg" "got: $TEST_ERR"
teardown_test

setup_test
# Idempotent re-init with the SAME case is still OK
ss start "Same-Case" --transcript-path "/foo/Same-Case.jsonl"
ss set "Same-Case" approach team
ss start "Same-Case" --transcript-path "/foo/Same-Case.jsonl"
assert_exit "$?" 0 "same-case re-init: still idempotent (regression check)"
assert_eq "$(ss get Same-Case approach)" "team" "same-case re-init: existing data preserved"
teardown_test

setup_test
# Subagent inits don't trigger the case-collision check (they nest under a parent)
ss start "cs-main" --transcript-path "/foo/projects/p/cs-main/cs-main.jsonl"
ss start "agent-CS" --transcript-path "/foo/projects/p/cs-main/subagents/agent-CS.jsonl"
ss start "agent-cs" --transcript-path "/foo/projects/p/cs-main/subagents/agent-cs.jsonl" 2>/dev/null
# Each subagent dir is under cs-main/subagents/, which on APFS would collide,
# but that's a deeper concern — subagents nest under parent, not at top level.
# At minimum, verify the second one doesn't blow up the suite.
ec=$?
[ "$ec" = "0" ] || [ "$ec" = "1" ] && ok "subagent case-collision: doesn't crash (exit=$ec)" || fail "subagent case" "exit=$ec"
teardown_test

section "REGRESSION: init heals corrupt state.json"

setup_test
mkdir -p "$TEST_ROOT/sessions/heal-1"
echo "garbage not json" > "$TEST_ROOT/sessions/heal-1/state.json"
ss start "heal-1" --transcript-path "/foo/heal-1.jsonl"
assert_exit "$?" 0 "init on corrupt state.json: succeeds"
state="$TEST_ROOT/sessions/heal-1/state.json"
jq -e . "$state" >/dev/null 2>&1 && ok "init on corrupt state: file is valid JSON after re-init" || fail "heal" "still corrupt"
assert_eq "$(jq -r .role "$state")"       "main" "init on corrupt state: defaults written (role=main)"
assert_eq "$(jq -r .session_id "$state")" "heal-1" "init on corrupt state: session_id correct"
teardown_test

setup_test
# Empty state.json also gets healed
mkdir -p "$TEST_ROOT/sessions/heal-2"
: > "$TEST_ROOT/sessions/heal-2/state.json"
ss start "heal-2" --transcript-path "/foo/heal-2.jsonl"
state="$TEST_ROOT/sessions/heal-2/state.json"
jq -e . "$state" >/dev/null 2>&1 && ok "init on empty state.json: file is valid JSON after re-init" || fail "heal-empty" "still empty/corrupt"
assert_eq "$(jq -r .session_id "$state")" "heal-2" "init on empty state: defaults written"
teardown_test

setup_test
# Healing does NOT clobber a valid existing state with mutations (idempotency preserved)
ss start "heal-3" --transcript-path "/foo/heal-3.jsonl"
ss set "heal-3" approach team
ss start "heal-3" --transcript-path "/foo/heal-3.jsonl"
assert_eq "$(ss get heal-3 approach)" "team" "init heals corruption but preserves valid existing state"
teardown_test

section "COVERAGE: set adds brand-new fields not in default schema"

setup_test
ss start "nf-1" --transcript-path "/foo/nf-1.jsonl"
ss set "nf-1" my_custom_field "hello"
assert_eq "$(ss get nf-1 my_custom_field)" "hello" "set adds brand-new field via scalar assignment"
ss set "nf-1" another_field "42"
state="$TEST_ROOT/sessions/nf-1/state.json"
assert_eq "$(jq -r '.another_field | type' "$state")" "number" "set on new field with numeric value: stored as number"
teardown_test

section "COVERAGE: get on array field"

setup_test
ss start "arr-1" --transcript-path "/foo/arr-1.jsonl"
ss set "arr-1" notes '["alpha","beta","gamma"]'
out=$(ss get "arr-1" notes)
# Contract: jq -r on arrays returns multi-line pretty-printed JSON.
# Document this explicitly so downstream callers don't expect scalar output.
echo "$out" | grep -q '"alpha"' && ok "get on array: contains 'alpha'" || fail "array get" "no alpha"
echo "$out" | grep -q '"beta"'  && ok "get on array: contains 'beta'"  || fail "array get" "no beta"
echo "$out" | grep -q '"gamma"' && ok "get on array: contains 'gamma'" || fail "array get" "no gamma"
teardown_test

section "COVERAGE: drain pattern (list | cleanup) clears all sessions"

setup_test
for sid in dr-1 dr-2 dr-3 dr-4 dr-5; do
    ss start "$sid" --transcript-path "/foo/$sid.jsonl"
done
# Common operational pattern: clean every session
for sid in $(ss list); do
    ss end "$sid"
done
remaining=$(ss list | wc -l | tr -d ' ')
assert_eq "$remaining" "0" "drain pattern: ss list | cleanup clears all sessions"
teardown_test

section "COVERAGE: tmux + zellij coexist on same session"

setup_test
ss start "co-1" --transcript-path "/foo/co-1.jsonl"
ss set "co-1" pane "zellij-X"
ss set "co-1" tmux-pane "Coding:1:0"
assert_eq "$(ss find-by-pane zellij-X)"            "co-1" "coexistence: zellij pane lookup finds session"
assert_eq "$(ss find-by-pane --tmux Coding:1:0)"   "co-1" "coexistence: tmux pane lookup finds same session"
state="$TEST_ROOT/sessions/co-1/state.json"
assert_eq "$(jq -r .pane "$state")"        "zellij-X"   "coexistence: .pane field stored"
assert_eq "$(jq -r '."tmux-pane"' "$state")" "Coding:1:0" "coexistence: .tmux-pane field stored"
teardown_test

section "COVERAGE: read-during-write atomicity"

setup_test
# Verify that concurrent get during set never sees a torn / partial / invalid JSON file.
# _atomic_write uses mktemp + mv, which is atomic on the same filesystem — readers
# see either old or new state, never a half-written intermediate.
ss start "rdw-1" --transcript-path "/foo/rdw-1.jsonl"
# Background writer
( for i in $(seq 1 50); do ss set "rdw-1" approach "W-$i"; done ) &
writer_pid=$!
# 100 concurrent readers
torn=0
for i in $(seq 1 100); do
    out=$(ss get "rdw-1" approach 2>/dev/null)
    # Either default ("solo") or one of the writer values W-N
    if [ "$out" != "solo" ] && [[ ! "$out" =~ ^W-[0-9]+$ ]]; then
        torn=$((torn+1))
    fi
done
wait $writer_pid
assert_eq "$torn" "0" "read-during-write: no torn reads across 100 concurrent gets (atomic mv guarantee)"
state="$TEST_ROOT/sessions/rdw-1/state.json"
jq -e . "$state" >/dev/null 2>&1 && ok "read-during-write: final state.json valid" || fail "rdw" "corrupt"
teardown_test

section "COVERAGE: pane collision (deterministic first-match)"

setup_test
ss start "pc-A" --transcript-path "/foo/pc-A.jsonl"
ss start "pc-B" --transcript-path "/foo/pc-B.jsonl"
ss set "pc-A" pane "shared"
ss set "pc-B" pane "shared"
out=$(ss find-by-pane "shared")
case "$out" in pc-A|pc-B) ok "pane collision: returns one of the colliding sessions ($out)";; *) fail "collision" "got '$out'";; esac
# Repeat-call determinism — same input → same output
out2=$(ss find-by-pane "shared")
assert_eq "$out2" "$out" "pane collision: lookup is deterministic"
teardown_test

section "COVERAGE: symlinked CLAUDE_DATA_ROOT works"

setup_test
real_root=$(mktemp -d)
sym_root=$(mktemp -u)
ln -s "$real_root" "$sym_root"
trap 'rm -rf "$real_root"; rm -f "$sym_root"' RETURN
export CLAUDE_DATA_ROOT="$sym_root"
ss start "sym-1" --transcript-path "/foo/sym-1.jsonl"
ss set "sym-1" approach team
out=$(ss get "sym-1" approach)
assert_eq "$out" "team" "symlinked CLAUDE_DATA_ROOT: round-trip works"
[ -f "$real_root/sessions/sym-1/state.json" ] && ok "symlinked root: data lands under real path" || fail "symlink" "data missing at real path"
unset CLAUDE_DATA_ROOT
rm -rf "$real_root"
rm -f "$sym_root"
trap - RETURN
teardown_test

section "COVERAGE: session_id regex edge cases"

setup_test
ss start "12345" --transcript-path "/foo/12345.jsonl"
assert_exit "$?" 0 "all-digit session_id accepted"
ss start "x" --transcript-path "/foo/x.jsonl"
assert_exit "$?" 0 "single-character session_id accepted"
long_id=$(printf 'a%.0s' $(seq 1 250))
ss start "$long_id" --transcript-path "/foo/long.jsonl"
assert_exit "$?" 0 "250-character session_id accepted"
ss start "_underscore_first" --transcript-path "/foo/u.jsonl"
assert_exit "$?" 0 "underscore-prefix session_id accepted"
ss start "MIXED_Case-123" --transcript-path "/foo/mc.jsonl"
assert_exit "$?" 0 "mixed-case + digits + hyphens accepted"
teardown_test

section "LAZY-CREATE: append, list, cleanup, init-override"

setup_test
# append reads on never-init'd agent-* with staged transcript → nested location
ss start "ap-parent" --transcript-path "/foo/projects/p/ap-parent/ap-parent.jsonl"
stage_subagent_transcript "agent-aps" "ap-parent" >/dev/null
ss read "agent-aps" "/some/file.ts"
assert_exit "$?" 0 "append reads on lazy-create agent-*: succeeds"
assert_file_exists "$TEST_ROOT/sessions/ap-parent/subagents/agent-aps/state.json" "append reads lazy-create: state.json at nested path"
assert_file_exists "$TEST_ROOT/sessions/ap-parent/subagents/agent-aps/reads.jsonl" "append reads lazy-create: reads.jsonl at nested path"
assert_eq "$(ss get agent-aps role)"              "subagent"  "append lazy-create: role=subagent"
assert_eq "$(ss get agent-aps parent_session_id)" "ap-parent" "append lazy-create: parent populated"
teardown_test

setup_test
# append skills on never-init'd agent-* with staged transcript → nested location
ss start "as-parent" --transcript-path "/foo/projects/p/as-parent/as-parent.jsonl"
stage_subagent_transcript "agent-ask" "as-parent" >/dev/null
ss skill "agent-ask" "/cc"
assert_file_exists "$TEST_ROOT/sessions/as-parent/subagents/agent-ask/skills.jsonl" "append skills lazy-create: nested skills.jsonl"
assert_eq "$(ss get agent-ask parent_session_id)" "as-parent" "append skills lazy-create: parent populated"
teardown_test

setup_test
# Lazy-created subagents appear in list --subagents <parent> (rejects caveat #2)
ss start "lp-parent" --transcript-path "/foo/projects/p/lp-parent/lp-parent.jsonl"
stage_subagent_transcript "agent-lps1" "lp-parent" >/dev/null
stage_subagent_transcript "agent-lps2" "lp-parent" >/dev/null
ss set "agent-lps1" approach team
ss merge "agent-lps2" '{"approach":"solo"}'
out=$(ss list --subagents "lp-parent" | sort | tr '\n' ' ')
assert_eq "$out" "agent-lps1 agent-lps2 " "list --subagents includes lazy-created subagents"
teardown_test

setup_test
# Cleanup of parent cascades to lazy-created subagents (rejects caveat #3)
ss start "cl-parent" --transcript-path "/foo/projects/p/cl-parent/cl-parent.jsonl"
stage_subagent_transcript "agent-cls" "cl-parent" >/dev/null
ss set "agent-cls" approach team
assert_dir_exists "$TEST_ROOT/sessions/cl-parent/subagents/agent-cls" "lazy-create: subagent at nested path before cleanup"
ss end "cl-parent"
assert_not_exists "$TEST_ROOT/sessions/cl-parent"                          "cleanup parent: parent dir gone"
assert_not_exists "$TEST_ROOT/sessions/cl-parent/subagents/agent-cls"      "cleanup parent: cascade reaches lazy-created subagent"
teardown_test

setup_test
# Explicit --transcript-path on init overrides the on-disk glob
# Stage a transcript pointing to one parent, but pass an explicit --transcript-path
# pointing to a DIFFERENT parent. Explicit arg should win.
ss start "io-parentA" --transcript-path "/foo/projects/p/io-parentA/io-parentA.jsonl"
ss start "io-parentB" --transcript-path "/foo/projects/p/io-parentB/io-parentB.jsonl"
stage_subagent_transcript "agent-iover" "io-parentA" >/dev/null
ss start "agent-iover" --transcript-path "/foo/projects/p/io-parentB/subagents/agent-iover.jsonl"
assert_eq "$(ss get agent-iover parent_session_id)" "io-parentB" "init --transcript-path: explicit arg wins over glob"
assert_dir_exists "$TEST_ROOT/sessions/io-parentB/subagents/agent-iover" "init override: lands under explicit parent"
assert_not_exists "$TEST_ROOT/sessions/io-parentA/subagents/agent-iover" "init override: NOT under glob-resolved parent"
teardown_test

setup_test
# CLAUDE_PROJECTS_ROOT override is honored — set/merge/append find transcripts there
custom_projects=$(mktemp -d)
trap 'rm -rf "$custom_projects"' RETURN
export CLAUDE_PROJECTS_ROOT="$custom_projects"
mkdir -p "$custom_projects/myproj/cp-parent/subagents"
: > "$custom_projects/myproj/cp-parent/subagents/agent-cp.jsonl"
ss start "cp-parent" --transcript-path "/foo/projects/p/cp-parent/cp-parent.jsonl"
ss set "agent-cp" approach team
assert_eq "$(ss get agent-cp parent_session_id)" "cp-parent" "CLAUDE_PROJECTS_ROOT override: glob honors custom path"
rm -rf "$custom_projects"
trap - RETURN
teardown_test

section "HUMAN-PROMPT PREDICATE (internal _human_prompt via prompt command)"

setup_test
ss start "hp-1" --transcript-path "/foo/hp-1.jsonl"
# Plain human prose → opens a turn
printf '%s' "fix the bug in PaymentService" | ss prompt "hp-1"
assert_eq "$(ss get hp-1 human_turns)" "1" "plain prose: turn opens"

# Short imperatives
printf '%s' "yes" | ss prompt "hp-1"
assert_eq "$(ss get hp-1 human_turns)" "2" "short imperative 'yes': turn opens"
printf '%s' "go ahead" | ss prompt "hp-1"
assert_eq "$(ss get hp-1 human_turns)" "3" "short imperative 'go ahead': turn opens"

# Multi-line bracket-prefixed prose (NOT single-line bracket-enclosed)
printf '[P0] do X\nthen Y' | ss prompt "hp-1"
assert_eq "$(ss get hp-1 human_turns)" "4" "multi-line bracket-prefixed prose: turn opens"

# Human input containing literal angle brackets (no balanced close tag for the leading word)
printf '%s' "use <strong> tags here please" | ss prompt "hp-1"
assert_eq "$(ss get hp-1 human_turns)" "5" "angle brackets in body without matching close: turn opens"

# Empty content is NOT a human prompt — silent no-op, count unchanged
printf '' | ss prompt "hp-1"
assert_exit "$?" 0 "empty stdin to prompt: exit 0 (silent no-op)"
assert_eq "$(ss get hp-1 human_turns)" "5" "empty stdin: human_turns unchanged"
teardown_test

setup_test
ss start "sys-1" --transcript-path "/foo/sys-1.jsonl"
# Each catalogued system shape — none should bump human_turns
printf '<task-notification>\n<task-id>foo</task-id>\n</task-notification>' | ss prompt "sys-1"
printf '<command-name>/clear</command-name>\n<command-message>clear</command-message>' | ss prompt "sys-1"
printf '<local-command-stdout>Copied to clipboard</local-command-stdout>' | ss prompt "sys-1"
printf '<local-command-stderr>Error: shell failed</local-command-stderr>' | ss prompt "sys-1"
printf '<local-command-caveat>Caveat: messages below were generated</local-command-caveat>' | ss prompt "sys-1"
printf '<system-reminder>\nResearch the codebase before proposing\n</system-reminder>' | ss prompt "sys-1"
printf '<bash-input>v file.md</bash-input>' | ss prompt "sys-1"
printf '<bash-stdout>output</bash-stdout><bash-stderr></bash-stderr>' | ss prompt "sys-1"
printf '<teammate-message teammate_id="x" color="green">hi</teammate-message>' | ss prompt "sys-1"
printf '%s' "[task-notification]" | ss prompt "sys-1"
printf '%s' "This session is being continued from a previous conversation" | ss prompt "sys-1"
printf '%s' "Base directory for this skill: /Users/jordan/.claude/skills/pcc" | ss prompt "sys-1"
assert_eq "$(ss get sys-1 human_turns)" "0" "all 12 system-injected shapes filtered: human_turns stays 0"
assert_eq "$(ss get sys-1 current_turn_start)" "" "system-injected shapes: current_turn_start stays null"
teardown_test

section "PROMPT — turn rotation"

setup_test
# Mock _now via PATH override so timestamps are deterministic
fakebin=$(mktemp -d)
export PATH="$fakebin:$PATH"
mock_now() {
    cat > "$fakebin/date" <<EOF
#!/bin/bash
if [ "\$1" = "+%s" ]; then echo "$1"; else /bin/date "\$@"; fi
EOF
    chmod +x "$fakebin/date"
}

mock_now 1000
ss start "rot-1" --transcript-path "/foo/rot-1.jsonl"
assert_eq "$(ss get rot-1 session_start)" "1000" "start records session_start"

mock_now 1100
printf '%s' "first prompt" | ss prompt "rot-1"
assert_eq "$(ss get rot-1 human_turns)"        "1"    "1st prompt: human_turns=1"
assert_eq "$(ss get rot-1 current_turn_start)" "1100" "1st prompt: current_turn_start set"
assert_eq "$(ss get rot-1 previous_turn_start)" ""    "1st prompt: previous_turn_start still null"

mock_now 1200
printf '%s' "second prompt" | ss prompt "rot-1"
assert_eq "$(ss get rot-1 human_turns)"         "2"    "2nd prompt: human_turns=2"
assert_eq "$(ss get rot-1 current_turn_start)"  "1200" "2nd prompt: current advances to 1200"
assert_eq "$(ss get rot-1 previous_turn_start)" "1100" "2nd prompt: previous rotates to 1100"

# System-injected prompt in between — does NOT rotate
mock_now 1250
printf '<task-notification>foo</task-notification>' | ss prompt "rot-1"
assert_eq "$(ss get rot-1 human_turns)"         "2"    "system-injected: human_turns unchanged"
assert_eq "$(ss get rot-1 current_turn_start)"  "1200" "system-injected: current_turn_start unchanged"

mock_now 1300
printf '%s' "third prompt" | ss prompt "rot-1"
assert_eq "$(ss get rot-1 human_turns)"         "3"    "3rd prompt: human_turns=3"
assert_eq "$(ss get rot-1 previous_turn_start)" "1200" "3rd prompt: previous rotates to 1200"

rm -rf "$fakebin"
export PATH="${PATH#$fakebin:}"
teardown_test

section "STOPPED — last-write-wins across multi-Stop"

setup_test
fakebin=$(mktemp -d)
export PATH="$fakebin:$PATH"
mock_now() {
    cat > "$fakebin/date" <<EOF
#!/bin/bash
if [ "\$1" = "+%s" ]; then echo "$1"; else /bin/date "\$@"; fi
EOF
    chmod +x "$fakebin/date"
}

mock_now 5000
ss start "stp-1" --transcript-path "/foo/stp-1.jsonl"
mock_now 5100
ss stopped "stp-1"
assert_eq "$(ss get stp-1 last_stop)" "5100" "1st stopped sets last_stop"
mock_now 5200
ss stopped "stp-1"
assert_eq "$(ss get stp-1 last_stop)" "5200" "2nd stopped overwrites (last-write-wins)"
mock_now 5300
ss stopped "stp-1"
assert_eq "$(ss get stp-1 last_stop)" "5300" "3rd stopped overwrites again"

rm -rf "$fakebin"
export PATH="${PATH#$fakebin:}"
teardown_test

section "TOOL-USED — concurrent increment"

setup_test
ss start "tu-1" --transcript-path "/foo/tu-1.jsonl"
ss tool-used "tu-1"
ss tool-used "tu-1"
ss tool-used "tu-1"
assert_eq "$(ss get tu-1 tools_used)" "3" "tool-used: 3 sequential increments"
teardown_test

setup_test
ss start "tu-2" --transcript-path "/foo/tu-2.jsonl"
(for i in $(seq 1 25); do ss tool-used "tu-2"; done) &
(for i in $(seq 1 25); do ss tool-used "tu-2"; done) &
wait
final=$(ss get tu-2 tools_used)
# Race semantics same as `set` — last-jq-result-wins, no flock. Final count
# may be ≤ 50 due to lost updates under contention, but must be > 0 and ≤ 50.
[ "$final" -ge 1 ] && [ "$final" -le 50 ] && ok "concurrent tool-used: count is in [1,50] ($final), no corruption" || fail "concurrent tool-used" "got $final"
state="$TEST_ROOT/sessions/tu-2/state.json"
jq -e . "$state" >/dev/null 2>&1 && ok "concurrent tool-used: state.json valid" || fail "tu-2" "corrupt"
teardown_test

section "STATS — derived snapshot"

setup_test
fakebin=$(mktemp -d)
export PATH="$fakebin:$PATH"
mock_now() {
    cat > "$fakebin/date" <<EOF
#!/bin/bash
if [ "\$1" = "+%s" ]; then echo "$1"; else /bin/date "\$@"; fi
EOF
    chmod +x "$fakebin/date"
}

mock_now 10000
ss start "st-1" --transcript-path "/foo/st-1.jsonl"

mock_now 10100
printf '%s' "first" | ss prompt "st-1"

# Mid-turn stats
mock_now 10150
out=$(ss stats "st-1")
assert_eq "$(echo "$out" | jq -r .session_start)"          "10000" "stats: session_start"
assert_eq "$(echo "$out" | jq -r .session_duration)"       "150"   "stats: session_duration mid-session"
assert_eq "$(echo "$out" | jq -r .human_turns)"            "1"     "stats: human_turns"
assert_eq "$(echo "$out" | jq -r .current_turn_start)"     "10100" "stats: current_turn_start"
assert_eq "$(echo "$out" | jq -r .current_turn_duration)"  "50"    "stats: current_turn_duration mid-turn"
assert_eq "$(echo "$out" | jq -r .previous_turn_start)"    "null"  "stats: previous_turn_start null on 1st turn"
assert_eq "$(echo "$out" | jq -r .previous_turn_duration)" "null"  "stats: previous_turn_duration null on 1st turn"

# Close 1st turn, start 2nd
mock_now 10200
ss stopped "st-1"
mock_now 10300
printf '%s' "second" | ss prompt "st-1"

mock_now 10350
out=$(ss stats "st-1")
assert_eq "$(echo "$out" | jq -r .human_turns)"            "2"   "stats: human_turns=2"
assert_eq "$(echo "$out" | jq -r .previous_turn_start)"    "10100" "stats: previous_turn_start=10100"
assert_eq "$(echo "$out" | jq -r .previous_turn_duration)" "100" "stats: previous_turn_duration=last_stop(10200)-prev_start(10100)=100"
assert_eq "$(echo "$out" | jq -r .current_turn_duration)"  "50"  "stats: current_turn_duration mid 2nd turn"
teardown_test

setup_test
# Crash-mid-turn: prompt opens turn, no stopped fires before next prompt
fakebin=$(mktemp -d)
export PATH="$fakebin:$PATH"
mock_now() {
    cat > "$fakebin/date" <<EOF
#!/bin/bash
if [ "\$1" = "+%s" ]; then echo "$1"; else /bin/date "\$@"; fi
EOF
    chmod +x "$fakebin/date"
}
mock_now 20000
ss start "crash-1" --transcript-path "/foo/crash-1.jsonl"
mock_now 20100
printf '%s' "first" | ss prompt "crash-1"
# No stopped fires!
mock_now 20200
printf '%s' "second" | ss prompt "crash-1"
out=$(ss stats "crash-1")
# previous_turn_start=20100, last_stop=null → previous_turn_duration=null
assert_eq "$(echo "$out" | jq -r .previous_turn_duration)" "null" "crash-mid-turn: previous_turn_duration is null (not negative)"
teardown_test

setup_test
# stats on missing session → empty JSON
out=$(ss stats "no-such-session")
assert_eq "$out" "{}" "stats on missing session: empty object"
teardown_test

section "IS-LONG-RUNNING — gate predicate"

setup_test
fakebin=$(mktemp -d)
export PATH="$fakebin:$PATH"
mock_now() {
    cat > "$fakebin/date" <<EOF
#!/bin/bash
if [ "\$1" = "+%s" ]; then echo "$1"; else /bin/date "\$@"; fi
EOF
    chmod +x "$fakebin/date"
}

mock_now 0
ss start "lr-1" --transcript-path "/foo/lr-1.jsonl"

# Below all defaults
mock_now 100
capture ss is-long-running "lr-1"
assert_exit "$TEST_CODE" 1 "is-long-running: below all thresholds → exit 1"

# Cross --turns threshold (default 5) — fire 5 prompts
for i in 1 2 3 4 5; do
    mock_now $((100 + i))
    printf 'p%d' "$i" | ss prompt "lr-1"
done
capture ss is-long-running "lr-1"
assert_exit "$TEST_CODE" 0 "is-long-running: 5 turns crosses --turns default"

# Custom --turns 10 — should fall below
capture ss is-long-running "lr-1" --turns 10
assert_exit "$TEST_CODE" 1 "is-long-running: --turns 10 override → 5 < 10 → exit 1"

# Cross --seconds threshold (default 600) — jump time
mock_now 700
capture ss is-long-running "lr-1" --turns 999 --tools 999
assert_exit "$TEST_CODE" 0 "is-long-running: session_duration 700 ≥ 600 → exit 0 even with other thresholds high"

# Cross --tools threshold (default 30)
ss start "lr-2" --transcript-path "/foo/lr-2.jsonl"
mock_now 0
for i in $(seq 1 30); do ss tool-used "lr-2"; done
capture ss is-long-running "lr-2"
assert_exit "$TEST_CODE" 0 "is-long-running: 30 tools crosses --tools default"

capture ss is-long-running "lr-2" --tools 50 --turns 999 --seconds 99999
assert_exit "$TEST_CODE" 1 "is-long-running: --tools 50 override → 30 < 50 → exit 1"

rm -rf "$fakebin"
export PATH="${PATH#$fakebin:}"
teardown_test

section "GET --path (replaces old `path` subcommand)"

setup_test
unset CLAUDE_DATA_ROOT  # default fallback test
out=$(ss get --path data-root)
assert_eq "$out" "$HOME/.claude" "get --path data-root: default fallback"
export CLAUDE_DATA_ROOT="$TEST_ROOT"
teardown_test

setup_test
out=$(ss get --path data-root)
assert_eq "$out" "$TEST_ROOT" "get --path data-root: respects \$CLAUDE_DATA_ROOT"
out=$(ss get --path sessions)
assert_eq "$out" "$TEST_ROOT/sessions" "get --path sessions"
out=$(ss get --path shaping)
assert_eq "$out" "$TEST_ROOT/shaping" "get --path shaping"
teardown_test

setup_test
ss start "gp-1" --transcript-path "/foo/gp-1.jsonl"
out=$(ss get --path "gp-1")
assert_eq "$out" "$TEST_ROOT/sessions/gp-1" "get --path <session_id>: main session dir"
teardown_test

setup_test
ss start "gp-main" --transcript-path "/foo/projects/p/gp-main/gp-main.jsonl"
ss start "agent-gp" --transcript-path "/foo/projects/p/gp-main/subagents/agent-gp.jsonl"
out=$(ss get --path "agent-gp")
assert_eq "$out" "$TEST_ROOT/sessions/gp-main/subagents/agent-gp" "get --path <subagent_id>: nested dir"
teardown_test

setup_test
ss start "current-1" --transcript-path "/foo/current-1.jsonl"
export CLAUDE_SESSION_ID="current-1"
out=$(ss get --path)
assert_eq "$out" "$TEST_ROOT/sessions/current-1" "get --path (no args): uses \$CLAUDE_SESSION_ID"
unset CLAUDE_SESSION_ID
teardown_test

setup_test
capture ss get --path
assert_exit "$TEST_CODE" 1 "get --path with no args and no \$CLAUDE_SESSION_ID exits 1"
teardown_test

setup_test
# Reserved keyword wins over a session_id named the same
ss start "data-root" --transcript-path "/foo/data-root.jsonl" 2>/dev/null  # session_id "data-root" passes regex
out=$(ss get --path data-root)
assert_eq "$out" "$TEST_ROOT" "get --path data-root: keyword wins over session named 'data-root'"
teardown_test

setup_test
# get <session_id> <field> — non-path mode unchanged
ss start "gf-1" --transcript-path "/foo/gf-1.jsonl"
out=$(ss get "gf-1" approach)
assert_eq "$out" "solo" "get <session_id> <field>: still works without --path"
teardown_test

section "REGRESSION: missing jq fails loud"

setup_test
if [ ! -x /bin/bash ]; then
    ok "skipped: /bin/bash not present on this system"
elif PATH=/bin command -v jq >/dev/null 2>&1; then
    ok "skipped: /bin/jq exists on this system (rare)"
else
    out=$(PATH=/bin /bin/bash "$HELPER" start "j-1" --transcript-path "/foo/j-1.jsonl" 2>&1)
    code=$?
    assert_exit "$code" 127 "missing jq: helper exits 127"
    echo "$out" | grep -q "jq is required" && ok "missing jq: emits clear error" || fail "missing jq error msg" "got: $out"
fi
teardown_test

# ============================================================================
# Summary
# ============================================================================
echo
printf '\033[1m─── Results ───\033[0m\n'
printf 'Tests run:    %d\n' "$TESTS_RUN"
printf 'Tests passed: \033[32m%d\033[0m\n' "$TESTS_PASSED"
if [ "$TESTS_FAILED" -gt 0 ]; then
    printf 'Tests failed: \033[31m%d\033[0m\n' "$TESTS_FAILED"
    exit 1
fi
printf 'Tests failed: %d\n' "$TESTS_FAILED"
exit 0
