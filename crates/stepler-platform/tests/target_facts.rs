use stepler_platform::{classify_surface, target_facts, ForegroundTarget, SurfaceKind};

#[test]
fn facts_detect_rocket_chat_title_and_process() {
    let target = target(
        "Chrome_WidgetWin_1",
        "Chrome_WidgetWin_1",
        Some("Rocket.Chat"),
        "Нет непрочитанных сообщений",
    );

    let facts = target_facts(&target);

    assert!(facts.is_browser_editor_class);
    assert!(facts.is_browser_like_technical_target);
    assert!(facts.is_rocket_chat);
    assert_eq!(
        classify_surface(&target).kind,
        SurfaceKind::RocketChatEditor
    );
}

#[test]
fn facts_detect_fast_browser_titles_without_turning_unknown_into_browser() {
    for title in ["Codex", "[CTP-11796] GS-Labs JIRA", "GS-Labs Wiki"] {
        let browser = target(
            "Chrome_WidgetWin_1",
            "Chrome_WidgetWin_1",
            Some("chrome"),
            title,
        );
        let browser_facts = target_facts(&browser);

        assert!(browser_facts.is_fast_browser_title, "{title}");
        assert!(browser_facts.is_browser_editor_class, "{title}");
        assert_eq!(
            classify_surface(&browser).kind,
            SurfaceKind::FastBrowserEditor,
            "{title}"
        );

        let adjacent = target(
            "CustomChromiumShell",
            "CustomControl",
            Some("custom"),
            title,
        );
        let adjacent_facts = target_facts(&adjacent);

        assert!(adjacent_facts.is_fast_browser_title, "{title}");
        assert!(!adjacent_facts.is_browser_editor_class, "{title}");
        assert!(!adjacent_facts.is_browser_like_technical_target, "{title}");
        assert_eq!(
            classify_surface(&adjacent).kind,
            SurfaceKind::Unknown,
            "{title}"
        );
    }
}

#[test]
fn facts_detect_sticky_notes_and_telegram_boundaries() {
    let sticky = target(
        "ApplicationFrameWindow",
        "Windows.UI.Input.InputSite.WindowClass",
        Some("Microsoft.Notes"),
        "Sticky Notes",
    );
    let sticky_facts = target_facts(&sticky);
    assert!(sticky_facts.is_sticky_notes);
    assert_eq!(classify_surface(&sticky).kind, SurfaceKind::StickyNotes);

    let telegram_process = target(
        "CustomQtWindow",
        "CustomQtWindow",
        Some("Telegram"),
        "Telegram",
    );
    let telegram_process_facts = target_facts(&telegram_process);
    assert!(telegram_process_facts.is_telegram_process);
    assert!(telegram_process_facts.is_telegram_technical_target);
    assert_eq!(
        classify_surface(&telegram_process).kind,
        SurfaceKind::TelegramDesktop
    );

    let qt_chat = target(
        "Qt51518QWindowIcon",
        "Qt51518QWindowIcon",
        None,
        "Contact @ user",
    );
    let qt_chat_facts = target_facts(&qt_chat);
    assert!(qt_chat_facts.is_telegram_classifier_class);
    assert!(qt_chat_facts.is_telegram_qt_chat_title);
    assert!(qt_chat_facts.is_telegram_technical_target);

    let qt_without_chat_title =
        target("Qt51518QWindowIcon", "Qt51518QWindowIcon", None, "Telegram");
    let qt_without_chat_title_facts = target_facts(&qt_without_chat_title);
    assert!(qt_without_chat_title_facts.is_telegram_classifier_class);
    assert!(!qt_without_chat_title_facts.is_telegram_qt_chat_title);
    assert!(!qt_without_chat_title_facts.is_telegram_technical_target);
    assert_eq!(
        classify_surface(&qt_without_chat_title).kind,
        SurfaceKind::TelegramDesktop
    );
}

#[test]
fn facts_keep_yandex_as_separate_surface_but_browser_like_technical_target() {
    let target = target(
        "Chrome_Yandex_WidgetWin_1",
        "Chrome_Yandex_WidgetWin_1",
        Some("browser"),
        "Yandex",
    );

    let facts = target_facts(&target);

    assert!(facts.is_yandex_browser_widget_class);
    assert!(!facts.is_browser_editor_class);
    assert!(facts.is_browser_like_technical_target);
    assert_eq!(
        classify_surface(&target).kind,
        SurfaceKind::YandexBrowserEditor
    );
}

#[test]
fn facts_detect_whatsapp_desktop_as_browser_editor() {
    let target = target(
        "WinUIDesktopWin32WindowClass",
        "Chrome_WidgetWin_0",
        Some("WhatsApp"),
        "WhatsApp",
    );

    let facts = target_facts(&target);

    assert!(facts.is_whatsapp_desktop);
    assert!(!facts.is_browser_editor_class);
    assert!(facts.is_browser_like_technical_target);
    assert_eq!(classify_surface(&target).kind, SurfaceKind::BrowserEditor);
}

fn target(
    app_class: &str,
    focused_class: &str,
    process_name: Option<&str>,
    title: &str,
) -> ForegroundTarget {
    ForegroundTarget {
        app_class: app_class.to_owned(),
        focused_class: focused_class.to_owned(),
        title: title.to_owned(),
        process_name: process_name.map(str::to_owned),
        window_id: String::from("fixture-window"),
        control_id: String::from("fixture-control"),
    }
}
