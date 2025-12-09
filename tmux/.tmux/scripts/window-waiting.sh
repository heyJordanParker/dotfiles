#!/bin/bash

window_id=$1

# Get list of pane IDs and commands for the window
panes=$(tmux list-panes -t "$window_id" -F "#{pane_id} #{pane_current_command}")

while read -r pane_id pane_command; do
  # Check if Claude is waiting
  if [ -f "/tmp/claude-waiting-${pane_id}" ]; then
    echo "*"
    exit 0
  fi

  # Check if zsh is waiting
  if [[ "$pane_command" == "zsh" ]]; then
    echo "*"
    exit 0
  fi
done <<< "$panes"

echo ""
