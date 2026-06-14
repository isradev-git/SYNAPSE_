# SYNAPSE_ Shell Environment — Zsh
# Cyberpunk prompt, plugin activation, completions, history, sysinfo.
# Sourced from ~/.zshrc via ZDOTDIR shim (auto) or `synapse_ --setup` (manual).

# Guard: prevent double-loading
[[ -n "$_SYNAPSE_ENV_LOADED" ]] && return
_SYNAPSE_ENV_LOADED=1

# ── Helper: try-source a list of paths ───────────────────────────────
_syn_try_source() {
    local f
    for f in "$@"; do
        [[ -f "$f" ]] && { source "$f" && return 0; }
    done
    return 1
}

# ── zsh options ───────────────────────────────────────────────────────
setopt AUTO_CD
setopt CORRECT
setopt NO_CASE_GLOB
setopt GLOB_DOTS
setopt EXTENDED_HISTORY
setopt SHARE_HISTORY
setopt HIST_IGNORE_DUPS
setopt HIST_IGNORE_SPACE
setopt HIST_REDUCE_BLANKS
setopt INTERACTIVE_COMMENTS

# ── History ───────────────────────────────────────────────────────────
HISTSIZE=100000
SAVEHIST=100000
HISTFILE="${HISTFILE:-$HOME/.zsh_history}"

# ── Completions ───────────────────────────────────────────────────────
autoload -Uz compinit
_SYN_ZCDUMP="${ZSH_COMPDUMP:-$HOME/.zcompdump}"
if [[ -f "$_SYN_ZCDUMP" ]] && [[ -z "$(find "$_SYN_ZCDUMP" -mtime +0 2>/dev/null)" ]]; then
    compinit -C -d "$_SYN_ZCDUMP"
else
    compinit -d "$_SYN_ZCDUMP"
fi

zmodload zsh/complist
zstyle ':completion:*' menu select
zstyle ':completion:*' matcher-list 'm:{a-z}={A-Za-z}' 'r:|=*' 'l:|=* r:|=*'
zstyle ':completion:*:default' list-colors "${(s.:.)LS_COLORS}"
zstyle ':completion:*' group-name ''
zstyle ':completion:*:descriptions' format '%F{243}── %d ──%f'
zstyle ':completion:*:warnings'     format '%F{196} no match: %d%f'
zstyle ':completion:*' squeeze-slashes true
bindkey -M menuselect 'h' vi-backward-char
bindkey -M menuselect 'k' vi-up-line-or-history
bindkey -M menuselect 'j' vi-down-line-or-history
bindkey -M menuselect 'l' vi-forward-char

# ── Key bindings ──────────────────────────────────────────────────────
bindkey '^[[A'    up-line-or-search
bindkey '^[[B'    down-line-or-search
bindkey '^[[1;5C' forward-word
bindkey '^[[1;5D' backward-word
bindkey '^[f'     forward-word
bindkey '^[b'     backward-word
bindkey '^U'      backward-kill-line
bindkey '^K'      kill-line
bindkey '^[[3~'   delete-char

# ── zsh-autosuggestions ───────────────────────────────────────────────
if (( ! ${+ZSH_AUTOSUGGEST_HIGHLIGHT_STYLE} )); then
    _syn_try_source \
        /opt/homebrew/share/zsh-autosuggestions/zsh-autosuggestions.zsh \
        /usr/local/share/zsh-autosuggestions/zsh-autosuggestions.zsh \
        /usr/share/zsh-autosuggestions/zsh-autosuggestions.zsh \
        "${ZSH:-}/plugins/zsh-autosuggestions/zsh-autosuggestions.zsh" \
        "$HOME/.zsh/zsh-autosuggestions/zsh-autosuggestions.zsh"
fi
ZSH_AUTOSUGGEST_HIGHLIGHT_STYLE='fg=#3C4560'
ZSH_AUTOSUGGEST_STRATEGY=(history completion)
ZSH_AUTOSUGGEST_BUFFER_MAX_SIZE=80

# ── zsh-syntax-highlighting (deferred to first precmd) ───────────────
_syn_load_highlight() {
    if (( ! ${+ZSH_HIGHLIGHT_HIGHLIGHTERS} )); then
        _syn_try_source \
            /opt/homebrew/share/zsh-syntax-highlighting/zsh-syntax-highlighting.zsh \
            /usr/local/share/zsh-syntax-highlighting/zsh-syntax-highlighting.zsh \
            /usr/share/zsh-syntax-highlighting/zsh-syntax-highlighting.zsh \
            "${ZSH:-}/plugins/zsh-syntax-highlighting/zsh-syntax-highlighting.zsh" \
            "$HOME/.zsh/zsh-syntax-highlighting/zsh-syntax-highlighting.zsh"
    fi
    if (( ${+ZSH_HIGHLIGHT_STYLES} )); then
        ZSH_HIGHLIGHT_STYLES[command]='fg=#D0D8E4,bold'
        ZSH_HIGHLIGHT_STYLES[builtin]='fg=#D0D8E4,bold'
        ZSH_HIGHLIGHT_STYLES[alias]='fg=#D0D8E4,bold'
        ZSH_HIGHLIGHT_STYLES[unknown-token]='fg=#FF003C'
        ZSH_HIGHLIGHT_STYLES[single-quoted-argument]='fg=#77E66B'
        ZSH_HIGHLIGHT_STYLES[double-quoted-argument]='fg=#77E66B'
        ZSH_HIGHLIGHT_STYLES[path]='fg=#B0BBC8,underline'
        ZSH_HIGHLIGHT_STYLES[comment]='fg=#3C4560'
    fi
    add-zsh-hook -d precmd _syn_load_highlight
}
autoload -Uz add-zsh-hook
add-zsh-hook precmd _syn_load_highlight

# ── Cyberpunk prompt ──────────────────────────────────────────────────
# Active whenever SYNAPSE_PROMPT != 0 and starship is not running.
# Intentionally overrides oh-my-zsh themes — set SYNAPSE_PROMPT=0 to keep yours.
if [[ -z "$STARSHIP_SHELL" && "${SYNAPSE_PROMPT:-1}" != "0" ]]; then

    autoload -Uz vcs_info
    zstyle ':vcs_info:git:*' formats       ' ⎇  %b%u%c'
    zstyle ':vcs_info:git:*' actionformats ' ⎇  %b|%a%u%c'
    zstyle ':vcs_info:*' check-for-changes yes
    zstyle ':vcs_info:*' stagedstr         '%F{green}●%f'
    zstyle ':vcs_info:*' unstagedstr       '%F{yellow}±%f'
    zstyle ':vcs_info:*' enable git

    _SYN_RED=$'\e[38;2;255;0;60m'
    _SYN_FG=$'\e[38;2;176;187;200m'
    _SYN_DIM=$'\e[38;2;60;69;96m'
    _SYN_MID=$'\e[38;2;208;216;228m'
    _SYN_RST=$'\e[0m'
    _SYN_BLD=$'\e[1m'

    _syn_precmd_vcs() { vcs_info; }
    add-zsh-hook precmd _syn_precmd_vcs

    _syn_short_path() {
        local p="${PWD/#$HOME/~}"
        local -a parts=("${(@s:/:)p}")
        if (( ${#parts} > 4 )); then
            echo "${parts[1]}/…/${parts[-2]}/${parts[-1]}"
        else
            echo "$p"
        fi
    }

    setopt PROMPT_SUBST

    # ╭─[SYN] user@host ❯ ~/path  ⎇ branch±
    # ╰─❯
    PROMPT='%{$_SYN_DIM%}╭─%{$_SYN_RED$_SYN_BLD%}[SYN]%{$_SYN_RST%} %{$_SYN_FG%}%n%{$_SYN_DIM%}@%{$_SYN_MID%}%m %{$_SYN_DIM%}❯%{$_SYN_RST%} %{$_SYN_MID%}$(_syn_short_path)%{$_SYN_DIM%}${vcs_info_msg_0_}
%{$_SYN_DIM%}╰─%{%(?.%{$_SYN_RED%}.%{$_SYN_FG%})%}❯%{$_SYN_RST%} '

    RPROMPT='%{$_SYN_DIM%}%T%{$_SYN_RST%}'
fi

# ── System info panel (shown once per SYNAPSE_ session) ──────────────
_syn_sysinfo() {
    local r=$'\e[38;2;255;0;60m'     # red accent
    local f=$'\e[38;2;176;187;200m'  # silver fg
    local d=$'\e[38;2;60;69;96m'     # dim blue-gray
    local m=$'\e[38;2;208;216;228m'  # mid white
    local g=$'\e[38;2;77;230;107m'   # green (ok)
    local y=$'\e[38;2;255;200;0m'    # amber (warn)
    local b=$'\e[1m'
    local rst=$'\e[0m'

    local os_name="" kernel="" cpu="" cores="" ram_used="" ram_total="" uptime_str=""

    if [[ "$OSTYPE" == darwin* ]]; then
        local prod ver mem_bytes vm_out pgsz pg_act pg_wrd
        prod=$(sw_vers -productName 2>/dev/null)
        ver=$(sw_vers -productVersion 2>/dev/null)
        os_name="${prod} ${ver}"
        kernel="Darwin $(uname -r) $(uname -m)"
        cpu=$(sysctl -n machdep.cpu.brand_string 2>/dev/null)
        [[ -z "$cpu" ]] && cpu=$(sysctl -n hw.model 2>/dev/null)
        cores=$(sysctl -n hw.logicalcpu 2>/dev/null)
        mem_bytes=$(sysctl -n hw.memsize 2>/dev/null)
        ram_total=$(( mem_bytes / 1024 / 1024 / 1024 ))
        vm_out=$(vm_stat 2>/dev/null)
        pgsz=$(pagesize 2>/dev/null || echo 16384)
        pg_act=$(awk '/Pages active/{gsub(/\./,"",$NF); print $NF+0}' <<< "$vm_out")
        pg_wrd=$(awk '/Pages wired/{gsub(/\./,"",$NF); print $NF+0}' <<< "$vm_out")
        ram_used=$(( (pg_act + pg_wrd) * pgsz / 1024 / 1024 / 1024 ))
        uptime_str=$(uptime 2>/dev/null | sed 's/.*up //' | sed 's/,.*//' | xargs)
    else
        [[ -f /etc/os-release ]] && os_name=$(. /etc/os-release && echo "$PRETTY_NAME")
        kernel="$(uname -r) $(uname -m)"
        cpu=$(awk -F': ' '/model name/{print $2; exit}' /proc/cpuinfo 2>/dev/null | xargs)
        [[ -z "$cpu" ]] && cpu=$(awk -F': ' '/Hardware/{print $2; exit}' /proc/cpuinfo 2>/dev/null | xargs)
        cores=$(nproc 2>/dev/null)
        if [[ -f /proc/meminfo ]]; then
            local mem_total_kb mem_avail_kb
            mem_total_kb=$(awk '/MemTotal/{print $2}' /proc/meminfo)
            mem_avail_kb=$(awk '/MemAvailable/{print $2}' /proc/meminfo)
            ram_total=$(( mem_total_kb / 1024 / 1024 ))
            ram_used=$(( (mem_total_kb - mem_avail_kb) / 1024 / 1024 ))
        fi
        uptime_str=$(uptime -p 2>/dev/null | sed 's/up //')
    fi

    local shell_extra=""
    [[ -n "$ZSH_THEME" ]] && shell_extra=" · oh-my-zsh (${ZSH_THEME})"
    [[ -n "$STARSHIP_SHELL" ]] && shell_extra=" · starship"
    local username="${USER:-$(whoami 2>/dev/null)}"
    local hostname_s; hostname_s=$(hostname -s 2>/dev/null || hostname)

    # RAM usage bar (20 blocks, color-coded by memory pressure)
    local ram_bar=""
    if [[ -n "$ram_total" && -n "$ram_used" && "$ram_total" -gt 0 ]]; then
        local filled=$(( ram_used * 20 / ram_total ))
        local empty=$(( 20 - filled ))
        local pct=$(( ram_used * 100 / ram_total ))
        local bar_col="$g"
        (( pct > 70 )) && bar_col="$y"
        (( pct > 85 )) && bar_col="$r"
        local i bar_f="" bar_e=""
        for (( i=0; i<filled; i++ )); do bar_f+="█"; done
        for (( i=0; i<empty;  i++ )); do bar_e+="░"; done
        ram_bar="${bar_col}${bar_f}${d}${bar_e}${rst}"
    fi

    local w=58
    local hline; printf -v hline '%*s' "$w" ''; hline="${hline// /═}"
    local tline; printf -v tline '%*s' "$w" ''; tline="${tline// /─}"

    printf '\n'
    printf '%s╔%s╗%s\n' "$r$b" "$hline" "$rst"
    local t1="  ⟦ SYNAPSE_ ⟧  NEURAL INTERFACE  ·  JACK IN  "
    local t2="  CHIBA CITY CONSTRUCT  ·  2049  ·  WINTERMUTE  "
    printf '%s║%s%s%s%*s%s║%s\n' \
        "$r$b" "$rst$m$b" "$t1" "$rst" $(( w - ${#t1} )) '' "$r$b" "$rst"
    printf '%s║%s%s%*s%s║%s\n' \
        "$r$b" "$d" "$t2" $(( w - ${#t2} )) '' "$r$b" "$rst"
    printf '%s╚%s╝%s\n' "$r$b" "$hline" "$rst"
    printf '\n'

    printf '%s// SYSTEM CORTEX %s%s\n' "$r" "${tline:17}" "$rst"
    printf "  ${d}%-12s${rst}  ${m}%s${rst}\n" "CONSTRUCT" "$os_name"
    printf "  ${d}%-12s${rst}  ${f}%s${rst}\n" "KERNEL" "$kernel"
    if [[ -n "$cores" ]]; then
        printf "  ${d}%-12s${rst}  ${m}%s  ${d}·  %s cores${rst}\n" "PROC" "$cpu" "$cores"
    else
        printf "  ${d}%-12s${rst}  ${m}%s${rst}\n" "PROC" "$cpu"
    fi
    if [[ -n "$ram_total" && -n "$ram_used" ]]; then
        printf "  ${d}%-12s${rst}  %s  ${r}%d GB${d} / ${m}%d GB${rst}\n" \
            "WETWARE" "$ram_bar" "$ram_used" "$ram_total"
    fi
    [[ -n "$uptime_str" ]] && \
        printf "  ${d}%-12s${rst}  ${d}%s${rst}\n" "UPLINK" "$uptime_str"
    printf '\n'

    printf '%s// SESSION NODE %s%s\n' "$r" "${tline:16}" "$rst"
    printf "  ${d}%-12s${rst}  ${m}%s${rst}\n" \
        "SHELL" "${SHELL##*/} ${ZSH_VERSION:-}${shell_extra}"
    printf "  ${d}%-12s${rst}  ${g}SYNAPSE_ v${SYNAPSE_VERSION:-0.2.0}${d}  ·  truecolor${rst}\n" \
        "TERMINAL"
    printf "  ${d}%-12s${rst}  ${m}%s${d}@${f}%s${rst}\n" \
        "OPERATOR" "$username" "$hostname_s"
    printf '\n'

    printf '%s%s%s\n' "$d" "$tline" "$rst"
    printf '  %sSECURE CHANNEL ESTABLISHED  ·  ICE NOMINAL  ·  %s%s\n\n' \
        "$d" "$(date '+%H:%M:%S')" "$rst"
}

if [[ -n "$SYNAPSE_INSIDE" && -t 1 && -z "$_SYNAPSE_GREETED" ]]; then
    _SYNAPSE_GREETED=1

    # Resolve fastfetch config path (written by SYNAPSE_ at startup)
    if [[ "$OSTYPE" == darwin* ]]; then
        _SYN_FF_CFG="$HOME/Library/Application Support/SYNAPSE_/fastfetch.jsonc"
    else
        _SYN_FF_CFG="${XDG_CONFIG_HOME:-$HOME/.config}/SYNAPSE_/fastfetch.jsonc"
    fi

    if command -v fastfetch &>/dev/null; then
        if [[ -f "$_SYN_FF_CFG" ]]; then
            fastfetch --config "$_SYN_FF_CFG"
        else
            fastfetch
        fi
    elif command -v neofetch &>/dev/null; then
        neofetch
    else
        _syn_sysinfo
    fi
    unset _SYN_FF_CFG
fi
