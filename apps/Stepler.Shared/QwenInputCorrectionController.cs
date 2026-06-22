using System.Diagnostics;
using System.Text;
using System.Text.Json;
using System.Runtime.InteropServices;
using System.Windows.Forms;

namespace Stepler.Shared;

internal sealed class QwenInputCorrectionController
{
    private readonly string _cliPath;
    private readonly Form _owner;
    private readonly TextBox _input;
    private readonly Action<string> _setStatus;
    private readonly Action<string> _log;
    private readonly Action<string, bool>? _showTiming;
    private readonly string _logPrefix;

    public QwenInputCorrectionController(
        string cliPath,
        Form owner,
        TextBox input,
        Action<string> setStatus,
        Action<string> log,
        Action<string, bool>? showTiming,
        string logPrefix)
    {
        _cliPath = cliPath;
        _owner = owner;
        _input = input;
        _setStatus = setStatus;
        _log = log;
        _showTiming = showTiming;
        _logPrefix = logPrefix;
    }

    public void ApplyCorrection(string mode)
    {
        var started = Stopwatch.StartNew();
        var label = mode == "pause" ? "P" : "CP";
        ShowTiming($"{label} нажата", failed: false);
        _setStatus($"{label} нажата");

        if (mode == "pause" && TryApplyFastPause(started, label))
        {
            return;
        }

        if (!File.Exists(_cliPath))
        {
            ShowFailure(label, "stepler-cli.exe не найден");
            return;
        }

        var text = _input.Text;
        if (string.IsNullOrWhiteSpace(text))
        {
            ShowFailure(label, "пустой ввод");
            return;
        }

        var args = new List<string>
        {
            "psreadline-plan",
            "--mode",
            mode,
            "--text-b64",
            Convert.ToBase64String(Encoding.Unicode.GetBytes(text)),
            "--cursor",
            _input.SelectionStart.ToString(),
        };
        if (_input.SelectionLength > 0)
        {
            args.Add("--selection-start");
            args.Add(_input.SelectionStart.ToString());
            args.Add("--selection-length");
            args.Add(_input.SelectionLength.ToString());
        }

        var result = RunCli(args);
        if (result.ExitCode != 0)
        {
            ShowFailure(label, $"нет замены {started.ElapsedMilliseconds} ms");
            return;
        }

        try
        {
            using var document = JsonDocument.Parse(result.Stdout);
            var root = document.RootElement;
            if (!root.TryGetProperty("applied", out var applied) || !applied.GetBoolean())
            {
                ShowFailure(label, $"нет замены {started.ElapsedMilliseconds} ms");
                return;
            }

            var nextText = root.TryGetProperty("text_b64", out var textBase64)
                ? Encoding.Unicode.GetString(Convert.FromBase64String(textBase64.GetString() ?? string.Empty))
                : root.GetProperty("text").GetString() ?? string.Empty;
            var cursor = root.TryGetProperty("cursor", out var cursorElement)
                ? cursorElement.GetInt32()
                : nextText.Length;

            _input.Text = nextText;
            RestoreSelection(cursor);
            ShowSuccessWithLayout(label, started, nextText);
            ScheduleFocusRestore(cursor);
        }
        catch (Exception error)
        {
            _log($"{_logPrefix} correction parse error {error}");
            ShowFailure(label, $"ошибка ответа {started.ElapsedMilliseconds} ms");
            ScheduleFocusRestore(_input.SelectionStart);
        }
    }

    private bool TryApplyFastPause(Stopwatch started, string label)
    {
        var text = _input.Text;
        if (string.IsNullOrWhiteSpace(text))
        {
            ShowFailure(label, "пустой ввод");
            return true;
        }

        var selectionStart = _input.SelectionStart;
        var selectionLength = _input.SelectionLength;
        var range = selectionLength > 0
            ? new TextRange(selectionStart, selectionStart + selectionLength)
            : WordRangeBeforeOrAroundCaret(text, selectionStart);
        if (range.IsEmpty || string.IsNullOrWhiteSpace(text[range.Start..range.End]))
        {
            ShowFailure(label, $"нет замены {started.ElapsedMilliseconds} ms");
            ScheduleFocusRestore(selectionStart);
            return true;
        }

        var expected = text[range.Start..range.End];
        var replacement = selectionLength > 0
            ? ConvertSelectedText(expected)
            : ConvertLayoutText(expected);
        if (replacement == expected)
        {
            ShowFailure(label, $"нет замены {started.ElapsedMilliseconds} ms");
            ScheduleFocusRestore(selectionStart);
            return true;
        }

        _input.Text = string.Concat(text.AsSpan(0, range.Start), replacement, text.AsSpan(range.End));
        var cursor = selectionLength > 0
            ? range.Start + replacement.Length
            : AdjustedCursorAfterReplacement(selectionStart, range, replacement.Length);
        RestoreSelection(cursor);
        ShowSuccessWithLayout(label, started, _input.Text);
        ScheduleFocusRestore(cursor);
        return true;
    }

    private void ShowSuccessWithLayout(string label, Stopwatch started, string text)
    {
        var layoutStarted = Stopwatch.StartNew();
        var layout = SwitchLayoutAfterCorrection(text);
        layoutStarted.Stop();

        var totalMs = started.ElapsedMilliseconds;
        if (layout.Target is null)
        {
            ShowSuccess(label, $"{totalMs} ms");
        }
        else if (layout.Applied)
        {
            ShowSuccess(label, $"{totalMs} ms, язык {layout.Target} {layoutStarted.ElapsedMilliseconds} ms");
        }
        else
        {
            ShowSuccess(label, $"{totalMs} ms, язык fail {layoutStarted.ElapsedMilliseconds} ms");
        }
    }

    private void RestoreSelection(int cursor)
    {
        _input.SelectionStart = Math.Clamp(cursor, 0, _input.TextLength);
        _input.SelectionLength = 0;
    }

    private void RestoreFocus(int cursor)
    {
        if (_owner.WindowState == FormWindowState.Minimized)
        {
            _owner.WindowState = FormWindowState.Normal;
        }

        _owner.Show();
        _owner.BringToFront();
        NativeMethods.SetForegroundWindow(_owner.Handle);
        _owner.Activate();
        _input.Focus();
        RestoreSelection(cursor);
    }

    private void ScheduleFocusRestore(int cursor)
    {
        RestoreFocus(cursor);
        _owner.BeginInvoke((Action)(() => RestoreFocus(cursor)));

        var timer = new System.Windows.Forms.Timer
        {
            Interval = 150,
        };
        timer.Tick += (_, _) =>
        {
            timer.Stop();
            timer.Dispose();
            if (!_owner.IsDisposed)
            {
                RestoreFocus(cursor);
            }
        };
        timer.Start();
    }

    private (string? Target, bool Applied) SwitchLayoutAfterCorrection(string text)
    {
        var targetLayout = DesiredLayoutForText(text);
        if (targetLayout is null)
        {
            return (null, false);
        }

        if (TrySwitchInputLanguage(targetLayout))
        {
            return (targetLayout, true);
        }

        _log($"{_logPrefix} layout switch failed target={targetLayout} reason=input_language_not_found");
        return (targetLayout, false);
    }

    private static string? DesiredLayoutForText(string text)
    {
        var russian = 0;
        var english = 0;
        foreach (var character in text)
        {
            if (IsRussianLetter(character))
            {
                russian++;
            }
            else if (character is >= 'A' and <= 'Z' or >= 'a' and <= 'z')
            {
                english++;
            }
        }

        if (russian > english)
        {
            return "russian";
        }
        if (english > russian)
        {
            return "english";
        }

        return null;
    }

    private static bool TrySwitchInputLanguage(string targetLayout)
    {
        var culturePrefix = targetLayout == "russian" ? "ru" : "en";
        foreach (InputLanguage language in InputLanguage.InstalledInputLanguages)
        {
            if (language.Culture.TwoLetterISOLanguageName.Equals(culturePrefix, StringComparison.OrdinalIgnoreCase))
            {
                InputLanguage.CurrentInputLanguage = language;
                return true;
            }
        }

        return false;
    }

    private CliResult RunCli(IEnumerable<string> arguments)
    {
        try
        {
            using var process = new Process();
            process.StartInfo = new ProcessStartInfo
            {
                FileName = _cliPath,
                UseShellExecute = false,
                CreateNoWindow = true,
                RedirectStandardOutput = true,
                RedirectStandardError = true,
            };
            foreach (var argument in arguments)
            {
                process.StartInfo.ArgumentList.Add(argument);
            }

            process.Start();
            var stdout = process.StandardOutput.ReadToEnd();
            var stderr = process.StandardError.ReadToEnd();
            process.WaitForExit(5000);
            if (!process.HasExited)
            {
                process.Kill(entireProcessTree: true);
                return new CliResult(1, stdout, "timeout");
            }

            if (!string.IsNullOrWhiteSpace(stderr))
            {
                _log($"{_logPrefix} cli stderr {stderr.Trim()}");
            }
            return new CliResult(process.ExitCode, stdout, stderr);
        }
        catch (Exception error)
        {
            _log($"{_logPrefix} cli error {error}");
            return new CliResult(1, string.Empty, error.Message);
        }
    }

    private static TextRange WordRangeBeforeOrAroundCaret(string text, int caret)
    {
        caret = Math.Clamp(caret, 0, text.Length);
        var wordEnd = caret;
        while (wordEnd > 0 && char.IsWhiteSpace(text[wordEnd - 1]))
        {
            wordEnd--;
        }

        var start = wordEnd;
        while (start > 0 && !char.IsWhiteSpace(text[start - 1]))
        {
            start--;
        }

        var end = wordEnd;
        if (end == caret)
        {
            while (end < text.Length && !char.IsWhiteSpace(text[end]))
            {
                end++;
            }
        }

        return start == wordEnd ? TextRange.Caret(caret) : new TextRange(start, end);
    }

    private static int AdjustedCursorAfterReplacement(int cursor, TextRange range, int replacementLength)
    {
        if (cursor <= range.Start)
        {
            return cursor;
        }
        if (cursor <= range.End)
        {
            return range.Start + replacementLength;
        }

        return cursor + replacementLength - (range.End - range.Start);
    }

    private static string ConvertSelectedText(string input)
    {
        var result = new StringBuilder(input.Length);
        var token = new StringBuilder();
        foreach (var character in input)
        {
            if (char.IsWhiteSpace(character))
            {
                if (token.Length > 0)
                {
                    result.Append(ConvertTokenByScript(token.ToString()));
                    token.Clear();
                }
                result.Append(character);
            }
            else
            {
                token.Append(character);
            }
        }

        if (token.Length > 0)
        {
            result.Append(ConvertTokenByScript(token.ToString()));
        }

        return result.ToString();
    }

    private static string ConvertTokenByScript(string token)
    {
        var hasRussian = token.Any(IsRussianLetter);
        var hasEnglish = token.Any(character => character is >= 'A' and <= 'Z' or >= 'a' and <= 'z');

        return (hasRussian, hasEnglish) switch
        {
            (true, false) => ConvertWithDirection(token, russianToEnglish: true),
            (false, true) => ConvertWithDirection(token, russianToEnglish: false),
            _ => ConvertLayoutText(token),
        };
    }

    private static string ConvertLayoutText(string input)
    {
        var russianCount = input.Count(character => RussianToEnglish(character) is not null);
        var englishCount = input.Count(character => EnglishToRussian(character) is not null);
        return ConvertWithDirection(input, russianToEnglish: russianCount > englishCount);
    }

    private static string ConvertWithDirection(string input, bool russianToEnglish)
    {
        var result = new StringBuilder(input.Length);
        foreach (var character in input)
        {
            result.Append(russianToEnglish
                ? RussianToEnglish(character) ?? character
                : EnglishToRussian(character) ?? character);
        }

        return result.ToString();
    }

    private static bool IsRussianLetter(char character)
    {
        return character is >= 'а' and <= 'я'
            or >= 'А' and <= 'Я'
            or 'ё'
            or 'Ё';
    }

    private static char? RussianToEnglish(char character)
    {
        return character switch
        {
            'й' => 'q', 'ц' => 'w', 'у' => 'e', 'к' => 'r', 'е' => 't', 'н' => 'y', 'г' => 'u',
            'ш' => 'i', 'щ' => 'o', 'з' => 'p', 'х' => '[', 'ъ' => ']', 'ф' => 'a', 'ы' => 's',
            'в' => 'd', 'а' => 'f', 'п' => 'g', 'р' => 'h', 'о' => 'j', 'л' => 'k', 'д' => 'l',
            'ж' => ';', 'э' => '\'', 'я' => 'z', 'ч' => 'x', 'с' => 'c', 'м' => 'v', 'и' => 'b',
            'т' => 'n', 'ь' => 'm', 'б' => ',', 'ю' => '.', 'ё' => '`', ',' => '?', '.' => '/',
            'Й' => 'Q', 'Ц' => 'W', 'У' => 'E', 'К' => 'R', 'Е' => 'T', 'Н' => 'Y', 'Г' => 'U',
            'Ш' => 'I', 'Щ' => 'O', 'З' => 'P', 'Х' => '[', 'Ъ' => ']', 'Ф' => 'A', 'Ы' => 'S',
            'В' => 'D', 'А' => 'F', 'П' => 'G', 'Р' => 'H', 'О' => 'J', 'Л' => 'K', 'Д' => 'L',
            'Ж' => ':', 'Э' => '"', 'Я' => 'Z', 'Ч' => 'X', 'С' => 'C', 'М' => 'V', 'И' => 'B',
            'Т' => 'N', 'Ь' => 'M', 'Б' => '<', 'Ю' => '>', 'Ё' => '~',
            _ => null,
        };
    }

    private static char? EnglishToRussian(char character)
    {
        return character switch
        {
            'q' => 'й', 'w' => 'ц', 'e' => 'у', 'r' => 'к', 't' => 'е', 'y' => 'н', 'u' => 'г',
            'i' => 'ш', 'o' => 'щ', 'p' => 'з', '[' => 'х', ']' => 'ъ', 'a' => 'ф', 's' => 'ы',
            'd' => 'в', 'f' => 'а', 'g' => 'п', 'h' => 'р', 'j' => 'о', 'k' => 'л', 'l' => 'д',
            ';' => 'ж', '\'' => 'э', 'z' => 'я', 'x' => 'ч', 'c' => 'с', 'v' => 'м', 'b' => 'и',
            'n' => 'т', 'm' => 'ь', ',' => 'б', '.' => 'ю', '`' => 'ё', '?' => ',', '/' => '.',
            'Q' => 'Й', 'W' => 'Ц', 'E' => 'У', 'R' => 'К', 'T' => 'Е', 'Y' => 'Н', 'U' => 'Г',
            'I' => 'Ш', 'O' => 'Щ', 'P' => 'З', 'A' => 'Ф', 'S' => 'Ы', 'D' => 'В', 'F' => 'А',
            'G' => 'П', 'H' => 'Р', 'J' => 'О', 'K' => 'Л', 'L' => 'Д', 'Z' => 'Я', 'X' => 'Ч',
            'C' => 'С', 'V' => 'М', 'B' => 'И', 'N' => 'Т', 'M' => 'Ь', '{' => 'Х', '}' => 'Ъ',
            ':' => 'Ж', '"' => 'Э', '<' => 'Б', '>' => 'Ю', '~' => 'Ё',
            _ => null,
        };
    }

    private void ShowSuccess(string label, string details)
    {
        var text = $"{label} {details}";
        ShowTiming(text, failed: false);
        _setStatus(text);
    }

    private void ShowFailure(string label, string details)
    {
        var text = $"{label} failed";
        ShowTiming(text, failed: true);
        _setStatus($"{text}: {details}");
    }

    private void ShowTiming(string text, bool failed)
    {
        _showTiming?.Invoke(text, failed);
        _owner.Update();
    }

    private readonly record struct TextRange(int Start, int End)
    {
        public bool IsEmpty => Start == End;

        public static TextRange Caret(int offset) => new(offset, offset);
    }

    private readonly record struct CliResult(int ExitCode, string Stdout, string Stderr);

    private static class NativeMethods
    {
        [DllImport("user32.dll")]
        public static extern bool SetForegroundWindow(IntPtr hWnd);
    }
}
