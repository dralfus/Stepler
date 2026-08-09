use crate::log_event::LogTrigger;
use crate::transaction::{OperationMetrics, OperationState};
use crate::types::{ReplacementPlan, TelemetryTiming, TextContext};

const UNKNOWN: &str = "unknown";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerformanceEvent {
    pub operation_id: String,
    pub timestamp_unix_ms: u128,
    pub trigger: LogTrigger,
    pub outcome: OperationState,
    pub build_version: String,
    pub environment_label: String,
    pub surface_kind: String,
    pub surface_confidence: u8,
    pub context_method: String,
    pub replacement_method: String,
    pub profile: String,
    pub algorithm_branch: String,
    pub selection_state: String,
    pub cold_warm: String,
    pub retry_count: u32,
    pub input_length: Option<usize>,
    pub replacement_length: Option<usize>,
    pub range: Option<(usize, usize)>,
    pub clipboard_used: bool,
    pub duration_ms: u128,
    pub timings: Vec<TelemetryTiming>,
}

impl PerformanceEvent {
    #[allow(clippy::too_many_arguments)]
    pub fn from_operation(
        operation_id: impl Into<String>,
        timestamp_unix_ms: u128,
        trigger: LogTrigger,
        outcome: OperationState,
        build_version: impl Into<String>,
        environment_label: impl Into<String>,
        context: Option<&TextContext>,
        plan: Option<&ReplacementPlan>,
        replacement_method: Option<&str>,
        replacement_retry_count: u32,
        replacement_timings: &[TelemetryTiming],
        metrics: Option<&OperationMetrics>,
        cold_warm: impl Into<String>,
        clipboard_used: bool,
    ) -> Self {
        let binding = context.and_then(|value| value.capabilities.method_binding.as_ref());
        let selection_state = context
            .map(|value| match value.selection_range {
                Some(range) if !range.is_empty() => "selected",
                Some(_) | None => "none",
            })
            .unwrap_or(UNKNOWN)
            .to_owned();
        let telemetry = context.map(|value| &value.telemetry);

        let mut timings = telemetry
            .map(|value| value.timings.clone())
            .unwrap_or_default();
        timings.extend(replacement_timings.iter().cloned());
        timings.extend(
            metrics
                .into_iter()
                .flat_map(|value| value.timings.iter())
                .map(|timing| TelemetryTiming {
                    phase: timing.state.as_str().to_owned(),
                    elapsed_ms: timing.elapsed_ms,
                }),
        );

        Self {
            operation_id: operation_id.into(),
            timestamp_unix_ms,
            trigger,
            outcome,
            build_version: build_version.into(),
            environment_label: environment_label.into(),
            surface_kind: telemetry
                .and_then(|value| value.surface_kind.clone())
                .unwrap_or_else(|| UNKNOWN.to_owned()),
            surface_confidence: telemetry
                .and_then(|value| value.surface_confidence)
                .unwrap_or_default(),
            context_method: binding
                .map(|value| value.context_method.as_str().to_owned())
                .unwrap_or_else(|| UNKNOWN.to_owned()),
            replacement_method: replacement_method.unwrap_or(UNKNOWN).to_owned(),
            profile: telemetry
                .and_then(|value| value.profile.clone())
                .unwrap_or_else(|| UNKNOWN.to_owned()),
            algorithm_branch: telemetry
                .and_then(|value| value.capture_branch.clone())
                .unwrap_or_else(|| UNKNOWN.to_owned()),
            selection_state,
            cold_warm: cold_warm.into(),
            retry_count: telemetry
                .map(|value| value.retry_count)
                .unwrap_or_default()
                .max(replacement_retry_count),
            input_length: context.map(|value| value.text_snapshot.len()),
            replacement_length: plan.map(|value| value.replacement_text.len()),
            range: plan.map(|value| (value.range.start, value.range.end)),
            clipboard_used,
            duration_ms: metrics.map(|value| value.duration_ms).unwrap_or_default(),
            timings,
        }
    }

    pub fn to_json_line(&self) -> String {
        let mut fields = Vec::new();
        fields.push(json_string_field("event", "performance_operation_v1"));
        fields.push(json_string_field("operation_id", &self.operation_id));
        fields.push(format!("\"timestamp_unix_ms\":{}", self.timestamp_unix_ms));
        fields.push(json_string_field("trigger", self.trigger.as_str()));
        fields.push(json_string_field("outcome", self.outcome.as_str()));
        fields.push(json_string_field("build_version", &self.build_version));
        fields.push(json_string_field(
            "environment_label",
            &self.environment_label,
        ));
        fields.push(json_string_field("surface_kind", &self.surface_kind));
        fields.push(format!(
            "\"surface_confidence\":{}",
            self.surface_confidence
        ));
        fields.push(json_string_field("context_method", &self.context_method));
        fields.push(json_string_field(
            "replacement_method",
            &self.replacement_method,
        ));
        fields.push(json_string_field("profile", &self.profile));
        fields.push(json_string_field(
            "algorithm_branch",
            &self.algorithm_branch,
        ));
        fields.push(json_string_field("selection_state", &self.selection_state));
        fields.push(json_string_field("cold_warm", &self.cold_warm));
        fields.push(format!("\"retry_count\":{}", self.retry_count));
        push_optional_usize(&mut fields, "input_length", self.input_length);
        push_optional_usize(&mut fields, "replacement_length", self.replacement_length);
        fields.push(format!(
            "\"range\":{}",
            self.range
                .map(|(start, end)| format!("[{start},{end}]"))
                .unwrap_or_else(|| "null".to_owned())
        ));
        fields.push(format!("\"clipboard_used\":{}", self.clipboard_used));
        fields.push(format!("\"duration_ms\":{}", self.duration_ms));
        fields.push(format!("\"timings_ms\":{}", timings_json(&self.timings)));
        format!("{{{}}}\n", fields.join(","))
    }
}

fn push_optional_usize(fields: &mut Vec<String>, name: &str, value: Option<usize>) {
    fields.push(format!(
        "\"{name}\":{}",
        value
            .map(|value| value.to_string())
            .unwrap_or_else(|| "null".to_owned())
    ));
}

fn timings_json(timings: &[TelemetryTiming]) -> String {
    let values = timings
        .iter()
        .map(|timing| {
            format!(
                "{{\"phase\":\"{}\",\"elapsed_ms\":{}}}",
                escape_json_string(&timing.phase),
                timing.elapsed_ms
            )
        })
        .collect::<Vec<_>>();
    format!("[{}]", values.join(","))
}

fn json_string_field(name: &str, value: &str) -> String {
    format!("\"{name}\":\"{}\"", escape_json_string(value))
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
    use crate::{Capabilities, ContextTelemetry, MethodBinding, MethodId, TextRange};

    #[test]
    fn performance_event_is_observable_without_user_text() {
        let mut context = TextContext::new("secret user text");
        context.capabilities = Capabilities {
            can_replace_directly: true,
            can_read_selection: true,
            can_read_caret: true,
            method_binding: Some(MethodBinding::new(
                MethodId::WebKeyboardSelection,
                vec![MethodId::WebKeyboardSelection],
            )),
        };
        context.telemetry = ContextTelemetry {
            surface_kind: Some("FastBrowserEditor".to_owned()),
            surface_confidence: Some(95),
            profile: Some("Fast".to_owned()),
            capture_branch: Some("web-keyboard-line-selection".to_owned()),
            retry_count: 1,
            timings: vec![TelemetryTiming {
                phase: "capture".to_owned(),
                elapsed_ms: 7,
            }],
        };
        context.selection_range = Some(TextRange::new(0, 6));
        let plan = ReplacementPlan {
            range: TextRange::new(0, 6),
            replacement_text: "result".to_owned(),
            reason: "test".to_owned(),
            confidence: 0.9,
            expected_before_text: "secret".to_owned(),
        };
        let event = PerformanceEvent::from_operation(
            "op-1",
            42,
            LogTrigger::Pause,
            OperationState::Completed,
            "1.0.test",
            "home-win11",
            Some(&context),
            Some(&plan),
            Some("web_keyboard_selection"),
            0,
            &[],
            None,
            "cold",
            true,
        );
        let json = event.to_json_line();

        assert!(json.contains("performance_operation_v1"));
        assert!(json.contains("FastBrowserEditor"));
        assert!(json.contains("\"retry_count\":1"));
        assert!(json.contains("\"phase\":\"capture\""));
        assert!(json.contains("\"selection_state\":\"selected\""));
        assert!(!json.contains("secret user text"));
        assert!(!json.contains("secret"));
    }
}
