use std::io::Write;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
mod psreadline;

use psreadline::{PsReadLineMethod, PsReadLineRequest};
use stepler_app::{
    guard_clipboard_from_snapshot, ClipboardGuardReport, OperationError, OperationRunner,
};
use stepler_core::{
    build_replacement_plan, CorrectionError, CorrectionMode, LogTrigger, MethodId,
    OperationLogEvent, OperationState,
};
use stepler_platform::{
    ClipboardBackend, ClipboardSnapshot, PlatformError, TextContextProvider, TextReplacer,
};
use stepler_platform_windows::{
    focus_diagnostics, install_console_modifier_release_handler,
    message_loop_with_keyboard_controls, method_diagnostics, release_modifier_keys,
    request_keyboard_control_action, try_forward_embedded_terminal_hotkey, KeyboardControlAction,
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
    if args.first().map(String::as_str) == Some("qwen-submit") {
        qwen_submit(&args);
        return;
    }
    if args.first().map(String::as_str) == Some("switch-layout") {
        switch_layout(&args);
        return;
    }
    if args.first().map(String::as_str) == Some("trigger-layout-control") {
        trigger_layout_control(&args);
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

    set_active_correction_mode(mode);
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
                let layout_result = switch_layout_after_replacement(
                    &WindowsLayoutSwitcher::new(),
                    &plan.expected_before_text,
                    &plan.replacement_text,
                    &context,
                    layout_hwnd_hint(&context.window_id, &context.control_id),
                );
                if let Some(layout_result) = layout_result {
                    println!("layout: {layout_result}");
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

fn switch_layout(args: &[String]) {
    let Some(target) = args.get(1).map(String::as_str) else {
        eprintln!(
            "usage: stepler-cli switch-layout <russian|english> [--hwnd <decimal|0xhex|hwnd:hex>]"
        );
        std::process::exit(2);
    };
    let switcher = WindowsLayoutSwitcher::new();
    let target_hwnd = arg_value(args, "--hwnd").and_then(parse_hwnd_arg);
    let result = match target.to_ascii_lowercase().as_str() {
        "russian" | "ru" => {
            if let Some(hwnd) = target_hwnd {
                switcher.switch_window_to_russian(hwnd)
            } else {
                switcher.switch_to_russian()
            }
        }
        "english" | "en" => {
            if let Some(hwnd) = target_hwnd {
                switcher.switch_window_to_english(hwnd)
            } else {
                switcher.switch_to_english()
            }
        }
        _ => {
            eprintln!("usage: stepler-cli switch-layout <russian|english> [--hwnd <decimal|0xhex|hwnd:hex>]");
            std::process::exit(2);
        }
    };
    if let Err(error) = result {
        eprintln!("switch-layout error: {error:?}");
        std::process::exit(1);
    }
}

fn trigger_layout_control(args: &[String]) {
    let Some(target) = args.get(1).map(String::as_str) else {
        eprintln!("usage: stepler-cli trigger-layout-control <russian|english>");
        std::process::exit(2);
    };
    let action = match target.to_ascii_lowercase().as_str() {
        "russian" | "ru" => stepler_platform_windows::KeyboardControlAction::SwitchToRussian,
        "english" | "en" => stepler_platform_windows::KeyboardControlAction::SwitchToEnglish,
        _ => {
            eprintln!("usage: stepler-cli trigger-layout-control <russian|english>");
            std::process::exit(2);
        }
    };
    if let Err(error) = request_keyboard_control_action(action) {
        eprintln!("trigger-layout-control error: {error:?}");
        std::process::exit(1);
    }
}

fn parse_hwnd_arg(value: &str) -> Option<isize> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    if let Some(hex) = value.strip_prefix("hwnd:") {
        return isize::from_str_radix(hex, 16).ok();
    }
    if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        return isize::from_str_radix(hex, 16).ok();
    }
    value.parse::<isize>().ok()
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
$hint.Text = 'Manual UIAutomationText fixture. Type text here, then press Pause or Ctrl+Pause while stepler hotkeys are running.'
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

fn qwen_submit(args: &[String]) {
    let text = match arg_value(args, "--text") {
        Some(text) => text.to_owned(),
        None => {
            eprintln!("usage: stepler-cli qwen-submit --text <text>");
            std::process::exit(2);
        }
    };
    let input_file = match qwen_input_file_from_marker() {
        Some(path) => path,
        None => {
            eprintln!("qwen-submit error: active Qwen input file marker was not found");
            std::process::exit(1);
        }
    };
    let line = format!("{{\"type\":\"submit\",\"text\":{}}}\n", json_string(&text));
    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&input_file)
        .and_then(|mut file| file.write_all(line.as_bytes()))
    {
        Ok(()) => println!("submitted: {}", input_file.display()),
        Err(error) => {
            eprintln!("qwen-submit error: {error}");
            std::process::exit(1);
        }
    }
}

fn qwen_input_file_from_marker() -> Option<std::path::PathBuf> {
    let local_app_data = std::env::var_os("LOCALAPPDATA")?;
    let marker_path = std::path::PathBuf::from(local_app_data)
        .join("Stepler")
        .join("state")
        .join("terminal-app-qwen.marker");
    let marker = std::fs::read_to_string(marker_path).ok()?;
    for part in marker.split(['\n', ';']) {
        let part = part.trim();
        let Some(value) = part.strip_prefix("input_file=") else {
            continue;
        };
        let value = value.trim();
        if !value.is_empty() {
            return Some(std::path::PathBuf::from(value));
        }
    }
    None
}

fn psreadline_plan(args: &[String]) {
    let mode = match arg_value(args, "--mode").map(|value| value.to_ascii_lowercase()) {
        Some(value) if value == "pause" => CorrectionMode::Pause,
        Some(value) if value == "scrolllock" => CorrectionMode::ScrollLock,
        _ => {
            eprintln!(
                "usage: stepler-cli psreadline-plan --mode <pause|scrolllock> --text-b64 <utf16le-base64> --cursor <utf16-index> [--selection-start <utf16-index> --selection-length <utf16-length>]"
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
        selection_start_utf16: arg_value(args, "--selection-start")
            .and_then(|value| value.parse().ok()),
        selection_length_utf16: arg_value(args, "--selection-length")
            .and_then(|value| value.parse().ok()),
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

    let show_methods = args.iter().any(|arg| arg == "--methods");
    let show_surface = args.iter().any(|arg| arg == "--surface");
    if show_methods || show_surface {
        match method_diagnostics() {
            Ok(info) => {
                if show_surface {
                    println!(
                        "surface: kind={} confidence={} web_profile={} risky_allowed={}",
                        info.surface.kind,
                        info.surface.confidence,
                        info.surface.web_keyboard_profile,
                        info.surface.allow_risky_methods
                    );
                    for evidence in &info.surface.evidence {
                        println!("  evidence: {evidence}");
                    }
                    println!(
                        "policy: pause_context=[{}] pause_replace=[{}]",
                        info.surface.pause_context_methods.join(","),
                        info.surface.pause_replace_methods.join(",")
                    );
                    println!(
                        "policy: scrolllock_context=[{}] scrolllock_replace=[{}]",
                        info.surface.scrolllock_context_methods.join(","),
                        info.surface.scrolllock_replace_methods.join(",")
                    );
                    println!(
                        "policy: forbidden=[{}]",
                        info.surface.forbidden_methods.join(",")
                    );
                    println!(
                        "probe_plan: methods=[{}] runtime=[{}] suppressed=[{}] fast={}",
                        info.probe_plan_methods.join(","),
                        info.runtime_probe_methods.join(","),
                        info.probe_plan_suppressed_methods.join(","),
                        info.probe_plan_fast
                    );
                    println!(
                        "resolver_pause: context={:?} replacement={:?}",
                        info.selected_pause_context_method, info.selected_pause_replacement_method
                    );
                    println!("resolver_trace_pause:");
                    for entry in &info.pause_trace {
                        println!(
                            "  - method={} outcome={} safety={} confidence={} rank={} replacement={:?} reason={}",
                            entry.method,
                            entry.outcome,
                            entry.safety,
                            entry.confidence,
                            entry.preference_rank,
                            entry.replacement_method,
                            entry.reason
                        );
                    }
                    println!(
                        "resolver_scrolllock: context={:?} replacement={:?}",
                        info.selected_scrolllock_context_method,
                        info.selected_scrolllock_replacement_method
                    );
                    println!("resolver_trace_scrolllock:");
                    for entry in &info.scrolllock_trace {
                        println!(
                            "  - method={} outcome={} safety={} confidence={} rank={} replacement={:?} reason={}",
                            entry.method,
                            entry.outcome,
                            entry.safety,
                            entry.confidence,
                            entry.preference_rank,
                            entry.replacement_method,
                            entry.reason
                        );
                    }
                }
                if show_methods {
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
                    if let Some(uia) = info.uia_focus {
                        println!(
                            "uia_focus: name={:?} control_type={} automation_id={:?} class_name={:?} framework_id={:?} keyboard_focus={} keyboard_focusable={}",
                            uia.name,
                            uia.control_type,
                            uia.automation_id,
                            uia.class_name,
                            uia.framework_id,
                            uia.has_keyboard_focus,
                            uia.is_keyboard_focusable
                        );
                    }
                    println!(
                        "resolver_first: context={:?} replacement={:?}",
                        info.selected_context_method, info.selected_replacement_method
                    );
                    println!(
                        "context: method={:?} error={:?} skipped={}",
                        info.context_method, info.context_error, info.context_skipped
                    );
                }
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
    let log_path = hotkey_log_path();
    let settings = RuntimeSettings::from_env();

    eprintln!("Stepler hotkey runner started.");
    eprintln!("Registered: Pause, Ctrl+Pause. Controls: LeftCtrl=RU, RightCtrl=EN, Menu=next.");
    eprintln!(
        "Settings: pause={} scrolllock={} ctrl_layout={} menu_next={} disable_caps={} insert_backspace={} risky_fallbacks={}",
        settings.pause_enabled,
        settings.scrolllock_enabled,
        settings.ctrl_layout_enabled,
        settings.menu_caps_enabled,
        settings.disable_caps_lock,
        settings.insert_as_backspace,
        settings.risky_fallbacks_enabled
    );
    eprintln!("Press Ctrl+C in this console to stop.");
    eprintln!("Log: {}", log_path.display());

    let result = message_loop_with_keyboard_controls(
        |mode| {
            if settings.hotkey_enabled(mode) {
                handle_hotkey_event(mode, &mut runner, &layout_switcher, log_path.as_path());
            } else {
                eprintln!("{mode:?}: disabled");
            }
        },
        |mode| {
            log_hotkey_received(log_path.as_path(), mode);
        },
        |mode| {
            log_hotkey_unsupported(log_path.as_path(), mode, "unsupported_surface");
        },
        |action| {
            if !settings.layout_action_enabled(action) {
                eprintln!("{action:?}: disabled");
            } else if let Err(error) = layout_switcher.handle_action(action) {
                stepler_platform_windows::append_hotkey_signal_log(&format!(
                    "runner_layout_action_result action={action:?} result=error error={error:?}"
                ));
                eprintln!("{action:?}: {error:?}");
            } else {
                stepler_platform_windows::append_hotkey_signal_log(&format!(
                    "runner_layout_action_result action={action:?} result=ok"
                ));
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

#[derive(Debug, Clone, Copy)]
struct RuntimeSettings {
    pause_enabled: bool,
    scrolllock_enabled: bool,
    ctrl_layout_enabled: bool,
    menu_caps_enabled: bool,
    disable_caps_lock: bool,
    insert_as_backspace: bool,
    risky_fallbacks_enabled: bool,
}

impl RuntimeSettings {
    fn from_env() -> Self {
        Self {
            pause_enabled: env_enabled("STEPLER_ENABLE_PAUSE", true),
            scrolllock_enabled: env_enabled("STEPLER_ENABLE_SCROLLLOCK", true),
            ctrl_layout_enabled: env_enabled("STEPLER_ENABLE_CTRL_LAYOUT", true),
            menu_caps_enabled: env_enabled("STEPLER_ENABLE_MENU_CAPS_LAYOUT", true),
            disable_caps_lock: env_enabled("STEPLER_DISABLE_CAPSLOCK", true),
            insert_as_backspace: env_enabled("STEPLER_INSERT_AS_BACKSPACE", true),
            risky_fallbacks_enabled: std::env::var_os("STEPLER_ALLOW_RISKY_FALLBACKS").is_some(),
        }
    }

    fn hotkey_enabled(self, mode: CorrectionMode) -> bool {
        match mode {
            CorrectionMode::Pause => self.pause_enabled,
            CorrectionMode::ScrollLock => self.scrolllock_enabled,
        }
    }

    fn layout_action_enabled(
        self,
        action: stepler_platform_windows::KeyboardControlAction,
    ) -> bool {
        use stepler_platform_windows::KeyboardControlAction;
        match action {
            KeyboardControlAction::SwitchToRussian | KeyboardControlAction::SwitchToEnglish => {
                self.ctrl_layout_enabled
            }
            KeyboardControlAction::SwitchToNext => self.menu_caps_enabled,
        }
    }
}

fn env_enabled(name: &str, default: bool) -> bool {
    match std::env::var(name) {
        Ok(value) => !matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "0" | "false" | "off" | "no"
        ),
        Err(_) => default,
    }
}

fn hotkey_log_path() -> std::path::PathBuf {
    match std::env::var_os("STEPLER_HOTKEY_LOG_PATH") {
        Some(value) if !value.is_empty() => std::path::PathBuf::from(value),
        _ => std::path::PathBuf::from("stepler_hotkey_log.jsonl"),
    }
}

fn handle_hotkey_event<F, C, R, B>(
    mode: CorrectionMode,
    runner: &mut OperationRunner<'_, F, C, R, B>,
    layout_switcher: &WindowsLayoutSwitcher,
    log_path: &std::path::Path,
) where
    F: stepler_platform::ForegroundProvider,
    C: stepler_platform::TextContextProvider,
    R: stepler_platform::TextReplacer,
    B: stepler_platform::ClipboardBackend,
{
    let started = Instant::now();
    set_active_correction_mode(mode);
    release_modifier_keys();
    let result = runner.handle_hotkey(mode);
    release_modifier_keys();

    match &result {
        Ok(outcome) => {
            let layout_result = switch_layout_after_replacement(
                layout_switcher,
                &outcome.plan.expected_before_text,
                &outcome.plan.replacement_text,
                &outcome.context,
                layout_hwnd_hint(&outcome.context.window_id, &outcome.context.control_id),
            );
            if let Some(layout_result) = &layout_result {
                eprintln!("layout after correction: {layout_result}");
            }
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
                timestamp_unix_ms: timestamp_unix_ms(),
                trigger: LogTrigger::from(mode),
                state: OperationState::Completed,
                app: Some(outcome.context.app_id.clone()),
                provider: Some(String::from("WindowsTextContextProvider")),
                replacer: Some(outcome.apply_result.method.clone()),
                range: Some(outcome.plan.range),
                expected_before_text: Some(log_preview(&outcome.plan.expected_before_text, 80)),
                replacement_text: Some(log_preview(&outcome.plan.replacement_text, 80)),
                resolver_trace: None,
                clipboard_used: false,
                duration_ms: outcome.metrics.duration_ms,
                timings: outcome.metrics.timings.clone(),
            };
            append_log(log_path, &event.to_json_line());
            eprintln!("{mode:?}: applied in {}ms", outcome.metrics.duration_ms);
        }
        Err(error) => {
            if matches!(try_forward_embedded_terminal_hotkey(mode), Ok(true)) {
                eprintln!("{mode:?}: forwarded to embedded terminal PSReadLine");
                let event = OperationLogEvent {
                    operation_id: String::from("embedded-terminal"),
                    timestamp_unix_ms: timestamp_unix_ms(),
                    trigger: LogTrigger::from(mode),
                    state: OperationState::Completed,
                    app: Some(String::from("embedded_terminal")),
                    provider: Some(String::from("WindowsTextContextProvider")),
                    replacer: Some(String::from("embedded_terminal_psreadline")),
                    range: None,
                    expected_before_text: Some(String::from(
                        "forwarded_to_embedded_terminal_psreadline",
                    )),
                    replacement_text: None,
                    resolver_trace: None,
                    clipboard_used: false,
                    duration_ms: started.elapsed().as_millis(),
                    timings: Vec::new(),
                };
                append_log(log_path, &event.to_json_line());
                release_modifier_keys();
                return;
            }
            let error_text = format_operation_error(error);
            let no_change = is_no_text_to_replace_error(error);
            let unsupported = is_unsupported_error(error);
            let resolver_trace =
                stepler_platform_windows::hotkey_failure_trace_summary(mode, &error_text).ok();
            let event = OperationLogEvent {
                operation_id: String::from("unknown"),
                timestamp_unix_ms: timestamp_unix_ms(),
                trigger: LogTrigger::from(mode),
                state: if no_change {
                    OperationState::NoChange
                } else if unsupported {
                    OperationState::Unsupported
                } else {
                    OperationState::RolledBackOrFailed
                },
                app: None,
                provider: Some(String::from("WindowsTextContextProvider")),
                replacer: Some(String::from("WindowsTextReplacer")),
                range: None,
                expected_before_text: Some(error_text),
                replacement_text: None,
                resolver_trace,
                clipboard_used: false,
                duration_ms: started.elapsed().as_millis(),
                timings: Vec::new(),
            };
            append_log(log_path, &event.to_json_line());
            eprintln!("{mode:?}: {error:?}");
        }
    }
    release_modifier_keys();
}

fn log_hotkey_received(log_path: &std::path::Path, mode: CorrectionMode) {
    let event = OperationLogEvent {
        operation_id: String::from("hotkey"),
        timestamp_unix_ms: timestamp_unix_ms(),
        trigger: LogTrigger::from(mode),
        state: OperationState::HotkeyReceived,
        app: None,
        provider: Some(String::from("WindowsTextContextProvider")),
        replacer: None,
        range: None,
        expected_before_text: Some(String::from("hotkey_received")),
        replacement_text: None,
        resolver_trace: None,
        clipboard_used: false,
        duration_ms: 0,
        timings: Vec::new(),
    };
    append_log(log_path, &event.to_json_line());
}

fn log_hotkey_unsupported(log_path: &std::path::Path, mode: CorrectionMode, reason: &str) {
    let event = OperationLogEvent {
        operation_id: String::from("unsupported"),
        timestamp_unix_ms: timestamp_unix_ms(),
        trigger: LogTrigger::from(mode),
        state: OperationState::Unsupported,
        app: None,
        provider: Some(String::from("WindowsTextContextProvider")),
        replacer: None,
        range: None,
        expected_before_text: Some(String::from(reason)),
        replacement_text: None,
        resolver_trace: None,
        clipboard_used: false,
        duration_ms: 0,
        timings: Vec::new(),
    };
    append_log(log_path, &event.to_json_line());
}

fn is_no_text_to_replace_error(error: &OperationError) -> bool {
    matches!(
        error,
        OperationError::Correction(CorrectionError::NoTextToReplace)
            | OperationError::CorrectionWithContext(CorrectionError::NoTextToReplace, _)
    )
}

fn is_unsupported_error(error: &OperationError) -> bool {
    matches!(
        error,
        OperationError::Platform(PlatformError::Unsupported)
            | OperationError::Platform(PlatformError::UnsupportedControl { .. })
    )
}

fn set_active_correction_mode(mode: CorrectionMode) {
    let value = match mode {
        CorrectionMode::Pause => "pause",
        CorrectionMode::ScrollLock => "scrolllock",
    };
    std::env::set_var("STEPLER_ACTIVE_CORRECTION_MODE", value);
}

fn timestamp_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DesiredLayout {
    Russian,
    English,
}

fn switch_layout_after_replacement(
    layout_switcher: &WindowsLayoutSwitcher,
    expected_before_text: &str,
    replacement_text: &str,
    context: &stepler_core::TextContext,
    hwnd_hint: Option<isize>,
) -> Option<String> {
    let Some(layout) = desired_layout_after_replacement(expected_before_text, replacement_text)
    else {
        return None;
    };
    if should_skip_layout_after_replacement(context) {
        return Some(format!("skipped_for_{}", context.app_id));
    }

    let action = match layout {
        DesiredLayout::Russian => KeyboardControlAction::SwitchToRussian,
        DesiredLayout::English => KeyboardControlAction::SwitchToEnglish,
    };
    let control_result = request_keyboard_control_action(action);
    std::thread::sleep(Duration::from_millis(20));

    let window_result = hwnd_hint.map(|hwnd| match layout {
        DesiredLayout::Russian => layout_switcher.switch_window_to_russian(hwnd),
        DesiredLayout::English => layout_switcher.switch_window_to_english(hwnd),
    });
    let foreground_result = match layout {
        DesiredLayout::Russian => layout_switcher.switch_to_russian(),
        DesiredLayout::English => layout_switcher.switch_to_english(),
    };

    if control_result.is_ok()
        || window_result.as_ref().is_some_and(Result::is_ok)
        || foreground_result.is_ok()
    {
        return Some(format!("switched_to_{layout:?}"));
    }

    if let Err(error) = foreground_result {
        eprintln!("layout after correction warning: {error:?}");
        if let Some(Err(window_error)) = window_result {
            eprintln!("layout after correction window warning: {window_error:?}");
        }
        if let Err(control_error) = control_result {
            eprintln!("layout after correction control warning: {control_error:?}");
        }
        return Some(format!("switch_failed_{error:?}"));
    }
    Some(format!("switch_failed_{layout:?}"))
}

fn should_skip_layout_after_replacement(context: &stepler_core::TextContext) -> bool {
    context
        .app_id
        .eq_ignore_ascii_case("rctrl_renwnd32/RICHEDIT60W")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context(app_id: &str, control_id: &str) -> stepler_core::TextContext {
        let mut context = stepler_core::TextContext::new("text");
        context.app_id = app_id.to_owned();
        context.control_id = control_id.to_owned();
        context
    }

    #[test]
    fn outlook_word_com_does_not_skip_layout_after_replacement() {
        let context = context("rctrl_renwnd32/_WwG", "outlook-word-com:0:hwnd:1234");

        assert!(!should_skip_layout_after_replacement(&context));
    }

    #[test]
    fn outlook_richedit_keeps_legacy_layout_skip() {
        let context = context("rctrl_renwnd32/RICHEDIT60W", "richedit:hwnd:1234");

        assert!(should_skip_layout_after_replacement(&context));
    }
}

fn desired_layout_after_replacement(
    expected_before_text: &str,
    replacement_text: &str,
) -> Option<DesiredLayout> {
    if expected_before_text == replacement_text {
        return None;
    }
    let russian = replacement_text
        .chars()
        .filter(|ch| is_russian_letter(*ch))
        .count();
    let english = replacement_text
        .chars()
        .filter(|ch| ch.is_ascii_alphabetic())
        .count();
    match russian.cmp(&english) {
        std::cmp::Ordering::Greater => Some(DesiredLayout::Russian),
        std::cmp::Ordering::Less => Some(DesiredLayout::English),
        std::cmp::Ordering::Equal => None,
    }
}

fn is_russian_letter(ch: char) -> bool {
    matches!(ch, 'а'..='я' | 'А'..='Я' | 'ё' | 'Ё')
}

fn layout_hwnd_hint(window_id: &str, control_id: &str) -> Option<isize> {
    parse_last_hwnd_id(control_id).or_else(|| parse_last_hwnd_id(window_id))
}

fn parse_last_hwnd_id(value: &str) -> Option<isize> {
    let index = value.rfind("hwnd:")?;
    let hex = value[index + "hwnd:".len()..]
        .split(|ch: char| !ch.is_ascii_hexdigit())
        .next()?;
    if hex.is_empty() {
        return None;
    }
    isize::from_str_radix(hex, 16).ok()
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
    if let OperationError::CorrectionWithContext(correction, context) = error {
        let method = context
            .capabilities
            .method_binding
            .as_ref()
            .map(|binding| binding.context_method.as_str())
            .unwrap_or("unknown");
        return format!(
            "Correction({correction:?}); method={method}; control={}; caret={}..{}; selection={:?}; text_preview={}",
            context.control_id,
            context.caret_range.start,
            context.caret_range.end,
            context.selection_range,
            log_preview(&context.text_snapshot, 120)
        );
    }
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
