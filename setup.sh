#!/bin/bash
set -e

REPO="heyJordanParker/dotfiles"
DOTFILES_DIR="$HOME/dotfiles"

if ! xcode-select -p &>/dev/null; then
  echo "==> Installing Xcode CLI tools..."
  xcode-select --install
  echo "Press enter when done..."
  read -r
fi

if ! command -v brew &>/dev/null; then
  echo "==> Installing Homebrew..."
  /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
  eval "$(/opt/homebrew/bin/brew shellenv)"
fi

if [ ! -d "$DOTFILES_DIR" ]; then
  echo "==> Cloning dotfiles..."
  git clone "https://github.com/$REPO.git" "$DOTFILES_DIR"
fi

SERVICES_DIR="$HOME/Developer/services"

echo "==> Creating Developer directories..."
mkdir -p "$HOME/Developer/references" "$SERVICES_DIR"

echo "==> Installing brew packages..."
brew bundle --file="$DOTFILES_DIR/Brewfile"

echo "==> Installing bun global packages..."
while read -r pkg; do
  [ -n "$pkg" ] && bun add -g "$pkg"
done < "$DOTFILES_DIR/bun/.config/bun/globals"
bun pm -g trust --all
agent-browser install

echo "==> Setting up services..."
if [ ! -d "$SERVICES_DIR/drawbridge" ]; then
  git clone https://github.com/heyJordanParker/drawbridge.git "$SERVICES_DIR/drawbridge"
fi
(cd "$SERVICES_DIR/drawbridge" && npm install && npm run build)
npx playwright install chromium
# Lando 3 hardcodes /Applications/Docker.app — create a stub app bundle that
# satisfies detection without symlinking to OrbStack (symlinks trigger macOS
# LaunchServices to activate OrbStack's window on any path access)
[ -L /Applications/Docker.app ] && rm /Applications/Docker.app
if [ ! -d /Applications/Docker.app ]; then
  mkdir -p /Applications/Docker.app/Contents/MacOS
  cat > /Applications/Docker.app/Contents/Info.plist << 'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key>
  <string>Docker</string>
  <key>CFBundleIdentifier</key>
  <string>com.docker.stub</string>
  <key>CFBundleShortVersionString</key>
  <string>4.36.0</string>
</dict>
</plist>
PLIST
  printf '#!/bin/sh\nopen -a OrbStack\n' > /Applications/Docker.app/Contents/MacOS/Docker
  chmod +x /Applications/Docker.app/Contents/MacOS/Docker
fi

echo "==> Linking dotfiles..."
cd "$DOTFILES_DIR"
stow -v zsh git tmux npm ssh nvim ghostty karabiner btop claude lazygit delta bat opencode atuin bun zellij

# Pre-grant zellij plugin permissions so zjstatus/autolock render without prompting
ZELLIJ_CACHE="$HOME/Library/Caches/org.Zellij-Contributors.Zellij"
mkdir -p "$ZELLIJ_CACHE"
cp "$DOTFILES_DIR/zellij/.config/zellij/permissions.kdl" "$ZELLIJ_CACHE/permissions.kdl"

# Build and install custom muxline zellij plugin
echo "==> Building muxline zellij plugin..."
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
if [ ! -x "$HOME/.cargo/bin/rustup" ]; then
    rustup-init -y --no-modify-path --default-toolchain stable >/dev/null 2>&1
fi
rustup target add wasm32-wasip1 >/dev/null 2>&1
mkdir -p "$HOME/.config/zellij/plugins"
(cd "$DOTFILES_DIR/zellij/plugins/muxline" && cargo build --target wasm32-wasip1 --release >/dev/null)
cp "$DOTFILES_DIR/zellij/plugins/muxline/target/wasm32-wasip1/release/muxline.wasm" "$HOME/.config/zellij/plugins/"

# Build bat theme cache
bat cache --build 2>/dev/null || true

echo ""
echo "Done. Restart terminal, enable 1Password SSH agent, then run:"
echo "  gh auth login"
echo "  ./setup-secrets.sh"
