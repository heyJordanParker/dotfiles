#!/usr/bin/env bash
# Archive a subagent's tracer log when that subagent stops.
#
# Active subagent logs live at:
#   <repo>/.tracer-cache/sessions/<session_id>/<agent_id>/
#
# On stop this hook moves them to:
#   <repo>/.tracer-cache/sessions/<session_id>/archived/<agent_id>/
#
# `<repo>` is `git -C "$PWD" rev-parse --show-toplevel` — the hook
# inherits cwd from the parent (the harness's open project) and resolves
# the repo root from there. When `$PWD` is not inside a git repo (or git
# is unavailable) the hook silently exits 0: the tracer's session-context
# module no-ops in that same condition, so there is nothing to archive.
#
# Why move rather than delete: the log may still be queried
# (debugging, drift detection, retrospective analysis). The tracer's
# session_log read path falls back to the archived directory when the
# active one is missing, so `trace docs --graph`, log
# queries, and other read surfaces continue to work for an archived
# subagent.
#
# Why archive at all: without lifecycle handling the active sessions/<sid>/
# directory grows unboundedly for long-running orchestrators that spawn many
# subagents — every subagent's per-agent dir stays under the active prefix
# forever. Moving stopped subagents under archived/ keeps the active set
# bounded to "actually still running."
#
# Wired into settings.json's UserPromptSubmit handler that parses the
# `<task-notification>` system message — Claude Code 2.1.131 does not fire
# `SubagentStop`, so the task-notification parse is the validated stop signal.
#
# Local-only tracer hook: wired in settings.json only, never in hooks.json.
#
# Arguments:
#   $1 — session_id (parent session UUID)
#   $2 — agent_id  (the subagent's task id; matches TRACER_AGENT_ID)
#
# Silent fallback: missing args, no resolvable repo root, missing source
# dir, and mv failures all exit 0 — never blocks the parent on archive
# churn.
set -euo pipefail

SESSION_ID="${1:-}"
AGENT_ID="${2:-}"
[ -z "$SESSION_ID" ] && exit 0
[ -z "$AGENT_ID" ] && exit 0

# Resolve the repo root from the inherited cwd. No repo → no
# log to archive (matches the tracer's standalone no-op).
REPO_ROOT="$(git -C "$PWD" rev-parse --show-toplevel 2>/dev/null || true)"
[ -z "$REPO_ROOT" ] && exit 0

base="$REPO_ROOT/.tracer-cache/sessions/$SESSION_ID"
active="$base/$AGENT_ID"
archived="$base/archived/$AGENT_ID"

# The subagent never wrote a log (no docs surfaced, no
# reads recorded). Nothing to archive.
[ -d "$active" ] || exit 0

# Guard against double-stop or re-runs: if an archived copy already exists,
# remove it before moving so mv lands cleanly. Two stops for the same agent
# id means the second observation supersedes the first.
if [ -d "$archived" ]; then
    rm -rf "$archived" 2>/dev/null || exit 0
fi

mkdir -p "$base/archived" 2>/dev/null || exit 0
mv "$active" "$archived" 2>/dev/null || true
exit 0
