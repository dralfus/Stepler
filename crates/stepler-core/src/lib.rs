mod engine;
mod language;
mod layout;
mod log_event;
mod transaction;
mod types;

pub use engine::{build_replacement_plan, CorrectionError};
pub use layout::{convert_layout_text, convert_selected_text};
pub use log_event::{LogTrigger, OperationLogEvent};
pub use transaction::{
    OperationGate, OperationMetrics, OperationState, StageTiming, Transaction, TransactionError,
};
pub use types::{
    Capabilities, CorrectionMode, MethodBinding, MethodId, ReplacementPlan, SelectionRange,
    TextContext, TextRange,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pause_builds_plan_for_selected_text() {
        let context = TextContext::new("k.,jdm").with_selection(Some(TextRange::new(0, 6)));

        let plan = build_replacement_plan(&context, CorrectionMode::Pause).unwrap();

        assert_eq!(plan.range, TextRange::new(0, 6));
        assert_eq!(plan.replacement_text, "любовь");
        assert_eq!(plan.expected_before_text, "k.,jdm");
    }

    #[test]
    fn pause_builds_plan_for_word_before_caret() {
        let context = TextContext::new("hello k.,jdm").with_caret(TextRange::caret(12));

        let plan = build_replacement_plan(&context, CorrectionMode::Pause).unwrap();

        assert_eq!(plan.range, TextRange::new(6, 12));
        assert_eq!(plan.replacement_text, "любовь");
        assert_eq!(plan.expected_before_text, "k.,jdm");
    }

    #[test]
    fn pause_builds_plan_for_word_before_trailing_space() {
        let context = TextContext::new("hello k.,jdm ").with_caret(TextRange::caret(13));

        let plan = build_replacement_plan(&context, CorrectionMode::Pause).unwrap();

        assert_eq!(plan.range, TextRange::new(6, 13));
        assert_eq!(plan.replacement_text, "любовь ");
        assert_eq!(plan.expected_before_text, "k.,jdm ");
    }

    #[test]
    fn pause_keeps_layout_punctuation_inside_word_before_trailing_space() {
        let context = TextContext::new("prefix ckf,fz\u{a0}")
            .with_caret(TextRange::caret("prefix ckf,fz\u{a0}".len()));

        let plan = build_replacement_plan(&context, CorrectionMode::Pause).unwrap();

        assert_eq!(
            plan.range,
            TextRange::new("prefix ".len(), "prefix ckf,fz\u{a0}".len())
        );
        assert_eq!(plan.expected_before_text, "ckf,fz\u{a0}");
        assert_eq!(plan.replacement_text, "слабая\u{a0}");
    }

    #[test]
    fn pause_does_not_cross_line_break_for_trailing_space_lookup() {
        let context = TextContext::new("k.,jdm\n").with_caret(TextRange::caret(7));

        let err = build_replacement_plan(&context, CorrectionMode::Pause).unwrap_err();

        assert_eq!(err, CorrectionError::NoTextToReplace);
    }

    #[test]
    fn pause_does_not_convert_filename_when_no_sparse_source_exists() {
        let context = TextContext::new("используй outlookhaging.md")
            .with_caret(TextRange::caret("используй outlookhaging.md".len()));

        let err = build_replacement_plan(&context, CorrectionMode::Pause).unwrap_err();

        assert_eq!(err, CorrectionError::NoTextToReplace);
    }

    #[test]
    fn pause_returns_none_when_no_word_is_available() {
        let context = TextContext::new("   ").with_caret(TextRange::caret(3));

        let err = build_replacement_plan(&context, CorrectionMode::Pause).unwrap_err();

        assert_eq!(err, CorrectionError::NoTextToReplace);
    }

    #[test]
    fn scrolllock_builds_plan_for_mixed_tail() {
        let context = TextContext::new("вальс поле long ghbdtn vbh");

        let plan = build_replacement_plan(&context, CorrectionMode::ScrollLock).unwrap();

        assert_eq!(
            plan.range,
            TextRange::new("вальс поле long ".len(), "вальс поле long ghbdtn vbh".len())
        );
        assert_eq!(plan.expected_before_text, "ghbdtn vbh");
        assert_eq!(plan.replacement_text, "привет мир");
    }

    #[test]
    fn scrolllock_builds_plan_for_single_mistyped_word() {
        let context = TextContext::new("k.,jdm");

        let plan = build_replacement_plan(&context, CorrectionMode::ScrollLock).unwrap();

        assert_eq!(plan.range, TextRange::new(0, "k.,jdm".len()));
        assert_eq!(plan.expected_before_text, "k.,jdm");
        assert_eq!(plan.replacement_text, "любовь");
    }

    #[test]
    fn scrolllock_builds_plan_for_selected_text_like_pause() {
        let context = TextContext::new("ghbdtn vbh").with_selection(Some(TextRange::new(0, 10)));

        let plan = build_replacement_plan(&context, CorrectionMode::ScrollLock).unwrap();

        assert_eq!(plan.range, TextRange::new(0, 10));
        assert_eq!(plan.expected_before_text, "ghbdtn vbh");
        assert_eq!(plan.replacement_text, "привет мир");
    }

    #[test]
    fn scrolllock_builds_plan_for_common_short_terminal_command() {
        let context = TextContext::new("пше");

        let plan = build_replacement_plan(&context, CorrectionMode::ScrollLock).unwrap();

        assert_eq!(plan.range, TextRange::new(0, "пше".len()));
        assert_eq!(plan.expected_before_text, "пше");
        assert_eq!(plan.replacement_text, "git");
    }

    #[test]
    fn scrolllock_uses_layout_override_for_plausible_source_word() {
        let context = TextContext::new("ddble");

        let plan = build_replacement_plan(&context, CorrectionMode::ScrollLock).unwrap();

        assert_eq!(plan.expected_before_text, "ddble");
        assert_eq!(plan.replacement_text, "ввиду");
    }

    #[test]
    fn scrolllock_expands_token_when_caret_is_inside_word() {
        let context = TextContext::new("раз два три. xtnsht,\nлюбовь\nk.,jdm")
            .with_caret(TextRange::caret("раз два три. xtnsht,\nлюбовь\nk.,j".len()));

        let plan = build_replacement_plan(&context, CorrectionMode::ScrollLock).unwrap();

        assert_eq!(plan.expected_before_text, "k.,jdm");
        assert_eq!(plan.replacement_text, "любовь");
    }

    #[test]
    fn scrolllock_keeps_valid_text_unchanged() {
        let context = TextContext::new("раз два три. четыре, ");

        let err = build_replacement_plan(&context, CorrectionMode::ScrollLock).unwrap_err();

        assert_eq!(err, CorrectionError::NoTextToReplace);
    }

    #[test]
    fn scrolllock_keeps_valid_english_prefix_before_short_function_phrase() {
        let context = TextContext::new("created DESC d nf,kbwt exntyj");

        let plan = build_replacement_plan(&context, CorrectionMode::ScrollLock).unwrap();

        assert_eq!(plan.expected_before_text, "d nf,kbwt exntyj");
        assert_eq!(plan.replacement_text, "в таблице учтено");
    }

    #[test]
    fn scrolllock_converts_sql_like_russian_suffix_to_english() {
        let context = TextContext::new("ORDER BY скуфеув ВУЫС");

        let plan = build_replacement_plan(&context, CorrectionMode::ScrollLock).unwrap();

        assert_eq!(plan.expected_before_text, "скуфеув ВУЫС");
        assert_eq!(plan.replacement_text, "created DESC");
    }

    #[test]
    fn scrolllock_converts_unknown_english_typed_in_russian_layout() {
        let context = TextContext::new("ыеи ьфекшч");

        let plan = build_replacement_plan(&context, CorrectionMode::ScrollLock).unwrap();

        assert_eq!(plan.expected_before_text, "ыеи ьфекшч");
        assert_eq!(plan.replacement_text, "stb matrix");
    }

    #[test]
    fn scrolllock_converts_sparse_mistyped_tokens_in_mixed_line() {
        let context = TextContext::new("щту вальс поле long ghbdtn vbh");

        let plan = build_replacement_plan(&context, CorrectionMode::ScrollLock).unwrap();

        assert_eq!(
            plan.range,
            TextRange::new(0, "щту вальс поле long ghbdtn vbh".len())
        );
        assert_eq!(plan.expected_before_text, "щту вальс поле long ghbdtn vbh");
        assert_eq!(plan.replacement_text, "one вальс поле long привет мир");
    }

    #[test]
    fn scrolllock_converts_single_sparse_mistyped_word_inside_mixed_line() {
        let context = TextContext::new("house dfkmc поле long привет мир");

        let plan = build_replacement_plan(&context, CorrectionMode::ScrollLock).unwrap();

        assert_eq!(
            plan.range,
            TextRange::new("house ".len(), "house dfkmc".len())
        );
        assert_eq!(plan.expected_before_text, "dfkmc");
        assert_eq!(plan.replacement_text, "вальс");
    }

    #[test]
    fn scrolllock_ignores_trailing_line_break_after_mistyped_line() {
        let context = TextContext::new(". \r\nbcgjkpeq outlookhaging.md\r\n");

        let plan = build_replacement_plan(&context, CorrectionMode::ScrollLock).unwrap();

        assert_eq!(plan.expected_before_text, "bcgjkpeq");
        assert_eq!(plan.replacement_text, "исползуй");
    }

    #[test]
    fn transaction_follows_successful_lifecycle() {
        let mut transaction = Transaction::new("op-1", CorrectionMode::Pause);

        transaction
            .transition_to(OperationState::HotkeyReceived)
            .unwrap();
        transaction
            .transition_to(OperationState::ContextCaptured)
            .unwrap();
        transaction
            .transition_to(OperationState::PlanBuilt)
            .unwrap();
        transaction
            .transition_to(OperationState::PreflightChecked)
            .unwrap();
        transaction
            .transition_to(OperationState::ReplacementApplied)
            .unwrap();
        transaction.transition_to(OperationState::Verified).unwrap();
        transaction
            .transition_to(OperationState::Completed)
            .unwrap();

        assert_eq!(transaction.state(), OperationState::Completed);
        assert!(transaction.is_terminal());
        assert!(!transaction.metrics().timings.is_empty());
    }

    #[test]
    fn transaction_rejects_double_apply() {
        let mut transaction = Transaction::new("op-1", CorrectionMode::Pause);

        transaction
            .transition_to(OperationState::HotkeyReceived)
            .unwrap();
        transaction
            .transition_to(OperationState::ContextCaptured)
            .unwrap();
        transaction
            .transition_to(OperationState::PlanBuilt)
            .unwrap();
        transaction
            .transition_to(OperationState::PreflightChecked)
            .unwrap();
        transaction
            .transition_to(OperationState::ReplacementApplied)
            .unwrap();

        let err = transaction
            .transition_to(OperationState::ReplacementApplied)
            .unwrap_err();

        assert_eq!(err, TransactionError::InvalidTransition);
    }

    #[test]
    fn transaction_metrics_include_failed_operations() {
        let mut transaction = Transaction::new("op-1", CorrectionMode::ScrollLock);

        transaction
            .transition_to(OperationState::HotkeyReceived)
            .unwrap();
        transaction.fail();

        let metrics = transaction.metrics();

        assert_eq!(transaction.state(), OperationState::RolledBackOrFailed);
        assert!(transaction.is_terminal());
        assert_eq!(
            metrics.timings.last().map(|timing| timing.state),
            Some(OperationState::RolledBackOrFailed)
        );
    }

    #[test]
    fn transaction_metrics_include_no_change_operations() {
        let mut transaction = Transaction::new("op-1", CorrectionMode::ScrollLock);

        transaction
            .transition_to(OperationState::HotkeyReceived)
            .unwrap();
        transaction.transition_to(OperationState::NoChange).unwrap();

        let metrics = transaction.metrics();

        assert_eq!(transaction.state(), OperationState::NoChange);
        assert!(transaction.is_terminal());
        assert_eq!(
            metrics.timings.last().map(|timing| timing.state),
            Some(OperationState::NoChange)
        );
    }

    #[test]
    fn transaction_metrics_include_unsupported_operations() {
        let mut transaction = Transaction::new("op-1", CorrectionMode::Pause);

        transaction
            .transition_to(OperationState::HotkeyReceived)
            .unwrap();
        transaction
            .transition_to(OperationState::Unsupported)
            .unwrap();

        let metrics = transaction.metrics();

        assert_eq!(transaction.state(), OperationState::Unsupported);
        assert!(transaction.is_terminal());
        assert_eq!(
            metrics.timings.last().map(|timing| timing.state),
            Some(OperationState::Unsupported)
        );
    }

    #[test]
    fn operation_gate_blocks_duplicate_control_until_release() {
        let mut gate = OperationGate::new();

        gate.try_acquire("window-1/control-1").unwrap();
        let err = gate.try_acquire("window-1/control-1").unwrap_err();

        assert_eq!(err, TransactionError::OperationAlreadyActive);

        gate.release("window-1/control-1");
        gate.try_acquire("window-1/control-1").unwrap();
    }

    #[test]
    fn operation_log_event_formats_jsonl() {
        let event = OperationLogEvent {
            operation_id: String::from("op-1"),
            timestamp_unix_ms: 1_718_000_000_123,
            trigger: LogTrigger::Pause,
            state: OperationState::ReplacementApplied,
            app: Some(String::from("Notepad")),
            provider: Some(String::from("Win32EditProvider")),
            replacer: Some(String::from("Win32EditReplacer")),
            range: Some(TextRange::new(10, 16)),
            expected_before_text: Some(String::from("k.,jdm")),
            replacement_text: Some(String::from("любовь")),
            resolver_trace: Some(String::from(
                "surface=EditControl; final=operation_failed:x",
            )),
            clipboard_used: false,
            duration_ms: 24,
            timings: vec![StageTiming {
                state: OperationState::ContextCaptured,
                elapsed_ms: 2,
            }],
        };

        let json = event.to_json_line();

        assert!(json.contains("\"operation_id\":\"op-1\""));
        assert!(json.contains("\"timestamp_unix_ms\":1718000000123"));
        assert!(json.contains("\"trigger\":\"Pause\""));
        assert!(json.contains("\"range\":[10,16]"));
        assert!(
            json.contains("\"resolver_trace\":\"surface=EditControl; final=operation_failed:x\"")
        );
        assert!(json.contains("\"timings_ms\":[{\"state\":\"ContextCaptured\",\"elapsed_ms\":2}]"));
        assert!(json.ends_with('\n'));
    }

    #[test]
    fn operation_log_event_formats_no_change_state() {
        let event = OperationLogEvent {
            operation_id: String::from("unknown"),
            timestamp_unix_ms: 1_718_000_000_123,
            trigger: LogTrigger::ScrollLock,
            state: OperationState::NoChange,
            app: None,
            provider: Some(String::from("WindowsTextContextProvider")),
            replacer: Some(String::from("WindowsTextReplacer")),
            range: None,
            expected_before_text: Some(String::from("Correction(NoTextToReplace)")),
            replacement_text: None,
            resolver_trace: None,
            clipboard_used: false,
            duration_ms: 42,
            timings: Vec::new(),
        };

        let json = event.to_json_line();

        assert!(json.contains("\"trigger\":\"ScrollLock\""));
        assert!(json.contains("\"state\":\"NoChange\""));
        assert!(json.contains("\"expected_before_text\":\"Correction(NoTextToReplace)\""));
    }

    #[test]
    fn operation_log_event_formats_unsupported_state() {
        let event = OperationLogEvent {
            operation_id: String::from("unsupported"),
            timestamp_unix_ms: 1_718_000_000_123,
            trigger: LogTrigger::Pause,
            state: OperationState::Unsupported,
            app: None,
            provider: Some(String::from("WindowsTextContextProvider")),
            replacer: None,
            range: None,
            expected_before_text: Some(String::from("unsupported_surface")),
            replacement_text: None,
            resolver_trace: None,
            clipboard_used: false,
            duration_ms: 0,
            timings: Vec::new(),
        };

        let json = event.to_json_line();

        assert!(json.contains("\"trigger\":\"Pause\""));
        assert!(json.contains("\"state\":\"Unsupported\""));
        assert!(json.contains("\"expected_before_text\":\"unsupported_surface\""));
    }
}
