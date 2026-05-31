# Локальные команды разработки

## Rust workspace

После установки Rust toolchain:

```powershell
cargo fmt --all -- --check
cargo test --workspace
```

Если нужен быстрый прогон только core:

```powershell
cargo test -p stepler-core
```

## Ручной smoke без hotkey

После этапа Win32 Edit adapter можно проверить активный Notepad без global hotkey:

1. Открыть Notepad.
2. Набрать `k.,jdm`.
3. Поставить курсор в конец слова.
4. Из корня репозитория Stepler выполнить:

```powershell
$env:Path="$env:USERPROFILE\.cargo\bin;$env:Path"
cargo run -p stepler-cli -- pause --delay 3 --apply
```

Для `Ctrl+Pause`:

```powershell
cargo run -p stepler-cli -- scrolllock --delay 3 --apply
```

После запуска команды за 3 секунды нужно кликнуть обратно в Notepad. CLI использует текущий focused Win32 control, строит `ReplacementPlan`, сверяет `expected_before_text` перед заменой и применяет замену через Win32 edit messages без clipboard.

Без применения замены можно посмотреть, какой control читается:

```powershell
cargo run -p stepler-cli -- pause --delay 3
```

## Диагностика focused control

Перед разработкой app-specific adapter-а можно посмотреть Win32-классы foreground/focused окна:

```powershell
cargo run -p stepler-cli -- diagnose-focus --delay 3
```

После запуска команды за 3 секунды нужно кликнуть в нужное поле ввода, например Codex/Windsurf/Terminal. Если класс focused control не `Edit`/`RichEdit*` и не поддержанный terminal-host, текущий adapter безопасно откажется от обработки и запишет диагностический `UnsupportedControl`.

## Ручной smoke PowerShell/Windows Terminal

Для PowerShell основной безопасный путь - PSReadLine adapter, а не эмуляция terminal copy/paste. Скрипт `scripts\Stepler.PSReadLine.ps1` регистрирует обработчики `Pause` и `Ctrl+Pause` через `Set-PSReadLineKeyHandler`: текущая строка и cursor читаются через `PSConsoleReadLine.GetBufferState`, план строится в `stepler-cli psreadline-plan`, а результат применяется через `RevertLine` + `Insert` + `SetCursorPosition`. Clipboard и terminal selection не используются.

Сначала собрать CLI:

```powershell
cargo build -p stepler-cli
```

В обычном режиме tray автоматически добавляет загрузчик adapter-а в user profile PowerShell при запуске Stepler. Новые PowerShell-сессии должны иметь команду `Get-SteplerPsReadLineStatus` без ручного dot-source.

Для диагностики или старого уже открытого окна PowerShell adapter можно загрузить вручную:

```powershell
. .\scripts\Stepler.PSReadLine.ps1
```

Для автозагрузки можно добавить эту строку в `$PROFILE` после сборки `stepler-cli`.

Важно: PSReadLine может регистрировать только клавиши из `System.ConsoleKey`. `Pause` поддерживается, поэтому операция умного режима строки в PSReadLine adapter по умолчанию назначена на `Ctrl+Pause`. При необходимости можно выбрать другой chord; если передать неподдерживаемый chord, скрипт предупредит и вернется к безопасному fallback:

```powershell
. .\scripts\Stepler.PSReadLine.ps1 -ScrollLockChord F8
```

Проверка:

1. Ввести `пше`.
2. Нажать `Ctrl+Pause`.
3. Ожидаемый результат: строка стала `git`, clipboard не изменился, в prompt не появились `Сс`, `COPYME` или другие побочные символы.

Для `Pause` можно ввести `k.,jdm`, поставить caret в конец и нажать `Pause`; ожидаемый результат - `любовь`.

Быстрая проверка CLI-контракта без UI:

```powershell
$b=[Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes('пше'))
target\debug\stepler-cli.exe psreadline-plan --mode scrolllock --text-b64 $b --cursor 3
```

Ожидается JSON с `"replacement":"git"` и `"text_b64"`; PowerShell-скрипт применяет именно `text_b64`, чтобы не зависеть от кодировки stdout.

### Экспериментальный terminal fallback

Низкоуровневый terminal adapter через Windows Terminal (`CASCADIA_HOSTING_WINDOW_CLASS`) и классическую консоль (`ConsoleWindowClass`) оставлен только как diagnostic/fallback. Его нельзя считать основным PowerShell-путем, потому что Windows Terminal не обязан принимать synthetic `Ctrl+C`/`Ctrl+Shift+C` как copy action. В текущей method-resolver policy risky `TerminalClipboardShortcut` для Windows Terminal заблокирован по умолчанию.

Проверка runner для не-PowerShell окон:

```powershell
cargo run -p stepler-cli -- run-hotkeys
```

В другом поддержанном окне:

1. Скопировать любой контрольный текст в clipboard, например `COPYME`.
2. Ввести в prompt `ghbdtn vbh`, не нажимая Enter.
3. Нажать `Ctrl+Pause`.
4. Проверить, что строка стала `привет мир`, команда не прервалась, а `Ctrl+V` вставляет исходный clipboard.

Watched UI-тест старого terminal fallback для воспроизведения ровно в активном окне пользователя:

```powershell
cargo test -p stepler-cli --test terminal_smoke watched_active_terminal_scrolllock_repro -- --ignored --nocapture --test-threads=1
```

Тест даст countdown, после чего нужно сфокусировать тот же PowerShell/Windows Terminal, где вручную введено `пше`. Затем тест запустит `stepler-cli scrolllock --apply` против текущего foreground-окна, выведет `hwnd`, Win32 class/title и stdout/stderr команды, затем удержит паузу для визуальной проверки результата. Этот тест не проверяет low-level hotkey hook; он нужен, чтобы воспроизводить один и тот же terminal adapter сценарий на реальном окне.

## Ручной smoke с hotkey runner

Минимальный runner без tray UI:

```powershell
cargo run -p stepler-cli -- run-hotkeys
```

Он перехватывает `Pause` и `Ctrl+Pause` через low-level keyboard hook, блокирует исходное нажатие, вызывает тот же pipeline, что CLI-smoke, и пишет JSONL-лог:

```text
stepler_hotkey_log.jsonl
```

При запуске из tray путь лога задается через `STEPLER_HOTKEY_LOG_PATH` и по умолчанию находится здесь:

```text
%LOCALAPPDATA%\Stepler\logs\stepler_hotkey_log.jsonl
```

Остановить runner можно через `Ctrl+C` в его консоли.

В этом же runner включены клавиши переключения раскладки по ТЗ:

- `Left Ctrl` отдельно - переключить активное окно на русскую раскладку.
- `Right Ctrl` отдельно - переключить активное окно на английскую раскладку.
- `Menu` - переключить активное окно на следующую раскладку.
- `Caps Lock` - переключить активное окно на следующую раскладку и заблокировать стандартный Caps Lock.

Если `Ctrl` используется вместе с другой клавишей, переключение по отпусканию `Ctrl` не выполняется. Обработка `Ctrl+колесо мыши` еще не подключена в Rust runner.

## Fixtures

Regression fixtures лежат в:

```text
tests\fixtures\known_cases.json
```

Статусы:

- `active` - кейс должен проходить в Stepler.
- `expected_failure` - известный сценарий, который зафиксирован, но еще не включен как обязательный regression test.

## Логи операций

Основной машинно-читаемый формат для будущего runtime: JSONL, одна строка на событие lifecycle операции.

Минимальные поля события:

```json
{
  "operation_id": "...",
  "trigger": "Pause",
  "state": "ReplacementApplied",
  "duration_ms": 24
}
```

Расширенные поля для операций замены:

```json
{
  "operation_id": "...",
  "trigger": "Pause",
  "app": "Notepad",
  "provider": "Win32EditProvider",
  "replacer": "Win32EditReplacer",
  "state": "ReplacementApplied",
  "range": [10, 16],
  "expected_before_text": "k.,jdm",
  "replacement_text": "любовь",
  "clipboard_used": false,
  "duration_ms": 24,
  "timings_ms": {
    "context": 4,
    "plan": 1,
    "preflight": 2,
    "replace": 12,
    "verify": 3,
    "clipboard_restore": 0
  }
}
```
