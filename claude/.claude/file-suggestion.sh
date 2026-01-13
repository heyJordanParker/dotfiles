#!/bin/bash
query=$(cat | jq -r '.query')

# Normalize query: strip asterisks (trailing slash handled after global detection)
query="${query//\*/}"

# ==============================================================================
# Excludes
# ==============================================================================

EXCLUDES=(
  .git
  node_modules
  vendor
  __pycache__
  .venv
  .Trash
  tmp
  dist
  build
  .next
  .bun
  worktrees
  .worktrees
)

GLOBAL_EXCLUDES=(
  Library
  Applications
  Movies
  Music
  Pictures
)

# ==============================================================================
# Helpers
# ==============================================================================

build_excludes() {
  for item in "$@"; do
    printf '%s ' "--exclude=$item"
  done
}

# Sort by depth (shallow first), then recency within same depth
sort_by_depth_then_recency() {
  # Batch stat call (1 process vs N), pure bash depth calculation (0 subprocesses vs N)
  xargs stat -f '%m %N' 2>/dev/null | while IFS= read -r line; do
    mtime="${line%% *}"
    path="${line#* }"
    slashes="${path//[^\/]/}"
    depth="${#slashes}"
    [[ "$path" == */ ]] && ((depth--))
    printf '%d %s %s\n' "$depth" "$mtime" "$path"
  done | sort -n -k1,1 -k2,2rn | cut -d' ' -f3-
}

dedupe() {
  awk '{ key=$0; gsub(/\/$/, "", key); if (!seen[key]++) print }'
}

# Parse path into search_dir + pattern (sets global vars)
parse_query() {
  local path="$1"
  local default_root="$2"

  if [[ "$path" == */ ]]; then
    search_dir="$path"
    pattern=""
  elif [[ "$path" == */* ]]; then
    search_dir="$(dirname "$path")/"
    pattern="$(basename "$path")"
  else
    search_dir="$default_root"
    pattern="$path"
  fi
}

# Expand directories when results are sparse (only direct children of root)
expand_sparse_dirs() {
  local root="$1"
  local -a results=()
  while IFS= read -r line; do
    results+=("$line")
  done

  if [[ ${#results[@]} -lt 5 ]]; then
    for path in "${results[@]}"; do
      echo "$path"
      # Expand if: single result OR direct child of search root
      if [[ -d "${path%/}" && ( ${#results[@]} -eq 1 || "$(dirname "${path%/}")" == "${root%/}" ) ]]; then
        fd --hidden --max-depth 1 "" "${path%/}/" 2>/dev/null
      fi
    done
  else
    printf '%s\n' "${results[@]}"
  fi
}

# ==============================================================================
# Unified Search
# ==============================================================================

search_files() {
  local root="$1"
  local query="$2"
  local global="$3"  # non-empty for global, empty for local

  local hidden_flag="" excludes="" depth_flag=""
  if [[ -n "$global" ]]; then
    [[ "$query" == .* ]] && hidden_flag="--hidden"
    excludes=$(build_excludes "${EXCLUDES[@]}" "${GLOBAL_EXCLUDES[@]}")
    [[ -z "$query" ]] && depth_flag="--max-depth 1" || depth_flag="--max-depth 2"
  else
    hidden_flag="--hidden"  # Always show hidden in local projects
    excludes=$(build_excludes "${EXCLUDES[@]}")
    [[ -z "$query" ]] && depth_flag="--max-depth 1" || depth_flag="--max-depth 3"
  fi

  {
    # 1. Prefix matches
    fd $hidden_flag $depth_flag $excludes --glob "${query}*" "$root" 2>/dev/null

    # 2. Zoxide frecency (global only, filtered to search root)
    [[ -n "$global" && -n "$query" ]] && zoxide query -l "$query" 2>/dev/null | grep "^${root%/}"

    # 3. Fuzzy matches
    [[ -n "$query" ]] && fd $hidden_flag $depth_flag $excludes "$query" "$root" 2>/dev/null
  } | dedupe | expand_sparse_dirs "$root" | sort_by_depth_then_recency | sed "s|^\./||; s|^$HOME|~|" | head -30
}

# ==============================================================================
# Main
# ==============================================================================

# Global path (/, ~, $VAR)
if [[ "$query" =~ ^[/~\$] ]]; then
  # Handle ~ expansion
  if [[ "$query" =~ ^~[^/] ]]; then
    query="~/${query#\~}"
  elif [[ "$query" == "~" ]]; then
    query="~/"
  fi

  expanded="${query/#\~/$HOME}"
  expanded=$(eval echo "$expanded" 2>/dev/null || echo "$expanded")

  parse_query "$expanded" "/"
  [[ "$search_dir" == "//" ]] && search_dir="/"

  search_files "$search_dir" "$pattern" "1"

# Local project search
else
  cd "$CLAUDE_PROJECT_DIR" || exit 1
  parse_query "$query" "."
  search_files "$search_dir" "$pattern" ""
fi
