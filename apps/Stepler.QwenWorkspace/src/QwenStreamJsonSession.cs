using System.Diagnostics;
using Stepler.Shared;

namespace Stepler.QwenWorkspace;

internal sealed class QwenStreamJsonSession : IDisposable
{
    private readonly string _workingDirectory;
    private readonly QwenPerPromptLaunchSettings _settings;
    private readonly bool _continueSession;
    private readonly Action<QwenStreamJsonOutput> _outputReceived;
    private readonly Action<string> _statusChanged;
    private readonly SemaphoreSlim _inputLock = new(1, 1);
    private Process? _process;
    private StreamWriter? _input;
    private bool _partialMessageSeen;

    public QwenStreamJsonSession(
        string workingDirectory,
        QwenPerPromptLaunchSettings settings,
        bool continueSession,
        Action<QwenStreamJsonOutput> outputReceived,
        Action<string> statusChanged)
    {
        _workingDirectory = workingDirectory;
        _settings = settings;
        _continueSession = continueSession;
        _outputReceived = outputReceived;
        _statusChanged = statusChanged;
    }

    public bool IsRunning
    {
        get
        {
            try
            {
                return _process is { HasExited: false };
            }
            catch
            {
                return false;
            }
        }
    }

    public void Start()
    {
        if (_process is not null)
        {
            throw new InvalidOperationException("Qwen stream session has already started.");
        }

        var process = new Process
        {
            StartInfo = CreateStartInfo(),
            EnableRaisingEvents = true,
        };
        process.OutputDataReceived += OnOutputDataReceived;
        process.ErrorDataReceived += OnErrorDataReceived;
        process.Exited += OnProcessExited;

        try
        {
            if (!process.Start())
            {
                throw new InvalidOperationException("Qwen process did not start.");
            }

            _process = process;
            _input = process.StandardInput;
            process.BeginOutputReadLine();
            process.BeginErrorReadLine();
            _statusChanged("Qwen per-prompt запущен");
        }
        catch
        {
            process.Dispose();
            throw;
        }
    }

    public async Task SendPromptAsync(string prompt)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(prompt);
        EnsureRunning();

        await _inputLock.WaitAsync().ConfigureAwait(false);
        try
        {
            EnsureRunning();
            var input = _input ?? throw new InvalidOperationException("Qwen input is unavailable.");
            await input.WriteLineAsync(QwenStreamJsonProtocol.SerializeUserMessage(prompt))
                .ConfigureAwait(false);
            await input.FlushAsync().ConfigureAwait(false);
        }
        finally
        {
            _inputLock.Release();
        }
    }

    public async Task RespondToPermissionAsync(string requestId, bool allowed)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(requestId);
        EnsureRunning();

        await _inputLock.WaitAsync().ConfigureAwait(false);
        try
        {
            EnsureRunning();
            var input = _input ?? throw new InvalidOperationException("Qwen input is unavailable.");
            await input.WriteLineAsync(
                    QwenStreamJsonProtocol.SerializeControlResponse(requestId, allowed))
                .ConfigureAwait(false);
            await input.FlushAsync().ConfigureAwait(false);
        }
        finally
        {
            _inputLock.Release();
        }
    }

    public void Dispose()
    {
        _inputLock.Dispose();
        var process = _process;
        _input = null;
        _process = null;
        if (process is null)
        {
            return;
        }

        try
        {
            process.OutputDataReceived -= OnOutputDataReceived;
            process.ErrorDataReceived -= OnErrorDataReceived;
            process.Exited -= OnProcessExited;
            if (!process.HasExited)
            {
                process.Kill(entireProcessTree: true);
                process.WaitForExit(3000);
            }
        }
        catch
        {
            // Closing the workspace must not fail because Qwen has already exited.
        }
        finally
        {
            process.Dispose();
        }
    }

    private ProcessStartInfo CreateStartInfo()
    {
        var npmDirectory = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
            "npm");
        var cliEntry = Path.Combine(
            npmDirectory,
            "node_modules",
            "@qwen-code",
            "qwen-code",
            "cli-entry.js");
        var qwenCommand = ResolveQwenCommand();
        var startInfo = new ProcessStartInfo
        {
            WorkingDirectory = _workingDirectory,
            UseShellExecute = false,
            CreateNoWindow = true,
            RedirectStandardInput = true,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
        };

        if (File.Exists(cliEntry))
        {
            startInfo.FileName = ResolveNodeCommand(npmDirectory);
            startInfo.ArgumentList.Add(cliEntry);
        }
        else
        {
            startInfo.FileName = Environment.GetEnvironmentVariable("ComSpec") ?? "cmd.exe";
            startInfo.ArgumentList.Add("/d");
            startInfo.ArgumentList.Add("/c");
            startInfo.ArgumentList.Add(qwenCommand);
        }

        foreach (var argument in QwenStreamJsonProtocol.BuildLaunchArguments(
                     _settings,
                     _continueSession))
        {
            startInfo.ArgumentList.Add(argument);
        }

        return startInfo;
    }

    private static string ResolveQwenCommand()
    {
        var appDataCommand = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData),
            "npm",
            "qwen.cmd");
        return File.Exists(appDataCommand) ? appDataCommand : "qwen.cmd";
    }

    private static string ResolveNodeCommand(string npmDirectory)
    {
        var bundledNode = Path.Combine(npmDirectory, "node.exe");
        return File.Exists(bundledNode) ? bundledNode : "node.exe";
    }

    private void EnsureRunning()
    {
        if (!IsRunning)
        {
            throw new InvalidOperationException("Qwen process is not running.");
        }
    }

    private void OnOutputDataReceived(object sender, DataReceivedEventArgs e)
    {
        if (string.IsNullOrWhiteSpace(e.Data))
        {
            return;
        }

        if (!QwenStreamJsonProtocol.TryParseOutputLine(e.Data, out var output))
        {
            _statusChanged("Qwen: некорректный stream-json output");
            return;
        }

        if (output.Type == "stream_event" && !string.IsNullOrEmpty(output.Text))
        {
            _partialMessageSeen = true;
        }
        else if (output.Type == "assistant" && _partialMessageSeen)
        {
            output = output with { Text = null };
        }
        else if (output.Type == "result")
        {
            _partialMessageSeen = false;
        }

        _outputReceived(output);
    }

    private void OnErrorDataReceived(object sender, DataReceivedEventArgs e)
    {
        if (!string.IsNullOrWhiteSpace(e.Data))
        {
            _statusChanged("Qwen: ошибка запуска или stderr");
        }
    }

    private void OnProcessExited(object? sender, EventArgs e)
    {
        _statusChanged("Qwen завершил работу");
    }
}
