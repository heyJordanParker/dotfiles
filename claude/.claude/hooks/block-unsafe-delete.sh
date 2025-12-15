#!/bin/bash
# Block rm commands outside whitelisted directories

# === WHITELIST - add directories here ===
ALLOWED_PREFIXES=(
  "/Users/jordan/dotfiles"
  "/Users/jordan/Developer"
  "/Users/jordan/Downloads"
  "/Users/jordan/Desktop"
  "/Users/jordan/conductor"
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

# Only check commands containing rm
# Match rm as a word (not part of another command like 'perform')
# Also match rm in subshells: $(rm ...) or `rm ...`
if ! echo "$command" | grep -qE '(^|[;&|[:space:]\$\(`])rm([[:space:]]|$)'; then
  exit 0
fi

# Piped rm (xargs rm, etc.) - can't validate paths, check cwd instead
if echo "$command" | grep -qE '\|.*rm([[:space:]]|$)'; then
  if ! is_allowed "$cwd"; then
    cat >&2 <<EOF
BLOCKED: Piped rm command outside allowed directories.

Cannot validate paths for piped rm commands.
Allowed: $(allowed_list)
Please run this command manually.
EOF
    exit 2
  fi
  exit 0
fi

# Subshell rm $(rm ...) or `rm ...` - can't reliably parse, check cwd
if echo "$command" | grep -qE '(\$\(|`)rm[[:space:]]'; then
  if ! is_allowed "$cwd"; then
    cat >&2 <<EOF
BLOCKED: Subshell rm command outside allowed directories.

Cannot validate paths for subshell rm commands.
Allowed: $(allowed_list)
Please run this command manually.
EOF
    exit 2
  fi
  exit 0
fi

# Check for globs - if present, validate cwd is in allowed dirs
if echo "$command" | grep -qE 'rm[[:space:]]+[^|;&]*[*?\[]'; then
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

# Extract paths from rm command (skip flags starting with -)
# Only extract if rm is followed by space+args
if ! echo "$command" | grep -qE 'rm[[:space:]]+'; then
  exit 0  # rm with no args - nothing to delete
fi
paths=$(echo "$command" | sed -E 's/.*rm[[:space:]]+//' | tr ' ' '\n' | grep -v '^-' | grep -v '^$')

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
