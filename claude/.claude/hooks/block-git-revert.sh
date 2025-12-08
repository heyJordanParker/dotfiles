#!/bin/bash
# Block destructive git operations that agents misuse for "reverting"

read -r input

command=$(echo "$input" | jq -r '.tool_input.command // ""')

# Normalize: strip flags that can evade detection (-C <path>, -c <key=val>, etc.)
normalized=$(echo "$command" | sed -E 's/git[[:space:]]+-[Cc][[:space:]]+[^[:space:]]+/git/g; s/git[[:space:]]+--git-dir[=][^[:space:]]+/git/g; s/git[[:space:]]+--work-tree[=][^[:space:]]+/git/g')

# Pattern 1: git reset (all forms are destructive)
if [[ "$normalized" =~ git[[:space:]]+reset ]]; then
  cat << 'EOF' >&2
BLOCKED: git reset is a destructive operation.

If you want to revert changes to specific lines, use the Edit tool to manually undo those changes.
If you truly need git reset, ask the user to run it manually.
EOF
  exit 2
fi

# Pattern 2: git checkout of files (not branch switches)
# Allow: --ours/--theirs (legitimate during merge/rebase conflicts)
if echo "$normalized" | grep -qE 'git[[:space:]]+checkout[[:space:]]+.*(--ours|--theirs)'; then
  exit 0
fi
# Block: checkout <ref> -- <path> (e.g., git checkout abc123^ -- .)
if echo "$normalized" | grep -qE 'git[[:space:]]+checkout[[:space:]]+[^[:space:]]+[[:space:]]+--[[:space:]]+'; then
  cat << 'EOF' >&2
BLOCKED: git checkout <ref> -- <path> is a destructive operation.

If you want to revert changes to specific lines, use the Edit tool to manually undo those changes.
If you truly need to checkout from a commit, ask the user to run the git command manually.
EOF
  exit 2
fi
# Block: checkout -- <file>, checkout <file.ext>, checkout <path/file>
if echo "$normalized" | grep -qE 'git[[:space:]]+checkout[[:space:]]+(--[[:space:]]+|[^-][^[:space:]]*\.[^[:space:]]+|[^-][^[:space:]]*/[^[:space:]]+)'; then
  cat << 'EOF' >&2
BLOCKED: git checkout of files is a destructive operation.

If you want to revert changes to specific lines, use the Edit tool to manually undo those changes.
If you truly need to discard file changes, ask the user to run the git command manually.
EOF
  exit 2
fi

# Pattern 3: git restore (file restoration)
if [[ "$normalized" =~ git[[:space:]]+restore ]]; then
  cat << 'EOF' >&2
BLOCKED: git restore is a destructive operation.

If you want to revert changes to specific lines, use the Edit tool to manually undo those changes.
If you truly need to restore files, ask the user to run the git command manually.
EOF
  exit 2
fi

exit 0
