eval "$(/opt/homebrew/bin/brew shellenv)"

# Initialize zoxide for all shell types
eval "$(zoxide init --cmd cd zsh)"

# Suppress zoxide noise in Claude Code sessions
if [[ -n "$CLAUDE_CODE_ENTRYPOINT" ]]; then
    cd() { __zoxide_z "$@" 2>/dev/null; }
fi

# Start persistent nvim server if not running (async)
(
  NVIM_SOCKET="$HOME/.cache/nvim/server.pipe"
  if ! timeout 1 nvim --server "$NVIM_SOCKET" --remote-expr "1" &>/dev/null; then
    mkdir -p ~/.cache/nvim
    nohup nvim --headless --listen "$NVIM_SOCKET" &>/dev/null &
  fi
) &!
