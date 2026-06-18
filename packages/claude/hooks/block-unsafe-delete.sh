#!/bin/bash
# Block rm commands outside whitelisted directories

# Resolve script path through any symlinks (Claude Code wires this hook from
# ~/.claude/hooks/block-unsafe-delete.sh, a stow symlink into the repo).
SOURCE="${BASH_SOURCE[0]}"
while [ -L "$SOURCE" ]; do
  TARGET="$(readlink "$SOURCE")"
  case "$TARGET" in
    /*) SOURCE="$TARGET" ;;
    *) SOURCE="$(cd -P "$(dirname "$SOURCE")" && pwd)/$TARGET" ;;
  esac
done
DOTFILES_DIR="$(cd -P "$(dirname "$SOURCE")/../../.." && pwd)"

# === WHITELIST - add directories here ===
ALLOWED_PREFIXES=(
  "$DOTFILES_DIR"
  "$HOME/Developer"
  "$HOME/Downloads"
  "$HOME/Desktop"
  "$HOME/conductor"
  "$HOME/.claude"
  "/tmp"
)

read -r input

command=$(echo "$input" | jq -r '.tool_input.command // ""')
cwd=$(echo "$input" | jq -r '.cwd // ""')

is_allowed() {
  local path="$1"
  for prefix in "${ALLOWED_PREFIXES[@]}"; do
    [[ "$path" == "$prefix"* ]] && return 0
  done
  return 1
}

# Generate allowed dirs list for error messages
allowed_list() {
  printf '%s\n' "${ALLOWED_PREFIXES[@]}" | sed "s|$HOME|~|g" | paste -sd ',' - | sed 's/,/, /g'
}

unresolvable() {
  cat >&2 <<EOF
BLOCKED: rm with a target this guard can't resolve.

This rm reaches its target through a pipe, a command substitution, or
an unexpanded shell variable, so the guard can't tell what it deletes.

Run rm against a literal path inside an allowed directory:
$(allowed_list)
Or delete the file manually.
EOF
  exit 2
}

# Match rm as a command word, including a qualified path (/bin/rm) or a `\`
# alias-bypass (\rm). The word boundary allows an optional `\` and optional path/
# prefix; 'perform', 'charm' etc. don't match.
if ! echo "$command" | grep -qE '(^|[;&|[:space:]$(`])\\?([^[:space:]]*/)?rm([[:space:]]|$)'; then
  exit 0
fi

# Piped rm (xargs rm, etc.) - targets come from stdin, unknowable.
if echo "$command" | grep -qE '\|.*rm([[:space:]]|$)'; then
  unresolvable
fi

# rm inside a command substitution - $(rm ...) / `rm ...`, qualified path or
# `\` alias-bypass too.
if echo "$command" | grep -qE '(\$\(|`)\\?([^[:space:]]*/)?rm[[:space:]]'; then
  unresolvable
fi

# rm with no operands - nothing to delete.
if ! echo "$command" | grep -qE 'rm[[:space:]]+'; then
  exit 0
fi
paths=$(echo "$command" | sed -E 's/.*rm[[:space:]]+//' | tr ' ' '\n' | grep -v '^-' | grep -v '^$')

# Any operand reached through an unexpanded expansion - can't resolve, block.
if echo "$paths" | grep -qE '[$`]'; then
  unresolvable
fi

# Glob operands - the matched set is fixed by cwd at runtime; gate on cwd.
if echo "$paths" | grep -qE '[*?[]'; then
  if ! is_allowed "$cwd"; then
    cat >&2 <<EOF
BLOCKED: rm with glob pattern outside allowed directories.

Allowed: $(allowed_list)
Please delete these files manually.
EOF
    exit 2
  fi
  exit 0
fi

for path in $paths; do
  # Expand tilde
  if [[ "$path" == ~* ]]; then
    path="${path/#\~/$HOME}"
  fi
  # Resolve to absolute path
  if [[ "$path" != /* ]]; then
    path="$cwd/$path"
  fi
  # Normalize (resolve .. and .)
  path=$(python3 -c "import os.path; print(os.path.normpath('$path'))")

  if ! is_allowed "$path"; then
    cat >&2 <<EOF
BLOCKED: Cannot delete '$path'

Allowed: $(allowed_list)
Please delete this file manually.
EOF
    exit 2
  fi
done

exit 0
