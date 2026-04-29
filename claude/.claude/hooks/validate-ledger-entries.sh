#!/bin/bash

# Validate ledger entry format in Claude.md files
# PreToolUse hook on Write|Edit|MultiEdit — deterministic, no LLM
# Checks: length (60 chars), word count (10), em dashes, periods, parentheticals,
#         duplicate version vs disk latest (Edit/MultiEdit), duplicate versions in payload
# See /cc reference claude-md.md for the format spec

set -uo pipefail

# Read hook event data
INPUT=$(cat)

# Skip agent sessions
SESSION_ID=$(echo "$INPUT" | jq -r '.session_id // ""' 2>/dev/null) || exit 0
[[ -z "$SESSION_ID" || "$SESSION_ID" == agent-* ]] && exit 0

TOOL_NAME=$(echo "$INPUT" | jq -r '.tool_name // ""' 2>/dev/null) || exit 0

# Extract file path, new content, and pre-edit content
# CONTENT       = what's being written/added (validated for format + duplicates)
# OLD_CONTENT   = the old_string(s) being replaced (used for carve-out: in-place update of an existing version is not a duplicate)
FILE_PATH=""
CONTENT=""
OLD_CONTENT=""
if [ "$TOOL_NAME" = "Write" ]; then
    FILE_PATH=$(echo "$INPUT" | jq -r '.tool_input.file_path // ""' 2>/dev/null) || exit 0
    CONTENT=$(echo "$INPUT" | jq -r '.tool_input.content // ""' 2>/dev/null) || exit 0
elif [ "$TOOL_NAME" = "Edit" ]; then
    FILE_PATH=$(echo "$INPUT" | jq -r '.tool_input.file_path // ""' 2>/dev/null) || exit 0
    CONTENT=$(echo "$INPUT" | jq -r '.tool_input.new_string // ""' 2>/dev/null) || exit 0
    OLD_CONTENT=$(echo "$INPUT" | jq -r '.tool_input.old_string // ""' 2>/dev/null) || exit 0
elif [ "$TOOL_NAME" = "MultiEdit" ]; then
    FILE_PATH=$(echo "$INPUT" | jq -r '.tool_input.file_path // ""' 2>/dev/null) || exit 0
    CONTENT=$(echo "$INPUT" | jq -r '[.tool_input.edits[]?.new_string // ""] | join("\n")' 2>/dev/null) || exit 0
    OLD_CONTENT=$(echo "$INPUT" | jq -r '[.tool_input.edits[]?.old_string // ""] | join("\n")' 2>/dev/null) || exit 0
else
    exit 0
fi

# Only check Claude.md files (case-insensitive — APFS lets agents pass either case)
BASENAME=$(basename "$FILE_PATH")
shopt -s nocasematch
[[ "$BASENAME" != "Claude.md" ]] && { shopt -u nocasematch; exit 0; }
shopt -u nocasematch

# Extract ledger entries (lines matching "- vN.N:" pattern)
LEDGER_ENTRIES=""
while IFS= read -r line; do
    [[ "$line" =~ ^-[[:space:]]+v[0-9] ]] && LEDGER_ENTRIES="${LEDGER_ENTRIES}${line}"$'\n'
done <<< "$CONTENT"

# No ledger entries in the diff — pass
[[ -z "$LEDGER_ENTRIES" ]] && exit 0

# Validate each entry
ERRORS=""
while IFS= read -r entry; do
    [[ -z "$entry" ]] && continue

    # Extract version label
    VERSION=$(echo "$entry" | sed -n 's/^\(- v[0-9][0-9.]*\).*/\1/p' 2>/dev/null) || VERSION="(unknown)"

    # Strip version prefix to get entry text
    TEXT_AFTER_VERSION=$(echo "$entry" | sed 's/^- v[0-9][0-9.]*: //' 2>/dev/null) || true

    # Check length (> 60 chars total including the "- vX.Y: " prefix)
    LENGTH=${#entry}
    if [ "$LENGTH" -gt 60 ]; then
        ERRORS="${ERRORS}${VERSION}: ${LENGTH} chars (max 60).\n"
    fi

    # Check word count (> 10 words after version prefix)
    WORD_COUNT=$(echo "$TEXT_AFTER_VERSION" | wc -w | tr -d ' ')
    if [ "$WORD_COUNT" -gt 10 ]; then
        ERRORS="${ERRORS}${VERSION}: ${WORD_COUNT} words (max 10).\n"
    fi

    # Check for periods (full stops, multi-sentence, file paths)
    # Exception: "Claude.md" is a proper term
    TEXT_CHECK=$(echo "$TEXT_AFTER_VERSION" | sed 's/Claude\.md//g' 2>/dev/null) || TEXT_CHECK="$TEXT_AFTER_VERSION"
    if echo "$TEXT_CHECK" | grep -q '\.' 2>/dev/null; then
        ERRORS="${ERRORS}${VERSION}: contains periods (single sentence, no full stops or file paths).\n"
    fi

    # Check for em dashes
    if echo "$TEXT_AFTER_VERSION" | grep -q '—' 2>/dev/null; then
        ERRORS="${ERRORS}${VERSION}: uses em dash (use simple connectors: to, for, because).\n"
    fi

    # Check for parentheticals (signal of detail dumping or implementation HOW)
    if echo "$TEXT_AFTER_VERSION" | grep -q '(' 2>/dev/null; then
        ERRORS="${ERRORS}${VERSION}: contains parentheses (drop the parenthetical — name the decision, not the detail).\n"
    fi

done <<< "$LEDGER_ENTRIES"

# Collect new entry versions (from the diff/payload)
NEW_VERSIONS=""
while IFS= read -r entry; do
    [[ -z "$entry" ]] && continue
    V=$(echo "$entry" | sed -n 's/^- \(v[0-9][0-9.]*\):.*/\1/p' 2>/dev/null) || V=""
    [[ -n "$V" ]] && NEW_VERSIONS="${NEW_VERSIONS}${V}"$'\n'
done <<< "$LEDGER_ENTRIES"

# Find latest version on disk (first ledger entry under ## Ledger header)
LATEST_DISK_VERSION=""
if [ -f "$FILE_PATH" ]; then
    LATEST_DISK_VERSION=$(awk '/^## Ledger/{f=1; next} f && /^-[[:space:]]+v[0-9]/{print; exit}' "$FILE_PATH" 2>/dev/null | sed -n 's/^- \(v[0-9][0-9.]*\):.*/\1/p') || LATEST_DISK_VERSION=""
fi

# Check 1: any new entry collides with latest disk version
# Skipped for Write — its payload is the whole file, so existing entries always match disk latest;
# sequential-Write cheese is still caught by check 2.
# Carve-out: if old_content also has a "- vX.Y:" line for the same version, this is an in-place
# update of the latest entry, not a duplicate.
if { [ "$TOOL_NAME" = "Edit" ] || [ "$TOOL_NAME" = "MultiEdit" ]; } && [ -n "$LATEST_DISK_VERSION" ] && [ -n "$NEW_VERSIONS" ]; then
    while IFS= read -r v; do
        [[ -z "$v" ]] && continue
        if [ "$v" = "$LATEST_DISK_VERSION" ]; then
            # In-place update carve-out: old_content already had this version
            if [[ "$OLD_CONTENT" == *"- ${v}:"* ]]; then
                continue
            fi
            ERRORS="${ERRORS}${v}: already the latest version on disk — bump to next version or update the existing entry in place.\n"
            break
        fi
    done <<< "$NEW_VERSIONS"
fi

# Check 2: duplicates among new entries
if [ -n "$NEW_VERSIONS" ]; then
    DUPES=$(echo "$NEW_VERSIONS" | sort | uniq -d)
    while IFS= read -r v; do
        [[ -z "$v" ]] && continue
        ERRORS="${ERRORS}${v}: multiple new entries for the same version — collapse into one line.\n"
    done <<< "$DUPES"
fi

# Report errors
if [ -n "$ERRORS" ]; then
    BLOCK_MSG="BLOCKED: ledger entry violations
${ERRORS}
Ledger entries name the decision. One line per version. The diff and file body show what changed; the ledger records the decision.

Format rules (see /cc reference claude-md.md):
- Max 60 chars total, max 10 words after version prefix
- Single sentence, no full stops, no em dashes, no parentheticals
- No mechanics ('promoted', 'consolidates', 'extracted', 'refactored') — name the decision
- No body-content repetition — the file already shows what changed
- One entry per version — multiple changes in one bump collapse into a single line
- Pattern: '- vX.Y: ACTION to/for/because REASONING'
- Good: 'v1.2: Coupon removed because offers own pricing'
- Good: 'v1.2: Three-way session split for distinct lifecycles'"
    echo -e "$BLOCK_MSG" >&2
    exit 2
fi

exit 0
