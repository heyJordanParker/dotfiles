# --- SYSTEM LIMITS ---
ulimit -n 65536

# --- ZINIT (Plugin Manager) ---
# Load BEFORE OMZ so completions are in fpath when OMZ runs compinit
source /opt/homebrew/opt/zinit/zinit.zsh
zinit light zsh-users/zsh-completions
zinit light zsh-users/zsh-autosuggestions
zinit light Aloxaf/fzf-tab

# --- OH-MY-ZSH ---
ZSH_THEME="robbyrussell"
plugins=(git)

source $ZSH/oh-my-zsh.sh
unsetopt share_history

# Completion styles
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
# Starship
eval "$(starship init zsh)"

# FZF
eval "$(fzf --zsh)"

# Zoxide (smart cd) — overrides `cd` with directory-frecency jumping.
# Lives in .zshrc (not .zprofile) because zellij spawns non-login shells.
eval "$(zoxide init --cmd cd zsh)"
if [[ -n "$CLAUDE_CODE_ENTRYPOINT" ]]; then
    cd() { __zoxide_z "$@" 2>/dev/null; }
fi

# Atuin (history)
eval "$(atuin init zsh --disable-up-arrow)"
_zsh_autosuggest_strategy_atuin() {
    suggestion=$(ATUIN_QUERY="$1" atuin search --cmd-only --limit 1 --search-mode prefix 2>/dev/null)
}
ZSH_AUTOSUGGEST_STRATEGY=(atuin history completion)

# --- ALIASES ---
alias ls='eza -1l --icons=always --hyperlink --group-directories-first'
alias copy='pbcopy'
alias reload='source ~/.zshrc'
alias vim='nvim'
alias nvim-lazy='NVIM_APPNAME=nvim-lazy nvim'
alias nvim-astro='NVIM_APPNAME=nvim-astro nvim'
alias nvim-chad='NVIM_APPNAME=nvim-chad nvim'
alias cld="USER_TYPE=ant CLAUDE_CODE_NO_FLICKER=1 CLAUDE_CODE_ENABLE_TASKS=true EDITOR=prompt-editor claude --dangerously-skip-permissions --teammate-mode in-process --agent cto"
alias cld45="cld --model claude-opus-4-5"
alias cldcopy="USER_TYPE=ant CLAUDE_CODE_NO_FLICKER=1 CLAUDE_CODE_ENABLE_TASKS=true EDITOR=prompt-editor CLAUDE_CONFIG_DIR=$HOME/.claude/profiles/copywriter claude --dangerously-skip-permissions --teammate-mode in-process --mcp-config $HOME/.claude/profiles/copywriter/.mcp.json --strict-mcp-config --agent copy-chief"
alias cldexp="USER_TYPE=ant CLAUDE_CODE_NO_FLICKER=1 CLAUDE_CODE_ENABLE_TASKS=true EDITOR=prompt-editor CLAUDE_CONFIG_DIR=$HOME/.claude/profiles/experimental claude --dangerously-skip-permissions --teammate-mode in-process --mcp-config $HOME/.claude/profiles/experimental/.mcp.json"
alias python='python3'
alias pip='pip3'
f() { find . -iname "*$1*" }
bun() { case "$1" in test|build|deploy|publish|login) local cmd="$1"; shift; command bun run "$cmd" "$@";; *) command bun "$@";; esac }

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

    # 3. Open the Popup
    if [ -n "$ZELLIJ" ]; then
        "$HOME/.local/bin/zellij-toggle-term" nvim-scratch "$HOME/.local/bin/tmux-nvim" 0
    elif [ -n "$TMUX" ]; then
        tmux display-popup -d '#{pane_current_path}' -xC -yC -w 80% -h 80% \
            -E "$HOME/.local/bin/tmux-nvim"
    fi
}

# Load local secrets last
[[ -f ~/.zshrc.local ]] && source ~/.zshrc.local

# Completions (bun, npm, etc.)
source ~/.zsh_completions.zsh

# Waiting Indicator Hooks
precmd() {
  if [ -n "$TMUX" ]; then
    is_active=$(tmux display-message -t "$TMUX_PANE" -p '#{window_active}' 2>/dev/null)
    if [ "$is_active" = "0" ]; then
       touch "/tmp/zsh-waiting-${TMUX_PANE}"
    fi
  fi
  if [ -n "$ZELLIJ" ]; then
    # Drive the status bar directly via pipe messages to muxline.
    # This is faster than zellij's internal CwdChanged polling (~1s
    # interval), so `cd` reflects in the top bar in one prompt-tick.
    # - cwd: publish $PWD so the plugin's formatter shows basename / ~.
    # - cmd: reset to "zsh" now that the prompt is back.
    # - completed: clear any attention indicator set while busy.
    zellij pipe --name "muxline::cwd::$ZELLIJ_PANE_ID" --payload "$PWD" >/dev/null 2>&1
    zellij pipe --name "muxline::cmd::$ZELLIJ_PANE_ID" --payload "zsh" >/dev/null 2>&1
    zellij pipe --name "muxline::completed::$ZELLIJ_PANE_ID" >/dev/null 2>&1
  fi
}

preexec() {
  if [ -n "$TMUX" ]; then
    /bin/rm -f "/tmp/zsh-waiting-${TMUX_PANE}"
  fi
  if [ -n "$ZELLIJ" ]; then
    # $1 is the command line as typed. Pipe its first token as the
    # active command (matches tmux's `pane_current_command`).
    local cmd_name="${1%% *}"
    [ -n "$cmd_name" ] && zellij pipe --name "muxline::cmd::$ZELLIJ_PANE_ID" --payload "$cmd_name" >/dev/null 2>&1
  fi
}

# Zellij: Claude session auto-resume after resurrection
__zellij_claude_resume() {
    add-zsh-hook -d precmd __zellij_claude_resume
    [ -z "$ZELLIJ" ] && return
    local mf="$HOME/.claude/zellij-sessions/${ZELLIJ_SESSION_NAME}--${ZELLIJ_PANE_ID}"
    [ ! -f "$mf" ] && return
    local sid=$(cat "$mf"); rm -f "$mf"
    [ -z "$sid" ] && return
    (sleep 0.5 && zellij action paste --pane-id "$ZELLIJ_PANE_ID" "cld --resume '$sid'" && zellij action send-keys --pane-id "$ZELLIJ_PANE_ID" "Enter") &!
}
add-zsh-hook precmd __zellij_claude_resume

claude-mem() { bun "$HOME/.claude/plugins/marketplaces/thedotmack/plugin/scripts/worker-service.cjs" "$@"; }
