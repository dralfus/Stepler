use super::*;

#[derive(Debug, Default)]
pub struct WindowsLayoutSwitcher {
    layouts: Vec<isize>,
    ordered_layouts: Vec<isize>,
    russian_layout: Option<isize>,
    english_layout: Option<isize>,
}

#[derive(Debug)]
pub struct PendingOutlookLayoutSwitch {
    foreground: isize,
    focused: isize,
    process_id: u32,
    target_layout: isize,
    started: std::time::Instant,
    dispatch_count: usize,
    max_dispatches: usize,
}

#[derive(Debug)]
pub struct PendingWindowLayoutSwitch {
    foreground: isize,
    focused: isize,
    foreground_process_id: u32,
    focused_process_id: u32,
    dispatch_hwnd: isize,
    dispatch_process_id: u32,
    target_layout: isize,
    started: std::time::Instant,
    dispatch_count: usize,
}

#[derive(Debug)]
pub enum PendingLayoutSwitch {
    Outlook(PendingOutlookLayoutSwitch),
    Window(PendingWindowLayoutSwitch),
}

impl PendingLayoutSwitch {
    #[cfg(windows)]
    pub fn complete(self) -> Result<(), PlatformError> {
        match self {
            Self::Outlook(pending) => complete_outlook_layout_change(pending),
            Self::Window(pending) => complete_window_layout_change(pending),
        }
    }

    #[cfg(not(windows))]
    pub fn complete(self) -> Result<(), PlatformError> {
        Err(PlatformError::Unsupported)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LayoutTransport {
    OutlookSystemHotkey,
    WindowMessage,
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
        self.english_layout = find_layout_by_primary_language(&self.layouts, LANG_ENGLISH_PRIMARY);
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

    #[cfg(windows)]
    pub fn begin_layout_action(
        &self,
        action: KeyboardControlAction,
        hwnd_hint: Option<isize>,
    ) -> Result<PendingLayoutSwitch, PlatformError> {
        let layout = match action {
            KeyboardControlAction::SwitchToRussian => self.russian_layout,
            KeyboardControlAction::SwitchToEnglish => self.english_layout,
            KeyboardControlAction::SwitchToNext => None,
        };
        let layout = layout.ok_or(PlatformError::Unsupported)?;
        let foreground = foreground_hwnd()?;
        if !outlook_layout_target(foreground) {
            return begin_window_layout_change(foreground, hwnd_hint, layout)
                .map(PendingLayoutSwitch::Window);
        }
        begin_outlook_layout_change(foreground, layout).map(PendingLayoutSwitch::Outlook)
    }

    #[cfg(not(windows))]
    pub fn begin_layout_action(
        &self,
        _action: KeyboardControlAction,
        _hwnd_hint: Option<isize>,
    ) -> Result<PendingLayoutSwitch, PlatformError> {
        Err(PlatformError::Unsupported)
    }
}

#[cfg(windows)]
pub(super) fn keyboard_layouts() -> Vec<isize> {
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
pub(super) fn keyboard_layouts() -> Vec<isize> {
    Vec::new()
}

pub(super) fn find_layout_by_language(layouts: &[isize], language_id: u16) -> Option<isize> {
    layouts
        .iter()
        .copied()
        .find(|layout| ((*layout as u32) & 0xFFFF) as u16 == language_id)
}

pub(super) fn find_layout_by_primary_language(
    layouts: &[isize],
    primary_language_id: u16,
) -> Option<isize> {
    layouts.iter().copied().find(|layout| {
        let language_id = ((*layout as u32) & 0xFFFF) as u16;
        language_id & 0x03FF == primary_language_id
    })
}

#[cfg(windows)]
pub(super) fn switch_foreground_layout(layout: isize) -> Result<(), PlatformError> {
    let foreground = foreground_hwnd()?;
    switch_window_layout(foreground, layout)
}

#[cfg(windows)]
pub(super) fn switch_window_layout(hwnd: isize, layout: isize) -> Result<(), PlatformError> {
    if hwnd == 0 {
        return Err(PlatformError::ForegroundUnavailable);
    }
    if outlook_layout_target(hwnd) {
        let pending = begin_outlook_layout_change(hwnd, layout)?;
        return complete_outlook_layout_change(pending);
    }

    post_layout_change_to_foreground_controls(hwnd, layout)?;

    for delay_ms in [40, 120, 220, 500, 900] {
        std::thread::sleep(std::time::Duration::from_millis(delay_ms));
        let thread_id = window_thread_id(hwnd)?;
        let hwnd_matches = unsafe { GetKeyboardLayout(thread_id) } == layout;
        append_hotkey_signal_log(&format!(
            "layout_verify hwnd={} layout={layout:X} hwnd_matches={hwnd_matches}",
            hwnd_id(hwnd)
        ));
        if hwnd_matches {
            return Ok(());
        }
        post_layout_change_to_foreground_controls(hwnd, layout)?;
    }

    Err(PlatformError::Unsupported)
}

#[cfg(not(windows))]
pub(super) fn switch_foreground_layout(_layout: isize) -> Result<(), PlatformError> {
    Err(PlatformError::Unsupported)
}

#[cfg(windows)]
fn outlook_layout_target(hwnd: isize) -> bool {
    let app_class = window_class_name(hwnd).unwrap_or_default();
    let focused_class = focused_window(hwnd)
        .and_then(window_class_name)
        .unwrap_or_default();
    let process_name = window_process_name(hwnd);
    layout_transport_for_surface(&app_class, &focused_class, process_name.as_deref())
        == LayoutTransport::OutlookSystemHotkey
}

fn layout_transport_for_surface(
    app_class: &str,
    focused_class: &str,
    process_name: Option<&str>,
) -> LayoutTransport {
    if is_outlook_class_or_process(app_class, focused_class, process_name) {
        LayoutTransport::OutlookSystemHotkey
    } else {
        LayoutTransport::WindowMessage
    }
}

#[cfg(windows)]
fn begin_window_layout_change(
    foreground: isize,
    hwnd_hint: Option<isize>,
    layout: isize,
) -> Result<PendingWindowLayoutSwitch, PlatformError> {
    let focused = focused_window(foreground).unwrap_or(foreground);
    let foreground_process_id = window_process_id(foreground)?;
    let focused_process_id = window_process_id(focused)?;
    let dispatch_hwnd = hwnd_hint
        .filter(|hwnd| *hwnd != 0)
        .filter(|hwnd| {
            window_process_id(*hwnd).is_ok_and(|process_id| {
                process_id == foreground_process_id || process_id == focused_process_id
            })
        })
        .unwrap_or(foreground);
    let dispatch_process_id = window_process_id(dispatch_hwnd)?;
    let current_layout = unsafe { GetKeyboardLayout(window_thread_id(focused)?) };
    let mut pending = PendingWindowLayoutSwitch {
        foreground,
        focused,
        foreground_process_id,
        focused_process_id,
        dispatch_hwnd,
        dispatch_process_id,
        target_layout: layout,
        started: std::time::Instant::now(),
        dispatch_count: 0,
    };

    if current_layout != layout {
        dispatch_window_layout(&mut pending)?;
    } else {
        append_hotkey_signal_log(&format!(
            "layout_ready hwnd={} focused={} layout={layout:X} transport=window_message already_active=true",
            hwnd_id(foreground),
            hwnd_id(focused)
        ));
    }
    Ok(pending)
}

#[cfg(windows)]
fn dispatch_window_layout(pending: &mut PendingWindowLayoutSwitch) -> Result<(), PlatformError> {
    ensure_window_layout_snapshot(pending)?;
    let mut sent = false;
    let mut seen = Vec::with_capacity(3);
    for hwnd in [pending.dispatch_hwnd, pending.foreground, pending.focused] {
        if hwnd == 0 || seen.contains(&hwnd) {
            continue;
        }
        seen.push(hwnd);
        sent |= post_layout_change(hwnd, pending.target_layout).is_ok();
    }
    if !sent {
        return Err(PlatformError::Unsupported);
    }
    pending.dispatch_count += 1;
    append_hotkey_signal_log(&format!(
        "layout_dispatch hwnd={} focused={} target={:X} transport=window_message attempt={}",
        hwnd_id(pending.foreground),
        hwnd_id(pending.focused),
        pending.target_layout,
        pending.dispatch_count
    ));
    Ok(())
}

#[cfg(windows)]
fn complete_window_layout_change(
    mut pending: PendingWindowLayoutSwitch,
) -> Result<(), PlatformError> {
    const DEADLINE_MS: u64 = 350;
    const RETRY_AFTER_MS: u128 = 70;
    const MAX_DISPATCHES: usize = 4;
    let mut last_dispatch_at = pending.started;

    loop {
        ensure_window_layout_snapshot(&pending)?;
        let current = unsafe { GetKeyboardLayout(window_thread_id(pending.focused)?) };
        if current == pending.target_layout {
            append_hotkey_signal_log(&format!(
                "layout_verified hwnd={} focused={} target={:X} elapsed_ms={} attempts={}",
                hwnd_id(pending.foreground),
                hwnd_id(pending.focused),
                pending.target_layout,
                pending.started.elapsed().as_millis(),
                pending.dispatch_count
            ));
            return Ok(());
        }
        if pending.started.elapsed() >= std::time::Duration::from_millis(DEADLINE_MS) {
            append_hotkey_signal_log(&format!(
                "layout_failed hwnd={} focused={} target={:X} actual={current:X} reason=deadline elapsed_ms={} attempts={}",
                hwnd_id(pending.foreground),
                hwnd_id(pending.focused),
                pending.target_layout,
                pending.started.elapsed().as_millis(),
                pending.dispatch_count
            ));
            return Err(PlatformError::Unsupported);
        }
        if last_dispatch_at.elapsed().as_millis() >= RETRY_AFTER_MS
            && pending.dispatch_count < MAX_DISPATCHES
        {
            dispatch_window_layout(&mut pending)?;
            last_dispatch_at = std::time::Instant::now();
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[cfg(windows)]
fn ensure_window_layout_snapshot(pending: &PendingWindowLayoutSwitch) -> Result<(), PlatformError> {
    let foreground = foreground_hwnd()?;
    let focused = focused_window(foreground).unwrap_or(foreground);
    let stable = foreground == pending.foreground
        && focused == pending.focused
        && window_process_id(foreground)? == pending.foreground_process_id
        && window_process_id(focused)? == pending.focused_process_id
        && window_process_id(pending.dispatch_hwnd)? == pending.dispatch_process_id;
    if stable {
        return Ok(());
    }

    append_hotkey_signal_log(&format!(
        "layout_abort expected_foreground={} actual_foreground={} expected_focus={} actual_focus={} reason=snapshot_changed",
        hwnd_id(pending.foreground),
        hwnd_id(foreground),
        hwnd_id(pending.focused),
        hwnd_id(focused)
    ));
    Err(PlatformError::ForegroundUnavailable)
}

#[cfg(windows)]
fn begin_outlook_layout_change(
    hwnd: isize,
    layout: isize,
) -> Result<PendingOutlookLayoutSwitch, PlatformError> {
    let foreground = foreground_hwnd()?;
    let process_id = window_process_id(hwnd)?;
    if window_process_id(foreground)? != process_id {
        append_hotkey_signal_log(&format!(
            "outlook_layout_abort reason=foreground_changed expected={} actual={}",
            hwnd_id(hwnd),
            hwnd_id(foreground)
        ));
        return Err(PlatformError::ForegroundUnavailable);
    }

    let Some(focused) = focused_window(foreground) else {
        append_hotkey_signal_log(&format!(
            "outlook_layout_abort hwnd={} reason=no_focus",
            hwnd_id(foreground)
        ));
        return Err(PlatformError::ForegroundUnavailable);
    };
    if window_process_id(focused)? != process_id {
        append_hotkey_signal_log(&format!(
            "outlook_layout_abort hwnd={} focused={} reason=focus_process_changed",
            hwnd_id(foreground),
            hwnd_id(focused)
        ));
        return Err(PlatformError::ForegroundUnavailable);
    }
    let focused_class = window_class_name(focused).unwrap_or_default();
    if !is_outlook_editable_focus_class(&focused_class) {
        append_hotkey_signal_log(&format!(
            "outlook_layout_abort hwnd={} focused={} class={} reason=noneditable_focus",
            hwnd_id(foreground),
            hwnd_id(focused),
            focused_class
        ));
        return Err(PlatformError::Unsupported);
    }

    let focused_thread = window_thread_id(focused)?;
    let current_layout = unsafe { GetKeyboardLayout(focused_thread) };
    let mut pending = PendingOutlookLayoutSwitch {
        foreground,
        focused,
        process_id,
        target_layout: layout,
        started: std::time::Instant::now(),
        dispatch_count: 0,
        max_dispatches: keyboard_layouts().len().clamp(1, 6),
    };
    if current_layout != layout {
        dispatch_outlook_layout_next(&mut pending)?;
    } else {
        append_hotkey_signal_log(&format!(
            "outlook_layout_ready hwnd={} focused={} layout={layout:X} transport=system_hotkey already_active=true",
            hwnd_id(foreground),
            hwnd_id(focused)
        ));
    }
    Ok(pending)
}

fn is_outlook_editable_focus_class(class_name: &str) -> bool {
    class_name.eq_ignore_ascii_case("_WwG")
        || class_name.eq_ignore_ascii_case("edit")
        || class_name.to_ascii_lowercase().starts_with("richedit")
}

#[cfg(windows)]
fn dispatch_outlook_layout_next(
    pending: &mut PendingOutlookLayoutSwitch,
) -> Result<(), PlatformError> {
    ensure_outlook_layout_snapshot(pending)?;
    send_system_layout_next()?;
    pending.dispatch_count += 1;
    append_hotkey_signal_log(&format!(
        "outlook_layout_dispatch hwnd={} focused={} target={:X} transport=system_hotkey attempt={}",
        hwnd_id(pending.foreground),
        hwnd_id(pending.focused),
        pending.target_layout,
        pending.dispatch_count
    ));
    Ok(())
}

#[cfg(windows)]
fn complete_outlook_layout_change(
    mut pending: PendingOutlookLayoutSwitch,
) -> Result<(), PlatformError> {
    const DEADLINE_MS: u64 = 250;
    const RETRY_AFTER_MS: u128 = 70;
    let mut last_dispatch_at = pending.started;

    loop {
        ensure_outlook_layout_snapshot(&pending)?;
        let current = unsafe { GetKeyboardLayout(window_thread_id(pending.focused)?) };
        if current == pending.target_layout {
            append_hotkey_signal_log(&format!(
                "outlook_layout_verified hwnd={} focused={} target={:X} elapsed_ms={} attempts={}",
                hwnd_id(pending.foreground),
                hwnd_id(pending.focused),
                pending.target_layout,
                pending.started.elapsed().as_millis(),
                pending.dispatch_count
            ));
            return Ok(());
        }

        if pending.started.elapsed() >= std::time::Duration::from_millis(DEADLINE_MS) {
            append_hotkey_signal_log(&format!(
                "outlook_layout_failed hwnd={} focused={} target={:X} actual={current:X} reason=deadline elapsed_ms={} attempts={}",
                hwnd_id(pending.foreground),
                hwnd_id(pending.focused),
                pending.target_layout,
                pending.started.elapsed().as_millis(),
                pending.dispatch_count
            ));
            return Err(PlatformError::Unsupported);
        }

        if last_dispatch_at.elapsed().as_millis() >= RETRY_AFTER_MS
            && pending.dispatch_count < pending.max_dispatches
        {
            dispatch_outlook_layout_next(&mut pending)?;
            last_dispatch_at = std::time::Instant::now();
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[cfg(windows)]
fn ensure_outlook_layout_snapshot(
    pending: &PendingOutlookLayoutSwitch,
) -> Result<(), PlatformError> {
    let foreground = foreground_hwnd()?;
    let focused = focused_window(foreground).ok_or(PlatformError::ForegroundUnavailable)?;
    let stable = foreground == pending.foreground
        && focused == pending.focused
        && window_process_id(foreground)? == pending.process_id
        && window_process_id(focused)? == pending.process_id;
    if stable {
        return Ok(());
    }

    append_hotkey_signal_log(&format!(
        "outlook_layout_abort expected_foreground={} actual_foreground={} expected_focus={} actual_focus={} reason=snapshot_changed",
        hwnd_id(pending.foreground),
        hwnd_id(foreground),
        hwnd_id(pending.focused),
        hwnd_id(focused)
    ));
    Err(PlatformError::ForegroundUnavailable)
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
    append_hotkey_signal_log(&format!(
        "layout_post hwnd={} layout={layout:X} sent={sent} posted={posted}",
        hwnd_id(hwnd)
    ));

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

#[cfg(test)]
mod tests {
    use super::{
        find_layout_by_language, find_layout_by_primary_language, is_outlook_editable_focus_class,
        layout_transport_for_surface, LayoutTransport, PlatformError, WindowsLayoutSwitcher,
    };

    #[test]
    fn english_primary_language_matches_us_and_uk_variants() {
        assert_eq!(
            find_layout_by_primary_language(&[0x0419, 0x0809], 0x0009),
            Some(0x0809)
        );
        assert_eq!(
            find_layout_by_primary_language(&[0x0409, 0x0419], 0x0009),
            Some(0x0409)
        );
    }

    #[test]
    fn primary_language_match_preserves_windows_order_for_multiple_english_variants() {
        assert_eq!(
            find_layout_by_primary_language(&[0x0C09, 0x0809, 0x0409], 0x0009),
            Some(0x0C09)
        );
    }

    #[test]
    fn primary_language_match_does_not_treat_russian_as_english() {
        assert_eq!(find_layout_by_primary_language(&[0x0419], 0x0009), None);
        assert_eq!(find_layout_by_language(&[0x0419], 0x0419), Some(0x0419));
    }

    #[test]
    fn missing_english_layout_fails_closed() {
        let switcher = WindowsLayoutSwitcher::default();

        assert!(matches!(
            switcher.switch_to_english(),
            Err(PlatformError::Unsupported)
        ));
    }

    #[test]
    fn outlook_layout_surface_is_recognized() {
        assert_eq!(
            layout_transport_for_surface("rctrl_renwnd32", "_WwG", Some("OUTLOOK")),
            LayoutTransport::OutlookSystemHotkey
        );
    }

    #[test]
    fn outlook_layout_allows_only_editable_focus_classes() {
        assert!(is_outlook_editable_focus_class("_WwG"));
        assert!(is_outlook_editable_focus_class("RICHEDIT60W"));
        assert!(is_outlook_editable_focus_class("RichEdit20WPT"));
        assert!(is_outlook_editable_focus_class("Edit"));
        assert!(!is_outlook_editable_focus_class("OutlookGrid"));
        assert!(!is_outlook_editable_focus_class("Button"));
    }

    #[test]
    fn unrelated_editor_is_not_an_outlook_layout_surface() {
        assert_eq!(
            layout_transport_for_surface(
                "Chrome_WidgetWin_1",
                "Chrome_RenderWidgetHostHWND",
                Some("chrome")
            ),
            LayoutTransport::WindowMessage
        );
    }
}
