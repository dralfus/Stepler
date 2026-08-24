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

## Правило изменения адаптеров

Перед изменением method adapter-а сначала определить, что именно меняется:

1. `AdapterContract`, если меняются технические возможности метода.
2. `ProbePolicy`, если метод должен или не должен пробоваться на surface.
3. `SurfacePolicy`, если метод должен или не должен выбираться resolver-ом.
4. `probe_contracts.tsv` / `resolver_contracts.tsv`, если меняется проверенная surface.
5. `replacement_behavior.tsv`, если меняется range/caret/selection behavior.

Не добавлять проверки вида "если это Jira/Rocket/Outlook" прямо в adapter probe.
App/surface routing должен жить в classifier, `ProbePolicy`, `SurfacePolicy` и
contract fixtures. Если хочется менять `WebKeyboardSelectionMethod.probe`,
сначала разделить: это technical predicate или policy-решение.

Если новое приложение определяется как `SurfaceKind::Unknown`, не расширять
обычный `Unknown` fallback ради быстрого исправления. Правильный путь - добавить
или уточнить surface contract в `probe_contracts.tsv` / `resolver_contracts.tsv`
и затем менять classifier/policy. Широкий probing для неизвестных окон разрешен
только как явная диагностика через `STEPLER_DIAGNOSTIC_UNKNOWN_PROBES`.

### Чеклист добавления нового приложения или surface

1. Снять диагностику в нужном поле ввода:

   ```powershell
   F:\distr\system\Stepler\dist\Stepler\stepler-cli.exe diagnose-focus --delay 3 --methods --surface
   ```

2. По выводу диагностики определить, что меняется:
   - только признаки окна -> `TargetFacts` / `classify_surface`;
   - разрешенные методы -> `ProbePolicy` / `SurfacePolicy`;
   - техническая возможность метода -> adapter implementation и
     `AdapterContract`.
3. Если нужна новая surface, добавить или уточнить `SurfaceKind` и classifier
   evidence. Не расширять `Unknown`.
4. Добавить или обновить строку в
   `crates/stepler-platform/tests/fixtures/probe_contracts.tsv`.
5. Добавить или обновить строку в
   `crates/stepler-platform/tests/fixtures/resolver_contracts.tsv`.
6. Проверить forbidden/risky/bridge methods:
   - risky method требует явного policy allowance;
   - bridge/control-plane method требует явного surface allowance;
   - generic clipboard/send_input fallback не должен попадать в обычную
     surface без отдельного contract.
7. Только после этого менять technical adapter, если contracts показывают, что
   проблема действительно в техническом capture/apply.
8. Для UI/runtime cases добавить manual smoke note в подходящий checklist или
   рядом с диагностикой задачи.
9. Минимальная проверка перед завершением:

   ```powershell
   cargo fmt
   cargo test -p stepler-platform
   ```

Актуальный план стабилизации архитектуры находится в
`docs/stabilization_plan_ru.md`. Старые phase-by-phase hardening/review
документы больше не являются источником правды.

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

Для диагностики surface classifier-а, probe policy и resolver trace:

```powershell
F:\distr\system\Stepler\dist\Stepler\stepler-cli.exe diagnose-focus --delay 3 --methods --surface
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

Для performance baseline перед запуском runner нужно задать обезличенную метку
окружения. На домашнем ПК используй `home-win11`, на рабочем - `work-win11`:

```powershell
$env:STEPLER_PERF_ENV = "home-win11"
cargo run -p stepler-cli -- run-hotkeys
```

Для обычного tray release переменная должна быть задана **до** его запуска.
Ниже команда останавливает только Stepler из текущего `dist` и запускает его в
той же PowerShell-сессии с нужной меткой:

```powershell
$env:STEPLER_PERF_ENV = "work-win11" # либо home-win11
$dist = "F:\distr\system\Stepler\dist\Stepler"
Get-Process Stepler, stepler-cli -ErrorAction SilentlyContinue |
  Where-Object { $_.Path -like "$dist\\*" } |
  Stop-Process -Force
Start-Process "$dist\Stepler.exe" -WorkingDirectory $dist -WindowStyle Hidden
```

Если переменная не задана, event получает явную метку `unlabeled` и не должен
попадать в сравнительный snapshot. Release build version читается из
`BUILD_INFO.txt` рядом с `stepler-cli.exe`; для debug-запуска допускается
`STEPLER_BUILD_VERSION`.

Строки с `event=performance_operation_v1` содержат только обезличенные поля:
build/environment, surface, method, profile, branch, trigger, selection,
cold/warm, retry, ranges, lengths, outcome и phase timings. Существующие
диагностические события с preview текста сохраняются отдельно и не должны
использоваться для performance aggregation.

В `timings_ms` обычные lifecycle timings и bridge timings имеют единую форму:

```json
[
  {"phase":"capture","elapsed_ms":12},
  {"phase":"verify","elapsed_ms":260},
  {"phase":"clipboard_restore","elapsed_ms":3}
]
```

Для PSReadLine primary event содержит `capture`, `correction_plan`,
`replacement` и `primary_layout_switch`. Фактический отложенный repair
записывается отдельным event с branch
`psreadline-delayed-layout-repair`, поэтому его длительность не смешивается с
основной операцией. Xterm события используют отдельные `capture`, `apply`,
`verify`, `retry` и `clipboard_restore`; Qwen получает `QwenTerminal` surface,
а SSH forwarding получает отдельные `SshRemote` surface и
`ssh-remote-forwarded` branch. Его `duration_ms` и фаза `apply` измеряют только
локальную передачу shortcut, а не сетевую задержку SSH.
Обычный PowerShell получает `PowerShell` surface.

### Воспроизводимый performance snapshot

После сбора данных текущей release-сборкой создай отдельный snapshot, не
перезаписывая накопительный JSONL. Лог обычно содержит несколько сборок,
поэтому сначала отфильтруй только текущую build/environment пару:

```powershell
$build = (Get-Content .\dist\Stepler\BUILD_INFO.txt |
  Where-Object { $_ -like "BuildVersion:*" }).Split(":", 2)[1].Trim()
$environment = "work-win11" # либо home-win11
$source = "$env:LOCALAPPDATA\Stepler\logs\stepler_hotkey_log.jsonl"
$input = Join-Path $env:TEMP "stepler-perf-$environment-$build.jsonl"
$output = Join-Path $env:TEMP "stepler-perf-$environment-$build.snapshot.json"

Get-Content $source | Where-Object {
  $_ -match '"event":"performance_operation_v1"' -and
  $_ -match ('"build_version":"' + [regex]::Escape($build) + '"') -and
  $_ -match ('"environment_label":"' + [regex]::Escape($environment) + '"')
} | Set-Content $input -Encoding utf8

& .\dist\Stepler\stepler-cli.exe performance-snapshot `
  --input $input `
  --output $output
```

Команда обрабатывает только строки `event=performance_operation_v1`, исключает
`environment_label=unlabeled` и требует один `build_version` на snapshot. При
нескольких сборках команда завершается ошибкой, чтобы baseline нельзя было
смешать. В `groups` cold и warm идут отдельными записями с `N`, p50/p90/p95,
max, failure rate, retry rate, outcome counts и вкладом фаз; `bottleneck_phase`
указывает фазу с наибольшим суммарным временем. В `sample_assessments` для
каждого method/surface/branch/trigger/selection набора указано, достаточно ли
30 успешных warm и 5 cold наблюдений, либо перечислены недостающие минимумы.
`warm_n` отражает все warm events, а `warm_completed_n` - только `Completed`;
именно второй счётчик участвует в статусе `sufficient`. Любой
`RolledBackOrFailed` показывается как `destructive_outcome_n` и переводит
assessment в `blocked_by_destructive_outcomes`.
Проверяются только environment labels `home-win11` и `work-win11`, а также
терминальные outcomes текущей telemetry-схемы.

Вход должен быть записан текущей telemetry-схемой, где timings имеют вид
`timings_ms[].phase`. Старый накопительный лог с `timings_ms[].state` не является
воспроизводимым T03 baseline: сначала собери новый лог после T02.

Остановить runner можно через `Ctrl+C` в его консоли.

В этом же runner включены клавиши переключения раскладки по ТЗ:

- `Left Ctrl` отдельно - переключить активное окно на русскую раскладку.
- `Right Ctrl` отдельно - переключить активное окно на английскую раскладку.
- `Menu` - переключить активное окно на следующую раскладку.
- `Caps Lock` - переключить активное окно на следующую раскладку и заблокировать стандартный Caps Lock.

Если `Ctrl` используется вместе с другой клавишей, переключение по отпусканию `Ctrl` не выполняется. Обработка `Ctrl+колесо мыши` еще не подключена в Rust runner.

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

Диагностические события могут содержать пользовательский текст и нужны только
для расследования ошибки. Для сравнения скорости используй исключительно
отдельные `performance_operation_v1` события: они не содержат текста.

Минимальная форма performance-события:

```json
{
  "event": "performance_operation_v1",
  "operation_id": "...",
  "trigger": "Pause",
  "outcome": "Completed",
  "build_version": "1.0.20260821.t2216",
  "environment_label": "work-win11",
  "surface_kind": "BrowserEditor",
  "context_method": "web_keyboard_selection",
  "replacement_method": "web_keyboard_selection",
  "algorithm_branch": "web-keyboard-line-selection",
  "clipboard_used": false,
  "duration_ms": 264,
  "timings_ms": [
    {"phase":"ContextCaptured","elapsed_ms":80},
    {"phase":"ReplacementApplied","elapsed_ms":64},
    {"phase":"Verified","elapsed_ms":120}
  ]
}
```
