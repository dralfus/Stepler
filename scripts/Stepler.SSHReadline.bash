# Stepler SSH/readline adapter for remote Bash sessions.
#
# Usage on the remote Linux host:
#   source /path/to/Stepler.SSHReadline.bash
#
# Windows Stepler sends private readline key sequences over the SSH terminal:
#   ESC [ 777 ; 1 u  -> Pause mode, convert word before/around cursor
#   ESC [ 777 ; 2 u  -> Ctrl+Pause mode, convert current command line

__stepler_readline_apply() {
    local mode="$1"
    local result encoded point

    if ! command -v python3 >/dev/null 2>&1; then
        return 0
    fi

    result=$(
        STEPLER_MODE="$mode" \
        STEPLER_LINE="$READLINE_LINE" \
        STEPLER_POINT="$READLINE_POINT" \
        python3 - <<'PY'
import base64
import os
import re

mode = os.environ.get("STEPLER_MODE", "pause")
line = os.environ.get("STEPLER_LINE", "")
try:
    point = int(os.environ.get("STEPLER_POINT", str(len(line))))
except ValueError:
    point = len(line)

point = max(0, min(point, len(line)))

en = "`qwertyuiop[]asdfghjkl;'zxcvbnm,./~QWERTYUIOP{}ASDFGHJKL:\"ZXCVBNM<>?"
ru = "ёйцукенгшщзхъфывапролджэячсмитьбю.ЁЙЦУКЕНГШЩЗХЪФЫВАПРОЛДЖЭЯЧСМИТЬБЮ,"
en_to_ru = dict(zip(en, ru))
ru_to_en = dict(zip(ru, en))

def is_ru(ch):
    return ch in ru_to_en

def is_en(ch):
    return ch in en_to_ru and (ch.isascii() and (ch.isalpha() or ch in "`[];',./{}:\"<>?~"))

def convert_token(token):
    ru_count = sum(1 for ch in token if ch in ru_to_en)
    en_count = sum(1 for ch in token if ch in en_to_ru)
    if ru_count > en_count:
        return "".join(ru_to_en.get(ch, ch) for ch in token)
    return "".join(en_to_ru.get(ch, ch) for ch in token)

def convert_line(text):
    parts = re.split(r"(\s+)", text)
    return "".join(part if part.isspace() else convert_token(part) for part in parts)

def word_range(text, cursor):
    if not text:
        return (0, 0)
    cursor = max(0, min(cursor, len(text)))
    end = cursor
    while end > 0 and text[end - 1].isspace():
        end -= 1
    if end == 0 and cursor < len(text) and not text[cursor].isspace():
        end = cursor
    if end < len(text) and not text[end].isspace():
        while end < len(text) and not text[end].isspace():
            end += 1
    start = end
    while start > 0 and not text[start - 1].isspace():
        start -= 1
    return (start, end)

if mode == "scrolllock":
    new_line = convert_line(line)
    new_point = min(len(new_line), point)
else:
    start, end = word_range(line, point)
    if start == end:
        new_line = line
        new_point = point
    else:
        replacement = convert_token(line[start:end])
        new_line = line[:start] + replacement + line[end:]
        new_point = start + len(replacement)

encoded = base64.b64encode(new_line.encode("utf-8")).decode("ascii")
print(f"{encoded}\t{new_point}")
PY
    ) || return 0

    encoded="${result%%$'\t'*}"
    point="${result#*$'\t'}"
    if [[ -z "$encoded" || "$point" == "$result" ]]; then
        return 0
    fi

    READLINE_LINE="$(printf '%s' "$encoded" | base64 --decode 2>/dev/null)"
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

