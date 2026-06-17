# Stepler SSH/readline adapter for remote Bash sessions.
#
# Usage on the remote Linux host:
#   source /path/to/Stepler.SSHReadline.bash
#   export STEPLER_REMOTE_BIN=/path/to/stepler-remote  # optional
#
# If stepler-remote is available, this script marks the terminal title so the
# Windows client knows this SSH session can safely receive Stepler sequences.
#
# Windows Stepler sends private readline key sequences over the SSH terminal:
#   ESC [ 777 ; 1 u  -> Pause mode, convert word before/around cursor
#   ESC [ 777 ; 2 u  -> Ctrl+Pause mode, convert phrase/line around cursor

__STEPLER_REMOTE_TITLE_MARKER="stepler-remote-ready"

__stepler_remote_bin() {
    if [[ -n "${STEPLER_REMOTE_BIN:-}" ]]; then
        printf '%s' "$STEPLER_REMOTE_BIN"
    elif [[ -x "$HOME/.local/bin/stepler-remote" ]]; then
        printf '%s' "$HOME/.local/bin/stepler-remote"
    else
        printf '%s' "stepler-remote"
    fi
}

__stepler_remote_available() {
    command -v "$(__stepler_remote_bin)" >/dev/null 2>&1
}

__stepler_readline_set_title() {
    if ! __stepler_remote_available; then
        return 0
    fi

    local dir="${PWD/#$HOME/~}"
    printf '\033]0;%s %s@%s:%s\007' \
        "$__STEPLER_REMOTE_TITLE_MARKER" \
        "${USER:-user}" \
        "${HOSTNAME:-host}" \
        "$dir"
}

__stepler_readline_install_prompt_marker() {
    case "${PS1:-}" in
        *"$__STEPLER_REMOTE_TITLE_MARKER"*) ;;
        *)
            PS1="${PS1:-}"'\[\e]0;stepler-remote-ready \u@\h: \w\a\]'
            ;;
    esac

    case ";${PROMPT_COMMAND:-};" in
        *";__stepler_readline_set_title;"*) ;;
        *)
            if [[ -n "${PROMPT_COMMAND:-}" ]]; then
                PROMPT_COMMAND="$PROMPT_COMMAND; __stepler_readline_set_title"
            else
                PROMPT_COMMAND="__stepler_readline_set_title"
            fi
            ;;
    esac
    __stepler_readline_set_title
}

__stepler_readline_apply() {
    local mode="$1"
    local bin="$(__stepler_remote_bin)"
    local result status rest point encoded decoded

    if ! command -v "$bin" >/dev/null 2>&1; then
        return 0
    fi

    result=$(
        printf '%s' "$READLINE_LINE" \
            | "$bin" bash --mode "$mode" --point "$READLINE_POINT" --point-units chars
    ) || return 0

    status="${result%%$'\t'*}"
    rest="${result#*$'\t'}"
    point="${rest%%$'\t'*}"
    encoded="${rest#*$'\t'}"
    if [[ "$status" != "ok" || -z "$encoded" || "$point" == "$rest" ]]; then
        return 0
    fi

    decoded="$(printf '%s' "$encoded" | base64 --decode 2>/dev/null)" || return 0
    READLINE_LINE="$decoded"
    READLINE_POINT="$point"
}

__stepler_readline_pause() {
    __stepler_readline_apply pause
}

__stepler_readline_scrolllock() {
    __stepler_readline_apply scrolllock
}

if __stepler_remote_available; then
    bind -x '"\e[777;1u": __stepler_readline_pause'
    bind -x '"\e[777;2u": __stepler_readline_scrolllock'
    __stepler_readline_install_prompt_marker
fi
