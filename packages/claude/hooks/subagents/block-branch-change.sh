#!/bin/bash
# Block branch-changing git operations for SUBAGENTS ONLY.
# Subagents share the parent's worktree; switching branches under sibling
# agents corrupts shared state. The main session is human-driven and keeps
# switching branches freely.
# Gracefully allows on any parse error.

read -r input

agent_id=$(echo "$input" | jq -r '.agent_id // empty' 2>/dev/null) || exit 0
[ -z "$agent_id" ] && exit 0   # main session — allow

command=$(echo "$input" | jq -r '.tool_input.command // ""' 2>/dev/null) || exit 0

# Strip git global flags to avoid evasion via -C/-c/--git-dir=/--work-tree=
normalized=$(echo "$command" | sed -E 's/git[[:space:]]+-[Cc][[:space:]]+[^[:space:]]+/git/g; s/git[[:space:]]+--git-dir[=][^[:space:]]+/git/g; s/git[[:space:]]+--work-tree[=][^[:space:]]+/git/g')

# Skip: any command with a ` -- ` separator is file-revert syntax,
# owned by block-git-revert.sh. We do not block what isn't our concern.
if echo "$normalized" | grep -qE '[[:space:]]--[[:space:]]'; then
  exit 0
fi

# Block:
#   git switch ...                 (switch is exclusively branch-related)
#   git branch -m/-M/-d/-D ...     (rename/delete branches)
#   git checkout -b/-B ...         (create branch)
#   git checkout <branch-name>     (bare alphanumeric token; no path chars)
# block-git-revert.sh already owns `git checkout <ref> -- <path>` and
# file-checkout shapes (anything with '/' or '.'), so this hook only
# catches the branch-switch path it lets through.
if echo "$normalized" | grep -qE '(git[[:space:]]+switch([[:space:]]|$))|(git[[:space:]]+branch[[:space:]]+-[mMdD])|(git[[:space:]]+checkout[[:space:]]+-[bB])|(git[[:space:]]+checkout[[:space:]]+[A-Za-z0-9_@][A-Za-z0-9_@-]*([[:space:]]|$))'; then
  cat << 'EOF' >&2
BLOCKED: branch changes are BANNED for subagents.

You are a subagent. The worktree is shared with the parent and sibling
subagents. Switching the branch moves HEAD under everyone — silently
corrupting their work. The main session handles branch changes; you do not.

Do NOT run: git switch, git checkout <branch>, git checkout -b/-B,
or git branch -m/-M/-d/-D — and do not route around this via an alias,
sh -c, or git -c alias.*=switch.

If a branch change is genuinely required, return to the user and state
plainly that a branch change is needed. The user runs it.
EOF
  exit 2
fi

exit 0
