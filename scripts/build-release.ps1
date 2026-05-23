param(
    [string]$Configuration = "Release",
    [string]$Runtime = "win-x64",
    [string]$DistDir = "",
    [string]$BuildVersion = "",
    [string]$FileVersion = ""
)

$ErrorActionPreference = "Stop"

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Resolve-Path (Join-Path $scriptDir "..")
Set-Location $repoRoot

if ([string]::IsNullOrWhiteSpace($DistDir)) {
    $DistDir = Join-Path $repoRoot "dist\Stepler"
}

if ([string]::IsNullOrWhiteSpace($BuildVersion)) {
    $BuildVersion = "0.1.0-alpha.$(Get-Date -Format 'yyyyMMdd.HHmm')"
}

if ([string]::IsNullOrWhiteSpace($FileVersion)) {
    $daysSinceEpoch = [int]([DateTime]::UtcNow.Date - [DateTime]'2026-01-01').TotalDays
    if ($daysSinceEpoch -lt 0) {
        $daysSinceEpoch = 0
    }
    if ($daysSinceEpoch -gt 65535) {
        $daysSinceEpoch = $daysSinceEpoch % 65535
    }
    $FileVersion = "0.1.0.$daysSinceEpoch"
}

$distPath = [System.IO.Path]::GetFullPath($DistDir)
$distScriptsPath = Join-Path $distPath "scripts"

Write-Host "Build version: $BuildVersion"
Write-Host "File version:  $FileVersion"

Write-Host "Building stepler-cli ($Configuration)..."
cargo build -p stepler-cli --release

Write-Host "Publishing Stepler tray host ($Configuration, $Runtime)..."
if (Test-Path $distPath) {
    try {
        Remove-Item $distPath -Recurse -Force
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
    -p:Version=$BuildVersion `
    -p:InformationalVersion=$BuildVersion `
    -p:FileVersion=$FileVersion `
    -p:AssemblyVersion=0.1.0.0 `
    -p:IncludeSourceRevisionInInformationalVersion=false

Write-Host "Copying runtime files..."
New-Item -ItemType Directory -Force -Path $distPath | Out-Null
New-Item -ItemType Directory -Force -Path $distScriptsPath | Out-Null

Copy-Item ".\target\release\stepler-cli.exe" (Join-Path $distPath "stepler-cli.exe") -Force
Copy-Item ".\scripts\Stepler.PSReadLine.ps1" (Join-Path $distScriptsPath "Stepler.PSReadLine.ps1") -Force

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
Stepler alpha build

Version:
  $BuildVersion

Run:
  Stepler.exe

Included:
  Stepler.exe              tray-only Windows UI
  stepler-cli.exe          hotkey runner and diagnostics
  scripts\Stepler.PSReadLine.ps1
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
