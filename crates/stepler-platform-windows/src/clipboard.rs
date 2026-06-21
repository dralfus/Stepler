use super::*;

#[cfg(windows)]
pub(super) fn capture_clipboard_text_only() -> Result<ClipboardSnapshot, PlatformError> {
    capture_clipboard_text_only_with_timeout(Duration::from_millis(450))
}

#[cfg(windows)]
pub(super) fn capture_clipboard_text_only_with_timeout(
    timeout: Duration,
) -> Result<ClipboardSnapshot, PlatformError> {
    let _guard = ClipboardGuard::open(timeout)?;
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
pub(super) fn restore_clipboard_text_only(
    snapshot: &ClipboardSnapshot,
) -> Result<(), PlatformError> {
    restore_clipboard_text_only_with_timeout(snapshot, Duration::from_millis(450))
}

#[cfg(windows)]
pub(super) fn restore_clipboard_text_only_with_timeout(
    snapshot: &ClipboardSnapshot,
    timeout: Duration,
) -> Result<(), PlatformError> {
    if let Some(text) = &snapshot.text {
        restore_clipboard_with_timeout(clipboard_snapshot_from_text(text), timeout)
    } else {
        let _guard = ClipboardGuard::open(timeout)?;
        unsafe {
            if EmptyClipboard() == 0 {
                return Err(PlatformError::ClipboardUnavailable);
            }
        }
        Ok(())
    }
}

#[cfg(windows)]
pub(super) fn clipboard_snapshot_from_text(text: &str) -> ClipboardSnapshot {
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
pub(super) fn capture_clipboard() -> Result<ClipboardSnapshot, PlatformError> {
    let _guard = ClipboardGuard::open(Duration::from_millis(450))?;
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
pub(super) fn capture_clipboard() -> Result<ClipboardSnapshot, PlatformError> {
    Err(PlatformError::Unsupported)
}

#[cfg(windows)]
pub(super) fn restore_clipboard(snapshot: ClipboardSnapshot) -> Result<(), PlatformError> {
    restore_clipboard_with_timeout(snapshot, Duration::from_millis(450))
}

#[cfg(windows)]
pub(super) fn restore_clipboard_with_timeout(
    snapshot: ClipboardSnapshot,
    timeout: Duration,
) -> Result<(), PlatformError> {
    let _guard = ClipboardGuard::open(timeout)?;
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
pub(super) fn restore_clipboard(_snapshot: ClipboardSnapshot) -> Result<(), PlatformError> {
    Err(PlatformError::Unsupported)
}

#[cfg(windows)]
pub(super) fn read_clipboard_text() -> Result<String, PlatformError> {
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
pub(super) fn clipboard_formats() -> Vec<u32> {
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
pub(super) fn read_clipboard_format_bytes(format: u32) -> Option<Vec<u8>> {
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
pub(super) fn global_alloc_from_bytes(bytes: &[u8]) -> Result<isize, PlatformError> {
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

pub(super) fn string_to_null_terminated_utf16(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

pub(super) fn utf16_to_le_bytes(input: &[u16]) -> Vec<u8> {
    input
        .iter()
        .flat_map(|unit| unit.to_le_bytes())
        .collect::<Vec<_>>()
}

#[cfg(test)]
pub(super) fn le_bytes_to_utf16(input: &[u8]) -> Vec<u16> {
    input
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .collect()
}

#[cfg(test)]
pub(super) fn utf16_until_nul_to_string(input: &[u16]) -> String {
    let len = input
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(input.len());
    String::from_utf16_lossy(&input[..len])
}

#[cfg(windows)]
struct ClipboardGuard;

#[cfg(windows)]
impl ClipboardGuard {
    fn open(timeout: Duration) -> Result<Self, PlatformError> {
        let started = Instant::now();
        while started.elapsed() < timeout {
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
