using System.Diagnostics;
using System.Drawing;
using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json;
using System.Windows.Forms;
using Microsoft.Win32;
using Stepler.Shared;

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
            SafeLog($"tray process start pid={Environment.ProcessId}");
            using var mutex = new Mutex(initiallyOwned: true, "Stepler.TrayHost", out var ownsMutex);
            if (!ownsMutex)
            {
                SafeLog($"tray already running pid={Environment.ProcessId}");
                return;
            }

            SafeLog($"tray main start pid={Environment.ProcessId}");
            Application.SetUnhandledExceptionMode(UnhandledExceptionMode.CatchException);
            ApplicationConfiguration.Initialize();
            Application.ThreadException += (_, error) =>
                SafeLog($"tray thread exception {error.Exception}");
            AppDomain.CurrentDomain.UnhandledException += (_, error) =>
                SafeLog($"tray unhandled exception terminating={error.IsTerminating} {error.ExceptionObject}");
            Application.ApplicationExit += (_, _) =>
                SafeLog("tray application exit");
            Application.Run(new SteplerTrayForm());
            SafeLog("tray main stop");
        }
        catch (Exception error)
        {
            SafeLog($"tray fatal {error}");
        }
    }

    internal static void SafeLog(string message)
    {
        var line = $"{DateTimeOffset.Now:o} {message}{Environment.NewLine}";
        try
        {
            File.AppendAllText(LogPath(), line);
        }
        catch
        {
            try
            {
                var fallbackPath = Path.Combine(Path.GetTempPath(), "Stepler.Tray.fallback.log");
                File.AppendAllText(fallbackPath, line);
            }
            catch
            {
                // Last-resort diagnostic logging must never crash the tray host.
            }
        }
    }

    internal static string LogPath()
    {
        return Path.Combine(LogDirectory(), "Stepler.Tray.log");
    }

    internal static string HotkeyLogPath()
    {
        return Path.Combine(LogDirectory(), "stepler_hotkey_log.jsonl");
    }

    internal static string LogDirectory()
    {
        var directory = Path.Combine(StateDirectory(), "logs");
        Directory.CreateDirectory(directory);
        return directory;
    }

    internal static string StateDirectory()
    {
        var localAppData = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
        var directory = Path.Combine(localAppData, "Stepler");
        Directory.CreateDirectory(directory);
        return directory;
    }

    private static void StopExistingProcesses()
    {
        var currentId = Environment.ProcessId;
        SafeLog($"stop requested current={currentId}");
        foreach (var processName in new[] { "Stepler", "Stepler.Tray", "stepler-cli" })
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
                    SafeLog($"stopped {processName} pid={process.Id}");
                }
                catch (Exception error)
                {
                    SafeLog($"stop failed {processName} pid={process.Id} {error.GetType().Name}");
                    // Best-effort emergency stop.
                }
            }
        }
    }
}

internal sealed class SteplerTrayForm : Form
{
    private readonly NotifyIcon _notifyIcon;
    private readonly ContextMenuStrip _trayMenu;
    private readonly Icon _appIcon;
    private readonly string? _repoRoot;
    private readonly ToolStripMenuItem _versionItem;
    private readonly ToolStripMenuItem _statusItem;
    private readonly ToolStripMenuItem _toggleItem;
    private readonly ToolStripMenuItem _restartItem;
    private readonly ToolStripMenuItem _pauseItem;
    private readonly ToolStripMenuItem _scrollLockItem;
    private readonly ToolStripMenuItem _ctrlLayoutItem;
    private readonly ToolStripMenuItem _menuCapsLayoutItem;
    private readonly ToolStripMenuItem _disableCapsLockItem;
    private readonly ToolStripMenuItem _insertAsBackspaceItem;
    private readonly ToolStripMenuItem _riskyFallbacksItem;
    private readonly ToolStripMenuItem _darkThemeItem;
    private readonly ToolStripMenuItem _showTimingOverlayItem;
    private readonly ToolStripMenuItem _timingOverlayDurationItem;
    private readonly ToolStripMenuItem _autostartItem;
    private readonly ToolStripMenuItem _qwenInputItem;
    private readonly ToolStripMenuItem _qwenWorkspaceItem;
    private readonly ToolStripMenuItem _qwenWorkspaceContinueItem;
    private readonly ToolStripMenuItem _qwenWorkspaceDirectoryItem;
    private readonly ToolStripMenuItem _openLayoutOverridesItem;
    private readonly ToolStripMenuItem _openHotkeyLogItem;
    private readonly ToolStripMenuItem _openTrayLogItem;
    private ControlWindow? _controlWindow;
    private QwenInputWindow? _qwenInputWindow;
    private HotkeyTimingOverlay? _timingOverlay;
    private FileSystemWatcher? _hotkeyLogWatcher;
    private readonly System.Windows.Forms.Timer _embeddedTerminalAckTimer;
    private Process? _runner;
    private RunnerJob? _runnerJob;
    private SteplerSettings _settings;
    private long _hotkeyLogPosition;
    private bool _stoppingRunner;
    private bool _closing;
    private string? _embeddedTerminalPendingLabel;

    public SteplerTrayForm()
    {
        _repoRoot = FindRepoRoot();
        _settings = SteplerSettingsStore.Load();
        Program.SafeLog($"settings loaded path={SteplerSettingsStore.SettingsPath()} {JsonSerializer.Serialize(_settings)}");
        _embeddedTerminalAckTimer = new System.Windows.Forms.Timer { Interval = 2000 };
        _embeddedTerminalAckTimer.Tick += (_, _) => ShowEmbeddedTerminalAckTimeout();

        Text = "Stepler";
        ShowInTaskbar = false;
        WindowState = FormWindowState.Minimized;
        FormBorderStyle = FormBorderStyle.FixedToolWindow;
        StartPosition = FormStartPosition.Manual;
        Location = new Point(-32000, -32000);
        Size = new Size(1, 1);
        Opacity = 0;

        _appIcon = SteplerIcon.Create();

        _versionItem = new ToolStripMenuItem($"Версия: {Application.ProductVersion}")
        {
            Enabled = false,
        };

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

        _scrollLockItem = new ToolStripMenuItem("Ctrl+Pause (умная строка)")
        {
            CheckOnClick = true,
        };
        _scrollLockItem.Click += (_, _) => UpdateSetting(settings => settings.ScrollLockEnabled = _scrollLockItem.Checked);

        _ctrlLayoutItem = new ToolStripMenuItem("Left/Right Ctrl: RU/EN")
        {
            CheckOnClick = true,
        };
        _ctrlLayoutItem.Click += (_, _) => UpdateSetting(settings => settings.CtrlLayoutSwitchEnabled = _ctrlLayoutItem.Checked);

        _menuCapsLayoutItem = new ToolStripMenuItem("Menu: следующая раскладка")
        {
            CheckOnClick = true,
        };
        _menuCapsLayoutItem.Click += (_, _) => UpdateSetting(settings => settings.MenuCapsSwitchEnabled = _menuCapsLayoutItem.Checked);

        _disableCapsLockItem = new ToolStripMenuItem("Отключить CapsLock")
        {
            CheckOnClick = true,
        };
        _disableCapsLockItem.Click += (_, _) => UpdateSetting(settings => settings.DisableCapsLock = _disableCapsLockItem.Checked);

        _insertAsBackspaceItem = new ToolStripMenuItem("Insert как Backspace")
        {
            CheckOnClick = true,
        };
        _insertAsBackspaceItem.Click += (_, _) => UpdateSetting(settings => settings.InsertAsBackspaceEnabled = _insertAsBackspaceItem.Checked);

        _riskyFallbacksItem = new ToolStripMenuItem("Risky fallback adapters")
        {
            CheckOnClick = true,
        };
        _riskyFallbacksItem.Click += (_, _) => UpdateSetting(settings => settings.RiskyFallbacksEnabled = _riskyFallbacksItem.Checked);

        _darkThemeItem = new ToolStripMenuItem("Темная тема")
        {
            CheckOnClick = true,
        };
        _darkThemeItem.Click += (_, _) =>
            UpdateSetting(settings => settings.DarkTheme = _darkThemeItem.Checked, restartRunner: false);

        _showTimingOverlayItem = new ToolStripMenuItem("Показывать время P/CP")
        {
            CheckOnClick = true,
        };
        _showTimingOverlayItem.Click += (_, _) =>
            UpdateSetting(settings => settings.ShowTimingOverlay = _showTimingOverlayItem.Checked, restartRunner: false);

        _timingOverlayDurationItem = new ToolStripMenuItem("Время индикатора...");
        _timingOverlayDurationItem.Click += (_, _) => ShowTimingOverlayDurationDialog();

        _autostartItem = new ToolStripMenuItem("Автозапуск Windows");
        _autostartItem.Click += (_, _) => ToggleAutostart();

        _qwenInputItem = new ToolStripMenuItem("Qwen input...");
        _qwenInputItem.Click += (_, _) => RunAfterMenuClose(ShowQwenInputWindow);

        _qwenWorkspaceItem = new ToolStripMenuItem("Qwen workspace...");
        _qwenWorkspaceItem.Click += (_, _) => RunAfterMenuClose(() => LaunchQwenWorkspace());

        _qwenWorkspaceContinueItem = new ToolStripMenuItem("Qwen workspace (--continue)");
        _qwenWorkspaceContinueItem.Click += (_, _) => RunAfterMenuClose(() => LaunchQwenWorkspace("--continue"));

        _qwenWorkspaceDirectoryItem = new ToolStripMenuItem("Папка проекта Qwen...");
        _qwenWorkspaceDirectoryItem.Click += (_, _) => RunAfterMenuClose(ShowQwenWorkspaceDirectoryDialog);

        _openLayoutOverridesItem = new ToolStripMenuItem("Открыть словарь исключений CP");
        _openLayoutOverridesItem.Click += (_, _) => OpenLayoutOverrides();

        _openHotkeyLogItem = new ToolStripMenuItem("Открыть лог hotkeys");
        _openHotkeyLogItem.Click += (_, _) => OpenHotkeyLog();

        _openTrayLogItem = new ToolStripMenuItem("Открыть лог tray");
        _openTrayLogItem.Click += (_, _) => OpenFileInNotepad(Program.LogPath());

        var showItem = new ToolStripMenuItem("Показать окно управления");
        showItem.Click += (_, _) => BeginInvoke((Action)ShowControlWindow);

        var exitItem = new ToolStripMenuItem("Выход");
        exitItem.Click += (_, _) => Close();

        var menu = new ContextMenuStrip();
        menu.Items.Add(_versionItem);
        menu.Items.Add(_statusItem);
        menu.Items.Add(new ToolStripSeparator());
        menu.Items.Add(_toggleItem);
        menu.Items.Add(_restartItem);
        menu.Items.Add(new ToolStripSeparator());
        menu.Items.Add(_pauseItem);
        menu.Items.Add(_scrollLockItem);
        menu.Items.Add(_ctrlLayoutItem);
        menu.Items.Add(_menuCapsLayoutItem);
        menu.Items.Add(_disableCapsLockItem);
        menu.Items.Add(_insertAsBackspaceItem);
        menu.Items.Add(_riskyFallbacksItem);
        menu.Items.Add(_darkThemeItem);
        menu.Items.Add(_showTimingOverlayItem);
        menu.Items.Add(_timingOverlayDurationItem);
        menu.Items.Add(new ToolStripSeparator());
        menu.Items.Add(_autostartItem);
        menu.Items.Add(_qwenInputItem);
        menu.Items.Add(_qwenWorkspaceItem);
        menu.Items.Add(_qwenWorkspaceContinueItem);
        menu.Items.Add(_qwenWorkspaceDirectoryItem);
        menu.Items.Add(new ToolStripSeparator());
        menu.Items.Add(_openLayoutOverridesItem);
        menu.Items.Add(_openHotkeyLogItem);
        menu.Items.Add(_openTrayLogItem);
        menu.Items.Add(new ToolStripSeparator());
        menu.Items.Add(showItem);
        menu.Items.Add(exitItem);
        menu.Opening += (_, _) => UpdateMenuState();
        ThemeApplier.Apply(menu, ThemePalette.FromDarkTheme(_settings.DarkTheme));
        _trayMenu = menu;

        _notifyIcon = new NotifyIcon
        {
            Icon = _appIcon,
            Text = "Stepler",
        };
        _notifyIcon.MouseUp += (_, eventArgs) =>
        {
            if (eventArgs.Button is MouseButtons.Left or MouseButtons.Right)
            {
                ShowTrayMenu();
            }
        };

        Load += (_, _) =>
        {
            _notifyIcon.Visible = true;
            EnsurePowerShellProfileAdapter();
            StartHotkeyLogWatcher();
            StartRunner();
            Program.SafeLog($"tray form ready handle={Handle}");
        };
        FormClosed += (_, _) =>
            Program.SafeLog("tray form closed");
        Shown += (_, _) => Hide();
    }

    protected override void OnFormClosing(FormClosingEventArgs e)
    {
        _closing = true;
        StopHotkeyLogWatcher();
        _embeddedTerminalAckTimer.Stop();
        _embeddedTerminalAckTimer.Dispose();
        _timingOverlay?.Close();
        _timingOverlay?.Dispose();
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
            ShowAndFocusControlWindow(_controlWindow);
            return;
        }

        _controlWindow = new ControlWindow(
            _statusItem.Text ?? "Stepler",
            ToggleRunner,
            RestartRunner,
            OpenHotkeyLog,
            () => OpenFileInNotepad(Program.LogPath()),
            Close,
            _settings.DarkTheme);
        ShowAndFocusControlWindow(_controlWindow);
    }

    private void ShowQwenInputWindow()
    {
        if (_qwenInputWindow is { IsDisposed: false })
        {
            ShowAndFocusWindow(_qwenInputWindow);
            return;
        }

        _qwenInputWindow = new QwenInputWindow(
            ResolveCliPath(),
            _settings.DarkTheme,
            (text, failed) =>
            {
                if (_settings.ShowTimingOverlay)
                {
                    ShowTimingOverlay(text, failed);
                }
            });
        ShowAndFocusWindow(_qwenInputWindow);
    }

    private void LaunchQwenWorkspace(params string[] arguments)
    {
        var workspacePath = ResolveQwenWorkspacePath();
        if (!File.Exists(workspacePath))
        {
            Program.SafeLog($"qwen workspace not found path={workspacePath}");
            return;
        }

        try
        {
            var startInfo = new ProcessStartInfo
            {
                FileName = workspacePath,
                WorkingDirectory = Path.GetDirectoryName(workspacePath) ?? AppContext.BaseDirectory,
                UseShellExecute = true,
            };
            startInfo.ArgumentList.Add("--workdir");
            startInfo.ArgumentList.Add(ResolveQwenWorkspaceWorkingDirectory());
            startInfo.ArgumentList.Add("--dark-theme");
            startInfo.ArgumentList.Add(_settings.DarkTheme ? "true" : "false");
            foreach (var argument in arguments)
            {
                startInfo.ArgumentList.Add(argument);
            }
            Process.Start(startInfo);
        }
        catch (Exception error)
        {
            Program.SafeLog($"qwen workspace launch error {error}");
        }
    }

    private void ShowQwenWorkspaceDirectoryDialog()
    {
        using var dialog = new FolderBrowserDialog
        {
            Description = "Выбери папку проекта, из которой запускать Qwen workspace",
            UseDescriptionForTitle = true,
            SelectedPath = ResolveQwenWorkspaceWorkingDirectory(),
            ShowNewFolderButton = false,
        };

        if (dialog.ShowDialog(this) != DialogResult.OK)
        {
            return;
        }

        UpdateSetting(
            settings => settings.QwenWorkspaceDirectory = dialog.SelectedPath,
            restartRunner: false);
    }

    private void ShowTrayMenu()
    {
        UpdateMenuState();
        NativeMethods.SetForegroundWindow(Handle);
        _trayMenu.Show(Cursor.Position);
    }

    private void RunAfterMenuClose(Action action)
    {
        var timer = new System.Windows.Forms.Timer
        {
            Interval = 75,
        };
        timer.Tick += (_, _) =>
        {
            timer.Stop();
            timer.Dispose();
            action();
        };
        timer.Start();
    }

    private static void ShowAndFocusControlWindow(ControlWindow window)
    {
        ShowAndFocusWindow(window);
    }

    private static void ShowAndFocusWindow(Form window)
    {
        if (window.WindowState == FormWindowState.Minimized)
        {
            window.WindowState = FormWindowState.Normal;
        }

        window.Show();
        window.TopMost = true;
        window.TopMost = false;
        window.BringToFront();
        window.Activate();

        var timer = new System.Windows.Forms.Timer
        {
            Interval = 100,
        };
        timer.Tick += (_, _) =>
        {
            timer.Stop();
            timer.Dispose();
            if (!window.IsDisposed)
            {
                window.Show();
                window.BringToFront();
                window.Activate();
            }
        };
        timer.Start();
    }

    private void StartRunner()
    {
        if (IsRunnerAlive())
        {
            SetStatus("Статус: работает");
            return;
        }

        var cliPath = ResolveCliPath();

        if (!File.Exists(cliPath))
        {
            SetStatus("Статус: ошибка - stepler-cli.exe не найден");
            Program.SafeLog("cli not found");
            return;
        }

        var workingDirectory = Path.GetDirectoryName(cliPath) ?? AppContext.BaseDirectory;
        try
        {
            StopOrphanRunners(cliPath);
            _stoppingRunner = false;
            _runner = Process.Start(new ProcessStartInfo
            {
                FileName = cliPath,
                Arguments = "run-hotkeys",
                WorkingDirectory = workingDirectory,
                UseShellExecute = false,
                CreateNoWindow = true,
                WindowStyle = ProcessWindowStyle.Hidden,
            }.WithSteplerSettings(_settings).WithSteplerRuntimePaths());

            if (_runner is not null)
            {
                _runner.EnableRaisingEvents = true;
                _runner.Exited += (_, _) => OnRunnerExited();
                TryAttachRunnerJob(_runner);
            }

            SetStatus(_runner is null ? "Статус: ошибка запуска" : "Статус: работает");
            UpdateMenuState();
            Program.SafeLog($"runner started pid={_runner?.Id.ToString() ?? "null"} cli={cliPath} cwd={workingDirectory}");
        }
        catch (Exception error)
        {
            Program.SafeLog($"runner start error {error}");
            SetStatus($"Статус: ошибка запуска ({error.GetType().Name})");
            UpdateMenuState();
        }
    }

    private void EnsurePowerShellProfileAdapter()
    {
        try
        {
            var adapterPath = Path.Combine(AppContext.BaseDirectory, "scripts", "Stepler.PSReadLine.ps1");
            if (!File.Exists(adapterPath) && _repoRoot is not null)
            {
                adapterPath = Path.Combine(_repoRoot, "scripts", "Stepler.PSReadLine.ps1");
            }

            if (!File.Exists(adapterPath))
            {
                Program.SafeLog("psreadline profile skip adapter not found");
                return;
            }

            var ensuredProfiles = PowerShellProfileManager.EnsureInstalled(adapterPath, ResolveCliPath());
            if (ensuredProfiles == 0)
            {
                Program.SafeLog("psreadline profile unavailable: no writable profile path");
                return;
            }

            Program.SafeLog($"psreadline profile ensured paths={ensuredProfiles} adapter={adapterPath} cli={ResolveCliPath()}");
        }
        catch (Exception error)
        {
            Program.SafeLog($"psreadline profile install error {error}");
        }
    }

    private void TryAttachRunnerJob(Process runner)
    {
        try
        {
            _runnerJob?.Dispose();
            _runnerJob = RunnerJob.CreateKillOnClose();
            _runnerJob.Assign(runner);
            Program.SafeLog($"runner job attached pid={runner.Id}");
        }
        catch (Exception error)
        {
            _runnerJob?.Dispose();
            _runnerJob = null;
            Program.SafeLog($"runner job attach failed pid={runner.Id} {error}");
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

        Program.SafeLog($"runner exited code={exitCode}");
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
                    if (!_stoppingRunner && !_closing)
                    {
                        SetStatus("Статус: обработчик упал, перезапуск...");
                        UpdateMenuState();
                        Program.SafeLog($"runner auto restart after exit code={exitCode}");
                        StartRunner();
                    }
                    else
                    {
                        SetStatus("Статус: обработчик остановлен");
                        UpdateMenuState();
                    }
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
            _stoppingRunner = true;
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

    private void UpdateSetting(Action<SteplerSettings> update, bool restartRunner = true)
    {
        update(_settings);
        SteplerSettingsStore.Save(_settings);
        Program.SafeLog($"settings saved {JsonSerializer.Serialize(_settings)}");
        ApplyCurrentTheme();

        if (restartRunner && IsRunnerAlive())
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
        _disableCapsLockItem.Checked = _settings.DisableCapsLock;
        _insertAsBackspaceItem.Checked = _settings.InsertAsBackspaceEnabled;
        _riskyFallbacksItem.Checked = _settings.RiskyFallbacksEnabled;
        _darkThemeItem.Checked = _settings.DarkTheme;
        _showTimingOverlayItem.Checked = _settings.ShowTimingOverlay;
        _timingOverlayDurationItem.Text = $"Время индикатора: {_settings.TimingOverlayDurationMs} ms";
        _autostartItem.Checked = AutostartManager.IsEnabled();
        _qwenWorkspaceDirectoryItem.Text = $"Папка проекта Qwen: {ShortPath(ResolveQwenWorkspaceWorkingDirectory())}";
    }

    private void StartHotkeyLogWatcher()
    {
        StopHotkeyLogWatcher();

        var logPath = Program.HotkeyLogPath();
        var directory = Path.GetDirectoryName(logPath);
        if (string.IsNullOrWhiteSpace(directory))
        {
            return;
        }

        Directory.CreateDirectory(directory);
        if (!File.Exists(logPath))
        {
            File.WriteAllText(logPath, string.Empty);
        }

        _hotkeyLogPosition = new FileInfo(logPath).Length;
        _hotkeyLogWatcher = new FileSystemWatcher(directory, Path.GetFileName(logPath))
        {
            NotifyFilter = NotifyFilters.LastWrite | NotifyFilters.Size | NotifyFilters.FileName,
            EnableRaisingEvents = true,
        };
        _hotkeyLogWatcher.Changed += (_, _) => BeginInvoke((Action)ReadNewHotkeyLogLines);
        _hotkeyLogWatcher.Created += (_, _) => BeginInvoke((Action)ReadNewHotkeyLogLines);
        Program.SafeLog($"hotkey timing watcher started path={logPath} position={_hotkeyLogPosition}");
    }

    private void StopHotkeyLogWatcher()
    {
        if (_hotkeyLogWatcher is null)
        {
            return;
        }

        _hotkeyLogWatcher.EnableRaisingEvents = false;
        _hotkeyLogWatcher.Dispose();
        _hotkeyLogWatcher = null;
    }

    private void ReadNewHotkeyLogLines()
    {
        if (!_settings.ShowTimingOverlay)
        {
            SyncHotkeyLogPosition();
            return;
        }

        var logPath = Program.HotkeyLogPath();
        try
        {
            if (!File.Exists(logPath))
            {
                _hotkeyLogPosition = 0;
                return;
            }

            using var stream = new FileStream(logPath, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete);
            if (stream.Length < _hotkeyLogPosition)
            {
                _hotkeyLogPosition = 0;
            }

            stream.Seek(_hotkeyLogPosition, SeekOrigin.Begin);
            using var reader = new StreamReader(stream);
            string? line;
            while ((line = reader.ReadLine()) is not null)
            {
                if (TryHandleEmbeddedTerminalTiming(line))
                {
                    continue;
                }

                if (TryFormatHotkeyTiming(line, out var text, out var failed))
                {
                    ShowTimingOverlay(text, failed);
                }
            }

            _hotkeyLogPosition = stream.Position;
        }
        catch (Exception error)
        {
            Program.SafeLog($"hotkey timing watcher read error {error.GetType().Name}: {error.Message}");
        }
    }

    private void SyncHotkeyLogPosition()
    {
        try
        {
            var logPath = Program.HotkeyLogPath();
            _hotkeyLogPosition = File.Exists(logPath) ? new FileInfo(logPath).Length : 0;
        }
        catch
        {
            _hotkeyLogPosition = 0;
        }
    }

    private void ShowTimingOverlay(string text, bool failed)
    {
        if (_timingOverlay is null || _timingOverlay.IsDisposed)
        {
            _timingOverlay = new HotkeyTimingOverlay();
        }

        _timingOverlay.ShowTiming(text, failed, _settings.TimingOverlayDurationMs, _settings.DarkTheme);
    }

    private bool TryHandleEmbeddedTerminalTiming(string line)
    {
        if (string.IsNullOrWhiteSpace(line))
        {
            return false;
        }

        try
        {
            using var document = JsonDocument.Parse(line);
            var root = document.RootElement;
            var label = TryGetHotkeyLabel(root);
            if (label is null)
            {
                return false;
            }

            if (IsEmbeddedTerminalForward(root))
            {
                _embeddedTerminalPendingLabel = label;
                _embeddedTerminalAckTimer.Stop();
                _embeddedTerminalAckTimer.Start();
                ShowTimingOverlay($"{label} ожидает обработчик", failed: false);
                return true;
            }

            if (!string.Equals(_embeddedTerminalPendingLabel, label, StringComparison.Ordinal)
                || !root.TryGetProperty("event", out var eventElement)
                || !string.Equals(eventElement.GetString(), "performance_operation_v1", StringComparison.Ordinal)
                || !root.TryGetProperty("context_method", out var contextMethod)
                || !string.Equals(contextMethod.GetString(), "psreadline", StringComparison.Ordinal))
            {
                return false;
            }

            _embeddedTerminalPendingLabel = null;
            _embeddedTerminalAckTimer.Stop();
            var outcome = root.TryGetProperty("outcome", out var outcomeElement)
                ? outcomeElement.GetString()
                : null;
            if (string.Equals(outcome, "Completed", StringComparison.Ordinal))
            {
                var duration = root.TryGetProperty("duration_ms", out var durationElement)
                    && durationElement.TryGetInt64(out var value)
                        ? value
                        : 0;
                ShowTimingOverlay($"{label} {duration} ms", failed: false);
            }
            else if (string.Equals(outcome, "NoChange", StringComparison.Ordinal))
            {
                ShowTimingOverlay($"{label} нечего менять", failed: false);
            }
            else
            {
                ShowTimingOverlay($"{label} failed", failed: true);
            }

            return true;
        }
        catch
        {
            return false;
        }
    }

    private void ShowEmbeddedTerminalAckTimeout()
    {
        _embeddedTerminalAckTimer.Stop();
        var label = _embeddedTerminalPendingLabel;
        _embeddedTerminalPendingLabel = null;
        if (label is not null)
        {
            ShowTimingOverlay($"{label} обработчик не загружен", failed: true);
        }
    }

    private void ApplyCurrentTheme()
    {
        var palette = ThemePalette.FromDarkTheme(_settings.DarkTheme);
        ThemeApplier.Apply(_trayMenu, palette);
        _controlWindow?.ApplyTheme(_settings.DarkTheme);
        _qwenInputWindow?.ApplyTheme(_settings.DarkTheme);
    }

    private static bool TryFormatHotkeyTiming(string line, out string text, out bool failed)
    {
        text = string.Empty;
        failed = false;

        if (string.IsNullOrWhiteSpace(line))
        {
            return false;
        }

        try
        {
            using var document = JsonDocument.Parse(line);
            var root = document.RootElement;
            if (!root.TryGetProperty("trigger", out var triggerElement))
            {
                return false;
            }

            var label = TryGetHotkeyLabel(root);
            if (label is null)
            {
                return false;
            }

            // Forwarding Ctrl+F11/F12 to the embedded terminal only means that
            // the chord was injected. A PSReadLine performance event confirms correction.
            if (IsEmbeddedTerminalForward(root))
            {
                return false;
            }

            // Performance records share the trigger field but use "outcome", not "state".
            // The timing overlay must only render OperationLogEvent records.
            if (!root.TryGetProperty("state", out var stateElement)
                || stateElement.ValueKind != JsonValueKind.String)
            {
                return false;
            }

            var state = stateElement.GetString();
            if (string.Equals(state, "HotkeyReceived", StringComparison.Ordinal))
            {
                failed = false;
                text = $"{label} нажата";
                return true;
            }
            if (string.Equals(state, "NoChange", StringComparison.Ordinal))
            {
                failed = false;
                text = $"{label} нечего менять";
                return true;
            }
            if (string.Equals(state, "Unsupported", StringComparison.Ordinal))
            {
                failed = false;
                text = $"{label} не поддерживается";
                return true;
            }

            failed = !string.Equals(state, "Completed", StringComparison.Ordinal);
            if (failed)
            {
                text = $"{label} failed";
                return true;
            }

            var duration = root.TryGetProperty("duration_ms", out var durationElement)
                && durationElement.TryGetInt64(out var value)
                    ? value
                    : 0;
            if (root.TryGetProperty("layout_result", out var layoutElement)
                && layoutElement.ValueKind == JsonValueKind.String
                && layoutElement.GetString()?.Contains("layout_failed_", StringComparison.Ordinal) == true)
            {
                text = $"{label} {duration} ms, язык не переключён";
                return true;
            }
            text = $"{label} {duration} ms";
            return true;
        }
        catch
        {
            return false;
        }
    }

    private static string? TryGetHotkeyLabel(JsonElement root)
    {
        if (!root.TryGetProperty("trigger", out var triggerElement))
        {
            return null;
        }

        return triggerElement.GetString() switch
        {
            "Pause" => "P",
            "ScrollLock" => "CP",
            _ => null,
        };
    }

    private static bool IsEmbeddedTerminalForward(JsonElement root)
    {
        return root.TryGetProperty("operation_id", out var operationId)
            && string.Equals(operationId.GetString(), "embedded-terminal", StringComparison.Ordinal)
            && root.TryGetProperty("app", out var app)
            && string.Equals(app.GetString(), "embedded_terminal", StringComparison.Ordinal)
            && root.TryGetProperty("replacer", out var replacer)
            && string.Equals(replacer.GetString(), "embedded_terminal_psreadline", StringComparison.Ordinal);
    }

    private void ShowTimingOverlayDurationDialog()
    {
        using var form = new Form
        {
            Text = "Время индикатора",
            ShowInTaskbar = false,
            FormBorderStyle = FormBorderStyle.FixedDialog,
            MaximizeBox = false,
            MinimizeBox = false,
            StartPosition = FormStartPosition.CenterScreen,
            ClientSize = new Size(260, 116),
        };
        var label = new Label
        {
            Text = "Показывать, мс:",
            Location = new Point(16, 18),
            Size = new Size(112, 24),
        };
        var input = new NumericUpDown
        {
            Minimum = 200,
            Maximum = 5000,
            Increment = 100,
            Value = Math.Clamp(_settings.TimingOverlayDurationMs, 200, 5000),
            Location = new Point(132, 16),
            Size = new Size(96, 24),
        };
        var ok = new Button
        {
            Text = "OK",
            DialogResult = DialogResult.OK,
            Location = new Point(54, 68),
            Size = new Size(72, 28),
        };
        var cancel = new Button
        {
            Text = "Отмена",
            DialogResult = DialogResult.Cancel,
            Location = new Point(136, 68),
            Size = new Size(72, 28),
        };
        form.Controls.AddRange(new Control[] { label, input, ok, cancel });
        ThemeApplier.Apply(form, ThemePalette.FromDarkTheme(_settings.DarkTheme));
        form.AcceptButton = ok;
        form.CancelButton = cancel;

        if (form.ShowDialog(this) == DialogResult.OK)
        {
            UpdateSetting(settings => settings.TimingOverlayDurationMs = (int)input.Value, restartRunner: false);
        }
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
            Program.SafeLog($"autostart error {error}");
            MessageBox.Show(
                "Не удалось изменить автозапуск. Подробности записаны в лог tray.",
                "Stepler",
                MessageBoxButtons.OK,
                MessageBoxIcon.Warning);
        }
    }

    private void OpenHotkeyLog()
    {
        OpenFileInNotepad(Program.HotkeyLogPath());
    }

    private void OpenLayoutOverrides()
    {
        var path = ResolveLayoutOverridesPath();
        try
        {
            var directory = Path.GetDirectoryName(path);
            if (!string.IsNullOrWhiteSpace(directory))
            {
                Directory.CreateDirectory(directory);
            }

            if (!File.Exists(path))
            {
                File.WriteAllText(
                    path,
                    "# source\ttarget" + Environment.NewLine +
                    "# Пример: ddble\tввиду" + Environment.NewLine);
            }

            OpenFileInNotepad(path);
        }
        catch (Exception error)
        {
            Program.SafeLog($"open layout overrides error path={path} {error}");
            MessageBox.Show(
                "Не удалось открыть словарь исключений. Подробности записаны в лог tray.",
                "Stepler",
                MessageBoxButtons.OK,
                MessageBoxIcon.Warning);
        }
    }

    private string ResolveLayoutOverridesPath()
    {
        var sideBySide = Path.Combine(AppContext.BaseDirectory, "resources", "layout-overrides.tsv");
        if (File.Exists(sideBySide) || _repoRoot is null)
        {
            return sideBySide;
        }

        var repoResource = Path.Combine(
            _repoRoot,
            "crates",
            "stepler-core",
            "resources",
            "layout-overrides.tsv");
        return File.Exists(repoResource) ? repoResource : sideBySide;
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
            Program.SafeLog($"open log error {error}");
        }
    }

    private static string? FindRepoRoot()
    {
        var configured = Environment.GetEnvironmentVariable("STEPLER_REPO_ROOT");
        if (!string.IsNullOrWhiteSpace(configured)
            && File.Exists(Path.Combine(configured, "Cargo.toml"))
            && Directory.Exists(Path.Combine(configured, "crates")))
        {
            return configured;
        }

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

        return null;
    }

    private string ResolveCliPath()
    {
        var configured = Environment.GetEnvironmentVariable("STEPLER_CLI_PATH");
        if (!string.IsNullOrWhiteSpace(configured))
        {
            return configured;
        }

        var sideBySide = Path.Combine(AppContext.BaseDirectory, "stepler-cli.exe");
        if (File.Exists(sideBySide))
        {
            return sideBySide;
        }

        if (_repoRoot is not null)
        {
            var releaseDist = Path.Combine(_repoRoot, "dist", "Stepler", "stepler-cli.exe");
            if (File.Exists(releaseDist))
            {
                return releaseDist;
            }
        }

        if (_repoRoot is not null)
        {
            return Path.Combine(_repoRoot, "target", "debug", "stepler-cli.exe");
        }

        return sideBySide;
    }

    private string ResolveQwenWorkspacePath()
    {
        var sideBySide = Path.Combine(AppContext.BaseDirectory, "Stepler.QwenWorkspace.exe");
        if (File.Exists(sideBySide))
        {
            return sideBySide;
        }

        if (_repoRoot is not null)
        {
            var releaseDist = Path.Combine(_repoRoot, "dist", "Stepler", "Stepler.QwenWorkspace.exe");
            if (File.Exists(releaseDist))
            {
                return releaseDist;
            }

            return Path.Combine(
                _repoRoot,
                "apps",
                "Stepler.QwenWorkspace",
                "bin",
                "Debug",
                "net9.0-windows",
                "Stepler.QwenWorkspace.exe");
        }

        return sideBySide;
    }

    private string ResolveQwenWorkspaceWorkingDirectory()
    {
        if (!string.IsNullOrWhiteSpace(_settings.QwenWorkspaceDirectory)
            && Directory.Exists(_settings.QwenWorkspaceDirectory))
        {
            return _settings.QwenWorkspaceDirectory;
        }

        var configured = Environment.GetEnvironmentVariable("STEPLER_QWEN_WORKDIR");
        if (!string.IsNullOrWhiteSpace(configured) && Directory.Exists(configured))
        {
            return configured;
        }

        return _repoRoot ?? Environment.GetFolderPath(Environment.SpecialFolder.UserProfile);
    }

    private static string ShortPath(string path)
    {
        if (path.Length <= 42)
        {
            return path;
        }

        var root = Path.GetPathRoot(path) ?? string.Empty;
        var name = Path.GetFileName(path.TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar));
        return string.IsNullOrWhiteSpace(name)
            ? path
            : $"{root}...\\{name}";
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

internal static class PowerShellProfileManager
{
    private const string BeginMarker = "# >>> Stepler PSReadLine adapter >>>";
    private const string EndMarker = "# <<< Stepler PSReadLine adapter <<<";

    public static int EnsureInstalled(string adapterPath, string cliPath)
    {
        var ensuredProfiles = 0;
        foreach (var profilePath in ProfilePaths())
        {
            try
            {
                EnsureProfileBlock(profilePath, adapterPath, cliPath);
                ensuredProfiles++;
            }
            catch (Exception error)
            {
                Program.SafeLog($"psreadline profile path skipped path={profilePath} error={error.GetType().Name}: {error.Message}");
            }
        }

        return ensuredProfiles;
    }

    private static IEnumerable<string> ProfilePaths()
    {
        var seen = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        foreach (var path in CandidateProfilePaths())
        {
            if (seen.Add(path))
            {
                yield return path;
            }
        }
    }

    private static IEnumerable<string> CandidateProfilePaths()
    {
        var myDocuments = Environment.GetFolderPath(Environment.SpecialFolder.MyDocuments);
        if (!string.IsNullOrWhiteSpace(myDocuments))
        {
            yield return Path.Combine(myDocuments, "PowerShell", "profile.ps1");
            yield return Path.Combine(myDocuments, "PowerShell", "Microsoft.PowerShell_profile.ps1");
            yield return Path.Combine(myDocuments, "WindowsPowerShell", "profile.ps1");
            yield return Path.Combine(myDocuments, "WindowsPowerShell", "Microsoft.PowerShell_profile.ps1");
        }

        var userProfile = Environment.GetFolderPath(Environment.SpecialFolder.UserProfile);
        if (!string.IsNullOrWhiteSpace(userProfile))
        {
            var documents = Path.Combine(userProfile, "Documents");
            yield return Path.Combine(documents, "PowerShell", "profile.ps1");
            yield return Path.Combine(documents, "PowerShell", "Microsoft.PowerShell_profile.ps1");
            yield return Path.Combine(documents, "WindowsPowerShell", "profile.ps1");
            yield return Path.Combine(documents, "WindowsPowerShell", "Microsoft.PowerShell_profile.ps1");
        }
    }

    private static void EnsureProfileBlock(string profilePath, string adapterPath, string cliPath)
    {
        var directory = Path.GetDirectoryName(profilePath);
        if (!string.IsNullOrWhiteSpace(directory))
        {
            Directory.CreateDirectory(directory);
        }

        var existing = File.Exists(profilePath) ? File.ReadAllText(profilePath) : string.Empty;
        var block = BuildProfileBlock(adapterPath, cliPath);
        var next = ReplaceManagedBlock(existing, block);
        if (!string.Equals(existing, next, StringComparison.Ordinal))
        {
            File.WriteAllText(profilePath, next);
        }
    }

    private static string ReplaceManagedBlock(string existing, string block)
    {
        var cleaned = RemoveManagedBlocks(existing);
        return JoinProfileParts(cleaned.TrimEnd(), block, string.Empty);
    }

    private static string RemoveManagedBlocks(string existing)
    {
        var cleaned = existing;
        while (true)
        {
            var begin = cleaned.IndexOf(BeginMarker, StringComparison.Ordinal);
            var end = cleaned.IndexOf(EndMarker, StringComparison.Ordinal);
            if (begin < 0 || end < begin)
            {
                return cleaned;
            }

            end += EndMarker.Length;
            cleaned = (cleaned[..begin].TrimEnd() + Environment.NewLine + cleaned[end..].TrimStart()).Trim();
        }
    }

    private static string JoinProfileParts(string before, string block, string after)
    {
        if (string.IsNullOrWhiteSpace(before) && string.IsNullOrWhiteSpace(after))
        {
            return block + Environment.NewLine;
        }

        if (string.IsNullOrWhiteSpace(after))
        {
            return before + Environment.NewLine + Environment.NewLine + block + Environment.NewLine;
        }

        if (string.IsNullOrWhiteSpace(before))
        {
            return block + Environment.NewLine + Environment.NewLine + after.TrimStart();
        }

        return before + Environment.NewLine + Environment.NewLine + block + Environment.NewLine + Environment.NewLine + after.TrimStart();
    }

    private static string BuildProfileBlock(string adapterPath, string cliPath)
    {
        var quotedAdapterPath = adapterPath.Replace("'", "''", StringComparison.Ordinal);
        var quotedCliPath = cliPath.Replace("'", "''", StringComparison.Ordinal);
        return string.Join(
            Environment.NewLine,
            BeginMarker,
            "try {",
            $"    $steplerPsReadLine = '{quotedAdapterPath}'",
            $"    $steplerCli = '{quotedCliPath}'",
            "    if (Test-Path -LiteralPath $steplerPsReadLine) {",
            "        Import-Module PSReadLine -ErrorAction SilentlyContinue",
            "        . $steplerPsReadLine -SteplerCli $steplerCli -Quiet",
            "    }",
            "} catch {",
            "}",
            EndMarker);
    }
}

internal sealed class HotkeyTimingOverlay : Form
{
    private const int WsExNoActivate = 0x08000000;
    private const int WsExToolWindow = 0x00000080;

    private readonly Label _label;
    private readonly System.Windows.Forms.Timer _hideTimer;

    public HotkeyTimingOverlay()
    {
        FormBorderStyle = FormBorderStyle.None;
        ShowInTaskbar = false;
        StartPosition = FormStartPosition.Manual;
        TopMost = true;
        BackColor = Color.FromArgb(28, 32, 36);
        Padding = new Padding(14, 8, 14, 8);
        AutoSize = true;
        AutoSizeMode = AutoSizeMode.GrowAndShrink;

        _label = new Label
        {
            AutoSize = true,
            Font = new Font("Segoe UI", 11, FontStyle.Bold, GraphicsUnit.Point),
            ForeColor = Color.White,
            BackColor = Color.Transparent,
            Text = "P 0 ms",
        };
        Controls.Add(_label);

        _hideTimer = new System.Windows.Forms.Timer
        {
            Interval = 1000,
        };
        _hideTimer.Tick += (_, _) =>
        {
            _hideTimer.Stop();
            Hide();
        };
    }

    protected override bool ShowWithoutActivation => true;

    protected override CreateParams CreateParams
    {
        get
        {
            var parameters = base.CreateParams;
            parameters.ExStyle |= WsExNoActivate | WsExToolWindow;
            return parameters;
        }
    }

    public void ShowTiming(string text, bool failed, int durationMs, bool darkTheme)
    {
        _label.Text = text;
        BackColor = failed
            ? Color.FromArgb(118, 33, 43)
            : darkTheme
                ? Color.FromArgb(28, 32, 36)
                : Color.FromArgb(245, 247, 250);
        _label.ForeColor = failed
            ? Color.FromArgb(255, 235, 238)
            : darkTheme
                ? Color.White
                : Color.FromArgb(30, 32, 36);

        PerformLayout();
        PositionNearTray();
        Show();
        _label.Refresh();
        Refresh();
        Update();

        _hideTimer.Stop();
        _hideTimer.Interval = Math.Clamp(durationMs, 200, 5000);
        _hideTimer.Start();
    }

    private void PositionNearTray()
    {
        var area = Screen.PrimaryScreen?.WorkingArea ?? Screen.FromControl(this).WorkingArea;
        var margin = 18;
        Location = new Point(
            Math.Max(area.Left + margin, area.Right - Width - margin),
            Math.Max(area.Top + margin, area.Bottom - Height - margin));
    }

    protected override void Dispose(bool disposing)
    {
        if (disposing)
        {
            _hideTimer.Dispose();
        }

        base.Dispose(disposing);
    }
}

internal sealed class SteplerSettings
{
    public bool PauseEnabled { get; set; } = true;
    public bool ScrollLockEnabled { get; set; } = true;
    public bool CtrlLayoutSwitchEnabled { get; set; } = true;
    public bool MenuCapsSwitchEnabled { get; set; } = true;
    public bool DisableCapsLock { get; set; } = true;
    public bool InsertAsBackspaceEnabled { get; set; } = true;
    public bool RiskyFallbacksEnabled { get; set; }
    public bool DarkTheme { get; set; } = true;
    public bool ShowTimingOverlay { get; set; } = true;
    public int TimingOverlayDurationMs { get; set; } = 1000;
    public string? QwenWorkspaceDirectory { get; set; }
}

internal readonly record struct ThemePalette(
    Color Window,
    Color Control,
    Color ControlHover,
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
                Color.FromArgb(55, 61, 69),
                Color.FromArgb(75, 82, 92),
                Color.FromArgb(245, 247, 250),
                Color.FromArgb(190, 197, 207),
                Color.FromArgb(22, 24, 27),
                Color.FromArgb(245, 247, 250))
            : new ThemePalette(
                SystemColors.Window,
                SystemColors.Control,
                SystemColors.ButtonFace,
                SystemColors.ControlDark,
                SystemColors.ControlText,
                SystemColors.GrayText,
                SystemColors.Window,
                SystemColors.WindowText);
    }
}

internal static class ThemeApplier
{
    public static void Apply(Form form, ThemePalette palette)
    {
        form.BackColor = palette.Window;
        form.ForeColor = palette.Text;
        NativeMethods.SetImmersiveDarkMode(form.Handle, palette.Window.GetBrightness() < 0.5f);
        ApplyControls(form.Controls, palette);
    }

    public static void Apply(ContextMenuStrip menu, ThemePalette palette)
    {
        menu.BackColor = palette.Control;
        menu.ForeColor = palette.Text;
        menu.ShowCheckMargin = false;
        menu.ShowImageMargin = true;
        menu.Renderer = new ThemedToolStripRenderer(palette);
        foreach (ToolStripItem item in menu.Items)
        {
            item.BackColor = palette.Control;
            item.ForeColor = item.Enabled ? palette.Text : palette.MutedText;
        }
    }

    private static void ApplyControls(Control.ControlCollection controls, ThemePalette palette)
    {
        foreach (Control control in controls)
        {
            switch (control)
            {
                case TextBoxBase textBox:
                    textBox.BackColor = palette.Input;
                    textBox.ForeColor = palette.InputText;
                    break;
                case Button button:
                    button.BackColor = palette.Control;
                    button.ForeColor = palette.Text;
                    button.FlatStyle = FlatStyle.Flat;
                    button.FlatAppearance.BorderColor = palette.Border;
                    button.FlatAppearance.MouseOverBackColor = palette.ControlHover;
                    break;
                case NumericUpDown numeric:
                    numeric.BackColor = palette.Input;
                    numeric.ForeColor = palette.InputText;
                    break;
                case Label label:
                    label.BackColor = Color.Transparent;
                    label.ForeColor = palette.Text;
                    break;
                default:
                    control.BackColor = palette.Window;
                    control.ForeColor = palette.Text;
                    break;
            }

            if (control.HasChildren)
            {
                ApplyControls(control.Controls, palette);
            }
        }
    }
}

internal sealed class ThemedToolStripRenderer : ToolStripProfessionalRenderer
{
    private readonly ThemePalette _palette;

    public ThemedToolStripRenderer(ThemePalette palette)
    {
        _palette = palette;
    }

    protected override void OnRenderToolStripBackground(ToolStripRenderEventArgs e)
    {
        using var brush = new SolidBrush(_palette.Control);
        e.Graphics.FillRectangle(brush, e.AffectedBounds);
    }

    protected override void OnRenderImageMargin(ToolStripRenderEventArgs e)
    {
        using var brush = new SolidBrush(_palette.Control);
        e.Graphics.FillRectangle(brush, e.AffectedBounds);
    }

    protected override void OnRenderMenuItemBackground(ToolStripItemRenderEventArgs e)
    {
        var color = e.Item.Selected ? _palette.ControlHover : _palette.Control;
        using var brush = new SolidBrush(color);
        e.Graphics.FillRectangle(brush, new Rectangle(Point.Empty, e.Item.Size));
    }

    protected override void OnRenderItemCheck(ToolStripItemImageRenderEventArgs e)
    {
        var box = new Rectangle(e.ImageRectangle.X + 2, e.ImageRectangle.Y + 2, 14, 14);
        using var background = new SolidBrush(_palette.ControlHover);
        using var border = new Pen(_palette.Border);
        using var check = new Pen(Color.FromArgb(93, 213, 181), 2f)
        {
            StartCap = System.Drawing.Drawing2D.LineCap.Round,
            EndCap = System.Drawing.Drawing2D.LineCap.Round,
        };

        e.Graphics.FillRectangle(background, box);
        e.Graphics.DrawRectangle(border, box);
        e.Graphics.SmoothingMode = System.Drawing.Drawing2D.SmoothingMode.AntiAlias;
        e.Graphics.DrawLines(check, new[]
        {
            new Point(box.Left + 3, box.Top + 7),
            new Point(box.Left + 6, box.Top + 10),
            new Point(box.Left + 11, box.Top + 4),
        });
    }

    protected override void OnRenderSeparator(ToolStripSeparatorRenderEventArgs e)
    {
        using var brush = new SolidBrush(_palette.Control);
        e.Graphics.FillRectangle(brush, new Rectangle(Point.Empty, e.Item.Size));
        using var pen = new Pen(_palette.Border);
        var y = e.Item.Height / 2;
        e.Graphics.DrawLine(pen, 0, y, e.Item.Width, y);
    }
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
            Program.SafeLog($"settings load error {error}");
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
        startInfo.Environment["STEPLER_DISABLE_CAPSLOCK"] = Bool(settings.DisableCapsLock);
        startInfo.Environment["STEPLER_INSERT_AS_BACKSPACE"] = Bool(settings.InsertAsBackspaceEnabled);
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

    public static ProcessStartInfo WithSteplerRuntimePaths(this ProcessStartInfo startInfo)
    {
        startInfo.Environment["STEPLER_HOTKEY_LOG_PATH"] = Program.HotkeyLogPath();
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
    private const int DwmwaUseImmersiveDarkMode = 20;
    private const int DwmwaUseImmersiveDarkModeBefore20H1 = 19;

    [System.Runtime.InteropServices.DllImport("user32.dll", SetLastError = true)]
    public static extern bool DestroyIcon(IntPtr handle);

    [DllImport("user32.dll")]
    public static extern bool SetForegroundWindow(IntPtr handle);

    public static void SetImmersiveDarkMode(IntPtr handle, bool enabled)
    {
        if (handle == IntPtr.Zero || !OperatingSystem.IsWindowsVersionAtLeast(10))
        {
            return;
        }

        var value = enabled ? 1 : 0;
        if (DwmSetWindowAttribute(handle, DwmwaUseImmersiveDarkMode, ref value, sizeof(int)) != 0)
        {
            _ = DwmSetWindowAttribute(handle, DwmwaUseImmersiveDarkModeBefore20H1, ref value, sizeof(int));
        }
    }

    [DllImport("dwmapi.dll", PreserveSig = true)]
    private static extern int DwmSetWindowAttribute(
        IntPtr hwnd,
        int dwAttribute,
        ref int pvAttribute,
        int cbAttribute);

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
    private readonly Label _versionLabel;
    private readonly Label _statusLabel;

    public ControlWindow(
        string status,
        Action toggleRunner,
        Action restartRunner,
        Action openHotkeyLog,
        Action openTrayLog,
        Action exit,
        bool darkTheme)
    {
        Text = "Stepler";
        ShowInTaskbar = true;
        FormBorderStyle = FormBorderStyle.FixedDialog;
        MaximizeBox = false;
        MinimizeBox = false;
        ClientSize = new Size(304, 204);
        StartPosition = FormStartPosition.Manual;

        var area = Screen.PrimaryScreen?.WorkingArea ?? new Rectangle(0, 0, 800, 600);
        Location = new Point(Math.Max(0, area.Right - Width - 16), Math.Max(0, area.Bottom - Height - 16));

        _versionLabel = new Label
        {
            AutoSize = false,
            Text = $"Версия: {Application.ProductVersion}",
            TextAlign = ContentAlignment.MiddleLeft,
            Location = new Point(16, 10),
            Size = new Size(280, 24),
        };

        _statusLabel = new Label
        {
            AutoSize = false,
            Text = status,
            TextAlign = ContentAlignment.MiddleLeft,
            Location = new Point(16, 36),
            Size = new Size(280, 28),
        };

        var toggleButton = Button("Вкл/выкл", 16, 78, toggleRunner);
        var restartButton = Button("Перезапустить", 160, 78, restartRunner);
        var hotkeyLogButton = Button("Лог hotkeys", 16, 118, openHotkeyLog);
        var trayLogButton = Button("Лог tray", 160, 118, openTrayLog);
        var exitButton = Button("Выход", 88, 160, exit);

        Controls.AddRange(new Control[]
        {
            _versionLabel,
            _statusLabel,
            toggleButton,
            restartButton,
            hotkeyLogButton,
            trayLogButton,
            exitButton,
        });
        ApplyTheme(darkTheme);
    }

    public void UpdateStatus(string status)
    {
        _statusLabel.Text = status;
    }

    public void ApplyTheme(bool darkTheme)
    {
        ThemeApplier.Apply(this, ThemePalette.FromDarkTheme(darkTheme));
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

internal sealed class QwenInputWindow : Form
{
    private readonly string _cliPath;
    private readonly TextBox _input;
    private readonly Label _status;
    private readonly QwenInputCorrectionController _correction;

    public QwenInputWindow(string cliPath, bool darkTheme, Action<string, bool> showTiming)
    {
        _cliPath = cliPath;

        Text = "Stepler Qwen Input";
        ShowInTaskbar = true;
        FormBorderStyle = FormBorderStyle.Sizable;
        MaximizeBox = true;
        MinimizeBox = false;
        ClientSize = new Size(520, 236);
        MinimumSize = new Size(540, 220);
        StartPosition = FormStartPosition.CenterScreen;
        KeyPreview = true;

        _input = new TextBox
        {
            AcceptsReturn = true,
            AcceptsTab = true,
            Multiline = true,
            ScrollBars = ScrollBars.Vertical,
            Location = new Point(12, 12),
            Size = new Size(496, 144),
            Font = new Font("Segoe UI", 10),
            Anchor = AnchorStyles.Top | AnchorStyles.Bottom | AnchorStyles.Left | AnchorStyles.Right,
            AllowDrop = true,
        };
        _input.DragEnter += OnInputDragEnter;
        _input.DragDrop += OnInputDragDrop;

        var submitButton = Button("Отправить", 12, 194, Submit);
        submitButton.Size = new Size(496, 30);
        submitButton.Anchor = AnchorStyles.Bottom | AnchorStyles.Left | AnchorStyles.Right;

        _status = new Label
        {
            AutoSize = false,
            Text = "Готово",
            TextAlign = ContentAlignment.MiddleLeft,
            Location = new Point(12, 164),
            Size = new Size(496, 24),
            Anchor = AnchorStyles.Bottom | AnchorStyles.Left | AnchorStyles.Right,
        };

        Controls.AddRange(new Control[]
        {
            _input,
            submitButton,
            _status,
        });

        _correction = new QwenInputCorrectionController(
            _cliPath,
            this,
            _input,
            SetStatus,
            Program.SafeLog,
            showTiming,
            "qwen input");
        KeyDown += OnQwenInputKeyDown;
        ApplyTheme(darkTheme);
    }

    public void ApplyTheme(bool darkTheme)
    {
        ThemeApplier.Apply(this, ThemePalette.FromDarkTheme(darkTheme));
    }

    private void OnQwenInputKeyDown(object? sender, KeyEventArgs e)
    {
        if (!QwenInputCorrectionController.TryGetCorrectionMode(e.KeyCode, e.Control, out var mode))
        {
            return;
        }

        e.Handled = true;
        e.SuppressKeyPress = true;
        _correction.ApplyCorrection(mode);
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

        var clientPoint = _input.PointToClient(new Point(e.X, e.Y));
        var insertionPoint = _input.GetCharIndexFromPosition(clientPoint);
        if (clientPoint.Y >= _input.ClientSize.Height - 4 && insertionPoint < _input.TextLength)
        {
            insertionPoint = _input.TextLength;
        }

        _input.Focus();
        _input.SelectionStart = Math.Clamp(insertionPoint, 0, _input.TextLength);
        _input.SelectedText = string.Join(Environment.NewLine, paths);
        SetStatus(paths.Length == 1 ? "Путь файла вставлен" : $"Пути файлов вставлены: {paths.Length}");
    }

    private void Submit()
    {
        if (!File.Exists(_cliPath))
        {
            SetStatus("stepler-cli.exe не найден");
            return;
        }

        var text = _input.Text;
        if (string.IsNullOrWhiteSpace(text))
        {
            return;
        }

        var result = RunCli(new[] { "qwen-submit", "--text", text });
        if (result.ExitCode == 0)
        {
            SetStatus("Отправлено в Qwen");
            _input.Clear();
        }
        else
        {
            SetStatus("Qwen input-file не найден");
        }
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
                Program.SafeLog($"qwen input cli stderr {stderr.Trim()}");
            }
            return new CliResult(process.ExitCode, stdout, stderr);
        }
        catch (Exception error)
        {
            Program.SafeLog($"qwen input cli error {error}");
            return new CliResult(1, string.Empty, error.Message);
        }
    }

    private void SetStatus(string text)
    {
        _status.Text = text;
    }

    private static Button Button(string text, int x, int y, Action action)
    {
        var button = new Button
        {
            Text = text,
            Location = new Point(x, y),
            Size = new Size(92, 30),
        };
        button.Click += (_, _) => action();
        return button;
    }

    private readonly record struct CliResult(int ExitCode, string Stdout, string Stderr);
}
