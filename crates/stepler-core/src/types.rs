#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorrectionMode {
    Pause,
    ScrollLock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextRange {
    pub start: usize,
    pub end: usize,
}

impl TextRange {
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub const fn caret(offset: usize) -> Self {
        Self {
            start: offset,
            end: offset,
        }
    }

    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }
}

pub type SelectionRange = TextRange;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MethodId {
    Win32EditMessages,
    TerminalClipboardShortcut,
    SshTerminal,
    ConsoleBuffer,
    PsReadLine,
    WordCom,
    UiAutomationEditableText,
    UiAutomationDocumentText,
    UiAutomationText,
    XtermKeyboardSelection,
    WebKeyboardSelection,
    ClipboardSelection,
    SendInput,
}

impl MethodId {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Win32EditMessages => "win32_edit_messages",
            Self::TerminalClipboardShortcut => "terminal_clipboard_shortcut",
            Self::SshTerminal => "ssh_terminal",
            Self::ConsoleBuffer => "console_buffer",
            Self::PsReadLine => "psreadline",
            Self::WordCom => "word_com",
            Self::UiAutomationEditableText => "uia_editable_text",
            Self::UiAutomationDocumentText => "uia_document_text",
            Self::UiAutomationText => "uia_text",
            Self::XtermKeyboardSelection => "xterm_keyboard_selection",
            Self::WebKeyboardSelection => "web_keyboard_selection",
            Self::ClipboardSelection => "clipboard_selection",
            Self::SendInput => "send_input",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MethodBinding {
    pub context_method: MethodId,
    pub replace_methods: Vec<MethodId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TelemetryTiming {
    pub phase: String,
    pub elapsed_ms: u128,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextTelemetry {
    pub surface_kind: Option<String>,
    pub surface_confidence: Option<u8>,
    pub profile: Option<String>,
    pub capture_branch: Option<String>,
    pub retry_count: u32,
    pub timings: Vec<TelemetryTiming>,
}

impl Default for ContextTelemetry {
    fn default() -> Self {
        Self {
            surface_kind: None,
            surface_confidence: None,
            profile: None,
            capture_branch: None,
            retry_count: 0,
            timings: Vec::new(),
        }
    }
}

impl MethodBinding {
    pub fn new(context_method: MethodId, replace_methods: Vec<MethodId>) -> Self {
        Self {
            context_method,
            replace_methods,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capabilities {
    pub can_replace_directly: bool,
    pub can_read_selection: bool,
    pub can_read_caret: bool,
    pub method_binding: Option<MethodBinding>,
}

impl Default for Capabilities {
    fn default() -> Self {
        Self {
            can_replace_directly: false,
            can_read_selection: true,
            can_read_caret: true,
            method_binding: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextContext {
    pub app_id: String,
    pub window_id: String,
    pub control_id: String,
    pub text_snapshot: String,
    pub caret_range: TextRange,
    pub selection_range: Option<SelectionRange>,
    pub capabilities: Capabilities,
    pub telemetry: ContextTelemetry,
}

impl TextContext {
    pub fn new(text_snapshot: impl Into<String>) -> Self {
        let text_snapshot = text_snapshot.into();
        let end = text_snapshot.len();
        Self {
            app_id: String::new(),
            window_id: String::new(),
            control_id: String::new(),
            text_snapshot,
            caret_range: TextRange::caret(end),
            selection_range: None,
            capabilities: Capabilities::default(),
            telemetry: ContextTelemetry::default(),
        }
    }

    pub fn with_caret(mut self, caret_range: TextRange) -> Self {
        self.caret_range = caret_range;
        self
    }

    pub fn with_selection(mut self, selection_range: Option<SelectionRange>) -> Self {
        self.selection_range = selection_range;
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReplacementPlan {
    pub range: TextRange,
    pub replacement_text: String,
    pub reason: String,
    pub confidence: f32,
    pub expected_before_text: String,
}
