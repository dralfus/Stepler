use super::*;

pub(super) fn is_supported_terminal_class(app_class: &str, focused_class: &str) -> bool {
    app_class == "CASCADIA_HOSTING_WINDOW_CLASS"
        || app_class == "ConsoleWindowClass"
        || focused_class == "Windows.UI.Input.InputSite.WindowClass"
}

pub(super) fn is_classic_console_class(app_class: &str, focused_class: &str) -> bool {
    app_class.eq_ignore_ascii_case("ConsoleWindowClass")
        && focused_class.eq_ignore_ascii_case("ConsoleWindowClass")
}

#[cfg(windows)]
pub(super) fn foreground_is_classic_console() -> bool {
    let Ok(foreground) = foreground_hwnd() else {
        return false;
    };
    let focused = focused_window(foreground).unwrap_or(foreground);
    let app_class = window_class_name(foreground).unwrap_or_default();
    let focused_class = window_class_name(focused).unwrap_or_default();
    is_classic_console_class(&app_class, &focused_class)
}

#[cfg(windows)]
pub(super) fn foreground_terminal_passthrough() -> TerminalPassthrough {
    let Ok(foreground) = foreground_hwnd() else {
        return TerminalPassthrough::None;
    };
    let focused = focused_window(foreground).unwrap_or(foreground);
    let app_class = window_class_name(foreground).unwrap_or_default();
    let focused_class = window_class_name(focused).unwrap_or_default();
    let title = window_title(foreground).unwrap_or_default();
    let passthrough = terminal_passthrough_for_window(&app_class, &focused_class, &title);
    append_hotkey_signal_log(&format!(
        "hook_terminal_detect kind={passthrough:?} app={app_class:?} focused={focused_class:?} title={title:?}"
    ));
    passthrough
}

pub(super) fn terminal_passthrough_for_window(
    app_class: &str,
    focused_class: &str,
    title: &str,
) -> TerminalPassthrough {
    if !is_psreadline_passthrough_terminal_class(app_class, focused_class) {
        return TerminalPassthrough::None;
    }
    if is_cmd_terminal_title(&title) {
        return TerminalPassthrough::None;
    }
    if is_ssh_terminal_title(&title) {
        return TerminalPassthrough::Ssh;
    }
    if is_local_psreadline_terminal_title(title) {
        return TerminalPassthrough::PsReadLine;
    }
    TerminalPassthrough::UnknownTerminal
}

#[cfg(windows)]
pub(super) fn terminal_needs_conservative_suppression() -> bool {
    let Ok(foreground) = foreground_hwnd() else {
        return false;
    };
    let focused = focused_window(foreground).unwrap_or(foreground);
    let app_class = window_class_name(foreground).unwrap_or_default();
    let focused_class = window_class_name(focused).unwrap_or_default();
    terminal_class_needs_conservative_suppression(&app_class, &focused_class)
}

pub(super) fn terminal_class_needs_conservative_suppression(
    app_class: &str,
    focused_class: &str,
) -> bool {
    is_supported_terminal_class(app_class, focused_class)
        && !app_class.eq_ignore_ascii_case("ConsoleWindowClass")
        && !focused_class.eq_ignore_ascii_case("ConsoleWindowClass")
}

pub(super) fn is_psreadline_passthrough_terminal_class(
    app_class: &str,
    focused_class: &str,
) -> bool {
    app_class == "CASCADIA_HOSTING_WINDOW_CLASS"
        || focused_class == "Windows.UI.Input.InputSite.WindowClass"
}

pub(super) fn is_ssh_terminal_target(target: &ForegroundTarget) -> bool {
    is_psreadline_passthrough_terminal_class(&target.app_class, &target.focused_class)
        && is_ssh_terminal_title(&target.title)
}

pub(super) fn is_cmd_terminal_title(title: &str) -> bool {
    title.to_ascii_lowercase().contains("cmd.exe")
}

pub(super) fn is_local_psreadline_terminal_title(title: &str) -> bool {
    let title = title.to_ascii_lowercase();
    if title.trim().is_empty()
        || title.contains('@')
        || title.contains("ssh")
        || title.contains("vpn")
        || title.contains("root")
        || title.contains("linux")
        || title.contains("ubuntu")
        || title.contains("debian")
    {
        return false;
    }

    title == "windows powershell"
        || title == "powershell"
        || title == "pwsh"
        || title.starts_with("windows powershell ")
        || title.starts_with("powershell ")
        || title.starts_with("pwsh ")
        || title.starts_with("powershell 7")
}

pub(super) fn is_ssh_terminal_title(title: &str) -> bool {
    let title = title.trim().to_ascii_lowercase();
    if title.is_empty()
        || is_local_psreadline_terminal_title(&title)
        || is_cmd_terminal_title(&title)
    {
        return false;
    }

    title.contains("ssh")
        || title.contains('@')
        || title.starts_with("vpn")
        || title.starts_with("root")
        || title.starts_with("ubuntu")
        || title.starts_with("debian")
        || title.starts_with("linux")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TerminalPauseHandling {
    PassThrough,
    Suppress,
    TranslateToCtrlF12,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TerminalPassthrough {
    None,
    PsReadLine,
    Ssh,
    UnknownTerminal,
}
