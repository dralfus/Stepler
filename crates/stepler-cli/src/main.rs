use std::io::Write;
use std::time::{Duration, Instant};
mod psreadline;

use psreadline::{PsReadLineMethod, PsReadLineRequest};
use stepler_app::{
    guard_clipboard_from_snapshot, ClipboardGuardReport, OperationError, OperationRunner,
};
use stepler_core::{
    build_replacement_plan, CorrectionMode, LogTrigger, MethodId, OperationLogEvent, OperationState,
};
use stepler_platform::{ClipboardBackend, ClipboardSnapshot, TextContextProvider, TextReplacer};
use stepler_platform_windows::{
    focus_diagnostics, install_console_modifier_release_handler,
    message_loop_with_keyboard_controls, method_diagnostics, release_modifier_keys,
    WindowsClipboardBackend, WindowsForegroundProvider, WindowsLayoutSwitcher,
    WindowsTextContextProvider, WindowsTextReplacer,
};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("run-hotkeys") {
        run_hotkeys();
        return;
    }
    if args.first().map(String::as_str) == Some("diagnose-focus") {
        diagnose_focus(&args);
        return;
    }
    if args.first().map(String::as_str) == Some("psreadline-plan") {
        psreadline_plan(&args);
        return;
    }
    if args.first().map(String::as_str) == Some("psreadline-self-test") {
        psreadline_self_test();
        return;
    }
    if args.first().map(String::as_str) == Some("uia-fixture") {
        uia_fixture();
        return;
    }

    let mode = match args.first().map(String::as_str) {
        Some("pause") | Some("Pause") => CorrectionMode::Pause,
        Some("scrolllock") | Some("ScrollLock") => CorrectionMode::ScrollLock,
        _ => {
            eprintln!(
                "usage: stepler-cli <pause|scrolllock|diagnose-focus|run-hotkeys|uia-fixture> [--apply] [--delay seconds]"
            );
            std::process::exit(2);
        }
    };
    let apply = args.iter().any(|arg| arg == "--apply");
    let delay = parse_delay_seconds(&args);

    if let Some(delay) = delay {
        eprintln!(
            "waiting {}s before reading focused control...",
            delay.as_secs()
        );
        std::thread::sleep(delay);
    }

    let provider = WindowsTextContextProvider;
    let replacer = WindowsTextReplacer;
    let clipboard = WindowsClipboardBackend;

    let context = match provider.text_context() {
        Ok(context) => context,
        Err(error) => {
            eprintln!("context error: {error:?}");
            std::process::exit(1);
        }
    };

    print_context(&context);

    let plan = match build_replacement_plan(&context, mode) {
        Ok(plan) => plan,
        Err(error) => {
            eprintln!("plan error: {error:?}");
            eprintln!(
                "hint: check that the caret is at the end of a mistyped word/phrase and text_preview shows the expected Notepad text"
            );
            std::process::exit(1);
        }
    };

    println!("range: {}..{}", plan.range.start, plan.range.end);
    println!("expected: {}", plan.expected_before_text);
    println!("replacement: {}", plan.replacement_text);
    println!("confidence: {:.3}", plan.confidence);

    if apply {
        let should_guard_clipboard = should_guard_clipboard(&context);
        let clipboard_before = should_guard_clipboard
            .then(|| clipboard.capture().ok())
            .flatten();
        match replacer.apply_replacement(&context, &plan) {
            Ok(result) => {
                if should_guard_clipboard {
                    if let Some(before) = clipboard_before {
                        let _ = guard_clipboard_from_snapshot(&clipboard, before);
                    }
                }
                println!("apply: {result:?}");
            }
            Err(error) => {
                if should_guard_clipboard {
                    if let Some(before) = clipboard_before {
                        let _ = guard_clipboard_from_snapshot(&clipboard, before);
                    }
                }
                eprintln!("apply error: {error:?}");
                std::process::exit(1);
            }
        }
    }
}

fn uia_fixture() {
    #[cfg(windows)]
    {
        let script = r#"
Add-Type -AssemblyName PresentationFramework
Add-Type -AssemblyName WindowsBase
$window = New-Object System.Windows.Window
$window.Title = 'Stepler UIA Fixture'
$window.Width = 560
$window.Height = 220
$window.WindowStartupLocation = 'CenterScreen'
$panel = New-Object System.Windows.Controls.StackPanel
$panel.Margin = '16'
$textbox = New-Object System.Windows.Controls.TextBox
[System.Windows.Automation.AutomationProperties]::SetAutomationId($textbox, 'SteplerUiaFixtureInput')
$textbox.Text = 'k.,jdm'
$textbox.FontSize = 26
$textbox.AcceptsReturn = $false
$textbox.TextWrapping = 'NoWrap'
$textbox.Height = 42
$hint = New-Object System.Windows.Controls.TextBlock
$hint.Margin = '0,12,0,0'
$hint.Text = 'Manual UIAutomationText fixture. Type text here, then press Pause or ScrollLock while stepler hotkeys are running.'
$hint.TextWrapping = 'Wrap'
$panel.Children.Add($textbox) | Out-Null
$panel.Children.Add($hint) | Out-Null
$window.Content = $panel
$window.Add_Loaded({
  $window.Activate() | Out-Null
  $textbox.Focus() | Out-Null
  $textbox.Select($textbox.Text.Length, 0)
  [System.Windows.Input.Keyboard]::Focus($textbox) | Out-Null
})
$window.ShowDialog() | Out-Null
"#;
        let status = std::process::Command::new("powershell.exe")
            .arg("-NoLogo")
            .arg("-NoProfile")
            .arg("-STA")
            .arg("-Command")
            .arg(script)
            .status()
            .expect("failed to launch UIA fixture");
        if !status.success() {
            std::process::exit(status.code().unwrap_or(1));
        }
    }

    #[cfg(not(windows))]
    {
        eprintln!("uia-fixture is Windows-only");
        std::process::exit(2);
    }
}

fn should_guard_clipboard(context: &stepler_core::TextContext) -> bool {
    let Some(binding) = &context.capabilities.method_binding else {
        return true;
    };
    binding.replace_methods.iter().any(|method| {
        matches!(
            method,
            MethodId::TerminalClipboardShortcut
                | MethodId::ConsoleBuffer
                | MethodId::ClipboardSelection
                | MethodId::SendInput
        )
    })
}

fn psreadline_self_test() {
    match psreadline::self_test_lines() {
        Ok(lines) => {
            for line in lines {
                println!("{line}");
            }
        }
        Err(error) => {
            eprintln!("psreadline-self-test error: {error:?}");
            std::process::exit(1);
        }
    }
}

fn psreadline_plan(args: &[String]) {
    let mode = match arg_value(args, "--mode").map(|value| value.to_ascii_lowercase()) {
        Some(value) if value == "pause" => CorrectionMode::Pause,
        Some(value) if value == "scrolllock" => CorrectionMode::ScrollLock,
        _ => {
            eprintln!(
                "usage: stepler-cli psreadline-plan --mode <pause|scrolllock> --text-b64 <utf16le-base64> --cursor <utf16-index>"
            );
            std::process::exit(2);
        }
    };
    let Some(text_b64) = arg_value(args, "--text-b64") else {
        eprintln!("psreadline-plan error: missing --text-b64");
        std::process::exit(2);
    };
    let cursor_utf16 = match arg_value(args, "--cursor").and_then(|value| value.parse().ok()) {
        Some(cursor) => cursor,
        None => {
            eprintln!("psreadline-plan error: missing or invalid --cursor");
            std::process::exit(2);
        }
    };

    let request = PsReadLineRequest {
        mode,
        text_b64: text_b64.to_owned(),
        cursor_utf16,
    };
    match PsReadLineMethod.plan(request) {
        Ok(plan) => println!("{}", plan.json),
        Err(error) => {
            eprintln!("psreadline-plan error: {error:?}");
            std::process::exit(match error {
                psreadline::PsReadLineError::InvalidText(_)
                | psreadline::PsReadLineError::InvalidCursor => 2,
                _ => 1,
            });
        }
    }
}

fn diagnose_focus(args: &[String]) {
    if let Some(delay) = parse_delay_seconds(args) {
        eprintln!(
            "waiting {}s before reading focused control...",
            delay.as_secs()
        );
        std::thread::sleep(delay);
    }

    match focus_diagnostics() {
        Ok(info) => {
            println!(
                "foreground: {} {} {:?}",
                info.foreground_hwnd, info.foreground_class, info.foreground_title
            );
            println!(
                "focused: {} {} {:?}",
                info.focused_hwnd, info.focused_class, info.focused_title
            );
        }
        Err(error) => {
            eprintln!("diagnose error: {error:?}");
            std::process::exit(1);
        }
    }

    if args.iter().any(|arg| arg == "--methods") {
        match method_diagnostics() {
            Ok(info) => {
                println!("method probes:");
                for probe in info.probes {
                    println!(
                        "  - method={} safety={} clipboard={} focus_stability={} preflight={} verify={} reason={}",
                        probe.method,
                        probe.safety,
                        probe.requires_clipboard,
                        probe.requires_focus_stability,
                        probe.can_preflight,
                        probe.can_verify,
                        probe.reason
                    );
                }
                println!(
                    "selected: context={:?} replacement={:?}",
                    info.selected_context_method, info.selected_replacement_method
                );
                println!(
                    "context: method={:?} error={:?}",
                    info.context_method, info.context_error
                );
            }
            Err(error) => {
                eprintln!("method diagnose error: {error:?}");
                std::process::exit(1);
            }
        }
    }
}

fn run_hotkeys() {
    if let Err(error) = install_console_modifier_release_handler() {
        eprintln!("console cleanup handler warning: {error:?}");
    }

    let foreground = WindowsForegroundProvider;
    let context_provider = WindowsTextContextProvider;
    let replacer = WindowsTextReplacer;
    let clipboard = WindowsClipboardBackend;
    let layout_switcher = WindowsLayoutSwitcher::new();
    let mut runner =
        OperationRunner::new_with_clipboard(&foreground, &context_provider, &replacer, &clipboard);
    let log_path = std::path::Path::new("stepler_hotkey_log.jsonl");

    eprintln!("Stepler hotkey runner started.");
    eprintln!(
        "Registered: Pause, ScrollLock. Controls: LeftCtrl=RU, RightCtrl=EN, Menu/Caps=next."
    );
    eprintln!("Press Ctrl+C in this console to stop.");
    eprintln!("Log: {}", log_path.display());

    let result = message_loop_with_keyboard_controls(
        |mode| handle_hotkey_event(mode, &mut runner, log_path),
        |action| {
            if let Err(error) = layout_switcher.handle_action(action) {
                eprintln!("{action:?}: {error:?}");
            } else {
                eprintln!("{action:?}: ok");
            }
        },
    );

    release_modifier_keys();

    if let Err(error) = result {
        eprintln!("hotkey runner error: {error:?}");
        std::process::exit(1);
    }
}

fn handle_hotkey_event<F, C, R, B>(
    mode: CorrectionMode,
    runner: &mut OperationRunner<'_, F, C, R, B>,
    log_path: &std::path::Path,
) where
    F: stepler_platform::ForegroundProvider,
    C: stepler_platform::TextContextProvider,
    R: stepler_platform::TextReplacer,
    B: stepler_platform::ClipboardBackend,
{
    let started = Instant::now();
    release_modifier_keys();
    let result = runner.handle_hotkey(mode);
    release_modifier_keys();

    match &result {
        Ok(outcome) => {
            if let Some(report) = &outcome.clipboard_guard {
                log_clipboard_guard(log_path, report);
                if report.donor_marker_seen {
                    eprintln!(
                        "clipboard warning: detected HotkeyHandler marker; old app may be handling the same hotkey"
                    );
                }
                if report.clipboard_changed && report.restore_ok {
                    eprintln!("clipboard: restored after operation");
                } else if !report.restore_ok {
                    eprintln!(
                        "clipboard restore warning: {:?}",
                        report.last_error.as_deref().unwrap_or("unknown")
                    );
                }
            }
            let event = OperationLogEvent {
                operation_id: outcome.operation_id.clone(),
                trigger: LogTrigger::from(mode),
                state: OperationState::Completed,
                app: Some(outcome.context.app_id.clone()),
                provider: Some(String::from("WindowsTextContextProvider")),
                replacer: Some(outcome.apply_result.method.clone()),
                range: Some(outcome.plan.range),
                expected_before_text: Some(log_preview(&outcome.plan.expected_before_text, 80)),
                replacement_text: Some(log_preview(&outcome.plan.replacement_text, 80)),
                clipboard_used: false,
                duration_ms: outcome.metrics.duration_ms,
                timings: outcome.metrics.timings.clone(),
            };
            append_log(log_path, &event.to_json_line());
            eprintln!("{mode:?}: applied in {}ms", outcome.metrics.duration_ms);
        }
        Err(error) => {
            let event = OperationLogEvent {
                operation_id: String::from("unknown"),
                trigger: LogTrigger::from(mode),
                state: OperationState::RolledBackOrFailed,
                app: None,
                provider: Some(String::from("WindowsTextContextProvider")),
                replacer: Some(String::from("WindowsTextReplacer")),
                range: None,
                expected_before_text: Some(format_operation_error(error)),
                replacement_text: None,
                clipboard_used: false,
                duration_ms: started.elapsed().as_millis(),
                timings: Vec::new(),
            };
            append_log(log_path, &event.to_json_line());
            eprintln!("{mode:?}: {error:?}");
        }
    }
}

fn log_clipboard_guard(path: &std::path::Path, report: &ClipboardGuardReport) {
    let before_summary = clipboard_summary(&report.before);
    let after_summary = report
        .after_before_restore
        .as_ref()
        .map(clipboard_summary)
        .unwrap_or_else(|| String::from("unavailable"));
    let final_summary = report
        .final_snapshot
        .as_ref()
        .map(clipboard_summary)
        .unwrap_or_else(|| String::from("unavailable"));

    append_log(
        path,
        &format!(
            "{{\"event\":\"clipboard_guard\",\"before\":{},\"after_before_restore\":{},\"clipboard_changed\":{},\"restore_ok\":{},\"restore_attempts\":{},\"donor_marker_seen\":{},\"final\":{},\"last_error\":{}}}\n",
            json_string(&before_summary),
            json_string(&after_summary),
            report.clipboard_changed,
            report.restore_ok,
            report.restore_attempts,
            report.donor_marker_seen,
            json_string(&final_summary),
            report
                .last_error
                .as_ref()
                .map(|error| json_string(error))
                .unwrap_or_else(|| String::from("null")),
        ),
    );
}

fn clipboard_summary(snapshot: &ClipboardSnapshot) -> String {
    let text = snapshot
        .text
        .as_ref()
        .map(|text| {
            let mut preview = text.chars().take(40).collect::<String>();
            if text.chars().count() > 40 {
                preview.push_str("...");
            }
            preview.replace("\r\n", "\\n")
        })
        .unwrap_or_else(|| String::from("<no text>"));
    let formats = snapshot
        .formats
        .iter()
        .map(|format| format.format.to_string())
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "seq={:?}; text={}; formats=[{}]",
        snapshot.sequence_number, text, formats
    )
}

fn json_string(value: &str) -> String {
    let mut escaped = String::from("\"");
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            ch if ch.is_control() => escaped.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => escaped.push(ch),
        }
    }
    escaped.push('"');
    escaped
}

fn append_log(path: &std::path::Path, line: &str) {
    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        Ok(mut file) => {
            let _ = file.write_all(line.as_bytes());
        }
        Err(error) => eprintln!("log write error: {error}"),
    }
}

fn format_operation_error(error: &OperationError) -> String {
    format!("{error:?}")
}

fn log_preview(value: &str, max_chars: usize) -> String {
    let mut preview = value.chars().take(max_chars).collect::<String>();
    if value.chars().count() > max_chars {
        preview.push_str("...");
    }
    preview
}

fn parse_delay_seconds(args: &[String]) -> Option<Duration> {
    args.windows(2)
        .find(|pair| pair[0] == "--delay")
        .and_then(|pair| pair[1].parse::<u64>().ok())
        .map(Duration::from_secs)
}

fn arg_value<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].as_str())
}

fn print_context(context: &stepler_core::TextContext) {
    println!("app: {}", context.app_id);
    println!("window: {}", context.window_id);
    println!("control: {}", context.control_id);
    println!(
        "caret: {}..{}",
        context.caret_range.start, context.caret_range.end
    );
    println!("selection: {:?}", context.selection_range);
    println!("text_len: {}", context.text_snapshot.len());
    println!("text_preview: {}", text_preview(context));
}

fn text_preview(context: &stepler_core::TextContext) -> String {
    let caret = context.caret_range.start.min(context.text_snapshot.len());
    let start = context.text_snapshot[..caret]
        .char_indices()
        .rev()
        .nth(40)
        .map(|(index, _)| index)
        .unwrap_or(0);
    let end = context.text_snapshot[caret..]
        .char_indices()
        .nth(40)
        .map(|(index, _)| caret + index)
        .unwrap_or(context.text_snapshot.len());

    let before = &context.text_snapshot[start..caret];
    let after = &context.text_snapshot[caret..end];
    format!("{before}|{after}").replace("\r\n", "\\n")
}
