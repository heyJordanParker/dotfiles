eval "$(/opt/homebrew/bin/brew shellenv)"

# Initialize zoxide for all shell types
eval "$(zoxide init --cmd cd zsh)"

# Start persistent nvim server if not running
NVIM_SOCKET="$HOME/.cache/nvim/server.pipe"
if ! nvim --server "$NVIM_SOCKET" --remote-expr "1" &>/dev/null; then
  mkdir -p ~/.cache/nvim
  nohup nvim --headless --listen "$NVIM_SOCKET" &>/dev/null &!
fi
