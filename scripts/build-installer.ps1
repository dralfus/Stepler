param(
    [string]$Configuration = "Release",
    [string]$Runtime = "win-x64",
    [string]$BuildVersion = "",
    [string]$FileVersion = "",
    [string]$DistDir = "",
    [string]$SetupOutputDir = ""
)

$ErrorActionPreference = "Stop"

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Resolve-Path (Join-Path $scriptDir "..")
$setupScript = Join-Path $repoRoot "setup.iss"
$releaseScript = Join-Path $scriptDir "build-release.ps1"
if ([string]::IsNullOrWhiteSpace($DistDir)) {
    $distDir = Join-Path $repoRoot "dist\Stepler"
} else {
    $distDir = [System.IO.Path]::GetFullPath($DistDir)
}
if ([string]::IsNullOrWhiteSpace($SetupOutputDir)) {
    $setupOutputDir = Join-Path $repoRoot "SetupOutput"
} else {
    $setupOutputDir = [System.IO.Path]::GetFullPath($SetupOutputDir)
}

Set-Location $repoRoot

$isccCandidates = New-Object System.Collections.Generic.List[string]
$resolvedIscc = Get-Command iscc -ErrorAction SilentlyContinue
if ($resolvedIscc -and $resolvedIscc.Path) {
    $isccCandidates.Add($resolvedIscc.Path)
}

foreach ($candidate in @(
    (Join-Path $env:LOCALAPPDATA "Programs\Inno Setup 6\ISCC.exe"),
    "C:\Program Files (x86)\Inno Setup 6\ISCC.exe",
    "C:\Program Files\Inno Setup 6\ISCC.exe"
)) {
    if ((Test-Path $candidate) -and -not $isccCandidates.Contains($candidate)) {
        $isccCandidates.Add($candidate)
    }
}

if (-not $isccCandidates) {
    throw "ISCC.exe not found. Install Inno Setup 6 or add ISCC.exe to PATH."
}

$isccPath = $isccCandidates[0]

Write-Host "Building release payload..." -ForegroundColor Cyan
& $releaseScript -Configuration $Configuration -Runtime $Runtime -DistDir $distDir -BuildVersion $BuildVersion -FileVersion $FileVersion

$exePath = Join-Path $distDir "Stepler.exe"
if (-not (Test-Path $exePath)) {
    throw "Release executable not found: $exePath"
}

$productVersion = (Get-Item $exePath).VersionInfo.ProductVersion
if ([string]::IsNullOrWhiteSpace($productVersion)) {
    throw "ProductVersion is empty for $exePath"
}

New-Item -ItemType Directory -Force -Path $setupOutputDir | Out-Null
Get-ChildItem -Path $setupOutputDir -Filter "SteplerSetup-*.exe" -File -ErrorAction SilentlyContinue |
    Remove-Item -Force -ErrorAction SilentlyContinue

Write-Host "Building installer for version $productVersion..." -ForegroundColor Cyan
$relativeDistDir = [System.IO.Path]::GetRelativePath($repoRoot, $distDir)
$relativeSetupOutputDir = [System.IO.Path]::GetRelativePath($repoRoot, $setupOutputDir)
& $isccPath "/DMyAppVersion=$productVersion" "/DMyAppDistDir=$relativeDistDir" "/DMyAppOutputDir=$relativeSetupOutputDir" $setupScript

$installerPath = Join-Path $setupOutputDir ("SteplerSetup-{0}.exe" -f $productVersion)
if (-not (Test-Path $installerPath)) {
    throw "Installer not found after compilation: $installerPath"
}

Write-Host ""
Write-Host "Installer built successfully:" -ForegroundColor Green
Write-Host $installerPath
