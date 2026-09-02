using System.Reflection;

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
