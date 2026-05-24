# SYNAPSE_ Shell Integration — Bash
# Source this in your ~/.bashrc:
#   [ -f ~/.config/SYNAPSE_/shell/synapse-integration.bash ] && source ~/.config/SYNAPSE_/shell/synapse-integration.bash

if [[ -z "$SYNAPSE_INSIDE" ]]; then
    return
fi

__synapse_osc() {
    printf '\e]%s\a' "$1"
}

__synapse_update_cwd() {
    __synapse_osc "7;file://$(hostname)$(pwd)"
}

# Track command timing
__synapse_recent_cmd=""
__synapse_cmd_start=0

__synapse_preexec_invoked=0
__synapse_preexec() {
    __synapse_osc '133;A'
    if [[ "$__synapse_preexec_invoked" == "0" ]]; then
        __synapse_preexec_invoked=1
        # Extract the command from BASH_COMMAND for timing
        __synapse_recent_cmd="$BASH_COMMAND"
        __synapse_cmd_start=$SECONDS
    fi
}

__synapse_prompt_command() {
    local ret=$?
    __synapse_preexec_invoked=0
    local elapsed=$(( SECONDS - __synapse_cmd_start ))
    __synapse_osc "133;D;$ret"
    __synapse_osc '133;C'
    __synapse_update_cwd

    if [[ $elapsed -ge 30 ]]; then
        __synapse_osc "777;notify;Command finished;${__synapse_recent_cmd} completed after ${elapsed}s"
    fi
}

trap '__synapse_preexec' DEBUG
PROMPT_COMMAND=__synapse_prompt_command
__synapse_update_cwd
