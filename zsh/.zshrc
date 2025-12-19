# --- PATH CONFIGURATION ---
export ZSH="$HOME/.oh-my-zsh"
export PATH="$HOME/.local/bin:$HOME/bin:/usr/local/bin:$PATH"
export PATH="/Users/jordan/.claude/local:$PATH"
export DEV_FOLDER="$HOME/Developer"
export DEV_BROWSER="Helium"
export BROWSER="/Applications/Helium.app/Contents/MacOS/Helium"

# --- OH-MY-ZSH & PLUGINS ---
ZSH_THEME="robbyrussell"
plugins=(git)

source $ZSH/oh-my-zsh.sh
unsetopt share_history

# --- ZINIT (Plugin Manager) ---
source /opt/homebrew/opt/zinit/zinit.zsh

# Load plugins (Light mode for speed)
zinit light zsh-users/zsh-completions
zinit light zsh-users/zsh-autosuggestions
zinit light Aloxaf/fzf-tab

# Initialize Completions (ONCE, at the end of plugins)
autoload -Uz compinit && compinit
zstyle ':completion:*' matcher-list 'm:{a-z}={A-Za-z}'
zstyle ':completion:*' list-colors "${(s.:.)LS_COLORS}"
zstyle ':completion:*' menu no
zstyle ':fzf-tab:complete:cd:*' fzf-preview 'ls --color $realpath'
zstyle ':fzf-tab:complete:__zoxide_z:*' fzf-preview 'ls --color $realpath'

# --- KEY BINDINGS & UI ---
bindkey '^?' backward-delete-char
bindkey '^[^?' backward-kill-word
bindkey "\e[1;3D" backward-word
bindkey "\e[1;3C" forward-word
bindkey "^[[1;9D" beginning-of-line
bindkey "^[[1;9C" end-of-line
echo -ne '\e[6 q' # Cursor shape

# --- TOOLS & INTEGRATIONS ---
# 1Password
export SSH_AUTH_SOCK="$HOME/Library/Group Containers/2BUA8C4S2C.com.1password/t/agent.sock"

# Starship
eval "$(starship init zsh)"

# FZF
eval "$(fzf --zsh)"

# Atuin (history)
eval "$(atuin init zsh)"
_zsh_autosuggest_strategy_atuin() {
    suggestion=$(ATUIN_QUERY="$1" atuin search --cmd-only --limit 1 --search-mode prefix 2>/dev/null)
}
ZSH_AUTOSUGGEST_STRATEGY=(atuin history completion)

# Bun
export BUN_INSTALL="$HOME/.bun"
export PATH="$BUN_INSTALL/bin:$PATH"

# Antigravity & Lando
export PATH="/Users/jordan/.antigravity/antigravity/bin:$PATH"
export PATH="/Users/jordan/.lando/bin:$PATH"

# Try experiment manager
eval "$(try init ~/Developer/experiments)"

# --- ALIASES ---
alias ls='eza -1l --icons=always --hyperlink --group-directories-first'
alias copy='pbcopy'
alias reload='source ~/.zshrc'
alias vim='nvim'
alias cld="EDITOR=prompt-editor claude --dangerously-skip-permissions"
alias python='python3'
alias pip='pip3'
f() { find . -iname "*$1*" }
bun() { [[ "$1" == "test" ]] && shift && command bun run test "$@" || command bun "$@" }

# --- THE FAST VIM POPUP ---

v() {
    local socket="$HOME/.cache/nvim/server.pipe"
    local cmd=""

    # 1. Build the command string
    if [ $# -eq 0 ]; then
        # No args: Just open
        :
    elif [ "$1" = "." ]; then
        # "v ." -> Change dir to current, open file explorer
        cmd="<C-\><C-n>:cd $(pwd)<CR>:e .<CR>"
    else
        # "v file" -> Open files
        cmd="<C-\><C-n>"
        for file in "$@"; do
            # ${file:A} is Zsh native absolute path (Instant)
            cmd+=":drop ${file:A}<CR>"
        done
    fi

    # 2. Fire command to socket (Async/Background)
    # We do this BEFORE opening tmux so it's ready when the window appears
    if [ -n "$cmd" ]; then
        # Only try to send if socket exists, otherwise script handles boot
        if [ -S "$socket" ]; then
            { nvim --server "$socket" --remote-send "$cmd" >/dev/null 2>&1 } &!
        fi
    fi

    # 3. Open the Popup (Executes the dumb script)
    tmux display-popup -d '#{pane_current_path}' -xC -yC -w 80% -h 80% \
        -E "$HOME/.local/bin/tmux-nvim"
}

# Load local secrets last
[[ -f ~/.zshrc.local ]] && source ~/.zshrc.local

# Completions (bun, npm, etc.)
source ~/.zsh_completions.zsh

# Tmux Waiting Indicator Hooks
precmd() {
  if [ -n "$TMUX" ]; then
    # Check if window is active (1 = active, 0 = inactive)
    # Explicitly target the current pane to be safe
    is_active=$(tmux display-message -t "$TMUX_PANE" -p '#{window_active}' 2>/dev/null)
    if [ "$is_active" = "0" ]; then
       touch "/tmp/zsh-waiting-${TMUX_PANE}"
    fi
  fi
}

preexec() {
  if [ -n "$TMUX" ]; then
    /bin/rm -f "/tmp/zsh-waiting-${TMUX_PANE}"
  fi
}
