#!/bin/bash
# Inject tracer-only context into Read/Glob tool calls via PreToolUse.
#
# Output `additionalContext` JSON (per Claude Code hook spec) so the agent
# sees the tracer signal native Read/Glob do NOT carry. Native tools already
# load Claude.md ancestors and `.claude/rules/` via the harness's auto-load —
# this hook adds only the lifecycle/complexity/graph signal on top.
#
# Read   → enrich the file with passive_context + caller/dependent counts.
# Glob   → run `trace glob <pattern> <base> --details`: every match
#          annotated with ccn + rank + lifecycle shoulder.
# Grep   → no-op for now (matched-file enrichment via ripgrep is a future
#          extension; native Grep already returns matched lines with file
#          paths the agent can Read individually).
#
# Silent fallback: any error path exits 0 with no output. Native tool runs.

read -r input

tool_name=$(echo "$input" | jq -r '.tool_name // ""' 2>/dev/null) || exit 0

case "$tool_name" in
    Read)
        target=$(echo "$input" | jq -r '.tool_input.file_path // empty' 2>/dev/null)
        [ -z "$target" ] && exit 0
        args=("context" "$target")
        ;;
    Glob)
        pattern=$(echo "$input" | jq -r '.tool_input.pattern // empty' 2>/dev/null)
        [ -z "$pattern" ] && exit 0
        base=$(echo "$input" | jq -r '.tool_input.path // empty' 2>/dev/null)
        [ -z "$base" ] && base="$PWD"
        args=("glob" "$pattern" "$base" "--details")
        ;;
    *)
        exit 0
        ;;
esac

# Resolve trace binary: prefer `trace` on PATH, fall back to the plugin launcher.
trace_bin=$(command -v trace 2>/dev/null)
[ -z "$trace_bin" ] && trace_bin="${CLAUDE_PLUGIN_ROOT:-$HOME/.claude/plugins/talents/talent-tree/packages/claude}/bin/trace"
[ ! -x "$trace_bin" ] && exit 0

# Hard timeout so a slow/hung tracer never blocks the tool.
context_output=$(timeout 5 "$trace_bin" "${args[@]}" 2>/dev/null) || exit 0
[ -z "$context_output" ] && exit 0

# Wrap as additionalContext JSON. jq safely encodes the multi-line string.
jq -nc --arg ctx "$context_output" \
    '{hookSpecificOutput: {hookEventName: "PreToolUse", additionalContext: $ctx}}' 2>/dev/null

exit 0
