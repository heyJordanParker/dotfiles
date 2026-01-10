#!/bin/bash
query=$(cat | jq -r '.query')

# ==============================================================================
# Exclusion Lists
# ==============================================================================

# Hard excludes: Always filtered out (build artifacts, caches, temp files)
# Note: Use globs for patterns with spaces (? matches single char)
HARD_EXCLUDES=(
  .git
  node_modules
  vendor
  __pycache__
  .venv
  .Trash
  Library/Caches
  'Library/Application?Support'
)

# Soft excludes: Filtered from fuzzy/deep search, but allowed in prefix match
# User typing ~/Lib finds ~/Library, but ~/acss won't find ~/Library/**/acss
SOFT_EXCLUDES=(
  Library
  Applications
  Movies
  Music
  Pictures
)

# ==============================================================================
# Helpers
# ==============================================================================

# Build fd --exclude args from an array (space-separated for word splitting)
build_excludes() {
  local arr=("$@")
  for item in "${arr[@]}"; do
    printf '%s ' "--exclude=$item"
  done
}

# Sort by recency (most recently modified first)
sort_by_recency() {
  while IFS= read -r f; do
    [[ -e "$f" ]] && stat -f '%m %N' "$f" 2>/dev/null
  done | sort -rn | cut -d' ' -f2-
}

# Dedupe while preserving order (normalize trailing slashes)
dedupe() {
  awk '{ key=$0; gsub(/\/$/, "", key); if (!seen[key]++) print }'
}

# ==============================================================================
# Global Path Completion (/, ~, $VAR)
# ==============================================================================

if [[ "$query" =~ ^[/~\$] ]]; then
  # Handle ~ without slash: ~ → ~/, ~foo → ~/foo
  if [[ "$query" =~ ^~[^/] ]]; then
    query="~/${query#\~}"
  elif [[ "$query" == "~" ]]; then
    query="~/"
  fi

  expanded="${query/#\~/$HOME}"
  expanded=$(eval echo "$expanded" 2>/dev/null || echo "$expanded")

  # Parse into directory + partial name
  if [[ "$expanded" == */ ]]; then
    search_dir="$expanded"
    search_pattern=""
  else
    search_dir=$(dirname "$expanded")
    search_pattern=$(basename "$expanded")
  fi

  # Normalize: ensure single trailing slash, handle root specially
  [[ "$search_dir" != */ ]] && search_dir="${search_dir}/"
  [[ "$search_dir" == "//" ]] && search_dir="/"

  # Hidden files only if pattern starts with .
  hidden_flag=""
  [[ "$search_pattern" == .* ]] && hidden_flag="--hidden"

  name_len=${#search_pattern}
  hard_excludes=$(build_excludes "${HARD_EXCLUDES[@]}")
  all_excludes=$(build_excludes "${HARD_EXCLUDES[@]}" "${SOFT_EXCLUDES[@]}")

  # Helper: if single dir match, also show its children
  expand_single_dir() {
    local input
    input=$(cat)
    local count
    count=$(echo "$input" | grep -c .)

    echo "$input"

    # If exactly one result and it's a directory, show its children too
    if [[ $count -eq 1 ]]; then
      local dir="${input%/}"
      if [[ -d "$dir" ]]; then
        fd $hidden_flag --max-depth 1 $hard_excludes "" "$dir/" 2>/dev/null \
          | head -10 | sort_by_recency
      fi
    fi
  }

  if [[ $name_len -lt 3 ]]; then
    # Short pattern: direct children first, zoxide fallback if empty
    short_results=$(fd $hidden_flag --max-depth 1 --glob "${search_pattern}*" $hard_excludes "$search_dir" 2>/dev/null \
      | head -30 | sort_by_recency)
    if [[ -n "$short_results" ]]; then
      echo "$short_results"
    else
      zoxide query -l "$search_pattern" 2>/dev/null | head -10
    fi
  else
    {
      # 1. zoxide frecency (already curated by user behavior)
      zoxide query -l "$search_pattern" 2>/dev/null | head -5

      # 2. Direct children prefix match (no soft excludes - user is typing toward it)
      fd $hidden_flag --max-depth 1 --glob "${search_pattern}*" $hard_excludes "$search_dir" 2>/dev/null \
        | head -20 | sort_by_recency

      # 3. Deep fuzzy search (apply soft excludes - avoid noise from rarely-used dirs)
      fd $hidden_flag --min-depth 2 --max-depth 4 $all_excludes "$search_pattern" "$search_dir" 2>/dev/null \
        | head -20 | sort_by_recency
    } | dedupe | expand_single_dir | head -30
  fi
  exit 0
fi

# ==============================================================================
# Project-Local Search (default)
# ==============================================================================

cd "$CLAUDE_PROJECT_DIR" || exit 1

PROJECT_EXCLUDES=(
  .git
  node_modules
  vendor
  dist
  build
  .next
  __pycache__
  .venv
  .bun
  worktrees
  .worktrees
)

project_excludes=$(build_excludes "${PROJECT_EXCLUDES[@]}")

if [[ ${#query} -lt 2 ]]; then
  fd --type f --hidden --max-depth 2 --exclude .git "" 2>/dev/null \
    | head -50 | sort_by_recency | head -15
else
  fd --type f --hidden $project_excludes "$query" 2>/dev/null \
    | head -50 | sort_by_recency | head -15
fi
