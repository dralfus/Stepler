# Stepler SSH/readline adapter for remote Bash sessions.
#
# Usage on the remote Linux host:
#   source /path/to/Stepler.SSHReadline.bash
#   export STEPLER_REMOTE_BIN=/path/to/stepler-remote  # optional
#
# Windows Stepler sends private readline key sequences over the SSH terminal:
#   ESC [ 777 ; 1 u  -> Pause mode, convert word before/around cursor
#   ESC [ 777 ; 2 u  -> Ctrl+Pause mode, convert phrase/line around cursor

__stepler_readline_apply() {
    local mode="$1"
    local bin="${STEPLER_REMOTE_BIN:-stepler-remote}"
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

bind -x '"\e[777;1u": __stepler_readline_pause'
bind -x '"\e[777;2u": __stepler_readline_scrolllock'
