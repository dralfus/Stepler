#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use stepler_core::{
    Capabilities, MethodBinding, MethodId, ReplacementPlan, TextContext, TextRange,
};
use stepler_platform::{
    ApplyReplacementResult, ClipboardBackend, ClipboardFormatSnapshot, ClipboardSnapshot,
    ForegroundControl, ForegroundProvider, ForegroundTarget, HotkeyListener, MethodProbe,
    MethodResolver, PlatformError, TextContextProvider, TextReplacer,
};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsFocusDiagnostics {
    pub foreground_hwnd: String,
    pub foreground_class: String,
    pub foreground_title: String,
    pub focused_hwnd: String,
    pub focused_class: String,
    pub focused_title: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsMethodDiagnostics {
    pub foreground: WindowsFocusDiagnostics,
    pub uia_focus: Option<WindowsUiaFocusDiagnostics>,
    pub probes: Vec<WindowsMethodProbeDiagnostics>,
    pub selected_context_method: Option<String>,
    pub selected_replacement_method: Option<String>,
    pub context_method: Option<String>,
    pub context_error: Option<String>,
    pub context_skipped: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsUiaFocusDiagnostics {
    pub name: String,
    pub control_type: String,
    pub automation_id: String,
    pub class_name: String,
    pub framework_id: String,
    pub has_keyboard_focus: bool,
    pub is_keyboard_focusable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsMethodProbeDiagnostics {
    pub method: String,
    pub safety: String,
    pub requires_clipboard: bool,
    pub requires_focus_stability: bool,
    pub can_preflight: bool,
    pub can_verify: bool,
    pub reason: String,
}

pub fn focus_diagnostics() -> Result<WindowsFocusDiagnostics, PlatformError> {
    focus_diagnostics_impl()
}

pub fn method_diagnostics() -> Result<WindowsMethodDiagnostics, PlatformError> {
    method_diagnostics_impl()
}

#[cfg(windows)]
pub fn try_forward_embedded_terminal_hotkey(
    mode: stepler_core::CorrectionMode,
) -> Result<bool, PlatformError> {
    let focus = uia_focus_diagnostics()?;
    if !is_embedded_terminal_uia_focus(&focus) {
        return Ok(false);
    }

    release_modifier_keys();
    std::thread::sleep(Duration::from_millis(20));
    match mode {
        stepler_core::CorrectionMode::Pause => send_key_chord_virtual(&[VK_CONTROL], VK_F11),
        stepler_core::CorrectionMode::ScrollLock => send_key_chord_virtual(&[VK_CONTROL], VK_F12),
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
pub fn message_loop_with_keyboard_controls<F, G>(
    mut on_hotkey: F,
    mut on_control: G,
) -> Result<(), PlatformError>
where
    F: FnMut(stepler_core::CorrectionMode),
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
                    on_hotkey(stepler_core::CorrectionMode::Pause);
                    drain_pending_hotkey_messages();
                }
                HOTKEY_ID_SCROLL_LOCK => {
                    append_hotkey_signal_log("wm_hotkey ctrl_pause");
                    std::thread::sleep(Duration::from_millis(180));
                    release_modifier_keys();
                    on_hotkey(stepler_core::CorrectionMode::ScrollLock);
                    drain_pending_hotkey_messages();
                }
                _ => {}
            },
            WM_STEPLER_HOTKEY => {
                if let Some(mode) = correction_mode_from_message_id(message.wparam) {
                    append_hotkey_signal_log(&format!("hook_message {mode:?}"));
                    if mode == stepler_core::CorrectionMode::ScrollLock {
                        std::thread::sleep(Duration::from_millis(180));
                        release_modifier_keys();
                    }
                    on_hotkey(mode);
                    drain_pending_hotkey_messages();
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
pub fn message_loop_with_keyboard_controls<F, G>(
    _on_hotkey: F,
    _on_control: G,
) -> Result<(), PlatformError>
where
    F: FnMut(stepler_core::CorrectionMode),
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
pub struct WindowsLayoutSwitcher {
    layouts: Vec<isize>,
    ordered_layouts: Vec<isize>,
    russian_layout: Option<isize>,
    english_layout: Option<isize>,
}

impl WindowsLayoutSwitcher {
    pub fn new() -> Self {
        let mut switcher = Self::default();
        switcher.reload_layouts();
        switcher
    }

    pub fn reload_layouts(&mut self) {
        self.layouts = keyboard_layouts();
        self.russian_layout = find_layout_by_language(&self.layouts, LANG_RUSSIAN);
        self.english_layout = find_layout_by_language(&self.layouts, LANG_ENGLISH);
        self.ordered_layouts.clear();
        if let Some(layout) = self.russian_layout {
            self.ordered_layouts.push(layout);
        }
        if let Some(layout) = self.english_layout {
            self.ordered_layouts.push(layout);
        }
        if self.ordered_layouts.is_empty() {
            self.ordered_layouts.extend(self.layouts.iter().copied());
        }
    }

    pub fn switch_to_russian(&self) -> Result<(), PlatformError> {
        let Some(layout) = self.russian_layout else {
            return Err(PlatformError::Unsupported);
        };
        switch_foreground_layout(layout)
    }

    pub fn switch_to_english(&self) -> Result<(), PlatformError> {
        let Some(layout) = self.english_layout else {
            return Err(PlatformError::Unsupported);
        };
        switch_foreground_layout(layout)
    }

    pub fn switch_window_to_russian(&self, hwnd: isize) -> Result<(), PlatformError> {
        let Some(layout) = self.russian_layout else {
            return Err(PlatformError::Unsupported);
        };
        switch_window_layout(hwnd, layout)
    }

    pub fn switch_window_to_english(&self, hwnd: isize) -> Result<(), PlatformError> {
        let Some(layout) = self.english_layout else {
            return Err(PlatformError::Unsupported);
        };
        switch_window_layout(hwnd, layout)
    }

    pub fn switch_to_next(&self) -> Result<(), PlatformError> {
        if self.ordered_layouts.len() < 2 {
            return Err(PlatformError::Unsupported);
        }

        let foreground = foreground_hwnd()?;
        let thread_id = window_thread_id(foreground)?;
        let current = unsafe { GetKeyboardLayout(thread_id) };
        let current_index = self
            .ordered_layouts
            .iter()
            .position(|layout| *layout == current)
            .unwrap_or(0);
        let next = self.ordered_layouts[(current_index + 1) % self.ordered_layouts.len()];
        switch_foreground_layout(next)
    }

    pub fn handle_action(&self, action: KeyboardControlAction) -> Result<(), PlatformError> {
        match action {
            KeyboardControlAction::SwitchToRussian => self.switch_to_russian(),
            KeyboardControlAction::SwitchToEnglish => self.switch_to_english(),
            KeyboardControlAction::SwitchToNext => self.switch_to_next(),
        }
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
struct Win32EditMessagesMethod;

#[cfg(windows)]
impl Win32EditMessagesMethod {
    fn probe(&self, target: &ForegroundTarget) -> Option<MethodProbe> {
        is_supported_edit_class(&target.focused_class)
            .then(|| MethodProbe::safe(MethodId::Win32EditMessages, "focused Win32 edit control"))
    }

    fn capture(
        &self,
        foreground: isize,
        focused: isize,
        app_class: String,
    ) -> Result<TextContext, PlatformError> {
        let text = window_text(focused)?;
        let (selection_start, selection_end) =
            edit_selection(focused).unwrap_or((text.len(), text.len()));
        let selection_start =
            edit_offset_to_byte_offset(&text, selection_start).unwrap_or(text.len());
        let selection_end = edit_offset_to_byte_offset(&text, selection_end).unwrap_or(text.len());
        let selection_range = if selection_start != selection_end {
            Some(TextRange::new(selection_start, selection_end))
        } else {
            None
        };

        Ok(TextContext {
            app_id: app_class,
            window_id: hwnd_id(foreground),
            control_id: hwnd_id(focused),
            text_snapshot: text,
            caret_range: TextRange::caret(selection_end),
            selection_range,
            capabilities: Capabilities {
                can_replace_directly: true,
                can_read_selection: true,
                can_read_caret: true,
                method_binding: Some(MethodBinding::new(
                    MethodId::Win32EditMessages,
                    vec![MethodId::Win32EditMessages],
                )),
            },
        })
    }

    fn apply(
        &self,
        context: &TextContext,
        plan: &ReplacementPlan,
    ) -> Result<ApplyReplacementResult, PlatformError> {
        let hwnd =
            parse_hwnd_id(&context.control_id).ok_or(PlatformError::ReplacementUnavailable)?;
        let focused_class = window_class_name(hwnd).unwrap_or_else(|| String::from("unknown"));
        if !is_supported_edit_class(&focused_class) {
            return Err(PlatformError::ReplacementUnavailable);
        }

        let current_text = window_text(hwnd)?;
        let actual_before = slice_by_range(&current_text, plan.range)
            .ok_or(PlatformError::PreflightFailed)?
            .to_owned();

        if actual_before != plan.expected_before_text {
            return Err(PlatformError::PreflightFailed);
        }

        set_edit_selection(hwnd, plan.range.start, plan.range.end)?;
        replace_edit_selection(hwnd, &plan.replacement_text)?;

        let actual_after = window_text(hwnd).ok();
        Ok(ApplyReplacementResult {
            applied: true,
            actual_before_text: Some(actual_before),
            actual_after_text: actual_after,
            method: MethodId::Win32EditMessages.as_str().to_owned(),
        })
    }
}

#[cfg(windows)]
#[derive(Debug, Default, Clone, Copy)]
struct ConsoleBufferMethod;

#[cfg(windows)]
impl ConsoleBufferMethod {
    fn probe(&self, target: &ForegroundTarget) -> Option<MethodProbe> {
        (target.app_class == "ConsoleWindowClass")
            .then(|| MethodProbe::safe(MethodId::ConsoleBuffer, "classic console buffer"))
    }

    fn capture(
        &self,
        foreground: isize,
        focused: isize,
        app_class: &str,
        focused_class: &str,
    ) -> Result<TextContext, PlatformError> {
        let input = read_console_input_text(foreground)?;
        let text_len = input.len();
        Ok(TextContext {
            app_id: format!("{app_class}/{focused_class}"),
            window_id: hwnd_id(foreground),
            control_id: format!("terminal-console:{}", hwnd_id(focused)),
            text_snapshot: input,
            caret_range: TextRange::caret(text_len),
            selection_range: None,
            capabilities: Capabilities {
                can_replace_directly: false,
                can_read_selection: false,
                can_read_caret: false,
                method_binding: Some(MethodBinding::new(
                    MethodId::ConsoleBuffer,
                    vec![MethodId::ConsoleBuffer],
                )),
            },
        })
    }

    fn apply(
        &self,
        context: &TextContext,
        plan: &ReplacementPlan,
    ) -> Result<ApplyReplacementResult, PlatformError> {
        let foreground = foreground_hwnd()?;
        let current_text = read_console_input_text(foreground)?;
        let actual_before = slice_by_range(&current_text, plan.range);
        if current_text != context.text_snapshot
            || actual_before != Some(plan.expected_before_text.as_str())
        {
            return Err(PlatformError::PreflightFailed);
        }

        let replacement = replace_range_text(&current_text, plan.range, &plan.replacement_text)
            .ok_or(PlatformError::PreflightFailed)?;
        clear_console_input_line(foreground)?;
        send_unicode_text(&replacement)?;
        std::thread::sleep(Duration::from_millis(60));

        Ok(ApplyReplacementResult {
            applied: true,
            actual_before_text: Some(current_text),
            actual_after_text: Some(replacement),
            method: MethodId::ConsoleBuffer.as_str().to_owned(),
        })
    }
}

#[cfg(windows)]
fn clear_console_input_line(hwnd: isize) -> Result<(), PlatformError> {
    for _ in 0..3 {
        send_key_virtual(VK_ESCAPE);
        std::thread::sleep(Duration::from_millis(45));
        match read_console_input_text(hwnd) {
            Err(PlatformError::ReplacementUnavailable) => return Ok(()),
            Ok(input) if input.trim().is_empty() => return Ok(()),
            Ok(_) => {}
            Err(error) => return Err(error),
        }
    }

    Err(PlatformError::PreflightFailed)
}

#[cfg(windows)]
#[derive(Debug, Default, Clone, Copy)]
struct TerminalClipboardShortcutMethod;

#[cfg(windows)]
impl TerminalClipboardShortcutMethod {
    fn probe(&self, target: &ForegroundTarget) -> Option<MethodProbe> {
        is_supported_terminal_class(&target.app_class, &target.focused_class).then(|| {
            MethodProbe::risky(
                MethodId::TerminalClipboardShortcut,
                "terminal clipboard shortcut fallback",
            )
        })
    }

    fn capture(
        &self,
        foreground: isize,
        focused: isize,
        app_class: &str,
        focused_class: &str,
    ) -> Result<TextContext, PlatformError> {
        let captured = read_terminal_left_text()?;
        let left_text = captured.text;
        let text_len = left_text.len();
        Ok(TextContext {
            app_id: format!("{app_class}/{focused_class}"),
            window_id: hwnd_id(foreground),
            control_id: format!(
                "terminal:{}:{}",
                hwnd_id(focused),
                captured.selection_kind.id()
            ),
            text_snapshot: left_text,
            caret_range: TextRange::caret(text_len),
            selection_range: None,
            capabilities: Capabilities {
                can_replace_directly: false,
                can_read_selection: false,
                can_read_caret: false,
                method_binding: Some(MethodBinding::new(
                    MethodId::TerminalClipboardShortcut,
                    vec![MethodId::TerminalClipboardShortcut],
                )),
            },
        })
    }

    fn apply(
        &self,
        context: &TextContext,
        plan: &ReplacementPlan,
    ) -> Result<ApplyReplacementResult, PlatformError> {
        let actual_before = slice_by_range(&context.text_snapshot, plan.range);
        if actual_before != Some(plan.expected_before_text.as_str()) {
            return Err(PlatformError::PreflightFailed);
        }

        let replacement =
            replace_range_text(&context.text_snapshot, plan.range, &plan.replacement_text)
                .ok_or(PlatformError::PreflightFailed)?;
        match TerminalSelectionKind::from_control_id(&context.control_id) {
            TerminalSelectionKind::LeftOfCaret => send_key_chord(&[VK_LSHIFT], VK_HOME),
            TerminalSelectionKind::PreviousWord => {
                send_key_chord(&[VK_CONTROL, VK_LSHIFT], VK_LEFT);
            }
        }
        std::thread::sleep(Duration::from_millis(40));
        restore_clipboard(clipboard_snapshot_from_text(&replacement))?;
        send_terminal_shortcut_with_english_layout(&[VK_CONTROL, VK_SHIFT], VK_V);
        std::thread::sleep(Duration::from_millis(60));

        Ok(ApplyReplacementResult {
            applied: true,
            actual_before_text: Some(context.text_snapshot.clone()),
            actual_after_text: Some(replacement),
            method: MethodId::TerminalClipboardShortcut.as_str().to_owned(),
        })
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

#[cfg(windows)]
#[derive(Debug, Default, Clone, Copy)]
struct ClipboardSelectionMethod;

#[cfg(windows)]
impl ClipboardSelectionMethod {
    fn probe(&self, target: &ForegroundTarget) -> Option<MethodProbe> {
        if is_supported_edit_class(&target.focused_class)
            || is_supported_terminal_class(&target.app_class, &target.focused_class)
            || is_word_target(target)
            || is_browser_like_target(target)
            || target.app_class.eq_ignore_ascii_case("Progman")
            || target.app_class.eq_ignore_ascii_case("WorkerW")
            || target.focused_class.eq_ignore_ascii_case("SysListView32")
        {
            return None;
        }

        Some(MethodProbe::risky(
            MethodId::ClipboardSelection,
            "generic selected text clipboard fallback",
        ))
    }

    fn capture(
        &self,
        foreground: isize,
        focused: isize,
        app_class: &str,
        focused_class: &str,
    ) -> Result<TextContext, PlatformError> {
        let snapshot = capture_clipboard()?;
        let sequence_before = snapshot.sequence_number;
        send_key_chord(&[VK_CONTROL], VK_C);
        let copied = wait_for_clipboard_selection_text(
            snapshot.text.as_deref(),
            sequence_before,
            Duration::from_millis(700),
        )
        .filter(|text| !text.trim().is_empty())
        .filter(|text| !looks_like_hotkeyhandler_marker(text))
        .ok_or(PlatformError::ReplacementUnavailable);
        let _ = restore_clipboard(snapshot);
        let text = copied?;
        let text_len = text.len();

        Ok(TextContext {
            app_id: format!("{app_class}/{focused_class}"),
            window_id: hwnd_id(foreground),
            control_id: format!("clipboard-selection:{}", hwnd_id(focused)),
            text_snapshot: text,
            caret_range: TextRange::caret(text_len),
            selection_range: Some(TextRange::new(0, text_len)),
            capabilities: Capabilities {
                can_replace_directly: false,
                can_read_selection: true,
                can_read_caret: false,
                method_binding: Some(MethodBinding::new(
                    MethodId::ClipboardSelection,
                    vec![MethodId::ClipboardSelection, MethodId::SendInput],
                )),
            },
        })
    }

    fn apply(
        &self,
        context: &TextContext,
        plan: &ReplacementPlan,
    ) -> Result<ApplyReplacementResult, PlatformError> {
        if plan.range != TextRange::new(0, context.text_snapshot.len())
            || plan.expected_before_text != context.text_snapshot
        {
            return Err(PlatformError::PreflightFailed);
        }

        restore_clipboard(clipboard_snapshot_from_text(&plan.replacement_text))?;
        send_key_chord(&[VK_CONTROL], VK_V);
        std::thread::sleep(Duration::from_millis(60));

        Ok(ApplyReplacementResult {
            applied: true,
            actual_before_text: Some(context.text_snapshot.clone()),
            actual_after_text: Some(plan.replacement_text.clone()),
            method: MethodId::ClipboardSelection.as_str().to_owned(),
        })
    }
}

fn looks_like_hotkeyhandler_marker(text: &str) -> bool {
    let trimmed = text.trim();
    trimmed.starts_with("__HKH_") || (trimmed.starts_with("__") && trimmed.contains("_MARKER_"))
}

#[cfg(windows)]
#[derive(Debug, Default, Clone, Copy)]
struct SendInputMethod;

#[cfg(windows)]
impl SendInputMethod {
    fn probe(&self, target: &ForegroundTarget) -> Option<MethodProbe> {
        if is_supported_terminal_class(&target.app_class, &target.focused_class)
            || is_word_target(target)
            || is_browser_like_target(target)
            || target.app_class.eq_ignore_ascii_case("Progman")
            || target.app_class.eq_ignore_ascii_case("WorkerW")
            || target.focused_class.eq_ignore_ascii_case("SysListView32")
        {
            return None;
        }

        let mut probe = MethodProbe::risky(MethodId::SendInput, "generic SendInput text fallback");
        probe.requires_clipboard = false;
        probe.can_preflight = false;
        probe.can_verify = false;
        Some(probe)
    }

    fn apply(
        &self,
        context: &TextContext,
        plan: &ReplacementPlan,
    ) -> Result<ApplyReplacementResult, PlatformError> {
        if plan.range != TextRange::new(0, context.text_snapshot.len())
            || plan.expected_before_text != context.text_snapshot
        {
            return Err(PlatformError::PreflightFailed);
        }

        send_unicode_text(&plan.replacement_text)?;
        std::thread::sleep(Duration::from_millis(40));

        Ok(ApplyReplacementResult {
            applied: true,
            actual_before_text: Some(context.text_snapshot.clone()),
            actual_after_text: Some(plan.replacement_text.clone()),
            method: MethodId::SendInput.as_str().to_owned(),
        })
    }
}

#[cfg(windows)]
#[derive(Debug, Default, Clone, Copy)]
struct WordComMethod;

#[cfg(windows)]
impl WordComMethod {
    fn probe(&self, target: &ForegroundTarget) -> Option<MethodProbe> {
        (is_word_target(target) || is_outlook_target(target))
            .then(|| MethodProbe::safe(MethodId::WordCom, "Word COM object model"))
    }

    fn capture(
        &self,
        foreground: isize,
        focused: isize,
        app_class: &str,
        focused_class: &str,
    ) -> Result<TextContext, PlatformError> {
        let process_name = window_process_name(foreground);
        let is_outlook =
            is_outlook_class_or_process(app_class, focused_class, process_name.as_deref());
        let output = run_powershell_script(
            if is_outlook {
                OUTLOOK_WORD_CAPTURE_SCRIPT
            } else {
                WORD_CAPTURE_SCRIPT
            },
            &[],
        )?;
        let fields = parse_key_value_lines(&output);
        if fields.get("ok").map(String::as_str) != Some("1") {
            return Err(PlatformError::ReplacementUnavailableReason(
                fields
                    .get("error")
                    .cloned()
                    .unwrap_or_else(|| String::from("uia_capture_failed")),
            ));
        }
        let text = fields
            .get("text_b64")
            .and_then(|value| decode_utf16le_base64(value).ok())
            .ok_or(PlatformError::ReplacementUnavailable)?;
        let base = fields
            .get("base")
            .and_then(|value| value.parse::<usize>().ok())
            .ok_or(PlatformError::ReplacementUnavailable)?;
        let selection_range = (fields.get("kind").map(String::as_str) == Some("selection"))
            .then(|| TextRange::new(0, text.len()));
        if text.trim().is_empty() {
            return Err(PlatformError::ReplacementUnavailable);
        }

        Ok(TextContext {
            app_id: format!("{app_class}/{focused_class}"),
            window_id: hwnd_id(foreground),
            control_id: format!(
                "{}:{}:{}",
                if is_outlook {
                    "outlook-word-com"
                } else {
                    "word-com"
                },
                base,
                hwnd_id(focused)
            ),
            caret_range: TextRange::caret(text.len()),
            selection_range,
            text_snapshot: text,
            capabilities: Capabilities {
                can_replace_directly: true,
                can_read_selection: true,
                can_read_caret: true,
                method_binding: Some(MethodBinding::new(
                    MethodId::WordCom,
                    vec![MethodId::WordCom],
                )),
            },
        })
    }

    fn apply(
        &self,
        context: &TextContext,
        plan: &ReplacementPlan,
    ) -> Result<ApplyReplacementResult, PlatformError> {
        let base = parse_word_com_base(&context.control_id)
            .ok_or(PlatformError::ReplacementUnavailable)?;
        let is_outlook = context.control_id.starts_with("outlook-word-com:");
        let actual_before = slice_by_range(&context.text_snapshot, plan.range)
            .ok_or(PlatformError::PreflightFailed)?
            .to_owned();
        if actual_before != plan.expected_before_text {
            return Err(PlatformError::PreflightFailed);
        }

        let abs_start = base + byte_offset_to_utf16(&context.text_snapshot, plan.range.start);
        let abs_end = base + byte_offset_to_utf16(&context.text_snapshot, plan.range.end);
        let original_caret =
            base + byte_offset_to_utf16(&context.text_snapshot, context.caret_range.end);
        let replacement_delta = plan.replacement_text.encode_utf16().count() as isize
            - plan.expected_before_text.encode_utf16().count() as isize;
        let target_caret = if context.caret_range.end >= plan.range.end {
            original_caret.saturating_add_signed(replacement_delta)
        } else {
            original_caret
        };
        let env = [
            ("STEPLER_WORD_START", abs_start.to_string()),
            ("STEPLER_WORD_END", abs_end.to_string()),
            ("STEPLER_WORD_CARET", target_caret.to_string()),
            (
                "STEPLER_WORD_EXPECTED_B64",
                encode_utf16le_base64(&plan.expected_before_text),
            ),
            (
                "STEPLER_WORD_REPLACEMENT_B64",
                encode_utf16le_base64(&plan.replacement_text),
            ),
        ];
        let output = run_powershell_script(
            if is_outlook {
                OUTLOOK_WORD_APPLY_SCRIPT
            } else {
                WORD_APPLY_SCRIPT
            },
            &env,
        )?;
        let fields = parse_key_value_lines(&output);
        if fields.get("ok").map(String::as_str) != Some("1") {
            return Err(PlatformError::PreflightFailed);
        }
        let actual_after = fields
            .get("after_b64")
            .and_then(|value| decode_utf16le_base64(value).ok());

        Ok(ApplyReplacementResult {
            applied: true,
            actual_before_text: Some(actual_before),
            actual_after_text: actual_after,
            method: MethodId::WordCom.as_str().to_owned(),
        })
    }
}

#[cfg(windows)]
#[derive(Debug, Default, Clone, Copy)]
struct WebKeyboardSelectionMethod;

#[cfg(windows)]
impl WebKeyboardSelectionMethod {
    fn probe(&self, target: &ForegroundTarget) -> Option<MethodProbe> {
        if !is_browser_like_target(target) && !is_notepad_like_target(target) {
            return None;
        }

        let mut probe = MethodProbe::safe(
            MethodId::WebKeyboardSelection,
            "browser/editor keyboard selection with clipboard preflight",
        );
        probe.requires_clipboard = true;
        Some(probe)
    }

    fn capture(
        &self,
        foreground: isize,
        focused: isize,
        app_class: &str,
        focused_class: &str,
    ) -> Result<TextContext, PlatformError> {
        let expected_foreground = foreground;
        if foreground_hwnd()? != expected_foreground {
            return Err(PlatformError::PreflightFailed);
        }

        for attempt in 0..2 {
            let snapshot = capture_clipboard_text_only()?;
            let scrolllock_mode = active_correction_mode_is_scrolllock();

            let selected = copy_selected_text_checked(&snapshot, Duration::from_millis(220))
                .filter(|text| !text.trim().is_empty())
                .filter(|text| !looks_like_hotkeyhandler_marker(text));
            if let Some(text) = selected {
                let text_len = text.len();
                let _ = restore_clipboard_text_only(&snapshot);
                return Ok(TextContext {
                    app_id: format!("{app_class}/{focused_class}"),
                    window_id: hwnd_id(foreground),
                    control_id: format!("web-keyboard-selection-selected:{}", hwnd_id(focused)),
                    text_snapshot: text,
                    caret_range: TextRange::caret(text_len),
                    selection_range: Some(TextRange::new(0, text_len)),
                    capabilities: Capabilities {
                        can_replace_directly: false,
                        can_read_selection: true,
                        can_read_caret: true,
                        method_binding: Some(MethodBinding::new(
                            MethodId::WebKeyboardSelection,
                            vec![MethodId::WebKeyboardSelection],
                        )),
                    },
                });
            }

            if scrolllock_mode {
                select_web_left_context();
                let copied_raw = copy_selected_text_checked(&snapshot, Duration::from_millis(280));
                let copied = copied_raw
                    .filter(|text| !text.trim().is_empty())
                    .filter(|text| !looks_like_hotkeyhandler_marker(text));
                send_key(VK_RIGHT);
                let _ = restore_clipboard_text_only(&snapshot);

                if let Some(text) = copied {
                    let text_len = text.len();
                    return Ok(TextContext {
                        app_id: format!("{app_class}/{focused_class}"),
                        window_id: hwnd_id(foreground),
                        control_id: format!("web-keyboard-selection:{}", hwnd_id(focused)),
                        text_snapshot: text,
                        caret_range: TextRange::caret(text_len),
                        selection_range: None,
                        capabilities: Capabilities {
                            can_replace_directly: false,
                            can_read_selection: false,
                            can_read_caret: true,
                            method_binding: Some(MethodBinding::new(
                                MethodId::WebKeyboardSelection,
                                vec![MethodId::WebKeyboardSelection],
                            )),
                        },
                    });
                }

                let snapshot = capture_clipboard_text_only()?;
                select_web_all_context();
                let copied_raw = copy_selected_text_checked(&snapshot, Duration::from_millis(280));
                let copied = copied_raw
                    .filter(|text| is_plausible_web_field_text(text))
                    .filter(|text| !looks_like_hotkeyhandler_marker(text));
                let _ = restore_clipboard_text_only(&snapshot);

                if let Some(text) = copied {
                    let text_len = text.len();
                    return Ok(TextContext {
                        app_id: format!("{app_class}/{focused_class}"),
                        window_id: hwnd_id(foreground),
                        control_id: format!("web-keyboard-field-selection:{}", hwnd_id(focused)),
                        text_snapshot: text,
                        caret_range: TextRange::caret(text_len),
                        selection_range: Some(TextRange::new(0, text_len)),
                        capabilities: Capabilities {
                            can_replace_directly: false,
                            can_read_selection: true,
                            can_read_caret: true,
                            method_binding: Some(MethodBinding::new(
                                MethodId::WebKeyboardSelection,
                                vec![MethodId::WebKeyboardSelection],
                            )),
                        },
                    });
                }
                send_key(VK_RIGHT);

                let snapshot = capture_clipboard_text_only()?;
                select_web_line_left_context();
                let copied_raw = copy_selected_text_checked(&snapshot, Duration::from_millis(280));
                let copied = copied_raw
                    .filter(|text| !text.trim().is_empty())
                    .filter(|text| !looks_like_hotkeyhandler_marker(text));
                send_key(VK_RIGHT);
                let _ = restore_clipboard_text_only(&snapshot);

                if let Some(text) = copied {
                    let text_len = text.len();
                    return Ok(TextContext {
                        app_id: format!("{app_class}/{focused_class}"),
                        window_id: hwnd_id(foreground),
                        control_id: format!("web-keyboard-line-selection:{}", hwnd_id(focused)),
                        text_snapshot: text,
                        caret_range: TextRange::caret(text_len),
                        selection_range: None,
                        capabilities: Capabilities {
                            can_replace_directly: false,
                            can_read_selection: false,
                            can_read_caret: true,
                            method_binding: Some(MethodBinding::new(
                                MethodId::WebKeyboardSelection,
                                vec![MethodId::WebKeyboardSelection],
                            )),
                        },
                    });
                }

                release_modifier_keys();
                std::thread::sleep(Duration::from_millis(50));
                continue;
            }

            select_web_left_context();
            let copied_raw = copy_selected_text_checked(&snapshot, Duration::from_millis(450));
            let copied = copied_raw
                .filter(|text| !text.trim().is_empty())
                .filter(|text| !looks_like_hotkeyhandler_marker(text));
            send_key(VK_RIGHT);
            let _ = restore_clipboard_text_only(&snapshot);

            if let Some(text) = copied {
                let text_len = text.len();
                return Ok(TextContext {
                    app_id: format!("{app_class}/{focused_class}"),
                    window_id: hwnd_id(foreground),
                    control_id: format!("web-keyboard-selection:{}", hwnd_id(focused)),
                    text_snapshot: text,
                    caret_range: TextRange::caret(text_len),
                    selection_range: None,
                    capabilities: Capabilities {
                        can_replace_directly: false,
                        can_read_selection: false,
                        can_read_caret: true,
                        method_binding: Some(MethodBinding::new(
                            MethodId::WebKeyboardSelection,
                            vec![MethodId::WebKeyboardSelection],
                        )),
                    },
                });
            }

            if attempt == 0 {
                release_modifier_keys();
                std::thread::sleep(Duration::from_millis(80));
                let snapshot = capture_clipboard_text_only()?;
                select_web_line_left_context();
                let copied_raw = copy_selected_text_checked(&snapshot, Duration::from_millis(450));
                let copied = copied_raw
                    .filter(|text| !text.trim().is_empty())
                    .filter(|text| !looks_like_hotkeyhandler_marker(text));
                send_key(VK_RIGHT);
                let _ = restore_clipboard_text_only(&snapshot);

                if let Some(text) = copied {
                    let text_len = text.len();
                    return Ok(TextContext {
                        app_id: format!("{app_class}/{focused_class}"),
                        window_id: hwnd_id(foreground),
                        control_id: format!("web-keyboard-line-selection:{}", hwnd_id(focused)),
                        text_snapshot: text,
                        caret_range: TextRange::caret(text_len),
                        selection_range: None,
                        capabilities: Capabilities {
                            can_replace_directly: false,
                            can_read_selection: false,
                            can_read_caret: true,
                            method_binding: Some(MethodBinding::new(
                                MethodId::WebKeyboardSelection,
                                vec![MethodId::WebKeyboardSelection],
                            )),
                        },
                    });
                }
            }

            let snapshot = capture_clipboard_text_only()?;
            select_web_all_context();
            let copied_raw = copy_selected_text_checked(&snapshot, Duration::from_millis(320));
            let copied = copied_raw
                .filter(|text| is_plausible_web_field_text(text))
                .filter(|text| !looks_like_hotkeyhandler_marker(text));
            let _ = restore_clipboard_text_only(&snapshot);

            if let Some(text) = copied {
                let text_len = text.len();
                return Ok(TextContext {
                    app_id: format!("{app_class}/{focused_class}"),
                    window_id: hwnd_id(foreground),
                    control_id: format!("web-keyboard-field-selection:{}", hwnd_id(focused)),
                    text_snapshot: text,
                    caret_range: TextRange::caret(text_len),
                    selection_range: Some(TextRange::new(0, text_len)),
                    capabilities: Capabilities {
                        can_replace_directly: false,
                        can_read_selection: true,
                        can_read_caret: true,
                        method_binding: Some(MethodBinding::new(
                            MethodId::WebKeyboardSelection,
                            vec![MethodId::WebKeyboardSelection],
                        )),
                    },
                });
            }
            send_key(VK_RIGHT);

            if attempt == 0 {
                release_modifier_keys();
                std::thread::sleep(Duration::from_millis(180));
            }
        }

        Err(PlatformError::ReplacementUnavailableReason(String::from(
            "web_keyboard_capture_empty_after_left_context_retry",
        )))
    }

    fn apply(
        &self,
        context: &TextContext,
        plan: &ReplacementPlan,
    ) -> Result<ApplyReplacementResult, PlatformError> {
        let actual_before = slice_by_range(&context.text_snapshot, plan.range)
            .ok_or_else(|| {
                PlatformError::ReplacementUnavailableReason(String::from(
                    "web_keyboard_preflight invalid_range",
                ))
            })?
            .to_owned();
        if actual_before != plan.expected_before_text {
            return Err(PlatformError::ReplacementUnavailableReason(format!(
                "web_keyboard_preflight plan_expected={} actual_range={}",
                preview_for_error(&plan.expected_before_text, 40),
                preview_for_error(&actual_before, 40)
            )));
        }
        let expected_foreground = parse_hwnd_id(&context.window_id).ok_or_else(|| {
            PlatformError::ReplacementUnavailableReason(String::from(
                "web_keyboard_preflight invalid_hwnd",
            ))
        })?;
        if foreground_hwnd()? != expected_foreground {
            return Err(PlatformError::ReplacementUnavailableReason(String::from(
                "web_keyboard_preflight foreground_changed",
            )));
        }

        if context.selection_range.is_some() {
            send_unicode_text(&plan.replacement_text)?;
            return Ok(ApplyReplacementResult {
                applied: true,
                actual_before_text: Some(actual_before),
                actual_after_text: Some(plan.replacement_text.clone()),
                method: MethodId::WebKeyboardSelection.as_str().to_owned(),
            });
        }

        let replace_entire_context = plan.range.end != context.text_snapshot.len();
        let replacement_text = if replace_entire_context {
            let mut rebuilt = String::with_capacity(
                context.text_snapshot.len() - actual_before.len() + plan.replacement_text.len(),
            );
            rebuilt.push_str(&context.text_snapshot[..plan.range.start]);
            rebuilt.push_str(&plan.replacement_text);
            rebuilt.push_str(&context.text_snapshot[plan.range.end..]);
            rebuilt
        } else {
            plan.replacement_text.clone()
        };
        let expected_selection = if replace_entire_context {
            context.text_snapshot.as_str()
        } else {
            actual_before.as_str()
        };

        let snapshot = capture_clipboard_text_only()?;
        let use_line_selection = is_web_keyboard_line_context(&context.control_id)
            && context.selection_range.is_none()
            && (plan.range.start == 0 || replace_entire_context);
        let mut selected = if use_line_selection {
            select_web_line_left_context();
            std::thread::sleep(Duration::from_millis(35));
            copy_selected_text_checked(&snapshot, Duration::from_millis(450))
        } else {
            select_left_utf16_units(expected_selection.encode_utf16().count())?;
            std::thread::sleep(Duration::from_millis(35));
            copy_selected_text_checked(&snapshot, Duration::from_millis(450))
        };
        if !replace_entire_context && selected.as_deref() != Some(expected_selection) {
            selected = extend_web_selection_to_expected_prefix(
                selected,
                &actual_before,
                &snapshot,
                Duration::from_millis(450),
            );
        }
        if selected.as_deref() != Some(expected_selection) {
            send_key(VK_RIGHT);
            let _ = restore_clipboard_text_only(&snapshot);
            return Err(PlatformError::ReplacementUnavailableReason(format!(
                "web_keyboard_preflight expected={} actual={}",
                preview_for_error(&actual_before, 40),
                preview_for_error(selected.as_deref().unwrap_or("<none>"), 40)
            )));
        }

        let _ = restore_clipboard_text_only(&snapshot);
        std::thread::sleep(Duration::from_millis(20));
        send_unicode_text(&replacement_text)?;

        Ok(ApplyReplacementResult {
            applied: true,
            actual_before_text: Some(actual_before),
            actual_after_text: Some(replacement_text),
            method: MethodId::WebKeyboardSelection.as_str().to_owned(),
        })
    }
}

#[cfg(windows)]
fn is_web_keyboard_line_context(control_id: &str) -> bool {
    control_id.starts_with("web-keyboard-line-selection:")
}

#[cfg(windows)]
#[derive(Debug, Default, Clone, Copy)]
struct UiAutomationTextMethod;

#[cfg(windows)]
impl UiAutomationTextMethod {
    fn probe(&self, target: &ForegroundTarget) -> Option<MethodProbe> {
        if is_supported_edit_class(&target.focused_class)
            || is_supported_terminal_class(&target.app_class, &target.focused_class)
            || is_word_target(target)
            || is_browser_like_target(target)
            || target.app_class.eq_ignore_ascii_case("Progman")
            || target.app_class.eq_ignore_ascii_case("WorkerW")
            || target.focused_class.eq_ignore_ascii_case("SysListView32")
        {
            return None;
        }

        Some(MethodProbe::safe(
            MethodId::UiAutomationText,
            "focused UI Automation text/value candidate",
        ))
    }

    fn capture(
        &self,
        foreground: isize,
        focused: isize,
        app_class: &str,
        focused_class: &str,
    ) -> Result<TextContext, PlatformError> {
        capture_uia_text_context(foreground, focused, app_class, focused_class, false)
    }

    fn apply(
        &self,
        context: &TextContext,
        plan: &ReplacementPlan,
    ) -> Result<ApplyReplacementResult, PlatformError> {
        apply_uia_text_replacement(context, plan, MethodId::UiAutomationText)
    }
}

#[cfg(windows)]
#[derive(Debug, Default, Clone, Copy)]
struct UiAutomationEditableTextMethod;

#[cfg(windows)]
impl UiAutomationEditableTextMethod {
    fn probe(&self, target: &ForegroundTarget) -> Option<MethodProbe> {
        if is_supported_edit_class(&target.focused_class)
            || is_supported_terminal_class(&target.app_class, &target.focused_class)
            || is_word_target(target)
            || target.app_class.eq_ignore_ascii_case("Progman")
            || target.app_class.eq_ignore_ascii_case("WorkerW")
            || target.focused_class.eq_ignore_ascii_case("SysListView32")
        {
            return None;
        }

        Some(MethodProbe::safe(
            MethodId::UiAutomationEditableText,
            "focused UI Automation editable text candidate",
        ))
    }

    fn capture(
        &self,
        foreground: isize,
        focused: isize,
        app_class: &str,
        focused_class: &str,
    ) -> Result<TextContext, PlatformError> {
        let mut context =
            capture_uia_text_context(foreground, focused, app_class, focused_class, true)?;
        context.capabilities.method_binding = Some(MethodBinding::new(
            MethodId::UiAutomationEditableText,
            vec![MethodId::UiAutomationEditableText],
        ));
        Ok(context)
    }

    fn apply(
        &self,
        context: &TextContext,
        plan: &ReplacementPlan,
    ) -> Result<ApplyReplacementResult, PlatformError> {
        apply_uia_text_replacement(context, plan, MethodId::UiAutomationEditableText)
    }
}

#[cfg(windows)]
#[derive(Debug, Default, Clone, Copy)]
struct UiAutomationDocumentTextMethod;

#[cfg(windows)]
impl UiAutomationDocumentTextMethod {
    fn probe(&self, target: &ForegroundTarget) -> Option<MethodProbe> {
        if is_supported_edit_class(&target.focused_class)
            || is_supported_terminal_class(&target.app_class, &target.focused_class)
            || is_word_target(target)
            || target.app_class.eq_ignore_ascii_case("Progman")
            || target.app_class.eq_ignore_ascii_case("WorkerW")
            || target.focused_class.eq_ignore_ascii_case("SysListView32")
        {
            return None;
        }

        Some(MethodProbe::safe(
            MethodId::UiAutomationDocumentText,
            "focused UI Automation document text selection candidate",
        ))
    }

    fn capture_with_options(
        &self,
        foreground: isize,
        focused: isize,
        app_class: &str,
        focused_class: &str,
        allow_caret_fallback: bool,
    ) -> Result<TextContext, PlatformError> {
        let env = [
            ("STEPLER_UIA_FOREGROUND_HWND", foreground.to_string()),
            (
                "STEPLER_UIA_DOCUMENT_ALLOW_CARET_FALLBACK",
                if allow_caret_fallback { "1" } else { "0" }.to_owned(),
            ),
        ];
        let output = run_powershell_script(UIA_DOCUMENT_CAPTURE_SCRIPT, &env)?;
        let fields = parse_key_value_lines(&output);
        if fields.get("ok").map(String::as_str) != Some("1") {
            return Err(PlatformError::ReplacementUnavailableReason(
                fields
                    .get("error")
                    .cloned()
                    .unwrap_or_else(|| String::from("uia_document_capture_failed")),
            ));
        }
        let text = fields
            .get("text_b64")
            .and_then(|value| decode_utf16le_base64(value).ok())
            .ok_or_else(|| {
                PlatformError::ReplacementUnavailableReason(String::from(
                    "missing_or_invalid_selection_text",
                ))
            })?;
        if text.trim().is_empty() {
            return Err(PlatformError::ReplacementUnavailableReason(String::from(
                "empty_selection_text",
            )));
        }
        let runtime_id = fields
            .get("runtime_id")
            .cloned()
            .unwrap_or_else(|| String::from("unknown"));
        let is_caret = fields.get("kind").map(String::as_str) == Some("caret");
        let control_prefix = if is_caret { "uia-doc-caret" } else { "uia-doc" };
        let selection_range = if is_caret {
            None
        } else {
            Some(TextRange::new(0, text.len()))
        };
        Ok(TextContext {
            app_id: format!("{app_class}/{focused_class}"),
            window_id: hwnd_id(foreground),
            control_id: format!("{control_prefix}:{}:{}", runtime_id, hwnd_id(focused)),
            text_snapshot: text.clone(),
            caret_range: TextRange::caret(text.len()),
            selection_range,
            capabilities: Capabilities {
                can_replace_directly: true,
                can_read_selection: !is_caret,
                can_read_caret: is_caret,
                method_binding: Some(MethodBinding::new(
                    MethodId::UiAutomationDocumentText,
                    vec![MethodId::UiAutomationDocumentText],
                )),
            },
        })
    }

    fn apply(
        &self,
        context: &TextContext,
        plan: &ReplacementPlan,
    ) -> Result<ApplyReplacementResult, PlatformError> {
        if context.control_id.starts_with("uia-doc-caret:") {
            return self.apply_caret_range(context, plan);
        }

        if plan.range != TextRange::new(0, context.text_snapshot.len())
            || plan.expected_before_text != context.text_snapshot
        {
            return Err(PlatformError::PreflightFailed);
        }
        if env_flag_enabled("STEPLER_UIA_DOCUMENT_STRICT_APPLY", false) {
            return self.apply_strict(context, plan);
        }

        let expected_foreground =
            parse_hwnd_id(&context.window_id).ok_or(PlatformError::PreflightFailed)?;
        if foreground_hwnd()? != expected_foreground {
            return Err(PlatformError::PreflightFailed);
        }

        send_unicode_text(&plan.replacement_text)?;
        std::thread::sleep(Duration::from_millis(5));

        Ok(ApplyReplacementResult {
            applied: true,
            actual_before_text: Some(context.text_snapshot.clone()),
            actual_after_text: Some(plan.replacement_text.clone()),
            method: MethodId::UiAutomationDocumentText.as_str().to_owned(),
        })
    }

    fn apply_caret_range(
        &self,
        context: &TextContext,
        plan: &ReplacementPlan,
    ) -> Result<ApplyReplacementResult, PlatformError> {
        let actual_before = slice_by_range(&context.text_snapshot, plan.range)
            .ok_or(PlatformError::PreflightFailed)?
            .to_owned();
        if actual_before != plan.expected_before_text {
            return Err(PlatformError::PreflightFailed);
        }
        let runtime_id = parse_uia_document_runtime_id(&context.control_id)
            .ok_or(PlatformError::ReplacementUnavailable)?;
        let left_len_utf16 = context.text_snapshot.encode_utf16().count();
        let start_utf16 = byte_offset_to_utf16(&context.text_snapshot, plan.range.start);
        let end_utf16 = byte_offset_to_utf16(&context.text_snapshot, plan.range.end);
        let select_output = run_powershell_script(
            UIA_DOCUMENT_SELECT_CARET_RANGE_SCRIPT,
            &[
                (
                    "STEPLER_UIA_FOREGROUND_HWND",
                    parse_hwnd_id(&context.window_id)
                        .map(|hwnd| hwnd.to_string())
                        .unwrap_or_default(),
                ),
                ("STEPLER_UIA_RUNTIME_ID", runtime_id),
                (
                    "STEPLER_UIA_EXPECTED_B64",
                    encode_utf16le_base64(&plan.expected_before_text),
                ),
                (
                    "STEPLER_UIA_START_DELTA_UTF16",
                    (start_utf16 as isize - left_len_utf16 as isize).to_string(),
                ),
                (
                    "STEPLER_UIA_END_DELTA_UTF16",
                    (end_utf16 as isize - left_len_utf16 as isize).to_string(),
                ),
            ],
        )?;
        let select_fields = parse_key_value_lines(&select_output);
        if select_fields.get("ok").map(String::as_str) != Some("1") {
            return Err(PlatformError::PreflightFailed);
        }

        let expected_foreground =
            parse_hwnd_id(&context.window_id).ok_or(PlatformError::PreflightFailed)?;
        if foreground_hwnd()? != expected_foreground {
            return Err(PlatformError::PreflightFailed);
        }

        send_unicode_text(&plan.replacement_text)?;
        std::thread::sleep(Duration::from_millis(20));

        Ok(ApplyReplacementResult {
            applied: true,
            actual_before_text: Some(actual_before),
            actual_after_text: Some(plan.replacement_text.clone()),
            method: MethodId::UiAutomationDocumentText.as_str().to_owned(),
        })
    }

    fn apply_strict(
        &self,
        context: &TextContext,
        plan: &ReplacementPlan,
    ) -> Result<ApplyReplacementResult, PlatformError> {
        let runtime_id = parse_uia_document_runtime_id(&context.control_id)
            .ok_or(PlatformError::ReplacementUnavailable)?;
        let select_output = run_powershell_script(
            UIA_DOCUMENT_SELECT_SCRIPT,
            &[
                (
                    "STEPLER_UIA_FOREGROUND_HWND",
                    parse_hwnd_id(&context.window_id)
                        .map(|hwnd| hwnd.to_string())
                        .unwrap_or_default(),
                ),
                ("STEPLER_UIA_RUNTIME_ID", runtime_id.clone()),
                (
                    "STEPLER_UIA_EXPECTED_B64",
                    encode_utf16le_base64(&plan.expected_before_text),
                ),
            ],
        )?;
        let select_fields = parse_key_value_lines(&select_output);
        if select_fields.get("ok").map(String::as_str) != Some("1") {
            return Err(PlatformError::PreflightFailed);
        }

        send_unicode_text(&plan.replacement_text)?;
        std::thread::sleep(Duration::from_millis(80));

        let verify_output = run_powershell_script(
            UIA_DOCUMENT_VERIFY_SCRIPT,
            &[
                (
                    "STEPLER_UIA_FOREGROUND_HWND",
                    parse_hwnd_id(&context.window_id)
                        .map(|hwnd| hwnd.to_string())
                        .unwrap_or_default(),
                ),
                ("STEPLER_UIA_RUNTIME_ID", runtime_id),
                (
                    "STEPLER_UIA_REPLACEMENT_B64",
                    encode_utf16le_base64(&plan.replacement_text),
                ),
            ],
        )?;
        let verify_fields = parse_key_value_lines(&verify_output);
        if verify_fields.get("ok").map(String::as_str) != Some("1") {
            return Err(PlatformError::PreflightFailed);
        }
        let actual_after = verify_fields
            .get("actual_b64")
            .and_then(|value| decode_utf16le_base64(value).ok());

        Ok(ApplyReplacementResult {
            applied: true,
            actual_before_text: Some(context.text_snapshot.clone()),
            actual_after_text: actual_after,
            method: MethodId::UiAutomationDocumentText.as_str().to_owned(),
        })
    }
}

#[cfg(windows)]
fn capture_uia_text_context(
    foreground: isize,
    focused: isize,
    app_class: &str,
    focused_class: &str,
    strict_editable: bool,
) -> Result<TextContext, PlatformError> {
    let strict_value = if strict_editable { "1" } else { "0" };
    let output = run_powershell_script(
        UIA_CAPTURE_SCRIPT,
        &[
            ("STEPLER_UIA_FOREGROUND_HWND", foreground.to_string()),
            ("STEPLER_UIA_STRICT_EDITABLE", strict_value.to_owned()),
        ],
    )?;
    let fields = parse_key_value_lines(&output);
    if fields.get("ok").map(String::as_str) != Some("1") {
        return Err(PlatformError::ReplacementUnavailableReason(
            fields
                .get("error")
                .cloned()
                .unwrap_or_else(|| String::from("uia_capture_failed")),
        ));
    }
    if fields.get("can_set_value").map(String::as_str) != Some("1") {
        return Err(PlatformError::ReplacementUnavailableReason(String::from(
            "no_writable_value",
        )));
    }

    let text = fields
        .get("text_b64")
        .and_then(|value| decode_utf16le_base64(value).ok())
        .ok_or_else(|| {
            PlatformError::ReplacementUnavailableReason(String::from("missing_or_invalid_text"))
        })?;
    if text.is_empty() {
        return Err(PlatformError::ReplacementUnavailableReason(String::from(
            "empty_text",
        )));
    }

    let selection_start_utf16 = fields
        .get("selection_start")
        .and_then(|value| value.parse::<usize>().ok());
    let selection_end_utf16 = fields
        .get("selection_end")
        .and_then(|value| value.parse::<usize>().ok());
    let caret_utf16 = fields
        .get("caret")
        .and_then(|value| value.parse::<usize>().ok());

    let selection_range =
        selection_start_utf16
            .zip(selection_end_utf16)
            .and_then(|(start, end)| {
                if start == end {
                    return None;
                }
                Some(TextRange::new(
                    edit_offset_to_byte_offset(&text, start)?,
                    edit_offset_to_byte_offset(&text, end)?,
                ))
            });
    let caret = caret_utf16
        .and_then(|caret| edit_offset_to_byte_offset(&text, caret))
        .or_else(|| selection_range.map(|range| range.end))
        .unwrap_or_else(|| text.len());
    let selection_range = selection_range.or_else(|| {
        let start = selection_start_utf16?;
        let end = selection_end_utf16?;
        if start != end {
            return None;
        }
        let caret = edit_offset_to_byte_offset(&text, start)?;
        (caret != text.len()).then_some(TextRange::caret(caret))
    });
    let user_selection_range = selection_range.and_then(|range| {
        if range.start == range.end {
            None
        } else {
            Some(range)
        }
    });
    let caret = user_selection_range.map(|range| range.end).unwrap_or(caret);
    let runtime_id = fields
        .get("runtime_id")
        .cloned()
        .unwrap_or_else(|| String::from("unknown"));
    let method = if strict_editable {
        MethodId::UiAutomationEditableText
    } else {
        MethodId::UiAutomationText
    };

    Ok(TextContext {
        app_id: format!("{app_class}/{focused_class}"),
        window_id: hwnd_id(foreground),
        control_id: format!("uia:{}:{}", runtime_id, hwnd_id(focused)),
        text_snapshot: text,
        caret_range: TextRange::caret(caret),
        selection_range: user_selection_range,
        capabilities: Capabilities {
            can_replace_directly: true,
            can_read_selection: user_selection_range.is_some(),
            can_read_caret: true,
            method_binding: Some(MethodBinding::new(method, vec![method])),
        },
    })
}

#[cfg(windows)]
fn apply_uia_text_replacement(
    context: &TextContext,
    plan: &ReplacementPlan,
    method: MethodId,
) -> Result<ApplyReplacementResult, PlatformError> {
    let actual_before = slice_by_range(&context.text_snapshot, plan.range)
        .ok_or(PlatformError::PreflightFailed)?
        .to_owned();
    if actual_before != plan.expected_before_text {
        return Err(PlatformError::PreflightFailed);
    }

    let replacement =
        replace_range_text(&context.text_snapshot, plan.range, &plan.replacement_text)
            .ok_or(PlatformError::PreflightFailed)?;
    let runtime_id =
        parse_uia_runtime_id(&context.control_id).ok_or(PlatformError::ReplacementUnavailable)?;
    let caret_after_utf16 = byte_offset_to_utf16(&context.text_snapshot, plan.range.start)
        + plan.replacement_text.encode_utf16().count();
    let env = [
        (
            "STEPLER_UIA_FOREGROUND_HWND",
            parse_hwnd_id(&context.window_id)
                .map(|hwnd| hwnd.to_string())
                .unwrap_or_default(),
        ),
        ("STEPLER_UIA_RUNTIME_ID", runtime_id),
        (
            "STEPLER_UIA_EXPECTED_B64",
            encode_utf16le_base64(&context.text_snapshot),
        ),
        (
            "STEPLER_UIA_REPLACEMENT_B64",
            encode_utf16le_base64(&replacement),
        ),
        ("STEPLER_UIA_CARET_UTF16", caret_after_utf16.to_string()),
    ];
    let output = run_powershell_script(UIA_APPLY_SCRIPT, &env)?;
    let fields = parse_key_value_lines(&output);
    if fields.get("ok").map(String::as_str) != Some("1") {
        return Err(PlatformError::PreflightFailed);
    }
    let actual_after = fields
        .get("after_b64")
        .and_then(|value| decode_utf16le_base64(value).ok());

    Ok(ApplyReplacementResult {
        applied: true,
        actual_before_text: Some(actual_before),
        actual_after_text: actual_after,
        method: method.as_str().to_owned(),
    })
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
mod tests {
    use super::*;
    use stepler_platform::HotkeyListener;

    #[test]
    fn skeleton_hotkey_listener_tracks_running_state() {
        let mut listener = WindowsHotkeyListener::default();

        assert!(!listener.is_running());

        listener.start().unwrap();
        assert!(listener.is_running());

        listener.stop().unwrap();
        assert!(!listener.is_running());
    }

    #[test]
    fn context_id_parser_rejects_invalid_ids() {
        assert_eq!(parse_hwnd_id("nope"), None);
        assert_eq!(parse_hwnd_id("hwnd:"), None);
        assert_eq!(parse_hwnd_id("hwnd:XYZ"), None);
        assert_eq!(parse_hwnd_id("hwnd:1A"), Some(0x1A));
    }

    #[cfg(windows)]
    #[test]
    fn test_foreground_override_parses_decimal_and_hex_hwnds() {
        unsafe {
            std::env::set_var("STEPLER_TEST_FOREGROUND_HWND", "123");
        }
        assert_eq!(test_foreground_hwnd_override(), Some(123));
        unsafe {
            std::env::set_var("STEPLER_TEST_FOREGROUND_HWND", "0x7B");
        }
        assert_eq!(test_foreground_hwnd_override(), Some(123));
        unsafe {
            std::env::remove_var("STEPLER_TEST_FOREGROUND_HWND");
        }
    }

    #[test]
    fn supported_edit_class_is_allowlisted() {
        assert!(is_supported_edit_class("Edit"));
        assert!(is_supported_edit_class("RICHEDIT50W"));
        assert!(is_supported_edit_class("RichEditD2DPT"));
        assert!(!is_supported_edit_class("ConsoleWindowClass"));
        assert!(!is_supported_edit_class("Notepad"));
    }

    #[test]
    fn supported_terminal_class_is_allowlisted() {
        assert!(is_supported_terminal_class(
            "CASCADIA_HOSTING_WINDOW_CLASS",
            "Windows.UI.Input.InputSite.WindowClass"
        ));
        assert!(is_supported_terminal_class(
            "ConsoleWindowClass",
            "ConsoleWindowClass"
        ));
        assert!(!is_supported_terminal_class("Notepad", "Edit"));
    }

    #[test]
    fn classic_console_class_requires_foreground_and_focus_console() {
        assert!(is_classic_console_class(
            "ConsoleWindowClass",
            "ConsoleWindowClass"
        ));
        assert!(!is_classic_console_class(
            "CASCADIA_HOSTING_WINDOW_CLASS",
            "Windows.UI.Input.InputSite.WindowClass"
        ));
    }

    #[test]
    fn classic_console_is_not_psreadline_passthrough_terminal() {
        assert!(is_psreadline_passthrough_terminal_class(
            "CASCADIA_HOSTING_WINDOW_CLASS",
            "Windows.UI.Input.InputSite.WindowClass"
        ));
        assert!(!is_psreadline_passthrough_terminal_class(
            "ConsoleWindowClass",
            "ConsoleWindowClass"
        ));
    }

    #[test]
    fn classic_console_does_not_need_conservative_suppression() {
        assert!(!terminal_class_needs_conservative_suppression(
            "ConsoleWindowClass",
            "ConsoleWindowClass"
        ));
    }

    #[test]
    fn windows_terminal_needs_conservative_suppression() {
        assert!(terminal_class_needs_conservative_suppression(
            "CASCADIA_HOSTING_WINDOW_CLASS",
            "Windows.UI.Input.InputSite.WindowClass"
        ));
    }

    #[test]
    fn cmd_terminal_title_is_detected() {
        assert!(is_cmd_terminal_title("C:\\WINDOWS\\system32\\cmd.exe"));
        assert!(!is_cmd_terminal_title("PowerShell 7 (x64)"));
    }

    #[test]
    fn ssh_terminal_title_is_detected_without_matching_powershell() {
        assert!(is_ssh_terminal_title("vpnuser"));
        assert!(is_ssh_terminal_title("root@example"));
        assert!(is_ssh_terminal_title("ssh user@example"));
        assert!(!is_ssh_terminal_title("Windows PowerShell"));
        assert!(!is_ssh_terminal_title("PowerShell"));
        assert!(!is_ssh_terminal_title("PowerShell 7 (x64)"));
        assert!(!is_ssh_terminal_title("C:\\WINDOWS\\system32\\cmd.exe"));
    }

    #[test]
    fn terminal_passthrough_keeps_only_local_powershell_forwardable() {
        assert_eq!(
            terminal_passthrough_for_window(
                "CASCADIA_HOSTING_WINDOW_CLASS",
                "Windows.UI.Input.InputSite.WindowClass",
                "Windows PowerShell"
            ),
            TerminalPassthrough::PsReadLine
        );
        assert_eq!(
            terminal_passthrough_for_window(
                "CASCADIA_HOSTING_WINDOW_CLASS",
                "Windows.UI.Input.InputSite.WindowClass",
                "PowerShell"
            ),
            TerminalPassthrough::PsReadLine
        );
        assert_eq!(
            terminal_passthrough_for_window(
                "CASCADIA_HOSTING_WINDOW_CLASS",
                "Windows.UI.Input.InputSite.WindowClass",
                "vpnuser"
            ),
            TerminalPassthrough::Ssh
        );
        assert_eq!(
            terminal_passthrough_for_window(
                "CASCADIA_HOSTING_WINDOW_CLASS",
                "Windows.UI.Input.InputSite.WindowClass",
                "PowerShell ssh user@example"
            ),
            TerminalPassthrough::Ssh
        );
        assert_eq!(
            terminal_passthrough_for_window(
                "CASCADIA_HOSTING_WINDOW_CLASS",
                "Windows.UI.Input.InputSite.WindowClass",
                ""
            ),
            TerminalPassthrough::UnknownTerminal
        );
    }

    #[test]
    fn context_capabilities_carry_method_binding() {
        let capabilities = Capabilities {
            can_replace_directly: true,
            can_read_selection: true,
            can_read_caret: true,
            method_binding: Some(MethodBinding::new(
                MethodId::Win32EditMessages,
                vec![MethodId::Win32EditMessages],
            )),
        };

        let binding = capabilities.method_binding.unwrap();
        assert_eq!(binding.context_method, MethodId::Win32EditMessages);
        assert_eq!(binding.replace_methods, vec![MethodId::Win32EditMessages]);
    }

    #[test]
    fn context_replacement_method_uses_first_bound_replace_method() {
        let context = TextContext {
            app_id: String::from("ConsoleWindowClass"),
            window_id: String::from("hwnd:1"),
            control_id: String::from("terminal-console:hwnd:1"),
            text_snapshot: String::from("пше"),
            caret_range: TextRange::caret("пше".len()),
            selection_range: None,
            capabilities: Capabilities {
                can_replace_directly: false,
                can_read_selection: false,
                can_read_caret: false,
                method_binding: Some(MethodBinding::new(
                    MethodId::ConsoleBuffer,
                    vec![MethodId::ConsoleBuffer],
                )),
            },
        };

        assert_eq!(
            context_replacement_method(&context),
            Some(MethodId::ConsoleBuffer)
        );
    }

    #[cfg(windows)]
    #[test]
    fn win32_edit_method_probes_supported_edit_controls() {
        let target = ForegroundTarget {
            app_class: String::from("Notepad"),
            focused_class: String::from("Edit"),
            title: String::new(),
            process_name: None,
            window_id: String::from("hwnd:1"),
            control_id: String::from("hwnd:2"),
        };

        let probe = Win32EditMessagesMethod.probe(&target).unwrap();

        assert_eq!(probe.method_id, MethodId::Win32EditMessages);
        assert_eq!(probe.safety, stepler_platform::ProbeSafety::Safe);
    }

    #[cfg(windows)]
    #[test]
    fn console_buffer_method_probes_classic_console() {
        let target = ForegroundTarget {
            app_class: String::from("ConsoleWindowClass"),
            focused_class: String::from("ConsoleWindowClass"),
            title: String::new(),
            process_name: None,
            window_id: String::from("hwnd:1"),
            control_id: String::from("hwnd:1"),
        };

        let probe = ConsoleBufferMethod.probe(&target).unwrap();

        assert_eq!(probe.method_id, MethodId::ConsoleBuffer);
        assert_eq!(probe.safety, stepler_platform::ProbeSafety::Safe);
    }

    #[cfg(windows)]
    #[test]
    fn terminal_clipboard_method_probes_windows_terminal_as_risky() {
        let target = ForegroundTarget {
            app_class: String::from("CASCADIA_HOSTING_WINDOW_CLASS"),
            focused_class: String::from("Windows.UI.Input.InputSite.WindowClass"),
            title: String::new(),
            process_name: None,
            window_id: String::from("hwnd:1"),
            control_id: String::from("hwnd:2"),
        };

        let probe = TerminalClipboardShortcutMethod.probe(&target).unwrap();

        assert_eq!(probe.method_id, MethodId::TerminalClipboardShortcut);
        assert_eq!(probe.safety, stepler_platform::ProbeSafety::Risky);
        assert!(probe.requires_clipboard);
    }

    #[cfg(windows)]
    #[test]
    fn ssh_terminal_method_probes_ssh_title_as_unsupported() {
        let target = ForegroundTarget {
            app_class: String::from("CASCADIA_HOSTING_WINDOW_CLASS"),
            focused_class: String::from("Windows.UI.Input.InputSite.WindowClass"),
            title: String::from("vpnuser"),
            process_name: None,
            window_id: String::from("hwnd:1"),
            control_id: String::from("hwnd:2"),
        };

        let probe = SshTerminalMethod.probe(&target).unwrap();

        assert_eq!(probe.method_id, MethodId::SshTerminal);
        assert_eq!(probe.safety, stepler_platform::ProbeSafety::Unsupported);
        assert!(!probe.requires_clipboard);
    }

    #[cfg(windows)]
    #[test]
    fn clipboard_selection_method_probes_unknown_controls_as_risky() {
        let target = ForegroundTarget {
            app_class: String::from("CustomAppWindow"),
            focused_class: String::from("CustomTextSurface"),
            title: String::new(),
            process_name: None,
            window_id: String::from("hwnd:1"),
            control_id: String::from("hwnd:2"),
        };

        let probe = ClipboardSelectionMethod.probe(&target).unwrap();

        assert_eq!(probe.method_id, MethodId::ClipboardSelection);
        assert_eq!(probe.safety, stepler_platform::ProbeSafety::Risky);
        assert!(probe.requires_clipboard);
    }

    #[cfg(windows)]
    #[test]
    fn send_input_method_probes_unknown_controls_as_risky_without_clipboard() {
        let target = ForegroundTarget {
            app_class: String::from("CustomAppWindow"),
            focused_class: String::from("CustomTextSurface"),
            title: String::new(),
            process_name: None,
            window_id: String::from("hwnd:1"),
            control_id: String::from("hwnd:2"),
        };

        let probe = SendInputMethod.probe(&target).unwrap();

        assert_eq!(probe.method_id, MethodId::SendInput);
        assert_eq!(probe.safety, stepler_platform::ProbeSafety::Risky);
        assert!(!probe.requires_clipboard);
    }

    #[cfg(windows)]
    #[test]
    fn uia_text_method_probes_unknown_non_special_controls() {
        let target = ForegroundTarget {
            app_class: String::from("ApplicationFrameWindow"),
            focused_class: String::from("Windows.UI.Core.CoreWindow"),
            title: String::from("Settings"),
            process_name: None,
            window_id: String::from("hwnd:1"),
            control_id: String::from("hwnd:2"),
        };

        let probe = UiAutomationTextMethod.probe(&target).unwrap();

        assert_eq!(probe.method_id, MethodId::UiAutomationText);
        assert_eq!(probe.safety, stepler_platform::ProbeSafety::Safe);
    }

    #[cfg(windows)]
    #[test]
    fn uia_editable_text_method_probes_browser_like_controls_as_strict_fallback() {
        let target = ForegroundTarget {
            app_class: String::from("Chrome_WidgetWin_1"),
            focused_class: String::from("Chrome_RenderWidgetHostHWND"),
            title: String::from("Confluence"),
            process_name: Some(String::from("chrome")),
            window_id: String::from("hwnd:1"),
            control_id: String::from("hwnd:2"),
        };

        let probe = UiAutomationEditableTextMethod.probe(&target).unwrap();

        assert_eq!(probe.method_id, MethodId::UiAutomationEditableText);
        assert_eq!(probe.safety, stepler_platform::ProbeSafety::Safe);
    }

    #[cfg(windows)]
    #[test]
    fn uia_document_text_method_probes_browser_like_controls() {
        let target = ForegroundTarget {
            app_class: String::from("MozillaWindowClass"),
            focused_class: String::from("MozillaWindowClass"),
            title: String::from("Confluence"),
            process_name: Some(String::from("firefox")),
            window_id: String::from("hwnd:1"),
            control_id: String::from("hwnd:2"),
        };

        let probe = UiAutomationDocumentTextMethod.probe(&target).unwrap();

        assert_eq!(probe.method_id, MethodId::UiAutomationDocumentText);
        assert_eq!(probe.safety, stepler_platform::ProbeSafety::Safe);
        assert!(!probe.requires_clipboard);
    }

    #[cfg(windows)]
    #[test]
    fn web_keyboard_selection_method_probes_browser_like_controls() {
        let target = ForegroundTarget {
            app_class: String::from("MozillaWindowClass"),
            focused_class: String::from("MozillaWindowClass"),
            title: String::from("Confluence"),
            process_name: Some(String::from("firefox")),
            window_id: String::from("hwnd:1"),
            control_id: String::from("hwnd:2"),
        };

        let probe = WebKeyboardSelectionMethod.probe(&target).unwrap();

        assert_eq!(probe.method_id, MethodId::WebKeyboardSelection);
        assert_eq!(probe.safety, stepler_platform::ProbeSafety::Safe);
        assert!(probe.requires_clipboard);
    }

    #[cfg(windows)]
    #[test]
    fn yandex_chrome_widget_is_browser_like() {
        let target = ForegroundTarget {
            app_class: String::from("Chrome_Yandex_WidgetWin_1"),
            focused_class: String::from("Chrome_Yandex_WidgetWin_1"),
            title: String::from("OneDrive"),
            process_name: Some(String::from("browser")),
            window_id: String::from("hwnd:1"),
            control_id: String::from("hwnd:2"),
        };

        assert!(is_browser_like_target(&target));
        assert!(WebKeyboardSelectionMethod.probe(&target).is_some());
        assert!(ClipboardSelectionMethod.probe(&target).is_none());
        assert!(SendInputMethod.probe(&target).is_none());
    }

    #[cfg(windows)]
    #[test]
    fn uia_text_method_does_not_probe_known_special_controls() {
        let edit = ForegroundTarget {
            app_class: String::from("Notepad"),
            focused_class: String::from("Edit"),
            title: String::new(),
            process_name: None,
            window_id: String::from("hwnd:1"),
            control_id: String::from("hwnd:2"),
        };
        let terminal = ForegroundTarget {
            app_class: String::from("CASCADIA_HOSTING_WINDOW_CLASS"),
            focused_class: String::from("Windows.UI.Input.InputSite.WindowClass"),
            title: String::new(),
            process_name: None,
            window_id: String::from("hwnd:1"),
            control_id: String::from("hwnd:2"),
        };

        assert!(UiAutomationTextMethod.probe(&edit).is_none());
        assert!(UiAutomationTextMethod.probe(&terminal).is_none());
    }

    #[cfg(windows)]
    #[test]
    fn generic_risky_methods_do_not_probe_browser_like_controls() {
        let target = ForegroundTarget {
            app_class: String::from("Chrome_WidgetWin_1"),
            focused_class: String::from("Chrome_RenderWidgetHostHWND"),
            title: String::from("Confluence"),
            process_name: Some(String::from("chrome")),
            window_id: String::from("hwnd:1"),
            control_id: String::from("hwnd:2"),
        };

        assert!(UiAutomationTextMethod.probe(&target).is_none());
        assert!(ClipboardSelectionMethod.probe(&target).is_none());
        assert!(SendInputMethod.probe(&target).is_none());
    }

    #[cfg(windows)]
    #[test]
    fn browser_like_document_text_selection_is_safe_but_caret_fallback_is_blocked() {
        let target = ForegroundTarget {
            app_class: String::from("Chrome_WidgetWin_1"),
            focused_class: String::from("Chrome_WidgetWin_1"),
            title: String::from("Browser-like editor"),
            process_name: Some(String::from("chrome")),
            window_id: String::from("hwnd:1"),
            control_id: String::from("hwnd:2"),
        };

        assert_eq!(
            UiAutomationDocumentTextMethod
                .probe(&target)
                .map(|probe| probe.method_id),
            Some(MethodId::UiAutomationDocumentText)
        );
        assert!(!allow_uia_document_caret_fallback(&target));
        assert!(UiAutomationTextMethod.probe(&target).is_none());
        assert!(ClipboardSelectionMethod.probe(&target).is_none());
        assert!(SendInputMethod.probe(&target).is_none());
    }

    #[cfg(windows)]
    #[test]
    fn word_com_method_probes_word_windows() {
        let target = ForegroundTarget {
            app_class: String::from("OpusApp"),
            focused_class: String::from("_WwG"),
            title: String::from("Document1 - Word"),
            process_name: Some(String::from("WINWORD")),
            window_id: String::from("hwnd:1"),
            control_id: String::from("hwnd:2"),
        };

        let probe = WordComMethod.probe(&target).unwrap();

        assert_eq!(probe.method_id, MethodId::WordCom);
        assert_eq!(probe.safety, stepler_platform::ProbeSafety::Safe);
        assert!(!probe.requires_clipboard);
    }

    #[cfg(windows)]
    #[test]
    fn word_com_method_probes_outlook_word_editor_windows() {
        let target = ForegroundTarget {
            app_class: String::from("rctrl_renwnd32"),
            focused_class: String::from("_WwG"),
            title: String::from("Untitled - Message"),
            process_name: Some(String::from("OUTLOOK")),
            window_id: String::from("hwnd:1"),
            control_id: String::from("hwnd:2"),
        };

        let probe = WordComMethod.probe(&target).unwrap();

        assert_eq!(probe.method_id, MethodId::WordCom);
        assert_eq!(probe.safety, stepler_platform::ProbeSafety::Safe);
        assert!(!probe.requires_clipboard);
    }

    #[test]
    fn word_com_control_id_carries_absolute_base() {
        assert_eq!(parse_word_com_base("word-com:42:hwnd:ABC"), Some(42));
        assert_eq!(
            parse_word_com_base("outlook-word-com:42:hwnd:ABC"),
            Some(42)
        );
        assert_eq!(parse_word_com_base("word-com:nope:hwnd:ABC"), None);
    }

    #[test]
    fn utf16le_base64_round_trips_word_text() {
        let encoded = encode_utf16le_base64("любовь");

        assert_eq!(encoded, "OwROBDEEPgQyBEwE");
        assert_eq!(decode_utf16le_base64(&encoded).unwrap(), "любовь");
    }

    #[test]
    fn unsupported_control_error_keeps_diagnostic_classes() {
        let error = PlatformError::UnsupportedControl {
            app_class: String::from("Chrome_WidgetWin_1"),
            focused_class: String::from("Chrome_RenderWidgetHostHWND"),
        };

        assert_eq!(
            format!("{error:?}"),
            "UnsupportedControl { app_class: \"Chrome_WidgetWin_1\", focused_class: \"Chrome_RenderWidgetHostHWND\" }"
        );
    }

    #[test]
    fn slice_by_range_uses_byte_offsets_and_checks_boundaries() {
        let text = "привет мир";

        assert_eq!(
            slice_by_range(text, TextRange::new(0, "привет".len())),
            Some("привет")
        );
        assert_eq!(slice_by_range(text, TextRange::new(1, 3)), None);
    }

    #[test]
    fn replace_range_text_preserves_terminal_prefix() {
        let text = "echo ghbdtn vbh";
        let start = text.find("ghbdtn").unwrap();
        let end = text.len();

        assert_eq!(
            replace_range_text(text, TextRange::new(start, end), "привет мир"),
            Some(String::from("echo привет мир"))
        );
    }

    #[test]
    fn console_prompt_line_parser_extracts_input() {
        assert_eq!(
            console_input_from_prompt_line("PS C:\\Users\\alexey.andreev> пше      "),
            "пше"
        );
        assert_eq!(console_input_from_prompt_line("ghbdtn vbh"), "ghbdtn vbh");
    }

    #[test]
    fn converts_offsets_between_utf16_and_utf8_boundaries() {
        let text = "a привет 🌍";
        let byte_offset = "a привет".len();
        let edit_offset = byte_offset_to_edit_offset(text, byte_offset).unwrap();

        assert_eq!(
            edit_offset_to_byte_offset(text, edit_offset),
            Some(byte_offset)
        );
        assert_eq!(edit_offset_to_byte_offset(text, 999), None);
        assert_eq!(byte_offset_to_edit_offset(text, 3), None);
    }

    #[test]
    fn converts_edit_offsets_with_crlf_counted_as_one_position() {
        let text = "one\r\ntwo\r\nвальс поле long ghbdtn vbh";
        let ghbdtn_start = text.find("ghbdtn").unwrap();
        let ghbdtn_end = ghbdtn_start + "ghbdtn".len();

        let edit_start = byte_offset_to_edit_offset(text, ghbdtn_start).unwrap();
        let edit_end = byte_offset_to_edit_offset(text, ghbdtn_end).unwrap();

        assert!(edit_start < ghbdtn_start);
        assert!(edit_end < ghbdtn_end);
        assert_eq!(
            edit_offset_to_byte_offset(text, edit_start),
            Some(ghbdtn_start)
        );
        assert_eq!(edit_offset_to_byte_offset(text, edit_end), Some(ghbdtn_end));
        assert_eq!(
            byte_offset_to_edit_offset(text, text.find('\n').unwrap()),
            None
        );
    }

    #[test]
    fn clipboard_wide_string_round_trips_with_nul_terminator() {
        let wide = string_to_null_terminated_utf16("тест");

        assert_eq!(wide.last(), Some(&0));
        assert_eq!(utf16_until_nul_to_string(&wide), "тест");
    }

    #[test]
    fn global_memory_bytes_round_up_to_even_utf16_size() {
        let wide = string_to_null_terminated_utf16("ab");
        let bytes = utf16_to_le_bytes(&wide);

        assert_eq!(bytes.len(), wide.len() * 2);
        assert_eq!(le_bytes_to_utf16(&bytes), wide);
    }

    #[test]
    fn clipboard_snapshot_can_hold_multiple_formats() {
        let snapshot = ClipboardSnapshot {
            text: Some(String::from("hello")),
            sequence_number: Some(42),
            formats: vec![
                ClipboardFormatSnapshot {
                    format: 1,
                    bytes: vec![1, 2, 3],
                },
                ClipboardFormatSnapshot {
                    format: CF_UNICODETEXT,
                    bytes: utf16_to_le_bytes(&string_to_null_terminated_utf16("hello")),
                },
            ],
        };

        assert_eq!(snapshot.formats.len(), 2);
        assert_eq!(snapshot.text.as_deref(), Some("hello"));
    }

    #[test]
    fn keyboard_control_action_message_ids_round_trip() {
        for action in [
            KeyboardControlAction::SwitchToRussian,
            KeyboardControlAction::SwitchToEnglish,
            KeyboardControlAction::SwitchToNext,
        ] {
            assert_eq!(
                KeyboardControlAction::from_message_id(action.message_id()),
                Some(action)
            );
        }
        assert_eq!(KeyboardControlAction::from_message_id(99), None);
    }

    #[test]
    fn keyboard_control_state_switches_only_on_single_ctrl() {
        let mut state = KeyboardControlHookState::default();

        assert_eq!(state.handle_key(VK_LCONTROL, true, false), None);
        assert_eq!(
            state.handle_key(VK_LCONTROL, false, true),
            Some(KeyboardControlAction::SwitchToRussian)
        );

        assert_eq!(state.handle_key(VK_RCONTROL, true, false), None);
        assert_eq!(state.handle_key(0x43, true, false), None);
        assert_eq!(state.handle_key(VK_RCONTROL, false, true), None);
    }

    #[test]
    fn keyboard_control_state_ignores_layout_controls_during_win_combo() {
        let mut state = KeyboardControlHookState::default();

        assert_eq!(state.handle_key(VK_LWIN, true, false), None);
        assert_eq!(state.handle_key(VK_LCONTROL, true, false), None);
        assert_eq!(state.handle_key(VK_LCONTROL, false, true), None);
        assert_eq!(state.handle_key(VK_LWIN, false, true), None);
        assert_eq!(state.handle_key(VK_LCONTROL, true, false), None);
        assert_eq!(state.handle_key(VK_LCONTROL, false, true), None);

        state.suspend_layout_controls_until = Some(Instant::now() - Duration::from_millis(1));
        assert_eq!(state.handle_key(VK_LCONTROL, true, false), None);
        assert_eq!(
            state.handle_key(VK_LCONTROL, false, true),
            Some(KeyboardControlAction::SwitchToRussian)
        );
    }

    #[test]
    fn keyboard_control_state_recovers_from_missing_win_key_up() {
        let mut state = KeyboardControlHookState::default();

        assert_eq!(state.handle_key(VK_LWIN, true, false), None);
        state.suspend_layout_controls_until = Some(Instant::now() - Duration::from_millis(1));
        assert_eq!(state.handle_key(VK_LCONTROL, true, false), None);
        assert_eq!(
            state.handle_key(VK_LCONTROL, false, true),
            Some(KeyboardControlAction::SwitchToRussian)
        );
    }

    #[test]
    fn keyboard_control_state_emits_correction_hotkey_once_until_key_up() {
        let mut state = KeyboardControlHookState::default();

        assert_eq!(
            state.handle_correction_hotkey(VK_PAUSE, true, false),
            Some(stepler_core::CorrectionMode::Pause)
        );
        assert_eq!(state.handle_correction_hotkey(VK_PAUSE, false, true), None);
        let mut state = KeyboardControlHookState::default();
        assert_eq!(
            state.handle_correction_hotkey(VK_CANCEL, true, false),
            Some(stepler_core::CorrectionMode::Pause)
        );
    }

    #[test]
    fn keyboard_control_state_maps_ctrl_pause_to_scrolllock_mode() {
        let mut state = KeyboardControlHookState::default();

        assert_eq!(state.handle_key(VK_LCONTROL, true, false), None);
        assert_eq!(state.handle_correction_hotkey(VK_PAUSE, true, false), None);
        assert_eq!(state.handle_correction_hotkey(VK_PAUSE, false, true), None);
        assert_eq!(state.handle_key(VK_LCONTROL, false, true), None);
        assert_eq!(
            state.take_pending_scroll_lock_if_released(),
            Some(stepler_core::CorrectionMode::ScrollLock)
        );

        let mut state = KeyboardControlHookState::default();
        assert_eq!(state.handle_key(VK_RCONTROL, true, false), None);
        assert_eq!(state.handle_correction_hotkey(VK_CANCEL, true, false), None);
        assert_eq!(state.handle_correction_hotkey(VK_CANCEL, false, true), None);
        assert_eq!(state.handle_key(VK_RCONTROL, false, true), None);
        assert_eq!(
            state.take_pending_scroll_lock_if_released(),
            Some(stepler_core::CorrectionMode::ScrollLock)
        );
    }

    #[test]
    fn keyboard_control_state_marks_ctrl_pause_as_used_when_terminal_handles_it() {
        let mut state = KeyboardControlHookState::default();

        assert_eq!(state.handle_key(VK_LCONTROL, true, false), None);
        assert_eq!(
            state.handle_terminal_pause_key(VK_PAUSE, true, false),
            TerminalPauseHandling::TranslateToCtrlF12
        );
        assert_eq!(
            state.handle_terminal_pause_key(VK_PAUSE, false, true),
            TerminalPauseHandling::Suppress
        );
        assert_eq!(state.handle_key(VK_LCONTROL, false, true), None);

        let mut state = KeyboardControlHookState::default();
        assert_eq!(state.handle_key(VK_RCONTROL, true, false), None);
        assert_eq!(
            state.handle_terminal_pause_key(VK_CANCEL, true, false),
            TerminalPauseHandling::TranslateToCtrlF12
        );
        assert_eq!(
            state.handle_terminal_pause_key(VK_CANCEL, false, true),
            TerminalPauseHandling::Suppress
        );
        assert_eq!(state.handle_key(VK_RCONTROL, false, true), None);
    }

    #[test]
    fn keyboard_control_state_maps_classic_console_ctrl_pause_immediately() {
        let mut state = KeyboardControlHookState::default();

        assert_eq!(state.handle_key(VK_LCONTROL, true, false), None);
        assert_eq!(
            state.handle_classic_console_pause_key(VK_CANCEL, true, false),
            Some(stepler_core::CorrectionMode::ScrollLock)
        );
        assert_eq!(
            state.handle_classic_console_pause_key(VK_CANCEL, false, true),
            None
        );
        assert_eq!(state.handle_key(VK_LCONTROL, false, true), None);
        assert_eq!(state.take_pending_scroll_lock_if_released(), None);
    }

    #[test]
    fn keyboard_control_state_maps_classic_console_plain_pause_immediately() {
        let mut state = KeyboardControlHookState::default();

        assert_eq!(
            state.handle_classic_console_pause_key(VK_PAUSE, true, false),
            Some(stepler_core::CorrectionMode::Pause)
        );
        assert_eq!(
            state.handle_classic_console_pause_key(VK_PAUSE, false, true),
            None
        );
    }

    #[test]
    fn keyboard_control_state_passes_plain_terminal_pause_through() {
        let mut state = KeyboardControlHookState::default();

        assert_eq!(
            state.handle_terminal_pause_key(VK_PAUSE, true, false),
            TerminalPauseHandling::PassThrough
        );
        assert_eq!(
            state.handle_terminal_pause_key(VK_PAUSE, false, true),
            TerminalPauseHandling::PassThrough
        );
    }

    #[test]
    fn keyboard_control_state_suppresses_scrolllock_companion_c_briefly() {
        let mut state = KeyboardControlHookState::default();

        assert_eq!(state.handle_key(VK_LCONTROL, true, false), None);
        assert_eq!(state.handle_correction_hotkey(VK_PAUSE, true, false), None);
        assert!(state.should_suppress_companion_key(VK_C));
        assert!(state.should_suppress_companion_key(VK_C));
        assert!(state.should_suppress_companion_key(VK_HOME));
        assert!(state.should_suppress_companion_key(VK_RIGHT));
        assert!(!state.should_suppress_companion_key(VK_LCONTROL));
    }

    #[cfg(windows)]
    #[test]
    fn injected_keyboard_events_are_not_interpreted_as_user_controls() {
        let event = KbdLlHookStruct {
            vk_code: VK_CONTROL,
            flags: LLKHF_INJECTED,
            ..KbdLlHookStruct::default()
        };

        assert!(should_ignore_keyboard_hook_event(event));
    }

    #[cfg(windows)]
    #[test]
    fn send_input_keyboard_struct_has_windows_x64_size() {
        assert_eq!(std::mem::size_of::<Input>(), 40);
    }

    #[cfg(windows)]
    #[test]
    fn send_input_uses_scan_codes_for_keyboard_events() {
        let input = Input::keyboard_scan_code(VK_C, false, 0);
        let keyboard = unsafe { input.input.ki };

        assert_eq!(keyboard.vk, 0);
        assert_ne!(keyboard.scan, 0);
        assert_eq!(keyboard.flags & KEYEVENTF_SCANCODE, KEYEVENTF_SCANCODE);
    }

    #[cfg(windows)]
    #[test]
    fn send_input_can_use_virtual_keys_for_terminal_shortcuts() {
        let input = Input::keyboard_virtual_key(VK_C, false, 0);
        let keyboard = unsafe { input.input.ki };

        assert_eq!(keyboard.vk, VK_C as u16);
        assert_eq!(keyboard.scan, 0);
        assert_eq!(keyboard.flags & KEYEVENTF_SCANCODE, 0);
    }

    #[cfg(windows)]
    #[test]
    fn send_input_can_emit_unicode_units() {
        let input = Input::keyboard_unicode('я' as u16, false);
        let keyboard = unsafe { input.input.ki };

        assert_eq!(keyboard.vk, 0);
        assert_eq!(keyboard.scan, 'я' as u16);
        assert_eq!(keyboard.flags & KEYEVENTF_UNICODE, KEYEVENTF_UNICODE);
    }

    #[test]
    fn hotkeyhandler_markers_are_not_user_selection_text() {
        assert!(looks_like_hotkeyhandler_marker(
            "__HKH_SELECTED_MARKER_cc58bf97238446389051bab0525c89da__"
        ));
        assert!(looks_like_hotkeyhandler_marker(
            "__HKH_LEFT_TEXT_MARKER_cc58bf97238446389051bab0525c89da__"
        ));
        assert!(!looks_like_hotkeyhandler_marker("k.,jdm"));
    }

    #[cfg(windows)]
    #[test]
    fn send_input_marks_navigation_keys_as_extended() {
        for vk in [VK_HOME, VK_END, VK_INSERT] {
            let input = Input::keyboard_scan_code(vk, false, 0);
            let keyboard = unsafe { input.input.ki };

            assert_eq!(
                keyboard.flags & KEYEVENTF_EXTENDEDKEY,
                KEYEVENTF_EXTENDEDKEY,
                "vk=0x{vk:X} must not be sent as a numpad key"
            );
        }
    }
}

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
fn focus_diagnostics_impl() -> Result<WindowsFocusDiagnostics, PlatformError> {
    let foreground = foreground_hwnd()?;
    let focused = focused_window(foreground).unwrap_or(foreground);
    Ok(WindowsFocusDiagnostics {
        foreground_hwnd: hwnd_id(foreground),
        foreground_class: window_class_name(foreground).unwrap_or_else(|| String::from("unknown")),
        foreground_title: window_title(foreground).unwrap_or_default(),
        focused_hwnd: hwnd_id(focused),
        focused_class: window_class_name(focused).unwrap_or_else(|| String::from("unknown")),
        focused_title: window_title(focused).unwrap_or_default(),
    })
}

#[cfg(windows)]
fn method_diagnostics_impl() -> Result<WindowsMethodDiagnostics, PlatformError> {
    let foreground = foreground_hwnd()?;
    let focused = focused_window(foreground).unwrap_or(foreground);
    let app_class = window_class_name(foreground).unwrap_or_else(|| String::from("unknown"));
    let focused_class = window_class_name(focused).unwrap_or_else(|| String::from("unknown"));
    let target = ForegroundTarget {
        app_class: app_class.clone(),
        focused_class: focused_class.clone(),
        title: window_title(foreground).unwrap_or_default(),
        process_name: window_process_name(foreground),
        window_id: hwnd_id(foreground),
        control_id: hwnd_id(focused),
    };
    let probes = windows_method_probes(&target);
    let decision = MethodResolver::default().resolve(&target, &probes).ok();
    let run_context = std::env::var("STEPLER_DIAGNOSE_CONTEXT")
        .map(|value| value == "1")
        .unwrap_or(false);
    let (context_method, context_error, context_skipped) = if run_context {
        let context = text_context();
        match context {
            Ok(context) => (
                context
                    .capabilities
                    .method_binding
                    .as_ref()
                    .map(|binding| binding.context_method.as_str().to_owned()),
                None,
                false,
            ),
            Err(error) => (None, Some(format!("{error:?}")), false),
        }
    } else {
        (None, None, true)
    };

    Ok(WindowsMethodDiagnostics {
        foreground: WindowsFocusDiagnostics {
            foreground_hwnd: hwnd_id(foreground),
            foreground_class: app_class,
            foreground_title: target.title,
            focused_hwnd: hwnd_id(focused),
            focused_class,
            focused_title: window_title(focused).unwrap_or_default(),
        },
        uia_focus: uia_focus_diagnostics().ok(),
        probes: probes
            .into_iter()
            .map(|probe| WindowsMethodProbeDiagnostics {
                method: probe.method_id.as_str().to_owned(),
                safety: format!("{:?}", probe.safety),
                requires_clipboard: probe.requires_clipboard,
                requires_focus_stability: probe.requires_focus_stability,
                can_preflight: probe.can_preflight,
                can_verify: probe.can_verify,
                reason: probe.reason,
            })
            .collect(),
        selected_context_method: decision
            .as_ref()
            .map(|decision| decision.context_method.as_str().to_owned()),
        selected_replacement_method: decision
            .as_ref()
            .map(|decision| decision.replacement_method.as_str().to_owned()),
        context_method,
        context_error,
        context_skipped,
    })
}

#[cfg(not(windows))]
fn focus_diagnostics_impl() -> Result<WindowsFocusDiagnostics, PlatformError> {
    Err(PlatformError::Unsupported)
}

#[cfg(not(windows))]
fn method_diagnostics_impl() -> Result<WindowsMethodDiagnostics, PlatformError> {
    Err(PlatformError::Unsupported)
}

#[cfg(windows)]
fn uia_focus_diagnostics() -> Result<WindowsUiaFocusDiagnostics, PlatformError> {
    let output = run_powershell_script(UIA_FOCUS_DIAGNOSTICS_SCRIPT, &[])?;
    let fields = parse_key_value_lines(&output);
    if fields.get("ok").map(String::as_str) != Some("1") {
        return Err(PlatformError::ReplacementUnavailable);
    }

    Ok(WindowsUiaFocusDiagnostics {
        name: fields.get("name").cloned().unwrap_or_default(),
        control_type: fields.get("control_type").cloned().unwrap_or_default(),
        automation_id: fields.get("automation_id").cloned().unwrap_or_default(),
        class_name: fields.get("class_name").cloned().unwrap_or_default(),
        framework_id: fields.get("framework_id").cloned().unwrap_or_default(),
        has_keyboard_focus: fields
            .get("has_keyboard_focus")
            .map(String::as_str)
            .is_some_and(|value| value == "1"),
        is_keyboard_focusable: fields
            .get("is_keyboard_focusable")
            .map(String::as_str)
            .is_some_and(|value| value == "1"),
    })
}

#[cfg(windows)]
fn text_context() -> Result<TextContext, PlatformError> {
    let foreground = foreground_hwnd()?;

    let focused = focused_window(foreground).unwrap_or(foreground);
    let app_class = window_class_name(foreground).unwrap_or_else(|| String::from("unknown"));
    let focused_class = window_class_name(focused).unwrap_or_else(|| String::from("unknown"));
    let target = ForegroundTarget {
        app_class: app_class.clone(),
        focused_class: focused_class.clone(),
        title: window_title(foreground).unwrap_or_default(),
        process_name: window_process_name(foreground),
        window_id: hwnd_id(foreground),
        control_id: hwnd_id(focused),
    };
    let probes = windows_method_probes(&target);
    let resolver = MethodResolver::default();
    let mut remaining = probes;
    let mut last_unavailable = None;
    while !remaining.is_empty() {
        let decision = match resolver.resolve(&target, &remaining) {
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
        MethodId::Win32EditMessages => {
            Win32EditMessagesMethod.capture(foreground, focused, app_class.to_owned())
        }
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
    let mut probes = Vec::new();
    if let Some(probe) = Win32EditMessagesMethod.probe(target) {
        probes.push(probe);
    }
    if let Some(probe) = ConsoleBufferMethod.probe(target) {
        probes.push(probe);
    }
    if let Some(probe) = SshTerminalMethod.probe(target) {
        probes.push(probe);
    }
    if let Some(probe) = WordComMethod.probe(target) {
        probes.push(probe);
    } else if let Some(probe) = TerminalClipboardShortcutMethod.probe(target) {
        probes.push(probe);
    }
    if let Some(probe) = UiAutomationEditableTextMethod.probe(target) {
        probes.push(probe);
    }
    if let Some(probe) = UiAutomationDocumentTextMethod.probe(target) {
        probes.push(probe);
    }
    if let Some(probe) = UiAutomationTextMethod.probe(target) {
        probes.push(probe);
    }
    if let Some(probe) = WebKeyboardSelectionMethod.probe(target) {
        probes.push(probe);
    }
    if let Some(probe) = ClipboardSelectionMethod.probe(target) {
        probes.push(probe);
    }
    if let Some(probe) = SendInputMethod.probe(target) {
        probes.push(probe);
    }
    probes
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
        Some(MethodId::WebKeyboardSelection) => WebKeyboardSelectionMethod.apply(context, plan),
        Some(MethodId::ClipboardSelection) => ClipboardSelectionMethod.apply(context, plan),
        Some(MethodId::SendInput) => SendInputMethod.apply(context, plan),
        Some(_) => Err(PlatformError::ReplacementUnavailable),
        None if context.control_id.starts_with("terminal-console:") => {
            ConsoleBufferMethod.apply(context, plan)
        }
        None if context.control_id.starts_with("terminal:") => {
            TerminalClipboardShortcutMethod.apply(context, plan)
        }
        None => Win32EditMessagesMethod.apply(context, plan),
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

#[cfg(windows)]
fn foreground_hwnd() -> Result<isize, PlatformError> {
    if let Some(hwnd) = test_foreground_hwnd_override() {
        return Ok(hwnd);
    }

    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd == 0 {
        return Err(PlatformError::ForegroundUnavailable);
    }
    Ok(hwnd)
}

#[cfg(windows)]
fn test_foreground_hwnd_override() -> Option<isize> {
    let value = std::env::var("STEPLER_TEST_FOREGROUND_HWND").ok()?;
    let value = value.trim();
    let hex = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"));
    match hex {
        Some(hex) => isize::from_str_radix(hex, 16).ok(),
        None => value.parse::<isize>().ok(),
    }
}

#[cfg(windows)]
fn window_thread_id(hwnd: isize) -> Result<u32, PlatformError> {
    let thread_id = unsafe { GetWindowThreadProcessId(hwnd, std::ptr::null_mut()) };
    if thread_id == 0 {
        return Err(PlatformError::ForegroundUnavailable);
    }
    Ok(thread_id)
}

#[cfg(windows)]
fn window_class_name(hwnd: isize) -> Option<String> {
    let mut buffer = [0u16; 256];
    let length = unsafe { GetClassNameW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32) };
    if length <= 0 {
        return None;
    }

    Some(String::from_utf16_lossy(&buffer[..length as usize]))
}

#[cfg(windows)]
fn window_title(hwnd: isize) -> Option<String> {
    let length = unsafe { GetWindowTextLengthW(hwnd) };
    if length <= 0 {
        return None;
    }

    let mut buffer = vec![0u16; length as usize + 1];
    let copied = unsafe { GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32) };
    if copied <= 0 {
        return None;
    }

    Some(String::from_utf16_lossy(&buffer[..copied as usize]))
}

#[cfg(windows)]
fn window_process_name(hwnd: isize) -> Option<String> {
    let mut process_id = 0u32;
    unsafe {
        GetWindowThreadProcessId(hwnd, &mut process_id as *mut u32);
    }
    if process_id == 0 {
        return None;
    }

    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process_id) };
    if process == 0 {
        return None;
    }

    let mut buffer = vec![0u16; 32768];
    let mut size = buffer.len() as u32;
    let ok = unsafe { QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut size) };
    unsafe {
        CloseHandle(process);
    }
    if ok == 0 || size == 0 {
        return None;
    }

    let path = String::from_utf16_lossy(&buffer[..size as usize]);
    std::path::Path::new(&path)
        .file_stem()
        .map(|name| name.to_string_lossy().into_owned())
}

fn hwnd_id(hwnd: isize) -> String {
    format!("hwnd:{hwnd:X}")
}

fn is_supported_edit_class(class_name: &str) -> bool {
    let class_name = class_name.to_ascii_lowercase();
    class_name == "edit" || class_name.starts_with("richedit")
}

fn is_supported_terminal_class(app_class: &str, focused_class: &str) -> bool {
    app_class == "CASCADIA_HOSTING_WINDOW_CLASS"
        || app_class == "ConsoleWindowClass"
        || focused_class == "Windows.UI.Input.InputSite.WindowClass"
}

fn is_classic_console_class(app_class: &str, focused_class: &str) -> bool {
    app_class.eq_ignore_ascii_case("ConsoleWindowClass")
        && focused_class.eq_ignore_ascii_case("ConsoleWindowClass")
}

#[cfg(windows)]
fn foreground_is_classic_console() -> bool {
    let Ok(foreground) = foreground_hwnd() else {
        return false;
    };
    let focused = focused_window(foreground).unwrap_or(foreground);
    let app_class = window_class_name(foreground).unwrap_or_default();
    let focused_class = window_class_name(focused).unwrap_or_default();
    is_classic_console_class(&app_class, &focused_class)
}

#[cfg(windows)]
fn foreground_terminal_passthrough() -> TerminalPassthrough {
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

fn terminal_passthrough_for_window(
    app_class: &str,
    focused_class: &str,
    title: &str,
) -> TerminalPassthrough {
    if is_cmd_terminal_title(&title) {
        return TerminalPassthrough::None;
    }
    if is_ssh_terminal_title(&title) {
        return TerminalPassthrough::Ssh;
    }
    if is_psreadline_passthrough_terminal_class(&app_class, &focused_class) {
        if is_local_psreadline_terminal_title(&title) {
            return TerminalPassthrough::PsReadLine;
        }
        return TerminalPassthrough::UnknownTerminal;
    }
    TerminalPassthrough::None
}

#[cfg(windows)]
fn terminal_needs_conservative_suppression() -> bool {
    let Ok(foreground) = foreground_hwnd() else {
        return false;
    };
    let focused = focused_window(foreground).unwrap_or(foreground);
    let app_class = window_class_name(foreground).unwrap_or_default();
    let focused_class = window_class_name(focused).unwrap_or_default();
    terminal_class_needs_conservative_suppression(&app_class, &focused_class)
}

fn terminal_class_needs_conservative_suppression(app_class: &str, focused_class: &str) -> bool {
    is_supported_terminal_class(app_class, focused_class)
        && !app_class.eq_ignore_ascii_case("ConsoleWindowClass")
        && !focused_class.eq_ignore_ascii_case("ConsoleWindowClass")
}

fn is_psreadline_passthrough_terminal_class(app_class: &str, focused_class: &str) -> bool {
    app_class == "CASCADIA_HOSTING_WINDOW_CLASS"
        || focused_class == "Windows.UI.Input.InputSite.WindowClass"
}

fn is_ssh_terminal_target(target: &ForegroundTarget) -> bool {
    is_psreadline_passthrough_terminal_class(&target.app_class, &target.focused_class)
        && is_ssh_terminal_title(&target.title)
}

fn is_cmd_terminal_title(title: &str) -> bool {
    title.to_ascii_lowercase().contains("cmd.exe")
}

fn is_local_psreadline_terminal_title(title: &str) -> bool {
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

fn is_ssh_terminal_title(title: &str) -> bool {
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

fn is_word_target(target: &ForegroundTarget) -> bool {
    target
        .process_name
        .as_deref()
        .is_some_and(|process| process.eq_ignore_ascii_case("WINWORD"))
        || target.app_class.eq_ignore_ascii_case("OpusApp")
        || target.focused_class.eq_ignore_ascii_case("_WwG")
        || target.title.to_ascii_lowercase().contains("word")
}

fn is_outlook_target(target: &ForegroundTarget) -> bool {
    is_outlook_class_or_process(
        &target.app_class,
        &target.focused_class,
        target.process_name.as_deref(),
    )
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

fn is_browser_like_target(target: &ForegroundTarget) -> bool {
    is_browser_like_class_or_process(
        &target.app_class,
        &target.focused_class,
        target.process_name.as_deref(),
    )
}

fn is_notepad_like_target(target: &ForegroundTarget) -> bool {
    let app_class = target.app_class.to_ascii_lowercase();
    let focused_class = target.focused_class.to_ascii_lowercase();
    let process_name = target
        .process_name
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();

    process_name == "notepad" || app_class.contains("notepad") || focused_class.contains("notepad")
}

fn allow_uia_document_caret_fallback(target: &ForegroundTarget) -> bool {
    let _ = target;
    false
}

fn is_browser_like_class_or_process(
    app_class: &str,
    focused_class: &str,
    process_name: Option<&str>,
) -> bool {
    let app_class = app_class.to_ascii_lowercase();
    let focused_class = focused_class.to_ascii_lowercase();
    let process_name = process_name.unwrap_or_default().to_ascii_lowercase();

    app_class.starts_with("chrome_widgetwin")
        || app_class.starts_with("chrome_yandex_widgetwin")
        || focused_class.starts_with("chrome_widgetwin")
        || focused_class.starts_with("chrome_yandex_widgetwin")
        || app_class == "mozillawindowclass"
        || focused_class == "mozillawindowclass"
        || matches!(
            process_name.as_str(),
            "chrome" | "browser" | "msedge" | "firefox" | "codex" | "code" | "windsurf"
        )
}

fn active_correction_mode_is_scrolllock() -> bool {
    std::env::var("STEPLER_ACTIVE_CORRECTION_MODE")
        .map(|value| value.eq_ignore_ascii_case("scrolllock"))
        .unwrap_or(false)
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

fn parse_key_value_lines(output: &str) -> std::collections::HashMap<String, String> {
    output
        .lines()
        .filter_map(|line| {
            let (key, value) = line.split_once('=')?;
            Some((key.trim().to_owned(), value.trim().to_owned()))
        })
        .collect()
}

fn byte_offset_to_utf16(text: &str, byte_offset: usize) -> usize {
    if byte_offset >= text.len() {
        return text.encode_utf16().count();
    }
    text[..byte_offset].encode_utf16().count()
}

fn encode_utf16le_base64(value: &str) -> String {
    let bytes = value
        .encode_utf16()
        .flat_map(|unit| unit.to_le_bytes())
        .collect::<Vec<_>>();
    encode_base64(&bytes)
}

fn decode_utf16le_base64(value: &str) -> Result<String, String> {
    let bytes = decode_base64(value)?;
    if bytes.len() % 2 != 0 {
        return Err(String::from("decoded UTF-16LE byte length is odd"));
    }
    let units = bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect::<Vec<_>>();
    String::from_utf16(&units).map_err(|error| format!("invalid UTF-16LE text: {error}"))
}

fn encode_base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);
        let triple = ((b0 as u32) << 16) | ((b1 as u32) << 8) | b2 as u32;
        output.push(TABLE[((triple >> 18) & 0x3F) as usize] as char);
        output.push(TABLE[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() >= 2 {
            output.push(TABLE[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            output.push('=');
        }
        if chunk.len() == 3 {
            output.push(TABLE[(triple & 0x3F) as usize] as char);
        } else {
            output.push('=');
        }
    }
    output
}

fn decode_base64(value: &str) -> Result<Vec<u8>, String> {
    let mut buffer = Vec::new();
    let mut accumulator = 0u32;
    let mut bits = 0u8;
    let mut padding_seen = false;

    for byte in value.bytes().filter(|byte| !byte.is_ascii_whitespace()) {
        if byte == b'=' {
            padding_seen = true;
            continue;
        }
        if padding_seen {
            return Err(String::from("non-padding base64 byte after padding"));
        }
        let Some(value) = base64_value(byte) else {
            return Err(format!("invalid base64 byte 0x{byte:02X}"));
        };
        accumulator = (accumulator << 6) | value as u32;
        bits += 6;
        while bits >= 8 {
            bits -= 8;
            buffer.push(((accumulator >> bits) & 0xFF) as u8);
        }
    }

    Ok(buffer)
}

fn base64_value(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

#[cfg(windows)]
const WORD_CAPTURE_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
function ConvertTo-B64([string] $Text) {
    [Convert]::ToBase64String([System.Text.Encoding]::Unicode.GetBytes($Text))
}
function Strip-WordRangeMarkers([string] $Text) {
    if ($null -eq $Text) { return '' }
    $Text.TrimEnd([char]13, [char]7)
}
$word = [Runtime.InteropServices.Marshal]::GetActiveObject('Word.Application')
$word.Activate()
try {
    $selection = $word.ActiveWindow.Selection
} catch {
    $selection = $word.Selection
}
$document = $word.ActiveDocument
$selectionStart = [int] $selection.Start
$selectionEnd = [int] $selection.End

if ($selectionStart -ne $selectionEnd) {
    $range = $document.Range($selectionStart, $selectionEnd)
    $text = Strip-WordRangeMarkers ([string] $range.Text)
    'ok=1'
    'kind=selection'
    'base=' + $selectionStart
    'text_b64=' + (ConvertTo-B64 $text)
    exit 0
}

$paragraphRange = $selection.Paragraphs.Item(1).Range
$paragraphStart = [int] $paragraphRange.Start
if ($selectionStart -le $paragraphStart) {
    'ok=0'
    'error=empty'
    exit 0
}

$leftRange = $document.Range($paragraphStart, $selectionStart)
$text = Strip-WordRangeMarkers ([string] $leftRange.Text)
$base = $paragraphStart
'ok=1'
'kind=paragraph_left'
'base=' + $base
'selection_start=' + $selectionStart
'paragraph_start=' + $paragraphStart
'text_b64=' + (ConvertTo-B64 $text)
"#;

#[cfg(windows)]
const WORD_APPLY_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
function From-B64([string] $Text) {
    [System.Text.Encoding]::Unicode.GetString([Convert]::FromBase64String($Text))
}
function Strip-WordRangeMarkers([string] $Text) {
    if ($null -eq $Text) { return '' }
    $Text.TrimEnd([char]13, [char]7)
}
function ConvertTo-B64([string] $Text) {
    [Convert]::ToBase64String([System.Text.Encoding]::Unicode.GetBytes($Text))
}
$start = [int] $env:STEPLER_WORD_START
$end = [int] $env:STEPLER_WORD_END
$targetCaret = [int] $env:STEPLER_WORD_CARET
$expected = From-B64 $env:STEPLER_WORD_EXPECTED_B64
$replacement = From-B64 $env:STEPLER_WORD_REPLACEMENT_B64
$word = [Runtime.InteropServices.Marshal]::GetActiveObject('Word.Application')
try { $word.Activate() } catch { }
$document = $word.ActiveDocument
$range = $document.Range($start, $end)
$actual = Strip-WordRangeMarkers ([string] $range.Text)
if ($actual -ne $expected) {
    'ok=0'
    'error=preflight'
    exit 0
}
$rightBefore = ''
try {
    $rightBefore = Strip-WordRangeMarkers ([string] $document.Range($end, $end + 1).Text)
} catch { }
$range.Text = $replacement
$caret = $targetCaret
$word.Selection.SetRange($caret, $caret)
Start-Sleep -Milliseconds 140
$rightAfter = ''
try {
    $rightAfter = Strip-WordRangeMarkers ([string] $document.Range($caret, $caret + 1).Text)
} catch { }
if ($rightAfter -eq 'с' -and $rightBefore -ne 'с') {
    try {
        $document.Range($caret, $caret + 1).Delete() | Out-Null
        $word.Selection.SetRange($caret, $caret)
    } catch { }
}
$afterEnd = $caret
try {
    $afterEnd = [Math]::Min($document.Content.End, $caret + 1)
} catch { }
$after = ''
try {
    $after = Strip-WordRangeMarkers ([string] $document.Range($start, $afterEnd).Text)
} catch { }
'ok=1'
'after_b64=' + (ConvertTo-B64 $after)
"#;

#[cfg(windows)]
const OUTLOOK_WORD_CAPTURE_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
function ConvertTo-B64([string] $Text) {
    [Convert]::ToBase64String([System.Text.Encoding]::Unicode.GetBytes($Text))
}
function Strip-WordRangeMarkers([string] $Text) {
    if ($null -eq $Text) { return '' }
    $Text.TrimEnd([char]13, [char]7)
}
$outlook = [Runtime.InteropServices.Marshal]::GetActiveObject('Outlook.Application')
$inspector = $outlook.ActiveInspector()
if ($null -eq $inspector) {
    'ok=0'
    'error=no_active_inspector'
    exit 0
}
$inspector.Activate()
$document = $inspector.WordEditor
if ($null -eq $document) {
    'ok=0'
    'error=no_word_editor'
    exit 0
}
$word = $document.Application
$selection = $word.Selection
$selectionStart = [int] $selection.Start
$selectionEnd = [int] $selection.End

if ($selectionStart -ne $selectionEnd) {
    $range = $document.Range($selectionStart, $selectionEnd)
    $text = Strip-WordRangeMarkers ([string] $range.Text)
    'ok=1'
    'kind=selection'
    'base=' + $selectionStart
    'text_b64=' + (ConvertTo-B64 $text)
    exit 0
}

$paragraphRange = $selection.Paragraphs.Item(1).Range
$paragraphStart = [int] $paragraphRange.Start
if ($selectionStart -le $paragraphStart) {
    'ok=0'
    'error=empty'
    exit 0
}

$leftRange = $document.Range($paragraphStart, $selectionStart)
$text = Strip-WordRangeMarkers ([string] $leftRange.Text)
$base = $paragraphStart
'ok=1'
'kind=paragraph_left'
'base=' + $base
'selection_start=' + $selectionStart
'paragraph_start=' + $paragraphStart
'text_b64=' + (ConvertTo-B64 $text)
"#;

#[cfg(windows)]
const OUTLOOK_WORD_APPLY_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
function From-B64([string] $Text) {
    [System.Text.Encoding]::Unicode.GetString([Convert]::FromBase64String($Text))
}
function Strip-WordRangeMarkers([string] $Text) {
    if ($null -eq $Text) { return '' }
    $Text.TrimEnd([char]13, [char]7)
}
function ConvertTo-B64([string] $Text) {
    [Convert]::ToBase64String([System.Text.Encoding]::Unicode.GetBytes($Text))
}
$start = [int] $env:STEPLER_WORD_START
$end = [int] $env:STEPLER_WORD_END
$targetCaret = [int] $env:STEPLER_WORD_CARET
$expected = From-B64 $env:STEPLER_WORD_EXPECTED_B64
$replacement = From-B64 $env:STEPLER_WORD_REPLACEMENT_B64
$outlook = [Runtime.InteropServices.Marshal]::GetActiveObject('Outlook.Application')
$inspector = $outlook.ActiveInspector()
if ($null -eq $inspector) {
    'ok=0'
    'error=no_active_inspector'
    exit 0
}
$inspector.Activate()
$document = $inspector.WordEditor
if ($null -eq $document) {
    'ok=0'
    'error=no_word_editor'
    exit 0
}
$word = $document.Application
$range = $document.Range($start, $end)
$actual = Strip-WordRangeMarkers ([string] $range.Text)
if ($actual -ne $expected) {
    'ok=0'
    'error=preflight'
    exit 0
}
$rightBefore = ''
try {
    $rightBefore = Strip-WordRangeMarkers ([string] $document.Range($end, $end + 1).Text)
} catch { }
$range.Text = $replacement
$caret = $targetCaret
$word.Selection.SetRange($caret, $caret)
Start-Sleep -Milliseconds 140
$rightAfter = ''
try {
    $rightAfter = Strip-WordRangeMarkers ([string] $document.Range($caret, $caret + 1).Text)
} catch { }
if ($rightAfter -eq 'с' -and $rightBefore -ne 'с') {
    try {
        $document.Range($caret, $caret + 1).Delete() | Out-Null
        $word.Selection.SetRange($caret, $caret)
    } catch { }
}
$afterEnd = $caret
try {
    $afterEnd = [Math]::Min($document.Content.End, $caret + 1)
} catch { }
$after = ''
try {
    $after = Strip-WordRangeMarkers ([string] $document.Range($start, $afterEnd).Text)
} catch { }
'ok=1'
'after_b64=' + (ConvertTo-B64 $after)
"#;

#[cfg(windows)]
const UIA_CAPTURE_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class SteplerUser32 {
    [DllImport("user32.dll")]
    public static extern IntPtr GetForegroundWindow();
}
'@
function Get-SteplerForegroundHandle {
    if (-not [string]::IsNullOrWhiteSpace($env:STEPLER_UIA_FOREGROUND_HWND)) {
        return [IntPtr]([Int64]::Parse($env:STEPLER_UIA_FOREGROUND_HWND))
    }
    [SteplerUser32]::GetForegroundWindow()
}
function ConvertTo-B64([string] $Text) {
    if ($null -eq $Text) { $Text = '' }
    [Convert]::ToBase64String([System.Text.Encoding]::Unicode.GetBytes($Text))
}
function Get-Pattern($Element, $Pattern) {
    try { return $Element.GetCurrentPattern($Pattern) } catch { return $null }
}
function Normalize-Text([string] $Text) {
    if ($null -eq $Text) { return '' }
    $Text.TrimEnd([char]13)
}
function Get-ValuePattern($Element) {
    if ($null -eq $Element) { return $null }
    Get-Pattern $Element ([System.Windows.Automation.ValuePattern]::Pattern)
}
function Is-WritableValueElement($Element) {
    $value = Get-ValuePattern $Element
    $null -ne $value -and -not $value.Current.IsReadOnly
}
function Find-WritableValueElement {
    $focused = [System.Windows.Automation.AutomationElement]::FocusedElement
    if (Is-WritableValueElement $focused) {
        return $focused
    }
    $foreground = [System.Windows.Automation.AutomationElement]::FromHandle((Get-SteplerForegroundHandle))
    if ($null -eq $foreground) { return $focused }
    $fixtureCondition = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::AutomationIdProperty,
        'SteplerUiaFixtureInput')
    $fixture = $foreground.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $fixtureCondition)
    if (Is-WritableValueElement $fixture) {
        return $fixture
    }
    $editCondition = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
        [System.Windows.Automation.ControlType]::Edit)
    $edits = $foreground.FindAll([System.Windows.Automation.TreeScope]::Descendants, $editCondition)
    foreach ($edit in $edits) {
        try {
            if ($edit.Current.HasKeyboardFocus -and (Is-WritableValueElement $edit)) {
                return $edit
            }
        } catch { }
    }
    return $focused
}
$element = Find-WritableValueElement
if ($null -eq $element) {
    'ok=0'
    exit 0
}
$strictEditable = $env:STEPLER_UIA_STRICT_EDITABLE -eq '1'
if ($strictEditable) {
    try {
        if (-not $element.Current.HasKeyboardFocus) {
            'ok=0'
            'error=no_keyboard_focus'
            exit 0
        }
        if (-not $element.Current.IsKeyboardFocusable) {
            'ok=0'
            'error=not_keyboard_focusable'
            exit 0
        }
        if ($element.Current.ControlType.ProgrammaticName -ne 'ControlType.Edit') {
            'ok=0'
            'error=not_edit_control'
            exit 0
        }
    } catch {
        'ok=0'
        'error=strict_metadata'
        exit 0
    }
}
$runtimeId = ($element.GetRuntimeId() -join '.')
$valuePattern = Get-Pattern $element ([System.Windows.Automation.ValuePattern]::Pattern)
$textPattern = Get-Pattern $element ([System.Windows.Automation.TextPattern]::Pattern)
$canSetValue = 0
$text = ''
if ($null -ne $valuePattern) {
    $text = [string]$valuePattern.Current.Value
    if (-not $valuePattern.Current.IsReadOnly) {
        $canSetValue = 1
    }
} elseif ($null -ne $textPattern) {
    $text = $textPattern.DocumentRange.GetText(-1)
}
$text = Normalize-Text $text
if ($text.Length -eq 0) {
    'ok=0'
    exit 0
}
if ($strictEditable) {
    if ($canSetValue -ne 1) {
        'ok=0'
        'error=no_writable_value'
        exit 0
    }
    if ($null -eq $textPattern) {
        'ok=0'
        'error=no_text_pattern'
        exit 0
    }
    if ($text.Length -gt 20000) {
        'ok=0'
        'error=text_too_large'
        exit 0
    }
    $newlineCount = ([regex]::Matches($text, "`n")).Count
    if ($newlineCount -gt 200) {
        'ok=0'
        'error=too_many_lines'
        exit 0
    }
}
$caret = $text.Length
$selectionStart = $caret
$selectionEnd = $caret
if ($null -ne $textPattern) {
    try {
        $selection = $textPattern.GetSelection()
        if ($null -ne $selection -and $selection.Length -gt 0) {
            $range = $selection[0]
            $document = $textPattern.DocumentRange
            $beforeStart = $document.Clone()
            $null = $beforeStart.MoveEndpointByRange(
                [System.Windows.Automation.Text.TextPatternRangeEndpoint]::End,
                $range,
                [System.Windows.Automation.Text.TextPatternRangeEndpoint]::Start)
            $beforeEnd = $document.Clone()
            $null = $beforeEnd.MoveEndpointByRange(
                [System.Windows.Automation.Text.TextPatternRangeEndpoint]::End,
                $range,
                [System.Windows.Automation.Text.TextPatternRangeEndpoint]::End)
            $selectionStart = (Normalize-Text $beforeStart.GetText(-1)).Length
            $selectionEnd = (Normalize-Text $beforeEnd.GetText(-1)).Length
            $caret = $selectionEnd
        }
    } catch {
        $caret = $text.Length
        $selectionStart = $caret
        $selectionEnd = $caret
    }
}
'ok=1'
'runtime_id=' + $runtimeId
'can_set_value=' + $canSetValue
'caret=' + $caret
'selection_start=' + $selectionStart
'selection_end=' + $selectionEnd
'text_b64=' + (ConvertTo-B64 $text)
"#;

#[cfg(windows)]
const UIA_FOCUS_DIAGNOSTICS_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
function Escape-Line([string] $Text) {
    if ($null -eq $Text) { return '' }
    $Text.Replace("`r", '\r').Replace("`n", '\n')
}
$element = [System.Windows.Automation.AutomationElement]::FocusedElement
if ($null -eq $element) {
    'ok=0'
    exit 0
}
'ok=1'
'name=' + (Escape-Line ([string]$element.Current.Name))
'control_type=' + ([string]$element.Current.ControlType.ProgrammaticName)
'automation_id=' + (Escape-Line ([string]$element.Current.AutomationId))
'class_name=' + (Escape-Line ([string]$element.Current.ClassName))
'framework_id=' + (Escape-Line ([string]$element.Current.FrameworkId))
'has_keyboard_focus=' + ($(if ($element.Current.HasKeyboardFocus) { '1' } else { '0' }))
'is_keyboard_focusable=' + ($(if ($element.Current.IsKeyboardFocusable) { '1' } else { '0' }))
"#;

#[cfg(windows)]
const UIA_APPLY_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class SteplerUser32 {
    [DllImport("user32.dll")]
    public static extern IntPtr GetForegroundWindow();
}
'@
function Get-SteplerForegroundHandle {
    if (-not [string]::IsNullOrWhiteSpace($env:STEPLER_UIA_FOREGROUND_HWND)) {
        return [IntPtr]([Int64]::Parse($env:STEPLER_UIA_FOREGROUND_HWND))
    }
    [SteplerUser32]::GetForegroundWindow()
}
function ConvertTo-B64([string] $Text) {
    if ($null -eq $Text) { $Text = '' }
    [Convert]::ToBase64String([System.Text.Encoding]::Unicode.GetBytes($Text))
}
function ConvertFrom-B64([string] $Text) {
    [System.Text.Encoding]::Unicode.GetString([Convert]::FromBase64String($Text))
}
function Get-Pattern($Element, $Pattern) {
    try { return $Element.GetCurrentPattern($Pattern) } catch { return $null }
}
function Get-CaretRange($Element) {
    $textPattern2 = Get-Pattern $Element ([System.Windows.Automation.TextPattern2]::Pattern)
    if ($null -eq $textPattern2) { return $null }
    try {
        $isActive = $false
        return $textPattern2.GetCaretRange([ref]$isActive)
    } catch {
        return $null
    }
}
function Runtime-Id($Element) {
    if ($null -eq $Element) { return '' }
    try { return ($Element.GetRuntimeId() -join '.') } catch { return '' }
}
function Find-ElementByRuntimeId([string] $RuntimeId) {
    $focused = [System.Windows.Automation.AutomationElement]::FocusedElement
    if ((Runtime-Id $focused) -eq $RuntimeId) {
        return $focused
    }
    $foreground = [System.Windows.Automation.AutomationElement]::FromHandle((Get-SteplerForegroundHandle))
    if ($null -eq $foreground) { return $focused }
    $condition = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
        [System.Windows.Automation.ControlType]::Edit)
    $all = $foreground.FindAll([System.Windows.Automation.TreeScope]::Descendants, $condition)
    foreach ($candidate in $all) {
        if ((Runtime-Id $candidate) -eq $RuntimeId) {
            return $candidate
        }
    }
    return $focused
}
$element = Find-ElementByRuntimeId $env:STEPLER_UIA_RUNTIME_ID
if ($null -eq $element) {
    'ok=0'
    exit 0
}
if ((Runtime-Id $element) -ne $env:STEPLER_UIA_RUNTIME_ID) {
    'ok=0'
    exit 0
}
$valuePattern = Get-Pattern $element ([System.Windows.Automation.ValuePattern]::Pattern)
if ($null -eq $valuePattern -or $valuePattern.Current.IsReadOnly) {
    'ok=0'
    exit 0
}
$expected = ConvertFrom-B64 $env:STEPLER_UIA_EXPECTED_B64
$replacement = ConvertFrom-B64 $env:STEPLER_UIA_REPLACEMENT_B64
if ([string]$valuePattern.Current.Value -ne $expected) {
    'ok=0'
    exit 0
}
$valuePattern.SetValue($replacement)
Start-Sleep -Milliseconds 30
$caret = 0
if (-not [string]::IsNullOrWhiteSpace($env:STEPLER_UIA_CARET_UTF16)) {
    $caret = [int]$env:STEPLER_UIA_CARET_UTF16
}
$textPattern = Get-Pattern $element ([System.Windows.Automation.TextPattern]::Pattern)
if ($null -ne $textPattern) {
    try {
        $range = $textPattern.DocumentRange.Clone()
        $null = $range.MoveEndpointByUnit(
            [System.Windows.Automation.Text.TextPatternRangeEndpoint]::Start,
            [System.Windows.Automation.Text.TextUnit]::Character,
            $caret)
        $null = $range.MoveEndpointByRange(
            [System.Windows.Automation.Text.TextPatternRangeEndpoint]::End,
            $range,
            [System.Windows.Automation.Text.TextPatternRangeEndpoint]::Start)
        $range.Select()
    } catch { }
}
'ok=1'
'after_b64=' + (ConvertTo-B64 ([string]$valuePattern.Current.Value))
"#;

#[cfg(windows)]
const UIA_DOCUMENT_CAPTURE_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class SteplerUser32 {
    [DllImport("user32.dll")]
    public static extern IntPtr GetForegroundWindow();
}
'@
function Get-SteplerForegroundHandle {
    if (-not [string]::IsNullOrWhiteSpace($env:STEPLER_UIA_FOREGROUND_HWND)) {
        return [IntPtr]([Int64]::Parse($env:STEPLER_UIA_FOREGROUND_HWND))
    }
    [SteplerUser32]::GetForegroundWindow()
}
function ConvertTo-B64([string] $Text) {
    if ($null -eq $Text) { $Text = '' }
    [Convert]::ToBase64String([System.Text.Encoding]::Unicode.GetBytes($Text))
}
function Normalize-Text([string] $Text) {
    if ($null -eq $Text) { return '' }
    $Text.TrimEnd([char]13)
}
function Get-Pattern($Element, $Pattern) {
    try { return $Element.GetCurrentPattern($Pattern) } catch { return $null }
}
function Runtime-Id($Element) {
    if ($null -eq $Element) { return '' }
    try { return ($Element.GetRuntimeId() -join '.') } catch { return '' }
}
function Has-TextPattern($Element) {
    $null -ne (Get-Pattern $Element ([System.Windows.Automation.TextPattern]::Pattern))
}
function Get-CaretRange($Element) {
    $textPattern2 = Get-Pattern $Element ([System.Windows.Automation.TextPattern2]::Pattern)
    if ($null -eq $textPattern2) { return $null }
    try {
        $isActive = $false
        return $textPattern2.GetCaretRange([ref]$isActive)
    } catch {
        return $null
    }
}
function Selection-Text($Element) {
    $textPattern = Get-Pattern $Element ([System.Windows.Automation.TextPattern]::Pattern)
    if ($null -eq $textPattern) { return $null }
    $selection = $null
    try { $selection = $textPattern.GetSelection() } catch { return $null }
    if ($null -eq $selection -or $selection.Length -eq 0) { return $null }
    $text = Normalize-Text ($selection[0].GetText(-1))
    if ([string]::IsNullOrWhiteSpace($text)) { return $null }
    return $text
}
function Supports-CaretRange($Element) {
    $null -ne (Get-CaretRange $Element)
}
function Find-TextElement {
    $allowCaret = $env:STEPLER_UIA_DOCUMENT_ALLOW_CARET_FALLBACK -eq '1'
    $focused = [System.Windows.Automation.AutomationElement]::FocusedElement
    if ($null -ne (Selection-Text $focused)) {
        return $focused
    }
    if ($allowCaret -and (Supports-CaretRange $focused)) {
        return $focused
    }
    $foreground = [System.Windows.Automation.AutomationElement]::FromHandle((Get-SteplerForegroundHandle))
    if ($null -eq $foreground) { return $focused }
    $condition = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::IsKeyboardFocusableProperty,
        $true)
    $all = $foreground.FindAll([System.Windows.Automation.TreeScope]::Descendants, $condition)
    foreach ($candidate in $all) {
        try {
            if ($candidate.Current.HasKeyboardFocus -and ($null -ne (Selection-Text $candidate))) {
                return $candidate
            }
            if ($allowCaret -and $candidate.Current.HasKeyboardFocus -and (Supports-CaretRange $candidate)) {
                return $candidate
            }
        } catch { }
    }
    $all = $foreground.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
    foreach ($candidate in $all) {
        try {
            if ($null -ne (Selection-Text $candidate)) {
                return $candidate
            }
            if ($allowCaret -and (Supports-CaretRange $candidate)) {
                return $candidate
            }
        } catch { }
    }
    return $focused
}
$element = Find-TextElement
if ($null -eq $element) {
    'ok=0'
    'error=no_text_element'
    exit 0
}
$textPattern = Get-Pattern $element ([System.Windows.Automation.TextPattern]::Pattern)
if ($null -eq $textPattern) {
    'ok=0'
    'error=no_text_pattern'
    exit 0
}
$selection = $null
try { $selection = $textPattern.GetSelection() } catch { }
if ($null -eq $selection -or $selection.Length -eq 0) {
    if ($env:STEPLER_UIA_DOCUMENT_ALLOW_CARET_FALLBACK -ne '1') {
        'ok=0'
        'error=no_selection'
        exit 0
    }
    $range = Get-CaretRange $element
    if ($null -eq $range) {
        'ok=0'
        'error=no_selection'
        exit 0
    }
} else {
    $range = $selection[0]
}
$isCollapsed = $false
try {
    $isCollapsed = 0 -eq $range.CompareEndpoints(
        [System.Windows.Automation.Text.TextPatternRangeEndpoint]::Start,
        $range,
        [System.Windows.Automation.Text.TextPatternRangeEndpoint]::End)
} catch { }
$text = Normalize-Text ($range.GetText(-1))
if ($isCollapsed -or [string]::IsNullOrWhiteSpace($text)) {
    if ($env:STEPLER_UIA_DOCUMENT_ALLOW_CARET_FALLBACK -ne '1') {
        'ok=0'
        'error=empty_selection_text'
        exit 0
    }
    $document = $textPattern.DocumentRange
    $beforeCaret = $document.Clone()
    $beforeCaret.MoveEndpointByRange(
        [System.Windows.Automation.Text.TextPatternRangeEndpoint]::End,
        $range,
        [System.Windows.Automation.Text.TextPatternRangeEndpoint]::End) | Out-Null
    $text = Normalize-Text ($beforeCaret.GetText(-1))
    if ([string]::IsNullOrWhiteSpace($text)) {
        'ok=0'
        'error=empty_caret_left_text'
        exit 0
    }
    if ($text.Length -gt 20000) {
        'ok=0'
        'error=caret_left_text_too_large'
        exit 0
    }
    'ok=1'
    'kind=caret'
    'runtime_id=' + (Runtime-Id $element)
    'text_b64=' + (ConvertTo-B64 $text)
    exit 0
}
'ok=1'
'kind=selection'
'runtime_id=' + (Runtime-Id $element)
'text_b64=' + (ConvertTo-B64 $text)
"#;

#[cfg(windows)]
const UIA_DOCUMENT_SELECT_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class SteplerUser32 {
    [DllImport("user32.dll")]
    public static extern IntPtr GetForegroundWindow();
}
'@
function Get-SteplerForegroundHandle {
    if (-not [string]::IsNullOrWhiteSpace($env:STEPLER_UIA_FOREGROUND_HWND)) {
        return [IntPtr]([Int64]::Parse($env:STEPLER_UIA_FOREGROUND_HWND))
    }
    [SteplerUser32]::GetForegroundWindow()
}
function ConvertFrom-B64([string] $Text) {
    [System.Text.Encoding]::Unicode.GetString([Convert]::FromBase64String($Text))
}
function Normalize-Text([string] $Text) {
    if ($null -eq $Text) { return '' }
    $Text.TrimEnd([char]13)
}
function Get-Pattern($Element, $Pattern) {
    try { return $Element.GetCurrentPattern($Pattern) } catch { return $null }
}
function Runtime-Id($Element) {
    if ($null -eq $Element) { return '' }
    try { return ($Element.GetRuntimeId() -join '.') } catch { return '' }
}
function Find-ElementByRuntimeId([string] $RuntimeId) {
    $focused = [System.Windows.Automation.AutomationElement]::FocusedElement
    if ((Runtime-Id $focused) -eq $RuntimeId) {
        return $focused
    }
    $foreground = [System.Windows.Automation.AutomationElement]::FromHandle((Get-SteplerForegroundHandle))
    if ($null -eq $foreground) { return $focused }
    $all = $foreground.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
    foreach ($candidate in $all) {
        if ((Runtime-Id $candidate) -eq $RuntimeId) {
            return $candidate
        }
    }
    return $focused
}
$element = Find-ElementByRuntimeId $env:STEPLER_UIA_RUNTIME_ID
if ($null -eq $element -or (Runtime-Id $element) -ne $env:STEPLER_UIA_RUNTIME_ID) {
    'ok=0'
    'error=element_changed'
    exit 0
}
$textPattern = Get-Pattern $element ([System.Windows.Automation.TextPattern]::Pattern)
if ($null -eq $textPattern) {
    'ok=0'
    'error=no_text_pattern'
    exit 0
}
$selection = $textPattern.GetSelection()
if ($null -eq $selection -or $selection.Length -eq 0) {
    'ok=0'
    'error=no_selection'
    exit 0
}
$range = $selection[0]
$actual = Normalize-Text ($range.GetText(-1))
$expected = ConvertFrom-B64 $env:STEPLER_UIA_EXPECTED_B64
if ($actual -ne $expected) {
    'ok=0'
    'error=preflight'
    exit 0
}
$range.Select()
'ok=1'
"#;

#[cfg(windows)]
const UIA_DOCUMENT_SELECT_CARET_RANGE_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class SteplerUser32 {
    [DllImport("user32.dll")]
    public static extern IntPtr GetForegroundWindow();
}
'@
function Get-SteplerForegroundHandle {
    if (-not [string]::IsNullOrWhiteSpace($env:STEPLER_UIA_FOREGROUND_HWND)) {
        return [IntPtr]([Int64]::Parse($env:STEPLER_UIA_FOREGROUND_HWND))
    }
    [SteplerUser32]::GetForegroundWindow()
}
function ConvertFrom-B64([string] $Text) {
    [System.Text.Encoding]::Unicode.GetString([Convert]::FromBase64String($Text))
}
function Normalize-Text([string] $Text) {
    if ($null -eq $Text) { return '' }
    $Text.TrimEnd([char]13)
}
function Get-Pattern($Element, $Pattern) {
    try { return $Element.GetCurrentPattern($Pattern) } catch { return $null }
}
function Runtime-Id($Element) {
    if ($null -eq $Element) { return '' }
    try { return ($Element.GetRuntimeId() -join '.') } catch { return '' }
}
function Find-ElementByRuntimeId([string] $RuntimeId) {
    $focused = [System.Windows.Automation.AutomationElement]::FocusedElement
    if ((Runtime-Id $focused) -eq $RuntimeId) {
        return $focused
    }
    $foreground = [System.Windows.Automation.AutomationElement]::FromHandle((Get-SteplerForegroundHandle))
    if ($null -eq $foreground) { return $focused }
    $all = $foreground.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
    foreach ($candidate in $all) {
        if ((Runtime-Id $candidate) -eq $RuntimeId) {
            return $candidate
        }
    }
    return $focused
}
$element = Find-ElementByRuntimeId $env:STEPLER_UIA_RUNTIME_ID
if ($null -eq $element -or (Runtime-Id $element) -ne $env:STEPLER_UIA_RUNTIME_ID) {
    'ok=0'
    'error=element_changed'
    exit 0
}
$textPattern = Get-Pattern $element ([System.Windows.Automation.TextPattern]::Pattern)
if ($null -eq $textPattern) {
    'ok=0'
    'error=no_text_pattern'
    exit 0
}
$selection = $null
try { $selection = $textPattern.GetSelection() } catch { }
if ($null -eq $selection -or $selection.Length -eq 0) {
    $range = Get-CaretRange $element
}
else {
    $range = $selection[0].Clone()
}
if ($null -eq $range) {
    'ok=0'
    'error=no_caret_selection'
    exit 0
}
$range = $range.Clone()
$startDelta = [int]$env:STEPLER_UIA_START_DELTA_UTF16
$endDelta = [int]$env:STEPLER_UIA_END_DELTA_UTF16
$range.MoveEndpointByUnit(
    [System.Windows.Automation.Text.TextPatternRangeEndpoint]::Start,
    [System.Windows.Automation.Text.TextUnit]::Character,
    $startDelta) | Out-Null
$range.MoveEndpointByUnit(
    [System.Windows.Automation.Text.TextPatternRangeEndpoint]::End,
    [System.Windows.Automation.Text.TextUnit]::Character,
    $endDelta) | Out-Null
$actual = Normalize-Text ($range.GetText(-1))
$expected = ConvertFrom-B64 $env:STEPLER_UIA_EXPECTED_B64
if ($actual -ne $expected) {
    'ok=0'
    'error=preflight'
    exit 0
}
$range.Select()
'ok=1'
"#;

#[cfg(windows)]
const UIA_DOCUMENT_VERIFY_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class SteplerUser32 {
    [DllImport("user32.dll")]
    public static extern IntPtr GetForegroundWindow();
}
'@
function Get-SteplerForegroundHandle {
    if (-not [string]::IsNullOrWhiteSpace($env:STEPLER_UIA_FOREGROUND_HWND)) {
        return [IntPtr]([Int64]::Parse($env:STEPLER_UIA_FOREGROUND_HWND))
    }
    [SteplerUser32]::GetForegroundWindow()
}
function ConvertTo-B64([string] $Text) {
    if ($null -eq $Text) { $Text = '' }
    [Convert]::ToBase64String([System.Text.Encoding]::Unicode.GetBytes($Text))
}
function ConvertFrom-B64([string] $Text) {
    [System.Text.Encoding]::Unicode.GetString([Convert]::FromBase64String($Text))
}
function Normalize-Text([string] $Text) {
    if ($null -eq $Text) { return '' }
    $Text.TrimEnd([char]13)
}
function Get-Pattern($Element, $Pattern) {
    try { return $Element.GetCurrentPattern($Pattern) } catch { return $null }
}
function Runtime-Id($Element) {
    if ($null -eq $Element) { return '' }
    try { return ($Element.GetRuntimeId() -join '.') } catch { return '' }
}
function Find-ElementByRuntimeId([string] $RuntimeId) {
    $focused = [System.Windows.Automation.AutomationElement]::FocusedElement
    if ((Runtime-Id $focused) -eq $RuntimeId) {
        return $focused
    }
    $foreground = [System.Windows.Automation.AutomationElement]::FromHandle((Get-SteplerForegroundHandle))
    if ($null -eq $foreground) { return $focused }
    $all = $foreground.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
    foreach ($candidate in $all) {
        if ((Runtime-Id $candidate) -eq $RuntimeId) {
            return $candidate
        }
    }
    return $focused
}
$element = Find-ElementByRuntimeId $env:STEPLER_UIA_RUNTIME_ID
if ($null -eq $element -or (Runtime-Id $element) -ne $env:STEPLER_UIA_RUNTIME_ID) {
    'ok=0'
    'error=element_changed'
    exit 0
}
$textPattern = Get-Pattern $element ([System.Windows.Automation.TextPattern]::Pattern)
if ($null -eq $textPattern) {
    'ok=0'
    'error=no_text_pattern'
    exit 0
}
$replacement = ConvertFrom-B64 $env:STEPLER_UIA_REPLACEMENT_B64
$selection = $textPattern.GetSelection()
if ($null -eq $selection -or $selection.Length -eq 0) {
    'ok=0'
    'error=no_caret_selection'
    exit 0
}
$range = $selection[0].Clone()
try {
    $null = $range.MoveEndpointByUnit(
        [System.Windows.Automation.Text.TextPatternRangeEndpoint]::Start,
        [System.Windows.Automation.Text.TextUnit]::Character,
        -1 * $replacement.Length)
    $actual = Normalize-Text ($range.GetText(-1))
    if ($actual -eq $replacement) {
        'ok=1'
        'actual_b64=' + (ConvertTo-B64 $actual)
        exit 0
    }
} catch { }
'ok=0'
'error=verify_failed'
"#;

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

#[cfg(windows)]
fn window_process_id(hwnd: isize) -> Result<u32, PlatformError> {
    let mut process_id = 0u32;
    let thread_id = unsafe { GetWindowThreadProcessId(hwnd, &mut process_id as *mut u32) };
    if thread_id == 0 || process_id == 0 {
        return Err(PlatformError::ForegroundUnavailable);
    }
    Ok(process_id)
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
    let marker = format!(
        "__STEPLER_COPY_MARKER_{}__",
        snapshot.sequence_number.unwrap_or(0)
    );
    let _ = restore_clipboard(clipboard_snapshot_from_text(&marker));
    release_modifier_keys();
    std::thread::sleep(Duration::from_millis(8));
    send_key_chord_virtual(&[VK_CONTROL], VK_C);
    wait_for_clipboard_text_different_from(&marker, timeout)
}

#[cfg(windows)]
fn wait_for_clipboard_text_different_from(marker: &str, timeout: Duration) -> Option<String> {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if let Ok(snapshot) = capture_clipboard_text_only() {
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
fn capture_clipboard_text_only() -> Result<ClipboardSnapshot, PlatformError> {
    let _guard = ClipboardGuard::open()?;
    let sequence_number = Some(unsafe { GetClipboardSequenceNumber() });
    let text = if unsafe { IsClipboardFormatAvailable(CF_UNICODETEXT) } != 0 {
        Some(read_clipboard_text()?)
    } else {
        None
    };

    Ok(ClipboardSnapshot {
        text,
        sequence_number,
        formats: Vec::new(),
    })
}

#[cfg(windows)]
fn restore_clipboard_text_only(snapshot: &ClipboardSnapshot) -> Result<(), PlatformError> {
    if let Some(text) = &snapshot.text {
        restore_clipboard(clipboard_snapshot_from_text(text))
    } else {
        let _guard = ClipboardGuard::open()?;
        unsafe {
            if EmptyClipboard() == 0 {
                return Err(PlatformError::ClipboardUnavailable);
            }
        }
        Ok(())
    }
}

#[cfg(windows)]
fn clipboard_snapshot_from_text(text: &str) -> ClipboardSnapshot {
    ClipboardSnapshot {
        text: Some(text.to_owned()),
        sequence_number: None,
        formats: vec![ClipboardFormatSnapshot {
            format: CF_UNICODETEXT,
            bytes: utf16_to_le_bytes(&string_to_null_terminated_utf16(text)),
        }],
    }
}

#[cfg(windows)]
fn send_key(vk: u32) {
    let _ = send_keyboard_input(&[
        KeyboardInputEvent::new(vk, false, KeyboardInputMode::ScanCode),
        KeyboardInputEvent::new(vk, true, KeyboardInputMode::ScanCode),
    ]);
}

#[cfg(windows)]
fn send_key_virtual(vk: u32) {
    let _ = send_keyboard_input(&[
        KeyboardInputEvent::new(vk, false, KeyboardInputMode::VirtualKey),
        KeyboardInputEvent::new(vk, true, KeyboardInputMode::VirtualKey),
    ]);
}

#[cfg(windows)]
fn send_stepler_control_key(vk: u32) -> Result<(), PlatformError> {
    let events = [
        KeyboardInputEvent::new_with_extra(
            vk,
            false,
            KeyboardInputMode::VirtualKey,
            STEPLER_INJECTED_CONTROL_MAGIC,
        ),
        KeyboardInputEvent::new_with_extra(
            vk,
            true,
            KeyboardInputMode::VirtualKey,
            STEPLER_INJECTED_CONTROL_MAGIC,
        ),
    ];
    send_keyboard_input(&events)
        .then_some(())
        .ok_or(PlatformError::Unsupported)
}

#[cfg(windows)]
fn send_key_chord(modifiers: &[u32], key: u32) {
    send_key_chord_with_mode(modifiers, key, KeyboardInputMode::ScanCode);
}

#[cfg(windows)]
fn send_key_chord_virtual(modifiers: &[u32], key: u32) {
    send_key_chord_mixed(modifiers, key);
}

#[cfg(windows)]
fn send_terminal_shortcut_with_english_layout(modifiers: &[u32], key: u32) {
    let original_layout = foreground_keyboard_layout().ok();
    if let Some(english_layout) = find_layout_by_language(&keyboard_layouts(), LANG_ENGLISH) {
        let _ = switch_foreground_layout(english_layout);
        std::thread::sleep(Duration::from_millis(40));
    }

    send_key_chord_virtual(modifiers, key);

    if let Some(layout) = original_layout {
        let _ = switch_foreground_layout(layout);
    }
    release_modifier_keys();
}

#[cfg(windows)]
fn send_ssh_terminal_sequence(mode: stepler_core::CorrectionMode) {
    let sequence = match mode {
        stepler_core::CorrectionMode::Pause => "\u{1b}[777;1u",
        stepler_core::CorrectionMode::ScrollLock => "\u{1b}[777;2u",
    };
    let _ = send_unicode_text(sequence);
}

#[cfg(windows)]
fn send_key_chord_with_mode(modifiers: &[u32], key: u32, mode: KeyboardInputMode) {
    let mut events = Vec::new();
    events.extend(
        modifiers
            .iter()
            .copied()
            .map(|modifier| KeyboardInputEvent::new(modifier, false, mode)),
    );
    if !send_keyboard_input(&events) {
        return;
    }
    std::thread::sleep(Duration::from_millis(10));

    if !send_keyboard_input(&[
        KeyboardInputEvent::new(key, false, mode),
        KeyboardInputEvent::new(key, true, mode),
    ]) {
        let _ = send_keyboard_input(
            &modifiers
                .iter()
                .rev()
                .copied()
                .map(|modifier| KeyboardInputEvent::new(modifier, true, mode))
                .collect::<Vec<_>>(),
        );
        return;
    }
    std::thread::sleep(Duration::from_millis(10));

    let mut events = Vec::new();
    events.extend(
        modifiers
            .iter()
            .rev()
            .copied()
            .map(|modifier| KeyboardInputEvent::new(modifier, true, mode)),
    );
    let _ = send_keyboard_input(&events);
    release_modifier_keys();
}

#[cfg(windows)]
fn send_key_chord_mixed(modifiers: &[u32], key: u32) {
    let mut events = Vec::new();
    events.extend(
        modifiers
            .iter()
            .copied()
            .map(|modifier| KeyboardInputEvent::new(modifier, false, KeyboardInputMode::ScanCode)),
    );
    if !send_keyboard_input(&events) {
        return;
    }
    std::thread::sleep(Duration::from_millis(10));

    if !send_keyboard_input(&[
        KeyboardInputEvent::new(key, false, KeyboardInputMode::VirtualKey),
        KeyboardInputEvent::new(key, true, KeyboardInputMode::VirtualKey),
    ]) {
        let _ = send_keyboard_input(
            &modifiers
                .iter()
                .rev()
                .copied()
                .map(|modifier| {
                    KeyboardInputEvent::new(modifier, true, KeyboardInputMode::ScanCode)
                })
                .collect::<Vec<_>>(),
        );
        return;
    }
    std::thread::sleep(Duration::from_millis(10));

    let mut events = Vec::new();
    events.extend(
        modifiers
            .iter()
            .rev()
            .copied()
            .map(|modifier| KeyboardInputEvent::new(modifier, true, KeyboardInputMode::ScanCode)),
    );
    let _ = send_keyboard_input(&events);
    release_modifier_keys();
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
    !trimmed.is_empty() && text.len() <= 512 && !text.contains('\r') && !text.contains('\n')
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
fn send_keyboard_input(events: &[KeyboardInputEvent]) -> bool {
    if events.is_empty() {
        return true;
    }
    if let Some(hwnd) = test_foreground_hwnd_override() {
        for event in events {
            let message = if event.key_up { WM_KEYUP } else { WM_KEYDOWN };
            unsafe {
                PostMessageW(hwnd, message, event.vk as usize, 0);
            }
        }
        return true;
    }

    let mut inputs = events
        .iter()
        .map(|event| match event.mode {
            KeyboardInputMode::ScanCode => {
                Input::keyboard_scan_code(event.vk, event.key_up, event.extra_info)
            }
            KeyboardInputMode::VirtualKey => {
                Input::keyboard_virtual_key(event.vk, event.key_up, event.extra_info)
            }
        })
        .collect::<Vec<_>>();

    let sent = unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_mut_ptr(),
            std::mem::size_of::<Input>() as i32,
        )
    };
    sent == inputs.len() as u32
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
fn append_hotkey_signal_log(message: &str) {
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

#[cfg(windows)]
fn send_unicode_text(text: &str) -> Result<(), PlatformError> {
    let mut inputs = Vec::new();
    for unit in text.encode_utf16() {
        inputs.push(Input::keyboard_unicode(unit, false));
        inputs.push(Input::keyboard_unicode(unit, true));
    }

    if inputs.is_empty() {
        return Ok(());
    }

    let sent = unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_mut_ptr(),
            std::mem::size_of::<Input>() as i32,
        )
    };
    if sent == inputs.len() as u32 {
        Ok(())
    } else {
        Err(PlatformError::ReplacementUnavailable)
    }
}

#[cfg(windows)]
#[derive(Debug, Clone, Copy)]
struct KeyboardInputEvent {
    vk: u32,
    key_up: bool,
    mode: KeyboardInputMode,
    extra_info: usize,
}

#[cfg(windows)]
impl KeyboardInputEvent {
    fn new(vk: u32, key_up: bool, mode: KeyboardInputMode) -> Self {
        Self {
            vk,
            key_up,
            mode,
            extra_info: 0,
        }
    }

    fn new_with_extra(vk: u32, key_up: bool, mode: KeyboardInputMode, extra_info: usize) -> Self {
        Self {
            vk,
            key_up,
            mode,
            extra_info,
        }
    }
}

#[cfg(windows)]
#[derive(Debug, Clone, Copy)]
enum KeyboardInputMode {
    ScanCode,
    VirtualKey,
}

fn parse_hwnd_id(value: &str) -> Option<isize> {
    let hex = value.strip_prefix("hwnd:")?;
    if hex.is_empty() {
        return None;
    }

    isize::from_str_radix(hex, 16).ok()
}

#[cfg(windows)]
fn foreground_keyboard_layout() -> Result<isize, PlatformError> {
    let foreground = foreground_hwnd()?;
    let thread_id = window_thread_id(foreground)?;
    Ok(unsafe { GetKeyboardLayout(thread_id) })
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
fn focused_window(foreground: isize) -> Option<isize> {
    let thread_id = unsafe { GetWindowThreadProcessId(foreground, std::ptr::null_mut()) };
    if thread_id == 0 {
        return None;
    }

    let mut info = GuiThreadInfo::default();
    info.cb_size = std::mem::size_of::<GuiThreadInfo>() as u32;
    let ok = unsafe { GetGUIThreadInfo(thread_id, &mut info as *mut GuiThreadInfo) };
    if ok == 0 || info.hwnd_focus == 0 {
        return None;
    }

    Some(info.hwnd_focus)
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

#[cfg(windows)]
fn capture_clipboard() -> Result<ClipboardSnapshot, PlatformError> {
    let _guard = ClipboardGuard::open()?;
    let sequence_number = Some(unsafe { GetClipboardSequenceNumber() });
    let formats = clipboard_formats();
    let mut snapshots = Vec::new();
    for format in formats {
        if let Some(bytes) = read_clipboard_format_bytes(format) {
            snapshots.push(ClipboardFormatSnapshot { format, bytes });
        }
    }

    let text = if snapshots
        .iter()
        .any(|snapshot| snapshot.format == CF_UNICODETEXT)
    {
        Some(read_clipboard_text()?)
    } else {
        None
    };

    Ok(ClipboardSnapshot {
        text,
        sequence_number,
        formats: snapshots,
    })
}

#[cfg(not(windows))]
fn capture_clipboard() -> Result<ClipboardSnapshot, PlatformError> {
    Err(PlatformError::Unsupported)
}

#[cfg(windows)]
fn restore_clipboard(snapshot: ClipboardSnapshot) -> Result<(), PlatformError> {
    let _guard = ClipboardGuard::open()?;
    unsafe {
        if EmptyClipboard() == 0 {
            return Err(PlatformError::ClipboardUnavailable);
        }

        for format_snapshot in snapshot.formats {
            let handle = global_alloc_from_bytes(&format_snapshot.bytes)?;
            if SetClipboardData(format_snapshot.format, handle) == 0 {
                GlobalFree(handle);
                return Err(PlatformError::ClipboardUnavailable);
            }
        }
    }

    Ok(())
}

#[cfg(not(windows))]
fn restore_clipboard(_snapshot: ClipboardSnapshot) -> Result<(), PlatformError> {
    Err(PlatformError::Unsupported)
}

#[cfg(windows)]
fn read_clipboard_text() -> Result<String, PlatformError> {
    let handle = unsafe { GetClipboardData(CF_UNICODETEXT) };
    if handle == 0 {
        return Err(PlatformError::ClipboardUnavailable);
    }

    let ptr = unsafe { GlobalLock(handle) } as *const u16;
    if ptr.is_null() {
        return Err(PlatformError::ClipboardUnavailable);
    }

    let mut len = 0;
    unsafe {
        while *ptr.add(len) != 0 {
            len += 1;
        }
    }

    let slice = unsafe { std::slice::from_raw_parts(ptr, len) };
    let text = String::from_utf16_lossy(slice);
    unsafe {
        GlobalUnlock(handle);
    }

    Ok(text)
}

#[cfg(windows)]
fn clipboard_formats() -> Vec<u32> {
    let mut formats = Vec::new();
    let mut current = 0;
    loop {
        let next = unsafe { EnumClipboardFormats(current) };
        if next == 0 {
            break;
        }
        formats.push(next);
        current = next;
    }
    formats
}

#[cfg(windows)]
fn read_clipboard_format_bytes(format: u32) -> Option<Vec<u8>> {
    let handle = unsafe { GetClipboardData(format) };
    if handle == 0 {
        return None;
    }

    let size = unsafe { GlobalSize(handle) };
    if size == 0 {
        return None;
    }

    let ptr = unsafe { GlobalLock(handle) } as *const u8;
    if ptr.is_null() {
        return None;
    }

    let bytes = unsafe { std::slice::from_raw_parts(ptr, size) }.to_vec();
    unsafe {
        GlobalUnlock(handle);
    }

    Some(bytes)
}

#[cfg(windows)]
fn global_alloc_from_bytes(bytes: &[u8]) -> Result<isize, PlatformError> {
    let handle = unsafe { GlobalAlloc(GMEM_MOVEABLE, bytes.len()) };
    if handle == 0 {
        return Err(PlatformError::ClipboardUnavailable);
    }

    let target = unsafe { GlobalLock(handle) } as *mut u8;
    if target.is_null() {
        unsafe {
            GlobalFree(handle);
        }
        return Err(PlatformError::ClipboardUnavailable);
    }

    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), target, bytes.len());
        GlobalUnlock(handle);
    }

    Ok(handle)
}

fn string_to_null_terminated_utf16(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn utf16_to_le_bytes(input: &[u16]) -> Vec<u8> {
    input
        .iter()
        .flat_map(|unit| unit.to_le_bytes())
        .collect::<Vec<_>>()
}

#[cfg(test)]
fn le_bytes_to_utf16(input: &[u8]) -> Vec<u16> {
    input
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect()
}

#[cfg(test)]
fn utf16_until_nul_to_string(input: &[u16]) -> String {
    let len = input
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(input.len());
    String::from_utf16_lossy(&input[..len])
}

#[cfg(windows)]
fn keyboard_layouts() -> Vec<isize> {
    let count = unsafe { GetKeyboardLayoutList(0, std::ptr::null_mut()) };
    if count <= 0 {
        return Vec::new();
    }

    let mut layouts = vec![0isize; count as usize];
    let loaded = unsafe { GetKeyboardLayoutList(count, layouts.as_mut_ptr()) };
    if loaded <= 0 {
        return Vec::new();
    }

    layouts.truncate(loaded as usize);
    layouts
}

#[cfg(not(windows))]
fn keyboard_layouts() -> Vec<isize> {
    Vec::new()
}

fn find_layout_by_language(layouts: &[isize], language_id: u16) -> Option<isize> {
    layouts
        .iter()
        .copied()
        .find(|layout| ((*layout as u32) & 0xFFFF) as u16 == language_id)
}

#[cfg(windows)]
fn switch_foreground_layout(layout: isize) -> Result<(), PlatformError> {
    let foreground = foreground_hwnd()?;
    switch_window_layout(foreground, layout)
}

#[cfg(windows)]
fn switch_window_layout(hwnd: isize, layout: isize) -> Result<(), PlatformError> {
    if hwnd == 0 {
        return Err(PlatformError::ForegroundUnavailable);
    }
    post_layout_change_to_foreground_controls(hwnd, layout)?;

    for delay_ms in [40, 120, 220, 500, 900] {
        std::thread::sleep(std::time::Duration::from_millis(delay_ms));
        let thread_id = window_thread_id(hwnd)?;
        if unsafe { GetKeyboardLayout(thread_id) } == layout {
            return Ok(());
        }
        post_layout_change_to_foreground_controls(hwnd, layout)?;
    }

    Err(PlatformError::Unsupported)
}

#[cfg(not(windows))]
fn switch_foreground_layout(_layout: isize) -> Result<(), PlatformError> {
    Err(PlatformError::Unsupported)
}

#[cfg(windows)]
fn post_layout_change(hwnd: isize, layout: isize) -> Result<(), PlatformError> {
    unsafe {
        ActivateKeyboardLayout(layout, KLF_SETFORPROCESS);
    }
    let mut send_result = 0isize;
    let sent = unsafe {
        SendMessageTimeoutW(
            hwnd,
            WM_INPUTLANGCHANGEREQUEST,
            0,
            layout,
            SMTO_ABORTIFHUNG,
            100,
            &mut send_result as *mut isize,
        )
    };
    let posted = unsafe { PostMessageW(hwnd, WM_INPUTLANGCHANGEREQUEST, 0, layout) };
    if sent == 0 && posted == 0 {
        return Err(PlatformError::Unsupported);
    }

    Ok(())
}

#[cfg(windows)]
fn post_layout_change_to_foreground_controls(
    hwnd: isize,
    layout: isize,
) -> Result<(), PlatformError> {
    post_layout_change(hwnd, layout)?;
    if let Some(focused) = focused_window(hwnd) {
        if focused != hwnd {
            let _ = post_layout_change(focused, layout);
        }
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
    pending_scroll_lock: bool,
    win_down: bool,
    last_pause_at: Option<Instant>,
    last_scroll_lock_at: Option<Instant>,
    suppress_c_until: Option<Instant>,
    suspend_layout_controls_until: Option<Instant>,
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
                let ctrl_down = self.left_ctrl_down || self.right_ctrl_down;
                if is_down
                    && !self.pause_down
                    && Self::debounce_allows(if ctrl_down {
                        self.last_scroll_lock_at
                    } else {
                        self.last_pause_at
                    })
                {
                    self.pause_down = true;
                    let now = Instant::now();
                    if ctrl_down {
                        self.left_ctrl_used |= self.left_ctrl_down;
                        self.right_ctrl_used |= self.right_ctrl_down;
                        self.last_scroll_lock_at = Some(now);
                        self.suppress_c_until = Some(now + Duration::from_millis(1_500));
                        self.pending_scroll_lock = true;
                        return None;
                    }

                    self.last_pause_at = Some(now);
                    return Some(stepler_core::CorrectionMode::Pause);
                }
                if is_up {
                    self.pause_down = false;
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

        let ctrl_down = self.left_ctrl_down || self.right_ctrl_down;
        if is_down && !self.pause_down {
            self.pause_down = true;
            if ctrl_down {
                self.left_ctrl_used |= self.left_ctrl_down;
                self.right_ctrl_used |= self.right_ctrl_down;
                return TerminalPauseHandling::TranslateToCtrlF12;
            }
            return TerminalPauseHandling::PassThrough;
        }

        if is_up {
            self.pause_down = false;
            if ctrl_down {
                return TerminalPauseHandling::Suppress;
            }
        }

        if ctrl_down {
            TerminalPauseHandling::Suppress
        } else {
            TerminalPauseHandling::PassThrough
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

        let ctrl_down = self.left_ctrl_down || self.right_ctrl_down;
        if is_down
            && !self.pause_down
            && Self::debounce_allows(if ctrl_down {
                self.last_scroll_lock_at
            } else {
                self.last_pause_at
            })
        {
            self.pause_down = true;
            let now = Instant::now();
            if ctrl_down {
                self.left_ctrl_used |= self.left_ctrl_down;
                self.right_ctrl_used |= self.right_ctrl_down;
                self.last_scroll_lock_at = Some(now);
                self.suppress_c_until = Some(now + Duration::from_millis(1_500));
                return Some(stepler_core::CorrectionMode::ScrollLock);
            }

            self.last_pause_at = Some(now);
            return Some(stepler_core::CorrectionMode::Pause);
        }

        if is_up {
            self.pause_down = false;
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
                    let was_used = self.left_ctrl_used;
                    self.left_ctrl_down = false;
                    self.left_ctrl_used = false;
                    if self.pending_scroll_lock {
                        return None;
                    }
                    let action = (!was_used).then_some(KeyboardControlAction::SwitchToRussian);
                    return action;
                }
                VK_RCONTROL => {
                    let was_used = self.right_ctrl_used;
                    self.right_ctrl_down = false;
                    self.right_ctrl_used = false;
                    if self.pending_scroll_lock {
                        return None;
                    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TerminalPauseHandling {
    PassThrough,
    Suppress,
    TranslateToCtrlF12,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TerminalPassthrough {
    None,
    PsReadLine,
    Ssh,
    UnknownTerminal,
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
            append_hotkey_signal_log(&format!(
                "hook_terminal_conservative_suppressed vk={vk_code} down={is_down} up={is_up}"
            ));
            return 1;
        }
        if terminal_passthrough == TerminalPassthrough::Ssh
            && is_down
            && env_flag_enabled("STEPLER_ENABLE_SSH_REMOTE_ADAPTER", false)
        {
            let mode = KEYBOARD_CONTROL_STATE
                .get_or_init(|| Mutex::new(KeyboardControlHookState::default()))
                .lock()
                .ok()
                .map(|mut state| {
                    let ctrl_down = state.left_ctrl_down || state.right_ctrl_down;
                    state.pause_down = true;
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
                "hook_ssh_terminal_forwarded mode={mode:?} vk={vk_code} down={is_down} up={is_up}"
            ));
            send_ssh_terminal_sequence(mode);
            return 1;
        }
        if matches!(
            terminal_passthrough,
            TerminalPassthrough::Ssh | TerminalPassthrough::UnknownTerminal
        ) {
            append_hotkey_signal_log(&format!(
                "hook_terminal_suppressed kind={terminal_passthrough:?} vk={vk_code} down={is_down} up={is_up}"
            ));
            let _ = KEYBOARD_CONTROL_STATE
                .get_or_init(|| Mutex::new(KeyboardControlHookState::default()))
                .lock()
                .map(|mut state| {
                    if is_down {
                        state.pause_down = true;
                        state.left_ctrl_used |= state.left_ctrl_down;
                        state.right_ctrl_used |= state.right_ctrl_down;
                    }
                    if is_up {
                        state.pause_down = false;
                    }
                });
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
                TerminalPauseHandling::TranslateToCtrlF12 => {
                    send_key_virtual(VK_F12);
                    return 1;
                }
            }
        }
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
    if should_ignore_keyboard_hook_event(event) {
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
            return CallNextHookEx(0, code, wparam, lparam);
        }
        if let Some(thread_id) = KEYBOARD_CONTROL_THREAD_ID.get().copied() {
            PostThreadMessageW(
                thread_id,
                WM_STEPLER_KEYBOARD_CONTROL,
                action.message_id(),
                0,
            );
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
    event.flags & LLKHF_INJECTED != 0 && event.extra_info != STEPLER_INJECTED_CONTROL_MAGIC
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
struct ClipboardGuard;

#[cfg(windows)]
impl ClipboardGuard {
    fn open() -> Result<Self, PlatformError> {
        let started = Instant::now();
        while started.elapsed() < Duration::from_millis(450) {
            let opened = unsafe { OpenClipboard(0) };
            if opened != 0 {
                return Ok(Self);
            }

            std::thread::sleep(Duration::from_millis(10));
        }

        Err(PlatformError::ClipboardUnavailable)
    }
}

#[cfg(windows)]
impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        unsafe {
            CloseClipboard();
        }
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
struct Input {
    input_type: u32,
    input: InputUnion,
}

#[cfg(windows)]
impl Input {
    fn keyboard_scan_code(vk: u32, key_up: bool, extra_info: usize) -> Self {
        let scan_code = unsafe { MapVirtualKeyW(vk, MAPVK_VK_TO_VSC_EX) };
        let extended = scan_code & 0xFF00 != 0 || is_extended_navigation_key(vk);
        let mut flags = KEYEVENTF_SCANCODE;
        if key_up {
            flags |= KEYEVENTF_KEYUP;
        }
        if extended {
            flags |= KEYEVENTF_EXTENDEDKEY;
        }

        Self {
            input_type: INPUT_KEYBOARD,
            input: InputUnion {
                ki: KeybdInput {
                    vk: 0,
                    scan: (scan_code & 0xFF) as u16,
                    flags,
                    time: 0,
                    extra_info,
                },
            },
        }
    }

    fn keyboard_virtual_key(vk: u32, key_up: bool, extra_info: usize) -> Self {
        let mut flags = 0;
        if key_up {
            flags |= KEYEVENTF_KEYUP;
        }
        if is_extended_navigation_key(vk) || matches!(vk, VK_RCONTROL | VK_RMENU) {
            flags |= KEYEVENTF_EXTENDEDKEY;
        }

        Self {
            input_type: INPUT_KEYBOARD,
            input: InputUnion {
                ki: KeybdInput {
                    vk: vk as u16,
                    scan: 0,
                    flags,
                    time: 0,
                    extra_info,
                },
            },
        }
    }

    fn keyboard_unicode(unit: u16, key_up: bool) -> Self {
        let mut flags = KEYEVENTF_UNICODE;
        if key_up {
            flags |= KEYEVENTF_KEYUP;
        }

        Self {
            input_type: INPUT_KEYBOARD,
            input: InputUnion {
                ki: KeybdInput {
                    vk: 0,
                    scan: unit,
                    flags,
                    time: 0,
                    extra_info: 0,
                },
            },
        }
    }
}

#[cfg(windows)]
fn is_extended_navigation_key(vk: u32) -> bool {
    matches!(
        vk,
        VK_HOME
            | VK_END
            | VK_INSERT
            | VK_DELETE
            | VK_LEFT
            | VK_RIGHT
            | VK_UP
            | VK_DOWN
            | VK_PRIOR
            | VK_NEXT
            | VK_DIVIDE
            | VK_NUMLOCK
    )
}

#[cfg(windows)]
#[repr(C)]
union InputUnion {
    ki: KeybdInput,
    padding: [u8; 32],
}

#[cfg(windows)]
#[repr(C)]
#[derive(Clone, Copy)]
struct KeybdInput {
    vk: u16,
    scan: u16,
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
