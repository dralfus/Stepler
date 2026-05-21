# Stepler Installation Guide

## Installer

Build the installer:

```powershell
cd F:\distr\system\Stepler
.\scripts\build-installer.ps1
```

The output is:

```text
SetupOutput\SteplerSetup-0.1.0.exe
```

Run the installer as administrator. It installs Stepler to:

```text
C:\Program Files\Stepler
```

## What The Installer Does

- copies `Stepler.exe`, `stepler-cli.exe`, runtime files, and `scripts\Stepler.PSReadLine.ps1`;
- creates Start Menu and optional Desktop shortcuts;
- registers `App Paths\Stepler.exe`, so `Stepler.exe` can be resolved by Windows;
- closes old `Stepler.exe`, `Stepler.Tray.exe`, and `stepler-cli.exe` processes before update;
- removes the previous installed version before installing the new one;
- offers to launch Stepler after install.

## Runtime Files

Settings:

```text
%APPDATA%\Stepler\settings.json
```

Logs:

```text
%LOCALAPPDATA%\Stepler\logs\Stepler.Tray.log
%LOCALAPPDATA%\Stepler\logs\stepler_hotkey_log.jsonl
```

## PowerShell Adapter

The installer copies the PSReadLine adapter to:

```text
C:\Program Files\Stepler\scripts\Stepler.PSReadLine.ps1
```

Manual load in PowerShell:

```powershell
Import-Module PSReadLine
. "C:\Program Files\Stepler\scripts\Stepler.PSReadLine.ps1"
Get-SteplerPsReadLineStatus
```

## Requirements

- Windows 10/11.
- .NET 9 Desktop Runtime, because the current installer is framework-dependent.
- Microsoft Word is optional and only needed for Word COM support.

## Uninstall

Use Windows Settings -> Apps -> Installed apps -> Stepler -> Uninstall.

The uninstaller removes installed files and shortcuts. User settings and logs under `%APPDATA%`/`%LOCALAPPDATA%` are intentionally left for diagnostics and future reinstall continuity.
