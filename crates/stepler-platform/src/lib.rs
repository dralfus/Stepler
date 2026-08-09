mod resolver;
mod surface;
mod target_facts;

pub use resolver::{
    MethodResolver, ResolveDecision, ResolveError, ResolveTraceEntry, ResolveTraceOutcome,
};
pub use surface::{
    classify_surface, default_app_policies, default_app_policy, default_probe_policies,
    default_surface_policies, default_surface_policy, probe_plan_for, probe_policy_for,
    split_preferences, surface_allows_risky_method, surface_policy_for,
    surface_uses_fast_web_keyboard, surface_uses_rocket_web_keyboard,
    web_keyboard_profile_for_surface, MethodPreferences, ProbePlan, ProbePolicy,
    SurfaceClassification, SurfaceKind, SurfacePolicy, WebKeyboardProfile,
};
pub use target_facts::{target_facts, TargetFacts};

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
    pub retry_count: u32,
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

pub const BRIDGE_METHOD_IDS: &[MethodId] = &[
    MethodId::TerminalClipboardShortcut,
    MethodId::SshTerminal,
    MethodId::PsReadLine,
    MethodId::XtermKeyboardSelection,
];

pub fn method_is_bridge_method(method: MethodId) -> bool {
    BRIDGE_METHOD_IDS.contains(&method)
}

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
    fn resolver_trace_explains_policy_rejections_and_acceptance() {
        let resolver = MethodResolver::default();
        let mut target = target("Chrome_WidgetWin_1", "Chrome_WidgetWin_1");
        target.title = String::from("Codex");
        let probes = vec![
            MethodProbe::risky(MethodId::ClipboardSelection, "clipboard fallback"),
            MethodProbe::safe(MethodId::UiAutomationDocumentText, "uia document"),
            MethodProbe::safe(MethodId::WebKeyboardSelection, "web keyboard"),
        ];

        let trace = resolver.trace_for_mode(&target, &probes, CorrectionMode::Pause);

        assert_eq!(trace[0].method, MethodId::WebKeyboardSelection);
        assert_eq!(trace[0].outcome, ResolveTraceOutcome::Accepted);
        assert!(trace
            .iter()
            .any(|entry| entry.method == MethodId::UiAutomationDocumentText
                && entry.outcome == ResolveTraceOutcome::ForbiddenByPolicy));
        assert!(trace
            .iter()
            .any(|entry| entry.method == MethodId::ClipboardSelection
                && entry.outcome == ResolveTraceOutcome::ForbiddenByPolicy));
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
        assert_eq!(decision.replacement_method, MethodId::WebKeyboardSelection);

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
            pause_methods: split_preferences(
                vec![MethodId::Win32EditMessages],
                vec![MethodId::Win32EditMessages],
            ),
            scrolllock_methods: split_preferences(
                vec![MethodId::WebKeyboardSelection],
                vec![MethodId::WebKeyboardSelection],
            ),
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
}
