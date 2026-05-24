# SYNAPSE_ Shell Integration — Zsh
# Source this in your ~/.zshrc:
#   [ -f ~/.config/SYNAPSE_/shell/synapse-integration.zsh ] && source ~/.config/SYNAPSE_/shell/synapse-integration.zsh

if [[ -z "$SYNAPSE_INSIDE" ]]; then
    return
fi

__synapse_osc() {
    printf '\e]%s\a' "$1"
}

# Track command timing for long-command notifications
__synapse_recent_cmd=""
__synapse_cmd_start=0

__synapse_preexec() {
    __synapse_osc '133;A'
    __synapse_recent_cmd="$1"
    __synapse_cmd_start=$SECONDS
}

__synapse_precmd() {
    local ret=$?
    local elapsed=$(( SECONDS - __synapse_cmd_start ))
    __synapse_osc "133;D;$ret"
    __synapse_osc '133;C'

    if [[ $elapsed -ge 30 ]]; then
        __synapse_osc "777;notify;Command finished;${__synapse_recent_cmd} completed after ${elapsed}s"
    fi
}

__synapse_cwd() {
    __synapse_osc "7;file://$(hostname)$(pwd)"
}

autoload -Uz add-zsh-hook
add-zsh-hook preexec __synapse_preexec
add-zsh-hook precmd __synapse_precmd
add-zsh-hook chpwd __synapse_cwd
__synapse_cwd
