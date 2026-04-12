#!/bin/bash
# Unified file suggestion for Claude Code's @ file picker.
# One smart search: project files first, workspace fills gaps.
# @query    → search project, then workspace if sparse
# @~/path   → explicit home navigation
# @$HOME/   → explicit home navigation
# Absolute paths (@/etc/hosts) handled natively by Claude Code.

set -o pipefail

# ==============================================================================
# Input
# ==============================================================================

if [[ -n "$TEST_MODE" ]]; then
  query="$1"
  PROJECT_DIR="$2"
else
  query=$(cat | jq -r '.query')
  PROJECT_DIR="${CLAUDE_PROJECT_DIR:-.}"
fi

query="${query//\*/}"
HOME_DIR="$HOME"
ABS_PROJECT=$(cd "$PROJECT_DIR" 2>/dev/null && pwd)

# ==============================================================================
# Excludes
# ==============================================================================

EXCLUDES=(
  .git node_modules vendor __pycache__ .venv .Trash .DS_Store
  dist build .next .bun worktrees .worktrees
  playwright-report test-results
)

GLOBAL_EXCLUDES=(
  Library Applications Movies Music Pictures
  System Volumes cores private opt
)

FD_EXCLUDES=()
for item in "${EXCLUDES[@]}"; do FD_EXCLUDES+=(--exclude "$item"); done

FD_GLOBAL_EXCLUDES=("${FD_EXCLUDES[@]}")
for item in "${GLOBAL_EXCLUDES[@]}"; do FD_GLOBAL_EXCLUDES+=(--exclude "$item"); done

WORKSPACE_DIRS=("$HOME_DIR/Developer" "$HOME_DIR/dotfiles" "$HOME_DIR/conductor")

EXCLUDE_RE='\.git|node_modules|vendor|__pycache__|\.venv|\.Trash|\.DS_Store|dist|build|\.next|\.bun|worktrees|\.worktrees|playwright-report|test-results'
GLOBAL_EXCLUDE_RE="$EXCLUDE_RE|Library|Applications|Movies|Music|Pictures|System|Volumes|cores|private|opt"

# ==============================================================================
# Helpers
# ==============================================================================

strip_trailing_slash() { sed 's|/$||'; }

dedupe() { awk '!seen[$0]++'; }

finalize() {
  awk -v lim="${1:-15}" '{ sub(/\/$/, ""); if (!seen[$0]++) { print; if (++n >= lim) exit } }'
}

expand_sparse() {
  local base_dir="$1"
  local -a lines=()
  while IFS= read -r line; do
    [[ -n "$line" ]] && lines+=("$line")
  done
  if [[ ${#lines[@]} -gt 0 && ${#lines[@]} -lt 5 ]]; then
    for path in "${lines[@]}"; do
      echo "$path"
      local full
      [[ "$path" == /* ]] && full="$path" || full="${base_dir%/}/${path}"
      full="${full%/}"
      if [[ -d "$full" ]]; then
        fd --hidden --max-depth 1 "${FD_EXCLUDES[@]}" "" "$full/" 2>/dev/null | while IFS= read -r child; do
          echo "${child#${base_dir%/}/}"
        done
      fi
    done
  else
    printf '%s\n' "${lines[@]}"
  fi
}

resolve_workspace_segment() {
  local segment="$1"
  for ws in "${WORKSPACE_DIRS[@]}"; do
    [[ -d "${ws}/${segment}" ]] && echo "${ws}/${segment}" && return 0
  done
  for ws in "${WORKSPACE_DIRS[@]}"; do
    [[ "$(basename "$ws")" == "$segment" && -d "$ws" ]] && echo "$ws" && return 0
  done
  # Fuzzy match: inline fd+fzf (avoids wait-in-subshell hang)
  local match
  for ws in "${WORKSPACE_DIRS[@]}"; do
    [[ -d "$ws" ]] || continue
    match=$(fd --max-depth 1 --type d "" "$ws" 2>/dev/null | fzf -i --filter="$segment" 2>/dev/null | head -1)
    if [[ -n "$match" ]]; then
      match="${match%/}"
      [[ -d "$match" ]] && echo "$match" && return 0
    fi
  done
  return 1
}

# ==============================================================================
# Data collection: git + transcripts (parallel, project-scoped)
# ==============================================================================

GIT_FILE="/tmp/fsg-git-$$"
TR_FILE="/tmp/fsg-tr-$$"

collect_git() {
  [[ -d "${ABS_PROJECT}/.git" ]] || return
  cd "$ABS_PROJECT" 2>/dev/null && {
    git diff --name-only 2>/dev/null
    git diff --name-only --cached 2>/dev/null
    git diff --name-only HEAD~20 HEAD 2>/dev/null
  } | sort -u
}

collect_transcripts() {
  local slug="${ABS_PROJECT//\//-}"
  local transcript_dir="$HOME_DIR/.claude/projects/${slug}"
  [[ -d "$transcript_dir" ]] || return
  find "$transcript_dir" -name '*.jsonl' -maxdepth 1 2>/dev/null \
    | xargs /bin/ls -t 2>/dev/null | head -5 | while read -r tf; do
    grep -o '"file_path":"[^"]*"' "$tf" 2>/dev/null
  done | sed 's/"file_path":"//;s/"//' | sort -u | while read -r fp; do
    [[ "$fp" == "${ABS_PROJECT}/"* ]] && echo "${fp#${ABS_PROJECT}/}"
  done
}


# ==============================================================================
# Boost scorer
# ==============================================================================

boost_and_rank() {
  local base_dir="$1"
  local prefix="$2"
  local limit="${3:-15}"

  wait $GIT_PID 2>/dev/null

  awk -v base="$base_dir" -v prefix="$prefix" -v git_file="$GIT_FILE" -v tr_file="$TR_FILE" '
  BEGIN {
    rank = 0
    while ((getline line < git_file) > 0) { if (line != "") git_mod[line] = 1 }
    close(git_file)
    while ((getline line < tr_file) > 0) { if (line != "") tr_recent[line] = 1 }
    close(tr_file)
  }
  {
    path = $0; rel = path; is_local = 0
    if (base != "" && base != ".") {
      before = rel
      sub("^" base, "", rel)
      if (rel != before) { gsub(/^\/+/, "", rel); is_local = 1 }
    }
    if (rel == "") next

    clean = rel; sub(/\/$/, "", clean)
    rank++; score = 10000 - rank
    if (is_local) {
      if (clean in git_mod) score += 500
      if (clean in tr_recent) score += 200
    }

    display = (is_local && prefix != "") ? prefix rel : rel
    printf "%d\t%s\n", score, display
  }' | sort -t$'\t' -k1,1rn | head -"$limit" | cut -f2-
}

# ==============================================================================
# Browse scorer — empty-pattern directory listing
# ==============================================================================

browse_rank() {
  local base_dir="$1"
  local prefix="$2"

  wait 2>/dev/null

  awk -v base="$base_dir" -v prefix="$prefix" -v git_file="$GIT_FILE" -v tr_file="$TR_FILE" '
  BEGIN {
    while ((getline line < git_file) > 0) { if (line != "") git_mod[line] = 1 }
    close(git_file)
    while ((getline line < tr_file) > 0) { if (line != "") tr_recent[line] = 1 }
    close(tr_file)
  }
  {
    path = $0; rel = path
    if (base != "" && base != ".") { sub("^" base, "", rel); gsub(/^\/+/, "", rel) }
    if (rel == "") next

    clean = rel; sub(/\/$/, "", clean)
    is_dir = (path ~ /\/$/) ? 1 : 0
    n = split(clean, parts, "/"); depth = n - 1
    score = 1000 - depth * 10
    if (is_dir) score += 200
    if (clean in git_mod) score += 100
    if (clean in tr_recent) score += 50

    display = (prefix != "") ? prefix rel : rel
    printf "%d\t%s\n", score, display
  }' | sort -t$'\t' -k1,1rn -k2,2 | head -15 | cut -f2-
}

# ==============================================================================
# Smart search — project first, workspace fills gaps
# ==============================================================================

smart_search() {
  local dir="$1"
  local pat="$2"
  local prefix="$3"

  [[ ! -d "$dir" ]] && return

  if [[ -z "$pat" ]]; then
    # Browse mode — ls -t for mtime sort (faster than fd+stat)
    /bin/ls -1tpA "$dir" 2>/dev/null \
      | grep -vE "^($EXCLUDE_RE)/?$" \
      | head -20 \
      | sed "s|^|${dir%/}/|" \
      | sed "s|^${ABS_PROJECT}/||" \
      | expand_sparse "$ABS_PROJECT" \
      | finalize
    return
  fi

  # Pattern search
  if [[ "$dir" != "${ABS_PROJECT}"* ]]; then
    # Workspace search: shallow fd + fzf only (no project-specific boosts)
    fd --hidden --max-depth 3 "${FD_EXCLUDES[@]}" "" "$dir" 2>/dev/null \
      | fzf -i --filter="$pat" 2>/dev/null \
      | finalize
  else
    # Project search: local + workspace fds in parallel through one fzf pass
    {
      fd --hidden "${FD_EXCLUDES[@]}" "" "$dir" 2>/dev/null &
      for ws in "${WORKSPACE_DIRS[@]}"; do
        [[ -d "$ws" ]] && fd --max-depth 2 "${FD_GLOBAL_EXCLUDES[@]}" "" "$ws" 2>/dev/null &
      done
      zoxide query -l 2>/dev/null | grep "^${HOME_DIR}/" | head -20 &
      wait
    } | fzf -i --filter="$pat" 2>/dev/null \
      | boost_and_rank "$dir" "$prefix" 15 \
      | finalize
  fi
}

# ==============================================================================
# Home navigation — explicit @~/ queries
# ==============================================================================

home_search() {
  local dir="$1"
  local pat="$2"

  local hidden_flag=""
  [[ "$pat" == .* ]] && hidden_flag="--hidden"

  if [[ -z "$pat" ]]; then
    local ls_flag="-1tp"
    [[ -n "$hidden_flag" ]] && ls_flag="-1tpA"
    /bin/ls $ls_flag "$dir" 2>/dev/null \
      | grep -vE "^($GLOBAL_EXCLUDE_RE)/?$" \
      | head -15 \
      | sed "s|^|${dir%/}/|" \
      | strip_trailing_slash
  else
    local ls_hidden="-1"
    [[ -n "$hidden_flag" ]] && ls_hidden="-1A"
    {
      /bin/ls $ls_hidden "$dir" 2>/dev/null | sed "s|^|${dir%/}/|"
      for ws in "${WORKSPACE_DIRS[@]}"; do
        [[ -d "$ws" ]] && fd $hidden_flag --max-depth 2 "${FD_GLOBAL_EXCLUDES[@]}" "" "$ws" 2>/dev/null &
      done
      wait
      zoxide query -l "$pat" 2>/dev/null | grep "^${HOME_DIR}/"
    } | fzf -i --filter="$pat" 2>/dev/null \
      | strip_trailing_slash | dedupe \
      | expand_sparse "$dir" \
      | finalize
  fi
}

# ==============================================================================
# Main
# ==============================================================================

# Start parallel data collection
collect_git > "$GIT_FILE" & GIT_PID=$!
collect_transcripts > "$TR_FILE" &

if [[ "$query" =~ ^[~\$] ]]; then
  # Explicit home navigation: @~/ or @$HOME/
  [[ "$query" == "~" ]] && query="~/"
  [[ "$query" =~ ^~[^/] ]] && query="~/${query#\~}"

  expanded="${query/#\~/$HOME_DIR}"
  expanded=$(eval echo "$expanded" 2>/dev/null || echo "$expanded")

  search_dir="" pattern=""
  if [[ "$expanded" == */ ]]; then
    search_dir="$expanded"
  elif [[ "$expanded" == */* ]]; then
    search_dir="$(dirname "$expanded")/"
    pattern="$(basename "$expanded")"
  else
    search_dir="$HOME_DIR/"
    pattern="$expanded"
  fi

  home_search "$search_dir" "$pattern"
else
  # Smart search: project first, workspace fills gaps
  cd "$ABS_PROJECT" 2>/dev/null || cd "$PROJECT_DIR" 2>/dev/null || exit 1

  search_dir="" pattern="" local_prefix=""
  if [[ "$query" == */ ]]; then
    search_dir="${ABS_PROJECT}/${query}"
    local_prefix="$query"
  elif [[ "$query" == */* ]]; then
    search_dir="${ABS_PROJECT}/$(dirname "$query")/"
    pattern="$(basename "$query")"
    local_prefix="$(dirname "$query")/"
  else
    search_dir="$ABS_PROJECT"
    pattern="$query"
  fi

  # Workspace fallback: resolve first path segment against workspace dirs
  if [[ ! -d "$search_dir" && "$query" == */* ]]; then
    first_seg="${query%%/*}"
    rest="${query#*/}"
    resolved=$(resolve_workspace_segment "$first_seg")
    if [[ -n "$resolved" ]]; then
      if [[ -z "$rest" || "$rest" == */ ]]; then
        search_dir="${resolved}/${rest}"
        local_prefix=""
      elif [[ "$rest" == */* ]]; then
        search_dir="${resolved}/$(dirname "$rest")/"
        pattern="$(basename "$rest")"
        local_prefix="${resolved}/$(dirname "$rest")/"
      else
        search_dir="${resolved}/"
        pattern="$rest"
        local_prefix="${resolved}/"
      fi
    fi
  fi

  smart_search "$search_dir" "$pattern" "$local_prefix"
fi

# Cleanup
rm -f "$GIT_FILE" "$TR_FILE" 2>/dev/null
