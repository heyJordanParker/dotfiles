#!/bin/bash

# Validate ledger entry format in Claude.md files
# PreToolUse hook on Write|Edit — deterministic, no LLM
# Checks: length, full stops, em dashes

set -uo pipefail

# Read hook event data
INPUT=$(cat)

# Skip agent sessions
SESSION_ID=$(echo "$INPUT" | jq -r '.session_id // ""' 2>/dev/null) || exit 0
[[ -z "$SESSION_ID" || "$SESSION_ID" == agent-* ]] && exit 0

TOOL_NAME=$(echo "$INPUT" | jq -r '.tool_name // ""' 2>/dev/null) || exit 0

# Extract file path and content
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

# Only check Claude.md files (belt & suspenders — if condition handles this too)
[[ "$(basename "$FILE_PATH")" != "Claude.md" && "$(basename "$FILE_PATH")" != "CLAUDE.md" ]] && exit 0

# Extract ledger entries (lines matching "- vN.N:" pattern)
# No heading search needed — pattern is unique to ledger sections in Claude.md files
LEDGER_ENTRIES=""
while IFS= read -r line; do
    [[ "$line" =~ ^-[[:space:]]+v[0-9] ]] && LEDGER_ENTRIES="${LEDGER_ENTRIES}${line}"$'\n'
done <<< "$CONTENT"

# No ledger entries found — pass
[[ -z "$LEDGER_ENTRIES" ]] && exit 0

# Validate each entry
ERRORS=""
while IFS= read -r entry; do
    [[ -z "$entry" ]] && continue

    # Extract version label
    VERSION=$(echo "$entry" | sed -n 's/^\(- v[0-9][0-9.]*\).*/\1/p' 2>/dev/null) || VERSION="(unknown)"

    # Strip version prefix to get entry text
    TEXT_AFTER_VERSION=$(echo "$entry" | sed 's/^- v[0-9][0-9.]*: //' 2>/dev/null) || true

    # Check length (> 180 chars)
    LENGTH=${#entry}
    if [ "$LENGTH" -gt 180 ]; then
        ERRORS="${ERRORS}${VERSION}: ${LENGTH} chars (max 180). Shorten to a single clear sentence.\n"
    fi

    # Check for periods (full stops, file paths, multi-sentence)
    # Exception: "Claude.md" is a proper term
    TEXT_CHECK=$(echo "$TEXT_AFTER_VERSION" | sed 's/Claude\.md//g' 2>/dev/null) || TEXT_CHECK="$TEXT_AFTER_VERSION"
    if echo "$TEXT_CHECK" | grep -q '\.' 2>/dev/null; then
        ERRORS="${ERRORS}${VERSION}: Contains periods. Use a single sentence with no full stops or file paths.\n"
    fi

    # Check for em dashes
    if echo "$TEXT_AFTER_VERSION" | grep -q '—' 2>/dev/null; then
        ERRORS="${ERRORS}${VERSION}: Uses em dash. Use simple connectors (to/for/because) instead.\n"
    fi

done <<< "$LEDGER_ENTRIES"

# Report errors
if [ -n "$ERRORS" ]; then
    BLOCK_MSG="BLOCKED: Ledger entry format violations:\n${ERRORS}\nUse /cc skill (reference: claude-md.md) for ledger format guidance. Entries should be: \"- vX.Y: ACTION to/for/because REASONING\""
    echo -e "$BLOCK_MSG" >&2
    exit 2
fi

exit 0
