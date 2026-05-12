#!/bin/bash
# Block file mutations when state is "proposing" (not yet approved)
# Covers Write|Edit|NotebookEdit (via tool_input.file_path)
# AND Bash file-mutating commands (via tool_input.command)
# Gracefully allows on any error (file missing, parse error)

read -r input

session_id=$(echo "$input" | jq -r '.session_id // ""' 2>/dev/null) || exit 0
[ -z "$session_id" ] && exit 0

state_file="/tmp/claude-session-state-${session_id}"
[ ! -f "$state_file" ] && "$HOME/.claude/hooks/initialize-session-state.sh" "$session_id"

state=$(jq -r '.state // "proposing"' "$state_file" 2>/dev/null) || exit 0
[ "$state" != "proposing" ] && exit 0

file_path=$(echo "$input" | jq -r '.tool_input.file_path // ""' 2>/dev/null) || file_path=""
command=$(echo "$input" | jq -r '.tool_input.command // ""' 2>/dev/null) || command=""
cwd=$(echo "$input" | jq -r '.cwd // ""' 2>/dev/null) || cwd=""
[ -z "$cwd" ] && cwd="$(pwd)"

block_message() {
    cat >&2 <<'EOF'
BLOCKED: A proposal is expected — do not edit code.

Update your proposal based on the user's feedback and present it again.
Only edit code after the user approves.
EOF
    exit 2
}

# --- Write/Edit/NotebookEdit branch ---
if [ -n "$file_path" ]; then
    [[ "$file_path" == *"/.claude/shaping/"* ]] && exit 0
    [[ "$file_path" == *"/.claude/plans/"* ]] && exit 0
    block_message
fi

# --- Bash branch ---
[ -z "$command" ] && exit 0

# Extract file-mutation target paths from the command string.
# Patterns covered:
#   1. Stdout/stderr redirects (>, >>, 1>, 2>, &>)
#   2. tee / tee -a
#   3. sed -i ... <file>
#   4. dd of=<path>
#   5. cp / mv / install <...> <dest>
#   6. touch / truncate <paths...>
# Heredoc-into-redirect (cat <<EOF > path) is covered by pattern 1.

targets=()

# 1. Redirects
while IFS= read -r tok; do
    [ -n "$tok" ] && targets+=("$tok")
done < <(echo "$command" | grep -oE '([0-9]?&?>{1,2})[[:space:]]*[^[:space:]|;&<>(){}`]+' | sed -E 's/^[0-9]?&?>{1,2}[[:space:]]*//')

# 2. tee / tee -a
while IFS= read -r tok; do
    [ -n "$tok" ] && targets+=("$tok")
done < <(echo "$command" | grep -oE '\btee\b([[:space:]]+-a)?[[:space:]]+[^[:space:]|;&<>(){}`]+' | sed -E 's/^tee([[:space:]]+-a)?[[:space:]]+//')

# 3. sed -i — last positional arg of the sed invocation
if echo "$command" | grep -qE '\bsed\b[[:space:]]+([^|;&]*[[:space:]])?-i\b'; then
    seg=$(echo "$command" | sed -E 's/^.*\bsed\b[[:space:]]+//' | sed -E 's/[|;&].*$//')
    last_tok=$(echo "$seg" | awk '{print $NF}')
    [ -n "$last_tok" ] && targets+=("$last_tok")
fi

# 4. dd of=path
while IFS= read -r tok; do
    [ -n "$tok" ] && targets+=("$tok")
done < <(echo "$command" | grep -oE '\bof=[^[:space:]|;&]+' | sed -E 's/^of=//')

# 5. cp / mv / install — destination is the last positional arg
for op in cp mv install; do
    if echo "$command" | grep -qE "\\b${op}\\b"; then
        seg=$(echo "$command" | sed -E "s/^.*\\b${op}\\b[[:space:]]+//" | sed -E 's/[|;&].*$//')
        non_flags=$(echo "$seg" | tr ' ' '\n' | grep -v '^-' | grep -v '^$')
        last_tok=$(echo "$non_flags" | tail -n1)
        [ -n "$last_tok" ] && targets+=("$last_tok")
    fi
done

# 6. touch / truncate — every positional arg is a target
for op in touch truncate; do
    if echo "$command" | grep -qE "\\b${op}\\b"; then
        seg=$(echo "$command" | sed -E "s/^.*\\b${op}\\b[[:space:]]+//" | sed -E 's/[|;&].*$//')
        while IFS= read -r tok; do
            [ -n "$tok" ] && targets+=("$tok")
        done < <(echo "$seg" | tr ' ' '\n' | grep -v '^-' | grep -v '^$')
    fi
done

# No mutation patterns matched → allow
[ ${#targets[@]} -eq 0 ] && exit 0

for raw in "${targets[@]}"; do
    # Strip surrounding quotes
    t="${raw%\"}"; t="${t#\"}"
    t="${t%\'}"; t="${t#\'}"

    # Always allow these device targets
    case "$t" in
        /dev/null|/dev/stdout|/dev/stderr|/dev/tty) continue ;;
    esac

    # Expand leading tilde
    if [[ "$t" == ~* ]]; then
        t="${t/#\~/$HOME}"
    fi

    # Resolve relative paths against cwd
    if [[ "$t" != /* ]]; then
        t="$cwd/$t"
    fi

    # Normalize (collapse .. and . segments)
    t=$(python3 -c "import os.path,sys; print(os.path.normpath(sys.argv[1]))" "$t" 2>/dev/null) || continue

    # Planning-artifact whitelist (mirrors the file_path branch above)
    [[ "$t" == *"/.claude/shaping/"* ]] && continue
    [[ "$t" == *"/.claude/plans/"* ]] && continue

    # Inside the project root (cwd or below) → block
    if [[ "$t" == "$cwd"/* || "$t" == "$cwd" ]]; then
        block_message
    fi
done

exit 0
