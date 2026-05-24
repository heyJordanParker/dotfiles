#!/usr/bin/env bash
# SessionStart hook: re-run `trace context prime` so the tracer session log
# mirrors the docs Claude Code's harness auto-loaded at session start or after
# compaction. Subsequent tracer emissions (Read enrichment, `trace docs`,
# `trace read --docs`) then dedup against what the agent already has in
# context.
#
# Wired on SessionStart matcher `startup|resume|clear|compact`. The matcher
# value arrives in the stdin payload's `source` field; this script picks the
# `trace context prime --reason` accordingly:
#
#   compact                              → --reason post_compact
#   startup | resume | clear (anything)  → --reason session_start
#
# Both reasons share the same context primer; only the event source string
# stamped on each emitted event differs (`context_prime_post_compact` vs
# `context_prime_session_start`). The log dedupes by content hash, so this
# hook is idempotent: nothing re-emits unless a doc actually changed.
#
# Pairs with `load-trace-context.sh` (the same matcher, sibling hook) — that
# script injects the trace context primer as additionalContext; this script
# updates the log's projection so tracer queries that follow honor what
# the harness re-emitted.
#
# Local-only tracer hook: wired in settings.json only, never in hooks.json.
#
# Silent fallback: any failure path exits 0 with no output so a session
# never breaks because of this hook's own brokenness.
set -euo pipefail

# Drain stdin so the hook payload doesn't sit unread.
INPUT=$(cat 2>/dev/null || true)

if ! command -v trace >/dev/null 2>&1; then
    exit 0
fi
if ! command -v jq >/dev/null 2>&1; then
    exit 0
fi

# Propagate session + agent identity from the payload into the trace
# subprocess env. The log resolves session id from env only.
SESSION_ID=$(printf '%s' "$INPUT" | jq -r '.session_id // empty' 2>/dev/null)
AGENT_ID=$(printf '%s' "$INPUT" | jq -r '.agent_id // empty' 2>/dev/null)
[ -z "$SESSION_ID" ] && exit 0
export CLAUDE_CODE_SESSION_ID="$SESSION_ID"
[ -n "$AGENT_ID" ] && export TRACER_AGENT_ID="$AGENT_ID"

# Pick the reason from the SessionStart matcher value. The hook spec
# delivers it as `source`; treat anything other than `compact` as a
# session-start lifecycle moment.
SOURCE=$(printf '%s' "$INPUT" | jq -r '.source // empty' 2>/dev/null)
if [ "$SOURCE" = "compact" ]; then
    REASON="post_compact"
else
    REASON="session_start"
fi

# Hard timeout so a slow tracer never blocks session start / post-compact.
timeout 15 trace context prime --reason "$REASON" >/dev/null 2>&1 || true
exit 0
