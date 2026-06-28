use crate::transaction::{OperationState, StageTiming};
use crate::types::{CorrectionMode, TextRange};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogTrigger {
    Pause,
    ScrollLock,
}

impl From<CorrectionMode> for LogTrigger {
    fn from(value: CorrectionMode) -> Self {
        match value {
            CorrectionMode::Pause => Self::Pause,
            CorrectionMode::ScrollLock => Self::ScrollLock,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationLogEvent {
    pub operation_id: String,
    pub timestamp_unix_ms: u128,
    pub trigger: LogTrigger,
    pub state: OperationState,
    pub app: Option<String>,
    pub provider: Option<String>,
    pub replacer: Option<String>,
    pub range: Option<TextRange>,
    pub expected_before_text: Option<String>,
    pub replacement_text: Option<String>,
    pub resolver_trace: Option<String>,
    pub clipboard_used: bool,
    pub duration_ms: u128,
    pub timings: Vec<StageTiming>,
}

impl OperationLogEvent {
    pub fn to_json_line(&self) -> String {
        let mut fields = Vec::new();
        fields.push(json_string_field("operation_id", &self.operation_id));
        fields.push(format!("\"timestamp_unix_ms\":{}", self.timestamp_unix_ms));
        fields.push(json_string_field("trigger", self.trigger.as_str()));
        fields.push(json_string_field("state", self.state.as_str()));
        if let Some(app) = &self.app {
            fields.push(json_string_field("app", app));
        }
        if let Some(provider) = &self.provider {
            fields.push(json_string_field("provider", provider));
        }
        if let Some(replacer) = &self.replacer {
            fields.push(json_string_field("replacer", replacer));
        }
        if let Some(range) = self.range {
            fields.push(format!("\"range\":[{},{}]", range.start, range.end));
        }
        if let Some(expected_before_text) = &self.expected_before_text {
            fields.push(json_string_field(
                "expected_before_text",
                expected_before_text,
            ));
        }
        if let Some(replacement_text) = &self.replacement_text {
            fields.push(json_string_field("replacement_text", replacement_text));
        }
        if let Some(resolver_trace) = &self.resolver_trace {
            fields.push(json_string_field("resolver_trace", resolver_trace));
        }
        fields.push(format!("\"clipboard_used\":{}", self.clipboard_used));
        fields.push(format!("\"duration_ms\":{}", self.duration_ms));
        if !self.timings.is_empty() {
            fields.push(format!("\"timings_ms\":{}", timings_json(&self.timings)));
        }

        format!("{{{}}}\n", fields.join(","))
    }
}

impl LogTrigger {
    fn as_str(self) -> &'static str {
        match self {
            Self::Pause => "Pause",
            Self::ScrollLock => "ScrollLock",
        }
    }
}

impl OperationState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::HotkeyReceived => "HotkeyReceived",
            Self::ContextCaptured => "ContextCaptured",
            Self::PlanBuilt => "PlanBuilt",
            Self::PreflightChecked => "PreflightChecked",
            Self::ReplacementApplied => "ReplacementApplied",
            Self::Verified => "Verified",
            Self::NoChange => "NoChange",
            Self::Unsupported => "Unsupported",
            Self::RolledBackOrFailed => "RolledBackOrFailed",
            Self::Completed => "Completed",
        }
    }
}

fn json_string_field(name: &str, value: &str) -> String {
    format!("\"{}\":\"{}\"", name, escape_json_string(value))
}

fn timings_json(timings: &[StageTiming]) -> String {
    let values = timings
        .iter()
        .map(|timing| {
            format!(
                "{{\"state\":\"{}\",\"elapsed_ms\":{}}}",
                timing.state.as_str(),
                timing.elapsed_ms
            )
        })
        .collect::<Vec<_>>();
    format!("[{}]", values.join(","))
}

fn escape_json_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            ch if ch.is_control() => escaped.push_str(&format!("\\u{:04X}", ch as u32)),
            ch => escaped.push(ch),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_json_strings() {
        assert_eq!(escape_json_string("a\"b\\c\n"), "a\\\"b\\\\c\\n");
    }
}
