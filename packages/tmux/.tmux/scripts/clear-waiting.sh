#!/bin/bash

window_id=$1

# Get list of pane IDs for the window
panes=$(tmux list-panes -t "$window_id" -F "#{pane_id}")

while read -r pane_id; do
  # Remove Claude waiting marker
  /bin/rm -f "/tmp/claude-waiting-${pane_id}"
  # Remove Zsh waiting marker
  /bin/rm -f "/tmp/zsh-waiting-${pane_id}"
done <<< "$panes"
