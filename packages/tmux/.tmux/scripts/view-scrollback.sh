#!/bin/bash

TARGET_PANE=$1

# Capture from MAIN tmux server (default socket)
tmux -L default capture-pane -t "$TARGET_PANE" -p -S -32768 > /tmp/tmux-scrollback

nvim -R -c "set norelativenumber nonumber signcolumn=no clipboard=unnamedplus" -c "norm G" /tmp/tmux-scrollback
