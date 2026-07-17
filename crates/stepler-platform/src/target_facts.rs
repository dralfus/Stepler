use crate::ForegroundTarget;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetFacts {
    pub is_classic_console: bool,
    pub is_outlook_process: bool,
    pub is_outlook_app_class: bool,
    pub is_outlook_search_edit: bool,
    pub is_outlook_word_editor: bool,
    pub is_word_process: bool,
    pub is_word_app_class: bool,
    pub is_win32_edit: bool,
    pub is_notepad_like: bool,
    pub is_sticky_notes: bool,
    pub is_windows_terminal: bool,
    pub is_windows_terminal_cmd_title: bool,
    pub is_qwen_terminal_title_or_marker: bool,
    pub is_telegram_process: bool,
    pub is_telegram_classifier_class: bool,
    pub is_telegram_qt_window_icon_class: bool,
    pub is_telegram_qt_chat_title: bool,
    pub is_telegram_technical_target: bool,
    pub is_browser_editor_class: bool,
    pub is_yandex_browser_widget_class: bool,
    pub is_whatsapp_desktop: bool,
    pub is_browser_like_technical_target: bool,
    pub is_fast_browser_title: bool,
    pub is_rocket_chat: bool,
    pub title_has_terminal_app_marker: bool,
}

pub fn target_facts(target: &ForegroundTarget) -> TargetFacts {
    let app = target.app_class.as_str();
    let focused = target.focused_class.as_str();
    let title = target.title.as_str();
    let process = target.process_name.as_deref().unwrap_or_default();

    let is_outlook_process = process_eq(process, "OUTLOOK");
    let is_outlook_app_class = class_eq(app, "rctrl_renwnd32");
    let is_word_process = process_eq(process, "WINWORD");
    let is_word_app_class = class_eq(app, "OpusApp");
    let is_telegram_process = process_eq(process, "Telegram");
    let is_telegram_classifier_class = class_eq(app, "Qt51518QWindowIcon");
    let is_telegram_qt_window_icon_class = is_qt_window_icon_class(app);
    let is_telegram_qt_chat_title = is_telegram_qt_window_icon_class && title.contains('@');
    let is_browser_editor_class = class_starts(app, "Chrome_WidgetWin")
        || class_eq(app, "MozillaWindowClass")
        || class_eq(focused, "Chrome_RenderWidgetHostHWND");
    let is_yandex_browser_widget_class = class_starts(app, "Chrome_Yandex_WidgetWin");
    let is_whatsapp_desktop = (process_eq(process, "WhatsApp")
        || title_contains(title, "WhatsApp"))
        && class_eq(app, "WinUIDesktopWin32WindowClass")
        && class_starts(focused, "Chrome_WidgetWin");

    TargetFacts {
        is_classic_console: class_eq(app, "ConsoleWindowClass")
            && class_eq(focused, "ConsoleWindowClass"),
        is_outlook_process,
        is_outlook_app_class,
        is_outlook_search_edit: is_outlook_process && class_eq(focused, "Edit"),
        is_outlook_word_editor: is_outlook_process && class_eq(focused, "_WwG"),
        is_word_process,
        is_word_app_class,
        is_win32_edit: class_eq(focused, "Edit"),
        is_notepad_like: title_contains(title, "Notepad")
            || process_eq(process, "Notepad")
            || class_eq(app, "Notepad"),
        is_sticky_notes: title_contains(title, "Sticky Notes")
            || process_eq(process, "Microsoft.Notes"),
        is_windows_terminal: class_eq(app, "CASCADIA_HOSTING_WINDOW_CLASS")
            && class_eq(focused, "Windows.UI.Input.InputSite.WindowClass"),
        is_windows_terminal_cmd_title: title_contains(title, "cmd.exe"),
        is_qwen_terminal_title_or_marker: title_contains(title, "qwen")
            || title_contains(title, "stepler-terminal-app"),
        is_telegram_process,
        is_telegram_classifier_class,
        is_telegram_qt_window_icon_class,
        is_telegram_qt_chat_title,
        is_telegram_technical_target: is_telegram_process || is_telegram_qt_chat_title,
        is_browser_editor_class,
        is_yandex_browser_widget_class,
        is_whatsapp_desktop,
        is_browser_like_technical_target: is_browser_editor_class
            || is_yandex_browser_widget_class
            || is_whatsapp_desktop,
        is_fast_browser_title: title_contains(title, "jira")
            || title_contains(title, "confluence")
            || title_contains(title, "gs-labs wiki")
            || title_contains(title, "chips")
            || title_contains(title, "codex"),
        is_rocket_chat: process_eq(process, "Rocket.Chat")
            || title_contains(title, "rocket.chat")
            || title_contains(title, "gs.chat")
            || title_contains(title, "нет непрочитанных")
            || title_contains(title, "unread messages"),
        title_has_terminal_app_marker: title_contains(title, "stepler-terminal-app"),
    }
}

fn class_eq(value: &str, expected: &str) -> bool {
    value.eq_ignore_ascii_case(expected)
}

fn class_starts(value: &str, expected_prefix: &str) -> bool {
    value
        .get(..expected_prefix.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(expected_prefix))
}

fn process_eq(value: &str, expected: &str) -> bool {
    value.eq_ignore_ascii_case(expected)
}

fn title_contains(title: &str, needle: &str) -> bool {
    title.to_lowercase().contains(&needle.to_lowercase())
}

fn is_qt_window_icon_class(class_name: &str) -> bool {
    let class_name = class_name.to_ascii_lowercase();
    class_name.starts_with("qt") && class_name.ends_with("qwindowicon")
}
