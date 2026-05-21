param(
    [string]$Configuration = "Release",
    [string]$Runtime = "win-x64"
)

$ErrorActionPreference = "Stop"

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Resolve-Path (Join-Path $scriptDir "..")
$setupScript = Join-Path $repoRoot "setup.iss"
$releaseScript = Join-Path $scriptDir "build-release.ps1"
$distDir = Join-Path $repoRoot "dist\Stepler"
$setupOutputDir = Join-Path $repoRoot "SetupOutput"

Set-Location $repoRoot

$isccCandidates = New-Object System.Collections.Generic.List[string]
$resolvedIscc = Get-Command iscc -ErrorAction SilentlyContinue
if ($resolvedIscc -and $resolvedIscc.Path) {
    $isccCandidates.Add($resolvedIscc.Path)
}

foreach ($candidate in @(
    "C:\Users\alexey.andreev\AppData\Local\Programs\Inno Setup 6\ISCC.exe",
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
& $releaseScript -Configuration $Configuration -Runtime $Runtime -DistDir $distDir

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
    Remove-Item -Force

Write-Host "Building installer for version $productVersion..." -ForegroundColor Cyan
& $isccPath "/DMyAppVersion=$productVersion" $setupScript

$installerPath = Join-Path $setupOutputDir ("SteplerSetup-{0}.exe" -f $productVersion)
if (-not (Test-Path $installerPath)) {
    throw "Installer not found after compilation: $installerPath"
}

Write-Host ""
Write-Host "Installer built successfully:" -ForegroundColor Green
Write-Host $installerPath
