#!/usr/bin/env bash
# SessionStart hook: inject the `trace context` repo primer as additional
# context for the agent. Runs once per session start; output goes into the
# agent's context window via hookSpecificOutput.additionalContext.
#
# Skip conditions (silent exit 0 — never blocks session start):
# - Not inside a git repo
# - `trace` binary not on PATH (plugin not installed / disabled)
# - Command times out (cold cache build on a brand-new repo)
# - jq missing (can't serialize)
set -euo pipefail

# Drain stdin so the hook payload doesn't sit unread.
cat >/dev/null 2>&1 || true

if ! command -v trace >/dev/null 2>&1; then
    exit 0
fi
if ! command -v jq >/dev/null 2>&1; then
    exit 0
fi
if ! git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    exit 0
fi

# Cold cache builds the architecture graph (~7s on a 1500-file repo);
# warm runs come in under a second. 15s timeout covers the cold-build
# case without making session start drag indefinitely on huge repos.
output=$(timeout 15 trace context 2>/dev/null) || exit 0
[ -z "$output" ] && exit 0

jq -nc --arg ctx "$output" '{
  hookSpecificOutput: {
    hookEventName: "SessionStart",
    additionalContext: $ctx
  }
}'
