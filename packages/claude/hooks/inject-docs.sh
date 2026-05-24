#!/bin/bash
# PreToolUse(Bash): when the agent runs a path-taking `trace` command,
# inject the project-docs (Claude.md / .claude rules ancestors) for the
# resolved path into the model's context via additionalContext.
#
# Thin wrapper over `trace docs <path>` (path-mode) — the binary owns
# query, emit, record, and the docs/already_loaded split. This script's
# responsibilities are limited to:
#   - parsing the agent's Bash command for the `trace` invocation
#   - resolving the path argument to an absolute target
#   - propagating session + agent identity into the subprocess
#   - rendering the binary's response as additionalContext
#
# LOCAL-ONLY: wired in settings.json only, never in the plugin-distributed
# hooks.json. Tracer hooks are our experimental local surface; plugin users
# get tracer as a command, never these hooks.
#
# Behavior:
# - Non-`trace` Bash, or a `trace` subcommand that takes no path → exit 0
#   (clean no-op, never blocks).
# - Path-taking `trace` subcommand → resolve the path, run
#   `trace docs <path> --source trace_inject_hook
#       --triggering-tool Bash --triggering-command <cmd> --json`.
#   - doc_count > 0 → inject the full response as additionalContext,
#     exit 0. The response carries `docs` (new) and may carry
#     `already_loaded` (skipped, with per-entry source) so the agent
#     sees both slices.
#   - doc_count == 0 (everything is already in context) → inject
#     nothing, exit 0.
#   - `trace docs` FAILS (non-zero) → BLOCK the trace command, exit 2,
#     with a loud explicit reason on stderr naming the path + the
#     error. The agent must not silently trace without docs.
# - Any infrastructure failure (missing jq/trace, parse error) → exit 0;
#   a hook never blocks the agent because the hook itself is broken.
#
# No recursion: the hook's own `trace docs` runs as a direct subprocess,
# not through the agent's Bash tool, so PreToolUse does not re-fire.

read -r input

command -v jq >/dev/null 2>&1 || exit 0
command -v trace >/dev/null 2>&1 || exit 0

command=$(echo "$input" | jq -r '.tool_input.command // ""' 2>/dev/null) || exit 0
[ -z "$command" ] && exit 0
cwd=$(echo "$input" | jq -r '.cwd // ""' 2>/dev/null) || cwd=""
[ -z "$cwd" ] && cwd="$(pwd)"

# Propagate session + agent identity from stdin into the trace subprocess
# env. `nested_memory::session_id()` resolves session id from env only, so
# without this the log no-ops on every call — the
# broken_dedupe failure mode where the same Claude.md is re-emitted on
# every Bash trace call.
session_id=$(echo "$input" | jq -r '.session_id // ""' 2>/dev/null)
agent_id=$(echo "$input" | jq -r '.agent_id // ""' 2>/dev/null)
[ -n "$session_id" ] && export CLAUDE_CODE_SESSION_ID="$session_id"
[ -n "$agent_id" ] && export TRACER_AGENT_ID="$agent_id"

# Only path-taking trace subcommands carry a meaningful file/dir for docs.
path_taking='read|info|list|tree|structure|grep|struct|find|glob|blame|history|diff'

# Find the `trace` token (basename, so /abs/trace matches), take the next
# token as subcommand and the first following non-flag token as the path.
# awk tokenizer is portable; BSD sed lacks \b word boundaries.
parsed=$(echo "$command" | awk '{
  for (i=1;i<=NF;i++) {
    t=$i; sub(/.*\//,"",t)
    if (t=="trace") {
      s=(i+1<=NF)?$(i+1):""
      p=""
      for (j=i+2;j<=NF;j++){ if($j ~ /^-/) continue; p=$j; break }
      print s "\t" p
      exit
    }
  }
}')
[ -z "$parsed" ] && exit 0
subcmd=${parsed%%$'\t'*}
pathtok=${parsed#*$'\t'}
echo "$subcmd" | grep -qE "^($path_taking)$" || exit 0

resolve_path() {
    local t="$1"
    t="${t%\"}"; t="${t#\"}"; t="${t%\'}"; t="${t#\'}"
    [ -z "$t" ] && return 0
    [[ "$t" == -* ]] && return 0
    case "$t" in
        *'$'*|*'`'*|*'*'*|*'?'*|*'['*|*'('*|*'{'*|'='*|'>'*|'<'*) return 0 ;;
    esac
    [[ "$t" == '~'* ]] && t="${t/#\~/$HOME}"
    [[ "$t" != /* ]] && t="$cwd/$t"
    python3 -c "import os.path,sys; print(os.path.normpath(sys.argv[1]))" "$t" 2>/dev/null || return 0
}

# Resolve the path token; fall back to cwd when absent/unresolvable.
target=""
if [ -n "$pathtok" ]; then
    rp=$(resolve_path "$pathtok")
    [ -n "$rp" ] && [ -e "$rp" ] && target="$rp"
fi
[ -z "$target" ] && target="$cwd"

# Direct subprocess (not the agent's Bash tool) → no PreToolUse recursion.
# The binary owns query + emit + record + the docs/already_loaded split.
response=$(trace docs "$target" \
    --source trace_inject_hook \
    --triggering-tool Bash \
    --triggering-command "$command" \
    --json 2>/tmp/inject-docs.err)
status=$?

if [ "$status" -ne 0 ]; then
    err=$(cat /tmp/inject-docs.err 2>/dev/null)
    cat >&2 <<EOF
BLOCKED: project-docs load failed for: $target

\`trace docs "$target" ...\` exited $status. The trace command is
blocked so the agent does not run it without project-docs context.

Underlying error:
$err
EOF
    exit 2
fi

doc_count=$(echo "$response" | jq -r '.doc_count // 0' 2>/dev/null) || doc_count=0
[ "$doc_count" -eq 0 ] 2>/dev/null && exit 0

jq -nc --arg ctx "$response" '{
  hookSpecificOutput: {
    hookEventName: "PreToolUse",
    additionalContext: $ctx
  }
}'
exit 0
