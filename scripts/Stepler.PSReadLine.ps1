param(
    [string] $SteplerCli = (Join-Path $PSScriptRoot '..\target\debug\stepler-cli.exe'),
    [string] $PauseChord = 'Ctrl+F11',
    [string] $ScrollLockChord = 'Ctrl+F12',
    [string[]] $AdditionalPauseChords = @(),
    [string[]] $AdditionalScrollLockChords = @(),
    [switch] $Quiet
)

if (-not (Test-Path -LiteralPath $SteplerCli)) {
    throw "stepler-cli.exe not found at '$SteplerCli'. Build it first: cargo build -p stepler-cli"
}

if (-not ('Microsoft.PowerShell.PSConsoleReadLine' -as [type])) {
    throw "PSReadLine is not loaded. Import-Module PSReadLine and load this adapter from an interactive PowerShell session."
}

$script:SteplerCli = (Resolve-Path -LiteralPath $SteplerCli).Path
$script:SteplerPsReadLineEnabled = $false

function Resolve-SteplerPsReadLineChord {
    param(
        [Parameter(Mandatory)]
        [string] $Chord,

        [string] $FallbackChord
    )

    $keyName = ($Chord -split '\+')[-1]
    $parsed = $null
    if ([System.Enum]::TryParse([System.ConsoleKey], $keyName, $true, [ref] $parsed)) {
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

    if ([string]::IsNullOrWhiteSpace($line)) {
        return
    }

    $textBytes = [System.Text.Encoding]::Unicode.GetBytes($line)
    $textBase64 = [Convert]::ToBase64String($textBytes)
    $output = & $script:SteplerCli psreadline-plan --mode $Mode --text-b64 $textBase64 --cursor $cursor 2>$null

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

$script:SteplerPsReadLineEnabled = $true

function Get-SteplerPsReadLineStatus {
    [pscustomobject] @{
        SteplerCli = $script:SteplerCli
        Enabled = $script:SteplerPsReadLineEnabled
        PauseChords = $script:SteplerPauseChords -join ', '
        ScrollLockChords = $script:SteplerScrollLockChords -join ', '
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
    Remove-Item Function:\Resolve-SteplerPsReadLineChord -ErrorAction SilentlyContinue

    if (-not $KeepStatusCommand) {
        Remove-Item Function:\Get-SteplerPsReadLineStatus -ErrorAction SilentlyContinue
        Remove-Item Function:\Disable-SteplerPsReadLine -ErrorAction SilentlyContinue
    }
}

if (-not $Quiet) {
    Write-Host "Stepler PSReadLine adapter loaded: $($script:SteplerPauseChords -join ', ')=Pause mode, $($script:SteplerScrollLockChords -join ', ')=ScrollLock mode -> $script:SteplerCli"
}
