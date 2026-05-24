# SYNAPSE_ Shell Integration — Fish
# Source this in your ~/.config/fish/config.fish:
#   test -f ~/.config/SYNAPSE_/shell/synapse-integration.fish && source ~/.config/SYNAPSE_/shell/synapse-integration.fish

if not set -q SYNAPSE_INSIDE
    exit 0
end

function __synapse_osc
    printf '\e]%s\a' $argv[1]
end

function __synapse_preexec --on-event fish_preexec
    __synapse_osc '133;A'
end

function __synapse_postexec --on-event fish_postexec
    set -l ret $status
    __synapse_osc "133;D;$ret"
    __synapse_osc '133;C'
end

function __synapse_cwd --on-variable PWD
    __synapse_osc "7;file://"(hostname)"$PWD"
end

__synapse_cwd
