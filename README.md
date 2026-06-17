# Stepler

Stepler - Windows-утилита для исправления текста, набранного в неверной раскладке, и быстрого переключения языков ввода. Приложение работает из системного трея, запускает фоновый hotkey runner и старается менять текст только через безопасный метод, подходящий для активного приложения.

## Основные клавиши

- `Pause` (`P`) - безусловно конвертирует выделенный текст или слово слева от курсора.
- `Ctrl+Pause` (`CP`) - умная конвертация строки: ищет только подозрительные слова во фразе и не трогает корректный русский/английский текст.
- `Left Ctrl` отдельным нажатием - переключает активное окно на русскую раскладку.
- `Right Ctrl` отдельным нажатием - переключает активное окно на английскую раскладку.
- `Menu`/`Caps Lock` - переключение на следующую раскладку, если включено в настройках tray.

Для PowerShell используется PSReadLine adapter. После запуска tray Stepler автоматически добавляет загрузчик adapter-а в user profile PowerShell; новые окна PowerShell должны работать без ручной команды `. scripts\Stepler.PSReadLine.ps1`.

Для terminal TUI-приложений, которые запускаются внутри PowerShell и перехватывают ввод, например Qwen CLI, Stepler использует отдельный terminal-app режим. `Stepler.PSReadLine.ps1` автоматически добавляет wrapper для команды `qwen`: на время работы Qwen заголовок вкладки меняется на `stepler-terminal-app qwen`, launcher создает marker-файл и запускает Qwen с `--input-file`. Это дает безопасный side-channel для отправки готового текста в Qwen без `Ctrl+C`/`Ctrl+Shift+C`. Уже набранную строку внутри Qwen TUI Stepler снаружи безопасно прочитать не может: Qwen воспринимает `Ctrl+Shift+C` как interrupt. После обновления Stepler уже открытое окно PowerShell нужно перезапустить или выполнить `. $PROFILE`, затем заново запустить `qwen`.

Если wrapper из PowerShell profile не подхватился или нужно явно запустить Qwen в Stepler-режиме, используй launcher из установленной папки:

```powershell
F:\distr\system\Stepler\dist\Stepler\scripts\Stepler.Qwen.ps1
```

Альтернатива, если удобнее запускать `.cmd`:

```powershell
F:\distr\system\Stepler\dist\Stepler\scripts\stepler-qwen.cmd
```

Оба launcher-а запускают настоящий `qwen` из `PATH`, но перед этим ставят заголовок окна `stepler-terminal-app qwen`.
Если Qwen сам перезаписывает заголовок обратно на `Windows PowerShell`, launcher всё равно оставляет marker-файл `%LOCALAPPDATA%\Stepler\state\terminal-app-qwen.marker`; Stepler использует его как явный сигнал terminal-app режима до выхода из Qwen.

Проверка side-channel отправки текста в запущенный через wrapper Qwen:

```powershell
F:\distr\system\Stepler\dist\Stepler\stepler-cli.exe qwen-submit --text "проверка из Stepler"
```

Команда дописывает JSONL `submit` в Qwen `--input-file`. Это не заменяет P/CP для уже набранной строки в TUI, но позволяет безопасно отправлять готовый текст без терминальных copy shortcuts.

В tray-меню есть пункт `Qwen input...`: это небольшое окно ввода, где можно набрать текст, применить `P`/`CP` к содержимому окна и отправить результат в запущенный через wrapper Qwen.

## Требования для PowerShell

PowerShell не поддерживается через общий terminal clipboard fallback. Для PowerShell используется отдельный безопасный `PSReadLine` adapter: он читает текущую строку через `PSConsoleReadLine.GetBufferState`, строит план замены через `stepler-cli psreadline-plan` и применяет результат через `RevertLine` + `Insert`.

Чтобы `Pause` и `Ctrl+Pause` работали в PowerShell/Windows Terminal, должны выполняться условия:

- В интерактивной сессии PowerShell должен быть загружен модуль `PSReadLine`.
- Скрипт `scripts\Stepler.PSReadLine.ps1` должен быть загружен в текущую сессию PowerShell. Tray Stepler пытается автоматически добавить загрузку в PowerShell profile пользователя.
- Execution policy должна разрешать запуск profile scripts. Обычно достаточно:

```powershell
Set-ExecutionPolicy -Scope CurrentUser -ExecutionPolicy RemoteSigned
```

- После изменения profile или execution policy уже открытые окна PowerShell нужно перезапустить или вручную выполнить:

```powershell
. $PROFILE
Get-SteplerPsReadLineStatus
```

Проверка состояния:

```powershell
Get-ExecutionPolicy -List
$PROFILE
Test-Path $PROFILE
Get-SteplerPsReadLineStatus
Get-PSReadLineKeyHandler -Bound |
  Where-Object { $_.Key -match 'Pause|F11|F12' -or $_.BriefDescription -match 'Stepler' }
Get-Command qwen
```

Если `$PROFILE` указывает в `OneDrive\Documents`, а Windows Defender Controlled Folder Access включен, Stepler может быть заблокирован при попытке создать или изменить profile. В этом случае Windows Security показывает уведомление о запрете изменения защищенной папки. Возможные решения:

- создать `$PROFILE` вручную и добавить загрузку `Stepler.PSReadLine.ps1`;
- разрешить `Stepler.exe` в Windows Security -> Virus & threat protection -> Ransomware protection -> Allow an app through Controlled folder access;
- отключить OneDrive backup/sync для папки Documents, чтобы PowerShell profile вернулся в обычный локальный путь.

Ручная загрузка adapter-а в текущем окне PowerShell:

```powershell
Import-Module PSReadLine
$adapter = Join-Path (Get-Location) "dist\Stepler\scripts\Stepler.PSReadLine.ps1"
. $adapter
Get-SteplerPsReadLineStatus
```

## Умная конвертация

`Pause` делает прямое преобразование раскладки для выбранного фрагмента: например `k.,jdm` -> `любовь`, `привет мир` -> `ghbdtn vbh`.

`Ctrl+Pause` работает осторожнее. Stepler строит план замены для текущего контекста и оценивает токены как русский/английский текст, набранный в неверной раскладке. Основная идея - вероятностный анализ: n-граммы, частотные языковые модели, score фразы и confidence threshold. Словари используются как дополнительный сигнал и тестовые данные, но не должны быть единственным критерием. Благодаря этому алгоритм должен уметь работать с неизвестными словами, именами, техническими терминами и смешанными строками.

Пример:

```text
вальс поле long ghbdtn vbh
```

После `Ctrl+Pause`:

```text
вальс поле long привет мир
```

Корректный префикс остается без изменений, заменяется только подозрительный диапазон.

## Архитектура method adapters

Stepler не стремится писать отдельный большой адаптер под каждое приложение. Вместо этого используется набор method adapters - реальных техник чтения контекста и замены текста. Runtime resolver смотрит на активное окно, capabilities и app policy, затем выбирает безопасную пару методов.

Поддерживаемые method adapters:

- `Win32EditMessages` - чтение/замена обычных Win32 edit controls через `WM_GETTEXT`, `EM_GETSEL`, `EM_REPLACESEL`.
- `UIAutomationEditableText` - строгий UI Automation adapter для focused `ControlType.Edit` с writable `ValuePattern` и `TextPattern`.
- `UIAutomationDocumentText` - adapter для web/document surfaces через UIA `TextPattern`: работает с выделенным текстом и, если UIA безопасно отдает collapsed caret range, с текстом слева от курсора.
- `UIAutomationText` - базовый UIA text/value adapter для совместимых controls.
- `WordCom` - Word object model; также используется для Outlook desktop через WordEditor.
- `PSReadLine` - безопасная работа с буфером ввода PowerShell.
- `SshTerminal` - remote helper для Bash/readline поверх SSH: Windows Stepler форвардит приватные escape-последовательности, а конвертация выполняется на удаленной машине через `stepler-remote`.
- `ConsoleBuffer` - чтение классического console buffer.
- `TerminalClipboardShortcut` - диагностический/fallback путь для терминалов.
- `ClipboardSelection` - risky fallback для уже выделенного текста через clipboard copy/paste с восстановлением буфера.
- `SendInput` - risky write-only fallback для ввода Unicode-текста в текущее выделение.

Risky/fallback методы по умолчанию не должны включаться для неизвестных приложений без явного разрешения policy или диагностического режима.

## Проверенные приложения

Это информационный список ручных проверок, а не обещание полной поддержки всех версий приложений.

| Приложение/поверхность | Проверенный сценарий | Method adapter | Наблюдаемое время |
| --- | --- | --- | --- |
| Notepad | `P`, `CP`, сохранение clipboard | `Win32EditMessages` | p50 ~18 ms, avg ~22 ms |
| PowerShell / Windows Terminal, локальная сессия | `P`, `CP`, selection, переключение раскладки после конвертации | `PSReadLine` | обычно быстрее web/Word; отдельные hotkey-forward события ~426 ms |
| PowerShell / Windows Terminal, внутри запущен SSH | `P`/`CP` работают только после установки remote helper на Linux host и opt-in на Windows клиенте | `SshTerminal` / Bash readline helper | зависит от SSH latency; обычно сравнимо с локальным readline |
| Qwen CLI / terminal TUI в Windows Terminal | безопасное подавление `P`/`CP`; side-channel submit через `--input-file`; уже набранный prompt buffer не читается | `TerminalApp` policy + Qwen `--input-file` | `TerminalClipboardShortcut` и `Ctrl+Shift+C` запрещены, потому что Qwen воспринимает их как interrupt |
| Microsoft Word desktop | `P`, `CP`, выделение, диапазон слева от курсора | `WordCom` | p50 ~2266 ms, avg ~2188 ms |
| Microsoft Outlook desktop compose | WordEditor в письме, ожидаемый путь поддержки | `WordCom` через Outlook WordEditor | p50 ~2266 ms, avg ~2188 ms |
| Windows Settings / Feedback Hub / WPF TextBox fixture | caret-aware замена в editable UIA поле | `UIAutomationEditableText` / `UIAutomationText` | p50 ~1304 ms, avg ~1324 ms |
| Confluence / JIRA в Chrome/Firefox | выделенный текст в web editor; no-selection только если UIA проходит strict caret preflight | `UIAutomationDocumentText` / `WebKeyboardSelection` | `UIAutomationDocumentText`: p50 ~1114 ms, avg ~1653 ms; `WebKeyboardSelection`: p50 ~603 ms, avg ~842 ms |
| Codex / browser-like / Electron-like поля ввода | `P`, `CP` через keyboard selection path, если поле допускает безопасное выделение/проверку | `WebKeyboardSelection` | p50 ~603 ms, avg ~842 ms |
| classic `cmd.exe` / `conhost.exe` | `P` частично работает; `CP` нестабилен, может очищать/портить текущую строку | `ConsoleBuffer` | p50 ~108 ms, avg ~285 ms, но нестабильно |
| `cmd.exe` внутри Windows Terminal | основной безопасный adapter пока не реализован; terminal clipboard shortcut остается diagnostic/fallback | `TerminalClipboardShortcut` / policy | не считается поддержанным |
| Browser-like / Electron-like окна без безопасного text API | fail-closed, risky методы только явно | policy + diagnostics | не применяется |

Для web/document editor-ов no-selection режим включается только через `UIAutomationDocumentText` caret preflight: UIA должен вернуть стабильный collapsed range, Stepler выделяет ровно рассчитанный диапазон слева от caret и перед вводом проверяет совпадение текста.

Для terminal TUI вроде Qwen CLI Stepler не включает fallback для всех терминалов. Поддержка включается только если title окна содержит `qwen`, ручной маркер `stepler-terminal-app` или marker-файл launcher-а Qwen. `TerminalClipboardShortcut` для Qwen запрещен, потому что он может отправлять голый `Ctrl+C`; `Ctrl+Shift+C` также не используется, потому что Qwen воспринимает его как interrupt. Для Qwen используется side-channel `--input-file`: Stepler может отправить готовое сообщение через `stepler-cli qwen-submit`, но не читает текущую строку TUI.

`UIAutomationEditableText` проверялся на обычных editable UIA полях: Windows Settings, Feedback Hub и тестовый WPF TextBox (`stepler-cli uia-fixture`). Это поля, которые UI Automation видит как редактируемый `ControlType.Edit` с writable `ValuePattern`.

`UIAutomationDocumentText` проверялся на document/web surfaces: например редакторы Confluence/JIRA в браузере, где поле выглядит для UIA не как классический edit control, а как документ с `TextPattern`. В таких местах надежность зависит от того, отдает ли приложение стабильный selection/caret range.

## Структура проекта

- `crates/stepler-core` - чистые типы коррекции, layout conversion и построение replacement plan.
- `crates/stepler-app` - operation runner, транзакции, clipboard guard.
- `crates/stepler-cli` - CLI, диагностика, hotkey runner и PSReadLine bridge.
- `crates/stepler-platform` - platform-neutral контракты resolver-а и адаптеров.
- `crates/stepler-platform-windows` - Windows method adapters, hotkey hook и layout switch.
- `crates/stepler-remote` - маленький Linux/SSH helper для Bash/readline, использующий тот же `stepler-core`.
- `crates/stepler-testkit` - тестовые helpers.
- `apps/Stepler.Tray` - tray-only Windows host, который запускает `stepler-cli run-hotkeys`.
- `scripts` - сборка release/installer и PSReadLine adapter.
- `docs` - release notes, smoke checklist и команды разработки.

`crates` - термин Rust: каждый каталог является отдельным пакетом внутри одного workspace. `apps` содержит пользовательские приложения. Build outputs находятся в `target`, `bin`, `obj`, `dist` и не должны попадать в commit.

## Команды разработки

Debug-сборка tray:

```powershell
dotnet build .\apps\Stepler.Tray\Stepler.Tray.csproj -c Debug
Start-Process .\apps\Stepler.Tray\bin\Debug\net9.0-windows\Stepler.exe
```

Rust-проверки:

```powershell
cargo fmt --all -- --check
cargo test --workspace
```

SSH на удаленный Linux host. Предпочтительный путь: собрать helper на машине разработчика через WSL, а на VPS только скопировать готовый бинарник.

```powershell
.\scripts\build-remote-linux.ps1
```

После этого скопируй файлы из `dist\Stepler\remote\linux-x64` на удаленную машину:

```bash
mkdir -p ~/.local/bin ~/.config/stepler
cp stepler-remote ~/.local/bin/
cp Stepler.SSHReadline.bash ~/.config/stepler/
chmod +x ~/.local/bin/stepler-remote
grep -qxF 'source ~/.config/stepler/Stepler.SSHReadline.bash' ~/.bashrc || echo 'source ~/.config/stepler/Stepler.SSHReadline.bash' >> ~/.bashrc
```

После этого открой новую SSH-сессию. Удаленный скрипт помечает terminal title только если `stepler-remote` найден на этом host; Windows Stepler форвардит `P`/`CP` только в такие помеченные SSH-сессии. Если на другом host helper не установлен, Stepler продолжает fail-closed поведение и подавляет `P`/`CP`, чтобы не портить удаленную строку ввода. Это не полноценный Linux-port с глобальными hotkeys, а узкий Bash/readline adapter для SSH.

Release build:

```powershell
.\scripts\build-release.ps1
Start-Process .\dist\Stepler\Stepler.exe
```

Installer build:

```powershell
.\scripts\build-installer.ps1
```

Инсталлятор создается в `SetupOutput\SteplerSetup-<version>.exe`.
Если `-BuildVersion` не указан, сборка получает уникальную версию со штампом времени, например `1.0.20260617.t0950`; эта же версия отображается в окне управления Stepler и записывается в `dist\Stepler\BUILD_INFO.txt`.

Дополнительные документы:

- [Команды разработки](docs/development_commands_ru.md)
- [Release smoke checklist](docs/release_smoke_checklist_ru.md)
- [Установка](INSTALL_README.md)
- [Release notes 1.0](docs/release_notes_1.0_ru.md)
