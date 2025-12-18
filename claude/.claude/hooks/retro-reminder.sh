#!/bin/bash
# Reminds user to run /retro if it's been 3+ days

marker="$HOME/.claude/.retro-marker"

# First run - no marker exists
if [ ! -f "$marker" ]; then
  echo "Tip: Run /retro to analyze conversation patterns"
  exit 0
fi

# Check if marker is older than 3 days
if [ "$(find "$marker" -mtime +3 2>/dev/null)" ]; then
  echo "It's been 3+ days since last /retro"
fi

exit 0
