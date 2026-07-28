use crate::{target_facts::target_facts, ForegroundTarget, ALL_METHOD_IDS};
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
    ExcelCellEditor,
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
                MethodId::UiAutomationEditableText,
                MethodId::WebKeyboardSelection,
            ]),
            scrolllock_methods: same_preferences(vec![
                MethodId::UiAutomationEditableText,
                MethodId::WebKeyboardSelection,
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
                MethodId::WebKeyboardSelection,
                MethodId::UiAutomationEditableText,
                MethodId::UiAutomationText,
            ]),
            scrolllock_methods: same_preferences(vec![
                MethodId::UiAutomationDocumentText,
                MethodId::WebKeyboardSelection,
                MethodId::UiAutomationEditableText,
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
        SurfacePolicy {
            surface: SurfaceKind::ExcelCellEditor,
            pause_methods: same_preferences(vec![MethodId::WebKeyboardSelection]),
            scrolllock_methods: same_preferences(vec![MethodId::WebKeyboardSelection]),
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
                MethodId::UiAutomationEditableText,
                MethodId::WebKeyboardSelection,
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
                MethodId::UiAutomationText,
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
        probe_policy(
            SurfaceKind::ExcelCellEditor,
            vec![MethodId::WebKeyboardSelection],
            false,
        ),
        probe_policy(SurfaceKind::Unknown, unknown_probe_methods(), false),
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
        .unwrap_or_else(|| probe_policy(SurfaceKind::Unknown, unknown_probe_methods(), false))
}

pub fn default_surface_policy() -> SurfacePolicy {
    let methods = conservative_unknown_methods();
    SurfacePolicy {
        surface: SurfaceKind::Unknown,
        pause_methods: same_preferences(methods.clone()),
        scrolllock_methods: same_preferences(methods.clone()),
        forbidden_methods: ALL_METHOD_IDS
            .iter()
            .copied()
            .filter(|method| !methods.contains(method))
            .collect(),
        allow_risky_methods: false,
    }
}

fn conservative_unknown_methods() -> Vec<MethodId> {
    vec![
        MethodId::UiAutomationEditableText,
        MethodId::UiAutomationDocumentText,
        MethodId::UiAutomationText,
    ]
}

fn unknown_probe_methods() -> Vec<MethodId> {
    if unknown_allows_diagnostic_probe() {
        ALL_METHOD_IDS.to_vec()
    } else {
        conservative_unknown_methods()
    }
}

fn unknown_allows_diagnostic_probe() -> bool {
    std::env::var_os("STEPLER_DIAGNOSTIC_UNKNOWN_PROBES").is_some()
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
    let facts = target_facts(target);

    if facts.is_classic_console {
        return surface(
            SurfaceKind::ClassicConsole,
            100,
            vec![
                "app_class=ConsoleWindowClass",
                "focused_class=ConsoleWindowClass",
            ],
        );
    }

    if facts.is_outlook_search_edit {
        return surface(
            SurfaceKind::OutlookSearch,
            100,
            vec!["process=OUTLOOK", "focused_class=Edit"],
        );
    }

    if facts.is_outlook_word_editor {
        return surface(
            SurfaceKind::OutlookWordEditor,
            100,
            vec!["process=OUTLOOK", "focused_class=_WwG"],
        );
    }

    if facts.is_outlook_process || facts.is_outlook_app_class {
        return surface(
            SurfaceKind::OutlookShell,
            90,
            vec!["process=OUTLOOK or app_class=rctrl_renwnd32"],
        );
    }

    if facts.is_word_process || facts.is_word_app_class {
        return surface(
            SurfaceKind::WordEditor,
            95,
            vec!["process=WINWORD or app_class=OpusApp"],
        );
    }

    if facts.is_excel_cell_editor {
        return surface(
            SurfaceKind::ExcelCellEditor,
            100,
            vec!["app_class=XLMAIN", "focused_class=EXCEL6"],
        );
    }

    if facts.is_win32_edit {
        return surface(SurfaceKind::Win32Edit, 95, vec!["focused_class=Edit"]);
    }

    if facts.is_notepad_like {
        return surface(
            SurfaceKind::NotepadLike,
            90,
            vec!["notepad-like app/process/title"],
        );
    }

    if facts.is_sticky_notes {
        return surface(
            SurfaceKind::StickyNotes,
            95,
            vec!["Sticky Notes title/process/app frame"],
        );
    }

    if facts.is_windows_terminal {
        if facts.is_windows_terminal_cmd_title {
            return surface(
                SurfaceKind::WindowsTerminalCmd,
                100,
                vec!["windows terminal", "title contains cmd.exe"],
            );
        }
        if facts.is_qwen_terminal_title_or_marker {
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

    if facts.is_telegram_process || facts.is_telegram_classifier_class {
        return surface(
            SurfaceKind::TelegramDesktop,
            95,
            vec!["process=Telegram or Qt window"],
        );
    }

    if facts.is_whatsapp_desktop {
        return surface(
            SurfaceKind::BrowserEditor,
            95,
            vec!["WhatsApp Desktop WinUI Chromium host"],
        );
    }

    if facts.is_browser_editor_class && facts.is_rocket_chat {
        return surface(
            SurfaceKind::RocketChatEditor,
            98,
            vec!["browser/electron editor class", "Rocket.Chat title/process"],
        );
    }

    if facts.is_browser_editor_class && facts.is_fast_browser_title {
        return surface(
            SurfaceKind::FastBrowserEditor,
            96,
            vec![
                "browser/electron editor class",
                "known fast browser editor title/process",
            ],
        );
    }

    if facts.is_yandex_browser_widget_class {
        return surface(
            SurfaceKind::YandexBrowserEditor,
            95,
            vec!["app_class=Chrome_Yandex_WidgetWin*"],
        );
    }

    if facts.is_browser_editor_class {
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
