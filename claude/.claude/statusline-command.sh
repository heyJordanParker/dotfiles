#!/bin/bash
input=$(cat)
cwd=$(echo "$input" | jq -r '.workspace.current_dir')
model=$(echo "$input" | jq -r '.model.display_name')
style=$(echo "$input" | jq -r '.output_style.name')

# Shorten home directory to ~
short_dir="${cwd/#$HOME/~}"

git_info=""

if git -C "$cwd" rev-parse --git-dir >/dev/null 2>&1; then
  branch=$(git -C "$cwd" --no-optional-locks branch --show-current 2>/dev/null || git -C "$cwd" --no-optional-locks rev-parse --short HEAD 2>/dev/null)
  if [ -n "$branch" ]; then
    git_info=" 󰘬 $branch"

    # Branch ahead/behind status
    upstream=$(git -C "$cwd" --no-optional-locks rev-parse --abbrev-ref '@{upstream}' 2>/dev/null)
    if [ -n "$upstream" ]; then
      ahead=$(git -C "$cwd" --no-optional-locks rev-list --count '@{upstream}..HEAD' 2>/dev/null)
      behind=$(git -C "$cwd" --no-optional-locks rev-list --count 'HEAD..@{upstream}' 2>/dev/null)
      branch_status=""
      [ "$ahead" -gt 0 ] 2>/dev/null && branch_status="↑${ahead}"
      [ "$behind" -gt 0 ] 2>/dev/null && branch_status="${branch_status}${branch_status:+ }↓${behind}"
      [ -n "$branch_status" ] && git_info="${git_info} [${branch_status}]"
    fi

  fi
fi

model_info=""
if [ "$model" != "null" ]; then
  clean_model="${model%% (*}"
  model_info=" 󰧑 $clean_model"
fi

style_info=""
[ "$style" != "default" ] && [ "$style" != "null" ] && style_info=" [$style]"

# Context usage progress bar (approximates /context output)
context_size=$(echo "$input" | jq -r '.context_window.context_window_size // 200000')
autocompact_buffer=45000  # fixed reservation
system_overhead=15000     # partial - rest is in cache_read when cached
current_tokens=$(echo "$input" | jq -r --argjson overhead $((autocompact_buffer + system_overhead)) '
  .context_window.current_usage |
  if . then (.input_tokens // 0) + (.output_tokens // 0) + (.cache_creation_input_tokens // 0) + (.cache_read_input_tokens // 0) + $overhead else 0 end
')
percentage=$((current_tokens * 100 / context_size))
[ "$percentage" -gt 100 ] && percentage=100

# Create progress bar (10 chars wide)
bar_width=10
filled=$((percentage * bar_width / 100))
empty=$((bar_width - filled))
bar=""
for ((i=0; i<filled; i++)); do bar="${bar}━"; done
for ((i=0; i<empty; i++)); do bar="${bar}┄"; done

progress_bar=$(printf "\033[90m%s %d%%\033[0m" "$bar" "$percentage")

# Intent classifier state
session_id=$(echo "$input" | jq -r '.session_id // ""')
classifier_status=""
if [ -n "$session_id" ]; then
  state_file="/tmp/claude-session-state-${session_id}"
  [ ! -f "$state_file" ] && /Users/jordan/.claude/hooks/initialize-session-state.sh "$session_id"
  if [ -f "$state_file" ]; then
    raw_state=$(jq -r '.state // "proposing"' "$state_file" 2>/dev/null) || raw_state="proposing"
    raw_approach=$(jq -r '.approach // "solo"' "$state_file" 2>/dev/null) || raw_approach="solo"

    # Capitalize first letter
    state_label="$(tr '[:lower:]' '[:upper:]' <<< "${raw_state:0:1}")${raw_state:1}"
    approach_label="$(tr '[:lower:]' '[:upper:]' <<< "${raw_approach:0:1}")${raw_approach:1}"

    # State colors: proposing=yellow, executing=green, auto=cyan
    case "$raw_state" in
      proposing) state_color="\033[33m" ;;
      executing) state_color="\033[32m" ;;
      auto)      state_color="\033[36m" ;;
      *)         state_color="\033[90m" ;;
    esac

    # Approach colors: solo=magenta, subagents=blue, team=bright cyan
    case "$raw_approach" in
      solo)      approach_color="\033[35m" ;;
      subagents) approach_color="\033[34m" ;;
      team)      approach_color="\033[96m" ;;
      *)         approach_color="\033[90m" ;;
    esac

    reset=$'\033[0m'
    dim=$'\033[90m'
    classifier_status=$(printf "%b%s%s %b->%s %b%s%s " "$state_color" "$state_label" "$reset" "$dim" "$reset" "$approach_color" "$approach_label" "$reset")
  fi
fi

# Line 1: directory + git branch + model
printf "\033[97m%s\033[0m\033[35m%s\033[0m\033[34m%s\033[0m\n" "$short_dir" "$git_info" "$model_info"

# Line 2: classifier state + style + progress bar
printf "%b\033[32m%s\033[0m %s\n" "$classifier_status" "$style_info" "$progress_bar"
