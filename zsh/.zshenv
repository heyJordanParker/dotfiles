# XDG Base Directory - ensures consistent config paths on macOS
export XDG_CONFIG_HOME="$HOME/.config"

# Default editor
export EDITOR="nvim"

# Safe delete - moves to Trash instead of permanent deletion
rm() {
  local args=()
  for arg in "$@"; do
    [[ "$arg" =~ ^-[rRfidv]+$ ]] && continue
    args+=("$arg")
  done
  trash "${args[@]}"
}
