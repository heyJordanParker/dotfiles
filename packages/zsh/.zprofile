eval "$(/opt/homebrew/bin/brew shellenv)"

# Prefer Homebrew Ruby over system Ruby (more modern)
export PATH="/opt/homebrew/opt/ruby/bin:$PATH"
# Reassert user command precedence after brew shellenv/path_helper so login
# non-interactive agent shells still resolve our tools before macOS binaries.
export PATH="$HOME/.local/bin:$HOME/.claude/local:$HOME/bin:$BUN_INSTALL/bin:$HOME/.antigravity/antigravity/bin:$HOME/.lando/bin:/opt/homebrew/opt/ruby/bin:/opt/homebrew/bin:/opt/homebrew/sbin:/usr/local/bin:${PATH:-/usr/bin:/bin:/usr/sbin:/sbin}"

# Added by OrbStack: command-line tools and integration
# This won't be added again if you remove it.
source ~/.orbstack/shell/init.zsh 2>/dev/null || :
