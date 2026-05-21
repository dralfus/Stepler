#![cfg(windows)]

use std::process::Command;
use std::process::{Child, Stdio};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;

static WORD_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[test]
#[ignore = "launches Microsoft Word through COM; close existing Word windows before running"]
fn word_com_direct_cli_replaces_pause_and_scrolllock_text() {
    let _guard = WORD_TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap();

    set_clipboard_text("WORD_CLIPBOARD_KEEP");
    let mut word = WordSession::launch("k.,jdm");
    let pause = run_stepler_direct("pause", word.hwnd);
    assert!(
        pause.status.success(),
        "pause failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&pause.stdout),
        String::from_utf8_lossy(&pause.stderr)
    );
    assert_eq!(word.read_text(), "любовь");
    assert_eq!(clipboard_text(), "WORD_CLIPBOARD_KEEP");

    word.replace_text("k.,jdm ghbdtn");
    word.set_caret_utf16(6);
    let middle_pause = run_stepler_direct("pause", word.hwnd);
    assert!(
        middle_pause.status.success(),
        "middle pause failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&middle_pause.stdout),
        String::from_utf8_lossy(&middle_pause.stderr)
    );
    assert_eq!(word.read_text(), "любовь ghbdtn");
    assert_eq!(clipboard_text(), "WORD_CLIPBOARD_KEEP");

    word.replace_text("k.,jdm ghbdtn");
    word.select_all();
    let selected_pause = run_stepler_direct("pause", word.hwnd);
    assert!(
        selected_pause.status.success(),
        "selected pause failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&selected_pause.stdout),
        String::from_utf8_lossy(&selected_pause.stderr)
    );
    assert_eq!(word.read_text(), "любовь привет");
    assert_eq!(clipboard_text(), "WORD_CLIPBOARD_KEEP");

    word.replace_text("safe prefix\nghbdtn vbh");
    let scrolllock_tail = run_stepler_direct("scrolllock", word.hwnd);
    assert!(
        scrolllock_tail.status.success(),
        "scrolllock tail failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&scrolllock_tail.stdout),
        String::from_utf8_lossy(&scrolllock_tail.stderr)
    );
    assert_eq!(word.read_text(), "safe prefix\rпривет мир");
    assert_eq!(clipboard_text(), "WORD_CLIPBOARD_KEEP");

    word.replace_text("пше");
    let scrolllock = run_stepler_direct("scrolllock", word.hwnd);
    assert!(
        scrolllock.status.success(),
        "scrolllock failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&scrolllock.stdout),
        String::from_utf8_lossy(&scrolllock.stderr)
    );
    assert_eq!(word.read_text(), "git");
    assert_eq!(clipboard_text(), "WORD_CLIPBOARD_KEEP");

    word.close();
}

#[test]
#[ignore = "launches Word and stepler-cli run-hotkeys; stop existing run-hotkeys before running"]
fn word_global_hotkeys_replace_pause_and_scrolllock_text() {
    let _guard = WORD_TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap();

    let mut runner = RunnerProcess::start();
    let mut word = WordSession::launch("k.,jdm");

    send_key(VK_PAUSE);
    thread::sleep(Duration::from_millis(1_500));
    assert_eq!(
        word.read_text(),
        "любовь",
        "runner stderr:\n{}",
        runner.stderr_text()
    );

    for _ in 0..5 {
        word.replace_text("пше");
        send_ctrl_pause();
        thread::sleep(Duration::from_millis(1_500));
        assert_eq!(
            word.read_text(),
            "git",
            "runner stderr:\n{}",
            runner.stderr_text()
        );
    }

    word.close();
    runner.stop();
}

struct RunnerProcess {
    child: Child,
    stderr: Option<std::process::ChildStderr>,
}

impl RunnerProcess {
    fn start() -> Self {
        let exe = env!("CARGO_BIN_EXE_stepler-cli");
        let mut child = Command::new(exe)
            .arg("run-hotkeys")
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to start stepler-cli run-hotkeys");
        let stderr = child.stderr.take();
        thread::sleep(Duration::from_millis(800));
        Self { child, stderr }
    }

    fn stop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }

    fn stderr_text(&mut self) -> String {
        use std::io::Read;

        self.stop();
        let mut text = String::new();
        if let Some(stderr) = &mut self.stderr {
            let _ = stderr.read_to_string(&mut text);
        }
        text
    }
}

impl Drop for RunnerProcess {
    fn drop(&mut self) {
        self.stop();
    }
}

struct WordSession {
    hwnd: isize,
    closed: bool,
}

impl WordSession {
    fn launch(text: &str) -> Self {
        let existing = run_powershell(GET_WORD_PROCESS_COUNT, &[]);
        assert!(
            existing.status.success(),
            "failed to inspect WINWORD processes\nstderr:\n{}",
            String::from_utf8_lossy(&existing.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&existing.stdout).trim(),
            "0",
            "Word UI smoke test requires no existing WINWORD processes"
        );

        let output = run_powershell(
            WORD_LAUNCH_SCRIPT,
            &[("STEPLER_WORD_TEXT_B64", encode_utf16le_base64(text))],
        );
        assert!(
            output.status.success(),
            "failed to launch Word\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let fields = parse_key_value_lines(&String::from_utf8_lossy(&output.stdout));
        let hwnd = fields
            .get("hwnd")
            .and_then(|value| value.parse::<isize>().ok())
            .unwrap_or_else(|| {
                panic!(
                    "launch script did not return hwnd\nstdout:\n{}\nstderr:\n{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                )
            });
        assert_ne!(hwnd, 0, "Word returned empty hwnd");
        set_foreground(hwnd);
        thread::sleep(Duration::from_millis(500));

        Self {
            hwnd,
            closed: false,
        }
    }

    fn replace_text(&self, text: &str) {
        let output = run_powershell(
            WORD_REPLACE_TEXT_SCRIPT,
            &[("STEPLER_WORD_TEXT_B64", encode_utf16le_base64(text))],
        );
        assert!(
            output.status.success(),
            "failed to replace Word text\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        set_foreground(self.hwnd);
        thread::sleep(Duration::from_millis(300));
    }

    fn read_text(&self) -> String {
        let output = run_powershell(WORD_READ_TEXT_SCRIPT, &[]);
        assert!(
            output.status.success(),
            "failed to read Word text\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let fields = parse_key_value_lines(&String::from_utf8_lossy(&output.stdout));
        fields
            .get("text_b64")
            .and_then(|value| decode_utf16le_base64(value).ok())
            .expect("read script did not return text")
    }

    fn select_all(&self) {
        let output = run_powershell(WORD_SELECT_ALL_SCRIPT, &[]);
        assert!(
            output.status.success(),
            "failed to select Word text\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        set_foreground(self.hwnd);
        thread::sleep(Duration::from_millis(300));
    }

    fn set_caret_utf16(&self, offset: usize) {
        let output = run_powershell(
            WORD_SET_CARET_SCRIPT,
            &[("STEPLER_WORD_CARET", offset.to_string())],
        );
        assert!(
            output.status.success(),
            "failed to set Word caret\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        set_foreground(self.hwnd);
        thread::sleep(Duration::from_millis(300));
    }

    fn close(&mut self) {
        if !self.closed {
            let _ = run_powershell(WORD_CLOSE_SCRIPT, &[]);
            self.closed = true;
        }
    }
}

impl Drop for WordSession {
    fn drop(&mut self) {
        self.close();
    }
}

fn run_stepler_direct(mode: &str, hwnd: isize) -> std::process::Output {
    let exe = env!("CARGO_BIN_EXE_stepler-cli");
    Command::new(exe)
        .arg(mode)
        .arg("--apply")
        .env("STEPLER_TEST_FOREGROUND_HWND", hwnd.to_string())
        .output()
        .expect("failed to run stepler-cli")
}

fn run_powershell(script: &str, env: &[(&str, String)]) -> std::process::Output {
    let mut command = Command::new("powershell.exe");
    command
        .arg("-NoProfile")
        .arg("-NonInteractive")
        .arg("-EncodedCommand")
        .arg(encode_utf16le_base64(script));
    for (key, value) in env {
        command.env(key, value);
    }
    command.output().expect("failed to run powershell.exe")
}

fn set_clipboard_text(text: &str) {
    let output = run_powershell(
        SET_CLIPBOARD_TEXT_SCRIPT,
        &[("STEPLER_CLIPBOARD_TEXT_B64", encode_utf16le_base64(text))],
    );
    assert!(
        output.status.success(),
        "failed to set clipboard\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn clipboard_text() -> String {
    let output = run_powershell(GET_CLIPBOARD_TEXT_SCRIPT, &[]);
    assert!(
        output.status.success(),
        "failed to read clipboard\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let fields = parse_key_value_lines(&String::from_utf8_lossy(&output.stdout));
    fields
        .get("text_b64")
        .and_then(|value| decode_utf16le_base64(value).ok())
        .expect("clipboard script did not return text")
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

fn set_foreground(hwnd: isize) {
    unsafe {
        SetForegroundWindow(hwnd);
    }
}

fn send_key(vk: u8) {
    let scan = match vk {
        VK_PAUSE => 0x45,
        _ => 0,
    };
    unsafe {
        keybd_event(vk, scan, 0, 0);
        thread::sleep(Duration::from_millis(50));
        keybd_event(vk, scan, KEYEVENTF_KEYUP, 0);
    }
}

fn send_ctrl_pause() {
    unsafe {
        keybd_event(VK_CONTROL, 0, 0, 0);
        thread::sleep(Duration::from_millis(30));
        keybd_event(VK_PAUSE, 0x45, 0, 0);
        thread::sleep(Duration::from_millis(50));
        keybd_event(VK_PAUSE, 0x45, KEYEVENTF_KEYUP, 0);
        thread::sleep(Duration::from_millis(30));
        keybd_event(VK_CONTROL, 0, KEYEVENTF_KEYUP, 0);
    }
}

const VK_CONTROL: u8 = 0x11;
const VK_PAUSE: u8 = 0x13;
const KEYEVENTF_KEYUP: u32 = 0x0002;

const GET_WORD_PROCESS_COUNT: &str = r#"
$count = @(Get-Process WINWORD -ErrorAction SilentlyContinue).Count
[Console]::WriteLine($count)
"#;

const SET_CLIPBOARD_TEXT_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
function From-B64([string] $Text) {
    [System.Text.Encoding]::Unicode.GetString([Convert]::FromBase64String($Text))
}
Set-Clipboard -Value (From-B64 $env:STEPLER_CLIPBOARD_TEXT_B64)
'ok=1'
"#;

const GET_CLIPBOARD_TEXT_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
function ConvertTo-B64([string] $Text) {
    [Convert]::ToBase64String([System.Text.Encoding]::Unicode.GetBytes($Text))
}
$text = Get-Clipboard -Raw
'text_b64=' + (ConvertTo-B64 ([string] $text))
"#;

const WORD_LAUNCH_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
function From-B64([string] $Text) {
    [System.Text.Encoding]::Unicode.GetString([Convert]::FromBase64String($Text))
}
$word = New-Object -ComObject Word.Application
$word.Visible = $true
$document = $word.Documents.Add()
$word.Activate()
$selection = $word.Selection
$selection.TypeText((From-B64 $env:STEPLER_WORD_TEXT_B64))
$selection.EndKey(6) | Out-Null
$hwnd = 0
try { $hwnd = [int64] $word.Hwnd } catch { }
if ($hwnd -eq 0) {
    try { $hwnd = [int64] $word.ActiveWindow.Hwnd } catch { }
}
'hwnd=' + $hwnd
"#;

const WORD_REPLACE_TEXT_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
function From-B64([string] $Text) {
    [System.Text.Encoding]::Unicode.GetString([Convert]::FromBase64String($Text))
}
$word = [Runtime.InteropServices.Marshal]::GetActiveObject('Word.Application')
$document = $word.ActiveDocument
$document.Content.Text = (From-B64 $env:STEPLER_WORD_TEXT_B64)
$word.Activate()
$word.Selection.EndKey(6) | Out-Null
'ok=1'
"#;

const WORD_READ_TEXT_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
function ConvertTo-B64([string] $Text) {
    [Convert]::ToBase64String([System.Text.Encoding]::Unicode.GetBytes($Text))
}
function Strip-WordRangeMarkers([string] $Text) {
    if ($null -eq $Text) { return '' }
    $Text.TrimEnd([char]13, [char]7)
}
$word = [Runtime.InteropServices.Marshal]::GetActiveObject('Word.Application')
$text = Strip-WordRangeMarkers ([string] $word.ActiveDocument.Content.Text)
'text_b64=' + (ConvertTo-B64 $text)
"#;

const WORD_SELECT_ALL_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
$word = [Runtime.InteropServices.Marshal]::GetActiveObject('Word.Application')
$word.Activate()
$document = $word.ActiveDocument
$start = [int] $document.Content.Start
$end = [int] $document.Content.End
if ($end -gt $start) { $end = $end - 1 }
$word.Selection.SetRange($start, $end)
'ok=1'
"#;

const WORD_SET_CARET_SCRIPT: &str = r#"
$ErrorActionPreference = 'Stop'
$word = [Runtime.InteropServices.Marshal]::GetActiveObject('Word.Application')
$word.Activate()
$document = $word.ActiveDocument
$offset = [int] $env:STEPLER_WORD_CARET
$start = [int] $document.Content.Start
$position = $start + $offset
$word.Selection.SetRange($position, $position)
'ok=1'
"#;

const WORD_CLOSE_SCRIPT: &str = r#"
$ErrorActionPreference = 'SilentlyContinue'
$word = [Runtime.InteropServices.Marshal]::GetActiveObject('Word.Application')
$word.DisplayAlerts = 0
foreach ($document in @($word.Documents)) {
    $document.Close($false)
}
$word.Quit()
'ok=1'
"#;

#[link(name = "user32")]
unsafe extern "system" {
    fn SetForegroundWindow(hwnd: isize) -> i32;
    fn keybd_event(vk: u8, scan: u8, flags: u32, extra_info: usize);
}
