# Stepler

Stepler - Windows-утилита для исправления текста, набранного в неверной раскладке, и быстрого переключения языков ввода. Приложение работает из системного трея, запускает фоновый hotkey runner и старается менять текст только через безопасный метод, подходящий для активного приложения.

## Основные клавиши

- `Pause` (`P`) - безусловно конвертирует выделенный текст или слово слева от курсора.
- `Ctrl+Pause` (`CP`) - умная конвертация строки: ищет только подозрительные слова во фразе и не трогает корректный русский/английский текст.
- `Left Ctrl` отдельным нажатием - переключает активное окно на русскую раскладку.
- `Right Ctrl` отдельным нажатием - переключает активное окно на английскую раскладку.
- `Menu`/`Caps Lock` - переключение на следующую раскладку, если включено в настройках tray.

Для PowerShell используется PSReadLine adapter. После запуска tray Stepler автоматически добавляет загрузчик adapter-а в user profile PowerShell; новые окна PowerShell должны работать без ручной команды `. scripts\Stepler.PSReadLine.ps1`.

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
- `UIAutomationDocumentText` - selection-only adapter для web/document surfaces через UIA `TextPattern`.
- `UIAutomationText` - базовый UIA text/value adapter для совместимых controls.
- `WordCom` - Word object model; также используется для Outlook desktop через WordEditor.
- `PSReadLine` - безопасная работа с буфером ввода PowerShell.
- `ConsoleBuffer` - чтение классического console buffer.
- `TerminalClipboardShortcut` - диагностический/fallback путь для терминалов.
- `ClipboardSelection` - risky fallback для уже выделенного текста через clipboard copy/paste с восстановлением буфера.
- `SendInput` - risky write-only fallback для ввода Unicode-текста в текущее выделение.

Risky/fallback методы по умолчанию не должны включаться для неизвестных приложений без явного разрешения policy или диагностического режима.

## Проверенные приложения

Это информационный список ручных проверок, а не обещание полной поддержки всех версий приложений.

| Приложение/поверхность | Проверенный сценарий | Method adapter |
| --- | --- | --- |
| Notepad | `P`, `CP`, сохранение clipboard | `Win32EditMessages` |
| PowerShell / Windows Terminal | `P`, `CP`, selection, переключение раскладки после конвертации | `PSReadLine` |
| Microsoft Word desktop | `P`, `CP`, выделение, диапазон слева от курсора | `WordCom` |
| Microsoft Outlook desktop compose | WordEditor в письме, ожидаемый путь поддержки | `WordCom` через Outlook WordEditor |
| Windows Settings / Feedback Hub / WPF TextBox fixture | caret-aware замена в editable UIA поле | `UIAutomationEditableText` / `UIAutomationText` |
| Confluence / JIRA в Chrome/Firefox | выделенный текст в web editor | `UIAutomationDocumentText` |
| Browser-like / Electron-like окна без безопасного text API | fail-closed, risky методы только явно | policy + diagnostics |

Для Confluence/JIRA текущий безопасный web-путь является selection-only: без выделения Stepler не пытается угадывать слово у caret, чтобы не удалить лишний текст в web editor.

## Структура проекта

- `crates/stepler-core` - чистые типы коррекции, layout conversion и построение replacement plan.
- `crates/stepler-app` - operation runner, транзакции, clipboard guard.
- `crates/stepler-cli` - CLI, диагностика, hotkey runner и PSReadLine bridge.
- `crates/stepler-platform` - platform-neutral контракты resolver-а и адаптеров.
- `crates/stepler-platform-windows` - Windows method adapters, hotkey hook и layout switch.
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

Дополнительные документы:

- [Команды разработки](docs/development_commands_ru.md)
- [Release smoke checklist](docs/release_smoke_checklist_ru.md)
- [Установка](INSTALL_README.md)
- [Release notes 0.1.0](docs/release_notes_0.1.0_ru.md)
