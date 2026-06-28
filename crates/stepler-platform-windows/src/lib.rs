#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use stepler_core::{
    Capabilities, CorrectionMode, MethodBinding, MethodId, ReplacementPlan, TextContext, TextRange,
};
use stepler_platform::{
    classify_surface, probe_plan_for, surface_policy_for, web_keyboard_profile_for_surface,
    ApplyReplacementResult, ClipboardBackend, ClipboardFormatSnapshot, ClipboardSnapshot,
    ForegroundControl, ForegroundProvider, ForegroundTarget, HotkeyListener, MethodProbe,
    MethodResolver, PlatformError, SurfaceKind, TextContextProvider, TextReplacer,
    WebKeyboardProfile,
};

mod clipboard;
use clipboard::*;

mod clipboard_selection;
use clipboard_selection::*;

mod console_buffer;
use console_buffer::*;

mod diagnostics;
use diagnostics::{
    focus_diagnostics_impl, hotkey_failure_trace_summary_impl, method_diagnostics_impl,
    uia_focus_diagnostics,
};
pub use diagnostics::{
    WindowsFocusDiagnostics, WindowsMethodDiagnostics, WindowsMethodProbeDiagnostics,
    WindowsResolveTraceDiagnostics, WindowsSurfaceDiagnostics, WindowsUiaFocusDiagnostics,
};

mod encoding;
use encoding::*;

mod keyboard_input;
use keyboard_input::*;

mod layout_switcher;
pub use layout_switcher::WindowsLayoutSwitcher;
use layout_switcher::{find_layout_by_language, keyboard_layouts, switch_foreground_layout};

#[cfg(windows)]
mod powershell_scripts;
#[cfg(windows)]
use powershell_scripts::*;

mod send_input;
use send_input::*;

mod terminal_clipboard;
use terminal_clipboard::*;

mod terminal_helpers;
use terminal_helpers::*;

mod uia_text;
use uia_text::*;

mod web_keyboard;
use web_keyboard::*;

mod web_keyboard_profile;
use web_keyboard_profile::*;

mod web_keyboard_support;
use web_keyboard_support::*;

mod window_info;
use window_info::*;

mod win32_edit;
use win32_edit::*;

mod word_com;
use word_com::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardControlAction {
    SwitchToRussian,
    SwitchToEnglish,
    SwitchToNext,
}

#[cfg(windows)]
pub fn request_keyboard_control_action(action: KeyboardControlAction) -> Result<(), PlatformError> {
    let key = match action {
        KeyboardControlAction::SwitchToRussian => VK_LCONTROL,
        KeyboardControlAction::SwitchToEnglish => VK_RCONTROL,
        KeyboardControlAction::SwitchToNext => VK_APPS,
    };
    send_stepler_control_key(key)
}

#[cfg(not(windows))]
pub fn request_keyboard_control_action(
    _action: KeyboardControlAction,
) -> Result<(), PlatformError> {
    Err(PlatformError::Unsupported)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisteredHotkey {
    Pause,
    ScrollLock,
}

pub fn focus_diagnostics() -> Result<WindowsFocusDiagnostics, PlatformError> {
    focus_diagnostics_impl()
}

pub fn method_diagnostics() -> Result<WindowsMethodDiagnostics, PlatformError> {
    method_diagnostics_impl()
}

pub fn hotkey_failure_trace_summary(
    mode: CorrectionMode,
    final_error: &str,
) -> Result<String, PlatformError> {
    hotkey_failure_trace_summary_impl(mode, final_error)
}

#[cfg(windows)]
pub fn try_forward_embedded_terminal_hotkey(
    mode: stepler_core::CorrectionMode,
) -> Result<bool, PlatformError> {
    if !env_flag_enabled("STEPLER_ENABLE_EMBEDDED_TERMINAL_PSREADLINE", true) {
        return Ok(false);
    }

    let foreground = foreground_hwnd()?;
    if !foreground_is_codex_embedded_terminal(foreground) {
        return Ok(false);
    }

    let focus = uia_focus_diagnostics()?;
    if !is_embedded_terminal_uia_focus(&focus) {
        return Ok(false);
    }

    release_modifier_keys();
    std::thread::sleep(Duration::from_millis(20));
    match mode {
        stepler_core::CorrectionMode::Pause => {
            append_hotkey_signal_log("embedded_terminal_psreadline_forward chord=Ctrl+F11");
            send_key_chord_virtual(&[VK_CONTROL], VK_F11);
        }
        stepler_core::CorrectionMode::ScrollLock => {
            append_hotkey_signal_log("embedded_terminal_psreadline_forward chord=Ctrl+F12");
            send_key_chord_virtual(&[VK_CONTROL], VK_F12);
        }
    }
    release_modifier_keys();
    Ok(true)
}

#[cfg(not(windows))]
pub fn try_forward_embedded_terminal_hotkey(
    _mode: stepler_core::CorrectionMode,
) -> Result<bool, PlatformError> {
    Err(PlatformError::Unsupported)
}

fn is_embedded_terminal_uia_focus(focus: &WindowsUiaFocusDiagnostics) -> bool {
    focus
        .class_name
        .eq_ignore_ascii_case("xterm-helper-textarea")
}

#[cfg(windows)]
fn foreground_is_stepler_qwen_surface() -> bool {
    let Ok(hwnd) = foreground_hwnd() else {
        return false;
    };
    let title = window_title(hwnd).unwrap_or_default();
    if !matches!(
        title.as_str(),
        "Stepler Qwen Input" | "Stepler Qwen Workspace"
    ) {
        return false;
    }

    window_class_name(hwnd)
        .map(|class_name| class_name.starts_with("WindowsForms10.Window."))
        .unwrap_or(false)
}

impl RegisteredHotkey {
    #[cfg(windows)]
    pub fn message_loop<F>(mut on_hotkey: F) -> Result<(), PlatformError>
    where
        F: FnMut(stepler_core::CorrectionMode),
    {
        register_hotkey(HOTKEY_ID_PAUSE, 0, VK_PAUSE)?;
        register_hotkey(HOTKEY_ID_SCROLL_LOCK, MOD_CONTROL, VK_PAUSE)?;
        let _registrations = RegisteredHotkeyGuard;

        let mut message = Msg::default();
        loop {
            let result = unsafe { GetMessageW(&mut message as *mut Msg, 0, 0, 0) };
            if result == -1 {
                return Err(PlatformError::HotkeyUnavailable);
            }
            if result == 0 {
                return Ok(());
            }

            if message.message == WM_HOTKEY {
                match message.wparam as i32 {
                    HOTKEY_ID_PAUSE => {
                        on_hotkey(stepler_core::CorrectionMode::Pause);
                        drain_pending_hotkey_messages();
                    }
                    HOTKEY_ID_SCROLL_LOCK => {
                        on_hotkey(stepler_core::CorrectionMode::ScrollLock);
                        drain_pending_hotkey_messages();
                    }
                    _ => {}
                }
            }
        }
    }

    #[cfg(not(windows))]
    pub fn message_loop<F>(_on_hotkey: F) -> Result<(), PlatformError>
    where
        F: FnMut(stepler_core::CorrectionMode),
    {
        Err(PlatformError::Unsupported)
    }
}

#[cfg(windows)]
pub fn message_loop_with_keyboard_controls<F, H, U, G>(
    mut on_hotkey: F,
    mut on_hotkey_received: H,
    mut on_hotkey_unsupported: U,
    mut on_control: G,
) -> Result<(), PlatformError>
where
    F: FnMut(stepler_core::CorrectionMode),
    H: FnMut(stepler_core::CorrectionMode),
    U: FnMut(stepler_core::CorrectionMode),
    G: FnMut(KeyboardControlAction),
{
    install_keyboard_control_hook()?;
    let _hook = KeyboardControlHookGuard;

    let mut message = Msg::default();
    loop {
        let result = unsafe { GetMessageW(&mut message as *mut Msg, 0, 0, 0) };
        if result == -1 {
            return Err(PlatformError::HotkeyUnavailable);
        }
        if result == 0 {
            return Ok(());
        }

        match message.message {
            WM_HOTKEY => match message.wparam as i32 {
                HOTKEY_ID_PAUSE => {
                    append_hotkey_signal_log("wm_hotkey pause");
                    let mode = stepler_core::CorrectionMode::Pause;
                    on_hotkey_received(mode);
                    on_hotkey(mode);
                    drain_pending_hotkey_messages();
                }
                HOTKEY_ID_SCROLL_LOCK => {
                    append_hotkey_signal_log("wm_hotkey ctrl_pause");
                    let mode = stepler_core::CorrectionMode::ScrollLock;
                    on_hotkey_received(mode);
                    std::thread::sleep(Duration::from_millis(180));
                    release_modifier_keys();
                    on_hotkey(mode);
                    drain_pending_hotkey_messages();
                }
                _ => {}
            },
            WM_STEPLER_HOTKEY => {
                if let Some(mode) = correction_mode_from_message_id(message.wparam) {
                    append_hotkey_signal_log(&format!("hook_message {mode:?}"));
                    on_hotkey_received(mode);
                    if mode == stepler_core::CorrectionMode::ScrollLock {
                        std::thread::sleep(Duration::from_millis(180));
                        release_modifier_keys();
                    }
                    on_hotkey(mode);
                    drain_pending_hotkey_messages();
                }
            }
            WM_STEPLER_HOTKEY_RECEIVED => {
                if let Some(mode) = correction_mode_from_message_id(message.wparam) {
                    append_hotkey_signal_log(&format!("hook_message_received {mode:?}"));
                    on_hotkey_received(mode);
                }
            }
            WM_STEPLER_HOTKEY_UNSUPPORTED => {
                if let Some(mode) = correction_mode_from_message_id(message.wparam) {
                    append_hotkey_signal_log(&format!("hook_message_unsupported {mode:?}"));
                    on_hotkey_unsupported(mode);
                }
            }
            WM_STEPLER_KEYBOARD_CONTROL => {
                if let Some(action) = KeyboardControlAction::from_message_id(message.wparam) {
                    on_control(action);
                }
            }
            _ => {}
        }
    }
}

#[cfg(not(windows))]
pub fn message_loop_with_keyboard_controls<F, H, U, G>(
    _on_hotkey: F,
    _on_hotkey_received: H,
    _on_hotkey_unsupported: U,
    _on_control: G,
) -> Result<(), PlatformError>
where
    F: FnMut(stepler_core::CorrectionMode),
    H: FnMut(stepler_core::CorrectionMode),
    U: FnMut(stepler_core::CorrectionMode),
    G: FnMut(KeyboardControlAction),
{
    Err(PlatformError::Unsupported)
}

#[cfg(windows)]
pub fn request_keyboard_control_message_loop_stop() -> bool {
    KEYBOARD_CONTROL_THREAD_ID
        .get()
        .copied()
        .map(|thread_id| unsafe { PostThreadMessageW(thread_id, WM_QUIT, 0, 0) != 0 })
        .unwrap_or(false)
}

#[cfg(not(windows))]
pub fn request_keyboard_control_message_loop_stop() -> bool {
    false
}

#[cfg(windows)]
pub fn install_console_modifier_release_handler() -> Result<(), PlatformError> {
    let ok = unsafe { SetConsoleCtrlHandler(Some(console_ctrl_handler), 1) };
    if ok == 0 {
        return Err(PlatformError::Unsupported);
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn install_console_modifier_release_handler() -> Result<(), PlatformError> {
    Ok(())
}

#[cfg(windows)]
pub fn release_modifier_keys() {
    for key in [
        VK_LCONTROL,
        VK_RCONTROL,
        VK_CONTROL,
        VK_LMENU,
        VK_RMENU,
        VK_MENU,
        VK_LSHIFT,
        VK_RSHIFT,
        VK_SHIFT,
    ] {
        if !should_release_modifier_key(key) {
            continue;
        }
        unsafe {
            keybd_event(key as u8, 0, KEYEVENTF_KEYUP, 0);
        }
    }
    let events = [
        VK_LCONTROL,
        VK_RCONTROL,
        VK_CONTROL,
        VK_LMENU,
        VK_RMENU,
        VK_MENU,
        VK_LSHIFT,
        VK_RSHIFT,
        VK_SHIFT,
    ]
    .iter()
    .copied()
    .filter(|key| should_release_modifier_key(*key))
    .map(|key| KeyboardInputEvent::new(key, true, KeyboardInputMode::VirtualKey))
    .collect::<Vec<_>>();
    if !events.is_empty() {
        let _ = send_keyboard_input(&events);
    }
}

#[cfg(not(windows))]
pub fn release_modifier_keys() {}

#[cfg(windows)]
fn should_release_modifier_key(vk: u32) -> bool {
    match vk {
        VK_LMENU | VK_RMENU | VK_MENU => (unsafe { GetAsyncKeyState(vk as i32) }) < 0,
        _ => true,
    }
}

impl KeyboardControlAction {
    fn message_id(self) -> usize {
        match self {
            Self::SwitchToRussian => 1,
            Self::SwitchToEnglish => 2,
            Self::SwitchToNext => 3,
        }
    }

    fn from_message_id(value: usize) -> Option<Self> {
        match value {
            1 => Some(Self::SwitchToRussian),
            2 => Some(Self::SwitchToEnglish),
            3 => Some(Self::SwitchToNext),
            _ => None,
        }
    }
}

#[cfg(windows)]
unsafe extern "system" fn console_ctrl_handler(_ctrl_type: u32) -> i32 {
    release_modifier_keys();
    0
}

fn correction_mode_message_id(mode: stepler_core::CorrectionMode) -> usize {
    match mode {
        stepler_core::CorrectionMode::Pause => 1,
        stepler_core::CorrectionMode::ScrollLock => 2,
    }
}

fn correction_mode_from_message_id(value: usize) -> Option<stepler_core::CorrectionMode> {
    match value {
        1 => Some(stepler_core::CorrectionMode::Pause),
        2 => Some(stepler_core::CorrectionMode::ScrollLock),
        _ => None,
    }
}

#[derive(Debug, Default)]
pub struct WindowsForegroundProvider;

impl ForegroundProvider for WindowsForegroundProvider {
    fn foreground_control(&self) -> Result<ForegroundControl, PlatformError> {
        foreground_control()
    }
}

#[derive(Debug, Default)]
pub struct WindowsTextContextProvider;

impl TextContextProvider for WindowsTextContextProvider {
    fn text_context(&self) -> Result<TextContext, PlatformError> {
        text_context()
    }
}

#[derive(Debug, Default)]
pub struct WindowsTextReplacer;

impl TextReplacer for WindowsTextReplacer {
    fn apply_replacement(
        &self,
        context: &TextContext,
        plan: &ReplacementPlan,
    ) -> Result<ApplyReplacementResult, PlatformError> {
        apply_replacement(context, plan)
    }
}

#[cfg(windows)]
#[derive(Debug, Default, Clone, Copy)]
struct SshTerminalMethod;

#[cfg(windows)]
impl SshTerminalMethod {
    fn probe(&self, target: &ForegroundTarget) -> Option<MethodProbe> {
        is_ssh_terminal_target(target).then(|| {
            MethodProbe::unsupported(
                MethodId::SshTerminal,
                "ssh terminal detected; no safe local line-editing API",
            )
        })
    }
}

fn looks_like_hotkeyhandler_marker(text: &str) -> bool {
    let trimmed = text.trim();
    trimmed.starts_with("__HKH_") || (trimmed.starts_with("__") && trimmed.contains("_MARKER_"))
}

#[derive(Debug, Default)]
pub struct WindowsClipboardBackend;

impl ClipboardBackend for WindowsClipboardBackend {
    fn capture(&self) -> Result<ClipboardSnapshot, PlatformError> {
        capture_clipboard()
    }

    fn restore(&self, snapshot: ClipboardSnapshot) -> Result<(), PlatformError> {
        restore_clipboard(snapshot)
    }
}

#[derive(Debug, Default)]
pub struct WindowsHotkeyListener {
    running: bool,
}

impl WindowsHotkeyListener {
    pub fn is_running(&self) -> bool {
        self.running
    }
}

impl HotkeyListener for WindowsHotkeyListener {
    fn start(&mut self) -> Result<(), PlatformError> {
        self.running = true;
        Ok(())
    }

    fn stop(&mut self) -> Result<(), PlatformError> {
        self.running = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests;

#[cfg(windows)]
fn foreground_control() -> Result<ForegroundControl, PlatformError> {
    let hwnd = foreground_hwnd()?;

    let class_name = window_class_name(hwnd).unwrap_or_else(|| String::from("unknown"));
    Ok(ForegroundControl {
        app_id: class_name,
        window_id: format!("hwnd:{hwnd:X}"),
        control_id: format!("hwnd:{hwnd:X}"),
    })
}

#[cfg(windows)]
fn text_context() -> Result<TextContext, PlatformError> {
    let foreground = foreground_hwnd()?;

    let focused = focused_window(foreground).unwrap_or(foreground);
    let app_class = window_class_name(foreground).unwrap_or_else(|| String::from("unknown"));
    let focused_class = window_class_name(focused).unwrap_or_else(|| String::from("unknown"));
    let mut title = window_title(foreground).unwrap_or_default();
    if let Some(marker_title) = active_terminal_app_marker_title() {
        if !title.contains(marker_title) {
            title = format!("{title} {marker_title}");
        }
    }
    let target = ForegroundTarget {
        app_class: app_class.clone(),
        focused_class: focused_class.clone(),
        title,
        process_name: window_process_name(foreground),
        window_id: hwnd_id(foreground),
        control_id: hwnd_id(focused),
    };
    let probes = windows_method_probes(&target);
    let resolver = MethodResolver::default();
    let mode = active_correction_mode();
    let mut remaining = probes;
    let mut last_unavailable = None;
    while !remaining.is_empty() {
        let decision = match resolver.resolve_for_mode(&target, &remaining, mode) {
            Ok(decision) => decision,
            Err(_) => {
                if let Some(error) = last_unavailable {
                    return Err(error);
                }
                return Err(PlatformError::UnsupportedControl {
                    app_class: app_class.clone(),
                    focused_class: focused_class.clone(),
                });
            }
        };

        match capture_by_method(
            &target,
            decision.context_method,
            foreground,
            focused,
            &app_class,
            &focused_class,
        ) {
            Ok(context) => return Ok(context),
            Err(error @ PlatformError::ReplacementUnavailable)
            | Err(error @ PlatformError::ReplacementUnavailableReason(_)) => {
                last_unavailable = Some(error);
                remaining.retain(|probe| probe.method_id != decision.context_method);
            }
            Err(error) => return Err(error),
        }
    }

    if let Some(error) = last_unavailable {
        return Err(error);
    }

    Err(PlatformError::UnsupportedControl {
        app_class,
        focused_class,
    })
}

#[cfg(windows)]
fn capture_by_method(
    target: &ForegroundTarget,
    method: MethodId,
    foreground: isize,
    focused: isize,
    app_class: &str,
    focused_class: &str,
) -> Result<TextContext, PlatformError> {
    match method {
        MethodId::Win32EditMessages => Win32EditMessagesMethod.capture(
            foreground,
            focused,
            app_class.to_owned(),
            focused_class.to_owned(),
        ),
        MethodId::ConsoleBuffer => {
            ConsoleBufferMethod.capture(foreground, focused, app_class, focused_class)
        }
        MethodId::TerminalClipboardShortcut => {
            TerminalClipboardShortcutMethod.capture(foreground, focused, app_class, focused_class)
        }
        MethodId::WordCom => WordComMethod.capture(foreground, focused, app_class, focused_class),
        MethodId::UiAutomationEditableText => {
            UiAutomationEditableTextMethod.capture(foreground, focused, app_class, focused_class)
        }
        MethodId::UiAutomationDocumentText => UiAutomationDocumentTextMethod.capture_with_options(
            foreground,
            focused,
            app_class,
            focused_class,
            allow_uia_document_caret_fallback(target),
        ),
        MethodId::UiAutomationText => {
            UiAutomationTextMethod.capture(foreground, focused, app_class, focused_class)
        }
        MethodId::XtermKeyboardSelection => {
            XtermKeyboardSelectionMethod.capture(foreground, focused, app_class, focused_class)
        }
        MethodId::WebKeyboardSelection => {
            WebKeyboardSelectionMethod.capture(foreground, focused, app_class, focused_class)
        }
        MethodId::ClipboardSelection => {
            ClipboardSelectionMethod.capture(foreground, focused, app_class, focused_class)
        }
        _ => Err(PlatformError::ReplacementUnavailable),
    }
}

#[cfg(windows)]
fn windows_method_probes(target: &ForegroundTarget) -> Vec<MethodProbe> {
    windows_runtime_probe_methods(target)
        .into_iter()
        .filter_map(|method| probe_method_by_id(method, target))
        .collect()
}

#[cfg(windows)]
fn windows_probe_plan_methods(target: &ForegroundTarget) -> Vec<MethodId> {
    probe_plan_for(target).probe_methods
}

#[cfg(windows)]
fn windows_runtime_probe_methods(target: &ForegroundTarget) -> Vec<MethodId> {
    windows_probe_plan_methods(target)
}

#[cfg(windows)]
fn probe_method_by_id(method: MethodId, target: &ForegroundTarget) -> Option<MethodProbe> {
    match method {
        MethodId::Win32EditMessages => Win32EditMessagesMethod.probe(target),
        MethodId::TerminalClipboardShortcut => TerminalClipboardShortcutMethod.probe(target),
        MethodId::SshTerminal => SshTerminalMethod.probe(target),
        MethodId::ConsoleBuffer => ConsoleBufferMethod.probe(target),
        MethodId::PsReadLine => Some(MethodProbe::safe(
            MethodId::PsReadLine,
            "PowerShell PSReadLine adapter",
        )),
        MethodId::WordCom => WordComMethod.probe(target),
        MethodId::UiAutomationEditableText => UiAutomationEditableTextMethod.probe(target),
        MethodId::UiAutomationDocumentText => UiAutomationDocumentTextMethod.probe(target),
        MethodId::UiAutomationText => UiAutomationTextMethod.probe(target),
        MethodId::XtermKeyboardSelection => XtermKeyboardSelectionMethod.probe(target),
        MethodId::WebKeyboardSelection => WebKeyboardSelectionMethod.probe(target),
        MethodId::ClipboardSelection => ClipboardSelectionMethod.probe(target),
        MethodId::SendInput => SendInputMethod.probe(target),
    }
}

#[cfg(not(windows))]
fn text_context() -> Result<TextContext, PlatformError> {
    Err(PlatformError::Unsupported)
}

#[cfg(windows)]
fn apply_replacement(
    context: &TextContext,
    plan: &ReplacementPlan,
) -> Result<ApplyReplacementResult, PlatformError> {
    match context_replacement_method(context) {
        Some(MethodId::Win32EditMessages) => Win32EditMessagesMethod.apply(context, plan),
        Some(MethodId::ConsoleBuffer) => ConsoleBufferMethod.apply(context, plan),
        Some(MethodId::TerminalClipboardShortcut) => {
            TerminalClipboardShortcutMethod.apply(context, plan)
        }
        Some(MethodId::WordCom) => WordComMethod.apply(context, plan),
        Some(MethodId::UiAutomationEditableText) => {
            UiAutomationEditableTextMethod.apply(context, plan)
        }
        Some(MethodId::UiAutomationDocumentText) => {
            UiAutomationDocumentTextMethod.apply(context, plan)
        }
        Some(MethodId::UiAutomationText) => UiAutomationTextMethod.apply(context, plan),
        Some(MethodId::XtermKeyboardSelection) => XtermKeyboardSelectionMethod.apply(context, plan),
        Some(MethodId::WebKeyboardSelection) => WebKeyboardSelectionMethod.apply(context, plan),
        Some(MethodId::ClipboardSelection) => ClipboardSelectionMethod.apply(context, plan),
        Some(MethodId::SendInput) => SendInputMethod.apply(context, plan),
        Some(_) => Err(PlatformError::ReplacementUnavailable),
        None => Err(PlatformError::ReplacementUnavailableReason(String::from(
            "missing_method_binding",
        ))),
    }
}

fn context_replacement_method(context: &TextContext) -> Option<MethodId> {
    context
        .capabilities
        .method_binding
        .as_ref()
        .and_then(|binding| binding.replace_methods.first().copied())
}

#[cfg(not(windows))]
fn apply_replacement(
    _context: &TextContext,
    _plan: &ReplacementPlan,
) -> Result<ApplyReplacementResult, PlatformError> {
    Err(PlatformError::Unsupported)
}

#[cfg(not(windows))]
fn foreground_control() -> Result<ForegroundControl, PlatformError> {
    Err(PlatformError::Unsupported)
}

fn is_supported_edit_class(class_name: &str) -> bool {
    let class_name = class_name.to_ascii_lowercase();
    class_name == "edit" || class_name.starts_with("richedit")
}

fn is_word_target(target: &ForegroundTarget) -> bool {
    target
        .process_name
        .as_deref()
        .is_some_and(|process| process.eq_ignore_ascii_case("WINWORD"))
        || target.app_class.eq_ignore_ascii_case("OpusApp")
        || target.focused_class.eq_ignore_ascii_case("_WwG")
        || target.title.to_ascii_lowercase().contains("word")
}

fn is_outlook_word_editor_target(target: &ForegroundTarget) -> bool {
    target
        .process_name
        .as_deref()
        .is_some_and(|process| process.eq_ignore_ascii_case("OUTLOOK"))
        && target.focused_class.eq_ignore_ascii_case("_WwG")
}

fn is_outlook_class_or_process(
    app_class: &str,
    focused_class: &str,
    process_name: Option<&str>,
) -> bool {
    process_name.is_some_and(|process| process.eq_ignore_ascii_case("OUTLOOK"))
        || app_class.eq_ignore_ascii_case("rctrl_renwnd32")
        || focused_class.eq_ignore_ascii_case("_WwG")
            && app_class.to_ascii_lowercase().contains("outlook")
}

#[cfg(windows)]
fn foreground_surface_kind(foreground: isize, app_class: &str, focused_class: &str) -> SurfaceKind {
    let title = window_title(foreground).unwrap_or_default();
    classify_surface(&ForegroundTarget {
        app_class: app_class.to_owned(),
        focused_class: focused_class.to_owned(),
        title,
        process_name: None,
        window_id: hwnd_id(foreground),
        control_id: String::new(),
    })
    .kind
}

#[cfg(windows)]
fn focused_is_xterm_textarea() -> bool {
    uia_focus_diagnostics()
        .map(|focus| is_embedded_terminal_uia_focus(&focus))
        .unwrap_or(false)
}

#[cfg(windows)]
fn is_xterm_terminal_target(target: &ForegroundTarget) -> bool {
    is_supported_terminal_class(&target.app_class, &target.focused_class)
        && focused_is_xterm_textarea()
}

#[cfg(windows)]
fn foreground_is_codex_embedded_terminal(foreground: isize) -> bool {
    foreground_is_codex_embedded_terminal_cached(foreground, true)
}

#[cfg(windows)]
fn refresh_foreground_is_codex_embedded_terminal(foreground: isize) -> bool {
    foreground_is_codex_embedded_terminal_cached(foreground, false)
}

#[cfg(windows)]
fn foreground_is_codex_embedded_terminal_cached(foreground: isize, allow_cached: bool) -> bool {
    let title = window_title(foreground).unwrap_or_default();
    if !title.eq_ignore_ascii_case("Codex") {
        return false;
    }

    static CACHE: OnceLock<Mutex<Option<(isize, Instant, bool)>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(None));
    if allow_cached {
        if let Ok(guard) = cache.lock() {
            if let Some((cached_foreground, cached_at, cached_value)) = *guard {
                if cached_foreground == foreground
                    && cached_at.elapsed() < Duration::from_millis(750)
                {
                    return cached_value;
                }
            }
        }
    }

    let value = focused_is_xterm_textarea();
    if let Ok(mut guard) = cache.lock() {
        *guard = Some((foreground, Instant::now(), value));
    }
    value
}

fn allow_uia_document_caret_fallback(target: &ForegroundTarget) -> bool {
    classify_surface(target).kind == SurfaceKind::StickyNotes
}

fn active_correction_mode_is_scrolllock() -> bool {
    std::env::var("STEPLER_ACTIVE_CORRECTION_MODE")
        .map(|value| value.eq_ignore_ascii_case("scrolllock"))
        .unwrap_or(false)
}

fn active_correction_mode() -> stepler_core::CorrectionMode {
    if active_correction_mode_is_scrolllock() {
        stepler_core::CorrectionMode::ScrollLock
    } else {
        stepler_core::CorrectionMode::Pause
    }
}

fn parse_word_com_base(control_id: &str) -> Option<usize> {
    let rest = control_id
        .strip_prefix("word-com:")
        .or_else(|| control_id.strip_prefix("outlook-word-com:"))?;
    rest.split(':').next()?.parse().ok()
}

fn parse_uia_runtime_id(control_id: &str) -> Option<String> {
    let rest = control_id.strip_prefix("uia:")?;
    Some(rest.split(':').next()?.to_owned())
}

fn parse_uia_document_runtime_id(control_id: &str) -> Option<String> {
    let rest = control_id
        .strip_prefix("uia-doc:")
        .or_else(|| control_id.strip_prefix("uia-doc-caret:"))?;
    Some(rest.split(':').next()?.to_owned())
}

#[cfg(windows)]
fn run_powershell_script(script: &str, env: &[(&str, String)]) -> Result<String, PlatformError> {
    let encoded = encode_utf16le_base64(script);
    let mut command = std::process::Command::new("powershell.exe");
    command
        .arg("-NoProfile")
        .arg("-NonInteractive")
        .arg("-EncodedCommand")
        .arg(encoded)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    for (key, value) in env {
        command.env(key, value);
    }
    command.creation_flags(CREATE_NO_WINDOW);

    let mut child = command
        .spawn()
        .map_err(|_| PlatformError::ReplacementUnavailable)?;
    let started = Instant::now();
    let timeout = Duration::from_secs(5);
    loop {
        match child
            .try_wait()
            .map_err(|_| PlatformError::ReplacementUnavailable)?
        {
            Some(status) => {
                let output = child
                    .wait_with_output()
                    .map_err(|_| PlatformError::ReplacementUnavailable)?;
                if !status.success() {
                    return Err(PlatformError::ReplacementUnavailable);
                }
                return Ok(String::from_utf8_lossy(&output.stdout).to_string());
            }
            None if started.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(PlatformError::ReplacementUnavailable);
            }
            None => std::thread::sleep(Duration::from_millis(20)),
        }
    }
}

#[cfg(windows)]
#[derive(Debug, Clone, PartialEq, Eq)]
struct TerminalSelectionCapture {
    text: String,
    selection_kind: TerminalSelectionKind,
}

#[cfg(windows)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TerminalSelectionKind {
    LeftOfCaret,
    PreviousWord,
}

#[cfg(windows)]
impl TerminalSelectionKind {
    fn id(self) -> &'static str {
        match self {
            Self::LeftOfCaret => "left",
            Self::PreviousWord => "word",
        }
    }

    fn from_control_id(control_id: &str) -> Self {
        if control_id.ends_with(":word") {
            Self::PreviousWord
        } else {
            Self::LeftOfCaret
        }
    }
}

#[cfg(windows)]
fn read_terminal_left_text() -> Result<TerminalSelectionCapture, PlatformError> {
    let snapshot = capture_clipboard()?;
    let captured = copy_terminal_selection_checked(&snapshot, TerminalSelectionKind::LeftOfCaret)
        .or_else(|| {
            copy_terminal_selection_checked(&snapshot, TerminalSelectionKind::PreviousWord)
        });
    let restore_result = restore_clipboard(snapshot);
    clear_terminal_selection_state();
    restore_result?;

    captured.ok_or_else(|| {
        PlatformError::ReplacementUnavailableReason(String::from(
            "terminal_capture_empty:left,word",
        ))
    })
}

#[cfg(windows)]
fn copy_terminal_selection_checked(
    snapshot: &ClipboardSnapshot,
    selection_kind: TerminalSelectionKind,
) -> Option<TerminalSelectionCapture> {
    send_key(VK_END);
    std::thread::sleep(Duration::from_millis(20));
    match selection_kind {
        TerminalSelectionKind::LeftOfCaret => send_key_chord(&[VK_LSHIFT], VK_HOME),
        TerminalSelectionKind::PreviousWord => send_key_chord(&[VK_CONTROL, VK_LSHIFT], VK_LEFT),
    }
    std::thread::sleep(Duration::from_millis(60));

    let marker = format!(
        "__STEPLER_TERMINAL_COPY_MARKER_{}_{}__",
        snapshot.sequence_number.unwrap_or(0),
        selection_kind.id()
    );
    let copied = copy_current_terminal_selection_with_variants(&marker, selection_kind);
    clear_terminal_selection_state();

    copied
        .map(|text| text.trim_end_matches(['\r', '\n']).to_owned())
        .filter(|text| !text.trim().is_empty())
        .filter(|text| !looks_like_hotkeyhandler_marker(text))
        .map(|text| TerminalSelectionCapture {
            text,
            selection_kind,
        })
}

#[cfg(windows)]
fn copy_current_terminal_selection_with_variants(
    marker: &str,
    selection_kind: TerminalSelectionKind,
) -> Option<String> {
    for variant in TerminalCopyVariant::all() {
        let _ = restore_clipboard(clipboard_snapshot_from_text(marker));
        release_modifier_keys();
        std::thread::sleep(Duration::from_millis(15));
        variant.send();
        let copied = wait_for_clipboard_text_different_from(marker, Duration::from_millis(450));
        append_hotkey_signal_log(&format!(
            "terminal_copy variant={} selection={} copied={}",
            variant.id(),
            selection_kind.id(),
            copied
                .as_ref()
                .map(|text| text.encode_utf16().count().to_string())
                .unwrap_or_else(|| String::from("none"))
        ));
        if copied.is_some() {
            return copied;
        }
    }

    None
}

#[cfg(windows)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TerminalCopyVariant {
    CtrlShiftC,
    CtrlInsert,
    CtrlC,
}

#[cfg(windows)]
impl TerminalCopyVariant {
    fn all() -> [Self; 3] {
        [Self::CtrlShiftC, Self::CtrlInsert, Self::CtrlC]
    }

    fn id(self) -> &'static str {
        match self {
            Self::CtrlShiftC => "ctrl_shift_c",
            Self::CtrlInsert => "ctrl_insert",
            Self::CtrlC => "ctrl_c",
        }
    }

    fn send(self) {
        match self {
            Self::CtrlShiftC => {
                send_terminal_shortcut_with_english_layout(&[VK_CONTROL, VK_SHIFT], VK_C)
            }
            Self::CtrlInsert => send_key_chord_virtual(&[VK_CONTROL], VK_INSERT),
            Self::CtrlC => send_terminal_shortcut_with_english_layout(&[VK_CONTROL], VK_C),
        }
    }
}

#[cfg(windows)]
fn clear_terminal_selection_state() {
    release_modifier_keys();
    send_key(VK_ESCAPE);
    std::thread::sleep(Duration::from_millis(10));
    send_key(VK_END);
    std::thread::sleep(Duration::from_millis(10));
    release_modifier_keys();
}

#[cfg(windows)]
fn read_console_input_text(hwnd: isize) -> Result<String, PlatformError> {
    let process_id = window_process_id(hwnd)?;
    let _attachment = ConsoleAttachment::attach(process_id)?;
    let output = unsafe { GetStdHandle(STD_OUTPUT_HANDLE) };
    if output == 0 || output == INVALID_HANDLE_VALUE {
        return Err(PlatformError::ReplacementUnavailable);
    }

    let mut info = ConsoleScreenBufferInfo::default();
    if unsafe { GetConsoleScreenBufferInfo(output, &mut info as *mut ConsoleScreenBufferInfo) } == 0
    {
        return Err(PlatformError::ReplacementUnavailable);
    }

    let width = info.size.x.max(1) as usize;
    let cursor_x = info.cursor_position.x.max(0) as usize;
    let row = info.cursor_position.y.max(0);
    let mut buffer = vec![0u16; width];
    let mut read = 0u32;
    let ok = unsafe {
        ReadConsoleOutputCharacterW(
            output,
            buffer.as_mut_ptr(),
            buffer.len() as u32,
            Coord { x: 0, y: row },
            &mut read as *mut u32,
        )
    };
    if ok == 0 {
        return Err(PlatformError::ReplacementUnavailable);
    }

    let line_width = cursor_x.min(read as usize).min(buffer.len());
    let line = String::from_utf16_lossy(&buffer[..line_width]);
    let input = console_input_from_prompt_line(&line);
    if input.trim().is_empty() {
        return Err(PlatformError::ReplacementUnavailable);
    }

    Ok(input)
}

fn console_input_from_prompt_line(line: &str) -> String {
    let trimmed_end = line.trim_end_matches(' ');
    for marker in ["> ", ">"] {
        if let Some(index) = trimmed_end.rfind(marker) {
            return trimmed_end[index + marker.len()..].to_owned();
        }
    }
    trimmed_end.trim_start().to_owned()
}

#[cfg(windows)]
struct ConsoleAttachment;

#[cfg(windows)]
impl ConsoleAttachment {
    fn attach(process_id: u32) -> Result<Self, PlatformError> {
        unsafe {
            FreeConsole();
            if AttachConsole(process_id) == 0 {
                return Err(PlatformError::ReplacementUnavailable);
            }
        }
        Ok(Self)
    }
}

#[cfg(windows)]
impl Drop for ConsoleAttachment {
    fn drop(&mut self) {
        unsafe {
            FreeConsole();
        }
    }
}

fn replace_range_text(text: &str, range: TextRange, replacement: &str) -> Option<String> {
    slice_by_range(text, range)?;
    let mut result = String::new();
    result.push_str(&text[..range.start]);
    result.push_str(replacement);
    result.push_str(&text[range.end..]);
    Some(result)
}

fn preview_for_error(text: &str, limit: usize) -> String {
    let mut preview = text.chars().take(limit).collect::<String>();
    if text.chars().count() > limit {
        preview.push_str("...");
    }
    preview.replace('\r', "\\r").replace('\n', "\\n")
}

#[cfg(windows)]
fn wait_for_clipboard_selection_text(
    text_before: Option<&str>,
    sequence_before: Option<u32>,
    timeout: Duration,
) -> Option<String> {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if let Ok(snapshot) = capture_clipboard() {
            if let Some(text) = snapshot.text {
                let sequence_changed = match (sequence_before, snapshot.sequence_number) {
                    (Some(before), Some(after)) => before != after,
                    _ => false,
                };
                let text_changed = text_before != Some(text.as_str());
                if sequence_changed || (sequence_before.is_none() && text_changed) {
                    return Some(text);
                }
            }
        }
        std::thread::sleep(Duration::from_millis(15));
    }
    None
}

#[cfg(windows)]
fn copy_selected_text_checked(snapshot: &ClipboardSnapshot, timeout: Duration) -> Option<String> {
    copy_selected_text_checked_with_chord(snapshot, &[VK_CONTROL], VK_C, timeout)
}

#[cfg(windows)]
fn copy_selected_text_checked_with_chord(
    snapshot: &ClipboardSnapshot,
    modifiers: &[u32],
    key: u32,
    timeout: Duration,
) -> Option<String> {
    copy_selected_text_checked_with_chord_and_clipboard_timeout(
        snapshot,
        modifiers,
        key,
        timeout,
        Duration::from_millis(450),
    )
}

#[cfg(windows)]
fn copy_selected_text_checked_with_chord_and_clipboard_timeout(
    snapshot: &ClipboardSnapshot,
    modifiers: &[u32],
    key: u32,
    timeout: Duration,
    clipboard_timeout: Duration,
) -> Option<String> {
    let marker = format!(
        "__STEPLER_COPY_MARKER_{}__",
        snapshot.sequence_number.unwrap_or(0)
    );
    restore_clipboard_with_timeout(clipboard_snapshot_from_text(&marker), clipboard_timeout)
        .ok()?;
    release_modifier_keys();
    std::thread::sleep(Duration::from_millis(8));
    send_key_chord_virtual(modifiers, key);
    wait_for_clipboard_text_different_from_with_clipboard_timeout(
        &marker,
        timeout,
        clipboard_timeout,
    )
}

#[cfg(windows)]
fn wait_for_clipboard_text_different_from(marker: &str, timeout: Duration) -> Option<String> {
    wait_for_clipboard_text_different_from_with_clipboard_timeout(
        marker,
        timeout,
        Duration::from_millis(450),
    )
}

#[cfg(windows)]
fn wait_for_clipboard_text_different_from_with_clipboard_timeout(
    marker: &str,
    timeout: Duration,
    clipboard_timeout: Duration,
) -> Option<String> {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if let Ok(snapshot) = capture_clipboard_text_only_with_timeout(clipboard_timeout) {
            if let Some(text) = snapshot.text {
                if text != marker {
                    return Some(text);
                }
            }
        }
        std::thread::sleep(Duration::from_millis(8));
    }
    None
}

#[cfg(windows)]
fn select_web_left_context() {
    let modifiers = [
        KeyboardInputEvent::new(VK_CONTROL, false, KeyboardInputMode::ScanCode),
        KeyboardInputEvent::new(VK_LSHIFT, false, KeyboardInputMode::ScanCode),
    ];
    let _ = send_keyboard_input(&modifiers);
    std::thread::sleep(Duration::from_millis(25));

    let mut events = Vec::new();
    for _ in 0..6 {
        events.push(KeyboardInputEvent::new(
            VK_LEFT,
            false,
            KeyboardInputMode::ScanCode,
        ));
        events.push(KeyboardInputEvent::new(
            VK_LEFT,
            true,
            KeyboardInputMode::ScanCode,
        ));
    }
    events.push(KeyboardInputEvent::new(
        VK_LSHIFT,
        true,
        KeyboardInputMode::ScanCode,
    ));
    events.push(KeyboardInputEvent::new(
        VK_CONTROL,
        true,
        KeyboardInputMode::ScanCode,
    ));
    let _ = send_keyboard_input(&events);
    release_modifier_keys();
}

#[cfg(windows)]
fn select_web_line_left_context() {
    let shift_down = [KeyboardInputEvent::new(
        VK_LSHIFT,
        false,
        KeyboardInputMode::ScanCode,
    )];
    let _ = send_keyboard_input(&shift_down);
    std::thread::sleep(Duration::from_millis(25));
    let _ = send_keyboard_input(&[
        KeyboardInputEvent::new(VK_HOME, false, KeyboardInputMode::ScanCode),
        KeyboardInputEvent::new(VK_HOME, true, KeyboardInputMode::ScanCode),
        KeyboardInputEvent::new(VK_LSHIFT, true, KeyboardInputMode::ScanCode),
    ]);
    release_modifier_keys();
}

#[cfg(windows)]
fn select_web_all_context() {
    send_key_chord_virtual(&[VK_CONTROL], VK_A);
    release_modifier_keys();
}

#[cfg(windows)]
fn is_plausible_web_field_text(text: &str) -> bool {
    let trimmed = text.trim();
    !trimmed.is_empty()
        && text.len() <= 512
        && !text.contains('\r')
        && !text.contains('\n')
        && !looks_like_browser_document_dump(text)
}

#[cfg(windows)]
fn select_left_utf16_units(count: usize) -> Result<(), PlatformError> {
    if count == 0 || count > 512 {
        return Err(PlatformError::PreflightFailed);
    }

    let shift_down = [KeyboardInputEvent::new(
        VK_LSHIFT,
        false,
        KeyboardInputMode::ScanCode,
    )];
    if !send_keyboard_input(&shift_down) {
        return Err(PlatformError::Unsupported);
    }
    std::thread::sleep(Duration::from_millis(25));

    let mut events = Vec::with_capacity(count * 2 + 1);
    for _ in 0..count {
        events.push(KeyboardInputEvent::new(
            VK_LEFT,
            false,
            KeyboardInputMode::ScanCode,
        ));
        events.push(KeyboardInputEvent::new(
            VK_LEFT,
            true,
            KeyboardInputMode::ScanCode,
        ));
    }
    events.push(KeyboardInputEvent::new(
        VK_LSHIFT,
        true,
        KeyboardInputMode::ScanCode,
    ));

    send_keyboard_input(&events)
        .then_some(())
        .ok_or(PlatformError::Unsupported)?;
    release_modifier_keys();
    Ok(())
}

#[cfg(windows)]
fn extend_web_selection_to_expected_prefix(
    selected: Option<String>,
    expected: &str,
    snapshot: &ClipboardSnapshot,
    timeout: Duration,
) -> Option<String> {
    let selected_text = selected.as_deref()?;
    if selected_text.is_empty() || !expected.ends_with(selected_text) {
        return selected;
    }

    let missing = &expected[..expected.len() - selected_text.len()];
    let missing_units = missing.encode_utf16().count();
    if missing_units == 0 || missing_units > 8 {
        return selected;
    }

    let shift_down = [KeyboardInputEvent::new(
        VK_LSHIFT,
        false,
        KeyboardInputMode::ScanCode,
    )];
    if !send_keyboard_input(&shift_down) {
        return selected;
    }
    std::thread::sleep(Duration::from_millis(25));

    let mut events = Vec::with_capacity(missing_units * 2 + 1);
    for _ in 0..missing_units {
        events.push(KeyboardInputEvent::new(
            VK_LEFT,
            false,
            KeyboardInputMode::ScanCode,
        ));
        events.push(KeyboardInputEvent::new(
            VK_LEFT,
            true,
            KeyboardInputMode::ScanCode,
        ));
    }
    events.push(KeyboardInputEvent::new(
        VK_LSHIFT,
        true,
        KeyboardInputMode::ScanCode,
    ));
    if !send_keyboard_input(&events) {
        release_modifier_keys();
        return selected;
    }
    release_modifier_keys();
    std::thread::sleep(Duration::from_millis(35));

    copy_selected_text_checked(snapshot, timeout).or(selected)
}

#[cfg(windows)]
fn drain_pending_hotkey_messages() {
    let mut message = Msg::default();
    loop {
        let removed =
            unsafe { PeekMessageW(&mut message as *mut Msg, 0, WM_HOTKEY, WM_HOTKEY, PM_REMOVE) };
        if removed == 0 {
            break;
        }
    }

    loop {
        let removed = unsafe {
            PeekMessageW(
                &mut message as *mut Msg,
                0,
                WM_STEPLER_HOTKEY,
                WM_STEPLER_HOTKEY,
                PM_REMOVE,
            )
        };
        if removed == 0 {
            break;
        }
    }
}

#[cfg(windows)]
pub fn append_hotkey_signal_log(message: &str) {
    let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") else {
        return;
    };
    let path = std::path::PathBuf::from(local_app_data)
        .join("Stepler")
        .join("logs")
        .join("hotkey_signal.log");
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut file| {
            use std::io::Write;
            writeln!(file, "{:?} {}", std::time::SystemTime::now(), message)
        });
}

fn slice_by_range(text: &str, range: TextRange) -> Option<&str> {
    if range.start > range.end
        || range.end > text.len()
        || !text.is_char_boundary(range.start)
        || !text.is_char_boundary(range.end)
    {
        return None;
    }

    Some(&text[range.start..range.end])
}

fn edit_offset_to_byte_offset(text: &str, edit_offset: usize) -> Option<usize> {
    let mut byte_index = 0;
    let mut edit_units = 0;

    while byte_index < text.len() {
        if edit_units == edit_offset {
            return Some(byte_index);
        }

        if text[byte_index..].starts_with("\r\n") {
            byte_index += "\r\n".len();
            edit_units += 1;
            continue;
        }

        let ch = text[byte_index..].chars().next()?;
        byte_index += ch.len_utf8();
        edit_units += ch.len_utf16();
    }

    if edit_units == edit_offset {
        Some(text.len())
    } else {
        None
    }
}

fn byte_offset_to_edit_offset(text: &str, byte_offset: usize) -> Option<usize> {
    if byte_offset > text.len() || !text.is_char_boundary(byte_offset) {
        return None;
    }

    let mut byte_index = 0;
    let mut edit_units = 0;

    while byte_index < byte_offset {
        if text[byte_index..].starts_with("\r\n") {
            let next_byte = byte_index + "\r\n".len();
            if byte_offset < next_byte {
                return None;
            }
            byte_index = next_byte;
            edit_units += 1;
            continue;
        }

        let ch = text[byte_index..].chars().next()?;
        byte_index += ch.len_utf8();
        edit_units += ch.len_utf16();
    }

    Some(edit_units)
}

#[cfg(windows)]
fn window_text(hwnd: isize) -> Result<String, PlatformError> {
    let length = unsafe { SendMessageW(hwnd, WM_GETTEXTLENGTH, 0, 0) };
    if length < 0 {
        return Err(PlatformError::ForegroundUnavailable);
    }

    let mut buffer = vec![0u16; length as usize + 1];
    let copied =
        unsafe { SendMessageW(hwnd, WM_GETTEXT, buffer.len(), buffer.as_mut_ptr() as isize) };
    if copied < 0 {
        return Err(PlatformError::ForegroundUnavailable);
    }

    Ok(String::from_utf16_lossy(&buffer[..copied as usize]))
}

#[cfg(windows)]
fn edit_selection(hwnd: isize) -> Option<(usize, usize)> {
    let mut start = 0u32;
    let mut end = 0u32;
    unsafe {
        SendMessageW(
            hwnd,
            EM_GETSEL,
            (&mut start as *mut u32) as usize,
            (&mut end as *mut u32) as isize,
        );
    }

    Some((start as usize, end as usize))
}

#[cfg(windows)]
fn set_edit_selection(hwnd: isize, start: usize, end: usize) -> Result<(), PlatformError> {
    let text = window_text(hwnd)?;
    let start = byte_offset_to_edit_offset(&text, start).ok_or(PlatformError::PreflightFailed)?;
    let end = byte_offset_to_edit_offset(&text, end).ok_or(PlatformError::PreflightFailed)?;
    unsafe {
        SendMessageW(hwnd, EM_SETSEL, start, end as isize);
    }
    Ok(())
}

#[cfg(windows)]
fn replace_edit_selection(hwnd: isize, replacement: &str) -> Result<(), PlatformError> {
    let replacement: Vec<u16> = replacement
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    unsafe {
        SendMessageW(hwnd, EM_REPLACESEL, 1, replacement.as_ptr() as isize);
    }
    Ok(())
}

#[derive(Debug, Default)]
struct KeyboardControlHookState {
    left_ctrl_down: bool,
    right_ctrl_down: bool,
    left_ctrl_used: bool,
    right_ctrl_used: bool,
    pause_down: bool,
    pause_down_at: Option<Instant>,
    pending_scroll_lock: bool,
    win_down: bool,
    last_pause_at: Option<Instant>,
    last_scroll_lock_at: Option<Instant>,
    suppress_c_until: Option<Instant>,
    suspend_layout_controls_until: Option<Instant>,
    last_layout_control_action_at: Option<Instant>,
}

impl KeyboardControlHookState {
    fn handle_correction_hotkey(
        &mut self,
        vk_code: u32,
        is_down: bool,
        is_up: bool,
    ) -> Option<stepler_core::CorrectionMode> {
        match vk_code {
            VK_PAUSE | VK_CANCEL => {
                if is_down {
                    self.recover_stale_pause_down();
                }
                let physical_left_ctrl_down = key_is_down(VK_LCONTROL);
                let physical_right_ctrl_down = key_is_down(VK_RCONTROL);
                let ctrl_down = self.left_ctrl_down
                    || self.right_ctrl_down
                    || physical_left_ctrl_down
                    || physical_right_ctrl_down;
                if is_down
                    && !self.pause_down
                    && Self::debounce_allows(if ctrl_down {
                        self.last_scroll_lock_at
                    } else {
                        self.last_pause_at
                    })
                {
                    self.mark_pause_down();
                    let now = Instant::now();
                    if ctrl_down {
                        self.left_ctrl_used |= self.left_ctrl_down || physical_left_ctrl_down;
                        self.right_ctrl_used |= self.right_ctrl_down || physical_right_ctrl_down;
                        self.last_scroll_lock_at = Some(now);
                        self.suppress_c_until = Some(now + Duration::from_millis(1_500));
                        self.pending_scroll_lock = true;
                        return None;
                    }

                    self.last_pause_at = Some(now);
                    return Some(stepler_core::CorrectionMode::Pause);
                }
                if is_up {
                    self.mark_pause_up();
                    return self.take_pending_scroll_lock_if_released();
                }
            }
            _ => {}
        }

        None
    }

    fn handle_terminal_pause_key(
        &mut self,
        vk_code: u32,
        is_down: bool,
        is_up: bool,
    ) -> TerminalPauseHandling {
        if !matches!(vk_code, VK_PAUSE | VK_CANCEL) {
            return TerminalPauseHandling::PassThrough;
        }

        let physical_left_ctrl_down = key_is_down(VK_LCONTROL);
        let physical_right_ctrl_down = key_is_down(VK_RCONTROL);
        let ctrl_down = self.left_ctrl_down
            || self.right_ctrl_down
            || physical_left_ctrl_down
            || physical_right_ctrl_down;
        if is_down {
            self.recover_stale_pause_down();
        }
        if is_down && !self.pause_down {
            self.mark_pause_down();
            if ctrl_down {
                self.left_ctrl_used |= self.left_ctrl_down || physical_left_ctrl_down;
                self.right_ctrl_used |= self.right_ctrl_down || physical_right_ctrl_down;
                return TerminalPauseHandling::TranslateToF14;
            }
            return TerminalPauseHandling::TranslateToF13;
        }

        if is_up {
            self.mark_pause_up();
            return TerminalPauseHandling::Suppress;
        }

        if ctrl_down {
            TerminalPauseHandling::Suppress
        } else {
            TerminalPauseHandling::Suppress
        }
    }

    fn handle_classic_console_pause_key(
        &mut self,
        vk_code: u32,
        is_down: bool,
        is_up: bool,
    ) -> Option<stepler_core::CorrectionMode> {
        if !matches!(vk_code, VK_PAUSE | VK_CANCEL) {
            return None;
        }

        let physical_left_ctrl_down = key_is_down(VK_LCONTROL);
        let physical_right_ctrl_down = key_is_down(VK_RCONTROL);
        let ctrl_down = self.left_ctrl_down
            || self.right_ctrl_down
            || physical_left_ctrl_down
            || physical_right_ctrl_down;
        if is_down {
            self.recover_stale_pause_down();
        }
        if is_down
            && !self.pause_down
            && Self::debounce_allows(if ctrl_down {
                self.last_scroll_lock_at
            } else {
                self.last_pause_at
            })
        {
            self.mark_pause_down();
            let now = Instant::now();
            if ctrl_down {
                self.left_ctrl_used |= self.left_ctrl_down || physical_left_ctrl_down;
                self.right_ctrl_used |= self.right_ctrl_down || physical_right_ctrl_down;
                self.last_scroll_lock_at = Some(now);
                self.suppress_c_until = Some(now + Duration::from_millis(1_500));
                return Some(stepler_core::CorrectionMode::ScrollLock);
            }

            self.last_pause_at = Some(now);
            return Some(stepler_core::CorrectionMode::Pause);
        }

        if is_up {
            self.mark_pause_up();
        }

        None
    }

    fn should_suppress_companion_key(&mut self, vk_code: u32) -> bool {
        if !is_scrolllock_companion_key(vk_code) {
            return false;
        }

        let Some(until) = self.suppress_c_until else {
            return false;
        };
        if Instant::now() <= until {
            return true;
        }

        self.suppress_c_until = None;
        false
    }

    fn debounce_allows(last_at: Option<Instant>) -> bool {
        last_at
            .map(|last_at| last_at.elapsed() >= Duration::from_millis(250))
            .unwrap_or(true)
    }

    fn layout_control_debounce_allows(last_at: Option<Instant>) -> bool {
        last_at
            .map(|last_at| last_at.elapsed() >= Duration::from_millis(650))
            .unwrap_or(true)
    }

    fn mark_pause_down(&mut self) {
        self.pause_down = true;
        self.pause_down_at = Some(Instant::now());
    }

    fn mark_pause_up(&mut self) {
        self.pause_down = false;
        self.pause_down_at = None;
    }

    fn recover_stale_pause_down(&mut self) {
        if self.pause_down
            && self
                .pause_down_at
                .is_some_and(|pause_down_at| pause_down_at.elapsed() >= Duration::from_millis(750))
        {
            self.pause_down = false;
            self.pause_down_at = None;
            self.pending_scroll_lock = false;
        }
    }

    fn take_pending_scroll_lock_if_released(&mut self) -> Option<stepler_core::CorrectionMode> {
        if self.pending_scroll_lock
            && !self.pause_down
            && !self.left_ctrl_down
            && !self.right_ctrl_down
        {
            self.pending_scroll_lock = false;
            return Some(stepler_core::CorrectionMode::ScrollLock);
        }

        None
    }

    fn handle_key(
        &mut self,
        vk_code: u32,
        is_down: bool,
        is_up: bool,
    ) -> Option<KeyboardControlAction> {
        if matches!(vk_code, VK_LWIN | VK_RWIN) {
            if is_down {
                self.win_down = true;
                self.suspend_layout_controls_until =
                    Some(Instant::now() + Duration::from_millis(1_500));
            }
            if is_up {
                self.win_down = false;
                self.suspend_layout_controls_until =
                    Some(Instant::now() + Duration::from_millis(1_500));
            }
            return None;
        }

        if self.win_down && self.layout_controls_are_suspended() {
            return None;
        }
        if self.win_down {
            self.win_down = false;
        }
        if self.layout_controls_are_suspended() {
            return None;
        }

        if is_down {
            match vk_code {
                VK_LCONTROL => {
                    self.right_ctrl_used = true;
                    self.right_ctrl_down = false;
                    self.left_ctrl_down = true;
                    self.left_ctrl_used = false;
                }
                VK_RCONTROL => {
                    self.left_ctrl_used = true;
                    self.left_ctrl_down = false;
                    self.right_ctrl_down = true;
                    self.right_ctrl_used = false;
                }
                VK_APPS => return Some(KeyboardControlAction::SwitchToNext),
                _ => {
                    if self.left_ctrl_down {
                        self.left_ctrl_used = true;
                    }
                    if self.right_ctrl_down {
                        self.right_ctrl_used = true;
                    }
                }
            }
        }

        if is_up {
            match vk_code {
                VK_LCONTROL => {
                    let was_down = self.left_ctrl_down;
                    let was_used = self.left_ctrl_used;
                    self.left_ctrl_down = false;
                    self.left_ctrl_used = false;
                    if self.pending_scroll_lock {
                        return None;
                    }
                    if !was_down {
                        append_hotkey_signal_log(
                            "hook_layout_action_ignored action=SwitchToRussian reason=missing_down",
                        );
                        return None;
                    }
                    if !Self::layout_control_debounce_allows(self.last_layout_control_action_at) {
                        append_hotkey_signal_log(
                            "hook_layout_action_debounced action=SwitchToRussian",
                        );
                        return None;
                    }
                    if !was_used {
                        self.last_layout_control_action_at = Some(Instant::now());
                    }
                    append_hotkey_signal_log(&format!(
                        "hook_layout_action action=SwitchToRussian was_used={was_used} left_down={} right_down={}",
                        self.left_ctrl_down, self.right_ctrl_down
                    ));
                    let action = (!was_used).then_some(KeyboardControlAction::SwitchToRussian);
                    return action;
                }
                VK_RCONTROL => {
                    let was_down = self.right_ctrl_down;
                    let was_used = self.right_ctrl_used;
                    self.right_ctrl_down = false;
                    self.right_ctrl_used = false;
                    if self.pending_scroll_lock {
                        return None;
                    }
                    if !was_down {
                        append_hotkey_signal_log(
                            "hook_layout_action_ignored action=SwitchToEnglish reason=missing_down",
                        );
                        return None;
                    }
                    if !Self::layout_control_debounce_allows(self.last_layout_control_action_at) {
                        append_hotkey_signal_log(
                            "hook_layout_action_debounced action=SwitchToEnglish",
                        );
                        return None;
                    }
                    if !was_used {
                        self.last_layout_control_action_at = Some(Instant::now());
                    }
                    append_hotkey_signal_log(&format!(
                        "hook_layout_action action=SwitchToEnglish was_used={was_used} left_down={} right_down={}",
                        self.left_ctrl_down, self.right_ctrl_down
                    ));
                    let action = (!was_used).then_some(KeyboardControlAction::SwitchToEnglish);
                    return action;
                }
                _ => {}
            }
        }

        None
    }

    fn layout_controls_are_suspended(&mut self) -> bool {
        let Some(until) = self.suspend_layout_controls_until else {
            return false;
        };
        if Instant::now() <= until {
            return true;
        }

        self.suspend_layout_controls_until = None;
        false
    }
}

#[cfg(windows)]
fn should_suppress_keyboard_companion_event(vk_code: u32) -> bool {
    KEYBOARD_CONTROL_STATE
        .get_or_init(|| Mutex::new(KeyboardControlHookState::default()))
        .lock()
        .ok()
        .is_some_and(|mut state| state.should_suppress_companion_key(vk_code))
}

#[cfg(windows)]
fn is_scrolllock_companion_key(vk_code: u32) -> bool {
    matches!(vk_code, VK_C | VK_HOME | VK_LEFT | VK_RIGHT)
}

static KEYBOARD_CONTROL_STATE: OnceLock<Mutex<KeyboardControlHookState>> = OnceLock::new();
#[cfg(windows)]
static KEYBOARD_CONTROL_HOOK: OnceLock<Mutex<isize>> = OnceLock::new();
#[cfg(windows)]
static KEYBOARD_CONTROL_THREAD_ID: OnceLock<u32> = OnceLock::new();

#[cfg(windows)]
fn install_keyboard_control_hook() -> Result<(), PlatformError> {
    let thread_id = unsafe { GetCurrentThreadId() };
    let _ = KEYBOARD_CONTROL_THREAD_ID.set(thread_id);
    let hook = unsafe {
        SetWindowsHookExW(
            WH_KEYBOARD_LL,
            Some(low_level_keyboard_proc),
            GetModuleHandleW(std::ptr::null()),
            0,
        )
    };
    if hook == 0 {
        return Err(PlatformError::HotkeyUnavailable);
    }

    *KEYBOARD_CONTROL_HOOK
        .get_or_init(|| Mutex::new(0))
        .lock()
        .map_err(|_| PlatformError::HotkeyUnavailable)? = hook;
    Ok(())
}

#[cfg(windows)]
struct KeyboardControlHookGuard;

#[cfg(windows)]
impl Drop for KeyboardControlHookGuard {
    fn drop(&mut self) {
        if let Some(hook) = KEYBOARD_CONTROL_HOOK.get() {
            if let Ok(mut hook) = hook.lock() {
                if *hook != 0 {
                    unsafe {
                        UnhookWindowsHookEx(*hook);
                    }
                    *hook = 0;
                }
            }
        }
    }
}

#[cfg(windows)]
unsafe extern "system" fn low_level_keyboard_proc(
    code: i32,
    wparam: usize,
    lparam: isize,
) -> isize {
    if code < 0 {
        return CallNextHookEx(0, code, wparam, lparam);
    }

    let event = *(lparam as *const KbdLlHookStruct);
    let vk_code = normalized_control_vk(event);
    let is_down = matches!(wparam as u32, WM_KEYDOWN | WM_SYSKEYDOWN);
    let is_up = matches!(wparam as u32, WM_KEYUP | WM_SYSKEYUP);
    if !(is_down || is_up) {
        return CallNextHookEx(0, code, wparam, lparam);
    }
    if matches!(vk_code, VK_PAUSE | VK_CANCEL) {
        if foreground_is_stepler_qwen_surface() {
            let _ = KEYBOARD_CONTROL_STATE
                .get_or_init(|| Mutex::new(KeyboardControlHookState::default()))
                .lock()
                .map(|mut state| {
                    if is_down {
                        state.mark_pause_down();
                        state.left_ctrl_used |= state.left_ctrl_down;
                        state.right_ctrl_used |= state.right_ctrl_down;
                    }
                    if is_up {
                        state.mark_pause_up();
                    }
                });
            append_hotkey_signal_log(&format!(
                "hook_qwen_surface_passthrough vk={vk_code} down={is_down} up={is_up}"
            ));
            return CallNextHookEx(0, code, wparam, lparam);
        }
        if foreground_hwnd()
            .map(refresh_foreground_is_codex_embedded_terminal)
            .unwrap_or(false)
        {
            let mode = KEYBOARD_CONTROL_STATE
                .get_or_init(|| Mutex::new(KeyboardControlHookState::default()))
                .lock()
                .ok()
                .and_then(|mut state| {
                    let mode = state.handle_correction_hotkey(vk_code, is_down, is_up);
                    mode.or_else(|| state.take_pending_scroll_lock_if_released())
                });
            if let Some(mode) = mode {
                append_hotkey_signal_log(&format!(
                    "hook_codex_embedded_terminal_posted mode={mode:?} vk={vk_code} down={is_down} up={is_up}"
                ));
                post_correction_hotkey_from_hook(mode, vk_code, is_down, is_up);
            } else {
                append_hotkey_signal_log(&format!(
                    "hook_codex_embedded_terminal_suppressed vk={vk_code} down={is_down} up={is_up}"
                ));
            }
            return 1;
        }
        if foreground_is_classic_console() {
            let mode = KEYBOARD_CONTROL_STATE
                .get_or_init(|| Mutex::new(KeyboardControlHookState::default()))
                .lock()
                .ok()
                .and_then(|mut state| {
                    state.handle_classic_console_pause_key(vk_code, is_down, is_up)
                });
            if let Some(mode) = mode {
                post_correction_hotkey_from_hook(mode, vk_code, is_down, is_up);
            }
            return 1;
        }
        let terminal_passthrough = foreground_terminal_passthrough();
        if terminal_passthrough == TerminalPassthrough::None
            && terminal_needs_conservative_suppression()
        {
            let mode = KEYBOARD_CONTROL_STATE
                .get_or_init(|| Mutex::new(KeyboardControlHookState::default()))
                .lock()
                .ok()
                .and_then(|mut state| {
                    state.handle_classic_console_pause_key(vk_code, is_down, is_up)
                });
            if let Some(mode) = mode {
                post_hotkey_received_from_hook(mode, vk_code, is_down, is_up);
                post_hotkey_unsupported_from_hook(mode, vk_code, is_down, is_up);
            }
            append_hotkey_signal_log(&format!(
                "hook_terminal_conservative_suppressed vk={vk_code} down={is_down} up={is_up}"
            ));
            return 1;
        }
        if terminal_passthrough == TerminalPassthrough::SshRemote && is_down {
            let mode = KEYBOARD_CONTROL_STATE
                .get_or_init(|| Mutex::new(KeyboardControlHookState::default()))
                .lock()
                .ok()
                .map(|mut state| {
                    let ctrl_down = state.left_ctrl_down || state.right_ctrl_down;
                    state.mark_pause_down();
                    state.left_ctrl_used |= state.left_ctrl_down;
                    state.right_ctrl_used |= state.right_ctrl_down;
                    if ctrl_down {
                        stepler_core::CorrectionMode::ScrollLock
                    } else {
                        stepler_core::CorrectionMode::Pause
                    }
                })
                .unwrap_or(stepler_core::CorrectionMode::Pause);
            append_hotkey_signal_log(&format!(
                "hook_ssh_remote_forwarded mode={mode:?} vk={vk_code} down={is_down} up={is_up}"
            ));
            post_hotkey_received_from_hook(mode, vk_code, is_down, is_up);
            send_ssh_terminal_sequence(mode);
            return 1;
        }
        if terminal_passthrough == TerminalPassthrough::SshRemote && is_up {
            append_hotkey_signal_log(&format!(
                "hook_ssh_remote_suppressed_up vk={vk_code} down={is_down} up={is_up}"
            ));
            let _ = KEYBOARD_CONTROL_STATE
                .get_or_init(|| Mutex::new(KeyboardControlHookState::default()))
                .lock()
                .map(|mut state| state.mark_pause_up());
            return 1;
        }
        if matches!(
            terminal_passthrough,
            TerminalPassthrough::Ssh | TerminalPassthrough::UnknownTerminal
        ) {
            let mode = KEYBOARD_CONTROL_STATE
                .get_or_init(|| Mutex::new(KeyboardControlHookState::default()))
                .lock()
                .ok()
                .and_then(|mut state| {
                    state.handle_classic_console_pause_key(vk_code, is_down, is_up)
                });
            if let Some(mode) = mode {
                post_hotkey_received_from_hook(mode, vk_code, is_down, is_up);
                post_hotkey_unsupported_from_hook(mode, vk_code, is_down, is_up);
            }
            append_hotkey_signal_log(&format!(
                "hook_terminal_suppressed kind={terminal_passthrough:?} vk={vk_code} down={is_down} up={is_up}"
            ));
            return 1;
        }
        if terminal_passthrough == TerminalPassthrough::TerminalApp {
            let mode = KEYBOARD_CONTROL_STATE
                .get_or_init(|| Mutex::new(KeyboardControlHookState::default()))
                .lock()
                .ok()
                .and_then(|mut state| {
                    let mode = state.handle_correction_hotkey(vk_code, is_down, is_up);
                    mode.or_else(|| state.take_pending_scroll_lock_if_released())
                });
            if let Some(mode) = mode {
                append_hotkey_signal_log(&format!(
                    "hook_terminal_app_posted mode={mode:?} vk={vk_code} down={is_down} up={is_up}"
                ));
                post_correction_hotkey_from_hook(mode, vk_code, is_down, is_up);
            } else {
                append_hotkey_signal_log(&format!(
                    "hook_terminal_app_suppressed vk={vk_code} down={is_down} up={is_up}"
                ));
            }
            return 1;
        }
        if terminal_passthrough == TerminalPassthrough::PsReadLine {
            let handling = KEYBOARD_CONTROL_STATE
                .get_or_init(|| Mutex::new(KeyboardControlHookState::default()))
                .lock()
                .ok()
                .map(|mut state| state.handle_terminal_pause_key(vk_code, is_down, is_up))
                .unwrap_or(TerminalPauseHandling::PassThrough);
            match handling {
                TerminalPauseHandling::PassThrough => {
                    return CallNextHookEx(0, code, wparam, lparam);
                }
                TerminalPauseHandling::Suppress => return 1,
                TerminalPauseHandling::TranslateToF13 => {
                    post_hotkey_received_from_hook(
                        stepler_core::CorrectionMode::Pause,
                        vk_code,
                        is_down,
                        is_up,
                    );
                    release_modifier_keys();
                    send_key_virtual(VK_F13);
                    return 1;
                }
                TerminalPauseHandling::TranslateToF14 => {
                    post_hotkey_received_from_hook(
                        stepler_core::CorrectionMode::ScrollLock,
                        vk_code,
                        is_down,
                        is_up,
                    );
                    release_modifier_keys();
                    send_key_virtual(VK_F14);
                    return 1;
                }
            }
        }
    }
    if should_ignore_keyboard_hook_event(event) {
        return CallNextHookEx(0, code, wparam, lparam);
    }
    if is_windows_language_switch_key(vk_code) {
        let _ = KEYBOARD_CONTROL_STATE
            .get_or_init(|| Mutex::new(KeyboardControlHookState::default()))
            .lock()
            .map(|mut state| {
                let _ = state.handle_key(vk_code, is_down, is_up);
            });
        return CallNextHookEx(0, code, wparam, lparam);
    }
    if vk_code == VK_CAPITAL && caps_lock_disabled() {
        append_hotkey_signal_log(&format!(
            "hook_capslock_suppressed down={is_down} up={is_up}"
        ));
        return 1;
    }
    if vk_code == VK_INSERT && insert_as_backspace_enabled() && no_modifier_keys_down() {
        if is_down {
            append_hotkey_signal_log("hook_insert_as_backspace");
            send_key_virtual(VK_BACK);
        }
        return 1;
    }
    if should_suppress_keyboard_companion_event(vk_code) {
        return 1;
    }

    let (mode, action, suppress_key) = KEYBOARD_CONTROL_STATE
        .get_or_init(|| Mutex::new(KeyboardControlHookState::default()))
        .lock()
        .ok()
        .map(|mut state| {
            let mode = state.handle_correction_hotkey(vk_code, is_down, is_up);
            let suppress_key = state.should_suppress_companion_key(vk_code);
            let action = state.handle_key(vk_code, is_down, is_up);
            let mode = mode.or_else(|| state.take_pending_scroll_lock_if_released());
            (mode, action, suppress_key)
        })
        .unwrap_or((None, None, false));

    if std::env::var_os("STEPLER_DEBUG_KEYS").is_some() {
        eprintln!(
            "key: vk={} normalized={} scan={} flags=0x{:X} down={} up={} mode={:?} action={:?}",
            event.vk_code, vk_code, event.scan_code, event.flags, is_down, is_up, mode, action
        );
    }

    if suppress_key {
        return 1;
    }

    if let Some(mode) = mode {
        if !correction_hotkey_enabled(mode) {
            append_hotkey_signal_log(&format!("hook_mode_disabled {mode:?}"));
            return CallNextHookEx(0, code, wparam, lparam);
        }
        if let Some(thread_id) = KEYBOARD_CONTROL_THREAD_ID.get().copied() {
            let posted = PostThreadMessageW(
                thread_id,
                WM_STEPLER_HOTKEY,
                correction_mode_message_id(mode),
                0,
            );
            append_hotkey_signal_log(&format!(
                "hook_post mode={mode:?} vk={vk_code} down={is_down} up={is_up} posted={posted}"
            ));
        } else {
            append_hotkey_signal_log(&format!("hook_no_thread mode={mode:?} vk={vk_code}"));
        }

        if is_up && matches!(vk_code, VK_LCONTROL | VK_RCONTROL) {
            return CallNextHookEx(0, code, wparam, lparam);
        }

        return 1;
    }

    if matches!(vk_code, VK_PAUSE | VK_CANCEL) {
        return 1;
    }

    if let Some(action) = action {
        if !layout_action_enabled(action) {
            append_hotkey_signal_log(&format!("hook_layout_action_disabled {action:?}"));
            return CallNextHookEx(0, code, wparam, lparam);
        }
        if let Some(thread_id) = KEYBOARD_CONTROL_THREAD_ID.get().copied() {
            let posted = PostThreadMessageW(
                thread_id,
                WM_STEPLER_KEYBOARD_CONTROL,
                action.message_id(),
                0,
            );
            append_hotkey_signal_log(&format!(
                "hook_layout_action_post action={action:?} vk={vk_code} posted={posted}"
            ));
        } else {
            append_hotkey_signal_log(&format!(
                "hook_layout_action_no_thread action={action:?} vk={vk_code}"
            ));
        }

        if vk_code == VK_APPS {
            return 1;
        }
    }

    CallNextHookEx(0, code, wparam, lparam)
}

#[cfg(windows)]
fn normalized_control_vk(event: KbdLlHookStruct) -> u32 {
    match event.vk_code {
        VK_CONTROL if event.flags & LLKHF_EXTENDED != 0 => VK_RCONTROL,
        VK_CONTROL => VK_LCONTROL,
        other => other,
    }
}

#[cfg(windows)]
fn should_ignore_keyboard_hook_event(event: KbdLlHookStruct) -> bool {
    event.flags & LLKHF_INJECTED != 0
}

#[cfg(windows)]
fn is_windows_language_switch_key(vk_code: u32) -> bool {
    matches!(vk_code, VK_LWIN | VK_RWIN | VK_SPACE)
}

#[cfg(windows)]
fn correction_hotkey_enabled(mode: stepler_core::CorrectionMode) -> bool {
    match mode {
        stepler_core::CorrectionMode::Pause => env_flag_enabled("STEPLER_ENABLE_PAUSE", true),
        stepler_core::CorrectionMode::ScrollLock => {
            env_flag_enabled("STEPLER_ENABLE_SCROLLLOCK", true)
        }
    }
}

#[cfg(windows)]
fn post_correction_hotkey_from_hook(
    mode: stepler_core::CorrectionMode,
    vk_code: u32,
    is_down: bool,
    is_up: bool,
) {
    if !correction_hotkey_enabled(mode) {
        append_hotkey_signal_log(&format!("hook_mode_disabled {mode:?}"));
        return;
    }
    if let Some(thread_id) = KEYBOARD_CONTROL_THREAD_ID.get().copied() {
        let posted = unsafe {
            PostThreadMessageW(
                thread_id,
                WM_STEPLER_HOTKEY,
                correction_mode_message_id(mode),
                0,
            )
        };
        append_hotkey_signal_log(&format!(
            "hook_post mode={mode:?} vk={vk_code} down={is_down} up={is_up} posted={posted}"
        ));
    } else {
        append_hotkey_signal_log(&format!("hook_no_thread mode={mode:?} vk={vk_code}"));
    }
}

#[cfg(windows)]
fn post_hotkey_received_from_hook(
    mode: stepler_core::CorrectionMode,
    vk_code: u32,
    is_down: bool,
    is_up: bool,
) {
    post_hotkey_signal_from_hook(
        WM_STEPLER_HOTKEY_RECEIVED,
        "hook_post_received",
        mode,
        vk_code,
        is_down,
        is_up,
    );
}

#[cfg(windows)]
fn post_hotkey_unsupported_from_hook(
    mode: stepler_core::CorrectionMode,
    vk_code: u32,
    is_down: bool,
    is_up: bool,
) {
    post_hotkey_signal_from_hook(
        WM_STEPLER_HOTKEY_UNSUPPORTED,
        "hook_post_unsupported",
        mode,
        vk_code,
        is_down,
        is_up,
    );
}

#[cfg(windows)]
fn post_hotkey_signal_from_hook(
    message: u32,
    label: &str,
    mode: stepler_core::CorrectionMode,
    vk_code: u32,
    is_down: bool,
    is_up: bool,
) {
    if let Some(thread_id) = KEYBOARD_CONTROL_THREAD_ID.get().copied() {
        let posted =
            unsafe { PostThreadMessageW(thread_id, message, correction_mode_message_id(mode), 0) };
        append_hotkey_signal_log(&format!(
            "{label} mode={mode:?} vk={vk_code} down={is_down} up={is_up} posted={posted}"
        ));
    } else {
        append_hotkey_signal_log(&format!("{label}_no_thread mode={mode:?} vk={vk_code}"));
    }
}

#[cfg(windows)]
fn layout_action_enabled(action: KeyboardControlAction) -> bool {
    match action {
        KeyboardControlAction::SwitchToRussian | KeyboardControlAction::SwitchToEnglish => {
            env_flag_enabled("STEPLER_ENABLE_CTRL_LAYOUT", true)
        }
        KeyboardControlAction::SwitchToNext => {
            env_flag_enabled("STEPLER_ENABLE_MENU_CAPS_LAYOUT", true)
        }
    }
}

#[cfg(windows)]
fn caps_lock_disabled() -> bool {
    env_flag_enabled("STEPLER_DISABLE_CAPSLOCK", true)
}

#[cfg(windows)]
fn insert_as_backspace_enabled() -> bool {
    env_flag_enabled("STEPLER_INSERT_AS_BACKSPACE", true)
}

#[cfg(windows)]
fn no_modifier_keys_down() -> bool {
    ![
        VK_LCONTROL,
        VK_RCONTROL,
        VK_SHIFT,
        VK_LSHIFT,
        VK_RSHIFT,
        VK_MENU,
        VK_LMENU,
        VK_RMENU,
        VK_LWIN,
        VK_RWIN,
    ]
    .iter()
    .any(|vk| unsafe { GetAsyncKeyState(*vk as i32) & i16::MIN != 0 })
}

#[cfg(all(windows, not(test)))]
fn key_is_down(vk: u32) -> bool {
    unsafe { GetAsyncKeyState(vk as i32) & i16::MIN != 0 }
}

#[cfg(all(windows, test))]
fn key_is_down(_vk: u32) -> bool {
    false
}

#[cfg(windows)]
fn env_flag_enabled(name: &str, default: bool) -> bool {
    match std::env::var(name) {
        Ok(value) => !matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "0" | "false" | "off" | "no"
        ),
        Err(_) => default,
    }
}

#[cfg(windows)]
fn register_hotkey(id: i32, modifiers: u32, virtual_key: u32) -> Result<(), PlatformError> {
    let ok = unsafe { RegisterHotKey(0, id, modifiers | MOD_NOREPEAT, virtual_key) };
    if ok == 0 {
        return Err(PlatformError::HotkeyUnavailable);
    }

    Ok(())
}

#[cfg(windows)]
struct RegisteredHotkeyGuard;

#[cfg(windows)]
impl Drop for RegisteredHotkeyGuard {
    fn drop(&mut self) {
        unsafe {
            UnregisterHotKey(0, HOTKEY_ID_PAUSE);
            UnregisterHotKey(0, HOTKEY_ID_SCROLL_LOCK);
        }
    }
}

#[cfg(windows)]
const WM_GETTEXT: u32 = 0x000D;
#[cfg(windows)]
const WM_GETTEXTLENGTH: u32 = 0x000E;
#[cfg(windows)]
const EM_GETSEL: u32 = 0x00B0;
#[cfg(windows)]
const EM_SETSEL: u32 = 0x00B1;
#[cfg(windows)]
const EM_REPLACESEL: u32 = 0x00C2;
#[cfg(windows)]
const CF_UNICODETEXT: u32 = 13;
#[cfg(windows)]
const GMEM_MOVEABLE: u32 = 0x0002;
#[cfg(windows)]
const WM_HOTKEY: u32 = 0x0312;
#[cfg(windows)]
const PM_REMOVE: u32 = 0x0001;
#[cfg(windows)]
const MOD_NOREPEAT: u32 = 0x4000;
#[cfg(windows)]
const MOD_CONTROL: u32 = 0x0002;
#[cfg(windows)]
const INPUT_KEYBOARD: u32 = 1;
#[cfg(windows)]
const MAPVK_VK_TO_VSC_EX: u32 = 4;
#[cfg(windows)]
const VK_PAUSE: u32 = 0x13;
#[cfg(windows)]
const VK_CANCEL: u32 = 0x03;
#[cfg(windows)]
const VK_A: u32 = 0x41;
#[cfg(windows)]
const VK_F11: u32 = 0x7A;
#[cfg(windows)]
const VK_F12: u32 = 0x7B;
#[cfg(windows)]
const VK_F13: u32 = 0x7C;
#[cfg(windows)]
const VK_F14: u32 = 0x7D;
#[cfg(windows)]
const VK_HOME: u32 = 0x24;
#[cfg(windows)]
const VK_END: u32 = 0x23;
const VK_C: u32 = 0x43;
#[cfg(windows)]
const VK_V: u32 = 0x56;
#[cfg(windows)]
const VK_BACK: u32 = 0x08;
#[cfg(windows)]
const VK_ESCAPE: u32 = 0x1B;
#[cfg(windows)]
const VK_INSERT: u32 = 0x2D;
#[cfg(windows)]
const VK_DELETE: u32 = 0x2E;
#[cfg(windows)]
const VK_LEFT: u32 = 0x25;
#[cfg(windows)]
const VK_UP: u32 = 0x26;
#[cfg(windows)]
const VK_RIGHT: u32 = 0x27;
#[cfg(windows)]
const VK_DOWN: u32 = 0x28;
#[cfg(windows)]
const VK_PRIOR: u32 = 0x21;
#[cfg(windows)]
const VK_NEXT: u32 = 0x22;
#[cfg(windows)]
const VK_DIVIDE: u32 = 0x6F;
#[cfg(windows)]
const VK_NUMLOCK: u32 = 0x90;
#[cfg(windows)]
const VK_SPACE: u32 = 0x20;
#[cfg(windows)]
const VK_LWIN: u32 = 0x5B;
#[cfg(windows)]
const VK_RWIN: u32 = 0x5C;
#[cfg(windows)]
const HOTKEY_ID_PAUSE: i32 = 1;
#[cfg(windows)]
const HOTKEY_ID_SCROLL_LOCK: i32 = 2;
const LANG_ENGLISH: u16 = 0x0409;
const LANG_RUSSIAN: u16 = 0x0419;
const VK_LCONTROL: u32 = 0xA2;
const VK_RCONTROL: u32 = 0xA3;
#[cfg(windows)]
const VK_CONTROL: u32 = 0x11;
#[cfg(windows)]
const VK_LMENU: u32 = 0xA4;
#[cfg(windows)]
const VK_RMENU: u32 = 0xA5;
#[cfg(windows)]
const VK_MENU: u32 = 0x12;
#[cfg(windows)]
const VK_LSHIFT: u32 = 0xA0;
#[cfg(windows)]
const VK_RSHIFT: u32 = 0xA1;
#[cfg(windows)]
const VK_SHIFT: u32 = 0x10;
const VK_APPS: u32 = 0x5D;
const VK_CAPITAL: u32 = 0x14;
#[cfg(windows)]
const WM_INPUTLANGCHANGEREQUEST: u32 = 0x0050;
#[cfg(windows)]
const SMTO_ABORTIFHUNG: u32 = 0x0002;
#[cfg(windows)]
const KLF_SETFORPROCESS: u32 = 0x00000100;
#[cfg(windows)]
const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;
#[cfg(windows)]
const STEPLER_INJECTED_CONTROL_MAGIC: usize = 0x5354_4550_4C45_5201;
#[cfg(windows)]
const WM_STEPLER_KEYBOARD_CONTROL: u32 = 0x8001;
#[cfg(windows)]
const WM_STEPLER_HOTKEY: u32 = 0x8002;
#[cfg(windows)]
const WM_STEPLER_HOTKEY_RECEIVED: u32 = 0x8003;
#[cfg(windows)]
const WM_STEPLER_HOTKEY_UNSUPPORTED: u32 = 0x8004;
#[cfg(windows)]
const WM_QUIT: u32 = 0x0012;
#[cfg(windows)]
const WH_KEYBOARD_LL: i32 = 13;
#[cfg(windows)]
const WM_KEYDOWN: u32 = 0x0100;
#[cfg(windows)]
const WM_KEYUP: u32 = 0x0101;
#[cfg(windows)]
const WM_SYSKEYDOWN: u32 = 0x0104;
#[cfg(windows)]
const WM_SYSKEYUP: u32 = 0x0105;
#[cfg(windows)]
const LLKHF_EXTENDED: u32 = 0x01;
#[cfg(windows)]
const LLKHF_INJECTED: u32 = 0x10;
#[cfg(windows)]
const KEYEVENTF_EXTENDEDKEY: u32 = 0x0001;
#[cfg(windows)]
const KEYEVENTF_KEYUP: u32 = 0x0002;
#[cfg(windows)]
const KEYEVENTF_SCANCODE: u32 = 0x0008;
#[cfg(windows)]
const KEYEVENTF_UNICODE: u32 = 0x0004;
#[cfg(windows)]
const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;
#[cfg(windows)]
const INVALID_HANDLE_VALUE: isize = -1isize;

#[cfg(windows)]
#[repr(C)]
#[derive(Default)]
struct Point {
    x: i32,
    y: i32,
}

#[cfg(windows)]
#[repr(C)]
#[derive(Default)]
struct Msg {
    hwnd: isize,
    message: u32,
    wparam: usize,
    lparam: isize,
    time: u32,
    pt: Point,
}

#[cfg(windows)]
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct KbdLlHookStruct {
    vk_code: u32,
    scan_code: u32,
    flags: u32,
    time: u32,
    extra_info: usize,
}

#[cfg(windows)]
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Coord {
    x: i16,
    y: i16,
}

#[cfg(windows)]
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct SmallRect {
    left: i16,
    top: i16,
    right: i16,
    bottom: i16,
}

#[cfg(windows)]
#[repr(C)]
#[derive(Clone, Copy, Default)]
struct ConsoleScreenBufferInfo {
    size: Coord,
    cursor_position: Coord,
    attributes: u16,
    window: SmallRect,
    maximum_window_size: Coord,
}

#[cfg(windows)]
#[repr(C)]
#[derive(Default)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[cfg(windows)]
#[repr(C)]
#[derive(Default)]
struct GuiThreadInfo {
    cb_size: u32,
    flags: u32,
    hwnd_active: isize,
    hwnd_focus: isize,
    hwnd_capture: isize,
    hwnd_menu_owner: isize,
    hwnd_move_size: isize,
    hwnd_caret: isize,
    rc_caret: Rect,
}

#[cfg(windows)]
#[link(name = "user32")]
unsafe extern "system" {
    fn ActivateKeyboardLayout(layout: isize, flags: u32) -> isize;
    fn GetForegroundWindow() -> isize;
    fn GetClassNameW(hwnd: isize, class_name: *mut u16, max_count: i32) -> i32;
    fn GetWindowTextLengthW(hwnd: isize) -> i32;
    fn GetWindowTextW(hwnd: isize, text: *mut u16, max_count: i32) -> i32;
    fn GetWindowThreadProcessId(hwnd: isize, process_id: *mut u32) -> u32;
    fn GetGUIThreadInfo(thread_id: u32, gui_thread_info: *mut GuiThreadInfo) -> i32;
    fn SendMessageW(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> isize;
    fn SendMessageTimeoutW(
        hwnd: isize,
        msg: u32,
        wparam: usize,
        lparam: isize,
        flags: u32,
        timeout: u32,
        result: *mut isize,
    ) -> isize;
    fn OpenClipboard(hwnd_new_owner: isize) -> i32;
    fn CloseClipboard() -> i32;
    fn EmptyClipboard() -> i32;
    fn EnumClipboardFormats(format: u32) -> u32;
    fn GetClipboardData(format: u32) -> isize;
    fn IsClipboardFormatAvailable(format: u32) -> i32;
    fn SetClipboardData(format: u32, mem: isize) -> isize;
    fn GetClipboardSequenceNumber() -> u32;
    fn RegisterHotKey(hwnd: isize, id: i32, modifiers: u32, virtual_key: u32) -> i32;
    fn UnregisterHotKey(hwnd: isize, id: i32) -> i32;
    fn GetMessageW(message: *mut Msg, hwnd: isize, min_filter: u32, max_filter: u32) -> i32;
    fn PeekMessageW(
        message: *mut Msg,
        hwnd: isize,
        min_filter: u32,
        max_filter: u32,
        remove_msg: u32,
    ) -> i32;
    fn PostMessageW(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> i32;
    fn PostThreadMessageW(thread_id: u32, msg: u32, wparam: usize, lparam: isize) -> i32;
    fn GetKeyboardLayout(thread_id: u32) -> isize;
    fn GetKeyboardLayoutList(count: i32, layouts: *mut isize) -> i32;
    fn MapVirtualKeyW(code: u32, map_type: u32) -> u32;
    fn SetWindowsHookExW(
        id_hook: i32,
        hook_proc: Option<unsafe extern "system" fn(i32, usize, isize) -> isize>,
        instance: isize,
        thread_id: u32,
    ) -> isize;
    fn SendInput(count: u32, inputs: *mut Input, size: i32) -> u32;
    fn CallNextHookEx(hook: isize, code: i32, wparam: usize, lparam: isize) -> isize;
    fn UnhookWindowsHookEx(hook: isize) -> i32;
    fn keybd_event(vk: u8, scan: u8, flags: u32, extra_info: usize);
    fn GetAsyncKeyState(virtual_key: i32) -> i16;
}

#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn AttachConsole(process_id: u32) -> i32;
    fn CloseHandle(object: isize) -> i32;
    fn FreeConsole() -> i32;
    fn GetCurrentThreadId() -> u32;
    fn GetModuleHandleW(module_name: *const u16) -> isize;
    fn SetConsoleCtrlHandler(
        handler: Option<unsafe extern "system" fn(u32) -> i32>,
        add: i32,
    ) -> i32;
    fn GlobalAlloc(flags: u32, bytes: usize) -> isize;
    fn GlobalLock(mem: isize) -> *mut std::ffi::c_void;
    fn GlobalUnlock(mem: isize) -> i32;
    fn GlobalFree(mem: isize) -> isize;
    fn GlobalSize(mem: isize) -> usize;
    fn GetConsoleScreenBufferInfo(
        console_output: isize,
        console_screen_buffer_info: *mut ConsoleScreenBufferInfo,
    ) -> i32;
    fn GetStdHandle(std_handle: u32) -> isize;
    fn OpenProcess(desired_access: u32, inherit_handle: i32, process_id: u32) -> isize;
    fn QueryFullProcessImageNameW(
        process: isize,
        flags: u32,
        exe_name: *mut u16,
        size: *mut u32,
    ) -> i32;
    fn ReadConsoleOutputCharacterW(
        console_output: isize,
        character: *mut u16,
        length: u32,
        read_coord: Coord,
        number_of_chars_read: *mut u32,
    ) -> i32;
}
