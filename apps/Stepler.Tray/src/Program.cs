using System.Diagnostics;
using System.Drawing;
using System.Runtime.InteropServices;
using System.Text.Json;
using System.Windows.Forms;
using Microsoft.Win32;

namespace Stepler.Tray;

internal static class Program
{
    [STAThread]
    private static void Main(string[] args)
    {
        if (args.Any(arg => string.Equals(arg, "--stop", StringComparison.OrdinalIgnoreCase)))
        {
            StopExistingProcesses();
            return;
        }

        try
        {
            using var mutex = new Mutex(initiallyOwned: true, "Stepler.TrayHost", out var ownsMutex);
            if (!ownsMutex)
            {
                return;
            }

            File.AppendAllText(LogPath(), $"{DateTimeOffset.Now:o} tray main start{Environment.NewLine}");
            ApplicationConfiguration.Initialize();
            Application.ThreadException += (_, error) =>
                File.AppendAllText(LogPath(), $"{DateTimeOffset.Now:o} tray thread exception {error.Exception}{Environment.NewLine}");
            AppDomain.CurrentDomain.UnhandledException += (_, error) =>
                File.AppendAllText(LogPath(), $"{DateTimeOffset.Now:o} tray unhandled exception {error.ExceptionObject}{Environment.NewLine}");
            Application.ApplicationExit += (_, _) =>
                File.AppendAllText(LogPath(), $"{DateTimeOffset.Now:o} tray application exit{Environment.NewLine}");
            Application.Run(new SteplerTrayForm());
            File.AppendAllText(LogPath(), $"{DateTimeOffset.Now:o} tray main stop{Environment.NewLine}");
        }
        catch (Exception error)
        {
            File.AppendAllText(LogPath(), $"{DateTimeOffset.Now:o} tray fatal {error}{Environment.NewLine}");
        }
    }

    internal static string LogPath()
    {
        return Path.Combine(AppContext.BaseDirectory, "Stepler.Tray.log");
    }

    private static void StopExistingProcesses()
    {
        var currentId = Environment.ProcessId;
        File.AppendAllText(LogPath(), $"{DateTimeOffset.Now:o} stop requested current={currentId}{Environment.NewLine}");
        foreach (var processName in new[] { "Stepler.Tray", "stepler-cli" })
        {
            foreach (var process in Process.GetProcessesByName(processName))
            {
                try
                {
                    if (process.Id == currentId)
                    {
                        continue;
                    }

                    process.Kill(entireProcessTree: true);
                    File.AppendAllText(LogPath(), $"{DateTimeOffset.Now:o} stopped {processName} pid={process.Id}{Environment.NewLine}");
                }
                catch (Exception error)
                {
                    File.AppendAllText(LogPath(), $"{DateTimeOffset.Now:o} stop failed {processName} pid={process.Id} {error.GetType().Name}{Environment.NewLine}");
                    // Best-effort emergency stop.
                }
            }
        }
    }
}

internal sealed class SteplerTrayForm : Form
{
    private readonly NotifyIcon _notifyIcon;
    private readonly Icon _appIcon;
    private readonly string? _repoRoot;
    private readonly ToolStripMenuItem _statusItem;
    private readonly ToolStripMenuItem _toggleItem;
    private readonly ToolStripMenuItem _restartItem;
    private readonly ToolStripMenuItem _pauseItem;
    private readonly ToolStripMenuItem _scrollLockItem;
    private readonly ToolStripMenuItem _ctrlLayoutItem;
    private readonly ToolStripMenuItem _menuCapsLayoutItem;
    private readonly ToolStripMenuItem _riskyFallbacksItem;
    private readonly ToolStripMenuItem _autostartItem;
    private readonly ToolStripMenuItem _openHotkeyLogItem;
    private readonly ToolStripMenuItem _openTrayLogItem;
    private ControlWindow? _controlWindow;
    private Process? _runner;
    private RunnerJob? _runnerJob;
    private SteplerSettings _settings;

    public SteplerTrayForm()
    {
        _repoRoot = FindRepoRoot();
        _settings = SteplerSettingsStore.Load();
        File.AppendAllText(
            Program.LogPath(),
            $"{DateTimeOffset.Now:o} settings loaded path={SteplerSettingsStore.SettingsPath()} {JsonSerializer.Serialize(_settings)}{Environment.NewLine}");

        Text = "Stepler";
        ShowInTaskbar = false;
        WindowState = FormWindowState.Minimized;
        FormBorderStyle = FormBorderStyle.FixedToolWindow;
        StartPosition = FormStartPosition.Manual;
        Location = new Point(-32000, -32000);
        Size = new Size(1, 1);
        Opacity = 0;

        _appIcon = SteplerIcon.Create();

        _statusItem = new ToolStripMenuItem("Статус: запуск...")
        {
            Enabled = false,
        };

        _toggleItem = new ToolStripMenuItem("Выключить обработчик");
        _toggleItem.Click += (_, _) => ToggleRunner();

        _restartItem = new ToolStripMenuItem("Перезапустить обработчик");
        _restartItem.Click += (_, _) => RestartRunner();

        _pauseItem = new ToolStripMenuItem("Pause")
        {
            CheckOnClick = true,
        };
        _pauseItem.Click += (_, _) => UpdateSetting(settings => settings.PauseEnabled = _pauseItem.Checked);

        _scrollLockItem = new ToolStripMenuItem("ScrollLock")
        {
            CheckOnClick = true,
        };
        _scrollLockItem.Click += (_, _) => UpdateSetting(settings => settings.ScrollLockEnabled = _scrollLockItem.Checked);

        _ctrlLayoutItem = new ToolStripMenuItem("Left/Right Ctrl: RU/EN")
        {
            CheckOnClick = true,
        };
        _ctrlLayoutItem.Click += (_, _) => UpdateSetting(settings => settings.CtrlLayoutSwitchEnabled = _ctrlLayoutItem.Checked);

        _menuCapsLayoutItem = new ToolStripMenuItem("Menu/Caps: следующая раскладка")
        {
            CheckOnClick = true,
        };
        _menuCapsLayoutItem.Click += (_, _) => UpdateSetting(settings => settings.MenuCapsSwitchEnabled = _menuCapsLayoutItem.Checked);

        _riskyFallbacksItem = new ToolStripMenuItem("Risky fallback adapters")
        {
            CheckOnClick = true,
        };
        _riskyFallbacksItem.Click += (_, _) => UpdateSetting(settings => settings.RiskyFallbacksEnabled = _riskyFallbacksItem.Checked);

        _autostartItem = new ToolStripMenuItem("Автозапуск Windows");
        _autostartItem.Click += (_, _) => ToggleAutostart();

        _openHotkeyLogItem = new ToolStripMenuItem("Открыть лог hotkeys");
        _openHotkeyLogItem.Click += (_, _) => OpenHotkeyLog();

        _openTrayLogItem = new ToolStripMenuItem("Открыть лог tray");
        _openTrayLogItem.Click += (_, _) => OpenFileInNotepad(Program.LogPath());

        var showItem = new ToolStripMenuItem("Показать окно управления");
        showItem.Click += (_, _) => ShowControlWindow();

        var exitItem = new ToolStripMenuItem("Выход");
        exitItem.Click += (_, _) => Close();

        var menu = new ContextMenuStrip();
        menu.Items.Add(_statusItem);
        menu.Items.Add(new ToolStripSeparator());
        menu.Items.Add(_toggleItem);
        menu.Items.Add(_restartItem);
        menu.Items.Add(new ToolStripSeparator());
        menu.Items.Add(_pauseItem);
        menu.Items.Add(_scrollLockItem);
        menu.Items.Add(_ctrlLayoutItem);
        menu.Items.Add(_menuCapsLayoutItem);
        menu.Items.Add(_riskyFallbacksItem);
        menu.Items.Add(new ToolStripSeparator());
        menu.Items.Add(_autostartItem);
        menu.Items.Add(new ToolStripSeparator());
        menu.Items.Add(_openHotkeyLogItem);
        menu.Items.Add(_openTrayLogItem);
        menu.Items.Add(new ToolStripSeparator());
        menu.Items.Add(showItem);
        menu.Items.Add(exitItem);
        menu.Opening += (_, _) => UpdateMenuState();

        _notifyIcon = new NotifyIcon
        {
            Icon = _appIcon,
            Text = "Stepler",
            ContextMenuStrip = menu,
        };

        Load += (_, _) =>
        {
            _notifyIcon.Visible = true;
            StartRunner();
            File.AppendAllText(Program.LogPath(), $"{DateTimeOffset.Now:o} tray form ready handle={Handle}{Environment.NewLine}");
        };
        FormClosed += (_, _) =>
            File.AppendAllText(Program.LogPath(), $"{DateTimeOffset.Now:o} tray form closed{Environment.NewLine}");
        Shown += (_, _) => Hide();
    }

    protected override void OnFormClosing(FormClosingEventArgs e)
    {
        StopRunner();
        _notifyIcon.Visible = false;
        _notifyIcon.Dispose();
        _appIcon.Dispose();
        base.OnFormClosing(e);
    }

    private void ShowControlWindow()
    {
        if (_controlWindow is { IsDisposed: false })
        {
            _controlWindow.UpdateStatus(_statusItem.Text ?? "Stepler");
            _controlWindow.Show();
            _controlWindow.Activate();
            return;
        }

        _controlWindow = new ControlWindow(
            _statusItem.Text ?? "Stepler",
            ToggleRunner,
            RestartRunner,
            OpenHotkeyLog,
            () => OpenFileInNotepad(Program.LogPath()),
            Close);
        _controlWindow.Show();
    }

    private void StartRunner()
    {
        if (IsRunnerAlive())
        {
            SetStatus("Статус: работает");
            return;
        }

        if (_repoRoot is null)
        {
            SetStatus("Статус: ошибка - не найден repo root");
            return;
        }

        var cliPath = Environment.GetEnvironmentVariable("STEPLER_CLI_PATH");
        if (string.IsNullOrWhiteSpace(cliPath))
        {
            cliPath = Path.Combine(_repoRoot, "target", "debug", "stepler-cli.exe");
        }

        if (!File.Exists(cliPath))
        {
            SetStatus("Статус: ошибка - stepler-cli.exe не найден");
            return;
        }

        try
        {
            StopOrphanRunners(cliPath);
            _runner = Process.Start(new ProcessStartInfo
            {
                FileName = cliPath,
                Arguments = "run-hotkeys",
                WorkingDirectory = _repoRoot,
                UseShellExecute = false,
                CreateNoWindow = true,
                WindowStyle = ProcessWindowStyle.Hidden,
            }.WithSteplerSettings(_settings));

            if (_runner is not null)
            {
                _runner.EnableRaisingEvents = true;
                _runner.Exited += (_, _) => OnRunnerExited();
                TryAttachRunnerJob(_runner);
            }

            SetStatus(_runner is null ? "Статус: ошибка запуска" : "Статус: работает");
            UpdateMenuState();
            File.AppendAllText(
                Program.LogPath(),
                $"{DateTimeOffset.Now:o} runner started pid={_runner?.Id.ToString() ?? "null"}{Environment.NewLine}");
        }
        catch (Exception error)
        {
            File.AppendAllText(Program.LogPath(), $"{DateTimeOffset.Now:o} runner start error {error}{Environment.NewLine}");
            SetStatus($"Статус: ошибка запуска ({error.GetType().Name})");
            UpdateMenuState();
        }
    }

    private void TryAttachRunnerJob(Process runner)
    {
        try
        {
            _runnerJob?.Dispose();
            _runnerJob = RunnerJob.CreateKillOnClose();
            _runnerJob.Assign(runner);
            File.AppendAllText(Program.LogPath(), $"{DateTimeOffset.Now:o} runner job attached pid={runner.Id}{Environment.NewLine}");
        }
        catch (Exception error)
        {
            _runnerJob?.Dispose();
            _runnerJob = null;
            File.AppendAllText(Program.LogPath(), $"{DateTimeOffset.Now:o} runner job attach failed pid={runner.Id} {error}{Environment.NewLine}");
        }
    }

    private void OnRunnerExited()
    {
        var exitCode = "unknown";
        try
        {
            exitCode = _runner?.ExitCode.ToString() ?? "null";
        }
        catch
        {
            // Process state may already be gone.
        }

        File.AppendAllText(Program.LogPath(), $"{DateTimeOffset.Now:o} runner exited code={exitCode}{Environment.NewLine}");
        if (!IsHandleCreated || IsDisposed)
        {
            return;
        }

        try
        {
            BeginInvoke(() =>
            {
                if (_runner is { HasExited: true })
                {
                    _runner.Dispose();
                    _runner = null;
                    _runnerJob?.Dispose();
                    _runnerJob = null;
                    SetStatus("Статус: обработчик остановлен");
                    UpdateMenuState();
                }
            });
        }
        catch
        {
            // Tray may be shutting down.
        }
    }

    private static void StopOrphanRunners(string cliPath)
    {
        foreach (var process in Process.GetProcessesByName("stepler-cli"))
        {
            try
            {
                var path = process.MainModule?.FileName;
                if (string.Equals(path, cliPath, StringComparison.OrdinalIgnoreCase))
                {
                    process.Kill(entireProcessTree: true);
                }
            }
            catch
            {
                // Best-effort cleanup before starting the single managed runner.
            }
        }
    }

    private void StopRunner()
    {
        try
        {
            if (_runner is { HasExited: false })
            {
                _runner.Kill(entireProcessTree: true);
                _runner.WaitForExit(2000);
            }
        }
        catch
        {
            // Shutdown must stay best-effort; stuck cleanup should not keep the tray app alive.
        }
        finally
        {
            _runner?.Dispose();
            _runner = null;
            _runnerJob?.Dispose();
            _runnerJob = null;
            SetStatus("Статус: выключен");
            UpdateMenuState();
        }
    }

    private void ToggleRunner()
    {
        if (IsRunnerAlive())
        {
            StopRunner();
        }
        else
        {
            StartRunner();
        }
    }

    private void RestartRunner()
    {
        StopRunner();
        StartRunner();
    }

    private void UpdateSetting(Action<SteplerSettings> update)
    {
        update(_settings);
        SteplerSettingsStore.Save(_settings);
        File.AppendAllText(
            Program.LogPath(),
            $"{DateTimeOffset.Now:o} settings saved {JsonSerializer.Serialize(_settings)}{Environment.NewLine}");

        if (IsRunnerAlive())
        {
            RestartRunner();
        }
        else
        {
            UpdateMenuState();
        }
    }

    private bool IsRunnerAlive()
    {
        try
        {
            return _runner is { HasExited: false };
        }
        catch
        {
            return false;
        }
    }

    private void SetStatus(string text)
    {
        _statusItem.Text = text;
        _notifyIcon.Text = text.Length <= 63 ? text : text[..63];
        _controlWindow?.UpdateStatus(text);
    }

    private void UpdateMenuState()
    {
        var running = IsRunnerAlive();
        _toggleItem.Text = running ? "Выключить обработчик" : "Включить обработчик";
        _restartItem.Enabled = running;
        _pauseItem.Checked = _settings.PauseEnabled;
        _scrollLockItem.Checked = _settings.ScrollLockEnabled;
        _ctrlLayoutItem.Checked = _settings.CtrlLayoutSwitchEnabled;
        _menuCapsLayoutItem.Checked = _settings.MenuCapsSwitchEnabled;
        _riskyFallbacksItem.Checked = _settings.RiskyFallbacksEnabled;
        _autostartItem.Checked = AutostartManager.IsEnabled();
    }

    private void ToggleAutostart()
    {
        try
        {
            if (AutostartManager.IsEnabled())
            {
                AutostartManager.Disable();
            }
            else
            {
                AutostartManager.Enable();
            }

            UpdateMenuState();
        }
        catch (Exception error)
        {
            File.AppendAllText(Program.LogPath(), $"{DateTimeOffset.Now:o} autostart error {error}{Environment.NewLine}");
            MessageBox.Show(
                "Не удалось изменить автозапуск. Подробности записаны в лог tray.",
                "Stepler",
                MessageBoxButtons.OK,
                MessageBoxIcon.Warning);
        }
    }

    private void OpenHotkeyLog()
    {
        if (_repoRoot is null)
        {
            return;
        }

        var logPath = Path.Combine(_repoRoot, "stepler_hotkey_log.jsonl");
        OpenFileInNotepad(logPath);
    }

    private static void OpenFileInNotepad(string path)
    {
        try
        {
            if (!File.Exists(path))
            {
                File.WriteAllText(path, string.Empty);
            }

            Process.Start(new ProcessStartInfo
            {
                FileName = "notepad.exe",
                ArgumentList = { path },
                UseShellExecute = true,
            });
        }
        catch (Exception error)
        {
            File.AppendAllText(Program.LogPath(), $"{DateTimeOffset.Now:o} open log error {error}{Environment.NewLine}");
        }
    }

    private static string? FindRepoRoot()
    {
        var current = AppContext.BaseDirectory;
        while (!string.IsNullOrWhiteSpace(current))
        {
            if (File.Exists(Path.Combine(current, "Cargo.toml"))
                && Directory.Exists(Path.Combine(current, "crates")))
            {
                return current;
            }

            current = Directory.GetParent(current)?.FullName;
        }

        var fallback = @"F:\distr\system\Stepler";
        return File.Exists(Path.Combine(fallback, "Cargo.toml")) ? fallback : null;
    }
}

internal static class AutostartManager
{
    private const string RunKeyPath = @"Software\Microsoft\Windows\CurrentVersion\Run";
    private const string ValueName = "Stepler";

    public static bool IsEnabled()
    {
        using var key = Registry.CurrentUser.OpenSubKey(RunKeyPath, writable: false);
        var expected = Quote(Application.ExecutablePath);
        return string.Equals(key?.GetValue(ValueName) as string, expected, StringComparison.OrdinalIgnoreCase);
    }

    public static void Enable()
    {
        using var key = Registry.CurrentUser.CreateSubKey(RunKeyPath);
        key?.SetValue(ValueName, Quote(Application.ExecutablePath), RegistryValueKind.String);
    }

    public static void Disable()
    {
        using var key = Registry.CurrentUser.OpenSubKey(RunKeyPath, writable: true);
        key?.DeleteValue(ValueName, throwOnMissingValue: false);
    }

    private static string Quote(string path)
    {
        return $"\"{path}\"";
    }
}

internal sealed class SteplerSettings
{
    public bool PauseEnabled { get; set; } = true;
    public bool ScrollLockEnabled { get; set; } = true;
    public bool CtrlLayoutSwitchEnabled { get; set; } = true;
    public bool MenuCapsSwitchEnabled { get; set; } = true;
    public bool RiskyFallbacksEnabled { get; set; }
}

internal static class SteplerSettingsStore
{
    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        WriteIndented = true,
    };

    public static string SettingsPath()
    {
        var appData = Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData);
        return Path.Combine(appData, "Stepler", "settings.json");
    }

    public static SteplerSettings Load()
    {
        try
        {
            var path = SettingsPath();
            if (!File.Exists(path))
            {
                var defaults = new SteplerSettings();
                Save(defaults);
                return defaults;
            }

            return JsonSerializer.Deserialize<SteplerSettings>(File.ReadAllText(path), JsonOptions)
                ?? new SteplerSettings();
        }
        catch (Exception error)
        {
            File.AppendAllText(Program.LogPath(), $"{DateTimeOffset.Now:o} settings load error {error}{Environment.NewLine}");
            return new SteplerSettings();
        }
    }

    public static void Save(SteplerSettings settings)
    {
        var path = SettingsPath();
        var directory = Path.GetDirectoryName(path);
        if (!string.IsNullOrWhiteSpace(directory))
        {
            Directory.CreateDirectory(directory);
        }

        File.WriteAllText(path, JsonSerializer.Serialize(settings, JsonOptions));
    }
}

internal static class ProcessStartInfoExtensions
{
    public static ProcessStartInfo WithSteplerSettings(
        this ProcessStartInfo startInfo,
        SteplerSettings settings)
    {
        startInfo.Environment["STEPLER_ENABLE_PAUSE"] = Bool(settings.PauseEnabled);
        startInfo.Environment["STEPLER_ENABLE_SCROLLLOCK"] = Bool(settings.ScrollLockEnabled);
        startInfo.Environment["STEPLER_ENABLE_CTRL_LAYOUT"] = Bool(settings.CtrlLayoutSwitchEnabled);
        startInfo.Environment["STEPLER_ENABLE_MENU_CAPS_LAYOUT"] = Bool(settings.MenuCapsSwitchEnabled);
        if (settings.RiskyFallbacksEnabled)
        {
            startInfo.Environment["STEPLER_ALLOW_RISKY_FALLBACKS"] = "1";
        }
        else
        {
            startInfo.Environment.Remove("STEPLER_ALLOW_RISKY_FALLBACKS");
        }

        return startInfo;
    }

    private static string Bool(bool value)
    {
        return value ? "1" : "0";
    }
}

internal static class SteplerIcon
{
    public static Icon Create()
    {
        using var bitmap = new Bitmap(32, 32);
        using var graphics = Graphics.FromImage(bitmap);
        graphics.SmoothingMode = System.Drawing.Drawing2D.SmoothingMode.AntiAlias;
        graphics.Clear(Color.Transparent);

        using var backgroundBrush = new SolidBrush(Color.FromArgb(28, 32, 36));
        using var accentBrush = new SolidBrush(Color.FromArgb(35, 211, 170));
        using var font = new Font("Segoe UI", 18, FontStyle.Bold, GraphicsUnit.Pixel);
        using var textFormat = new StringFormat
        {
            Alignment = StringAlignment.Center,
            LineAlignment = StringAlignment.Center,
        };

        graphics.FillEllipse(backgroundBrush, 2, 2, 28, 28);
        graphics.DrawString("S", font, accentBrush, new RectangleF(1, 1, 30, 29), textFormat);

        var handle = bitmap.GetHicon();
        try
        {
            using var icon = Icon.FromHandle(handle);
            return (Icon)icon.Clone();
        }
        finally
        {
            NativeMethods.DestroyIcon(handle);
        }
    }
}

internal static class NativeMethods
{
    [System.Runtime.InteropServices.DllImport("user32.dll", SetLastError = true)]
    public static extern bool DestroyIcon(IntPtr handle);

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern IntPtr CreateJobObject(IntPtr lpJobAttributes, string? lpName);

    [DllImport("kernel32.dll", SetLastError = true)]
    public static extern bool SetInformationJobObject(
        IntPtr hJob,
        int jobObjectInfoClass,
        ref JOBOBJECT_EXTENDED_LIMIT_INFORMATION lpJobObjectInfo,
        int cbJobObjectInfoLength);

    [DllImport("kernel32.dll", SetLastError = true)]
    public static extern bool AssignProcessToJobObject(IntPtr hJob, IntPtr hProcess);

    [DllImport("kernel32.dll", SetLastError = true)]
    public static extern bool CloseHandle(IntPtr hObject);
}

[StructLayout(LayoutKind.Sequential)]
internal struct JOBOBJECT_BASIC_LIMIT_INFORMATION
{
    public long PerProcessUserTimeLimit;
    public long PerJobUserTimeLimit;
    public int LimitFlags;
    public nuint MinimumWorkingSetSize;
    public nuint MaximumWorkingSetSize;
    public int ActiveProcessLimit;
    public nuint Affinity;
    public int PriorityClass;
    public int SchedulingClass;
}

[StructLayout(LayoutKind.Sequential)]
internal struct IO_COUNTERS
{
    public ulong ReadOperationCount;
    public ulong WriteOperationCount;
    public ulong OtherOperationCount;
    public ulong ReadTransferCount;
    public ulong WriteTransferCount;
    public ulong OtherTransferCount;
}

[StructLayout(LayoutKind.Sequential)]
internal struct JOBOBJECT_EXTENDED_LIMIT_INFORMATION
{
    public JOBOBJECT_BASIC_LIMIT_INFORMATION BasicLimitInformation;
    public IO_COUNTERS IoInfo;
    public nuint ProcessMemoryLimit;
    public nuint JobMemoryLimit;
    public nuint PeakProcessMemoryUsed;
    public nuint PeakJobMemoryUsed;
}

internal sealed class RunnerJob : IDisposable
{
    private const int JobObjectExtendedLimitInformation = 9;
    private const int JobObjectLimitKillOnJobClose = 0x00002000;
    private IntPtr _handle;

    private RunnerJob(IntPtr handle)
    {
        _handle = handle;
    }

    public static RunnerJob CreateKillOnClose()
    {
        var handle = NativeMethods.CreateJobObject(IntPtr.Zero, null);
        if (handle == IntPtr.Zero)
        {
            throw new InvalidOperationException($"CreateJobObject failed: {Marshal.GetLastWin32Error()}");
        }

        var info = new JOBOBJECT_EXTENDED_LIMIT_INFORMATION();
        info.BasicLimitInformation.LimitFlags = JobObjectLimitKillOnJobClose;
        var size = Marshal.SizeOf<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>();
        if (!NativeMethods.SetInformationJobObject(handle, JobObjectExtendedLimitInformation, ref info, size))
        {
            var error = Marshal.GetLastWin32Error();
            NativeMethods.CloseHandle(handle);
            throw new InvalidOperationException($"SetInformationJobObject failed: {error}");
        }

        return new RunnerJob(handle);
    }

    public void Assign(Process process)
    {
        if (_handle == IntPtr.Zero)
        {
            throw new ObjectDisposedException(nameof(RunnerJob));
        }

        if (!NativeMethods.AssignProcessToJobObject(_handle, process.Handle))
        {
            throw new InvalidOperationException($"AssignProcessToJobObject failed: {Marshal.GetLastWin32Error()}");
        }
    }

    public void Dispose()
    {
        var handle = Interlocked.Exchange(ref _handle, IntPtr.Zero);
        if (handle != IntPtr.Zero)
        {
            NativeMethods.CloseHandle(handle);
        }
    }
}

internal sealed class ControlWindow : Form
{
    private readonly Label _statusLabel;

    public ControlWindow(
        string status,
        Action toggleRunner,
        Action restartRunner,
        Action openHotkeyLog,
        Action openTrayLog,
        Action exit)
    {
        Text = "Stepler";
        ShowInTaskbar = true;
        FormBorderStyle = FormBorderStyle.FixedDialog;
        MaximizeBox = false;
        MinimizeBox = false;
        ClientSize = new Size(304, 178);
        StartPosition = FormStartPosition.Manual;

        var area = Screen.PrimaryScreen?.WorkingArea ?? new Rectangle(0, 0, 800, 600);
        Location = new Point(Math.Max(0, area.Right - Width - 16), Math.Max(0, area.Bottom - Height - 16));

        _statusLabel = new Label
        {
            AutoSize = false,
            Text = status,
            TextAlign = ContentAlignment.MiddleLeft,
            Location = new Point(16, 12),
            Size = new Size(280, 28),
        };

        var toggleButton = Button("Вкл/выкл", 16, 52, toggleRunner);
        var restartButton = Button("Перезапустить", 160, 52, restartRunner);
        var hotkeyLogButton = Button("Лог hotkeys", 16, 92, openHotkeyLog);
        var trayLogButton = Button("Лог tray", 160, 92, openTrayLog);
        var exitButton = Button("Выход", 88, 134, exit);

        Controls.AddRange(new Control[]
        {
            _statusLabel,
            toggleButton,
            restartButton,
            hotkeyLogButton,
            trayLogButton,
            exitButton,
        });
    }

    public void UpdateStatus(string status)
    {
        _statusLabel.Text = status;
    }

    private static Button Button(string text, int x, int y, Action action)
    {
        var button = new Button
        {
            Text = text,
            Location = new Point(x, y),
            Size = new Size(128, 30),
        };
        button.Click += (_, _) => action();
        return button;
    }
}
