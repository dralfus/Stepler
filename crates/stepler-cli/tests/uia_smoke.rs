#![cfg(windows)]

use std::process::{Child, Command};
use std::thread;
use std::time::{Duration, Instant};

#[test]
#[ignore = "launches a visible WPF fixture and drives UI Automation against it"]
fn uia_value_pattern_replaces_wpf_textbox() {
    let title = format!("Stepler UIA Fixture {}", std::process::id());
    let result_path = std::env::temp_dir().join(format!("stepler-uia-{}.txt", std::process::id()));
    let _ = std::fs::remove_file(&result_path);

    let _fixture = FixtureGuard(launch_wpf_fixture(&title, &result_path, "k.,jdm"));
    let hwnd = wait_for_window(&title, Duration::from_secs(8)).expect("fixture window not found");
    unsafe {
        SetForegroundWindow(hwnd);
    }
    thread::sleep(Duration::from_millis(700));

    let exe = std::env::var("CARGO_BIN_EXE_stepler-cli")
        .unwrap_or_else(|_| String::from("target\\debug\\stepler-cli.exe"));
    let output = Command::new(exe)
        .arg("pause")
        .arg("--apply")
        .env("STEPLER_TEST_FOREGROUND_HWND", hwnd.to_string())
        .output()
        .expect("failed to run stepler-cli pause --apply");
    assert!(
        output.status.success(),
        "stepler-cli failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("method: \"uia_text\""),
        "expected UIAutomationText apply method\nstdout:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );

    let text = wait_for_file_text(&result_path, "любовь", Duration::from_secs(3))
        .expect("fixture did not observe replacement text");
    assert_eq!(text, "любовь");

    let _ = std::fs::remove_file(result_path);
}

#[test]
#[ignore = "launches a visible WPF fixture and drives UI Automation against it"]
fn uia_value_pattern_scrolllock_does_not_duplicate_prefix() {
    let title = format!("Stepler UIA Fixture Scroll {}", std::process::id());
    let result_path =
        std::env::temp_dir().join(format!("stepler-uia-scroll-{}.txt", std::process::id()));
    let _ = std::fs::remove_file(&result_path);
    let source = "house вальс поле long привет vbh";

    let _fixture = FixtureGuard(launch_wpf_fixture(&title, &result_path, source));
    let hwnd = wait_for_window(&title, Duration::from_secs(8)).expect("fixture window not found");
    unsafe {
        SetForegroundWindow(hwnd);
    }
    thread::sleep(Duration::from_millis(700));

    let exe = std::env::var("CARGO_BIN_EXE_stepler-cli")
        .unwrap_or_else(|_| String::from("target\\debug\\stepler-cli.exe"));
    let output = Command::new(exe)
        .arg("scrolllock")
        .arg("--apply")
        .env("STEPLER_TEST_FOREGROUND_HWND", hwnd.to_string())
        .output()
        .expect("failed to run stepler-cli scrolllock --apply");
    assert!(
        output.status.success(),
        "stepler-cli failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("method: \"uia_text\""),
        "expected UIAutomationText apply method\nstdout:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );

    let text = wait_for_file_text(
        &result_path,
        "house вальс поле long привет мир",
        Duration::from_secs(3),
    )
    .expect("fixture did not observe replacement text");
    assert_eq!(text, "house вальс поле long привет мир");

    let _ = std::fs::remove_file(result_path);
}

#[test]
#[ignore = "launches a visible WPF fixture and verifies sparse UIA ScrollLock replacement"]
fn uia_value_pattern_scrolllock_replaces_single_sparse_word() {
    let title = format!("Stepler UIA Fixture Sparse {}", std::process::id());
    let result_path =
        std::env::temp_dir().join(format!("stepler-uia-sparse-{}.txt", std::process::id()));
    let _ = std::fs::remove_file(&result_path);
    let source = "house dfkmc поле long привет мир";

    let _fixture = FixtureGuard(launch_wpf_fixture(&title, &result_path, source));
    let hwnd = wait_for_window(&title, Duration::from_secs(8)).expect("fixture window not found");
    unsafe {
        SetForegroundWindow(hwnd);
    }
    thread::sleep(Duration::from_millis(700));

    let exe = std::env::var("CARGO_BIN_EXE_stepler-cli")
        .unwrap_or_else(|_| String::from("target\\debug\\stepler-cli.exe"));
    let output = Command::new(exe)
        .arg("scrolllock")
        .arg("--apply")
        .env("STEPLER_TEST_FOREGROUND_HWND", hwnd.to_string())
        .output()
        .expect("failed to run stepler-cli scrolllock --apply");
    assert!(
        output.status.success(),
        "stepler-cli failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let text = wait_for_file_text(
        &result_path,
        "house вальс поле long привет мир",
        Duration::from_secs(3),
    )
    .expect("fixture did not observe sparse replacement text");
    assert_eq!(text, "house вальс поле long привет мир");

    let _ = std::fs::remove_file(result_path);
}

#[test]
#[ignore = "launches a visible WPF fixture and verifies UIA caret-aware replacement"]
fn uia_value_pattern_pause_uses_word_before_caret_and_restores_caret() {
    let title = format!("Stepler UIA Fixture Caret {}", std::process::id());
    let result_path =
        std::env::temp_dir().join(format!("stepler-uia-caret-{}.txt", std::process::id()));
    let _ = std::fs::remove_file(&result_path);
    let source = "house k.,jdm tail";
    let caret = "house k.,jdm".encode_utf16().count();

    let _fixture = FixtureGuard(launch_wpf_fixture_with_caret(
        &title,
        &result_path,
        source,
        caret,
        true,
    ));
    let hwnd = wait_for_window(&title, Duration::from_secs(8)).expect("fixture window not found");
    unsafe {
        SetForegroundWindow(hwnd);
    }
    thread::sleep(Duration::from_millis(700));

    let exe = std::env::var("CARGO_BIN_EXE_stepler-cli")
        .unwrap_or_else(|_| String::from("target\\debug\\stepler-cli.exe"));
    let output = Command::new(exe)
        .arg("pause")
        .arg("--apply")
        .env("STEPLER_TEST_FOREGROUND_HWND", hwnd.to_string())
        .output()
        .expect("failed to run stepler-cli pause --apply");
    assert!(
        output.status.success(),
        "stepler-cli failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let expected_text = "house любовь tail";
    let expected_caret = "house любовь".encode_utf16().count();
    let expected = format!("{expected_text}|{expected_caret}");
    let observed =
        wait_for_file_text(&result_path, &expected, Duration::from_secs(3)).unwrap_or_default();
    assert_eq!(observed, expected);

    let _ = std::fs::remove_file(result_path);
}

struct FixtureGuard(Child);

impl Drop for FixtureGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn launch_wpf_fixture(title: &str, result_path: &std::path::Path, initial_text: &str) -> Child {
    launch_wpf_fixture_with_caret(
        title,
        result_path,
        initial_text,
        initial_text.encode_utf16().count(),
        false,
    )
}

fn launch_wpf_fixture_with_caret(
    title: &str,
    result_path: &std::path::Path,
    initial_text: &str,
    caret_utf16: usize,
    include_caret_in_result: bool,
) -> Child {
    let result_expression = if include_caret_in_result {
        "$textbox.Text + '|' + $textbox.CaretIndex"
    } else {
        "$textbox.Text"
    };
    let script = format!(
        r#"
Add-Type -AssemblyName PresentationFramework
Add-Type -AssemblyName WindowsBase
$window = New-Object System.Windows.Window
$window.Title = '{title}'
$window.Width = 420
$window.Height = 120
$textbox = New-Object System.Windows.Controls.TextBox
[System.Windows.Automation.AutomationProperties]::SetAutomationId($textbox, 'SteplerUiaFixtureInput')
$textbox.Text = '{initial_text}'
$textbox.CaretIndex = {caret_utf16}
$textbox.FontSize = 24
$textbox.Margin = '16'
$window.Content = $textbox
$timer = New-Object System.Windows.Threading.DispatcherTimer
$timer.Interval = [TimeSpan]::FromMilliseconds(100)
$timer.Add_Tick({{
  [System.IO.File]::WriteAllText('{result_path}', ({result_expression}), [System.Text.Encoding]::UTF8)
}})
$window.Add_Loaded({{
  $timer.Start()
  $window.Activate() | Out-Null
  $textbox.Focus() | Out-Null
  $textbox.CaretIndex = {caret_utf16}
  [System.Windows.Input.Keyboard]::Focus($textbox) | Out-Null
}})
$window.ShowDialog() | Out-Null
"#,
        title = ps_escape(title),
        initial_text = ps_escape(initial_text),
        result_path = ps_escape(&result_path.display().to_string()),
        caret_utf16 = caret_utf16,
        result_expression = result_expression
    );

    Command::new("powershell.exe")
        .arg("-NoLogo")
        .arg("-NoProfile")
        .arg("-STA")
        .arg("-Command")
        .arg(script)
        .spawn()
        .expect("failed to launch WPF UIA fixture")
}

fn wait_for_window(title: &str, timeout: Duration) -> Option<isize> {
    let title = to_wide(title);
    let started = Instant::now();
    while started.elapsed() < timeout {
        let hwnd = unsafe { FindWindowW(std::ptr::null(), title.as_ptr()) };
        if hwnd != 0 {
            return Some(hwnd);
        }
        thread::sleep(Duration::from_millis(100));
    }
    None
}

fn wait_for_file_text(path: &std::path::Path, expected: &str, timeout: Duration) -> Option<String> {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if let Ok(text) = std::fs::read_to_string(path) {
            let normalized = text.trim_start_matches('\u{feff}').to_owned();
            if normalized == expected {
                return Some(normalized);
            }
        }
        thread::sleep(Duration::from_millis(100));
    }
    std::fs::read_to_string(path)
        .ok()
        .map(|text| text.trim_start_matches('\u{feff}').to_owned())
}

fn ps_escape(value: &str) -> String {
    value.replace('\'', "''")
}

fn to_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[link(name = "user32")]
unsafe extern "system" {
    fn FindWindowW(class_name: *const u16, window_name: *const u16) -> isize;
    fn SetForegroundWindow(hwnd: isize) -> i32;
}
