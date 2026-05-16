# Stepler

Stepler is a new implementation of HotkeyHandler. The project starts with a Rust core that can build deterministic text replacement plans without touching the active application or the clipboard.

Current workspace crates:

- `crates/stepler-core` - pure correction types and layout conversion.
- `crates/stepler-testkit` - small helpers for future tests.
- `apps/Stepler.Tray` - Windows tray-only host that runs the shared hotkey pipeline through `stepler-cli run-hotkeys`.

Development commands are documented in `docs/development_commands_ru.md`.

Tray app dev run:

```powershell
dotnet build .\apps\Stepler.Tray\Stepler.Tray.csproj -c Debug
Start-Process .\apps\Stepler.Tray\bin\Debug\net9.0-windows\Stepler.Tray.exe
```

The tray host starts `target\debug\stepler-cli.exe run-hotkeys` hidden and closes it from the tray menu.
