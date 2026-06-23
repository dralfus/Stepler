use super::*;

pub(super) fn is_web_keyboard_technical_target(target: &ForegroundTarget) -> bool {
    is_browser_like_target(target)
        || is_telegram_target(target)
        || is_notepad_like_target(target)
        || is_sticky_notes_target(target)
}

pub(super) fn is_browser_like_target(target: &ForegroundTarget) -> bool {
    target
        .app_class
        .to_ascii_lowercase()
        .starts_with("chrome_widgetwin")
        || target
            .app_class
            .to_ascii_lowercase()
            .starts_with("chrome_yandex_widgetwin")
        || target.app_class.eq_ignore_ascii_case("MozillaWindowClass")
        || target
            .focused_class
            .eq_ignore_ascii_case("Chrome_RenderWidgetHostHWND")
}

pub(super) fn is_telegram_target(target: &ForegroundTarget) -> bool {
    target
        .process_name
        .as_deref()
        .is_some_and(|process| process.eq_ignore_ascii_case("Telegram"))
        || is_telegram_qt_class(&target.app_class) && target.title.contains('@')
}

fn is_telegram_qt_class(class_name: &str) -> bool {
    let class_name = class_name.to_ascii_lowercase();
    class_name.starts_with("qt") && class_name.ends_with("qwindowicon")
}

fn is_notepad_like_target(target: &ForegroundTarget) -> bool {
    target.title.to_ascii_lowercase().contains("notepad")
        || target
            .process_name
            .as_deref()
            .is_some_and(|process| process.eq_ignore_ascii_case("Notepad"))
        || target.app_class.eq_ignore_ascii_case("Notepad")
}

fn is_sticky_notes_target(target: &ForegroundTarget) -> bool {
    target.title.to_ascii_lowercase().contains("sticky notes")
        || target
            .process_name
            .as_deref()
            .is_some_and(|process| process.eq_ignore_ascii_case("Microsoft.Notes"))
}

pub(super) fn has_terminal_app_marker(target: &ForegroundTarget) -> bool {
    has_active_terminal_app_marker()
        || target
            .title
            .to_ascii_lowercase()
            .contains("stepler-terminal-app")
}
