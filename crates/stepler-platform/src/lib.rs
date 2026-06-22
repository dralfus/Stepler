use stepler_core::{CorrectionMode, MethodId, ReplacementPlan, TextContext};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlatformError {
    Unsupported,
    UnsupportedControl {
        app_class: String,
        focused_class: String,
    },
    ForegroundUnavailable,
    ClipboardUnavailable,
    HotkeyUnavailable,
    ReplacementUnavailable,
    ReplacementUnavailableReason(String),
    PreflightFailed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForegroundControl {
    pub app_id: String,
    pub window_id: String,
    pub control_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForegroundTarget {
    pub app_class: String,
    pub focused_class: String,
    pub title: String,
    pub process_name: Option<String>,
    pub window_id: String,
    pub control_id: String,
}

impl ForegroundTarget {
    pub fn app_key(&self) -> &str {
        self.process_name
            .as_deref()
            .unwrap_or(self.app_class.as_str())
    }
}

impl ForegroundControl {
    pub fn key(&self) -> String {
        format!("{}/{}/{}", self.app_id, self.window_id, self.control_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardSnapshot {
    pub text: Option<String>,
    pub sequence_number: Option<u32>,
    pub formats: Vec<ClipboardFormatSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardFormatSnapshot {
    pub format: u32,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyReplacementResult {
    pub applied: bool,
    pub actual_before_text: Option<String>,
    pub actual_after_text: Option<String>,
    pub method: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeSafety {
    Safe,
    Risky,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodProbe {
    pub method_id: MethodId,
    pub safety: ProbeSafety,
    pub confidence: u8,
    pub requires_clipboard: bool,
    pub requires_focus_stability: bool,
    pub can_preflight: bool,
    pub can_verify: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdapterContract {
    pub method: MethodId,
    pub can_read_selection: bool,
    pub can_read_caret: bool,
    pub can_replace_selection: bool,
    pub can_replace_range_before_caret: bool,
    pub uses_clipboard: bool,
    pub risky: bool,
}

impl AdapterContract {
    pub const fn new(method: MethodId) -> Self {
        Self {
            method,
            can_read_selection: false,
            can_read_caret: false,
            can_replace_selection: false,
            can_replace_range_before_caret: false,
            uses_clipboard: false,
            risky: false,
        }
    }

    pub const fn read_selection(mut self) -> Self {
        self.can_read_selection = true;
        self
    }

    pub const fn read_caret(mut self) -> Self {
        self.can_read_caret = true;
        self
    }

    pub const fn replace_selection(mut self) -> Self {
        self.can_replace_selection = true;
        self
    }

    pub const fn replace_range_before_caret(mut self) -> Self {
        self.can_replace_range_before_caret = true;
        self
    }

    pub const fn clipboard(mut self) -> Self {
        self.uses_clipboard = true;
        self
    }

    pub const fn risky(mut self) -> Self {
        self.risky = true;
        self
    }
}

pub const ALL_METHOD_IDS: &[MethodId] = &[
    MethodId::Win32EditMessages,
    MethodId::TerminalClipboardShortcut,
    MethodId::SshTerminal,
    MethodId::ConsoleBuffer,
    MethodId::PsReadLine,
    MethodId::WordCom,
    MethodId::UiAutomationEditableText,
    MethodId::UiAutomationDocumentText,
    MethodId::UiAutomationText,
    MethodId::XtermKeyboardSelection,
    MethodId::WebKeyboardSelection,
    MethodId::ClipboardSelection,
    MethodId::SendInput,
];

pub fn adapter_contract(method: MethodId) -> AdapterContract {
    match method {
        MethodId::Win32EditMessages => AdapterContract::new(method)
            .read_selection()
            .read_caret()
            .replace_selection()
            .replace_range_before_caret(),
        MethodId::TerminalClipboardShortcut => AdapterContract::new(method)
            .read_selection()
            .read_caret()
            .replace_selection()
            .replace_range_before_caret()
            .clipboard()
            .risky(),
        MethodId::SshTerminal => AdapterContract::new(method)
            .read_selection()
            .read_caret()
            .replace_selection()
            .replace_range_before_caret(),
        MethodId::ConsoleBuffer => AdapterContract::new(method)
            .read_selection()
            .read_caret()
            .replace_selection()
            .replace_range_before_caret(),
        MethodId::PsReadLine => AdapterContract::new(method)
            .read_selection()
            .read_caret()
            .replace_selection()
            .replace_range_before_caret(),
        MethodId::WordCom => AdapterContract::new(method)
            .read_selection()
            .read_caret()
            .replace_selection()
            .replace_range_before_caret(),
        MethodId::UiAutomationEditableText => AdapterContract::new(method)
            .read_selection()
            .read_caret()
            .replace_selection()
            .replace_range_before_caret(),
        MethodId::UiAutomationDocumentText => AdapterContract::new(method)
            .read_selection()
            .read_caret()
            .replace_selection()
            .replace_range_before_caret(),
        MethodId::UiAutomationText => AdapterContract::new(method)
            .read_selection()
            .read_caret()
            .replace_selection()
            .replace_range_before_caret(),
        MethodId::XtermKeyboardSelection => AdapterContract::new(method)
            .read_selection()
            .read_caret()
            .replace_selection()
            .replace_range_before_caret()
            .clipboard(),
        MethodId::WebKeyboardSelection => AdapterContract::new(method)
            .read_selection()
            .read_caret()
            .replace_selection()
            .replace_range_before_caret()
            .clipboard(),
        MethodId::ClipboardSelection => AdapterContract::new(method)
            .read_selection()
            .replace_selection()
            .clipboard()
            .risky(),
        MethodId::SendInput => AdapterContract::new(method).replace_selection().risky(),
    }
}

impl MethodProbe {
    pub fn safe(method_id: MethodId, reason: impl Into<String>) -> Self {
        Self {
            method_id,
            safety: ProbeSafety::Safe,
            confidence: 100,
            requires_clipboard: false,
            requires_focus_stability: true,
            can_preflight: true,
            can_verify: true,
            reason: reason.into(),
        }
    }

    pub fn risky(method_id: MethodId, reason: impl Into<String>) -> Self {
        Self {
            method_id,
            safety: ProbeSafety::Risky,
            confidence: 50,
            requires_clipboard: true,
            requires_focus_stability: true,
            can_preflight: true,
            can_verify: false,
            reason: reason.into(),
        }
    }

    pub fn unsupported(method_id: MethodId, reason: impl Into<String>) -> Self {
        Self {
            method_id,
            safety: ProbeSafety::Unsupported,
            confidence: 0,
            requires_clipboard: false,
            requires_focus_stability: false,
            can_preflight: false,
            can_verify: false,
            reason: reason.into(),
        }
    }
}

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
pub struct MethodPreferences {
    pub context_methods: Vec<MethodId>,
    pub replace_methods: Vec<MethodId>,
}

impl SurfacePolicy {
    pub fn matches(&self, classification: &SurfaceClassification) -> bool {
        self.surface == classification.kind
    }

    fn preferences_for(&self, mode: CorrectionMode) -> &MethodPreferences {
        match mode {
            CorrectionMode::Pause => &self.pause_methods,
            CorrectionMode::ScrollLock => &self.scrolllock_methods,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolveDecision {
    pub context_method: MethodId,
    pub replacement_method: MethodId,
    pub safety: ProbeSafety,
    pub reason: String,
    pub surface: SurfaceClassification,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveError {
    NoSupportedMethod,
    ForbiddenByPolicy(MethodId),
    RiskyMethodBlocked(MethodId),
}

#[derive(Debug, Clone)]
pub struct MethodResolver {
    policies: Vec<SurfacePolicy>,
}

impl MethodResolver {
    pub fn new(policies: Vec<SurfacePolicy>) -> Self {
        Self { policies }
    }

    pub fn resolve(
        &self,
        target: &ForegroundTarget,
        probes: &[MethodProbe],
    ) -> Result<ResolveDecision, ResolveError> {
        self.resolve_for_mode(target, probes, CorrectionMode::Pause)
    }

    pub fn resolve_for_mode(
        &self,
        target: &ForegroundTarget,
        probes: &[MethodProbe],
        mode: CorrectionMode,
    ) -> Result<ResolveDecision, ResolveError> {
        let classification = classify_surface(target);
        let policy = self.policy_for(&classification);
        let preferences = policy.preferences_for(mode);
        let mut candidates = probes
            .iter()
            .filter(|probe| probe.safety != ProbeSafety::Unsupported)
            .collect::<Vec<_>>();
        candidates.sort_by_key(|probe| {
            (
                method_preference_rank(probe.method_id, &preferences.context_methods),
                std::cmp::Reverse(probe.confidence),
            )
        });

        for probe in candidates {
            if policy.forbidden_methods.contains(&probe.method_id) {
                continue;
            }
            if probe.safety == ProbeSafety::Risky
                && (!policy.allow_risky_methods
                    || !surface_allows_risky_method(classification.kind, probe.method_id))
            {
                continue;
            }
            let replacement_method = policy
                .preferences_for(mode)
                .replace_methods
                .iter()
                .copied()
                .find(|method| *method == probe.method_id)
                .unwrap_or(probe.method_id);
            if policy.forbidden_methods.contains(&replacement_method) {
                continue;
            }

            return Ok(ResolveDecision {
                context_method: probe.method_id,
                replacement_method,
                safety: probe.safety,
                reason: format!(
                    "{} via surface {:?} confidence={} evidence={}",
                    probe.reason,
                    classification.kind,
                    classification.confidence,
                    classification.evidence.join("; ")
                ),
                surface: classification,
            });
        }

        if probes.iter().any(|probe| {
            probe.safety != ProbeSafety::Unsupported
                && policy.forbidden_methods.contains(&probe.method_id)
        }) {
            return Err(ResolveError::ForbiddenByPolicy(
                probes
                    .iter()
                    .find(|probe| policy.forbidden_methods.contains(&probe.method_id))
                    .map(|probe| probe.method_id)
                    .unwrap(),
            ));
        }
        if probes.iter().any(|probe| {
            probe.safety == ProbeSafety::Risky
                && (!policy.allow_risky_methods
                    || !surface_allows_risky_method(classification.kind, probe.method_id))
                && !policy.forbidden_methods.contains(&probe.method_id)
        }) {
            return Err(ResolveError::RiskyMethodBlocked(
                probes
                    .iter()
                    .find(|probe| probe.safety == ProbeSafety::Risky)
                    .map(|probe| probe.method_id)
                    .unwrap(),
            ));
        }

        Err(ResolveError::NoSupportedMethod)
    }

    fn policy_for(&self, classification: &SurfaceClassification) -> SurfacePolicy {
        self.policies
            .iter()
            .find(|policy| policy.matches(classification))
            .cloned()
            .unwrap_or_else(default_surface_policy)
    }
}

impl Default for MethodResolver {
    fn default() -> Self {
        Self::new(default_surface_policies())
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

fn method_preference_rank(method: MethodId, preferred_methods: &[MethodId]) -> usize {
    preferred_methods
        .iter()
        .position(|preferred| *preferred == method)
        .unwrap_or(usize::MAX)
}

pub trait ForegroundProvider {
    fn foreground_control(&self) -> Result<ForegroundControl, PlatformError>;
}

pub trait TextContextProvider {
    fn text_context(&self) -> Result<TextContext, PlatformError>;
}

pub trait TextReplacer {
    fn apply_replacement(
        &self,
        context: &TextContext,
        plan: &ReplacementPlan,
    ) -> Result<ApplyReplacementResult, PlatformError>;
}

pub trait ClipboardBackend {
    fn capture(&self) -> Result<ClipboardSnapshot, PlatformError>;
    fn restore(&self, snapshot: ClipboardSnapshot) -> Result<(), PlatformError>;
}

pub trait HotkeyListener {
    fn start(&mut self) -> Result<(), PlatformError>;
    fn stop(&mut self) -> Result<(), PlatformError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HotkeyEvent {
    pub mode: CorrectionMode,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolver_prefers_win32_edit_for_edit_controls() {
        let resolver = MethodResolver::default();
        let target = target("Notepad", "Edit");
        let probes = vec![
            MethodProbe::risky(MethodId::ClipboardSelection, "clipboard fallback"),
            MethodProbe::safe(MethodId::Win32EditMessages, "edit control"),
        ];

        let decision = resolver.resolve(&target, &probes).unwrap();

        assert_eq!(decision.context_method, MethodId::Win32EditMessages);
        assert_eq!(decision.replacement_method, MethodId::Win32EditMessages);
        assert_eq!(decision.safety, ProbeSafety::Safe);
    }

    #[test]
    fn resolver_blocks_risky_terminal_clipboard_for_windows_terminal() {
        let resolver = MethodResolver::default();
        let target = target(
            "CASCADIA_HOSTING_WINDOW_CLASS",
            "Windows.UI.Input.InputSite.WindowClass",
        );
        let probes = vec![MethodProbe::risky(
            MethodId::TerminalClipboardShortcut,
            "terminal shortcut",
        )];

        let error = resolver.resolve(&target, &probes).unwrap_err();

        assert_eq!(
            error,
            ResolveError::ForbiddenByPolicy(MethodId::TerminalClipboardShortcut)
        );
    }

    #[test]
    fn resolver_allows_terminal_clipboard_for_cmd_inside_windows_terminal() {
        let resolver = MethodResolver::default();
        let mut target = target(
            "CASCADIA_HOSTING_WINDOW_CLASS",
            "Windows.UI.Input.InputSite.WindowClass",
        );
        target.title = String::from("C:\\WINDOWS\\system32\\cmd.exe");
        let probes = vec![MethodProbe::risky(
            MethodId::TerminalClipboardShortcut,
            "terminal shortcut",
        )];

        let decision = resolver.resolve(&target, &probes).unwrap();

        assert_eq!(decision.context_method, MethodId::TerminalClipboardShortcut);
        assert_eq!(
            decision.replacement_method,
            MethodId::TerminalClipboardShortcut
        );
    }

    #[test]
    fn resolver_forbids_generic_clipboard_for_browser_policy() {
        let resolver = MethodResolver::default();
        let target = target("Chrome_WidgetWin_1", "Chrome_RenderWidgetHostHWND");
        let probes = vec![MethodProbe::risky(
            MethodId::ClipboardSelection,
            "clipboard fallback",
        )];

        let error = resolver.resolve(&target, &probes).unwrap_err();

        assert_eq!(
            error,
            ResolveError::ForbiddenByPolicy(MethodId::ClipboardSelection)
        );
    }

    #[test]
    fn resolver_prefers_word_com_for_word_policy() {
        let resolver = MethodResolver::default();
        let mut target = target("OpusApp", "_WwG");
        target.process_name = Some(String::from("WINWORD"));
        let probes = vec![
            MethodProbe::safe(MethodId::UiAutomationText, "uia fallback"),
            MethodProbe::safe(MethodId::WordCom, "word object model"),
        ];

        let decision = resolver.resolve(&target, &probes).unwrap();

        assert_eq!(decision.context_method, MethodId::WordCom);
        assert_eq!(decision.replacement_method, MethodId::WordCom);
    }

    #[test]
    fn resolver_blocks_generic_clipboard_for_word_policy() {
        let resolver = MethodResolver::default();
        let target = target("OpusApp", "_WwG");
        let probes = vec![MethodProbe::risky(
            MethodId::ClipboardSelection,
            "clipboard fallback",
        )];

        let error = resolver.resolve(&target, &probes).unwrap_err();

        assert_eq!(
            error,
            ResolveError::ForbiddenByPolicy(MethodId::ClipboardSelection)
        );
    }

    #[test]
    fn resolver_uses_xterm_keyboard_for_qwen_inside_windows_terminal() {
        let resolver = MethodResolver::default();
        let mut target = target(
            "CASCADIA_HOSTING_WINDOW_CLASS",
            "Windows.UI.Input.InputSite.WindowClass",
        );
        target.title = String::from("Qwen CLI");
        let probes = vec![
            MethodProbe::risky(MethodId::TerminalClipboardShortcut, "terminal shortcut"),
            MethodProbe::safe(MethodId::XtermKeyboardSelection, "xterm keyboard"),
        ];

        let decision = resolver.resolve(&target, &probes).unwrap();

        assert_eq!(decision.context_method, MethodId::XtermKeyboardSelection);
        assert_eq!(
            decision.replacement_method,
            MethodId::XtermKeyboardSelection
        );
    }

    #[test]
    fn resolver_blocks_terminal_clipboard_for_qwen_inside_windows_terminal() {
        let resolver = MethodResolver::default();
        let mut target = target(
            "CASCADIA_HOSTING_WINDOW_CLASS",
            "Windows.UI.Input.InputSite.WindowClass",
        );
        target.title = String::from("Qwen CLI");
        let probes = vec![MethodProbe::risky(
            MethodId::TerminalClipboardShortcut,
            "terminal shortcut",
        )];

        let error = resolver.resolve(&target, &probes).unwrap_err();

        assert_eq!(
            error,
            ResolveError::ForbiddenByPolicy(MethodId::TerminalClipboardShortcut)
        );
    }

    #[test]
    fn resolver_allows_xterm_keyboard_selection_inside_windows_terminal() {
        let resolver = MethodResolver::default();
        let mut target = target(
            "CASCADIA_HOSTING_WINDOW_CLASS",
            "Windows.UI.Input.InputSite.WindowClass",
        );
        target.title = String::from("Windows PowerShell");
        let probes = vec![MethodProbe::safe(
            MethodId::XtermKeyboardSelection,
            "xterm textarea keyboard selection with terminal copy/paste shortcuts",
        )];

        let decision = resolver.resolve(&target, &probes).unwrap();

        assert_eq!(decision.context_method, MethodId::XtermKeyboardSelection);
        assert_eq!(
            decision.replacement_method,
            MethodId::XtermKeyboardSelection
        );
    }

    #[test]
    fn resolver_prefers_word_com_for_outlook_policy() {
        let resolver = MethodResolver::default();
        let mut target = target("rctrl_renwnd32", "_WwG");
        target.process_name = Some(String::from("OUTLOOK"));
        let probes = vec![
            MethodProbe::safe(MethodId::UiAutomationEditableText, "uia fallback"),
            MethodProbe::safe(MethodId::WordCom, "outlook word editor"),
        ];

        let decision = resolver.resolve(&target, &probes).unwrap();

        assert_eq!(decision.context_method, MethodId::WordCom);
        assert_eq!(decision.replacement_method, MethodId::WordCom);
    }

    #[test]
    fn resolver_allows_win32_edit_for_outlook_search_policy() {
        let resolver = MethodResolver::default();
        let mut target = target("rctrl_renwnd32", "Edit");
        target.process_name = Some(String::from("OUTLOOK"));
        let probes = vec![MethodProbe::safe(
            MethodId::Win32EditMessages,
            "outlook search edit",
        )];

        let decision = resolver.resolve(&target, &probes).unwrap();

        assert_eq!(decision.context_method, MethodId::Win32EditMessages);
        assert_eq!(decision.replacement_method, MethodId::Win32EditMessages);
    }

    #[test]
    fn resolver_forbids_uia_for_outlook_policy() {
        let resolver = MethodResolver::default();
        let mut target = target("rctrl_renwnd32", "SUPERGRID");
        target.process_name = Some(String::from("OUTLOOK"));
        let probes = vec![MethodProbe::safe(
            MethodId::UiAutomationEditableText,
            "uia fallback",
        )];

        let error = resolver.resolve(&target, &probes).unwrap_err();

        assert_eq!(
            error,
            ResolveError::ForbiddenByPolicy(MethodId::UiAutomationEditableText)
        );
    }

    #[test]
    fn resolver_prefers_web_keyboard_for_browser_like_classes() {
        let resolver = MethodResolver::default();
        let target = target("Chrome_WidgetWin_1", "Chrome_WidgetWin_1");
        let probes = vec![
            MethodProbe::safe(MethodId::XtermKeyboardSelection, "xterm keyboard"),
            MethodProbe::safe(MethodId::WebKeyboardSelection, "web keyboard"),
            MethodProbe::safe(MethodId::UiAutomationDocumentText, "document selection"),
            MethodProbe::safe(MethodId::UiAutomationText, "uia text"),
            MethodProbe::safe(MethodId::UiAutomationEditableText, "editable text"),
        ];

        let decision = resolver.resolve(&target, &probes).unwrap();

        assert_eq!(decision.context_method, MethodId::WebKeyboardSelection);
        assert_eq!(decision.replacement_method, MethodId::WebKeyboardSelection);
    }

    #[test]
    fn resolver_allows_uia_document_for_yandex_browser_policy() {
        let resolver = MethodResolver::default();
        let target = target("Chrome_Yandex_WidgetWin_1", "Chrome_Yandex_WidgetWin_1");
        let probes = vec![MethodProbe::safe(
            MethodId::UiAutomationDocumentText,
            "document text",
        )];

        let decision = resolver.resolve(&target, &probes).unwrap();

        assert_eq!(decision.context_method, MethodId::UiAutomationDocumentText);
        assert_eq!(
            decision.replacement_method,
            MethodId::UiAutomationDocumentText
        );
    }

    #[test]
    fn resolver_forbids_uia_document_for_browser_policy() {
        let resolver = MethodResolver::default();
        let target = target("Chrome_WidgetWin_1", "Chrome_WidgetWin_1");
        let probes = vec![MethodProbe::safe(
            MethodId::UiAutomationDocumentText,
            "document selection",
        )];

        let error = resolver.resolve(&target, &probes).unwrap_err();

        assert_eq!(
            error,
            ResolveError::ForbiddenByPolicy(MethodId::UiAutomationDocumentText)
        );
    }

    #[test]
    fn resolver_prefers_keyboard_selection_for_telegram_policy() {
        let resolver = MethodResolver::default();
        let mut target = target("Qt51518QWindowIcon", "Qt51518QWindowIcon");
        target.process_name = Some(String::from("Telegram"));
        let probes = vec![
            MethodProbe::safe(MethodId::UiAutomationEditableText, "editable text"),
            MethodProbe::safe(MethodId::WebKeyboardSelection, "keyboard selection"),
        ];

        let decision = resolver.resolve(&target, &probes).unwrap();

        assert_eq!(decision.context_method, MethodId::WebKeyboardSelection);
        assert_eq!(decision.replacement_method, MethodId::WebKeyboardSelection);
    }

    #[test]
    fn resolver_prefers_uia_document_text_for_sticky_notes_policy() {
        let resolver = MethodResolver::default();
        let mut target = target("ApplicationFrameWindow", "Windows.UI.Core.CoreWindow");
        target.title = String::from("Sticky Notes");
        target.process_name = Some(String::from("Microsoft.Notes"));
        let probes = vec![
            MethodProbe::safe(MethodId::UiAutomationEditableText, "editable text"),
            MethodProbe::safe(MethodId::UiAutomationDocumentText, "document text"),
            MethodProbe::safe(MethodId::WebKeyboardSelection, "keyboard selection"),
        ];

        let decision = resolver.resolve(&target, &probes).unwrap();

        assert_eq!(decision.context_method, MethodId::UiAutomationDocumentText);
        assert_eq!(
            decision.replacement_method,
            MethodId::UiAutomationDocumentText
        );
    }

    #[test]
    fn resolver_contracts_for_verified_applications() {
        struct Contract {
            name: &'static str,
            target: ForegroundTarget,
            probes: Vec<MethodProbe>,
            expected_surface: SurfaceKind,
            expected: MethodId,
        }

        let resolver = MethodResolver::default();
        let mut contracts = Vec::new();

        contracts.push(Contract {
            name: "notepad edit",
            target: target("Notepad", "Edit"),
            probes: vec![
                MethodProbe::safe(MethodId::Win32EditMessages, "win32 edit"),
                MethodProbe::safe(MethodId::UiAutomationDocumentText, "uia document"),
                MethodProbe::safe(MethodId::WebKeyboardSelection, "keyboard selection"),
            ],
            expected_surface: SurfaceKind::Win32Edit,
            expected: MethodId::Win32EditMessages,
        });

        contracts.push(Contract {
            name: "classic console",
            target: target("ConsoleWindowClass", "ConsoleWindowClass"),
            probes: vec![
                MethodProbe::safe(MethodId::ConsoleBuffer, "classic console"),
                MethodProbe::risky(MethodId::TerminalClipboardShortcut, "terminal shortcut"),
            ],
            expected_surface: SurfaceKind::ClassicConsole,
            expected: MethodId::ConsoleBuffer,
        });

        let mut windows_terminal_ps = target(
            "CASCADIA_HOSTING_WINDOW_CLASS",
            "Windows.UI.Input.InputSite.WindowClass",
        );
        windows_terminal_ps.title = String::from("Windows PowerShell");
        contracts.push(Contract {
            name: "windows terminal powershell",
            target: windows_terminal_ps,
            probes: vec![
                MethodProbe::safe(MethodId::PsReadLine, "psreadline"),
                MethodProbe::safe(MethodId::XtermKeyboardSelection, "xterm keyboard"),
                MethodProbe::risky(MethodId::TerminalClipboardShortcut, "terminal shortcut"),
            ],
            expected_surface: SurfaceKind::WindowsTerminalPowerShell,
            expected: MethodId::PsReadLine,
        });

        let mut windows_terminal_cmd = target(
            "CASCADIA_HOSTING_WINDOW_CLASS",
            "Windows.UI.Input.InputSite.WindowClass",
        );
        windows_terminal_cmd.title = String::from("C:\\WINDOWS\\system32\\cmd.exe");
        contracts.push(Contract {
            name: "windows terminal command prompt",
            target: windows_terminal_cmd,
            probes: vec![MethodProbe::risky(
                MethodId::TerminalClipboardShortcut,
                "terminal shortcut",
            )],
            expected_surface: SurfaceKind::WindowsTerminalCmd,
            expected: MethodId::TerminalClipboardShortcut,
        });

        let mut qwen_terminal = target(
            "CASCADIA_HOSTING_WINDOW_CLASS",
            "Windows.UI.Input.InputSite.WindowClass",
        );
        qwen_terminal.title = String::from("stepler-terminal-app qwen");
        contracts.push(Contract {
            name: "qwen terminal app",
            target: qwen_terminal,
            probes: vec![
                MethodProbe::safe(MethodId::PsReadLine, "psreadline"),
                MethodProbe::safe(MethodId::XtermKeyboardSelection, "xterm keyboard"),
                MethodProbe::risky(MethodId::TerminalClipboardShortcut, "terminal shortcut"),
            ],
            expected_surface: SurfaceKind::QwenTerminal,
            expected: MethodId::XtermKeyboardSelection,
        });

        let mut chrome_codex = target("Chrome_WidgetWin_1", "Chrome_WidgetWin_1");
        chrome_codex.title = String::from("Codex");
        chrome_codex.process_name = Some(String::from("Codex"));
        contracts.push(Contract {
            name: "codex windows app",
            target: chrome_codex,
            probes: browser_probes(),
            expected_surface: SurfaceKind::FastBrowserEditor,
            expected: MethodId::WebKeyboardSelection,
        });

        let mut chrome_jira = target("Chrome_WidgetWin_1", "Chrome_WidgetWin_1");
        chrome_jira.title = String::from("[CTP-11796] GS-Labs JIRA - Google Chrome");
        chrome_jira.process_name = Some(String::from("chrome"));
        contracts.push(Contract {
            name: "jira web chrome",
            target: chrome_jira,
            probes: browser_probes(),
            expected_surface: SurfaceKind::FastBrowserEditor,
            expected: MethodId::WebKeyboardSelection,
        });

        let mut firefox_confluence = target("MozillaWindowClass", "MozillaWindowClass");
        firefox_confluence.title = String::from("CVE - Chips - GS-Labs Wiki - Mozilla Firefox");
        firefox_confluence.process_name = Some(String::from("firefox"));
        contracts.push(Contract {
            name: "confluence web firefox",
            target: firefox_confluence,
            probes: browser_probes(),
            expected_surface: SurfaceKind::FastBrowserEditor,
            expected: MethodId::WebKeyboardSelection,
        });

        let mut rocket_chat = target("Chrome_WidgetWin_1", "Chrome_WidgetWin_1");
        rocket_chat.title = String::from("Нет непрочитанных сообщений");
        rocket_chat.process_name = Some(String::from("Rocket.Chat"));
        contracts.push(Contract {
            name: "rocket chat search",
            target: rocket_chat,
            probes: browser_probes(),
            expected_surface: SurfaceKind::RocketChatEditor,
            expected: MethodId::WebKeyboardSelection,
        });

        let mut telegram = target("Qt51518QWindowIcon", "Qt51518QWindowIcon");
        telegram.process_name = Some(String::from("Telegram"));
        contracts.push(Contract {
            name: "telegram desktop",
            target: telegram,
            probes: vec![
                MethodProbe::safe(MethodId::UiAutomationEditableText, "editable text"),
                MethodProbe::safe(MethodId::WebKeyboardSelection, "keyboard selection"),
            ],
            expected_surface: SurfaceKind::TelegramDesktop,
            expected: MethodId::WebKeyboardSelection,
        });

        let mut word = target("OpusApp", "_WwG");
        word.process_name = Some(String::from("WINWORD"));
        contracts.push(Contract {
            name: "word editor",
            target: word,
            probes: vec![
                MethodProbe::safe(MethodId::WordCom, "word object model"),
                MethodProbe::safe(MethodId::UiAutomationDocumentText, "uia document"),
            ],
            expected_surface: SurfaceKind::WordEditor,
            expected: MethodId::WordCom,
        });

        let mut outlook_editor = target("rctrl_renwnd32", "_WwG");
        outlook_editor.process_name = Some(String::from("OUTLOOK"));
        contracts.push(Contract {
            name: "outlook word editor",
            target: outlook_editor,
            probes: vec![
                MethodProbe::safe(MethodId::WordCom, "outlook word editor"),
                MethodProbe::safe(MethodId::UiAutomationEditableText, "uia editable"),
            ],
            expected_surface: SurfaceKind::OutlookWordEditor,
            expected: MethodId::WordCom,
        });

        let mut outlook_search = target("rctrl_renwnd32", "Edit");
        outlook_search.process_name = Some(String::from("OUTLOOK"));
        contracts.push(Contract {
            name: "outlook search",
            target: outlook_search,
            probes: vec![
                MethodProbe::safe(MethodId::Win32EditMessages, "win32 edit"),
                MethodProbe::safe(MethodId::UiAutomationEditableText, "uia editable"),
            ],
            expected_surface: SurfaceKind::OutlookSearch,
            expected: MethodId::Win32EditMessages,
        });

        let mut sticky = target(
            "ApplicationFrameWindow",
            "Windows.UI.Input.InputSite.WindowClass",
        );
        sticky.title = String::from("Sticky Notes");
        sticky.process_name = Some(String::from("Microsoft.Notes"));
        contracts.push(Contract {
            name: "sticky notes",
            target: sticky,
            probes: vec![
                MethodProbe::safe(MethodId::UiAutomationDocumentText, "document text"),
                MethodProbe::safe(MethodId::WebKeyboardSelection, "keyboard selection"),
                MethodProbe::risky(MethodId::TerminalClipboardShortcut, "terminal shortcut"),
            ],
            expected_surface: SurfaceKind::StickyNotes,
            expected: MethodId::UiAutomationDocumentText,
        });

        for contract in contracts {
            let decision = resolver
                .resolve(&contract.target, &contract.probes)
                .unwrap_or_else(|error| panic!("{} failed to resolve: {:?}", contract.name, error));
            assert_eq!(
                decision.surface.kind, contract.expected_surface,
                "{} surface kind",
                contract.name
            );
            assert_eq!(
                decision.context_method, contract.expected,
                "{} context method",
                contract.name
            );
            assert_eq!(
                decision.replacement_method, contract.expected,
                "{} replacement method",
                contract.name
            );
        }
    }

    #[test]
    fn resolver_contracts_keep_fallbacks_inside_surface_boundaries() {
        let resolver = MethodResolver::default();

        let mut browser = target("Chrome_WidgetWin_1", "Chrome_WidgetWin_1");
        browser.title = String::from("Codex");
        let decision = resolver
            .resolve(
                &browser,
                &[MethodProbe::safe(
                    MethodId::UiAutomationEditableText,
                    "editable fallback",
                )],
            )
            .unwrap();
        assert_eq!(decision.surface.kind, SurfaceKind::FastBrowserEditor);
        assert_eq!(decision.context_method, MethodId::UiAutomationEditableText);

        let mut sticky = target(
            "ApplicationFrameWindow",
            "Windows.UI.Input.InputSite.WindowClass",
        );
        sticky.title = String::from("Sticky Notes");
        sticky.process_name = Some(String::from("Microsoft.Notes"));
        let decision = resolver
            .resolve(
                &sticky,
                &[MethodProbe::safe(
                    MethodId::WebKeyboardSelection,
                    "keyboard fallback",
                )],
            )
            .unwrap();
        assert_eq!(decision.surface.kind, SurfaceKind::StickyNotes);
        assert_eq!(decision.context_method, MethodId::WebKeyboardSelection);

        let mut outlook_shell = target("rctrl_renwnd32", "SUPERGRID");
        outlook_shell.process_name = Some(String::from("OUTLOOK"));
        let error = resolver
            .resolve(
                &outlook_shell,
                &[MethodProbe::safe(
                    MethodId::UiAutomationEditableText,
                    "wrong outlook fallback",
                )],
            )
            .unwrap_err();
        assert_eq!(
            error,
            ResolveError::ForbiddenByPolicy(MethodId::UiAutomationEditableText)
        );

        let terminal = target(
            "CASCADIA_HOSTING_WINDOW_CLASS",
            "Windows.UI.Input.InputSite.WindowClass",
        );
        let error = resolver
            .resolve(
                &terminal,
                &[MethodProbe::risky(
                    MethodId::TerminalClipboardShortcut,
                    "terminal shortcut",
                )],
            )
            .unwrap_err();
        assert_eq!(
            error,
            ResolveError::ForbiddenByPolicy(MethodId::TerminalClipboardShortcut)
        );
    }

    #[test]
    fn web_keyboard_profiles_are_derived_from_surface_kind() {
        assert_eq!(
            web_keyboard_profile_for_surface(SurfaceKind::BrowserEditor),
            WebKeyboardProfile::Standard
        );
        assert_eq!(
            web_keyboard_profile_for_surface(SurfaceKind::FastBrowserEditor),
            WebKeyboardProfile::Fast
        );
        assert_eq!(
            web_keyboard_profile_for_surface(SurfaceKind::RocketChatEditor),
            WebKeyboardProfile::RocketSearch
        );
    }

    #[test]
    fn adapter_contracts_cover_all_methods() {
        for method in ALL_METHOD_IDS {
            assert_eq!(adapter_contract(*method).method, *method);
        }
    }

    #[test]
    fn adapter_contracts_mark_risky_and_clipboard_boundaries() {
        assert!(adapter_contract(MethodId::TerminalClipboardShortcut).risky);
        assert!(adapter_contract(MethodId::TerminalClipboardShortcut).uses_clipboard);
        assert!(adapter_contract(MethodId::ClipboardSelection).risky);
        assert!(adapter_contract(MethodId::ClipboardSelection).uses_clipboard);
        assert!(adapter_contract(MethodId::SendInput).risky);
        assert!(!adapter_contract(MethodId::SendInput).uses_clipboard);

        assert!(adapter_contract(MethodId::WebKeyboardSelection).uses_clipboard);
        assert!(!adapter_contract(MethodId::WebKeyboardSelection).risky);
        assert!(adapter_contract(MethodId::XtermKeyboardSelection).uses_clipboard);
        assert!(!adapter_contract(MethodId::XtermKeyboardSelection).risky);
        assert!(!adapter_contract(MethodId::Win32EditMessages).uses_clipboard);
    }

    #[test]
    fn adapter_contracts_capture_selection_and_caret_capabilities() {
        let win32 = adapter_contract(MethodId::Win32EditMessages);
        assert!(win32.can_read_selection);
        assert!(win32.can_read_caret);
        assert!(win32.can_replace_range_before_caret);

        let clipboard = adapter_contract(MethodId::ClipboardSelection);
        assert!(clipboard.can_read_selection);
        assert!(!clipboard.can_read_caret);
        assert!(clipboard.can_replace_selection);
        assert!(!clipboard.can_replace_range_before_caret);

        let send_input = adapter_contract(MethodId::SendInput);
        assert!(!send_input.can_read_selection);
        assert!(!send_input.can_read_caret);
        assert!(send_input.can_replace_selection);
        assert!(!send_input.can_replace_range_before_caret);
    }

    #[test]
    fn resolver_can_use_different_method_preferences_for_pause_and_scrolllock() {
        let resolver = MethodResolver::new(vec![SurfacePolicy {
            surface: SurfaceKind::Win32Edit,
            pause_methods: same_preferences(vec![MethodId::Win32EditMessages]),
            scrolllock_methods: same_preferences(vec![MethodId::WebKeyboardSelection]),
            forbidden_methods: vec![],
            allow_risky_methods: false,
        }]);
        let target = target("Notepad", "Edit");
        let probes = vec![
            MethodProbe::safe(MethodId::Win32EditMessages, "win32 edit"),
            MethodProbe::safe(MethodId::WebKeyboardSelection, "keyboard selection"),
        ];

        let pause = resolver
            .resolve_for_mode(&target, &probes, CorrectionMode::Pause)
            .unwrap();
        let scrolllock = resolver
            .resolve_for_mode(&target, &probes, CorrectionMode::ScrollLock)
            .unwrap();

        assert_eq!(pause.context_method, MethodId::Win32EditMessages);
        assert_eq!(scrolllock.context_method, MethodId::WebKeyboardSelection);
    }

    fn target(app_class: &str, focused_class: &str) -> ForegroundTarget {
        ForegroundTarget {
            app_class: app_class.to_owned(),
            focused_class: focused_class.to_owned(),
            title: String::new(),
            process_name: None,
            window_id: String::from("hwnd:1"),
            control_id: String::from("hwnd:2"),
        }
    }

    fn browser_probes() -> Vec<MethodProbe> {
        vec![
            MethodProbe::safe(MethodId::WebKeyboardSelection, "keyboard selection"),
            MethodProbe::safe(MethodId::UiAutomationEditableText, "editable text"),
            MethodProbe::safe(MethodId::UiAutomationDocumentText, "document text"),
            MethodProbe::risky(MethodId::ClipboardSelection, "clipboard fallback"),
        ]
    }
}
