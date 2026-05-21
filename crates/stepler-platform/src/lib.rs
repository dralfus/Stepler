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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppPolicy {
    pub app_matcher: String,
    pub preferred_context_methods: Vec<MethodId>,
    pub preferred_replace_methods: Vec<MethodId>,
    pub forbidden_methods: Vec<MethodId>,
    pub allow_risky_methods: bool,
}

impl AppPolicy {
    pub fn matches(&self, target: &ForegroundTarget) -> bool {
        self.app_matcher == "*"
            || matcher_matches(&self.app_matcher, &target.app_class)
            || matcher_matches(&self.app_matcher, &target.focused_class)
            || target
                .process_name
                .as_ref()
                .is_some_and(|process| matcher_matches(&self.app_matcher, process))
    }
}

fn matcher_matches(matcher: &str, value: &str) -> bool {
    value.eq_ignore_ascii_case(matcher)
        || value
            .get(..matcher.len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(matcher))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolveDecision {
    pub context_method: MethodId,
    pub replacement_method: MethodId,
    pub safety: ProbeSafety,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveError {
    NoSupportedMethod,
    ForbiddenByPolicy(MethodId),
    RiskyMethodBlocked(MethodId),
}

#[derive(Debug, Clone)]
pub struct MethodResolver {
    policies: Vec<AppPolicy>,
}

impl MethodResolver {
    pub fn new(policies: Vec<AppPolicy>) -> Self {
        Self { policies }
    }

    pub fn resolve(
        &self,
        target: &ForegroundTarget,
        probes: &[MethodProbe],
    ) -> Result<ResolveDecision, ResolveError> {
        let policy = self.policy_for(target);
        let mut candidates = probes
            .iter()
            .filter(|probe| probe.safety != ProbeSafety::Unsupported)
            .collect::<Vec<_>>();
        candidates.sort_by_key(|probe| {
            (
                method_preference_rank(probe.method_id, &policy.preferred_context_methods),
                std::cmp::Reverse(probe.confidence),
            )
        });

        for probe in candidates {
            if policy.forbidden_methods.contains(&probe.method_id) {
                continue;
            }
            if probe.safety == ProbeSafety::Risky && !policy.allow_risky_methods {
                continue;
            }
            let replacement_method = policy
                .preferred_replace_methods
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
                reason: format!("{} via policy {}", probe.reason, policy.app_matcher),
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
                && !policy.allow_risky_methods
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

    fn policy_for(&self, target: &ForegroundTarget) -> AppPolicy {
        self.policies
            .iter()
            .find(|policy| policy.matches(target))
            .cloned()
            .unwrap_or_else(default_app_policy)
    }
}

impl Default for MethodResolver {
    fn default() -> Self {
        Self::new(default_app_policies())
    }
}

pub fn default_app_policies() -> Vec<AppPolicy> {
    vec![
        AppPolicy {
            app_matcher: String::from("Edit"),
            preferred_context_methods: vec![MethodId::Win32EditMessages],
            preferred_replace_methods: vec![MethodId::Win32EditMessages],
            forbidden_methods: vec![],
            allow_risky_methods: false,
        },
        AppPolicy {
            app_matcher: String::from("ConsoleWindowClass"),
            preferred_context_methods: vec![MethodId::ConsoleBuffer],
            preferred_replace_methods: vec![MethodId::ConsoleBuffer],
            forbidden_methods: vec![MethodId::TerminalClipboardShortcut],
            allow_risky_methods: false,
        },
        AppPolicy {
            app_matcher: String::from("CASCADIA_HOSTING_WINDOW_CLASS"),
            preferred_context_methods: vec![MethodId::PsReadLine],
            preferred_replace_methods: vec![MethodId::PsReadLine],
            forbidden_methods: vec![MethodId::TerminalClipboardShortcut],
            allow_risky_methods: false,
        },
        AppPolicy {
            app_matcher: String::from("Chrome_WidgetWin"),
            preferred_context_methods: vec![
                MethodId::WebKeyboardSelection,
                MethodId::UiAutomationEditableText,
            ],
            preferred_replace_methods: vec![
                MethodId::WebKeyboardSelection,
                MethodId::UiAutomationEditableText,
            ],
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
        AppPolicy {
            app_matcher: String::from("MozillaWindowClass"),
            preferred_context_methods: vec![
                MethodId::WebKeyboardSelection,
                MethodId::UiAutomationEditableText,
            ],
            preferred_replace_methods: vec![
                MethodId::WebKeyboardSelection,
                MethodId::UiAutomationEditableText,
            ],
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
        AppPolicy {
            app_matcher: String::from("WINWORD"),
            preferred_context_methods: vec![
                MethodId::WordCom,
                MethodId::UiAutomationEditableText,
                MethodId::UiAutomationDocumentText,
                MethodId::UiAutomationText,
            ],
            preferred_replace_methods: vec![
                MethodId::WordCom,
                MethodId::UiAutomationEditableText,
                MethodId::UiAutomationDocumentText,
                MethodId::UiAutomationText,
            ],
            forbidden_methods: vec![
                MethodId::Win32EditMessages,
                MethodId::TerminalClipboardShortcut,
                MethodId::ClipboardSelection,
            ],
            allow_risky_methods: false,
        },
        AppPolicy {
            app_matcher: String::from("OpusApp"),
            preferred_context_methods: vec![
                MethodId::WordCom,
                MethodId::UiAutomationEditableText,
                MethodId::UiAutomationDocumentText,
                MethodId::UiAutomationText,
            ],
            preferred_replace_methods: vec![
                MethodId::WordCom,
                MethodId::UiAutomationEditableText,
                MethodId::UiAutomationDocumentText,
                MethodId::UiAutomationText,
            ],
            forbidden_methods: vec![
                MethodId::Win32EditMessages,
                MethodId::TerminalClipboardShortcut,
                MethodId::ClipboardSelection,
            ],
            allow_risky_methods: false,
        },
        default_app_policy(),
    ]
}

pub fn default_app_policy() -> AppPolicy {
    AppPolicy {
        app_matcher: String::from("*"),
        preferred_context_methods: vec![
            MethodId::Win32EditMessages,
            MethodId::UiAutomationEditableText,
            MethodId::UiAutomationDocumentText,
            MethodId::UiAutomationText,
            MethodId::WebKeyboardSelection,
            MethodId::ConsoleBuffer,
            MethodId::PsReadLine,
            MethodId::ClipboardSelection,
            MethodId::SendInput,
        ],
        preferred_replace_methods: vec![
            MethodId::Win32EditMessages,
            MethodId::UiAutomationEditableText,
            MethodId::UiAutomationDocumentText,
            MethodId::UiAutomationText,
            MethodId::WebKeyboardSelection,
            MethodId::ConsoleBuffer,
            MethodId::PsReadLine,
            MethodId::ClipboardSelection,
            MethodId::SendInput,
        ],
        forbidden_methods: vec![],
        allow_risky_methods: risky_fallbacks_enabled(),
    }
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
    fn resolver_prefers_web_keyboard_for_browser_like_classes() {
        let resolver = MethodResolver::default();
        let target = target("Chrome_WidgetWin_1", "Chrome_WidgetWin_1");
        let probes = vec![
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
}
