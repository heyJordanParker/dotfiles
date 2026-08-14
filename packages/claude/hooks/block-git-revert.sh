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
If you are here to prove a failure is pre-existing: stop. That is the
orchestrator's call, not yours. Report the exact command and its red output and
let it attribute the failure. Do not look for another route to a before state.
If a human truly needs this, the human runs it manually.
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
If you are here to prove a failure is pre-existing: stop. That is the
orchestrator's call, not yours. Report the exact command and its red output and
let it attribute the failure. Do not look for another route to a before state.
If a human truly needs this, the human runs it manually.
EOF
  exit 2
fi
# Block: checkout -- <file>, checkout <file.ext>, checkout <path/file>
if echo "$normalized" | grep -qE 'git[[:space:]]+checkout[[:space:]]+(--[[:space:]]+|[^-][^[:space:]]*\.[^[:space:]]+|[^-][^[:space:]]*/[^[:space:]]+)'; then
  cat << 'EOF' >&2
BLOCKED: git checkout of files is a destructive operation.

If you want to revert changes to specific lines, use the Edit tool to manually undo those changes.
If you are here to prove a failure is pre-existing: stop. That is the
orchestrator's call, not yours. Report the exact command and its red output and
let it attribute the failure. Do not look for another route to a before state.
If a human truly needs this, the human runs it manually.
EOF
  exit 2
fi

# Pattern 3: git restore (file restoration)
if [[ "$normalized" =~ git[[:space:]]+restore ]]; then
  cat << 'EOF' >&2
BLOCKED: git restore is a destructive operation.

If you want to revert changes to specific lines, use the Edit tool to manually undo those changes.
If you are here to prove a failure is pre-existing: stop. That is the
orchestrator's call, not yours. Report the exact command and its red output and
let it attribute the failure. Do not look for another route to a before state.
If a human truly needs this, the human runs it manually.
EOF
  exit 2
fi

# Pattern 4: git stash is BANNED. Only pure read-only inspection is allowed
# (stash list / stash show). Subtract those occurrences, then block if ANY
# stash verb survives — so a mutating stash cannot be smuggled past by
# appending `&& git stash list`.
residual=$(echo "$normalized" | sed -E 's/git[[:space:]]+stash[[:space:]]+(list|show)[^&|;]*//g')
if [[ "$residual" =~ git[[:space:]]+stash ]]; then
  cat << 'EOF' >&2
BLOCKED: git stash is BANNED for agents. This is not a soft limit. Never run it.

git stash hides or discards uncommitted work in a worktree shared by other
agents. It is the single most common way agent work is silently lost. It has
already destroyed real work in this repo.

Do NOT stash. Do NOT pop, drop, clear, push, apply, or save a stash. Do NOT
hide a mutating stash behind a trailing `&& git stash list`, an alias, sh -c,
or git -c alias.*=stash. Adding a read-only stash command does not make this
allowed.

To run something against a clean tree: commit your work first, then run it.
If you are here to prove a failure is pre-existing: stop. That is the
orchestrator's call, not yours. Report the exact command and its red output and
let it attribute the failure. Do not look for another route to a before state.
If a human truly needs this, the human runs it manually.
EOF
  exit 2
fi

exit 0
