#![cfg(windows)]

use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[test]
#[ignore = "launches Notepad and installs global hotkeys; run manually on Windows"]
fn scrolllock_notepad_preserves_clipboard() {
    let mut runner = RunnerProcess::start();
    let mut notepad = NotepadProcess::start();

    let edit = wait_for_focused_control(notepad.hwnd, Duration::from_secs(5));
    assert_ne!(edit, 0, "focused Notepad edit control was not found");

    set_window_text(edit, "COPYME PHRASE\r\nghbdtn vbh");
    set_clipboard_text("one two.");
    let before = clipboard_text();

    let len = send_message(edit, WM_GETTEXTLENGTH, 0, 0) as usize;
    send_message(edit, EM_SETSEL, len, len);
    set_foreground(notepad.hwnd);
    thread::sleep(Duration::from_millis(250));
    send_key(VK_SCROLL);
    thread::sleep(Duration::from_millis(2_500));

    let after = clipboard_text();

    notepad.stop();
    runner.stop();

    assert_eq!(before, "one two.");
    assert_eq!(after, before);
}

struct RunnerProcess {
    child: Child,
}

impl RunnerProcess {
    fn start() -> Self {
        let exe = env!("CARGO_BIN_EXE_stepler-cli");
        let child = Command::new(exe)
            .arg("run-hotkeys")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("failed to start stepler-cli run-hotkeys");
        thread::sleep(Duration::from_millis(800));
        Self { child }
    }

    fn stop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for RunnerProcess {
    fn drop(&mut self) {
        self.stop();
    }
}

struct NotepadProcess {
    child: Child,
    hwnd: isize,
}

impl NotepadProcess {
    fn start() -> Self {
        let child = Command::new("notepad.exe")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("failed to start Notepad");
        let hwnd = wait_for_notepad_window(Duration::from_secs(5));
        assert_ne!(hwnd, 0, "Notepad window was not found");
        set_foreground(hwnd);
        thread::sleep(Duration::from_millis(700));
        Self { child, hwnd }
    }

    fn stop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for NotepadProcess {
    fn drop(&mut self) {
        self.stop();
    }
}

fn wait_for_notepad_window(timeout: Duration) -> isize {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if let Some(hwnd) = find_window_by_class("Notepad") {
            return hwnd;
        }
        thread::sleep(Duration::from_millis(100));
    }
    0
}

fn wait_for_focused_control(root: isize, timeout: Duration) -> isize {
    let started = Instant::now();
    while started.elapsed() < timeout {
        set_foreground(root);
        if let Some(hwnd) = focused_window(root) {
            let class_name = class_name(hwnd);
            if class_name == "Edit" || class_name.starts_with("RichEdit") {
                return hwnd;
            }
        }
        thread::sleep(Duration::from_millis(100));
    }
    0
}

fn find_window_by_class(class: &str) -> Option<isize> {
    let mut state = WindowSearch {
        target: class.to_owned(),
        found: 0,
    };
    unsafe {
        EnumWindows(
            Some(enum_windows_find_class),
            (&mut state as *mut WindowSearch) as isize,
        );
    }
    (state.found != 0).then_some(state.found)
}

struct WindowSearch {
    target: String,
    found: isize,
}

unsafe extern "system" fn enum_windows_find_class(hwnd: isize, lparam: isize) -> i32 {
    let state = &mut *(lparam as *mut WindowSearch);
    if class_name(hwnd) == state.target {
        state.found = hwnd;
        return 0;
    }
    1
}

fn focused_window(foreground: isize) -> Option<isize> {
    let thread_id = unsafe { GetWindowThreadProcessId(foreground, std::ptr::null_mut()) };
    if thread_id == 0 {
        return None;
    }
    let mut info = GuiThreadInfo {
        cb_size: std::mem::size_of::<GuiThreadInfo>() as u32,
        ..GuiThreadInfo::default()
    };
    let ok = unsafe { GetGUIThreadInfo(thread_id, &mut info as *mut GuiThreadInfo) };
    (ok != 0 && info.hwnd_focus != 0).then_some(info.hwnd_focus)
}

fn set_foreground(hwnd: isize) {
    unsafe {
        SetForegroundWindow(hwnd);
    }
}

fn set_window_text(hwnd: isize, text: &str) {
    let wide = wide_null(text);
    unsafe {
        SendMessageW(hwnd, WM_SETTEXT, 0, wide.as_ptr() as isize);
    }
}

fn send_message(hwnd: isize, message: u32, wparam: usize, lparam: usize) -> isize {
    unsafe { SendMessageW(hwnd, message, wparam, lparam as isize) }
}

fn send_key(vk: u8) {
    unsafe {
        keybd_event(vk, 0, 0, 0);
        thread::sleep(Duration::from_millis(50));
        keybd_event(vk, 0, KEYEVENTF_KEYUP, 0);
    }
}

fn set_clipboard_text(text: &str) {
    let _guard = ClipboardGuard::open();
    unsafe {
        EmptyClipboard();
        let bytes = utf16_bytes(&wide_null(text));
        let handle = GlobalAlloc(GMEM_MOVEABLE, bytes.len());
        assert_ne!(handle, 0, "GlobalAlloc failed");
        let target = GlobalLock(handle) as *mut u8;
        assert!(!target.is_null(), "GlobalLock failed");
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), target, bytes.len());
        GlobalUnlock(handle);
        assert_ne!(
            SetClipboardData(CF_UNICODETEXT, handle),
            0,
            "SetClipboardData failed"
        );
    }
}

fn clipboard_text() -> String {
    let _guard = ClipboardGuard::open();
    unsafe {
        let handle = GetClipboardData(CF_UNICODETEXT);
        assert_ne!(handle, 0, "clipboard does not contain CF_UNICODETEXT");
        let ptr = GlobalLock(handle) as *const u16;
        assert!(!ptr.is_null(), "GlobalLock failed");
        let mut len = 0;
        while *ptr.add(len) != 0 {
            len += 1;
        }
        let text = String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len));
        GlobalUnlock(handle);
        text
    }
}

struct ClipboardGuard;

impl ClipboardGuard {
    fn open() -> Self {
        for _ in 0..20 {
            if unsafe { OpenClipboard(0) } != 0 {
                return Self;
            }
            thread::sleep(Duration::from_millis(25));
        }
        panic!("OpenClipboard failed");
    }
}

impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        unsafe {
            CloseClipboard();
        }
    }
}

fn class_name(hwnd: isize) -> String {
    let mut buffer = [0u16; 256];
    let len = unsafe { GetClassNameW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32) };
    String::from_utf16_lossy(&buffer[..len.max(0) as usize])
}

fn wide_null(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn utf16_bytes(input: &[u16]) -> Vec<u8> {
    input.iter().flat_map(|unit| unit.to_le_bytes()).collect()
}

#[repr(C)]
#[derive(Default)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

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

const WM_SETTEXT: u32 = 0x000C;
const WM_GETTEXTLENGTH: u32 = 0x000E;
const EM_SETSEL: u32 = 0x00B1;
const VK_SCROLL: u8 = 0x91;
const KEYEVENTF_KEYUP: u32 = 0x0002;
const CF_UNICODETEXT: u32 = 13;
const GMEM_MOVEABLE: u32 = 0x0002;

#[link(name = "user32")]
unsafe extern "system" {
    fn EnumWindows(
        callback: Option<unsafe extern "system" fn(isize, isize) -> i32>,
        lparam: isize,
    ) -> i32;
    fn GetClassNameW(hwnd: isize, class_name: *mut u16, max_count: i32) -> i32;
    fn GetWindowThreadProcessId(hwnd: isize, process_id: *mut u32) -> u32;
    fn GetGUIThreadInfo(thread_id: u32, gui_thread_info: *mut GuiThreadInfo) -> i32;
    fn SetForegroundWindow(hwnd: isize) -> i32;
    fn SendMessageW(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> isize;
    fn keybd_event(vk: u8, scan: u8, flags: u32, extra_info: usize);
    fn OpenClipboard(hwnd_new_owner: isize) -> i32;
    fn CloseClipboard() -> i32;
    fn EmptyClipboard() -> i32;
    fn GetClipboardData(format: u32) -> isize;
    fn SetClipboardData(format: u32, mem: isize) -> isize;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GlobalAlloc(flags: u32, bytes: usize) -> isize;
    fn GlobalLock(mem: isize) -> *mut std::ffi::c_void;
    fn GlobalUnlock(mem: isize) -> i32;
}
