#!/bin/bash

TARGET_PANE=$1

# Capture scrollback from the target pane to a temporary file
tmux capture-pane -t "$TARGET_PANE" -p -S -32768 > /tmp/tmux-scrollback

# Open in nvim (read-only, jump to end, no sidebar)
nvim -R -c "set norelativenumber nonumber signcolumn=no clipboard=unnamedplus" -c "norm G" /tmp/tmux-scrollback
