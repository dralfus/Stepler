use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct WebKeyboardTimingProfile {
    pub selected_timeout: Duration,
    pub short_context_timeout: Duration,
    pub line_context_timeout: Duration,
    pub clipboard_timeout: Duration,
    pub retry_pause: Duration,
    pub attempt_pause: Duration,
}

pub(super) fn web_keyboard_timing_profile(profile: WebKeyboardProfile) -> WebKeyboardTimingProfile {
    if web_keyboard_profile_is_fast(profile) {
        WebKeyboardTimingProfile {
            selected_timeout: Duration::from_millis(120),
            short_context_timeout: Duration::from_millis(180),
            line_context_timeout: Duration::from_millis(260),
            clipboard_timeout: Duration::from_millis(120),
            retry_pause: Duration::from_millis(30),
            attempt_pause: Duration::from_millis(60),
        }
    } else {
        WebKeyboardTimingProfile {
            selected_timeout: Duration::from_millis(220),
            short_context_timeout: Duration::from_millis(280),
            line_context_timeout: Duration::from_millis(450),
            clipboard_timeout: Duration::from_millis(450),
            retry_pause: Duration::from_millis(80),
            attempt_pause: Duration::from_millis(180),
        }
    }
}

pub(super) fn web_keyboard_control_prefix(base: &str, profile: WebKeyboardProfile) -> &str {
    if web_keyboard_profile_is_rocket(profile) {
        match base {
            "web-keyboard-selection" => "web-keyboard-rocket-fast-selection",
            "web-keyboard-line-selection" => "web-keyboard-rocket-fast-line-selection",
            _ => base,
        }
    } else if web_keyboard_profile_is_fast(profile) {
        match base {
            "web-keyboard-selection" => "web-keyboard-fast-selection",
            "web-keyboard-line-selection" => "web-keyboard-fast-line-selection",
            _ => base,
        }
    } else {
        base
    }
}

pub(super) fn web_keyboard_profile_is_fast(profile: WebKeyboardProfile) -> bool {
    if !env_flag_enabled("STEPLER_ENABLE_WEB_FAST_PROFILE", true) {
        return false;
    }
    profile != WebKeyboardProfile::Standard
}

pub(super) fn web_keyboard_profile_is_rocket(profile: WebKeyboardProfile) -> bool {
    if !env_flag_enabled("STEPLER_ENABLE_WEB_FAST_PROFILE", true) {
        return false;
    }
    profile == WebKeyboardProfile::RocketSearch
}

#[cfg(all(windows, test))]
pub(super) fn web_keyboard_fast_profile_title_matches(title: &str) -> bool {
    let target = ForegroundTarget {
        app_class: String::from("Chrome_WidgetWin_1"),
        focused_class: String::from("Chrome_WidgetWin_1"),
        title: title.to_owned(),
        process_name: None,
        window_id: String::new(),
        control_id: String::new(),
    };
    web_keyboard_profile_is_fast(web_keyboard_effective_profile_for_title(
        web_keyboard_profile_for_surface(classify_surface(&target).kind),
        title,
    ))
}

#[cfg(all(windows, test))]
pub(super) fn web_keyboard_rocket_fast_profile_title_matches(title: &str) -> bool {
    let target = ForegroundTarget {
        app_class: String::from("Chrome_WidgetWin_1"),
        focused_class: String::from("Chrome_WidgetWin_1"),
        title: title.to_owned(),
        process_name: None,
        window_id: String::new(),
        control_id: String::new(),
    };
    web_keyboard_profile_is_rocket(web_keyboard_profile_for_surface(
        classify_surface(&target).kind,
    ))
}
