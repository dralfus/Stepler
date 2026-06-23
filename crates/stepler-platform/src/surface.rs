use crate::{ForegroundTarget, ALL_METHOD_IDS};
use stepler_core::{CorrectionMode, MethodId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SurfaceKind {
    Win32Edit,
    NotepadLike,
    ClassicConsole,
    WindowsTerminalCmd,
    WindowsTerminalPowerShell,
    QwenTerminal,
    BrowserEditor,
    FastBrowserEditor,
    RocketChatEditor,
    YandexBrowserEditor,
    TelegramDesktop,
    StickyNotes,
    OutlookSearch,
    OutlookWordEditor,
    OutlookShell,
    WordEditor,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WebKeyboardProfile {
    Standard,
    Fast,
    RocketSearch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceClassification {
    pub kind: SurfaceKind,
    pub confidence: u8,
    pub evidence: Vec<String>,
}

impl SurfaceClassification {
    fn new(kind: SurfaceKind, confidence: u8, evidence: Vec<String>) -> Self {
        Self {
            kind,
            confidence,
            evidence,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfacePolicy {
    pub surface: SurfaceKind,
    pub pause_methods: MethodPreferences,
    pub scrolllock_methods: MethodPreferences,
    pub forbidden_methods: Vec<MethodId>,
    pub allow_risky_methods: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbePolicy {
    pub surface: SurfaceKind,
    pub probe_methods: Vec<MethodId>,
    pub suppressed_methods: Vec<MethodId>,
    pub fast_probe: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbePlan {
    pub surface: SurfaceClassification,
    pub probe_methods: Vec<MethodId>,
    pub suppressed_methods: Vec<MethodId>,
    pub fast_probe: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodPreferences {
    pub context_methods: Vec<MethodId>,
    pub replace_methods: Vec<MethodId>,
}

impl SurfacePolicy {
    pub fn matches(&self, classification: &SurfaceClassification) -> bool {
        self.surface == classification.kind
    }

    pub(crate) fn preferences_for(&self, mode: CorrectionMode) -> &MethodPreferences {
        match mode {
            CorrectionMode::Pause => &self.pause_methods,
            CorrectionMode::ScrollLock => &self.scrolllock_methods,
        }
    }
}

pub fn default_app_policies() -> Vec<SurfacePolicy> {
    default_surface_policies()
}

fn same_preferences(methods: Vec<MethodId>) -> MethodPreferences {
    MethodPreferences {
        context_methods: methods.clone(),
        replace_methods: methods,
    }
}

pub fn split_preferences(
    context_methods: Vec<MethodId>,
    replace_methods: Vec<MethodId>,
) -> MethodPreferences {
    MethodPreferences {
        context_methods,
        replace_methods,
    }
}

pub fn default_surface_policies() -> Vec<SurfacePolicy> {
    vec![
        SurfacePolicy {
            surface: SurfaceKind::Win32Edit,
            pause_methods: same_preferences(vec![MethodId::Win32EditMessages]),
            scrolllock_methods: same_preferences(vec![MethodId::Win32EditMessages]),
            forbidden_methods: vec![],
            allow_risky_methods: false,
        },
        SurfacePolicy {
            surface: SurfaceKind::NotepadLike,
            pause_methods: same_preferences(vec![
                MethodId::Win32EditMessages,
                MethodId::WebKeyboardSelection,
                MethodId::UiAutomationEditableText,
                MethodId::UiAutomationDocumentText,
                MethodId::UiAutomationText,
            ]),
            scrolllock_methods: same_preferences(vec![
                MethodId::Win32EditMessages,
                MethodId::WebKeyboardSelection,
                MethodId::UiAutomationEditableText,
                MethodId::UiAutomationDocumentText,
                MethodId::UiAutomationText,
            ]),
            forbidden_methods: vec![
                MethodId::TerminalClipboardShortcut,
                MethodId::ClipboardSelection,
                MethodId::SendInput,
            ],
            allow_risky_methods: false,
        },
        SurfacePolicy {
            surface: SurfaceKind::ClassicConsole,
            pause_methods: same_preferences(vec![MethodId::ConsoleBuffer]),
            scrolllock_methods: same_preferences(vec![MethodId::ConsoleBuffer]),
            forbidden_methods: vec![MethodId::TerminalClipboardShortcut],
            allow_risky_methods: false,
        },
        SurfacePolicy {
            surface: SurfaceKind::WindowsTerminalCmd,
            pause_methods: same_preferences(vec![MethodId::TerminalClipboardShortcut]),
            scrolllock_methods: same_preferences(vec![MethodId::TerminalClipboardShortcut]),
            forbidden_methods: vec![MethodId::PsReadLine],
            allow_risky_methods: true,
        },
        SurfacePolicy {
            surface: SurfaceKind::QwenTerminal,
            pause_methods: same_preferences(vec![MethodId::XtermKeyboardSelection]),
            scrolllock_methods: same_preferences(vec![MethodId::XtermKeyboardSelection]),
            forbidden_methods: vec![MethodId::PsReadLine, MethodId::TerminalClipboardShortcut],
            allow_risky_methods: false,
        },
        SurfacePolicy {
            surface: SurfaceKind::WindowsTerminalPowerShell,
            pause_methods: same_preferences(vec![MethodId::PsReadLine]),
            scrolllock_methods: same_preferences(vec![MethodId::PsReadLine]),
            forbidden_methods: vec![MethodId::TerminalClipboardShortcut],
            allow_risky_methods: false,
        },
        SurfacePolicy {
            surface: SurfaceKind::BrowserEditor,
            pause_methods: same_preferences(vec![
                MethodId::WebKeyboardSelection,
                MethodId::UiAutomationEditableText,
            ]),
            scrolllock_methods: same_preferences(vec![
                MethodId::WebKeyboardSelection,
                MethodId::UiAutomationEditableText,
            ]),
            forbidden_methods: vec![
                MethodId::Win32EditMessages,
                MethodId::UiAutomationDocumentText,
                MethodId::UiAutomationText,
                MethodId::TerminalClipboardShortcut,
                MethodId::ClipboardSelection,
                MethodId::SendInput,
            ],
            allow_risky_methods: false,
        },
        SurfacePolicy {
            surface: SurfaceKind::FastBrowserEditor,
            pause_methods: same_preferences(vec![
                MethodId::WebKeyboardSelection,
                MethodId::UiAutomationEditableText,
            ]),
            scrolllock_methods: same_preferences(vec![
                MethodId::WebKeyboardSelection,
                MethodId::UiAutomationEditableText,
            ]),
            forbidden_methods: vec![
                MethodId::Win32EditMessages,
                MethodId::UiAutomationDocumentText,
                MethodId::UiAutomationText,
                MethodId::TerminalClipboardShortcut,
                MethodId::ClipboardSelection,
                MethodId::SendInput,
            ],
            allow_risky_methods: false,
        },
        SurfacePolicy {
            surface: SurfaceKind::RocketChatEditor,
            pause_methods: same_preferences(vec![
                MethodId::WebKeyboardSelection,
                MethodId::UiAutomationEditableText,
            ]),
            scrolllock_methods: same_preferences(vec![
                MethodId::WebKeyboardSelection,
                MethodId::UiAutomationEditableText,
            ]),
            forbidden_methods: vec![
                MethodId::Win32EditMessages,
                MethodId::UiAutomationDocumentText,
                MethodId::UiAutomationText,
                MethodId::TerminalClipboardShortcut,
                MethodId::ClipboardSelection,
                MethodId::SendInput,
            ],
            allow_risky_methods: false,
        },
        SurfacePolicy {
            surface: SurfaceKind::YandexBrowserEditor,
            pause_methods: same_preferences(vec![
                MethodId::WebKeyboardSelection,
                MethodId::UiAutomationDocumentText,
                MethodId::UiAutomationEditableText,
            ]),
            scrolllock_methods: same_preferences(vec![
                MethodId::WebKeyboardSelection,
                MethodId::UiAutomationDocumentText,
                MethodId::UiAutomationEditableText,
            ]),
            forbidden_methods: vec![
                MethodId::Win32EditMessages,
                MethodId::UiAutomationText,
                MethodId::TerminalClipboardShortcut,
                MethodId::ClipboardSelection,
                MethodId::SendInput,
            ],
            allow_risky_methods: false,
        },
        SurfacePolicy {
            surface: SurfaceKind::TelegramDesktop,
            pause_methods: same_preferences(vec![
                MethodId::WebKeyboardSelection,
                MethodId::UiAutomationEditableText,
                MethodId::UiAutomationDocumentText,
            ]),
            scrolllock_methods: same_preferences(vec![
                MethodId::WebKeyboardSelection,
                MethodId::UiAutomationEditableText,
                MethodId::UiAutomationDocumentText,
            ]),
            forbidden_methods: vec![
                MethodId::Win32EditMessages,
                MethodId::UiAutomationText,
                MethodId::TerminalClipboardShortcut,
                MethodId::ClipboardSelection,
                MethodId::SendInput,
            ],
            allow_risky_methods: false,
        },
        SurfacePolicy {
            surface: SurfaceKind::StickyNotes,
            pause_methods: same_preferences(vec![
                MethodId::UiAutomationDocumentText,
                MethodId::UiAutomationEditableText,
                MethodId::WebKeyboardSelection,
                MethodId::UiAutomationText,
            ]),
            scrolllock_methods: same_preferences(vec![
                MethodId::UiAutomationDocumentText,
                MethodId::UiAutomationEditableText,
                MethodId::WebKeyboardSelection,
                MethodId::UiAutomationText,
            ]),
            forbidden_methods: vec![
                MethodId::Win32EditMessages,
                MethodId::TerminalClipboardShortcut,
                MethodId::ClipboardSelection,
                MethodId::SendInput,
            ],
            allow_risky_methods: false,
        },
        SurfacePolicy {
            surface: SurfaceKind::OutlookSearch,
            pause_methods: same_preferences(vec![MethodId::Win32EditMessages]),
            scrolllock_methods: same_preferences(vec![MethodId::Win32EditMessages]),
            forbidden_methods: vec![
                MethodId::UiAutomationEditableText,
                MethodId::UiAutomationDocumentText,
                MethodId::UiAutomationText,
                MethodId::TerminalClipboardShortcut,
                MethodId::ClipboardSelection,
                MethodId::SendInput,
            ],
            allow_risky_methods: false,
        },
        SurfacePolicy {
            surface: SurfaceKind::OutlookWordEditor,
            pause_methods: same_preferences(vec![MethodId::WordCom]),
            scrolllock_methods: same_preferences(vec![MethodId::WordCom]),
            forbidden_methods: vec![
                MethodId::Win32EditMessages,
                MethodId::UiAutomationEditableText,
                MethodId::UiAutomationDocumentText,
                MethodId::UiAutomationText,
                MethodId::TerminalClipboardShortcut,
                MethodId::ClipboardSelection,
                MethodId::SendInput,
            ],
            allow_risky_methods: false,
        },
        SurfacePolicy {
            surface: SurfaceKind::OutlookShell,
            pause_methods: same_preferences(vec![MethodId::Win32EditMessages, MethodId::WordCom]),
            scrolllock_methods: same_preferences(vec![
                MethodId::Win32EditMessages,
                MethodId::WordCom,
            ]),
            forbidden_methods: vec![
                MethodId::UiAutomationEditableText,
                MethodId::UiAutomationDocumentText,
                MethodId::UiAutomationText,
                MethodId::TerminalClipboardShortcut,
                MethodId::ClipboardSelection,
                MethodId::SendInput,
            ],
            allow_risky_methods: false,
        },
        SurfacePolicy {
            surface: SurfaceKind::WordEditor,
            pause_methods: same_preferences(vec![
                MethodId::WordCom,
                MethodId::UiAutomationEditableText,
                MethodId::UiAutomationDocumentText,
                MethodId::UiAutomationText,
            ]),
            scrolllock_methods: same_preferences(vec![
                MethodId::WordCom,
                MethodId::UiAutomationEditableText,
                MethodId::UiAutomationDocumentText,
                MethodId::UiAutomationText,
            ]),
            forbidden_methods: vec![
                MethodId::Win32EditMessages,
                MethodId::TerminalClipboardShortcut,
                MethodId::ClipboardSelection,
            ],
            allow_risky_methods: false,
        },
        default_surface_policy(),
    ]
}

pub fn default_app_policy() -> SurfacePolicy {
    default_surface_policy()
}

pub fn surface_policy_for(kind: SurfaceKind) -> SurfacePolicy {
    let classification = SurfaceClassification::new(kind, 100, vec![String::from("explicit")]);
    default_surface_policies()
        .into_iter()
        .find(|policy| policy.matches(&classification))
        .unwrap_or_else(default_surface_policy)
}

pub fn default_probe_policies() -> Vec<ProbePolicy> {
    vec![
        probe_policy(
            SurfaceKind::Win32Edit,
            vec![MethodId::Win32EditMessages],
            false,
        ),
        probe_policy(
            SurfaceKind::NotepadLike,
            vec![
                MethodId::Win32EditMessages,
                MethodId::WebKeyboardSelection,
                MethodId::UiAutomationEditableText,
                MethodId::UiAutomationDocumentText,
                MethodId::UiAutomationText,
            ],
            false,
        ),
        probe_policy(
            SurfaceKind::ClassicConsole,
            vec![MethodId::ConsoleBuffer],
            false,
        ),
        probe_policy(
            SurfaceKind::WindowsTerminalCmd,
            vec![MethodId::TerminalClipboardShortcut],
            false,
        ),
        probe_policy(
            SurfaceKind::QwenTerminal,
            vec![MethodId::XtermKeyboardSelection],
            false,
        ),
        probe_policy(
            SurfaceKind::WindowsTerminalPowerShell,
            vec![MethodId::PsReadLine],
            false,
        ),
        probe_policy(
            SurfaceKind::BrowserEditor,
            vec![
                MethodId::WebKeyboardSelection,
                MethodId::UiAutomationEditableText,
            ],
            false,
        ),
        probe_policy(
            SurfaceKind::FastBrowserEditor,
            vec![
                MethodId::WebKeyboardSelection,
                MethodId::UiAutomationEditableText,
            ],
            true,
        ),
        probe_policy(
            SurfaceKind::RocketChatEditor,
            vec![
                MethodId::WebKeyboardSelection,
                MethodId::UiAutomationEditableText,
            ],
            true,
        ),
        probe_policy(
            SurfaceKind::YandexBrowserEditor,
            vec![
                MethodId::WebKeyboardSelection,
                MethodId::UiAutomationDocumentText,
                MethodId::UiAutomationEditableText,
            ],
            false,
        ),
        probe_policy(
            SurfaceKind::TelegramDesktop,
            vec![
                MethodId::WebKeyboardSelection,
                MethodId::UiAutomationEditableText,
            ],
            false,
        ),
        probe_policy(
            SurfaceKind::StickyNotes,
            vec![
                MethodId::UiAutomationDocumentText,
                MethodId::WebKeyboardSelection,
                MethodId::UiAutomationEditableText,
            ],
            false,
        ),
        probe_policy(
            SurfaceKind::OutlookSearch,
            vec![MethodId::Win32EditMessages],
            false,
        ),
        probe_policy(
            SurfaceKind::OutlookWordEditor,
            vec![MethodId::WordCom],
            false,
        ),
        probe_policy(
            SurfaceKind::OutlookShell,
            vec![MethodId::Win32EditMessages, MethodId::WordCom],
            false,
        ),
        probe_policy(SurfaceKind::WordEditor, vec![MethodId::WordCom], false),
        probe_policy(SurfaceKind::Unknown, ALL_METHOD_IDS.to_vec(), false),
    ]
}

pub fn probe_plan_for(target: &ForegroundTarget) -> ProbePlan {
    let surface = classify_surface(target);
    let policy = probe_policy_for(surface.kind);
    ProbePlan {
        surface,
        probe_methods: policy.probe_methods,
        suppressed_methods: policy.suppressed_methods,
        fast_probe: policy.fast_probe,
    }
}

pub fn probe_policy_for(kind: SurfaceKind) -> ProbePolicy {
    default_probe_policies()
        .into_iter()
        .find(|policy| policy.surface == kind)
        .unwrap_or_else(|| probe_policy(SurfaceKind::Unknown, ALL_METHOD_IDS.to_vec(), false))
}

pub fn default_surface_policy() -> SurfacePolicy {
    SurfacePolicy {
        surface: SurfaceKind::Unknown,
        pause_methods: same_preferences(vec![
            MethodId::Win32EditMessages,
            MethodId::UiAutomationEditableText,
            MethodId::UiAutomationDocumentText,
            MethodId::UiAutomationText,
            MethodId::WebKeyboardSelection,
            MethodId::ConsoleBuffer,
            MethodId::PsReadLine,
            MethodId::ClipboardSelection,
            MethodId::SendInput,
        ]),
        scrolllock_methods: same_preferences(vec![
            MethodId::Win32EditMessages,
            MethodId::UiAutomationEditableText,
            MethodId::UiAutomationDocumentText,
            MethodId::UiAutomationText,
            MethodId::WebKeyboardSelection,
            MethodId::ConsoleBuffer,
            MethodId::PsReadLine,
            MethodId::ClipboardSelection,
            MethodId::SendInput,
        ]),
        forbidden_methods: vec![],
        allow_risky_methods: risky_fallbacks_enabled(),
    }
}

fn probe_policy(
    surface: SurfaceKind,
    probe_methods: Vec<MethodId>,
    fast_probe: bool,
) -> ProbePolicy {
    let suppressed_methods = ALL_METHOD_IDS
        .iter()
        .copied()
        .filter(|method| !probe_methods.contains(method))
        .collect();
    ProbePolicy {
        surface,
        probe_methods,
        suppressed_methods,
        fast_probe,
    }
}

pub fn classify_surface(target: &ForegroundTarget) -> SurfaceClassification {
    let app = target.app_class.as_str();
    let focused = target.focused_class.as_str();
    let title = target.title.as_str();
    let process = target.process_name.as_deref().unwrap_or_default();

    if class_eq(app, "ConsoleWindowClass") && class_eq(focused, "ConsoleWindowClass") {
        return surface(
            SurfaceKind::ClassicConsole,
            100,
            vec![
                "app_class=ConsoleWindowClass",
                "focused_class=ConsoleWindowClass",
            ],
        );
    }

    if process_eq(process, "OUTLOOK") && class_eq(focused, "Edit") {
        return surface(
            SurfaceKind::OutlookSearch,
            100,
            vec!["process=OUTLOOK", "focused_class=Edit"],
        );
    }

    if process_eq(process, "OUTLOOK") && class_eq(focused, "_WwG") {
        return surface(
            SurfaceKind::OutlookWordEditor,
            100,
            vec!["process=OUTLOOK", "focused_class=_WwG"],
        );
    }

    if process_eq(process, "OUTLOOK") || class_eq(app, "rctrl_renwnd32") {
        return surface(
            SurfaceKind::OutlookShell,
            90,
            vec!["process=OUTLOOK or app_class=rctrl_renwnd32"],
        );
    }

    if process_eq(process, "WINWORD") || class_eq(app, "OpusApp") {
        return surface(
            SurfaceKind::WordEditor,
            95,
            vec!["process=WINWORD or app_class=OpusApp"],
        );
    }

    if class_eq(focused, "Edit") {
        return surface(SurfaceKind::Win32Edit, 95, vec!["focused_class=Edit"]);
    }

    if target_is_notepad_like(target) {
        return surface(
            SurfaceKind::NotepadLike,
            90,
            vec!["notepad-like app/process/title"],
        );
    }

    if target_is_sticky_notes(target) {
        return surface(
            SurfaceKind::StickyNotes,
            95,
            vec!["Sticky Notes title/process/app frame"],
        );
    }

    if target_is_windows_terminal(target) {
        if title_contains(title, "cmd.exe") {
            return surface(
                SurfaceKind::WindowsTerminalCmd,
                100,
                vec!["windows terminal", "title contains cmd.exe"],
            );
        }
        if title_contains(title, "qwen") || title_contains(title, "stepler-terminal-app") {
            return surface(
                SurfaceKind::QwenTerminal,
                100,
                vec!["windows terminal", "title/marker indicates qwen"],
            );
        }
        return surface(
            SurfaceKind::WindowsTerminalPowerShell,
            90,
            vec!["windows terminal default local shell"],
        );
    }

    if target_is_telegram(target) {
        return surface(
            SurfaceKind::TelegramDesktop,
            95,
            vec!["process=Telegram or Qt window"],
        );
    }

    if target_is_browser_editor(target) && target_is_rocket_chat(target) {
        return surface(
            SurfaceKind::RocketChatEditor,
            98,
            vec!["browser/electron editor class", "Rocket.Chat title/process"],
        );
    }

    if target_is_browser_editor(target) && target_is_fast_browser_editor(target) {
        return surface(
            SurfaceKind::FastBrowserEditor,
            96,
            vec![
                "browser/electron editor class",
                "known fast browser editor title/process",
            ],
        );
    }

    if class_starts(app, "Chrome_Yandex_WidgetWin") {
        return surface(
            SurfaceKind::YandexBrowserEditor,
            95,
            vec!["app_class=Chrome_Yandex_WidgetWin*"],
        );
    }

    if target_is_browser_editor(target) {
        return surface(
            SurfaceKind::BrowserEditor,
            90,
            vec!["browser/electron editor class"],
        );
    }

    surface(SurfaceKind::Unknown, 10, vec!["no explicit surface match"])
}

pub fn surface_allows_risky_method(kind: SurfaceKind, method: MethodId) -> bool {
    matches!(
        (kind, method),
        (
            SurfaceKind::WindowsTerminalCmd,
            MethodId::TerminalClipboardShortcut
        )
    )
}

pub fn surface_uses_fast_web_keyboard(kind: SurfaceKind) -> bool {
    web_keyboard_profile_for_surface(kind) != WebKeyboardProfile::Standard
}

pub fn surface_uses_rocket_web_keyboard(kind: SurfaceKind) -> bool {
    web_keyboard_profile_for_surface(kind) == WebKeyboardProfile::RocketSearch
}

pub fn web_keyboard_profile_for_surface(kind: SurfaceKind) -> WebKeyboardProfile {
    match kind {
        SurfaceKind::RocketChatEditor => WebKeyboardProfile::RocketSearch,
        SurfaceKind::FastBrowserEditor => WebKeyboardProfile::Fast,
        _ => WebKeyboardProfile::Standard,
    }
}

fn surface(kind: SurfaceKind, confidence: u8, evidence: Vec<&str>) -> SurfaceClassification {
    SurfaceClassification::new(
        kind,
        confidence,
        evidence.into_iter().map(str::to_owned).collect(),
    )
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

fn target_is_windows_terminal(target: &ForegroundTarget) -> bool {
    class_eq(&target.app_class, "CASCADIA_HOSTING_WINDOW_CLASS")
        && class_eq(
            &target.focused_class,
            "Windows.UI.Input.InputSite.WindowClass",
        )
}

fn target_is_browser_editor(target: &ForegroundTarget) -> bool {
    class_starts(&target.app_class, "Chrome_WidgetWin")
        || class_eq(&target.app_class, "MozillaWindowClass")
        || class_eq(&target.focused_class, "Chrome_RenderWidgetHostHWND")
}

fn target_is_fast_browser_editor(target: &ForegroundTarget) -> bool {
    let title = target.title.as_str();
    title_contains(title, "jira")
        || title_contains(title, "confluence")
        || title_contains(title, "gs-labs wiki")
        || title_contains(title, "chips")
        || title_contains(title, "codex")
}

fn target_is_rocket_chat(target: &ForegroundTarget) -> bool {
    process_eq(
        target.process_name.as_deref().unwrap_or_default(),
        "Rocket.Chat",
    ) || title_contains(&target.title, "rocket.chat")
        || title_contains(&target.title, "gs.chat")
        || title_contains(&target.title, "нет непрочитанных")
        || title_contains(&target.title, "unread messages")
}

fn target_is_telegram(target: &ForegroundTarget) -> bool {
    process_eq(
        target.process_name.as_deref().unwrap_or_default(),
        "Telegram",
    ) || class_eq(&target.app_class, "Qt51518QWindowIcon")
}

fn target_is_sticky_notes(target: &ForegroundTarget) -> bool {
    title_contains(&target.title, "Sticky Notes")
        || process_eq(
            target.process_name.as_deref().unwrap_or_default(),
            "Microsoft.Notes",
        )
}

fn target_is_notepad_like(target: &ForegroundTarget) -> bool {
    title_contains(&target.title, "Notepad")
        || process_eq(
            target.process_name.as_deref().unwrap_or_default(),
            "Notepad",
        )
        || class_eq(&target.app_class, "Notepad")
}

fn risky_fallbacks_enabled() -> bool {
    std::env::var_os("STEPLER_ALLOW_RISKY_FALLBACKS").is_some()
}
