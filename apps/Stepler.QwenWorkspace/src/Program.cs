using System.Diagnostics;
using System.Runtime.InteropServices;

namespace Stepler.QwenWorkspace;

internal static class Program
{
    [STAThread]
    private static void Main(string[] args)
    {
        ApplicationConfiguration.Initialize();
        Application.Run(new WorkspaceForm(AppContext.BaseDirectory, WorkspaceOptions.Parse(args)));
    }

    internal static void SafeLog(string message)
    {
        try
        {
            var logDir = Path.Combine(
                Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
                "Stepler",
                "logs");
            Directory.CreateDirectory(logDir);
            File.AppendAllText(
                Path.Combine(logDir, "Stepler.QwenWorkspace.log"),
                $"{DateTimeOffset.Now:o} {message}{Environment.NewLine}");
        }
        catch
        {
        }
    }
}

internal sealed class WorkspaceForm : Form
{
    private readonly string _baseDirectory;
    private readonly string _cliPath;
    private readonly string _qwenScriptPath;
    private readonly string _qwenWorkingDirectory;
    private readonly string[] _qwenArguments;
    private readonly bool _darkTheme;
    private readonly SplitContainer _split;
    private readonly Panel _terminalHost;
    private readonly TextBox _input;
    private readonly Label _status;
    private readonly Button _submitButton;
    private readonly System.Windows.Forms.Timer _syncTimer;
    private Process? _terminalProcess;
    private IntPtr _terminalHwnd;
    private nint _originalParent;
    private nint _originalOwner;
    private int _originalStyle;

    public WorkspaceForm(string baseDirectory, WorkspaceOptions options)
    {
        _baseDirectory = baseDirectory;
        _cliPath = Path.Combine(_baseDirectory, "stepler-cli.exe");
        _qwenScriptPath = Path.Combine(_baseDirectory, "scripts", "Stepler.Qwen.ps1");
        _qwenWorkingDirectory = options.WorkingDirectory;
        _qwenArguments = options.QwenArguments;
        _darkTheme = options.DarkTheme;

        Text = "Stepler Qwen Workspace";
        StartPosition = FormStartPosition.CenterScreen;
        MinimumSize = new Size(820, 560);
        ClientSize = new Size(1100, 760);

        _split = new SplitContainer
        {
            Dock = DockStyle.Fill,
            Orientation = Orientation.Horizontal,
            SplitterWidth = 6,
            Panel1MinSize = 240,
            Panel2MinSize = 150,
        };

        _terminalHost = new Panel
        {
            Dock = DockStyle.Fill,
            BackColor = Color.Black,
        };
        _split.Panel1.Controls.Add(_terminalHost);

        _input = new TextBox
        {
            AcceptsReturn = true,
            AcceptsTab = true,
            Multiline = true,
            ScrollBars = ScrollBars.Vertical,
            Dock = DockStyle.Fill,
            Font = new Font("Segoe UI", 10),
            AllowDrop = true,
        };
        _input.DragEnter += OnInputDragEnter;
        _input.DragDrop += OnInputDragDrop;

        _submitButton = new Button
        {
            Text = "Отправить",
            Dock = DockStyle.Bottom,
            Height = 36,
        };
        _submitButton.Click += (_, _) => Submit();

        _syncTimer = new System.Windows.Forms.Timer
        {
            Interval = 250,
        };
        _syncTimer.Tick += (_, _) => ResizeEmbeddedTerminal();

        _status = new Label
        {
            Text = "Запуск Qwen...",
            Dock = DockStyle.Bottom,
            Height = 24,
            TextAlign = ContentAlignment.MiddleLeft,
        };

        _split.Panel2.Controls.Add(_input);
        _split.Panel2.Controls.Add(_status);
        _split.Panel2.Controls.Add(_submitButton);
        Controls.Add(_split);
        ApplyTheme();

        Shown += (_, _) =>
        {
            SetInitialSplitterDistance();
            LaunchAndEmbedTerminal();
        };
        Resize += (_, _) => ResizeEmbeddedTerminal();
        Move += (_, _) => ResizeEmbeddedTerminal();
        _split.SplitterMoved += (_, _) => ResizeEmbeddedTerminal();
        FormClosing += OnFormClosing;
    }

    private void SetInitialSplitterDistance()
    {
        var min = _split.Panel1MinSize;
        var max = _split.Height - _split.Panel2MinSize;
        if (max >= min)
        {
            var preferred = Math.Clamp((int)(_split.Height * 0.72), min, max);
            _split.SplitterDistance = preferred;
        }
    }

    private void LaunchAndEmbedTerminal()
    {
        if (!File.Exists(_qwenScriptPath))
        {
            SetStatus("Не найден scripts\\Stepler.Qwen.ps1");
            return;
        }

        var shell = ResolvePowerShell();
        using var process = new Process();
        process.StartInfo = new ProcessStartInfo
        {
            FileName = shell,
            UseShellExecute = false,
            CreateNoWindow = false,
            WorkingDirectory = _qwenWorkingDirectory,
        };
        process.StartInfo.ArgumentList.Add("-NoExit");
        process.StartInfo.ArgumentList.Add("-ExecutionPolicy");
        process.StartInfo.ArgumentList.Add("Bypass");
        process.StartInfo.ArgumentList.Add("-File");
        process.StartInfo.ArgumentList.Add(_qwenScriptPath);
        foreach (var argument in _qwenArguments)
        {
            process.StartInfo.ArgumentList.Add(argument);
        }

        try
        {
            process.Start();
            _terminalProcess = Process.GetProcessById(process.Id);
        }
        catch (Exception error)
        {
            Program.SafeLog($"terminal start failed {error}");
            SetStatus("Не удалось запустить PowerShell/Qwen");
            return;
        }

        var hwnd = WaitForQwenWindow(_terminalProcess, TimeSpan.FromSeconds(12));
        if (hwnd == IntPtr.Zero)
        {
            SetStatus("Окно PowerShell не найдено");
            return;
        }

        EmbedTerminal(hwnd);
        SetStatus("Qwen запущен");
        BeginInvoke(FocusInput);
    }

    private void EmbedTerminal(IntPtr hwnd)
    {
        _terminalHwnd = hwnd;
        _originalParent = NativeMethods.GetParent(hwnd);
        _originalOwner = NativeMethods.GetWindowLongPtr(hwnd, NativeMethods.GWLP_HWNDPARENT);
        _originalStyle = NativeMethods.GetWindowLong(hwnd, NativeMethods.GWL_STYLE);
        var nextStyle = _originalStyle
            & ~NativeMethods.WS_CAPTION
            & ~NativeMethods.WS_THICKFRAME;
        NativeMethods.SetWindowLongPtr(hwnd, NativeMethods.GWLP_HWNDPARENT, Handle);
        NativeMethods.SetWindowLong(hwnd, NativeMethods.GWL_STYLE, nextStyle);
        NativeMethods.SetWindowPos(
            hwnd,
            0,
            0,
            0,
            0,
            0,
            NativeMethods.SWP_NOMOVE
                | NativeMethods.SWP_NOSIZE
                | NativeMethods.SWP_NOZORDER
                | NativeMethods.SWP_FRAMECHANGED);
        NativeMethods.ShowWindow(hwnd, NativeMethods.SW_SHOW);
        ResizeEmbeddedTerminal();
        _syncTimer.Start();
    }

    private void FocusInput()
    {
        Activate();
        _input.Focus();
    }

    private void ResizeEmbeddedTerminal()
    {
        if (_terminalHwnd == IntPtr.Zero)
        {
            return;
        }

        if (WindowState == FormWindowState.Minimized)
        {
            return;
        }

        var bounds = _terminalHost.RectangleToScreen(_terminalHost.ClientRectangle);
        NativeMethods.SetWindowPos(
            _terminalHwnd,
            Handle,
            bounds.Left,
            bounds.Top,
            Math.Max(1, bounds.Width),
            Math.Max(1, bounds.Height),
            NativeMethods.SWP_NOACTIVATE);
    }

    private void OnFormClosing(object? sender, FormClosingEventArgs e)
    {
        if (_terminalHwnd == IntPtr.Zero)
        {
            return;
        }

        _syncTimer.Stop();
        if (_originalParent != 0)
        {
            NativeMethods.SetParent(_terminalHwnd, _originalParent);
        }
        NativeMethods.SetWindowLongPtr(_terminalHwnd, NativeMethods.GWLP_HWNDPARENT, _originalOwner);
        if (_originalStyle != 0)
        {
            NativeMethods.SetWindowLong(_terminalHwnd, NativeMethods.GWL_STYLE, _originalStyle);
        }
        NativeMethods.ShowWindow(_terminalHwnd, NativeMethods.SW_SHOW);
        _terminalHwnd = IntPtr.Zero;
    }

    private void Submit()
    {
        var text = _input.Text;
        if (string.IsNullOrWhiteSpace(text))
        {
            return;
        }

        if (!File.Exists(_cliPath))
        {
            SetStatus("stepler-cli.exe не найден");
            return;
        }

        try
        {
            using var process = new Process();
            process.StartInfo = new ProcessStartInfo
            {
                FileName = _cliPath,
                UseShellExecute = false,
                CreateNoWindow = true,
                RedirectStandardError = true,
                RedirectStandardOutput = true,
            };
            process.StartInfo.ArgumentList.Add("qwen-submit");
            process.StartInfo.ArgumentList.Add("--text");
            process.StartInfo.ArgumentList.Add(text);
            process.Start();
            process.WaitForExit(5000);
            if (!process.HasExited)
            {
                process.Kill(entireProcessTree: true);
                SetStatus("Отправка: timeout");
                return;
            }

            if (process.ExitCode == 0)
            {
                _input.Clear();
                SetStatus("Отправлено в Qwen");
            }
            else
            {
                var stderr = process.StandardError.ReadToEnd();
                Program.SafeLog($"qwen submit failed {stderr.Trim()}");
                SetStatus("Qwen input-file не найден");
            }
        }
        catch (Exception error)
        {
            Program.SafeLog($"qwen submit error {error}");
            SetStatus("Ошибка отправки");
        }
    }

    private void OnInputDragEnter(object? sender, DragEventArgs e)
    {
        e.Effect = e.Data?.GetDataPresent(DataFormats.FileDrop) == true
            ? DragDropEffects.Copy
            : DragDropEffects.None;
    }

    private void OnInputDragDrop(object? sender, DragEventArgs e)
    {
        if (e.Data?.GetData(DataFormats.FileDrop) is not string[] paths || paths.Length == 0)
        {
            return;
        }

        _input.Focus();
        _input.SelectedText = string.Join(Environment.NewLine, paths);
        SetStatus(paths.Length == 1 ? "Путь файла вставлен" : $"Пути файлов вставлены: {paths.Length}");
    }

    private void SetStatus(string text)
    {
        _status.Text = text;
    }

    protected override void Dispose(bool disposing)
    {
        if (disposing)
        {
            _syncTimer.Dispose();
        }

        base.Dispose(disposing);
    }

    private void ApplyTheme()
    {
        var palette = ThemePalette.FromDarkTheme(_darkTheme);
        BackColor = palette.Window;
        ForeColor = palette.Text;
        _split.BackColor = palette.Border;
        _split.Panel2.BackColor = palette.Window;
        _terminalHost.BackColor = Color.Black;
        _input.BackColor = palette.Input;
        _input.ForeColor = palette.InputText;
        _input.BorderStyle = BorderStyle.FixedSingle;
        _status.BackColor = palette.Window;
        _status.ForeColor = palette.MutedText;
        _submitButton.BackColor = palette.Control;
        _submitButton.ForeColor = palette.Text;
        _submitButton.FlatStyle = FlatStyle.Flat;
        _submitButton.FlatAppearance.BorderColor = palette.Border;
        NativeMethods.SetImmersiveDarkMode(Handle, _darkTheme);
    }

    private static string ResolvePowerShell()
    {
        var pwsh = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.ProgramFiles),
            "PowerShell",
            "7",
            "pwsh.exe");
        return File.Exists(pwsh) ? pwsh : "powershell.exe";
    }

    private static IntPtr WaitForQwenWindow(Process process, TimeSpan timeout)
    {
        var started = Stopwatch.StartNew();
        while (started.Elapsed < timeout)
        {
            process.Refresh();
            if (process.HasExited)
            {
                return IntPtr.Zero;
            }
            if (process.MainWindowHandle != IntPtr.Zero)
            {
                return process.MainWindowHandle;
            }
            var qwenWindow = NativeMethods.FindTopLevelWindowByTitle("stepler-terminal-app qwen");
            if (qwenWindow != IntPtr.Zero)
            {
                return qwenWindow;
            }
            Thread.Sleep(100);
        }
        return IntPtr.Zero;
    }
}

internal sealed record WorkspaceOptions(string WorkingDirectory, bool DarkTheme, string[] QwenArguments)
{
    public static WorkspaceOptions Parse(string[] args)
    {
        var workingDirectory = Environment.GetEnvironmentVariable("STEPLER_QWEN_WORKDIR");
        var darkTheme = true;
        var qwenArguments = new List<string>();

        for (var index = 0; index < args.Length; index++)
        {
            if (args[index] == "--workdir" && index + 1 < args.Length)
            {
                workingDirectory = args[++index];
                continue;
            }

            if (args[index] == "--dark-theme" && index + 1 < args.Length)
            {
                darkTheme = bool.TryParse(args[++index], out var parsed) ? parsed : darkTheme;
                continue;
            }

            qwenArguments.Add(args[index]);
        }

        if (string.IsNullOrWhiteSpace(workingDirectory) || !Directory.Exists(workingDirectory))
        {
            workingDirectory = Environment.GetFolderPath(Environment.SpecialFolder.UserProfile);
        }

        return new WorkspaceOptions(workingDirectory, darkTheme, qwenArguments.ToArray());
    }
}

internal readonly record struct ThemePalette(
    Color Window,
    Color Control,
    Color Border,
    Color Text,
    Color MutedText,
    Color Input,
    Color InputText)
{
    public static ThemePalette FromDarkTheme(bool darkTheme)
    {
        return darkTheme
            ? new ThemePalette(
                Color.FromArgb(30, 32, 36),
                Color.FromArgb(43, 47, 53),
                Color.FromArgb(75, 82, 92),
                Color.FromArgb(245, 247, 250),
                Color.FromArgb(190, 197, 207),
                Color.FromArgb(22, 24, 27),
                Color.FromArgb(245, 247, 250))
            : new ThemePalette(
                SystemColors.Window,
                SystemColors.Control,
                SystemColors.ControlDark,
                SystemColors.ControlText,
                SystemColors.GrayText,
                SystemColors.Window,
                SystemColors.WindowText);
    }
}

internal static class NativeMethods
{
    public const int GWL_STYLE = -16;
    public const int GWLP_HWNDPARENT = -8;
    public const int SW_SHOW = 5;
    public const int WS_CHILD = 0x40000000;
    public const int WS_CAPTION = 0x00C00000;
    public const int WS_THICKFRAME = 0x00040000;
    public const int SWP_NOSIZE = 0x0001;
    public const int SWP_NOMOVE = 0x0002;
    public const int SWP_NOZORDER = 0x0004;
    public const int SWP_NOACTIVATE = 0x0010;
    public const int SWP_FRAMECHANGED = 0x0020;
    private const int DWMWA_USE_IMMERSIVE_DARK_MODE = 20;

    public static nint FindTopLevelWindowByTitle(string titlePart)
    {
        nint result = 0;
        EnumWindows((hwnd, _) =>
        {
            var length = GetWindowTextLength(hwnd);
            if (length <= 0)
            {
                return true;
            }

            var builder = new System.Text.StringBuilder(length + 1);
            GetWindowText(hwnd, builder, builder.Capacity);
            if (builder.ToString().Contains(titlePart, StringComparison.OrdinalIgnoreCase))
            {
                result = hwnd;
                return false;
            }

            return true;
        }, 0);
        return result;
    }

    public static void SetImmersiveDarkMode(nint handle, bool enabled)
    {
        var value = enabled ? 1 : 0;
        DwmSetWindowAttribute(handle, DWMWA_USE_IMMERSIVE_DARK_MODE, ref value, sizeof(int));
    }

    [DllImport("user32.dll", SetLastError = true)]
    public static extern nint SetParent(nint hWndChild, nint hWndNewParent);

    [DllImport("user32.dll", SetLastError = true)]
    public static extern nint GetParent(nint hWnd);

    [DllImport("user32.dll", SetLastError = true)]
    public static extern int GetWindowLong(nint hWnd, int nIndex);

    [DllImport("user32.dll", SetLastError = true)]
    public static extern int SetWindowLong(nint hWnd, int nIndex, int dwNewLong);

    [DllImport("user32.dll", EntryPoint = "GetWindowLongPtrW", SetLastError = true)]
    public static extern nint GetWindowLongPtr(nint hWnd, int nIndex);

    [DllImport("user32.dll", EntryPoint = "SetWindowLongPtrW", SetLastError = true)]
    public static extern nint SetWindowLongPtr(nint hWnd, int nIndex, nint dwNewLong);

    [DllImport("user32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static extern bool MoveWindow(nint hWnd, int x, int y, int width, int height, [MarshalAs(UnmanagedType.Bool)] bool repaint);

    [DllImport("user32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static extern bool SetWindowPos(nint hWnd, nint hWndInsertAfter, int x, int y, int cx, int cy, uint flags);

    [DllImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static extern bool ShowWindow(nint hWnd, int nCmdShow);

    [DllImport("user32.dll")]
    public static extern nint SetFocus(nint hWnd);

    [DllImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static extern bool SetForegroundWindow(nint hWnd);

    [DllImport("user32.dll")]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool EnumWindows(EnumWindowsProc callback, nint lParam);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern int GetWindowText(nint hWnd, System.Text.StringBuilder text, int count);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern int GetWindowTextLength(nint hWnd);

    [DllImport("dwmapi.dll")]
    private static extern int DwmSetWindowAttribute(nint hwnd, int attribute, ref int attributeValue, int attributeSize);

    private delegate bool EnumWindowsProc(nint hwnd, nint lParam);
}
