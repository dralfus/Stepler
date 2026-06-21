use super::*;

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
