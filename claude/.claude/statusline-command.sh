#!/bin/bash
input=$(cat)
cwd=$(echo "$input" | jq -r '.workspace.current_dir')
model=$(echo "$input" | jq -r '.model.display_name')
style=$(echo "$input" | jq -r '.output_style.name')

# Shorten home directory to ~
short_dir="${cwd/#$HOME/~}"

git_info=""
git_file_status=""

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

    # File status (for line 2)
    status=$(git -C "$cwd" --no-optional-locks status --porcelain 2>/dev/null)
    if [ -n "$status" ]; then
      added=$(echo "$status" | grep -c -E '^A|^\?\?')
      modified=$(echo "$status" | grep -c -E '^.?M')
      deleted=$(echo "$status" | grep -c -E '^.?D')
      dim=$'\033[90m'
      reset=$'\033[0m'
      [ "$added" -gt 0 ] && git_file_status="${git_file_status}${dim}󰐕${added}${reset} "
      [ "$modified" -gt 0 ] && git_file_status="${git_file_status}${dim}󰏫${modified}${reset} "
      [ "$deleted" -gt 0 ] && git_file_status="${git_file_status}${dim}󰍴${deleted}${reset} "
    fi
  fi
fi

model_info=""
[ "$model" != "Claude 3.5 Sonnet" ] && [ "$model" != "null" ] && model_info="󰧑 $model"

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

# Context usage progress bar
total_input=$(echo "$input" | jq -r '.context_window.total_input_tokens // 0')
total_output=$(echo "$input" | jq -r '.context_window.total_output_tokens // 0')
context_size=$(echo "$input" | jq -r '.context_window.context_window_size // 200000')
total_tokens=$((total_input + total_output))
percentage=$((total_tokens * 100 / context_size))
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

# Line 2: model + style + duration + git file status + progress bar
printf "\033[34m%s\033[0m\033[32m%s\033[0m \033[90m%s\033[0m %b%s\n" "$model_info" "$style_info" "$duration" "$git_file_status" "$progress_bar"
