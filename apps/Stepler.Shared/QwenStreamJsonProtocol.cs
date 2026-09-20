using System.Text.Json;
using System.Text.Json.Serialization;

namespace Stepler.Shared;

public sealed record QwenPerPromptLaunchSettings(
    string Model,
    int MaxSessionTurns,
    int MaxToolCalls,
    string MaxWallTime,
    int MaxSubagentDepth)
{
    public static QwenPerPromptLaunchSettings Default { get; } = new(
        "qwen38-flash-next",
        16,
        30,
        "20m",
        1);

    public static bool IsValidWallTime(string? value)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            return false;
        }

        var normalized = value.Trim();
        var multiplier = 1d;
        if (normalized.EndsWith('s'))
        {
            normalized = normalized[..^1];
        }
        else if (normalized.EndsWith('m'))
        {
            normalized = normalized[..^1];
            multiplier = 60;
        }
        else if (normalized.EndsWith('h'))
        {
            normalized = normalized[..^1];
            multiplier = 60 * 60;
        }

        return double.TryParse(
                   normalized,
                   System.Globalization.NumberStyles.Float,
                   System.Globalization.CultureInfo.InvariantCulture,
                   out var amount)
            && amount > 0
            && amount * multiplier <= TimeSpan.FromDays(24).TotalSeconds;
    }

    public static QwenPerPromptLaunchSettings Normalize(QwenPerPromptLaunchSettings settings)
    {
        ArgumentNullException.ThrowIfNull(settings);

        var defaults = Default;
        return new QwenPerPromptLaunchSettings(
            string.IsNullOrWhiteSpace(settings.Model) ? defaults.Model : settings.Model.Trim(),
            settings.MaxSessionTurns > 0 ? settings.MaxSessionTurns : defaults.MaxSessionTurns,
            settings.MaxToolCalls >= 0 ? settings.MaxToolCalls : defaults.MaxToolCalls,
            IsValidWallTime(settings.MaxWallTime) ? settings.MaxWallTime.Trim() : defaults.MaxWallTime,
            settings.MaxSubagentDepth > 0 ? settings.MaxSubagentDepth : defaults.MaxSubagentDepth);
    }

    public IEnumerable<string> ToArgumentList()
    {
        yield return "--model";
        yield return Model;
        yield return "--max-session-turns";
        yield return MaxSessionTurns.ToString(System.Globalization.CultureInfo.InvariantCulture);
        yield return "--max-tool-calls";
        yield return MaxToolCalls.ToString(System.Globalization.CultureInfo.InvariantCulture);
        yield return "--max-wall-time";
        yield return MaxWallTime;
        yield return "--max-subagent-depth";
        yield return MaxSubagentDepth.ToString(System.Globalization.CultureInfo.InvariantCulture);
    }
}

public static class QwenStreamJsonProtocol
{
    public static IReadOnlyList<string> BuildLaunchArguments(
        QwenPerPromptLaunchSettings settings,
        bool continueSession)
    {
        settings = QwenPerPromptLaunchSettings.Normalize(settings);

        var arguments = settings.ToArgumentList().ToList();
        if (continueSession)
        {
            arguments.Add("--continue");
        }

        // These flags belong to the workspace transport and must not be editable
        // through the user-facing Qwen launch settings.
        arguments.Add("--input-format");
        arguments.Add("stream-json");
        arguments.Add("--output-format");
        arguments.Add("stream-json");
        arguments.Add("--include-partial-messages");
        return arguments;
    }

    public static bool TryParseOutputLine(string line, out QwenStreamJsonOutput output)
    {
        output = new QwenStreamJsonOutput("unknown");
        if (string.IsNullOrWhiteSpace(line))
        {
            return false;
        }

        try
        {
            using var document = JsonDocument.Parse(line);
            var root = document.RootElement;
            if (!root.TryGetProperty("type", out var typeElement)
                || typeElement.ValueKind != JsonValueKind.String)
            {
                return false;
            }

            var type = typeElement.GetString() ?? "unknown";
            switch (type)
            {
                case "assistant":
                    output = new QwenStreamJsonOutput(type, ExtractAssistantText(root));
                    return true;

                case "stream_event":
                    output = new QwenStreamJsonOutput(type, ExtractStreamEventText(root));
                    return true;

                case "result":
                    output = new QwenStreamJsonOutput(
                        type,
                        IsError: root.TryGetProperty("is_error", out var errorElement)
                            && errorElement.ValueKind == JsonValueKind.True,
                        Subtype: GetString(root, "subtype"));
                    return true;

                case "control_request":
                    var request = root.TryGetProperty("request", out var requestElement)
                        ? requestElement
                        : default;
                    output = new QwenStreamJsonOutput(
                        type,
                        RequestId: GetString(root, "request_id"),
                        ToolName: GetString(request, "tool_name"),
                        Subtype: GetString(request, "subtype"));
                    return true;

                default:
                    output = new QwenStreamJsonOutput(type, Subtype: GetString(root, "subtype"));
                    return true;
            }
        }
        catch (JsonException)
        {
            return false;
        }
    }

    public static string SerializeUserMessage(string prompt)
    {
        ArgumentNullException.ThrowIfNull(prompt);

        return JsonSerializer.Serialize(new UserMessageEnvelope(
            "user",
            new UserMessage("user", prompt),
            null));
    }

    public static string SerializeControlResponse(string requestId, bool allowed)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(requestId);

        return JsonSerializer.Serialize(new ControlResponseEnvelope(
            "control_response",
            new ControlResponseBody(
                "success",
                requestId,
                new ControlDecision(allowed),
                null)));
    }

    private static string? ExtractAssistantText(JsonElement root)
    {
        if (!root.TryGetProperty("message", out var message)
            || !message.TryGetProperty("content", out var content)
            || content.ValueKind != JsonValueKind.Array)
        {
            return null;
        }

        var parts = new List<string>();
        foreach (var block in content.EnumerateArray())
        {
            var blockType = GetString(block, "type");
            if (blockType == "text")
            {
                var text = GetString(block, "text");
                if (!string.IsNullOrEmpty(text))
                {
                    parts.Add(text);
                }
            }
            else if (blockType == "tool_use")
            {
                var toolName = GetString(block, "name");
                if (!string.IsNullOrWhiteSpace(toolName))
                {
                    parts.Add($"[Инструмент: {toolName}]");
                }
            }
        }

        return parts.Count == 0 ? null : string.Concat(parts);
    }

    private static string? ExtractStreamEventText(JsonElement root)
    {
        if (!root.TryGetProperty("event", out var eventElement))
        {
            return null;
        }

        if (eventElement.TryGetProperty("delta", out var delta)
            && delta.ValueKind == JsonValueKind.Object)
        {
            var text = GetString(delta, "text");
            if (!string.IsNullOrEmpty(text))
            {
                return text;
            }
        }

        if (eventElement.TryGetProperty("content_block", out var contentBlock)
            && contentBlock.ValueKind == JsonValueKind.Object)
        {
            return GetString(contentBlock, "text");
        }

        return null;
    }

    private static string? GetString(JsonElement element, string propertyName)
    {
        return element.ValueKind == JsonValueKind.Object
            && element.TryGetProperty(propertyName, out var property)
            && property.ValueKind == JsonValueKind.String
            ? property.GetString()
            : null;
    }

    private sealed record UserMessageEnvelope(
        [property: JsonPropertyName("type")] string Type,
        [property: JsonPropertyName("message")] UserMessage Message,
        [property: JsonPropertyName("parent_tool_use_id")] string? ParentToolUseId);

    private sealed record UserMessage(
        [property: JsonPropertyName("role")] string Role,
        [property: JsonPropertyName("content")] string Content);

    private sealed record ControlResponseEnvelope(
        [property: JsonPropertyName("type")] string Type,
        [property: JsonPropertyName("response")] ControlResponseBody Response);

    private sealed record ControlResponseBody(
        [property: JsonPropertyName("subtype")] string Subtype,
        [property: JsonPropertyName("request_id")] string RequestId,
        [property: JsonPropertyName("response")] ControlDecision? Response,
        [property: JsonPropertyName("error")] string? Error);

    private sealed record ControlDecision(
        [property: JsonPropertyName("allowed")] bool Allowed);
}

public sealed record QwenStreamJsonOutput(
    string Type,
    string? Text = null,
    bool IsError = false,
    string? RequestId = null,
    string? ToolName = null,
    string? Subtype = null);
