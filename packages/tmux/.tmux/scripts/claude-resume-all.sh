#!/bin/bash
# Resume Claude sessions after tmux-resurrect

MAPPING_DIR="$HOME/.claude/tmux-sessions"
[ ! -d "$MAPPING_DIR" ] && exit 0

sleep 1  # Wait for shells to init

resumed_sessions=""

# Use tab delimiter to handle spaces in session names
while IFS=$'\t' read -r stable_id pane_id cmd; do
    # Only resume in shell panes
    case "$cmd" in zsh|bash|fish|sh) ;; *) continue ;; esac

    mapping_file="$MAPPING_DIR/${stable_id//:/--}"
    [ ! -f "$mapping_file" ] && continue

    session_id=$(cat "$mapping_file")
    [ -z "$session_id" ] && continue

    # Skip if already resumed (prevents duplicate from pane renumbering)
    if [[ "$resumed_sessions" == *"$session_id"* ]]; then
        rm -f "$mapping_file"
        continue
    fi
    resumed_sessions="$resumed_sessions $session_id"

    # Resume session (Claude handles invalid IDs gracefully)
    tmux send-keys -t "$pane_id" "cld --resume '$session_id'" Enter

    # Clean up mapping after sending (one-shot)
    rm -f "$mapping_file"
done < <(tmux list-panes -a -F $'#{session_name}:#{window_index}:#{pane_index}\t#{pane_id}\t#{pane_current_command}')
