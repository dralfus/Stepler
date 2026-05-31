# Установка Stepler

## Инсталлятор

Собрать инсталлятор:

```powershell
cd <repo>\Stepler
.\scripts\build-installer.ps1
```

Результат:

```text
SetupOutput\SteplerSetup-<version>.exe
```

По умолчанию версия получает штамп сборки, например `0.1.0-alpha.20260523.1234`.
Эта же версия отображается в окне управления Stepler над статусом и записывается в `BUILD_INFO.txt`.

Можно задать версию вручную:

```powershell
.\scripts\build-installer.ps1 -BuildVersion 0.1.0-alpha.20260523.1234
```

Запустите инсталлятор от администратора. Он устанавливает Stepler в:

```text
C:\Program Files\Stepler
```

## Что Делает Инсталлятор

- копирует `Stepler.exe`, `stepler-cli.exe`, runtime-файлы, `BUILD_INFO.txt` и `scripts\Stepler.PSReadLine.ps1`;
- создает ярлыки в Start Menu и, опционально, на Desktop;
- регистрирует `App Paths\Stepler.exe`, чтобы Windows могла находить `Stepler.exe`;
- закрывает старые процессы `Stepler.exe`, `Stepler.Tray.exe` и `stepler-cli.exe` перед обновлением;
- удаляет предыдущую установленную версию перед установкой новой;
- предлагает запустить Stepler после установки.

## Runtime-Файлы

Настройки:

```text
%APPDATA%\Stepler\settings.json
```

Логи:

```text
%LOCALAPPDATA%\Stepler\logs\Stepler.Tray.log
%LOCALAPPDATA%\Stepler\logs\stepler_hotkey_log.jsonl
```

## PowerShell-Адаптер

Инсталлятор копирует PSReadLine-адаптер в:

```text
C:\Program Files\Stepler\scripts\Stepler.PSReadLine.ps1
```

Ручная загрузка в PowerShell:

```powershell
Import-Module PSReadLine
. "C:\Program Files\Stepler\scripts\Stepler.PSReadLine.ps1"
Get-SteplerPsReadLineStatus
```

## Требования

- Windows 10/11.
- .NET 9 Desktop Runtime, потому что текущий инсталлятор framework-dependent.
- Microsoft Word опционален и нужен только для Word COM support.

## Удаление

Используйте Windows Settings -> Apps -> Installed apps -> Stepler -> Uninstall.

Деинсталлятор удаляет установленные файлы и ярлыки. Пользовательские настройки и логи в `%APPDATA%`/`%LOCALAPPDATA%` намеренно остаются для диагностики и последующих переустановок.
