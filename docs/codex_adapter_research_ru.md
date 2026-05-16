# Codex adapter: первичная диагностика

Дата первичной проверки: 2026-05-08.

## Win32 focus

Найденное top-level окно:

```text
title: Codex
class: Chrome_WidgetWin_1
```

После `SetForegroundWindow` Win32 focus остается на top-level окне:

```text
foreground: Chrome_WidgetWin_1 / Codex
focused:    Chrome_WidgetWin_1 / Codex
```

Вывода focused `Edit`/`RichEdit*` для поля ввода нет, поэтому текущий `Win32EditProvider` должен безопасно отказываться с `UnsupportedControl`.

## UI Automation

`AutomationElement::FromHandle` для окна `Codex` возвращает root:

```text
ControlType.Window
Name: Codex
ClassName: Chrome_WidgetWin_1
```

Control view descendants на момент проверки содержали только Chromium shell/panes:

```text
Pane / Intermediate D3D Window
Pane / RootView
Pane / NonClientView
Pane / WinFrameView
Pane / ClientView
Pane / View
```

`Edit`, `Document`, `TextPattern` или `ValuePattern` для поля ввода не обнаружены.

## Вывод

Codex в текущей конфигурации выглядит как Chromium/Electron окно без доступного Win32 edit-control и без полезного UIA text pattern. Следующий adapter-path нельзя строить как простой `UIAutomationProvider`.

Безопасная стратегия:

- `Win32EditProvider` для Codex должен fail-closed.
- Для Codex adapter нужно отдельно исследовать Chromium accessibility или другой app-specific API.
- Clipboard/synthetic input fallback допускается только после отдельного проектного решения и UI smoke-tests на clipboard invariant/no delayed paste.

## Команды

Диагностика foreground/focused:

```powershell
cargo run -p stepler-cli -- diagnose-focus --delay 3
```

Ручной smoke Notepad/clipboard:

```powershell
cargo test -p stepler-cli --test notepad_smoke -- --ignored --nocapture
```
