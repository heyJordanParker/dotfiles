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

# brew bundle passes no formula names, so a non-official tap it reads is untrusted
echo "==> Trusting third-party taps..."
brew trust --formula rjyo/moshi/moshi-hook zippoxer/tap/recall tobi/try/try \
  tw93/tap/mole oven-sh/bun/bun modem-dev/tap/hunk roots/tap/trellis-cli \
  felixkratz/formulae/borders cristianoliveira/tap/aerospace-scratchpad

echo "==> Installing brew packages..."
brew bundle --file="$DOTFILES_DIR/Brewfile"

if [ ! -x "$HOME/.local/bin/claude" ]; then
  echo "==> Installing Claude Code..."
  curl -fsSL https://claude.ai/install.sh | bash
fi

if [ ! -x "$HOME/.local/bin/herdr" ]; then
  echo "==> Installing herdr..."
  curl -fsSL https://herdr.dev/install.sh | sh
fi

echo "==> Installing bun global packages..."
while read -r pkg; do
  [ -n "$pkg" ] && bun add -g "$pkg"
done < "$DOTFILES_DIR/packages/bun/globals"
bun pm -g trust --all
agent-browser install

echo "==> Installing the Rust toolchain tracer builds with..."
# tracer builds through rustup's stable cargo, and its releases cross-compile the
# Linux files with cargo-zigbuild (zig comes from the Brewfile).
export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
if [ ! -x "$HOME/.cargo/bin/rustup" ] && ! command -v rustc &>/dev/null; then
    rustup-init -y --no-modify-path --default-toolchain stable >/dev/null 2>&1
fi
rustup target add x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu >/dev/null
cargo install cargo-zigbuild --locked >/dev/null

echo "==> Linking the tracer mod (code intelligence)..."
npm install -g @cmodjs/cli
if [ ! -d "$HOME/Developer/tracer" ]; then
  git clone https://github.com/heyJordanParker/tracer.git "$HOME/Developer/tracer"
fi
cmod link "$HOME/Developer/tracer" --yes

echo "==> Setting up moshi (mobile terminal for agent sessions)..."
# `moshi-hook service install` is systemd-only; macOS goes through brew services
brew services start moshi-hook
moshi-hook host enable-ssh

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
# Lando resolves docker from the bundle first and PATH second; without this
# file a thin PATH (launchd, cron, env -i) reads as "no engine" and lando
# auto-runs `lando setup`, which installs Docker Desktop over the stub.
mkdir -p /Applications/Docker.app/Contents/Resources/bin
printf '#!/bin/sh\nexec /Applications/OrbStack.app/Contents/MacOS/xbin/docker "$@"\n' > /Applications/Docker.app/Contents/Resources/bin/docker
chmod +x /Applications/Docker.app/Contents/Resources/bin/docker

echo "==> Linking dotfiles & syncing generated files..."
# All repo maintenance — the stow package->target mapping, restow, and the
# generated codex files — lives in scripts/sync.py so setup.sh and the
# pre-commit hook share one implementation. setup.sh stays the only bash file.
if ! command -v python3 >/dev/null 2>&1; then
  echo "ERROR: python3 is required for repo maintenance (scripts/sync.py)." >&2
  exit 1
fi
# codex reads its system config from /etc/codex, the one stow target outside $HOME
sudo install -d -o "$USER" -g staff /etc/codex
python3 "$DOTFILES_DIR/scripts/sync.py"
git -C "$DOTFILES_DIR" config core.hooksPath scripts/git-hooks
cd "$DOTFILES_DIR"

# Build bat theme cache
bat cache --build 2>/dev/null || true

echo ""
echo "Done. Restart terminal, enable 1Password SSH agent, then run:"
echo "  gh auth login"
echo "  cf auth login"
echo "  ./setup-secrets.sh"
