# XDG Base Directory - ensures consistent config paths on macOS
export XDG_CONFIG_HOME="$HOME/.config"

# Default editor
export EDITOR="nvim"

# Developer directories
export SERVICES_DIR="$HOME/Developer/services"
export DEV_FOLDER="$HOME/Developer"

# Tool roots and global command path. Keep this file pure: no command
# substitution, no output, no interactive shell setup.
export ZSH="$HOME/.oh-my-zsh"
export BUN_INSTALL="$HOME/.bun"
export DEV_BROWSER="Helium"
export BROWSER="/Applications/Helium.app/Contents/MacOS/Helium"
export SSH_AUTH_SOCK="$HOME/Library/Group Containers/2BUA8C4S2C.com.1password/t/agent.sock"
export HOMEBREW_NO_ENV_HINTS=1
# agent-browser sessions leaked by agents self-clean fast instead of idling an hour
export AGENT_BROWSER_IDLE_TIMEOUT_MS=300000
# npm auth lives untracked so `npm login` never writes into the dotfiles tree
export NPM_CONFIG_USERCONFIG="$HOME/.config/npm/npmrc"
export NPM_CONFIG_FUND=false
export PATH="$HOME/.local/bin:$HOME/.claude/local:$HOME/bin:$BUN_INSTALL/bin:$HOME/.antigravity/antigravity/bin:$HOME/.lando/bin:/opt/homebrew/opt/ruby/bin:/opt/homebrew/bin:/opt/homebrew/sbin:/usr/local/bin:${PATH:-/usr/bin:/bin:/usr/sbin:/sbin}"

# Safe delete - moves to Trash instead of permanent deletion
rm() {
  local args=()
  for arg in "$@"; do
    [[ "$arg" =~ ^-[rRfidv]+$ ]] && continue
    args+=("$arg")
  done
  trash "${args[@]}"
}

# Machine-local secrets (untracked; repopulate from 1Password via setup-secrets.sh)
[[ -r "$HOME/.config/zsh/secrets.env" ]] && source "$HOME/.config/zsh/secrets.env"
