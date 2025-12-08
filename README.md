# dotfiles

macOS dotfiles managed with [GNU Stow](https://www.gnu.org/software/stow/).

## New Machine Setup

```bash
curl -fsSL https://raw.githubusercontent.com/heyJordanParker/dotfiles/master/setup.sh | bash
```

This installs Xcode CLI tools, Homebrew, all packages from `Brewfile`, bun, and symlinks configs.

## Manual Usage

```bash
# Add a new config
cd ~/dotfiles
mkdir -p newapp/.config/newapp
mv ~/.config/newapp/config newapp/.config/newapp/
stow newapp

# Remove symlinks
stow -D newapp

# Re-link after changes
stow -R newapp
```

## Structure

Each top-level directory is a stow package. Files inside mirror their target location relative to `$HOME`.
