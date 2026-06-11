// Generated-in-place module split: embedded PowerShell scripts used by Windows method adapters.
pub(super) const WORD_CAPTURE_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
function ConvertTo-B64([string] $Text) {
    [Convert]::ToBase64String([System.Text.Encoding]::Unicode.GetBytes($Text))
}
function Strip-WordRangeMarkers([string] $Text) {
    if ($null -eq $Text) { return '' }
    $Text.TrimEnd([char]13, [char]7)
}
$word = [Runtime.InteropServices.Marshal]::GetActiveObject('Word.Application')
$word.Activate()
try {
    $selection = $word.ActiveWindow.Selection
} catch {
    $selection = $word.Selection
}
$document = $word.ActiveDocument
$selectionStart = [int] $selection.Start
$selectionEnd = [int] $selection.End

if ($selectionStart -ne $selectionEnd) {
    $range = $document.Range($selectionStart, $selectionEnd)
    $text = Strip-WordRangeMarkers ([string] $range.Text)
    'ok=1'
    'kind=selection'
    'base=' + $selectionStart
    'text_b64=' + (ConvertTo-B64 $text)
    exit 0
}

$paragraphRange = $selection.Paragraphs.Item(1).Range
$paragraphStart = [int] $paragraphRange.Start
if ($selectionStart -le $paragraphStart) {
    'ok=0'
    'error=empty'
    exit 0
}

$leftRange = $document.Range($paragraphStart, $selectionStart)
$text = Strip-WordRangeMarkers ([string] $leftRange.Text)
$base = $paragraphStart
'ok=1'
'kind=paragraph_left'
'base=' + $base
'selection_start=' + $selectionStart
'paragraph_start=' + $paragraphStart
'text_b64=' + (ConvertTo-B64 $text)
"#;

#[cfg(windows)]
pub(super) const WORD_APPLY_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
function From-B64([string] $Text) {
    [System.Text.Encoding]::Unicode.GetString([Convert]::FromBase64String($Text))
}
function Strip-WordRangeMarkers([string] $Text) {
    if ($null -eq $Text) { return '' }
    $Text.TrimEnd([char]13, [char]7)
}
function ConvertTo-B64([string] $Text) {
    [Convert]::ToBase64String([System.Text.Encoding]::Unicode.GetBytes($Text))
}
$start = [int] $env:STEPLER_WORD_START
$end = [int] $env:STEPLER_WORD_END
$targetCaret = [int] $env:STEPLER_WORD_CARET
$expected = From-B64 $env:STEPLER_WORD_EXPECTED_B64
$replacement = From-B64 $env:STEPLER_WORD_REPLACEMENT_B64
$word = [Runtime.InteropServices.Marshal]::GetActiveObject('Word.Application')
try { $word.Activate() } catch { }
$document = $word.ActiveDocument
$range = $document.Range($start, $end)
$actual = Strip-WordRangeMarkers ([string] $range.Text)
if ($actual -ne $expected) {
    'ok=0'
    'error=preflight'
    exit 0
}
$rightBefore = ''
try {
    $rightBefore = Strip-WordRangeMarkers ([string] $document.Range($end, $end + 1).Text)
} catch { }
$range.Text = $replacement
$caret = $targetCaret
$word.Selection.SetRange($caret, $caret)
Start-Sleep -Milliseconds 140
$rightAfter = ''
try {
    $rightAfter = Strip-WordRangeMarkers ([string] $document.Range($caret, $caret + 1).Text)
} catch { }
if ($rightAfter -eq 'с' -and $rightBefore -ne 'с') {
    try {
        $document.Range($caret, $caret + 1).Delete() | Out-Null
        $word.Selection.SetRange($caret, $caret)
    } catch { }
}
$afterEnd = $caret
try {
    $afterEnd = [Math]::Min($document.Content.End, $caret + 1)
} catch { }
$after = ''
try {
    $after = Strip-WordRangeMarkers ([string] $document.Range($start, $afterEnd).Text)
} catch { }
'ok=1'
'after_b64=' + (ConvertTo-B64 $after)
"#;

#[cfg(windows)]
pub(super) const OUTLOOK_WORD_CAPTURE_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
function ConvertTo-B64([string] $Text) {
    [Convert]::ToBase64String([System.Text.Encoding]::Unicode.GetBytes($Text))
}
function Strip-WordRangeMarkers([string] $Text) {
    if ($null -eq $Text) { return '' }
    $Text.TrimEnd([char]13, [char]7)
}
$outlook = [Runtime.InteropServices.Marshal]::GetActiveObject('Outlook.Application')
$inspector = $outlook.ActiveInspector()
if ($null -eq $inspector) {
    'ok=0'
    'error=no_active_inspector'
    exit 0
}
$inspector.Activate()
$document = $inspector.WordEditor
if ($null -eq $document) {
    'ok=0'
    'error=no_word_editor'
    exit 0
}
$word = $document.Application
$selection = $word.Selection
$selectionStart = [int] $selection.Start
$selectionEnd = [int] $selection.End

if ($selectionStart -ne $selectionEnd) {
    $range = $document.Range($selectionStart, $selectionEnd)
    $text = Strip-WordRangeMarkers ([string] $range.Text)
    'ok=1'
    'kind=selection'
    'base=' + $selectionStart
    'text_b64=' + (ConvertTo-B64 $text)
    exit 0
}

$paragraphRange = $selection.Paragraphs.Item(1).Range
$paragraphStart = [int] $paragraphRange.Start
if ($selectionStart -le $paragraphStart) {
    'ok=0'
    'error=empty'
    exit 0
}

$leftRange = $document.Range($paragraphStart, $selectionStart)
$text = Strip-WordRangeMarkers ([string] $leftRange.Text)
$base = $paragraphStart
'ok=1'
'kind=paragraph_left'
'base=' + $base
'selection_start=' + $selectionStart
'paragraph_start=' + $paragraphStart
'text_b64=' + (ConvertTo-B64 $text)
"#;

#[cfg(windows)]
pub(super) const OUTLOOK_WORD_APPLY_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
function From-B64([string] $Text) {
    [System.Text.Encoding]::Unicode.GetString([Convert]::FromBase64String($Text))
}
function Strip-WordRangeMarkers([string] $Text) {
    if ($null -eq $Text) { return '' }
    $Text.TrimEnd([char]13, [char]7)
}
function ConvertTo-B64([string] $Text) {
    [Convert]::ToBase64String([System.Text.Encoding]::Unicode.GetBytes($Text))
}
$start = [int] $env:STEPLER_WORD_START
$end = [int] $env:STEPLER_WORD_END
$targetCaret = [int] $env:STEPLER_WORD_CARET
$expected = From-B64 $env:STEPLER_WORD_EXPECTED_B64
$replacement = From-B64 $env:STEPLER_WORD_REPLACEMENT_B64
$outlook = [Runtime.InteropServices.Marshal]::GetActiveObject('Outlook.Application')
$inspector = $outlook.ActiveInspector()
if ($null -eq $inspector) {
    'ok=0'
    'error=no_active_inspector'
    exit 0
}
$inspector.Activate()
$document = $inspector.WordEditor
if ($null -eq $document) {
    'ok=0'
    'error=no_word_editor'
    exit 0
}
$word = $document.Application
$range = $document.Range($start, $end)
$actual = Strip-WordRangeMarkers ([string] $range.Text)
if ($actual -ne $expected) {
    'ok=0'
    'error=preflight'
    exit 0
}
$rightBefore = ''
try {
    $rightBefore = Strip-WordRangeMarkers ([string] $document.Range($end, $end + 1).Text)
} catch { }
$range.Text = $replacement
$caret = $targetCaret
$word.Selection.SetRange($caret, $caret)
Start-Sleep -Milliseconds 140
$rightAfter = ''
try {
    $rightAfter = Strip-WordRangeMarkers ([string] $document.Range($caret, $caret + 1).Text)
} catch { }
if ($rightAfter -eq 'с' -and $rightBefore -ne 'с') {
    try {
        $document.Range($caret, $caret + 1).Delete() | Out-Null
        $word.Selection.SetRange($caret, $caret)
    } catch { }
}
$afterEnd = $caret
try {
    $afterEnd = [Math]::Min($document.Content.End, $caret + 1)
} catch { }
$after = ''
try {
    $after = Strip-WordRangeMarkers ([string] $document.Range($start, $afterEnd).Text)
} catch { }
'ok=1'
'after_b64=' + (ConvertTo-B64 $after)
"#;

#[cfg(windows)]
pub(super) const UIA_CAPTURE_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class SteplerUser32 {
    [DllImport("user32.dll")]
    public static extern IntPtr GetForegroundWindow();
}
'@
function Get-SteplerForegroundHandle {
    if (-not [string]::IsNullOrWhiteSpace($env:STEPLER_UIA_FOREGROUND_HWND)) {
        return [IntPtr]([Int64]::Parse($env:STEPLER_UIA_FOREGROUND_HWND))
    }
    [SteplerUser32]::GetForegroundWindow()
}
function ConvertTo-B64([string] $Text) {
    if ($null -eq $Text) { $Text = '' }
    [Convert]::ToBase64String([System.Text.Encoding]::Unicode.GetBytes($Text))
}
function Get-Pattern($Element, $Pattern) {
    try { return $Element.GetCurrentPattern($Pattern) } catch { return $null }
}
function Normalize-Text([string] $Text) {
    if ($null -eq $Text) { return '' }
    $Text.TrimEnd([char]13)
}
function Get-ValuePattern($Element) {
    if ($null -eq $Element) { return $null }
    Get-Pattern $Element ([System.Windows.Automation.ValuePattern]::Pattern)
}
function Is-WritableValueElement($Element) {
    $value = Get-ValuePattern $Element
    $null -ne $value -and -not $value.Current.IsReadOnly
}
function Find-WritableValueElement {
    $focused = [System.Windows.Automation.AutomationElement]::FocusedElement
    if (Is-WritableValueElement $focused) {
        return $focused
    }
    $foreground = [System.Windows.Automation.AutomationElement]::FromHandle((Get-SteplerForegroundHandle))
    if ($null -eq $foreground) { return $focused }
    $fixtureCondition = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::AutomationIdProperty,
        'SteplerUiaFixtureInput')
    $fixture = $foreground.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $fixtureCondition)
    if (Is-WritableValueElement $fixture) {
        return $fixture
    }
    $editCondition = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
        [System.Windows.Automation.ControlType]::Edit)
    $edits = $foreground.FindAll([System.Windows.Automation.TreeScope]::Descendants, $editCondition)
    foreach ($edit in $edits) {
        try {
            if ($edit.Current.HasKeyboardFocus -and (Is-WritableValueElement $edit)) {
                return $edit
            }
        } catch { }
    }
    return $focused
}
$element = Find-WritableValueElement
if ($null -eq $element) {
    'ok=0'
    exit 0
}
$strictEditable = $env:STEPLER_UIA_STRICT_EDITABLE -eq '1'
if ($strictEditable) {
    try {
        if (-not $element.Current.HasKeyboardFocus) {
            'ok=0'
            'error=no_keyboard_focus'
            exit 0
        }
        if (-not $element.Current.IsKeyboardFocusable) {
            'ok=0'
            'error=not_keyboard_focusable'
            exit 0
        }
        if ($element.Current.ControlType.ProgrammaticName -ne 'ControlType.Edit') {
            'ok=0'
            'error=not_edit_control'
            exit 0
        }
    } catch {
        'ok=0'
        'error=strict_metadata'
        exit 0
    }
}
$runtimeId = ($element.GetRuntimeId() -join '.')
$valuePattern = Get-Pattern $element ([System.Windows.Automation.ValuePattern]::Pattern)
$textPattern = Get-Pattern $element ([System.Windows.Automation.TextPattern]::Pattern)
$canSetValue = 0
$text = ''
if ($null -ne $valuePattern) {
    $text = [string]$valuePattern.Current.Value
    if (-not $valuePattern.Current.IsReadOnly) {
        $canSetValue = 1
    }
} elseif ($null -ne $textPattern) {
    $text = $textPattern.DocumentRange.GetText(-1)
}
$text = Normalize-Text $text
if ($text.Length -eq 0) {
    'ok=0'
    exit 0
}
if ($strictEditable) {
    if ($canSetValue -ne 1) {
        'ok=0'
        'error=no_writable_value'
        exit 0
    }
    if ($null -eq $textPattern) {
        'ok=0'
        'error=no_text_pattern'
        exit 0
    }
    if ($text.Length -gt 20000) {
        'ok=0'
        'error=text_too_large'
        exit 0
    }
    $newlineCount = ([regex]::Matches($text, "`n")).Count
    if ($newlineCount -gt 200) {
        'ok=0'
        'error=too_many_lines'
        exit 0
    }
}
$caret = $text.Length
$selectionStart = $caret
$selectionEnd = $caret
if ($null -ne $textPattern) {
    try {
        $selection = $textPattern.GetSelection()
        if ($null -ne $selection -and $selection.Length -gt 0) {
            $range = $selection[0]
            $document = $textPattern.DocumentRange
            $beforeStart = $document.Clone()
            $null = $beforeStart.MoveEndpointByRange(
                [System.Windows.Automation.Text.TextPatternRangeEndpoint]::End,
                $range,
                [System.Windows.Automation.Text.TextPatternRangeEndpoint]::Start)
            $beforeEnd = $document.Clone()
            $null = $beforeEnd.MoveEndpointByRange(
                [System.Windows.Automation.Text.TextPatternRangeEndpoint]::End,
                $range,
                [System.Windows.Automation.Text.TextPatternRangeEndpoint]::End)
            $selectionStart = (Normalize-Text $beforeStart.GetText(-1)).Length
            $selectionEnd = (Normalize-Text $beforeEnd.GetText(-1)).Length
            $caret = $selectionEnd
        }
    } catch {
        $caret = $text.Length
        $selectionStart = $caret
        $selectionEnd = $caret
    }
}
'ok=1'
'runtime_id=' + $runtimeId
'can_set_value=' + $canSetValue
'caret=' + $caret
'selection_start=' + $selectionStart
'selection_end=' + $selectionEnd
'text_b64=' + (ConvertTo-B64 $text)
"#;

#[cfg(windows)]
pub(super) const UIA_FOCUS_DIAGNOSTICS_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
function Escape-Line([string] $Text) {
    if ($null -eq $Text) { return '' }
    $Text.Replace("`r", '\r').Replace("`n", '\n')
}
$element = [System.Windows.Automation.AutomationElement]::FocusedElement
if ($null -eq $element) {
    'ok=0'
    exit 0
}
'ok=1'
'name=' + (Escape-Line ([string]$element.Current.Name))
'control_type=' + ([string]$element.Current.ControlType.ProgrammaticName)
'automation_id=' + (Escape-Line ([string]$element.Current.AutomationId))
'class_name=' + (Escape-Line ([string]$element.Current.ClassName))
'framework_id=' + (Escape-Line ([string]$element.Current.FrameworkId))
'has_keyboard_focus=' + ($(if ($element.Current.HasKeyboardFocus) { '1' } else { '0' }))
'is_keyboard_focusable=' + ($(if ($element.Current.IsKeyboardFocusable) { '1' } else { '0' }))
"#;

#[cfg(windows)]
pub(super) const UIA_APPLY_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class SteplerUser32 {
    [DllImport("user32.dll")]
    public static extern IntPtr GetForegroundWindow();
}
'@
function Get-SteplerForegroundHandle {
    if (-not [string]::IsNullOrWhiteSpace($env:STEPLER_UIA_FOREGROUND_HWND)) {
        return [IntPtr]([Int64]::Parse($env:STEPLER_UIA_FOREGROUND_HWND))
    }
    [SteplerUser32]::GetForegroundWindow()
}
function ConvertTo-B64([string] $Text) {
    if ($null -eq $Text) { $Text = '' }
    [Convert]::ToBase64String([System.Text.Encoding]::Unicode.GetBytes($Text))
}
function ConvertFrom-B64([string] $Text) {
    [System.Text.Encoding]::Unicode.GetString([Convert]::FromBase64String($Text))
}
function Get-Pattern($Element, $Pattern) {
    try { return $Element.GetCurrentPattern($Pattern) } catch { return $null }
}
function Get-CaretRange($Element) {
    $textPattern2 = Get-Pattern $Element ([System.Windows.Automation.TextPattern2]::Pattern)
    if ($null -eq $textPattern2) { return $null }
    try {
        $isActive = $false
        return $textPattern2.GetCaretRange([ref]$isActive)
    } catch {
        return $null
    }
}
function Runtime-Id($Element) {
    if ($null -eq $Element) { return '' }
    try { return ($Element.GetRuntimeId() -join '.') } catch { return '' }
}
function Find-ElementByRuntimeId([string] $RuntimeId) {
    $focused = [System.Windows.Automation.AutomationElement]::FocusedElement
    if ((Runtime-Id $focused) -eq $RuntimeId) {
        return $focused
    }
    $foreground = [System.Windows.Automation.AutomationElement]::FromHandle((Get-SteplerForegroundHandle))
    if ($null -eq $foreground) { return $focused }
    $condition = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
        [System.Windows.Automation.ControlType]::Edit)
    $all = $foreground.FindAll([System.Windows.Automation.TreeScope]::Descendants, $condition)
    foreach ($candidate in $all) {
        if ((Runtime-Id $candidate) -eq $RuntimeId) {
            return $candidate
        }
    }
    return $focused
}
$element = Find-ElementByRuntimeId $env:STEPLER_UIA_RUNTIME_ID
if ($null -eq $element) {
    'ok=0'
    exit 0
}
if ((Runtime-Id $element) -ne $env:STEPLER_UIA_RUNTIME_ID) {
    'ok=0'
    exit 0
}
$valuePattern = Get-Pattern $element ([System.Windows.Automation.ValuePattern]::Pattern)
if ($null -eq $valuePattern -or $valuePattern.Current.IsReadOnly) {
    'ok=0'
    exit 0
}
$expected = ConvertFrom-B64 $env:STEPLER_UIA_EXPECTED_B64
$replacement = ConvertFrom-B64 $env:STEPLER_UIA_REPLACEMENT_B64
if ([string]$valuePattern.Current.Value -ne $expected) {
    'ok=0'
    exit 0
}
$valuePattern.SetValue($replacement)
Start-Sleep -Milliseconds 30
$caret = 0
if (-not [string]::IsNullOrWhiteSpace($env:STEPLER_UIA_CARET_UTF16)) {
    $caret = [int]$env:STEPLER_UIA_CARET_UTF16
}
$textPattern = Get-Pattern $element ([System.Windows.Automation.TextPattern]::Pattern)
if ($null -ne $textPattern) {
    try {
        $range = $textPattern.DocumentRange.Clone()
        $null = $range.MoveEndpointByUnit(
            [System.Windows.Automation.Text.TextPatternRangeEndpoint]::Start,
            [System.Windows.Automation.Text.TextUnit]::Character,
            $caret)
        $null = $range.MoveEndpointByRange(
            [System.Windows.Automation.Text.TextPatternRangeEndpoint]::End,
            $range,
            [System.Windows.Automation.Text.TextPatternRangeEndpoint]::Start)
        $range.Select()
    } catch { }
}
'ok=1'
'after_b64=' + (ConvertTo-B64 ([string]$valuePattern.Current.Value))
"#;

#[cfg(windows)]
pub(super) const UIA_DOCUMENT_CAPTURE_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class SteplerUser32 {
    [DllImport("user32.dll")]
    public static extern IntPtr GetForegroundWindow();
}
'@
function Get-SteplerForegroundHandle {
    if (-not [string]::IsNullOrWhiteSpace($env:STEPLER_UIA_FOREGROUND_HWND)) {
        return [IntPtr]([Int64]::Parse($env:STEPLER_UIA_FOREGROUND_HWND))
    }
    [SteplerUser32]::GetForegroundWindow()
}
function ConvertTo-B64([string] $Text) {
    if ($null -eq $Text) { $Text = '' }
    [Convert]::ToBase64String([System.Text.Encoding]::Unicode.GetBytes($Text))
}
function Normalize-Text([string] $Text) {
    if ($null -eq $Text) { return '' }
    $Text.TrimEnd([char]13)
}
function Get-Pattern($Element, $Pattern) {
    try { return $Element.GetCurrentPattern($Pattern) } catch { return $null }
}
function Runtime-Id($Element) {
    if ($null -eq $Element) { return '' }
    try { return ($Element.GetRuntimeId() -join '.') } catch { return '' }
}
function Has-TextPattern($Element) {
    $null -ne (Get-Pattern $Element ([System.Windows.Automation.TextPattern]::Pattern))
}
function Get-CaretRange($Element) {
    $textPattern2 = Get-Pattern $Element ([System.Windows.Automation.TextPattern2]::Pattern)
    if ($null -eq $textPattern2) { return $null }
    try {
        $isActive = $false
        return $textPattern2.GetCaretRange([ref]$isActive)
    } catch {
        return $null
    }
}
function Selection-Text($Element) {
    $textPattern = Get-Pattern $Element ([System.Windows.Automation.TextPattern]::Pattern)
    if ($null -eq $textPattern) { return $null }
    $selection = $null
    try { $selection = $textPattern.GetSelection() } catch { return $null }
    if ($null -eq $selection -or $selection.Length -eq 0) { return $null }
    $text = Normalize-Text ($selection[0].GetText(-1))
    if ([string]::IsNullOrWhiteSpace($text)) { return $null }
    return $text
}
function Supports-CaretRange($Element) {
    $null -ne (Get-CaretRange $Element)
}
function Find-TextElement {
    $allowCaret = $env:STEPLER_UIA_DOCUMENT_ALLOW_CARET_FALLBACK -eq '1'
    $focused = [System.Windows.Automation.AutomationElement]::FocusedElement
    if ($null -ne (Selection-Text $focused)) {
        return $focused
    }
    if ($allowCaret -and (Supports-CaretRange $focused)) {
        return $focused
    }
    $foreground = [System.Windows.Automation.AutomationElement]::FromHandle((Get-SteplerForegroundHandle))
    if ($null -eq $foreground) { return $focused }
    $condition = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::IsKeyboardFocusableProperty,
        $true)
    $all = $foreground.FindAll([System.Windows.Automation.TreeScope]::Descendants, $condition)
    foreach ($candidate in $all) {
        try {
            if ($candidate.Current.HasKeyboardFocus -and ($null -ne (Selection-Text $candidate))) {
                return $candidate
            }
            if ($allowCaret -and $candidate.Current.HasKeyboardFocus -and (Supports-CaretRange $candidate)) {
                return $candidate
            }
        } catch { }
    }
    $all = $foreground.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
    foreach ($candidate in $all) {
        try {
            if ($null -ne (Selection-Text $candidate)) {
                return $candidate
            }
            if ($allowCaret -and (Supports-CaretRange $candidate)) {
                return $candidate
            }
        } catch { }
    }
    return $focused
}
$element = Find-TextElement
if ($null -eq $element) {
    'ok=0'
    'error=no_text_element'
    exit 0
}
$textPattern = Get-Pattern $element ([System.Windows.Automation.TextPattern]::Pattern)
if ($null -eq $textPattern) {
    'ok=0'
    'error=no_text_pattern'
    exit 0
}
$selection = $null
try { $selection = $textPattern.GetSelection() } catch { }
if ($null -eq $selection -or $selection.Length -eq 0) {
    if ($env:STEPLER_UIA_DOCUMENT_ALLOW_CARET_FALLBACK -ne '1') {
        'ok=0'
        'error=no_selection'
        exit 0
    }
    $range = Get-CaretRange $element
    if ($null -eq $range) {
        'ok=0'
        'error=no_selection'
        exit 0
    }
} else {
    $range = $selection[0]
}
$isCollapsed = $false
try {
    $isCollapsed = 0 -eq $range.CompareEndpoints(
        [System.Windows.Automation.Text.TextPatternRangeEndpoint]::Start,
        $range,
        [System.Windows.Automation.Text.TextPatternRangeEndpoint]::End)
} catch { }
$text = Normalize-Text ($range.GetText(-1))
if ($isCollapsed -or [string]::IsNullOrWhiteSpace($text)) {
    if ($env:STEPLER_UIA_DOCUMENT_ALLOW_CARET_FALLBACK -ne '1') {
        'ok=0'
        'error=empty_selection_text'
        exit 0
    }
    $document = $textPattern.DocumentRange
    $beforeCaret = $document.Clone()
    $beforeCaret.MoveEndpointByRange(
        [System.Windows.Automation.Text.TextPatternRangeEndpoint]::End,
        $range,
        [System.Windows.Automation.Text.TextPatternRangeEndpoint]::End) | Out-Null
    $text = Normalize-Text ($beforeCaret.GetText(-1))
    if ([string]::IsNullOrWhiteSpace($text)) {
        'ok=0'
        'error=empty_caret_left_text'
        exit 0
    }
    if ($text.Length -gt 20000) {
        'ok=0'
        'error=caret_left_text_too_large'
        exit 0
    }
    'ok=1'
    'kind=caret'
    'runtime_id=' + (Runtime-Id $element)
    'text_b64=' + (ConvertTo-B64 $text)
    exit 0
}
'ok=1'
'kind=selection'
'runtime_id=' + (Runtime-Id $element)
'text_b64=' + (ConvertTo-B64 $text)
"#;

#[cfg(windows)]
pub(super) const UIA_DOCUMENT_SELECT_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class SteplerUser32 {
    [DllImport("user32.dll")]
    public static extern IntPtr GetForegroundWindow();
}
'@
function Get-SteplerForegroundHandle {
    if (-not [string]::IsNullOrWhiteSpace($env:STEPLER_UIA_FOREGROUND_HWND)) {
        return [IntPtr]([Int64]::Parse($env:STEPLER_UIA_FOREGROUND_HWND))
    }
    [SteplerUser32]::GetForegroundWindow()
}
function ConvertFrom-B64([string] $Text) {
    [System.Text.Encoding]::Unicode.GetString([Convert]::FromBase64String($Text))
}
function Normalize-Text([string] $Text) {
    if ($null -eq $Text) { return '' }
    $Text.TrimEnd([char]13)
}
function Get-Pattern($Element, $Pattern) {
    try { return $Element.GetCurrentPattern($Pattern) } catch { return $null }
}
function Runtime-Id($Element) {
    if ($null -eq $Element) { return '' }
    try { return ($Element.GetRuntimeId() -join '.') } catch { return '' }
}
function Find-ElementByRuntimeId([string] $RuntimeId) {
    $focused = [System.Windows.Automation.AutomationElement]::FocusedElement
    if ((Runtime-Id $focused) -eq $RuntimeId) {
        return $focused
    }
    $foreground = [System.Windows.Automation.AutomationElement]::FromHandle((Get-SteplerForegroundHandle))
    if ($null -eq $foreground) { return $focused }
    $all = $foreground.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
    foreach ($candidate in $all) {
        if ((Runtime-Id $candidate) -eq $RuntimeId) {
            return $candidate
        }
    }
    return $focused
}
$element = Find-ElementByRuntimeId $env:STEPLER_UIA_RUNTIME_ID
if ($null -eq $element -or (Runtime-Id $element) -ne $env:STEPLER_UIA_RUNTIME_ID) {
    'ok=0'
    'error=element_changed'
    exit 0
}
$textPattern = Get-Pattern $element ([System.Windows.Automation.TextPattern]::Pattern)
if ($null -eq $textPattern) {
    'ok=0'
    'error=no_text_pattern'
    exit 0
}
$selection = $textPattern.GetSelection()
if ($null -eq $selection -or $selection.Length -eq 0) {
    'ok=0'
    'error=no_selection'
    exit 0
}
$range = $selection[0]
$actual = Normalize-Text ($range.GetText(-1))
$expected = ConvertFrom-B64 $env:STEPLER_UIA_EXPECTED_B64
if ($actual -ne $expected) {
    'ok=0'
    'error=preflight'
    exit 0
}
$range.Select()
'ok=1'
"#;

#[cfg(windows)]
pub(super) const UIA_DOCUMENT_SELECT_CARET_RANGE_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class SteplerUser32 {
    [DllImport("user32.dll")]
    public static extern IntPtr GetForegroundWindow();
}
'@
function Get-SteplerForegroundHandle {
    if (-not [string]::IsNullOrWhiteSpace($env:STEPLER_UIA_FOREGROUND_HWND)) {
        return [IntPtr]([Int64]::Parse($env:STEPLER_UIA_FOREGROUND_HWND))
    }
    [SteplerUser32]::GetForegroundWindow()
}
function ConvertFrom-B64([string] $Text) {
    [System.Text.Encoding]::Unicode.GetString([Convert]::FromBase64String($Text))
}
function Normalize-Text([string] $Text) {
    if ($null -eq $Text) { return '' }
    $Text.TrimEnd([char]13)
}
function Get-Pattern($Element, $Pattern) {
    try { return $Element.GetCurrentPattern($Pattern) } catch { return $null }
}
function Runtime-Id($Element) {
    if ($null -eq $Element) { return '' }
    try { return ($Element.GetRuntimeId() -join '.') } catch { return '' }
}
function Find-ElementByRuntimeId([string] $RuntimeId) {
    $focused = [System.Windows.Automation.AutomationElement]::FocusedElement
    if ((Runtime-Id $focused) -eq $RuntimeId) {
        return $focused
    }
    $foreground = [System.Windows.Automation.AutomationElement]::FromHandle((Get-SteplerForegroundHandle))
    if ($null -eq $foreground) { return $focused }
    $all = $foreground.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
    foreach ($candidate in $all) {
        if ((Runtime-Id $candidate) -eq $RuntimeId) {
            return $candidate
        }
    }
    return $focused
}
$element = Find-ElementByRuntimeId $env:STEPLER_UIA_RUNTIME_ID
if ($null -eq $element -or (Runtime-Id $element) -ne $env:STEPLER_UIA_RUNTIME_ID) {
    'ok=0'
    'error=element_changed'
    exit 0
}
$textPattern = Get-Pattern $element ([System.Windows.Automation.TextPattern]::Pattern)
if ($null -eq $textPattern) {
    'ok=0'
    'error=no_text_pattern'
    exit 0
}
$selection = $null
try { $selection = $textPattern.GetSelection() } catch { }
if ($null -eq $selection -or $selection.Length -eq 0) {
    $range = Get-CaretRange $element
}
else {
    $range = $selection[0].Clone()
}
if ($null -eq $range) {
    'ok=0'
    'error=no_caret_selection'
    exit 0
}
$range = $range.Clone()
$startDelta = [int]$env:STEPLER_UIA_START_DELTA_UTF16
$endDelta = [int]$env:STEPLER_UIA_END_DELTA_UTF16
$range.MoveEndpointByUnit(
    [System.Windows.Automation.Text.TextPatternRangeEndpoint]::Start,
    [System.Windows.Automation.Text.TextUnit]::Character,
    $startDelta) | Out-Null
$range.MoveEndpointByUnit(
    [System.Windows.Automation.Text.TextPatternRangeEndpoint]::End,
    [System.Windows.Automation.Text.TextUnit]::Character,
    $endDelta) | Out-Null
$actual = Normalize-Text ($range.GetText(-1))
$expected = ConvertFrom-B64 $env:STEPLER_UIA_EXPECTED_B64
if ($actual -ne $expected) {
    'ok=0'
    'error=preflight'
    exit 0
}
$range.Select()
'ok=1'
"#;

#[cfg(windows)]
pub(super) const UIA_DOCUMENT_VERIFY_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class SteplerUser32 {
    [DllImport("user32.dll")]
    public static extern IntPtr GetForegroundWindow();
}
'@
function Get-SteplerForegroundHandle {
    if (-not [string]::IsNullOrWhiteSpace($env:STEPLER_UIA_FOREGROUND_HWND)) {
        return [IntPtr]([Int64]::Parse($env:STEPLER_UIA_FOREGROUND_HWND))
    }
    [SteplerUser32]::GetForegroundWindow()
}
function ConvertTo-B64([string] $Text) {
    if ($null -eq $Text) { $Text = '' }
    [Convert]::ToBase64String([System.Text.Encoding]::Unicode.GetBytes($Text))
}
function ConvertFrom-B64([string] $Text) {
    [System.Text.Encoding]::Unicode.GetString([Convert]::FromBase64String($Text))
}
function Normalize-Text([string] $Text) {
    if ($null -eq $Text) { return '' }
    $Text.TrimEnd([char]13)
}
function Get-Pattern($Element, $Pattern) {
    try { return $Element.GetCurrentPattern($Pattern) } catch { return $null }
}
function Runtime-Id($Element) {
    if ($null -eq $Element) { return '' }
    try { return ($Element.GetRuntimeId() -join '.') } catch { return '' }
}
function Find-ElementByRuntimeId([string] $RuntimeId) {
    $focused = [System.Windows.Automation.AutomationElement]::FocusedElement
    if ((Runtime-Id $focused) -eq $RuntimeId) {
        return $focused
    }
    $foreground = [System.Windows.Automation.AutomationElement]::FromHandle((Get-SteplerForegroundHandle))
    if ($null -eq $foreground) { return $focused }
    $all = $foreground.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
    foreach ($candidate in $all) {
        if ((Runtime-Id $candidate) -eq $RuntimeId) {
            return $candidate
        }
    }
    return $focused
}
$element = Find-ElementByRuntimeId $env:STEPLER_UIA_RUNTIME_ID
if ($null -eq $element -or (Runtime-Id $element) -ne $env:STEPLER_UIA_RUNTIME_ID) {
    'ok=0'
    'error=element_changed'
    exit 0
}
$textPattern = Get-Pattern $element ([System.Windows.Automation.TextPattern]::Pattern)
if ($null -eq $textPattern) {
    'ok=0'
    'error=no_text_pattern'
    exit 0
}
$replacement = ConvertFrom-B64 $env:STEPLER_UIA_REPLACEMENT_B64
$selection = $textPattern.GetSelection()
if ($null -eq $selection -or $selection.Length -eq 0) {
    'ok=0'
    'error=no_caret_selection'
    exit 0
}
$range = $selection[0].Clone()
try {
    $null = $range.MoveEndpointByUnit(
        [System.Windows.Automation.Text.TextPatternRangeEndpoint]::Start,
        [System.Windows.Automation.Text.TextUnit]::Character,
        -1 * $replacement.Length)
    $actual = Normalize-Text ($range.GetText(-1))
    if ($actual -eq $replacement) {
        'ok=1'
        'actual_b64=' + (ConvertTo-B64 $actual)
        exit 0
    }
} catch { }
'ok=0'
'error=verify_failed'
"#;
