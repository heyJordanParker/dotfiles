#!/bin/bash
# Force code reading/searching through `trace` instead of the raw tools it
# replaces, and stop the agent filtering trace's own output.
#
#   A. `trace` piped into a text-trimmer (grep/rg/head/tail/sed/awk/cut/
#      sort/uniq/wc/column/fold/tr) or jq, or redirected into a repo file —
#      filtering defeats trace's enriched intelligence (callers, complexity,
#      nearest Claude.md + rules, git activity). Partial output comes from
#      the in-binary `trace ... --json --filter '<jq>'`, never a pipe.
#   B. raw cat/grep/egrep/fgrep/rg/find/sed/awk/head/tail invoked against a
#      path that exists inside the repo — use the equivalent trace
#      subcommand.
#
# An unpiped, unredirected `trace` always passes — including while
# state=proposing — so research tooling is never caught by proposing-mode.
# This hook never inspects state; it is orthogonal to
# block-edits-during-proposal.sh.
#
# Graceful exit 0 on any infrastructure failure (missing jq/python3, parse
# error, no command) — a hook never blocks the agent because it is broken.

read -r input

command=$(echo "$input" | jq -r '.tool_input.command // ""' 2>/dev/null) || exit 0
[ -z "$command" ] && exit 0
cwd=$(echo "$input" | jq -r '.cwd // ""' 2>/dev/null) || cwd=""
[ -z "$cwd" ] && cwd="$(pwd)"

trimmers='grep|egrep|fgrep|rg|sed|awk|head|tail|cut|sort|uniq|wc|column|fold|tr|jq'

block_message() {
    cat >&2 <<'EOF'
BLOCKED: don't filter trace or hand-roll code reads.

trace returns scoped code intelligence — callers, complexity, nearest
Claude.md + rules, git activity. Piping it through grep/head/sed/awk/jq,
or using raw grep/find/sed/cat on repo files, throws that away.

Re-run the trace command with no pipe and no redirect; read all of it:
  grep -r / rg         -> trace grep <pattern> [-l <lang>]
  cat / head / sed -n  -> trace read <file> [<method>]
  find                 -> trace find <pattern> [<base>]
For partial output, use the in-binary filter — never a pipe:
  trace ... | jq '<expr>'  -> trace ... --json --filter '<expr>'
EOF
    exit 2
}

# Resolve a raw token to an absolute, normalized path. Echoes nothing for
# flags, non-path-shaped tokens (glob/var/substitution), or unresolvable ones.
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

# 0 = path is inside the repo and not whitelisted; 1 = outside or whitelisted.
inside_repo() {
    local p="$1"
    case "$p" in
        /dev/null|/dev/stdout|/dev/stderr|/dev/tty) return 1 ;;
    esac
    [[ "$p" == *"/.claude/shaping/"* ]] && return 1
    [[ "$p" == *"/.claude/plans/"* ]] && return 1
    [[ "$p" == *"/.tracer-cache/"* ]] && return 1
    [[ "$p" == "$cwd"/* || "$p" == "$cwd" ]] && return 0
    return 1
}

# --- A. trace piped into a trimmer / redirected into the repo -------------

if echo "$command" | grep -qE '\btrace\b'; then
    after=$(echo "$command" | sed -E 's/^.*\btrace\b//')
    if echo "$after" | grep -qE '\|[[:space:]]*(sudo[[:space:]]+)?('"$trimmers"')\b'; then
        block_message
    fi
    while IFS= read -r tok; do
        [ -z "$tok" ] && continue
        rp=$(resolve_path "$tok")
        [ -n "$rp" ] && inside_repo "$rp" && block_message
    done < <(echo "$after" | grep -oE '([0-9]?&?>{1,2})[[:space:]]*[^[:space:]|;&<>(){}`]+' | sed -E 's/^[0-9]?&?>{1,2}[[:space:]]*//')
fi

# --- B. raw replaced tools against an in-repo path ------------------------

# Split into pipeline/list segments. Normalize two-char operators to single
# chars first (BSD sed has no \n in replacement), then split with tr.
segments=$(echo "$command" | sed -E 's/\|\|/|/g; s/&&/\&/g' | tr '|;&' '\n')

while IFS= read -r seg; do
    [ -z "$seg" ] && continue
    seg=$(echo "$seg" | sed -E 's/^[[:space:]]+//; s/^([A-Za-z_][A-Za-z0-9_]*=[^[:space:]]*[[:space:]]+)+//')
    head_tok=$(echo "$seg" | awk '{print $1}')
    [ -z "$head_tok" ] && continue
    case "${head_tok##*/}" in
        cat|grep|egrep|fgrep|rg|find|sed|awk|head|tail) ;;
        *) continue ;;
    esac
    rest=$(echo "$seg" | cut -d' ' -f2-)
    for tok in $rest; do
        rp=$(resolve_path "$tok") || continue
        [ -z "$rp" ] && continue
        if [ -e "$rp" ] && inside_repo "$rp"; then
            block_message
        fi
    done
done <<EOF
$segments
EOF

exit 0
