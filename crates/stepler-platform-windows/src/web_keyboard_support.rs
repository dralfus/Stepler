use super::*;

pub(super) fn is_web_keyboard_technical_target(target: &ForegroundTarget) -> bool {
    let facts = stepler_platform::target_facts(target);
    facts.is_browser_like_technical_target
        || facts.is_telegram_technical_target
        || facts.is_notepad_like
        || facts.is_sticky_notes
}

pub(super) fn is_browser_like_target(target: &ForegroundTarget) -> bool {
    stepler_platform::target_facts(target).is_browser_like_technical_target
}

#[cfg(test)]
pub(super) fn is_telegram_target(target: &ForegroundTarget) -> bool {
    stepler_platform::target_facts(target).is_telegram_technical_target
}

pub(super) fn has_terminal_app_marker(target: &ForegroundTarget) -> bool {
    has_active_terminal_app_marker()
        || stepler_platform::target_facts(target).title_has_terminal_app_marker
}
