# XDG Base Directory - ensures consistent config paths on macOS
export XDG_CONFIG_HOME="$HOME/.config"

# Default editor
export EDITOR="nvim"

# Developer directories
export SERVICES_DIR="$HOME/Developer/services"

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
