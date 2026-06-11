use super::*;

#[cfg(windows)]
pub(super) fn send_key(vk: u32) {
    let _ = send_keyboard_input(&[
        KeyboardInputEvent::new(vk, false, KeyboardInputMode::ScanCode),
        KeyboardInputEvent::new(vk, true, KeyboardInputMode::ScanCode),
    ]);
}

#[cfg(windows)]
pub(super) fn send_key_virtual(vk: u32) {
    let _ = send_keyboard_input(&[
        KeyboardInputEvent::new(vk, false, KeyboardInputMode::VirtualKey),
        KeyboardInputEvent::new(vk, true, KeyboardInputMode::VirtualKey),
    ]);
}

#[cfg(windows)]
pub(super) fn send_stepler_control_key(vk: u32) -> Result<(), PlatformError> {
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
pub(super) fn send_key_chord(modifiers: &[u32], key: u32) {
    send_key_chord_with_mode(modifiers, key, KeyboardInputMode::ScanCode);
}

#[cfg(windows)]
pub(super) fn send_key_chord_virtual(modifiers: &[u32], key: u32) {
    send_key_chord_mixed(modifiers, key);
}

#[cfg(windows)]
pub(super) fn send_terminal_shortcut_with_english_layout(modifiers: &[u32], key: u32) {
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
pub(super) fn send_ssh_terminal_sequence(mode: stepler_core::CorrectionMode) {
    let sequence = match mode {
        stepler_core::CorrectionMode::Pause => "\u{1b}[777;1u",
        stepler_core::CorrectionMode::ScrollLock => "\u{1b}[777;2u",
    };
    let _ = send_unicode_text(sequence);
}

#[cfg(windows)]
pub(super) fn send_key_chord_with_mode(modifiers: &[u32], key: u32, mode: KeyboardInputMode) {
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
pub(super) fn send_key_chord_mixed(modifiers: &[u32], key: u32) {
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
pub(super) fn send_keyboard_input(events: &[KeyboardInputEvent]) -> bool {
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
pub(super) fn send_unicode_text(text: &str) -> Result<(), PlatformError> {
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
pub(super) struct KeyboardInputEvent {
    vk: u32,
    key_up: bool,
    mode: KeyboardInputMode,
    extra_info: usize,
}

#[cfg(windows)]
impl KeyboardInputEvent {
    pub(super) fn new(vk: u32, key_up: bool, mode: KeyboardInputMode) -> Self {
        Self {
            vk,
            key_up,
            mode,
            extra_info: 0,
        }
    }

    pub(super) fn new_with_extra(
        vk: u32,
        key_up: bool,
        mode: KeyboardInputMode,
        extra_info: usize,
    ) -> Self {
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
pub(super) enum KeyboardInputMode {
    ScanCode,
    VirtualKey,
}

#[cfg(windows)]
#[repr(C)]
pub(super) struct Input {
    input_type: u32,
    pub(super) input: InputUnion,
}

#[cfg(windows)]
impl Input {
    pub(super) fn keyboard_scan_code(vk: u32, key_up: bool, extra_info: usize) -> Self {
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

    pub(super) fn keyboard_virtual_key(vk: u32, key_up: bool, extra_info: usize) -> Self {
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

    pub(super) fn keyboard_unicode(unit: u16, key_up: bool) -> Self {
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
pub(super) fn is_extended_navigation_key(vk: u32) -> bool {
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
pub(super) union InputUnion {
    pub(super) ki: KeybdInput,
    padding: [u8; 32],
}

#[cfg(windows)]
#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct KeybdInput {
    pub(super) vk: u16,
    pub(super) scan: u16,
    pub(super) flags: u32,
    pub(super) time: u32,
    pub(super) extra_info: usize,
}
