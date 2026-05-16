#![cfg(windows)]

use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{mpsc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

static FAKE_TERMINAL_STATE: OnceLock<Mutex<FakeTerminalState>> = OnceLock::new();
static TERMINAL_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
const CREATE_NEW_CONSOLE: u32 = 0x00000010;

#[derive(Debug, Default)]
struct FakeTerminalState {
    text: String,
    leaked_chars: String,
    suppress_next_chars: usize,
    ctrl_down: bool,
    shift_down: bool,
}

#[test]
#[ignore = "creates a fake Windows Terminal window and drives stepler-cli against it"]
fn scrolllock_fake_windows_terminal_does_not_print_copy_chord() {
    let _guard = terminal_test_guard();
    let terminal = FakeWindowsTerminal::start("пше");
    set_foreground(terminal.hwnd);
    thread::sleep(Duration::from_millis(300));
    set_clipboard_text("COPYME");

    let exe = env!("CARGO_BIN_EXE_stepler-cli");
    let output = Command::new(exe)
        .arg("scrolllock")
        .arg("--delay")
        .arg("1")
        .arg("--apply")
        .env("STEPLER_TEST_FOREGROUND_HWND", terminal.hwnd.to_string())
        .output()
        .expect("failed to run stepler-cli scrolllock");

    let state = fake_state();
    assert!(
        output.status.success(),
        "stepler-cli failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(state.text, "git");
    assert!(
        !state.leaked_chars.contains('С'),
        "service shortcut leaked chars into terminal input: {:?}",
        state.leaked_chars
    );
    assert_eq!(clipboard_text(), "COPYME");
}

#[test]
#[ignore = "opens a real visible powershell.exe window; watch it while the test runs"]
fn watched_powershell_scrolllock_repro_visible_window() {
    let _guard = terminal_test_guard();
    let transcript_path = std::env::temp_dir().join("stepler_watched_powershell_transcript.txt");
    let _ = std::fs::remove_file(&transcript_path);

    let mut runner = RunnerProcess::start();
    let mut powershell = VisiblePowerShell::start(&transcript_path);
    set_foreground(powershell.hwnd);
    show_window(powershell.hwnd);
    thread::sleep(Duration::from_millis(800));

    paste_text_to_window(powershell.hwnd, "пше");
    thread::sleep(Duration::from_millis(500));
    set_clipboard_text("COPYME");
    send_key(VK_SCROLL);

    let hold_secs = std::env::var("STEPLER_WATCH_HOLD_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(3);
    eprintln!(
        "Watch the visible PowerShell window now. It should show `git`, not `сСпшеС`/`1спше`. Holding for {hold_secs}s..."
    );
    thread::sleep(Duration::from_secs(hold_secs));

    send_key(VK_RETURN);
    thread::sleep(Duration::from_millis(1_500));
    let transcript = read_text_lossy(&transcript_path);
    let clipboard_after = clipboard_text();

    powershell.stop();
    runner.stop();

    assert!(
        transcript.matches("STEPLER_GIT_CALLED").count() >= 2,
        "expected converted `git` command to execute; transcript was:\n{transcript}"
    );
    assert!(
        !transcript.contains("сСпшеС") && !transcript.contains("1спше"),
        "known bad terminal leakage reproduced; transcript was:\n{transcript}"
    );
    assert_eq!(clipboard_after, "COPYME");
}

#[test]
#[ignore = "manual watched test: focus an existing PowerShell/Windows Terminal with `пше` typed"]
fn watched_active_terminal_scrolllock_repro() {
    let _guard = terminal_test_guard();

    let countdown_secs = std::env::var("STEPLER_WATCH_COUNTDOWN_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(3);
    eprintln!(
        "Focus the real target PowerShell/Windows Terminal now, type `пше`, and leave the caret at the end."
    );
    eprintln!("Running `stepler-cli scrolllock --apply` against the foreground window in {countdown_secs}s...");
    for remaining in (1..=countdown_secs).rev() {
        eprintln!("{remaining}...");
        thread::sleep(Duration::from_secs(1));
    }

    let hwnd = unsafe { GetForegroundWindow() };
    assert_ne!(hwnd, 0, "no foreground window before sending ScrollLock");
    eprintln!(
        "Foreground target: hwnd=0x{hwnd:X}; class=`{}`; title=`{}`",
        class_name(hwnd),
        window_text(hwnd)
    );

    set_clipboard_text("COPYME");
    let exe = env!("CARGO_BIN_EXE_stepler-cli");
    let output = Command::new(exe)
        .arg("scrolllock")
        .arg("--apply")
        .output()
        .expect("failed to run stepler-cli scrolllock");
    eprintln!(
        "stepler-cli stdout:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
    eprintln!(
        "stepler-cli stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.status.success(),
        "stepler-cli scrolllock failed with status {:?}",
        output.status.code()
    );

    let hold_secs = std::env::var("STEPLER_WATCH_HOLD_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(3);
    eprintln!(
        "Correction applied. Watch the target window: expected `git`, not unchanged `пше` or leaked `Спше`/`сСпшеС`/`1спше`. Holding for {hold_secs}s..."
    );
    thread::sleep(Duration::from_secs(hold_secs));

    let clipboard_after = clipboard_text();
    assert_eq!(
        clipboard_after, "COPYME",
        "terminal operation changed clipboard text"
    );
}

#[test]
#[ignore = "manual diagnostic: checks which Windows Terminal copy shortcut works in the active window"]
fn watched_active_terminal_copy_shortcut_diagnostic() {
    let _guard = terminal_test_guard();
    let variant =
        std::env::var("STEPLER_TERMINAL_COPY_VARIANT").unwrap_or_else(|_| "ctrl_shift_c".into());

    if variant == "manual_selection_right_click_copy" {
        eprintln!(
            "Focus PowerShell/Windows Terminal now and manually select `пше` with the mouse."
        );
    } else {
        eprintln!(
            "Focus PowerShell/Windows Terminal now, type `пше`, and leave the caret at the end."
        );
    }
    eprintln!("Testing copy variant `{variant}` in 3s...");
    for remaining in (1..=3).rev() {
        eprintln!("{remaining}...");
        thread::sleep(Duration::from_secs(1));
    }

    let hwnd = unsafe { GetForegroundWindow() };
    assert_ne!(hwnd, 0, "no foreground window before shortcut diagnostic");
    eprintln!(
        "Foreground target: hwnd=0x{hwnd:X}; class=`{}`; title=`{}`",
        class_name(hwnd),
        window_text(hwnd)
    );

    set_clipboard_text("COPYME");
    if variant != "manual_selection_right_click_copy" {
        send_real_chord(&[VK_SHIFT], VK_HOME);
        thread::sleep(Duration::from_millis(150));
    }
    match variant.as_str() {
        "ctrl_insert" => send_real_chord(&[VK_CONTROL], VK_INSERT),
        "left_ctrl_insert" => send_real_chord(&[VK_LCONTROL], VK_INSERT),
        "ctrl_c" => send_real_chord(&[VK_CONTROL], VK_C),
        "left_ctrl_c" => send_real_chord(&[VK_LCONTROL], VK_C),
        "ctrl_shift_c" => send_real_chord(&[VK_CONTROL, VK_SHIFT], VK_C),
        "right_click_copy" => right_click_window_center(hwnd),
        "manual_selection_right_click_copy" => right_click_window_center(hwnd),
        "english_ctrl_shift_c" => {
            post_layout_change(hwnd, ENGLISH_LAYOUT);
            thread::sleep(Duration::from_millis(300));
            send_real_chord(&[VK_CONTROL, VK_SHIFT], VK_C);
            post_layout_change(hwnd, RUSSIAN_LAYOUT);
        }
        "left_ctrl_shift_c" => send_real_chord(&[VK_LCONTROL, VK_SHIFT], VK_C),
        other => panic!("unknown STEPLER_TERMINAL_COPY_VARIANT `{other}`"),
    }
    thread::sleep(Duration::from_millis(500));
    if variant != "manual_selection_right_click_copy" {
        send_real_key(VK_END);
    }
    thread::sleep(Duration::from_secs(3));

    eprintln!("clipboard after `{variant}`: `{}`", clipboard_text());
    eprintln!("Check the terminal line for leaked text such as `Сс`.");
}

#[test]
#[ignore = "manual diagnostic: inspects the active terminal through UI Automation"]
fn watched_active_terminal_uia_diagnostic() {
    let _guard = terminal_test_guard();

    eprintln!("Focus PowerShell/Windows Terminal now, type `пше`, and leave the caret at the end.");
    eprintln!("Inspecting focused element through UIA in 3s...");
    for remaining in (1..=3).rev() {
        eprintln!("{remaining}...");
        thread::sleep(Duration::from_secs(1));
    }

    let hwnd = unsafe { GetForegroundWindow() };
    assert_ne!(hwnd, 0, "no foreground window before UIA diagnostic");
    eprintln!(
        "Foreground target: hwnd=0x{hwnd:X}; class=`{}`; title=`{}`",
        class_name(hwnd),
        window_text(hwnd)
    );

    let script = r#"
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$focused = [System.Windows.Automation.AutomationElement]::FocusedElement
if ($null -eq $focused) {
  'focused=null'
  exit 0
}
'focused.name=' + $focused.Current.Name
'focused.class=' + $focused.Current.ClassName
'focused.control_type=' + $focused.Current.ControlType.ProgrammaticName
$patterns = @()
foreach ($pattern in $focused.GetSupportedPatterns()) {
  $patterns += $pattern.ProgrammaticName
}
'focused.patterns=' + ($patterns -join ',')
$textPattern = $null
try {
  $textPattern = $focused.GetCurrentPattern([System.Windows.Automation.TextPattern]::Pattern)
} catch {
  'text_pattern_error=' + $_.Exception.Message
}
if ($null -ne $textPattern) {
  $text = $textPattern.DocumentRange.GetText(4096)
  'text_pattern_text_begin'
  $text
  'text_pattern_text_end'
}
"#;
    let output = Command::new("powershell.exe")
        .arg("-NoLogo")
        .arg("-NoProfile")
        .arg("-Command")
        .arg(script)
        .output()
        .expect("failed to run UIA diagnostic powershell");

    eprintln!("uia stdout:\n{}", String::from_utf8_lossy(&output.stdout));
    eprintln!("uia stderr:\n{}", String::from_utf8_lossy(&output.stderr));
    assert!(
        output.status.success(),
        "UIA diagnostic powershell failed with status {:?}",
        output.status.code()
    );
}

fn terminal_test_guard() -> std::sync::MutexGuard<'static, ()> {
    TERMINAL_TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("terminal test lock was poisoned")
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
            .stderr(Stdio::inherit())
            .spawn()
            .expect("failed to start stepler-cli run-hotkeys");
        thread::sleep(Duration::from_millis(900));
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

struct VisiblePowerShell {
    child: Child,
    hwnd: isize,
}

impl VisiblePowerShell {
    fn start(transcript_path: &Path) -> Self {
        let title = format!("Stepler watched PowerShell {}", std::process::id());
        let command = format!(
            "Remove-Module PSReadLine -ErrorAction SilentlyContinue; $host.UI.RawUI.WindowTitle='{title}'; [Console]::Title='{title}'; function git {{ 'STEPLER_GIT_CALLED' }}; Start-Transcript -Path '{}' -Force",
            transcript_path.display()
        );
        let child = Command::new("powershell.exe")
            .arg("-NoLogo")
            .arg("-NoProfile")
            .arg("-NoExit")
            .arg("-Command")
            .arg(command)
            .creation_flags(CREATE_NEW_CONSOLE)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("failed to start visible powershell.exe");
        let hwnd = wait_for_window_title(&title, Duration::from_secs(10));
        assert_ne!(hwnd, 0, "visible PowerShell window was not found");
        set_foreground(hwnd);
        show_window(hwnd);
        thread::sleep(Duration::from_millis(1_200));
        Self { child, hwnd }
    }

    fn stop(&mut self) {
        if std::env::var_os("STEPLER_KEEP_WATCH_WINDOW").is_some() {
            return;
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for VisiblePowerShell {
    fn drop(&mut self) {
        self.stop();
    }
}

fn wait_for_window_title(title: &str, timeout: Duration) -> isize {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if let Some(hwnd) = find_window_by_title(title) {
            return hwnd;
        }
        thread::sleep(Duration::from_millis(100));
    }
    0
}

fn find_window_by_title(title: &str) -> Option<isize> {
    let mut state = TitleSearch {
        title: title.to_owned(),
        found: 0,
    };
    unsafe {
        EnumWindows(
            Some(enum_windows_find_title),
            (&mut state as *mut TitleSearch) as isize,
        );
    }
    (state.found != 0).then_some(state.found)
}

struct TitleSearch {
    title: String,
    found: isize,
}

unsafe extern "system" fn enum_windows_find_title(hwnd: isize, lparam: isize) -> i32 {
    let state = &mut *(lparam as *mut TitleSearch);
    if IsWindowVisible(hwnd) != 0 && window_text(hwnd) == state.title {
        state.found = hwnd;
        return 0;
    }
    1
}

fn read_text_lossy(path: &Path) -> String {
    let bytes = std::fs::read(path).unwrap_or_default();
    String::from_utf8_lossy(&bytes).into_owned()
}

fn fake_state() -> FakeTerminalState {
    let state = FAKE_TERMINAL_STATE
        .get()
        .expect("fake terminal state was not initialized")
        .lock()
        .expect("fake terminal state was poisoned");
    FakeTerminalState {
        text: state.text.clone(),
        leaked_chars: state.leaked_chars.clone(),
        suppress_next_chars: state.suppress_next_chars,
        ctrl_down: state.ctrl_down,
        shift_down: state.shift_down,
    }
}

struct FakeWindowsTerminal {
    hwnd: isize,
}

impl FakeWindowsTerminal {
    fn start(initial_text: &str) -> Self {
        let _ = FAKE_TERMINAL_STATE.set(Mutex::new(FakeTerminalState {
            text: initial_text.to_owned(),
            leaked_chars: String::new(),
            suppress_next_chars: 0,
            ctrl_down: false,
            shift_down: false,
        }));
        if let Some(state) = FAKE_TERMINAL_STATE.get() {
            let mut state = state.lock().expect("fake terminal state was poisoned");
            state.text = initial_text.to_owned();
            state.leaked_chars.clear();
            state.suppress_next_chars = 0;
            state.ctrl_down = false;
            state.shift_down = false;
        }

        let (tx, rx) = mpsc::channel();
        thread::spawn(move || unsafe {
            let class_name = wide_null("CASCADIA_HOSTING_WINDOW_CLASS");
            let wnd_class = WndClassW {
                style: 0,
                wnd_proc: Some(fake_terminal_wnd_proc),
                cls_extra: 0,
                wnd_extra: 0,
                instance: GetModuleHandleW(std::ptr::null()),
                icon: 0,
                cursor: 0,
                background: 0,
                menu_name: std::ptr::null(),
                class_name: class_name.as_ptr(),
            };
            RegisterClassW(&wnd_class as *const WndClassW);
            let title = wide_null("Stepler Fake Windows Terminal");
            let hwnd = CreateWindowExW(
                0,
                class_name.as_ptr(),
                title.as_ptr(),
                WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                200,
                200,
                640,
                240,
                0,
                0,
                wnd_class.instance,
                std::ptr::null_mut(),
            );
            tx.send(hwnd).expect("failed to publish fake terminal hwnd");

            let mut message = Msg::default();
            while GetMessageW(&mut message as *mut Msg, 0, 0, 0) > 0 {
                TranslateMessage(&message as *const Msg);
                DispatchMessageW(&message as *const Msg);
            }
        });

        let hwnd = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("fake terminal did not create a window");
        assert_ne!(hwnd, 0, "CreateWindowExW failed");
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(3) {
            set_foreground(hwnd);
            if unsafe { GetForegroundWindow() } == hwnd {
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }
        Self { hwnd }
    }
}

impl Drop for FakeWindowsTerminal {
    fn drop(&mut self) {
        unsafe {
            PostMessageW(self.hwnd, WM_CLOSE, 0, 0);
        }
    }
}

unsafe extern "system" fn fake_terminal_wnd_proc(
    hwnd: isize,
    message: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    match message {
        WM_KEYDOWN => {
            let key = wparam as u32;
            update_modifier_state(key, true);
            let (ctrl_down, shift_down) = fake_modifier_state();
            if ctrl_down && shift_down && key == VK_C as u32 {
                set_clipboard_text(&fake_state().text);
                suppress_fake_terminal_chars(1);
                return 0;
            }
            if ctrl_down && shift_down && key == VK_V as u32 {
                let pasted = clipboard_text();
                if let Some(state) = FAKE_TERMINAL_STATE.get() {
                    let mut state = state.lock().expect("fake terminal state was poisoned");
                    state.text = pasted;
                    state.suppress_next_chars = state.suppress_next_chars.saturating_add(2);
                }
                return 0;
            }
            DefWindowProcW(hwnd, message, wparam, lparam)
        }
        WM_KEYUP => {
            update_modifier_state(wparam as u32, false);
            DefWindowProcW(hwnd, message, wparam, lparam)
        }
        WM_CHAR => {
            if consume_suppressed_fake_terminal_char() {
                return 0;
            }
            let (ctrl_down, _) = fake_modifier_state();
            if ctrl_down {
                return 0;
            }
            if let Some(ch) = char::from_u32(wparam as u32) {
                if let Some(state) = FAKE_TERMINAL_STATE.get() {
                    let mut state = state.lock().expect("fake terminal state was poisoned");
                    state.leaked_chars.push(ch);
                    state.text.insert(0, ch);
                }
            }
            0
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

fn update_modifier_state(key: u32, is_down: bool) {
    if let Some(state) = FAKE_TERMINAL_STATE.get() {
        let mut state = state.lock().expect("fake terminal state was poisoned");
        match key {
            key if key == VK_CONTROL as u32 || key == VK_LCONTROL as u32 => {
                state.ctrl_down = is_down
            }
            key if key == VK_SHIFT as u32 || key == VK_LSHIFT as u32 => state.shift_down = is_down,
            _ => {}
        }
    }
}

fn fake_modifier_state() -> (bool, bool) {
    let Some(state) = FAKE_TERMINAL_STATE.get() else {
        return (false, false);
    };
    let state = state.lock().expect("fake terminal state was poisoned");
    (state.ctrl_down, state.shift_down)
}

fn suppress_fake_terminal_chars(count: usize) {
    if let Some(state) = FAKE_TERMINAL_STATE.get() {
        let mut state = state.lock().expect("fake terminal state was poisoned");
        state.suppress_next_chars = state.suppress_next_chars.saturating_add(count);
    }
}

fn consume_suppressed_fake_terminal_char() -> bool {
    let Some(state) = FAKE_TERMINAL_STATE.get() else {
        return false;
    };
    let mut state = state.lock().expect("fake terminal state was poisoned");
    if state.suppress_next_chars == 0 {
        return false;
    }
    state.suppress_next_chars -= 1;
    true
}

fn set_foreground(hwnd: isize) {
    unsafe {
        SetForegroundWindow(hwnd);
    }
}

fn show_window(hwnd: isize) {
    unsafe {
        ShowWindow(hwnd, SW_RESTORE);
        SetWindowPos(hwnd, HWND_TOPMOST, 120, 120, 1000, 520, SWP_SHOWWINDOW);
        SetWindowPos(hwnd, HWND_NOTOPMOST, 120, 120, 1000, 520, SWP_SHOWWINDOW);
    }
}

fn paste_text_to_window(hwnd: isize, text: &str) {
    set_clipboard_text(text);
    post_key_chord(hwnd, &[VK_SHIFT], VK_INSERT);
    thread::sleep(Duration::from_millis(500));
}

fn post_key_chord(hwnd: isize, modifiers: &[u16], key: u16) {
    for modifier in modifiers {
        unsafe {
            PostMessageW(hwnd, WM_KEYDOWN, *modifier as usize, 0);
        }
    }
    thread::sleep(Duration::from_millis(50));
    unsafe {
        PostMessageW(hwnd, WM_KEYDOWN, key as usize, 0);
        thread::sleep(Duration::from_millis(50));
        PostMessageW(hwnd, WM_KEYUP, key as usize, 0);
    }
    thread::sleep(Duration::from_millis(50));
    for modifier in modifiers.iter().rev() {
        unsafe {
            PostMessageW(hwnd, WM_KEYUP, *modifier as usize, 0);
        }
    }
}

fn send_key(vk: u16) {
    send_real_key(vk);
}

fn send_real_key(vk: u16) {
    send_input_events(&[
        TestKeyEvent::scan_code(vk, false),
        TestKeyEvent::scan_code(vk, true),
    ]);
    thread::sleep(Duration::from_millis(50));
}

fn send_real_chord(modifiers: &[u16], key: u16) {
    for modifier in modifiers {
        send_input_events(&[TestKeyEvent::scan_code(*modifier, false)]);
        thread::sleep(Duration::from_millis(30));
    }
    send_input_events(&[
        TestKeyEvent::virtual_key(key, false),
        TestKeyEvent::virtual_key(key, true),
    ]);
    thread::sleep(Duration::from_millis(50));
    for modifier in modifiers.iter().rev() {
        send_input_events(&[TestKeyEvent::scan_code(*modifier, true)]);
        thread::sleep(Duration::from_millis(30));
    }
}

fn send_input_events(events: &[TestKeyEvent]) {
    let mut inputs = events
        .iter()
        .map(|event| Input::keyboard(*event))
        .collect::<Vec<_>>();
    let sent = unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_mut_ptr(),
            std::mem::size_of::<Input>() as i32,
        )
    };
    assert_eq!(
        sent,
        inputs.len() as u32,
        "SendInput sent only {sent} events"
    );
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
        if handle == 0 {
            return String::new();
        }
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
        for _ in 0..40 {
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

fn wide_null(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn utf16_bytes(input: &[u16]) -> Vec<u8> {
    input.iter().flat_map(|unit| unit.to_le_bytes()).collect()
}

fn window_text(hwnd: isize) -> String {
    let mut buffer = [0u16; 512];
    let len = unsafe { GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32) };
    String::from_utf16_lossy(&buffer[..len.max(0) as usize])
}

fn class_name(hwnd: isize) -> String {
    let mut buffer = [0u16; 256];
    let len = unsafe { GetClassNameW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32) };
    String::from_utf16_lossy(&buffer[..len.max(0) as usize])
}

fn post_layout_change(hwnd: isize, layout: isize) {
    unsafe {
        PostMessageW(hwnd, WM_INPUTLANGCHANGEREQUEST, 0, layout);
    }
}

fn right_click_window_center(hwnd: isize) {
    let mut rect = Rect::default();
    let ok = unsafe { GetWindowRect(hwnd, &mut rect as *mut Rect) };
    assert_ne!(ok, 0, "GetWindowRect failed");
    let x = rect.left + (rect.right - rect.left) / 2;
    let y = rect.top + (rect.bottom - rect.top) / 2;
    unsafe {
        SetCursorPos(x, y);
    }
    thread::sleep(Duration::from_millis(50));
    send_input_events(&[
        TestKeyEvent::mouse(MOUSEEVENTF_RIGHTDOWN),
        TestKeyEvent::mouse(MOUSEEVENTF_RIGHTUP),
    ]);
}

const WS_OVERLAPPEDWINDOW: u32 = 0x00CF0000;
const WS_VISIBLE: u32 = 0x10000000;
const HWND_TOPMOST: isize = -1;
const HWND_NOTOPMOST: isize = -2;
const SW_RESTORE: i32 = 9;
const SWP_SHOWWINDOW: u32 = 0x0040;
const WM_CLOSE: u32 = 0x0010;
const WM_DESTROY: u32 = 0x0002;
const WM_KEYDOWN: u32 = 0x0100;
const WM_KEYUP: u32 = 0x0101;
const WM_CHAR: u32 = 0x0102;
const WM_INPUTLANGCHANGEREQUEST: u32 = 0x0050;
const INPUT_MOUSE: u32 = 0;
const INPUT_KEYBOARD: u32 = 1;
const MAPVK_VK_TO_VSC_EX: u32 = 4;
const KEYEVENTF_EXTENDEDKEY: u32 = 0x0001;
const KEYEVENTF_KEYUP: u32 = 0x0002;
const KEYEVENTF_SCANCODE: u32 = 0x0008;
const MOUSEEVENTF_RIGHTDOWN: u32 = 0x0008;
const MOUSEEVENTF_RIGHTUP: u32 = 0x0010;
const VK_SHIFT: u16 = 0x10;
const VK_LSHIFT: u16 = 0xA0;
const VK_CONTROL: u16 = 0x11;
const VK_LCONTROL: u16 = 0xA2;
const VK_INSERT: u16 = 0x2D;
const VK_DELETE: u16 = 0x2E;
const VK_HOME: u16 = 0x24;
const VK_END: u16 = 0x23;
const VK_LEFT: u16 = 0x25;
const VK_UP: u16 = 0x26;
const VK_RIGHT: u16 = 0x27;
const VK_DOWN: u16 = 0x28;
const VK_PRIOR: u16 = 0x21;
const VK_NEXT: u16 = 0x22;
const VK_DIVIDE: u16 = 0x6F;
const VK_NUMLOCK: u16 = 0x90;
const VK_C: u16 = 0x43;
const VK_V: u16 = 0x56;
const VK_RETURN: u16 = 0x0D;
const VK_SCROLL: u16 = 0x91;
const CF_UNICODETEXT: u32 = 13;
const GMEM_MOVEABLE: u32 = 0x0002;
const ENGLISH_LAYOUT: isize = 0x0409_0409;
const RUSSIAN_LAYOUT: isize = 0x0419_0419;

#[repr(C)]
#[derive(Default)]
struct Point {
    x: i32,
    y: i32,
}

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

#[repr(C)]
struct WndClassW {
    style: u32,
    wnd_proc: Option<unsafe extern "system" fn(isize, u32, usize, isize) -> isize>,
    cls_extra: i32,
    wnd_extra: i32,
    instance: isize,
    icon: isize,
    cursor: isize,
    background: isize,
    menu_name: *const u16,
    class_name: *const u16,
}

#[repr(C)]
#[derive(Default)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[derive(Clone, Copy)]
struct TestKeyEvent {
    vk: u16,
    key_up: bool,
    mode: TestKeyMode,
    mouse_flags: u32,
}

impl TestKeyEvent {
    fn scan_code(vk: u16, key_up: bool) -> Self {
        Self {
            vk,
            key_up,
            mode: TestKeyMode::ScanCode,
            mouse_flags: 0,
        }
    }

    fn virtual_key(vk: u16, key_up: bool) -> Self {
        Self {
            vk,
            key_up,
            mode: TestKeyMode::VirtualKey,
            mouse_flags: 0,
        }
    }

    fn mouse(flags: u32) -> Self {
        Self {
            vk: 0,
            key_up: false,
            mode: TestKeyMode::Mouse,
            mouse_flags: flags,
        }
    }
}

#[derive(Clone, Copy)]
enum TestKeyMode {
    ScanCode,
    VirtualKey,
    Mouse,
}

#[repr(C)]
struct Input {
    input_type: u32,
    input: InputUnion,
}

impl Input {
    fn keyboard(event: TestKeyEvent) -> Self {
        if matches!(event.mode, TestKeyMode::Mouse) {
            return Self {
                input_type: INPUT_MOUSE,
                input: InputUnion {
                    mi: MouseInput {
                        dx: 0,
                        dy: 0,
                        mouse_data: 0,
                        flags: event.mouse_flags,
                        time: 0,
                        extra_info: 0,
                    },
                },
            };
        }

        let keybd_input = match event.mode {
            TestKeyMode::ScanCode => {
                let scan_code = unsafe { MapVirtualKeyW(event.vk as u32, MAPVK_VK_TO_VSC_EX) };
                let mut flags = KEYEVENTF_SCANCODE;
                if event.key_up {
                    flags |= KEYEVENTF_KEYUP;
                }
                if scan_code & 0xFF00 != 0 || is_extended_navigation_key(event.vk) {
                    flags |= KEYEVENTF_EXTENDEDKEY;
                }
                KeybdInput {
                    vk: 0,
                    scan: (scan_code & 0xFF) as u16,
                    flags,
                    time: 0,
                    extra_info: 0,
                }
            }
            TestKeyMode::VirtualKey => {
                let mut flags = 0;
                if event.key_up {
                    flags |= KEYEVENTF_KEYUP;
                }
                KeybdInput {
                    vk: event.vk,
                    scan: 0,
                    flags,
                    time: 0,
                    extra_info: 0,
                }
            }
            TestKeyMode::Mouse => unreachable!("handled above"),
        };
        Self {
            input_type: INPUT_KEYBOARD,
            input: InputUnion { ki: keybd_input },
        }
    }
}

fn is_extended_navigation_key(vk: u16) -> bool {
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

#[repr(C)]
union InputUnion {
    mi: MouseInput,
    ki: KeybdInput,
    padding: [u8; 32],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct MouseInput {
    dx: i32,
    dy: i32,
    mouse_data: u32,
    flags: u32,
    time: u32,
    extra_info: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct KeybdInput {
    vk: u16,
    scan: u16,
    flags: u32,
    time: u32,
    extra_info: usize,
}

#[link(name = "user32")]
unsafe extern "system" {
    fn CloseClipboard() -> i32;
    fn CreateWindowExW(
        ex_style: u32,
        class_name: *const u16,
        window_name: *const u16,
        style: u32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        parent: isize,
        menu: isize,
        instance: isize,
        param: *mut std::ffi::c_void,
    ) -> isize;
    fn DefWindowProcW(hwnd: isize, message: u32, wparam: usize, lparam: isize) -> isize;
    fn DispatchMessageW(message: *const Msg) -> isize;
    fn EmptyClipboard() -> i32;
    fn EnumWindows(
        callback: Option<unsafe extern "system" fn(isize, isize) -> i32>,
        lparam: isize,
    ) -> i32;
    fn GetClipboardData(format: u32) -> isize;
    fn GetClassNameW(hwnd: isize, class_name: *mut u16, max_count: i32) -> i32;
    fn GetForegroundWindow() -> isize;
    fn GetWindowRect(hwnd: isize, rect: *mut Rect) -> i32;
    fn GetWindowTextW(hwnd: isize, text: *mut u16, max_count: i32) -> i32;
    fn GetMessageW(message: *mut Msg, hwnd: isize, min_filter: u32, max_filter: u32) -> i32;
    fn IsWindowVisible(hwnd: isize) -> i32;
    fn OpenClipboard(hwnd_new_owner: isize) -> i32;
    fn PostMessageW(hwnd: isize, msg: u32, wparam: usize, lparam: isize) -> i32;
    fn PostQuitMessage(exit_code: i32);
    fn RegisterClassW(window_class: *const WndClassW) -> u16;
    fn SetClipboardData(format: u32, mem: isize) -> isize;
    fn SetForegroundWindow(hwnd: isize) -> i32;
    fn SetCursorPos(x: i32, y: i32) -> i32;
    fn SetWindowPos(
        hwnd: isize,
        hwnd_insert_after: isize,
        x: i32,
        y: i32,
        cx: i32,
        cy: i32,
        flags: u32,
    ) -> i32;
    fn ShowWindow(hwnd: isize, command_show: i32) -> i32;
    fn TranslateMessage(message: *const Msg) -> i32;
    fn SendInput(count: u32, inputs: *mut Input, size: i32) -> u32;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetModuleHandleW(module_name: *const u16) -> isize;
    fn GlobalAlloc(flags: u32, bytes: usize) -> isize;
    fn GlobalLock(mem: isize) -> *mut std::ffi::c_void;
    fn GlobalUnlock(mem: isize) -> i32;
    fn MapVirtualKeyW(code: u32, map_type: u32) -> u32;
}
