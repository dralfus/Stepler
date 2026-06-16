param(
    [string]$DistDir = "",
    [string]$WslDistro = ""
)

$ErrorActionPreference = "Stop"

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Resolve-Path (Join-Path $scriptDir "..")

if ([string]::IsNullOrWhiteSpace($DistDir)) {
    $DistDir = Join-Path $repoRoot "dist\Stepler\remote\linux-x64"
}

function ConvertTo-BashSingleQuoted([string]$Value) {
    return "'" + $Value.Replace("'", "'\''") + "'"
}

$wslArgs = @()
if (-not [string]::IsNullOrWhiteSpace($WslDistro)) {
    $wslArgs += @("-d", $WslDistro)
}

$wslRepoRoot = (& wsl.exe @wslArgs wslpath -a "$repoRoot").Trim()
if ([string]::IsNullOrWhiteSpace($wslRepoRoot)) {
    throw "Cannot resolve repository path inside WSL."
}

$quotedRepo = ConvertTo-BashSingleQuoted $wslRepoRoot
$buildCommand = "cd $quotedRepo && cargo build --release -p stepler-remote"

Write-Host "Building stepler-remote in WSL..."
& wsl.exe @wslArgs bash -lc $buildCommand
if ($LASTEXITCODE -ne 0) {
    throw "WSL build failed. Install Rust in WSL, not on the remote VPS: curl https://sh.rustup.rs -sSf | sh"
}

$sourceBinary = Join-Path $repoRoot "target\release\stepler-remote"
if (-not (Test-Path $sourceBinary)) {
    throw "Linux binary was not found at '$sourceBinary'."
}

$distPath = [System.IO.Path]::GetFullPath($DistDir)
New-Item -ItemType Directory -Force -Path $distPath | Out-Null
Copy-Item $sourceBinary (Join-Path $distPath "stepler-remote") -Force
Copy-Item (Join-Path $repoRoot "scripts\Stepler.SSHReadline.bash") `
    (Join-Path $distPath "Stepler.SSHReadline.bash") -Force

$installText = @'
Stepler SSH remote helper

Copy these files to the remote Linux host:

  mkdir -p ~/.local/bin ~/.config/stepler
  cp stepler-remote ~/.local/bin/
  cp Stepler.SSHReadline.bash ~/.config/stepler/
  chmod +x ~/.local/bin/stepler-remote
  grep -qxF 'source ~/.config/stepler/Stepler.SSHReadline.bash' ~/.bashrc || echo 'source ~/.config/stepler/Stepler.SSHReadline.bash' >> ~/.bashrc

On the Windows client, enable SSH forwarding before starting Stepler:

  setx STEPLER_ENABLE_SSH_REMOTE_ADAPTER 1

Then restart Stepler and open a new SSH session.
'@

Set-Content -Path (Join-Path $distPath "README_REMOTE_LINUX.txt") -Value $installText -Encoding UTF8

Write-Host ""
Write-Host "Remote Linux helper output:"
Write-Host "  $distPath"
Get-ChildItem $distPath | Select-Object Name,Length,LastWriteTime | Format-Table -AutoSize
