#!/bin/bash

# Ban deferred work in planning documents
# PreToolUse hook on Write|Edit — blocks writes containing deferred items
# Planning docs identified by: path under ~/.claude/shaping/ OR frontmatter markers
# Uses claude -p for LLM-based deferral detection

set -uo pipefail
# NOTE: no set -e — graceful allow on any failure

# Read hook event data
INPUT=$(cat)

# Skip agent sessions
SESSION_ID=$(echo "$INPUT" | jq -r '.session_id // ""' 2>/dev/null) || exit 0
[[ -z "$SESSION_ID" || "$SESSION_ID" == agent-* ]] && exit 0

TOOL_NAME=$(echo "$INPUT" | jq -r '.tool_name // ""' 2>/dev/null) || exit 0

# Extract file path and content based on tool type
FILE_PATH=""
CONTENT=""
if [ "$TOOL_NAME" = "Write" ]; then
    FILE_PATH=$(echo "$INPUT" | jq -r '.tool_input.file_path // ""' 2>/dev/null) || exit 0
    CONTENT=$(echo "$INPUT" | jq -r '.tool_input.content // ""' 2>/dev/null) || exit 0
elif [ "$TOOL_NAME" = "Edit" ]; then
    FILE_PATH=$(echo "$INPUT" | jq -r '.tool_input.file_path // ""' 2>/dev/null) || exit 0
    CONTENT=$(echo "$INPUT" | jq -r '.tool_input.new_string // ""' 2>/dev/null) || exit 0
else
    exit 0
fi

# Must be a markdown file
[[ "$FILE_PATH" != *.md ]] && exit 0

# Check if planning doc: path under ~/.claude/shaping/
IS_PLANNING_DOC=false
[[ "$FILE_PATH" == *"/.claude/shaping/"* ]] && IS_PLANNING_DOC=true

# Check frontmatter markers
if [ "$IS_PLANNING_DOC" = "false" ]; then
    if [ "$TOOL_NAME" = "Write" ]; then
        HEADER=$(echo "$CONTENT" | head -5 2>/dev/null) || true
    elif [ "$TOOL_NAME" = "Edit" ] && [ -f "$FILE_PATH" ]; then
        HEADER=$(head -5 "$FILE_PATH" 2>/dev/null) || true
    else
        HEADER=""
    fi
    if echo "$HEADER" | grep -q '^\(shaping\|modeling\|slicing\): true' 2>/dev/null; then
        IS_PLANNING_DOC=true
    fi
fi

# Not a planning doc — pass
[ "$IS_PLANNING_DOC" = "false" ] && exit 0

# LLM evaluation for deferral
JSON_SCHEMA='{"type":"object","properties":{"ok":{"type":"boolean"},"reason":{"type":"string"}},"required":["ok"]}'

SYSTEM_PROMPT="You are a planning document validator. Check if content defers any work. Output structured JSON only."

EVAL_PROMPT="Does this planning document content defer any work?

Rules:
- Every item in a planning document ships. If it doesn't ship, it doesn't go in the document
- Deferral includes: items marked as \"deferred\", punted to a future phase, declared out of scope for now, or any form of \"deferred\" or \"this ships later, not now\"
- Slice labels (V1, V2, V3) and phase labels (Phase 1, Phase 2) are implementation ordering — all slices ship. They are NOT deferral unless they explicitly state work is pushed out of scope or will be done later
- Optional items (\"Nice-to-have\", \"Undecided\", \"if time allows\") are also banned — every item in a planning doc is committed work, not a maybe. If it's not in scope, it's not in the document
- Presenting options (\"Option A or B\", \"we could do X or Y\") is also banned — the user decides options and scope before the plan is finalized. Plans contain decisions, not choices
- For Edit operations, you only see the replacement text (new_string), not the full document. Judge only what you see — do not infer deferral from slice/phase labels alone

File: ${FILE_PATH}
Content:
${CONTENT}

Return JSON:
- No deferral: {\"ok\": true}
- Deferral found: {\"ok\": false, \"reason\": \"[what was deferred and where]\"}"

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

# If classification failed, allow (graceful)
if [ -z "$RESULT" ] || [ "$RESULT" = "null" ]; then
    exit 0
fi

# Check result
OK=$(echo "$RESULT" | jq -r 'if .ok == false then "false" else "true" end' 2>/dev/null) || exit 0
if [ "$OK" = "false" ]; then
    REASON=$(echo "$RESULT" | jq -r '.reason // "Deferred work detected"' 2>/dev/null) || REASON="Deferred work detected"
    echo "BLOCKED: Planning document contains deferred work. ${REASON}. Every item in a planning doc ships — include it as real work or remove it entirely." >&2
    exit 2
fi

exit 0
