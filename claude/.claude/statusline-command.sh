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

mode_display=""
session_file=$(ls -t /tmp/claude-session-* 2>/dev/null | head -1)
if [ -n "$session_file" ]; then
  mode=$(grep "^MODE=" "$session_file" 2>/dev/null | cut -d= -f2-)
  approach=$(grep "^APPROACH=" "$session_file" 2>/dev/null | cut -d= -f2-)
  # Mode in yellow, approach in cyan, separated by space
  [ -n "$mode" ] && mode_display=$(printf "\033[33m%s\033[0m" "$mode")
  [ -n "$approach" ] && mode_display=$(printf "%s \033[36m%s\033[0m" "$mode_display" "$approach")
  [ -n "$mode_display" ] && mode_display="$mode_display "
fi

model_info=""
[ "$model" != "null" ] && model_info="󰧑 $model"

style_info=""
[ "$style" != "default" ] && [ "$style" != "null" ] && style_info=" [$style]"

# Extract usage metrics
duration_ms=$(echo "$input" | jq -r '.cost.total_duration_ms // 0')

# Format duration
if [ "$duration_ms" -gt 0 ]; then
  duration_s=$((duration_ms / 1000))
  if [ $duration_s -lt 60 ]; then
    duration="<1m"
  elif [ $duration_s -lt 3600 ]; then
    minutes=$((duration_s / 60))
    duration="${minutes}m"
  else
    hours=$((duration_s / 3600))
    minutes=$(((duration_s % 3600) / 60))
    duration="${hours}h${minutes}m"
  fi
else
  duration="<1m"
fi

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

# Line 1: directory + git branch
printf "\033[97m%s\033[0m\033[35m%s\033[0m\n" "$short_dir" "$git_info"

# Line 2: mode + approach + model + style + duration + progress bar
printf "%b\033[34m%s\033[0m\033[32m%s\033[0m \033[90m%s\033[0m %s\n" "$mode_display" "$model_info" "$style_info" "$duration" "$progress_bar"
