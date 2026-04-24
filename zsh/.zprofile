eval "$(/opt/homebrew/bin/brew shellenv)"
export HOMEBREW_NO_ENV_HINTS=1

# Prefer Homebrew Ruby over system Ruby (more modern)
export PATH="/opt/homebrew/opt/ruby/bin:$PATH"

# zoxide init lives in .zshrc (not here) because .zprofile is sourced only for
# LOGIN shells; zellij spawns non-login interactive shells, so .zshrc is the
# right place for interactive-command setup like zoxide's `cd` override.

# Start persistent nvim server if not running (async)
(
  NVIM_SOCKET="$HOME/.cache/nvim/server.pipe"
  if ! timeout 1 nvim --server "$NVIM_SOCKET" --remote-expr "1" &>/dev/null; then
    mkdir -p ~/.cache/nvim
    nohup nvim --headless --listen "$NVIM_SOCKET" &>/dev/null &
  fi
) &!

# Added by OrbStack: command-line tools and integration
# This won't be added again if you remove it.
source ~/.orbstack/shell/init.zsh 2>/dev/null || :
