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
codex_status=""
if [ -n "$session_id" ]; then
  state_file="${CLAUDE_DATA_ROOT:-$HOME/.claude}/sessions/${session_id}/state.json"
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
      interview) state_color="\033[97m" ;;
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

  columns=${COLUMNS:-}
  [[ "$columns" =~ ^[1-9][0-9]*$ ]] || columns=""
  session_dir="${CLAUDE_DATA_ROOT:-$HOME/.claude}/sessions/${session_id}"
  now=$(date +%s)
  shopt -s nullglob
  records=("$session_dir"/codex-run-*.json)
  if [ "${#records[@]}" -gt 0 ]; then
      while IFS= read -r -d '' agent &&
            IFS= read -r -d '' started_at &&
            IFS= read -r -d '' updated_at &&
            IFS= read -r -d '' activity &&
            IFS= read -r -d '' phase &&
            IFS= read -r -d '' fresh_input_tokens &&
            IFS= read -r -d '' pid; do
        is_dead=false
        if ! [[ "$pid" =~ ^[1-9][0-9]*$ ]] || ! kill -0 "$pid" 2>/dev/null; then
          is_dead=true
        fi
        if [ -n "$activity" ]; then
          activity=$(printf '%s' "$activity" | tr '\n' ' ' | sed 's/[[:space:]]\+/ /g;s/^ //;s/ $//')
          activity="${activity##*; }"
          while [[ "$activity" =~ ^[A-Za-z_][A-Za-z0-9_]*=[^[:space:]]+[[:space:]]+ ]]; do
            activity="${activity#"${BASH_REMATCH[0]}"}"
          done
          activity="${activity%% <<*}"
          activity="${activity% -}"
          activity="${activity%% | *}"
          activity="${activity%% >*}"
        fi
        [ -n "$activity" ] || activity="$phase"
        elapsed=$(( now - started_at ))
        [ "$elapsed" -lt 0 ] && elapsed=0
        if [ "$elapsed" -lt 60 ]; then
          elapsed_label="${elapsed}s"
        elif [ "$elapsed" -lt 3600 ]; then
          elapsed_label="$((elapsed / 60))m $((elapsed % 60))s"
        elif [ "$elapsed" -lt 86400 ]; then
          elapsed_label="$((elapsed / 3600))h $(((elapsed % 3600) / 60))m $((elapsed % 60))s"
        else
          elapsed_label="$((elapsed / 86400))d $(((elapsed % 86400) / 3600))h $(((elapsed % 3600) / 60))m"
        fi
        idle_for=$(( now - updated_at ))
        if [ "$idle_for" -ge 45 ]; then
          if [ "$idle_for" -lt 60 ]; then
            activity="idle ${idle_for}s"
          elif [ "$idle_for" -lt 3600 ]; then
            activity="idle $((idle_for / 60))m $((idle_for % 60))s"
          elif [ "$idle_for" -lt 86400 ]; then
            activity="idle $((idle_for / 3600))h $(((idle_for % 3600) / 60))m $((idle_for % 60))s"
          else
            activity="idle $((idle_for / 86400))d $(((idle_for % 86400) / 3600))h $(((idle_for % 3600) / 60))m"
          fi
        fi
        [ "$is_dead" = true ] && activity="failed — runner dead"
        trailer="$elapsed_label"
        if [[ "$fresh_input_tokens" =~ ^[1-9][0-9]*$ ]]; then
          if [ "$fresh_input_tokens" -ge 1000 ]; then token_label=$(awk -v tokens="$fresh_input_tokens" 'BEGIN {printf "%.1fk", tokens / 1000}'); else token_label="$fresh_input_tokens"; fi
          trailer+=" · ↓ ${token_label} tokens"
        fi
        glyph="○"
        row_color="37"
        if [ "$is_dead" = true ]; then
          glyph="✕"
          row_color="31"
        fi
        printf -v row "%s %s (codex)" "$glyph" "$agent"
        if [ -n "$columns" ]; then
          # Claude Code renders the statusline into a viewport four cells
          # narrower than the COLUMNS it exports, and ellipsizes the overflow.
          row_width=$((columns - 4))
          content_width=$row_width
          [ "$content_width" -lt 1 ] && continue
          row="${row:0:content_width}"
          if [ $(( ${#row} + ${#trailer} + 2 )) -gt "$content_width" ]; then
            trailer="$elapsed_label"
          fi
          if [ $(( ${#row} + ${#trailer} + 2 )) -gt "$content_width" ]; then
            trailer=""
          fi
          activity_width=$((content_width * 45 / 100))
          content_width=$((content_width - ${#row} - ${#trailer} - 2))
          [ -n "$trailer" ] && content_width=$((content_width - 2))
          [ "$activity_width" -gt "$content_width" ] && activity_width="$content_width"
          if [ "$activity_width" -gt 0 ]; then
            if [ "${#activity}" -gt "$activity_width" ]; then
              activity="${activity:0:activity_width}"
              activity="${activity% *}"
              [ -n "$activity" ] && activity+="…"
            fi
            [ -n "$activity" ] && row+="  $activity"
          fi
          padding=$((row_width - ${#row} - ${#trailer}))
          [ "$padding" -gt 0 ] && printf -v padding "%*s" "$padding" "" || padding=""
          row+="${padding}${trailer}"
          row=$'\033['"${row_color}"'m'"${row}"$'\033[0m'
        else
          [ -n "$activity" ] && row+="  $activity"
          row=$'\033['"${row_color}"'m'"${row}  ${trailer}"$'\033[0m'
        fi
        codex_status+="$row"$'\n'
      done < <(jq --raw-output0 --arg session_id "$session_id" '
        select(.session == $session_id and .status == "running") |
        (.agent // "unknown"), (.started_at // 0), (.updated_at // 0),
        (.activity // ""), (.phase // ""), (.fresh_input_tokens // ""), (.pid // "")
      ' "${records[@]}" 2>/dev/null)
  fi
fi

# Line 1: directory + git branch + model
printf "\033[97m%s\033[0m\033[35m%s\033[0m\033[34m%s\033[0m\n" "$short_dir" "$git_info" "$model_info"

# Line 2: classifier state + style + progress bar
printf "%b\033[32m%s\033[0m %s\n" "$classifier_status" "$style_info" "$progress_bar"

# Running codex jobs
if [ -n "$codex_status" ]; then
  printf "%s" "$codex_status"
fi

# Status segments are optional; their absence must not blank the whole line.
exit 0
