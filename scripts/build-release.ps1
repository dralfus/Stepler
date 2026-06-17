param(
    [string]$Configuration = "Release",
    [string]$Runtime = "win-x64",
    [string]$DistDir = "",
    [string]$BuildVersion = "",
    [string]$FileVersion = "",
    [switch]$BuildLinuxRemote,
    [string]$WslDistro = ""
)

$ErrorActionPreference = "Stop"

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Resolve-Path (Join-Path $scriptDir "..")
Set-Location $repoRoot

if ([string]::IsNullOrWhiteSpace($DistDir)) {
    $DistDir = Join-Path $repoRoot "dist\Stepler"
}

if ([string]::IsNullOrWhiteSpace($BuildVersion)) {
    $buildDate = Get-Date -Format 'yyyyMMdd'
    $buildTime = Get-Date -Format 'HHmm'
    $BuildVersion = "1.0.$buildDate.t$buildTime"
}

if ([string]::IsNullOrWhiteSpace($FileVersion)) {
    $daysSinceEpoch = [int]([DateTime]::UtcNow.Date - [DateTime]'2026-01-01').TotalDays
    if ($daysSinceEpoch -lt 0) {
        $daysSinceEpoch = 0
    }
    if ($daysSinceEpoch -gt 65535) {
        $daysSinceEpoch = $daysSinceEpoch % 65535
    }
    $FileVersion = "1.0.0.$daysSinceEpoch"
}

$distPath = [System.IO.Path]::GetFullPath($DistDir)
$distScriptsPath = Join-Path $distPath "scripts"
$distResourcesPath = Join-Path $distPath "resources"

Write-Host "Build version: $BuildVersion"
Write-Host "File version:  $FileVersion"

Write-Host "Building stepler-cli ($Configuration)..."
cargo build -p stepler-cli --release
if ($LASTEXITCODE -ne 0) {
    throw "cargo build failed with exit code $LASTEXITCODE"
}

Write-Host "Publishing Stepler tray host ($Configuration, $Runtime)..."
if (Test-Path $distPath) {
    try {
        Get-ChildItem $distPath -Force | ForEach-Object {
            if ($_.Name -eq "remote") {
                return
            }
            Remove-Item $_.FullName -Recurse -Force
        }
    }
    catch {
        throw "Cannot clean release output '$distPath'. Close Stepler if it is running from this folder and retry. $($_.Exception.Message)"
    }
}

dotnet publish ".\apps\Stepler.Tray\Stepler.Tray.csproj" `
    -nologo `
    -c $Configuration `
    -r $Runtime `
    --self-contained false `
    -o $distPath `
    -p:Version=1.0.0 `
    -p:InformationalVersion=$BuildVersion `
    -p:FileVersion=$FileVersion `
    -p:AssemblyVersion=1.0.0.0 `
    -p:IncludeSourceRevisionInInformationalVersion=false
if ($LASTEXITCODE -ne 0) {
    throw "dotnet publish failed with exit code $LASTEXITCODE"
}

Write-Host "Copying runtime files..."
New-Item -ItemType Directory -Force -Path $distPath | Out-Null
New-Item -ItemType Directory -Force -Path $distScriptsPath | Out-Null
New-Item -ItemType Directory -Force -Path $distResourcesPath | Out-Null

Copy-Item ".\target\release\stepler-cli.exe" (Join-Path $distPath "stepler-cli.exe") -Force
Copy-Item ".\scripts\Stepler.PSReadLine.ps1" (Join-Path $distScriptsPath "Stepler.PSReadLine.ps1") -Force
Copy-Item ".\scripts\Stepler.SSHReadline.bash" (Join-Path $distScriptsPath "Stepler.SSHReadline.bash") -Force
Copy-Item ".\scripts\Stepler.Qwen.ps1" (Join-Path $distScriptsPath "Stepler.Qwen.ps1") -Force
Copy-Item ".\scripts\stepler-qwen.cmd" (Join-Path $distScriptsPath "stepler-qwen.cmd") -Force
Copy-Item ".\crates\stepler-core\resources\layout-overrides.tsv" (Join-Path $distResourcesPath "layout-overrides.tsv") -Force

if ($BuildLinuxRemote) {
    $remoteDistPath = Join-Path $distPath "remote\linux-x64"
    $remoteArgs = @("-DistDir", $remoteDistPath)
    if (-not [string]::IsNullOrWhiteSpace($WslDistro)) {
        $remoteArgs += @("-WslDistro", $WslDistro)
    }
    & ".\scripts\build-remote-linux.ps1" @remoteArgs
}

$buildInfo = @"
Stepler build

BuildVersion: $BuildVersion
FileVersion: $FileVersion
Configuration: $Configuration
Runtime: $Runtime
BuiltAt: $((Get-Date).ToString("yyyy-MM-dd HH:mm:ss zzz"))
"@

Set-Content -Path (Join-Path $distPath "BUILD_INFO.txt") -Value $buildInfo -Encoding UTF8

$readme = @"
Stepler 1.0 build

Version:
  $BuildVersion

Run:
  Stepler.exe

Included:
  Stepler.exe              tray-only Windows UI
  stepler-cli.exe          hotkey runner and diagnostics
  scripts\Stepler.PSReadLine.ps1
  scripts\Stepler.SSHReadline.bash
  scripts\Stepler.Qwen.ps1
  scripts\stepler-qwen.cmd
  resources\layout-overrides.tsv
  BUILD_INFO.txt

Logs:
  %LOCALAPPDATA%\Stepler\logs\Stepler.Tray.log
  %LOCALAPPDATA%\Stepler\logs\stepler_hotkey_log.jsonl

PowerShell PSReadLine adapter:
  Stepler tray installs an auto-load block into the current user's PowerShell profiles.
  Existing PowerShell windows must be restarted after first launch/update.
  Manual fallback:
    Import-Module PSReadLine
    . <this folder>\scripts\Stepler.PSReadLine.ps1

SSH Bash/readline adapter:
  Preferred: build the Linux helper on the developer machine with:
    .\scripts\build-remote-linux.ps1
  or run build-release with -BuildLinuxRemote.
  Copy remote\linux-x64\stepler-remote and remote\linux-x64\Stepler.SSHReadline.bash
  to the Linux host. Cargo is not needed on the remote VPS.
  Open a new SSH session after installation. The remote script marks the terminal title only
  when stepler-remote is available; Stepler forwards Pause/Ctrl+Pause only to marked SSH tabs.

Qwen CLI inside PowerShell/Windows Terminal:
  Run:
    scripts\Stepler.Qwen.ps1
  or:
    scripts\stepler-qwen.cmd
  The launcher marks the terminal title as "stepler-terminal-app qwen" while Qwen is running.

Main shortcuts:
  Pause                    convert current word/selection
  Ctrl+Pause               convert phrase around caret
  Left Ctrl                switch active window to RU
  Right Ctrl               switch active window to EN
"@

Set-Content -Path (Join-Path $distPath "README_RELEASE.txt") -Value $readme -Encoding UTF8

Write-Host ""
Write-Host "Release output:"
Write-Host "  $distPath"
Get-ChildItem $distPath | Select-Object Name,Length,LastWriteTime | Format-Table -AutoSize
