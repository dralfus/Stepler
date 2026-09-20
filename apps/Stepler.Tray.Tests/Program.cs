using System.Reflection;
using System.Text.Json;
using Stepler.Shared;

var trayAssembly = Assembly.Load("Stepler");
var trayFormType = trayAssembly.GetType("Stepler.Tray.SteplerTrayForm")
    ?? throw new InvalidOperationException("SteplerTrayForm was not found.");
var formatter = trayFormType.GetMethod(
    "TryFormatHotkeyTiming",
    BindingFlags.Static | BindingFlags.NonPublic)
    ?? throw new InvalidOperationException("TryFormatHotkeyTiming was not found.");

var arguments = new object?[]
{
    "{\"operation_id\":\"embedded-terminal\",\"trigger\":\"ScrollLock\",\"state\":\"Completed\",\"app\":\"embedded_terminal\",\"replacer\":\"embedded_terminal_psreadline\",\"duration_ms\":600}",
    null,
    false,
};
var handled = (bool)(formatter.Invoke(null, arguments)
    ?? throw new InvalidOperationException("Formatter returned null."));

if (handled)
{
    throw new InvalidOperationException(
        "Embedded terminal forwarding must not be rendered as a completed correction.");
}

Console.WriteLine("embedded terminal forwarding does not render a false success");

var defaults = QwenPerPromptLaunchSettings.Default;
AssertEqual("qwen38-flash-next", defaults.Model, "default model");
AssertEqual(16, defaults.MaxSessionTurns, "default max session turns");
AssertEqual(30, defaults.MaxToolCalls, "default max tool calls");
AssertEqual("20m", defaults.MaxWallTime, "default max wall time");
AssertEqual(1, defaults.MaxSubagentDepth, "default max subagent depth");
AssertEqual(true, QwenPerPromptLaunchSettings.IsValidWallTime("20m"), "valid wall time");
AssertEqual(false, QwenPerPromptLaunchSettings.IsValidWallTime("not-a-duration"), "invalid wall time");

var normalized = QwenPerPromptLaunchSettings.Normalize(
    new QwenPerPromptLaunchSettings(" ", 0, -1, "not-a-duration", -1));
AssertEqual(defaults, normalized, "invalid launch settings normalization");

var launchArguments = defaults.ToArgumentList().ToArray();
AssertSequence(
    new[]
    {
        "--model", "qwen38-flash-next",
        "--max-session-turns", "16",
        "--max-tool-calls", "30",
        "--max-wall-time", "20m",
        "--max-subagent-depth", "1",
    },
    launchArguments,
    "default Qwen launch arguments");

var prompt = "Проверь русский prompt\nи сохрани его без изменений.";
var message = JsonDocument.Parse(QwenStreamJsonProtocol.SerializeUserMessage(prompt));
AssertEqual("user", message.RootElement.GetProperty("type").GetString(), "stream-json message type");
AssertEqual(
    prompt,
    message.RootElement.GetProperty("message").GetProperty("content").GetString(),
    "stream-json prompt content");
AssertEqual(
    "user",
    message.RootElement.GetProperty("message").GetProperty("role").GetString(),
    "stream-json prompt role");

var controlResponse = JsonDocument.Parse(
    QwenStreamJsonProtocol.SerializeControlResponse("request-1", allowed: false));
AssertEqual(
    "control_response",
    controlResponse.RootElement.GetProperty("type").GetString(),
    "control response type");
AssertEqual(
    "request-1",
    controlResponse.RootElement
        .GetProperty("response")
        .GetProperty("request_id")
        .GetString(),
    "control response request id");
AssertEqual(
    false,
    controlResponse.RootElement
        .GetProperty("response")
        .GetProperty("response")
        .GetProperty("allowed")
        .GetBoolean(),
    "control response decision");

Console.WriteLine("Qwen per-prompt protocol contract is valid");

if (!QwenStreamJsonProtocol.TryParseOutputLine(
        "{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"Ответ Qwen\"}]}}",
        out var assistantOutput)
    || assistantOutput.Text != "Ответ Qwen")
{
    throw new InvalidOperationException("assistant stream-json output was not parsed");
}

if (!QwenStreamJsonProtocol.TryParseOutputLine(
        "{\"type\":\"control_request\",\"request_id\":\"req-2\",\"request\":{\"subtype\":\"can_use_tool\",\"tool_name\":\"Bash\"}}",
        out var permissionOutput)
    || permissionOutput.RequestId != "req-2"
    || permissionOutput.ToolName != "Bash")
{
    throw new InvalidOperationException("permission stream-json output was not parsed");
}

if (!QwenStreamJsonProtocol.TryParseOutputLine(
        "{\"type\":\"result\",\"is_error\":true,\"subtype\":\"error_during_execution\"}",
        out var resultOutput)
    || !resultOutput.IsError
    || resultOutput.Subtype != "error_during_execution")
{
    throw new InvalidOperationException("result stream-json output was not parsed");
}

Console.WriteLine("Qwen per-prompt output parsing is valid");

if (QwenStreamJsonProtocol.TryParseOutputLine("not-json", out _))
{
    throw new InvalidOperationException("malformed stream-json output was accepted");
}

AssertSequence(
    new[]
    {
        "--model", "qwen38-flash-next",
        "--max-session-turns", "16",
        "--max-tool-calls", "30",
        "--max-wall-time", "20m",
        "--max-subagent-depth", "1",
        "--continue",
        "--input-format", "stream-json",
        "--output-format", "stream-json",
        "--include-partial-messages",
    },
    QwenStreamJsonProtocol.BuildLaunchArguments(defaults, continueSession: true),
    "per-prompt Qwen launch arguments");

static void AssertEqual<T>(T expected, T actual, string name)
{
    if (!EqualityComparer<T>.Default.Equals(expected, actual))
    {
        throw new InvalidOperationException($"{name}: expected {expected}, got {actual}");
    }
}

static void AssertSequence<T>(IReadOnlyList<T> expected, IReadOnlyList<T> actual, string name)
{
    if (!expected.SequenceEqual(actual))
    {
        throw new InvalidOperationException(
            $"{name}: expected [{string.Join(", ", expected)}], got [{string.Join(", ", actual)}]");
    }
}
