#!/bin/bash
# PreToolUse(Bash): when the agent runs a path-taking `trace` command,
# inject the project-docs (Claude.md / .claude rules ancestors) for the
# resolved path into the model's context via additionalContext.
#
# Why: `trace read` no longer auto-injects project-docs (off by default).
# This hook owns that surface for trace usage, sourcing the deduped docs
# from `trace docs <path> --json` so per-session read-once dedupe is shared
# with any explicit `trace docs` / `read --docs` in the same session.
#
# LOCAL-ONLY: wired in settings.json only, never in the plugin-distributed
# hooks.json. Tracer hooks are our experimental local surface; plugin users
# get tracer as a command, never these hooks.
#
# Behavior:
# - Non-`trace` Bash, or a `trace` subcommand that takes no path → exit 0
#   (clean no-op, never blocks).
# - Path-taking `trace` subcommand → resolve the path, run `trace docs`.
#   - docs found → inject them, exit 0.
#   - empty doc set (none for the path, or already surfaced this session
#     via the shared dedupe) → inject nothing, exit 0.
#   - `trace docs` FAILS (non-zero) → BLOCK the trace command, exit 2, with
#     a loud explicit reason on stderr naming the path + the error. The
#     agent must not silently trace without project docs.
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
docs_json=$(trace docs "$target" --json 2>/tmp/inject-docs.err)
status=$?

if [ "$status" -ne 0 ]; then
    err=$(cat /tmp/inject-docs.err 2>/dev/null)
    cat >&2 <<EOF
BLOCKED: project-docs load failed for: $target

\`trace docs "$target" --json\` exited $status. The trace command is
blocked so the agent does not run it without project-docs context.

Underlying error:
$err
EOF
    exit 2
fi

count=$(echo "$docs_json" | jq -r '.doc_count // 0' 2>/dev/null) || count=0
[ "$count" -eq 0 ] 2>/dev/null && exit 0

jq -nc --arg ctx "$docs_json" '{
  hookSpecificOutput: {
    hookEventName: "PreToolUse",
    additionalContext: $ctx
  }
}'
exit 0
