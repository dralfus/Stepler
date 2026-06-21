use super::*;

#[cfg(windows)]
pub(super) fn foreground_hwnd() -> Result<isize, PlatformError> {
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
pub(super) fn test_foreground_hwnd_override() -> Option<isize> {
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
pub(super) fn window_thread_id(hwnd: isize) -> Result<u32, PlatformError> {
    let thread_id = unsafe { GetWindowThreadProcessId(hwnd, std::ptr::null_mut()) };
    if thread_id == 0 {
        return Err(PlatformError::ForegroundUnavailable);
    }
    Ok(thread_id)
}

#[cfg(windows)]
pub(super) fn window_class_name(hwnd: isize) -> Option<String> {
    let mut buffer = [0u16; 256];
    let length = unsafe { GetClassNameW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32) };
    if length <= 0 {
        return None;
    }

    Some(String::from_utf16_lossy(&buffer[..length as usize]))
}

#[cfg(windows)]
pub(super) fn window_title(hwnd: isize) -> Option<String> {
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
pub(super) fn window_process_name(hwnd: isize) -> Option<String> {
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

pub(super) fn hwnd_id(hwnd: isize) -> String {
    format!("hwnd:{hwnd:X}")
}

#[cfg(windows)]
pub(super) fn window_process_id(hwnd: isize) -> Result<u32, PlatformError> {
    let mut process_id = 0u32;
    let thread_id = unsafe { GetWindowThreadProcessId(hwnd, &mut process_id as *mut u32) };
    if thread_id == 0 || process_id == 0 {
        return Err(PlatformError::ForegroundUnavailable);
    }
    Ok(process_id)
}

pub(super) fn parse_hwnd_id(value: &str) -> Option<isize> {
    let hex = value.strip_prefix("hwnd:")?;
    if hex.is_empty() {
        return None;
    }

    isize::from_str_radix(hex, 16).ok()
}

#[cfg(windows)]
pub(super) fn foreground_keyboard_layout() -> Result<isize, PlatformError> {
    let foreground = foreground_hwnd()?;
    let thread_id = window_thread_id(foreground)?;
    Ok(unsafe { GetKeyboardLayout(thread_id) })
}

#[cfg(windows)]
pub(super) fn focused_window(foreground: isize) -> Option<isize> {
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
