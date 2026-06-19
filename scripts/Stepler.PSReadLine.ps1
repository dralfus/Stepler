param(
    [string] $SteplerCli = $null,
    [string] $PauseChord = 'Pause',
    [string] $ScrollLockChord = 'Ctrl+Pause',
    [string[]] $AdditionalPauseChords = @('F13', 'Ctrl+F11'),
    [string[]] $AdditionalScrollLockChords = @('F14', 'Ctrl+F12'),
    [switch] $Quiet
)

if ([string]::IsNullOrWhiteSpace($SteplerCli)) {
    $distCli = Join-Path $PSScriptRoot '..\stepler-cli.exe'
    $debugCli = Join-Path $PSScriptRoot '..\target\debug\stepler-cli.exe'
    if (Test-Path -LiteralPath $distCli) {
        $SteplerCli = $distCli
    } else {
        $SteplerCli = $debugCli
    }
}

if (-not (Test-Path -LiteralPath $SteplerCli)) {
    throw "stepler-cli.exe not found at '$SteplerCli'. Build it first: cargo build -p stepler-cli"
}

if (-not ('Microsoft.PowerShell.PSConsoleReadLine' -as [type])) {
    throw "PSReadLine is not loaded. Import-Module PSReadLine and load this adapter from an interactive PowerShell session."
}

if (-not ('SteplerUser32' -as [type])) {
    Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class SteplerUser32 {
    [DllImport("user32.dll")]
    public static extern IntPtr GetForegroundWindow();
}
'@
}

$script:SteplerCli = (Resolve-Path -LiteralPath $SteplerCli).Path
$script:SteplerPsReadLineEnabled = $false
$script:SteplerTerminalAppWrapperNames = @()

function Resolve-SteplerPsReadLineChord {
    param(
        [Parameter(Mandatory)]
        [string] $Chord,

        [string] $FallbackChord
    )

    $keyName = ($Chord -split '\+')[-1]
    $knownKey = [System.Enum]::GetNames([System.ConsoleKey]) | Where-Object { $_ -ieq $keyName } | Select-Object -First 1
    if ($knownKey) {
        return $Chord
    }

    if ([string]::IsNullOrWhiteSpace($FallbackChord)) {
        Write-Warning "PSReadLine does not recognize key '$Chord'. Skipping this binding."
        return $null
    }

    Write-Warning "PSReadLine does not recognize key '$Chord'. Falling back to '$FallbackChord'."
    return $FallbackChord
}

function Invoke-SteplerPsReadLineCorrection {
    param(
        [Parameter(Mandatory)]
        [ValidateSet('pause', 'scrolllock')]
        [string] $Mode
    )

    $line = $null
    $cursor = 0
    [Microsoft.PowerShell.PSConsoleReadLine]::GetBufferState([ref] $line, [ref] $cursor)
    $selectionStart = 0
    $selectionLength = 0
    try {
        [Microsoft.PowerShell.PSConsoleReadLine]::GetSelectionState([ref] $selectionStart, [ref] $selectionLength)
    } catch {
        $selectionStart = 0
        $selectionLength = 0
    }

    if ([string]::IsNullOrWhiteSpace($line)) {
        return
    }

    $textBytes = [System.Text.Encoding]::Unicode.GetBytes($line)
    $textBase64 = [Convert]::ToBase64String($textBytes)
    $args = @('psreadline-plan', '--mode', $Mode, '--text-b64', $textBase64, '--cursor', $cursor)
    if ($selectionLength -gt 0) {
        $args += @('--selection-start', $selectionStart, '--selection-length', $selectionLength)
    }
    $output = & $script:SteplerCli @args 2>$null

    if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($output)) {
        return
    }

    try {
        $plan = $output | ConvertFrom-Json
    } catch {
        return
    }

    if (-not $plan.applied) {
        return
    }

    if ($plan.text_b64) {
        $nextLine = [System.Text.Encoding]::Unicode.GetString([Convert]::FromBase64String([string] $plan.text_b64))
    } else {
        $nextLine = [string] $plan.text
    }

    [Microsoft.PowerShell.PSConsoleReadLine]::RevertLine()
    [Microsoft.PowerShell.PSConsoleReadLine]::Insert($nextLine)
    try {
        [Microsoft.PowerShell.PSConsoleReadLine]::SetCursorPosition([int] $plan.cursor)
    } catch {
        # Older PSReadLine versions may not expose SetCursorPosition. In that case,
        # leaving the cursor at the end is safer than mutating the terminal buffer.
    }

    $replacementText = if ($null -ne $plan.replacement_text) {
        [string] $plan.replacement_text
    } else {
        [string] $plan.replacement
    }

    $targetLayout = Get-SteplerTargetLayout -Text $replacementText
    if ($targetLayout) {
        Invoke-SteplerLayoutSwitch -TargetLayout $targetLayout -TargetHwnd ([SteplerUser32]::GetForegroundWindow())
    }
}

function Invoke-SteplerLayoutSwitch {
    param(
        [Parameter(Mandatory)]
        [ValidateSet('russian', 'english')]
        [string] $TargetLayout,

        [Parameter(Mandatory)]
        [IntPtr] $TargetHwnd
    )

    $hwndValue = $TargetHwnd.ToInt64().ToString()
    $psResult = Set-SteplerPowerShellInputLanguage -TargetLayout $TargetLayout
    Write-SteplerPsReadLineLog -Message "layout ps target=$TargetLayout hwnd=$hwndValue result=$psResult"

    $controlOutput = & $script:SteplerCli trigger-layout-control $TargetLayout 2>&1
    $controlExitCode = $LASTEXITCODE
    Write-SteplerPsReadLineLog -Message "layout control target=$TargetLayout hwnd=$hwndValue exit=$controlExitCode output=$controlOutput"

    $syncOutput = & $script:SteplerCli switch-layout $TargetLayout --hwnd $hwndValue 2>&1
    $syncExitCode = $LASTEXITCODE
    Write-SteplerPsReadLineLog -Message "layout sync target=$TargetLayout hwnd=$hwndValue exit=$syncExitCode output=$syncOutput"

    $encodedCli = [Convert]::ToBase64String([System.Text.Encoding]::Unicode.GetBytes($script:SteplerCli))
    $encodedLayout = [Convert]::ToBase64String([System.Text.Encoding]::Unicode.GetBytes($TargetLayout))
    $encodedHwnd = [Convert]::ToBase64String([System.Text.Encoding]::Unicode.GetBytes($hwndValue))
    $script = @"
`$cli = [System.Text.Encoding]::Unicode.GetString([Convert]::FromBase64String('$encodedCli'))
`$layout = [System.Text.Encoding]::Unicode.GetString([Convert]::FromBase64String('$encodedLayout'))
`$hwnd = [System.Text.Encoding]::Unicode.GetString([Convert]::FromBase64String('$encodedHwnd'))
Start-Sleep -Milliseconds 120
& `$cli trigger-layout-control `$layout 2>`$null | Out-Null
& `$cli switch-layout `$layout --hwnd `$hwnd 2>`$null | Out-Null
`$exit1 = `$LASTEXITCODE
Start-Sleep -Milliseconds 260
& `$cli trigger-layout-control `$layout 2>`$null | Out-Null
& `$cli switch-layout `$layout --hwnd `$hwnd 2>`$null | Out-Null
`$exit2 = `$LASTEXITCODE
`$log = Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) 'Stepler\logs\psreadline_layout.log'
`$dir = Split-Path -Parent `$log
New-Item -ItemType Directory -Force -Path `$dir | Out-Null
Add-Content -LiteralPath `$log -Value ("{0:o} layout delayed target={1} hwnd={2} exit1={3} exit2={4}" -f [DateTimeOffset]::Now, `$layout, `$hwnd, `$exit1, `$exit2)
"@
    Start-Process -FilePath powershell.exe -WindowStyle Hidden -ArgumentList @(
        '-NoLogo',
        '-NoProfile',
        '-ExecutionPolicy',
        'Bypass',
        '-Command',
        $script
    ) | Out-Null
}

function Set-SteplerPowerShellInputLanguage {
    param(
        [Parameter(Mandatory)]
        [ValidateSet('russian', 'english')]
        [string] $TargetLayout
    )

    try {
        Add-Type -AssemblyName System.Windows.Forms -ErrorAction Stop
        $targetPrefix = if ($TargetLayout -eq 'russian') { 'ru' } else { 'en' }
        $target = [System.Windows.Forms.InputLanguage]::InstalledInputLanguages |
            Where-Object { $_.Culture.TwoLetterISOLanguageName -ieq $targetPrefix } |
            Select-Object -First 1

        if ($null -eq $target) {
            return 'missing'
        }

        $before = [System.Windows.Forms.InputLanguage]::CurrentInputLanguage
        [System.Windows.Forms.InputLanguage]::CurrentInputLanguage = $target
        Start-Sleep -Milliseconds 15
        $after = [System.Windows.Forms.InputLanguage]::CurrentInputLanguage
        return ("before={0}/{1}; after={2}/{3}; target={4}/{5}" -f
            $before.Culture.Name,
            $before.Handle.ToInt64(),
            $after.Culture.Name,
            $after.Handle.ToInt64(),
            $target.Culture.Name,
            $target.Handle.ToInt64())
    } catch {
        return "error=$($_.Exception.Message)"
    }
}

function Write-SteplerPsReadLineLog {
    param(
        [Parameter(Mandatory)]
        [string] $Message
    )

    try {
        $log = Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) 'Stepler\logs\psreadline_layout.log'
        $dir = Split-Path -Parent $log
        New-Item -ItemType Directory -Force -Path $dir | Out-Null
        Add-Content -LiteralPath $log -Value ("{0:o} {1}" -f [DateTimeOffset]::Now, $Message)
    } catch {
    }
}

function Get-SteplerTargetLayout {
    param(
        [AllowNull()]
        [string] $Text
    )

    if ([string]::IsNullOrEmpty($Text)) {
        return $null
    }

    $russianCount = ([regex]::Matches($Text, '\p{IsCyrillic}')).Count
    $englishCount = ([regex]::Matches($Text, '[A-Za-z]')).Count

    if ($russianCount -gt $englishCount) {
        return 'russian'
    }
    if ($englishCount -gt $russianCount) {
        return 'english'
    }

    return $null
}

function Invoke-SteplerTerminalAppCommand {
    param(
        [Parameter(Mandatory)]
        [string] $Name,

        [Parameter(Mandatory)]
        [string] $WindowTitle,

        [AllowNull()]
        [string[]] $Arguments
    )

    $command = Get-Command $Name -CommandType Application,ExternalScript -All -ErrorAction SilentlyContinue |
        Select-Object -First 1
    if ($null -eq $command) {
        throw "External command '$Name' was not found."
    }

    $previousTitle = $Host.UI.RawUI.WindowTitle
    $stateDir = Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) 'Stepler\state'
    $logDir = Join-Path ([Environment]::GetFolderPath('LocalApplicationData')) 'Stepler\logs'
    $markerPath = Join-Path $stateDir "terminal-app-$Name.marker"
    $inputFile = Join-Path $stateDir ("{0}-input-{1}.jsonl" -f $Name, $PID)
    $jsonFile = Join-Path $logDir ("{0}-events-{1}.jsonl" -f $Name, $PID)
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
        $Host.UI.RawUI.WindowTitle = $WindowTitle
        $commandArgs = @($Arguments)
        if ($Name -eq 'qwen' -and $commandArgs -notcontains '--input-file') {
            $commandArgs = @('--input-file', $inputFile) + $commandArgs
        }
        if ($Name -eq 'qwen' -and $commandArgs -notcontains '--json-file') {
            $commandArgs = @('--json-file', $jsonFile) + $commandArgs
        }
        & $command.Source @commandArgs
    } finally {
        Remove-Item -LiteralPath $markerPath -ErrorAction SilentlyContinue
        Remove-Item -LiteralPath $inputFile -ErrorAction SilentlyContinue
        try {
            $Host.UI.RawUI.WindowTitle = $previousTitle
        } catch {
        }
    }
}

function Register-SteplerTerminalAppWrapper {
    param(
        [Parameter(Mandatory)]
        [string] $Name,

        [Parameter(Mandatory)]
        [string] $WindowTitle
    )

    $command = Get-Command $Name -CommandType Application,ExternalScript -All -ErrorAction SilentlyContinue |
        Select-Object -First 1
    if ($null -eq $command) {
        return
    }

    $functionName = "global:$Name"
    $scriptBlock = {
        param(
            [Parameter(ValueFromRemainingArguments = $true)]
            [string[]] $Arguments
        )

        Invoke-SteplerTerminalAppCommand -Name '__STEPLER_TERMINAL_APP_NAME__' -WindowTitle '__STEPLER_TERMINAL_APP_TITLE__' -Arguments $Arguments
    }.ToString().
        Replace('__STEPLER_TERMINAL_APP_NAME__', $Name).
        Replace('__STEPLER_TERMINAL_APP_TITLE__', $WindowTitle)

    Set-Item -Path "Function:\$functionName" -Value ([scriptblock]::Create($scriptBlock)) -Force
    if ($script:SteplerTerminalAppWrapperNames -notcontains $Name) {
        $script:SteplerTerminalAppWrapperNames += $Name
    }
}

$script:SteplerPauseChord = Resolve-SteplerPsReadLineChord -Chord $PauseChord -FallbackChord 'Ctrl+F11'
$script:SteplerPauseChords = @($script:SteplerPauseChord)
$script:SteplerScrollLockChord = Resolve-SteplerPsReadLineChord -Chord $ScrollLockChord -FallbackChord 'Ctrl+F12'
$script:SteplerScrollLockChords = @($script:SteplerScrollLockChord)

foreach ($chord in $AdditionalPauseChords) {
    $resolvedChord = Resolve-SteplerPsReadLineChord -Chord $chord -FallbackChord $null
    if (-not [string]::IsNullOrWhiteSpace($resolvedChord) -and $script:SteplerPauseChords -notcontains $resolvedChord) {
        $script:SteplerPauseChords += $resolvedChord
    }
}

foreach ($chord in $AdditionalScrollLockChords) {
    $resolvedChord = Resolve-SteplerPsReadLineChord -Chord $chord -FallbackChord $null
    if (-not [string]::IsNullOrWhiteSpace($resolvedChord) -and $script:SteplerScrollLockChords -notcontains $resolvedChord) {
        $script:SteplerScrollLockChords += $resolvedChord
    }
}

foreach ($chord in $script:SteplerPauseChords) {
    Set-PSReadLineKeyHandler -Chord $chord -BriefDescription SteplerPause -Description 'Stepler: fix the word or selection before the cursor' -ScriptBlock {
        Invoke-SteplerPsReadLineCorrection -Mode pause
    }
}

foreach ($chord in $script:SteplerScrollLockChords) {
    Set-PSReadLineKeyHandler -Chord $chord -BriefDescription SteplerScrollLock -Description 'Stepler: fix mistyped layout in the current PowerShell input' -ScriptBlock {
        Invoke-SteplerPsReadLineCorrection -Mode scrolllock
    }
}

Register-SteplerTerminalAppWrapper -Name 'qwen' -WindowTitle 'stepler-terminal-app qwen'

$script:SteplerPsReadLineEnabled = $true

function Get-SteplerPsReadLineStatus {
    [pscustomobject] @{
        SteplerCli = $script:SteplerCli
        Enabled = $script:SteplerPsReadLineEnabled
        PauseChords = $script:SteplerPauseChords -join ', '
        ScrollLockChords = $script:SteplerScrollLockChords -join ', '
        TerminalAppWrappers = $script:SteplerTerminalAppWrapperNames -join ', '
    }
}

function Disable-SteplerPsReadLine {
    param(
        [switch] $KeepStatusCommand
    )

    $chords = @()
    if ($script:SteplerPauseChords) {
        $chords += $script:SteplerPauseChords
    }
    if ($script:SteplerScrollLockChords) {
        $chords += $script:SteplerScrollLockChords
    }
    $chords = $chords | Where-Object { -not [string]::IsNullOrWhiteSpace($_) } | Select-Object -Unique

    if ($chords.Count -gt 0) {
        Remove-PSReadLineKeyHandler -Chord $chords -ErrorAction SilentlyContinue
    }

    $script:SteplerPsReadLineEnabled = $false

    Remove-Item Function:\Invoke-SteplerPsReadLineCorrection -ErrorAction SilentlyContinue
    Remove-Item Function:\Invoke-SteplerLayoutSwitch -ErrorAction SilentlyContinue
    Remove-Item Function:\Set-SteplerPowerShellInputLanguage -ErrorAction SilentlyContinue
    Remove-Item Function:\Write-SteplerPsReadLineLog -ErrorAction SilentlyContinue
    Remove-Item Function:\Get-SteplerTargetLayout -ErrorAction SilentlyContinue
    Remove-Item Function:\Invoke-SteplerTerminalAppCommand -ErrorAction SilentlyContinue
    Remove-Item Function:\Register-SteplerTerminalAppWrapper -ErrorAction SilentlyContinue
    foreach ($wrapperName in $script:SteplerTerminalAppWrapperNames) {
        Remove-Item "Function:\$wrapperName" -ErrorAction SilentlyContinue
    }
    Remove-Item Function:\Resolve-SteplerPsReadLineChord -ErrorAction SilentlyContinue

    if (-not $KeepStatusCommand) {
        Remove-Item Function:\Get-SteplerPsReadLineStatus -ErrorAction SilentlyContinue
        Remove-Item Function:\Disable-SteplerPsReadLine -ErrorAction SilentlyContinue
    }
}

if (-not $Quiet) {
    Write-Host "Stepler PSReadLine adapter loaded: $($script:SteplerPauseChords -join ', ')=Pause mode, $($script:SteplerScrollLockChords -join ', ')=ScrollLock mode -> $script:SteplerCli"
}
