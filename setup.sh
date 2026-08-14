#!/bin/bash
set -e

REPO="heyJordanParker/dotfiles"

# Prefer the script's own directory; fall back to $HOME/dotfiles only when
# piped (no on-disk $0). Reruns from any clone location resolve to that clone.
if [ -f "${BASH_SOURCE[0]}" ] && [ -d "$(dirname "${BASH_SOURCE[0]}")/packages" ]; then
  DOTFILES_DIR="$(cd -P "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
else
  DOTFILES_DIR="$HOME/dotfiles"
fi

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
done < "$DOTFILES_DIR/packages/bun/globals"
bun pm -g trust --all
agent-browser install

echo "==> Installing tracer (code intelligence CLI)..."
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
if [ ! -x "$HOME/.cargo/bin/rustup" ] && ! command -v rustc &>/dev/null; then
    rustup-init -y --no-modify-path --default-toolchain stable >/dev/null 2>&1
fi
(cd "$DOTFILES_DIR/tools/tracer" && cargo build --release)
mkdir -p "$HOME/.local/bin"
install -m 755 "$DOTFILES_DIR/tools/tracer/target/release/trace" "$HOME/.local/bin/trace"
echo "==> Regenerating tracer plugin crate mirror..."
(cd "$DOTFILES_DIR/tools/tracer" && cargo xtask sync-dist)

echo "==> Installing prompt-reviewer (local prompt-review CLI)..."
# Compiles llama.cpp in-process (needs cmake, from the Brewfile). The model
# is downloaded on demand by `review-prompt download`, never here.
(cd "$DOTFILES_DIR/tools/prompt-reviewer" && cargo build --release)
install -m 755 "$DOTFILES_DIR/tools/prompt-reviewer/target/release/review-prompt" "$HOME/.local/bin/review-prompt"

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
fi
# Rewritten every run, outside the guard, because the bundle outlives the script.
# orbctl starts the engine directly; `open -a` hands OrbStack to LaunchServices,
# which cold-launches it in the foreground and takes focus from the frontmost app.
printf '#!/bin/sh\nexec /Applications/OrbStack.app/Contents/MacOS/bin/orbctl start\n' > /Applications/Docker.app/Contents/MacOS/Docker
chmod +x /Applications/Docker.app/Contents/MacOS/Docker

echo "==> Linking dotfiles & syncing generated files..."
# All repo maintenance — the stow package->target mapping, restow, and the
# generated codex files — lives in scripts/sync.py so setup.sh and the
# pre-commit hook share one implementation. setup.sh stays the only bash file.
if ! command -v python3 >/dev/null 2>&1; then
  echo "ERROR: python3 is required for repo maintenance (scripts/sync.py)." >&2
  exit 1
fi
python3 "$DOTFILES_DIR/scripts/sync.py"
git -C "$DOTFILES_DIR" config core.hooksPath scripts/git-hooks
cd "$DOTFILES_DIR"

# Pre-grant zellij plugin permissions so zjstatus/autolock render without prompting
ZELLIJ_CACHE="$HOME/Library/Caches/org.Zellij-Contributors.Zellij"
mkdir -p "$ZELLIJ_CACHE"
cp "$DOTFILES_DIR/packages/zellij/permissions.kdl" "$ZELLIJ_CACHE/permissions.kdl"

# Build and install custom muxline zellij plugin
echo "==> Building muxline zellij plugin..."
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
if [ ! -x "$HOME/.cargo/bin/rustup" ]; then
    rustup-init -y --no-modify-path --default-toolchain stable >/dev/null 2>&1
fi
rustup target add wasm32-wasip1 >/dev/null 2>&1
mkdir -p "$HOME/.config/zellij/plugins"
(cd "$DOTFILES_DIR/packages/zellij/build/muxline" && cargo build --target wasm32-wasip1 --release >/dev/null)
cp "$DOTFILES_DIR/packages/zellij/build/muxline/target/wasm32-wasip1/release/muxline.wasm" "$HOME/.config/zellij/plugins/"

# Build bat theme cache
bat cache --build 2>/dev/null || true

echo ""
echo "Done. Restart terminal, enable 1Password SSH agent, then run:"
echo "  gh auth login"
echo "  ./setup-secrets.sh"
