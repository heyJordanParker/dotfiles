#!/bin/bash
input=$(cat)
cwd=$(echo "$input" | jq -r '.workspace.current_dir')
model=$(echo "$input" | jq -r '.model.display_name')

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

compact_window=$(jq -r '.autoCompactWindow | numbers' "${CLAUDE_CONFIG_DIR:-$HOME/.claude}/settings.json" 2>/dev/null)
percentage=$(echo "$input" | jq -r --argjson setting "${compact_window:-null}" '
  .context_window as $c | (([$c.context_window_size, $setting] | map(numbers) | min) - 33000) as $threshold |
  ($c.current_usage // {}) | ((.input_tokens // 0) + (.cache_creation_input_tokens // 0) + (.cache_read_input_tokens // 0)) * 100 / $threshold | floor | [., 100] | min
')

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
  state_file="${CLAUDE_DATA_ROOT:-$HOME/.claude}/sessions/${session_id}/state.json"
  if [ -f "$state_file" ]; then
    raw_state=$(jq -r '.state // "propose"' "$state_file" 2>/dev/null) || raw_state="propose"
    raw_mode=$(jq -r '.mode // "build"' "$state_file" 2>/dev/null) || raw_mode="build"

    # A value off the axis reads as the default rather than as the other label: the
    # label below is a two-way pick, so an unrecognized state would print "Executing"
    # over a session that is proposing. Reachable only from a state.json written
    # before the mode axis, which sync.py migrates.
    case "$raw_state" in propose|execute) ;; *) raw_state=propose ;; esac
    case "$raw_mode" in orchestrate|build|interview) ;; *) raw_mode=build ;; esac

    # Capitalize first letter
    state_label=$([ "$raw_state" = propose ] && echo Proposing || echo Executing)
    mode_label=$([ "$raw_mode" = orchestrate ] && echo Orchestrating || ([ "$raw_mode" = build ] && echo Building || echo Interviewing))

    # State colors: proposing=yellow, executing=green, auto=cyan
    case "$raw_state" in
      propose) state_color="\033[33m" ;;
      execute) state_color="\033[32m" ;;
      *)         state_color="\033[90m" ;;
    esac

    # Approach colors: solo=magenta, subagents=blue, team=bright cyan
    case "$raw_mode" in
      orchestrate) mode_color="\033[34m" ;;
      build)       mode_color="\033[35m" ;;
      interview)   mode_color="\033[96m" ;;
      *)           mode_color="\033[90m" ;;
    esac

    reset=$'\033[0m'
    dim=$'\033[90m'
    classifier_status=$(printf "%b%s%s %b->%s %b%s%s " "$state_color" "$state_label" "$reset" "$dim" "$reset" "$mode_color" "$mode_label" "$reset")
  fi
fi

# Line 1: directory + git branch + model
printf "\033[97m%s\033[0m\033[35m%s\033[0m\033[34m%s\033[0m\n" "$short_dir" "$git_info" "$model_info"

# Line 2: classifier state + progress bar
printf "%b %s\n" "$classifier_status" "$progress_bar"

# Status segments are optional; their absence must not blank the whole line.
exit 0
