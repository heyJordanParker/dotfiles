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

echo "==> Installing brew packages..."
brew bundle --file="$DOTFILES_DIR/Brewfile"

if [ ! -d "$HOME/.bun" ]; then
  echo "==> Installing bun..."
  curl -fsSL https://bun.sh/install | bash
fi

echo "==> Linking dotfiles..."
cd "$DOTFILES_DIR"
stow -v zsh git tmux npm ssh nvim ghostty karabiner btop claude lazygit delta bat

# Build bat theme cache
bat cache --build 2>/dev/null || true

echo ""
echo "Done. Restart terminal, enable 1Password SSH agent, run: gh auth login"
