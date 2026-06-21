# Stepler 1.0

Первый релиз Stepler для Windows.

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
  - `SshTerminal` remote helper для SSH/Bash;
  - `ClipboardSelection` как risky fallback;
  - `SendInput` как risky fallback.
- App policies и runtime resolver с fail-closed поведением для неподтвержденных приложений.
- Инсталлятор Inno Setup.

## Основные поддерживаемые сценарии

- Notepad.
- Microsoft Word через COM.
- Microsoft Outlook compose через WordEditor.
- PowerShell через `scripts\Stepler.PSReadLine.ps1`.
- SSH-сессии в PowerShell/Windows Terminal при установленном `stepler-remote` на удаленном Bash host.
- Browser/Electron-like поля через безопасные web/UIA adapters там, где проходит preflight.
- UI Automation `ValuePattern` поля, например часть Windows Settings/Feedback Hub/WPF fixture.

## Логи

```text
%LOCALAPPDATA%\Stepler\logs\Stepler.Tray.log
%LOCALAPPDATA%\Stepler\logs\stepler_hotkey_log.jsonl
```

## Известные ограничения

- Classic `cmd.exe`/`conhost.exe` и `cmd.exe` внутри Windows Terminal не считаются поддержанными сценариями.
- Risky fallback adapters выключены по умолчанию.
- `Ctrl+Pause` используется для умного режима строки во всех приложениях, включая PowerShell через PSReadLine.
- Инсталлятор требует установленный .NET 9 Desktop Runtime.
- Полноценная Linux desktop-версия отложена; для SSH/Bash есть отдельный remote helper.
