param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]] $Arguments
)

$ErrorActionPreference = 'Stop'

$qwenCommand = Get-Command qwen.cmd, qwen.exe, qwen.ps1 -CommandType Application,ExternalScript -ErrorAction SilentlyContinue |
    Where-Object { $_.Source -notlike '*\Stepler.Qwen.ps1' } |
    Select-Object -First 1

if ($null -eq $qwenCommand) {
    throw "qwen was not found in PATH. Install Qwen CLI first or add it to PATH."
}

$previousTitle = $Host.UI.RawUI.WindowTitle
$stateDir = Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) 'Stepler\state'
$logDir = Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) 'Stepler\logs'
$markerPath = Join-Path $stateDir 'terminal-app-qwen.marker'
$inputFile = Join-Path $stateDir ("qwen-input-{0}.jsonl" -f $PID)
$jsonFile = Join-Path $logDir ("qwen-events-{0}.jsonl" -f $PID)
try {
    New-Item -ItemType Directory -Force -Path $stateDir | Out-Null
    New-Item -ItemType Directory -Force -Path $logDir | Out-Null
    New-Item -ItemType File -Force -Path $inputFile | Out-Null
    Clear-Content -LiteralPath $inputFile
    New-Item -ItemType File -Force -Path $jsonFile | Out-Null
    Clear-Content -LiteralPath $jsonFile
    Set-Content -LiteralPath $markerPath -Value @(
        "pid=$PID"
        "started=$([DateTimeOffset]::Now.ToString('o'))"
        "input_file=$inputFile"
        "json_file=$jsonFile"
    ) -Encoding UTF8
    $Host.UI.RawUI.WindowTitle = 'stepler-terminal-app qwen'
    $qwenArgs = @($Arguments)
    if ($qwenArgs -notcontains '--input-file') {
        $qwenArgs = @('--input-file', $inputFile) + $qwenArgs
    }
    if ($qwenArgs -notcontains '--json-file') {
        $qwenArgs = @('--json-file', $jsonFile) + $qwenArgs
    }
    & $qwenCommand.Source @qwenArgs
    exit $LASTEXITCODE
} finally {
    Remove-Item -LiteralPath $markerPath -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $inputFile -ErrorAction SilentlyContinue
    try {
        $Host.UI.RawUI.WindowTitle = $previousTitle
    } catch {
    }
}
