#!/bin/bash
FILE=$(jq -r '.tool_input.file_path // empty')
if [[ "$FILE" == *.md && -f "$FILE" ]]; then
  HEADER=$(head -5 "$FILE" 2>/dev/null)
  if echo "$HEADER" | grep -q '^shaping: true'; then
    cat >&2 <<'MSG'
Shaping ripple check:
- Changed Requirements? → update Fit Check + any Gaps, Open Questions by Part
- Changed Shape (A, B...) Parts? → update Fit Check + any Gaps, Open Questions by Part
- Changed Boundaries? → check if modeling artifacts + plan still respect them
- Updated a Mermaid diagram? → Affordance tables are the source of truth. Update tables FIRST, then render
- Changed Work Streams Detail? → update Work Streams Mermaid
MSG
    exit 2
  fi
  if echo "$HEADER" | grep -q '^modeling: true'; then
    cat >&2 <<'MSG'
Modeling sync check:
- Updated affordances.md? → Re-render the 3-section presentation (DB Schema, UX, Architecture). Keep affordances.md structure complete (all UI, Code, Data Store tables with full wiring)
- Updated the presentation? → Verify every affordance from affordances.md appears in exactly one section. Update affordances.md if new affordances were discovered during presentation
- Changed a model/type/architecture? → Update BOTH affordances.md tables AND the shaping doc's parts table
- Changed the shaping doc's parts? → Check if affordances.md + presentation need updating
- Added/removed a boundary in shaping doc? → Check if modeling artifacts respect it
MSG
    exit 2
  fi
fi
exit 0
