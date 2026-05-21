# Stepler 0.1.0 alpha

Первый alpha-релиз Stepler для Windows.

## Что входит

- Tray-only приложение `Stepler.exe`.
- Фоновый runner `stepler-cli.exe`.
- Горячие клавиши `Pause` и `Ctrl+Pause`.
- Переключение раскладки одиночными `Left Ctrl`, `Right Ctrl`, `Menu`/`Caps Lock`.
- Настройки tray с сохранением в `%APPDATA%\Stepler\settings.json`.
- Method adapters:
  - `Win32EditMessages`;
  - `UIAutomationEditableText`;
  - `UIAutomationDocumentText`;
  - `UIAutomationText`;
  - `WordCom`;
  - `PSReadLine`;
  - `ConsoleBuffer`;
  - `TerminalClipboardShortcut`;
  - `ClipboardSelection` как risky fallback;
  - `SendInput` как risky fallback.
- App policies и runtime resolver с fail-closed поведением для неподтвержденных приложений.
- Инсталлятор Inno Setup.

## Основные поддерживаемые сценарии

- Notepad.
- Microsoft Word через COM.
- PowerShell через `scripts\Stepler.PSReadLine.ps1`.
- UI Automation `ValuePattern` поля, например часть Windows Settings/Feedback Hub/WPF fixture.
- Выделенный текст в UIA `Document/TextPattern` surfaces, например Confluence/JIRA в браузере, через selection-only `UIAutomationDocumentText`.

## Логи

```text
%LOCALAPPDATA%\Stepler\logs\Stepler.Tray.log
%LOCALAPPDATA%\Stepler\logs\stepler_hotkey_log.jsonl
```

## Известные ограничения

- Browser/Electron/Codex-style поля не считаются основным поддерживаемым путем в alpha.
- Risky fallback adapters выключены по умолчанию.
- `Ctrl+Pause` используется для умного режима строки во всех приложениях, включая PowerShell через PSReadLine.
- Инсталлятор требует установленный .NET 9 Desktop Runtime.
- Linux support отложен.
