use std::time::{Duration, Instant};
use stepler_core::{
    build_replacement_plan, CorrectionError, CorrectionMode, MethodId, OperationGate,
    OperationMetrics, OperationState, ReplacementPlan, TextContext, Transaction, TransactionError,
};
use stepler_platform::{
    ApplyReplacementResult, ClipboardBackend, ClipboardSnapshot, ForegroundProvider, PlatformError,
    TextContextProvider, TextReplacer,
};

#[derive(Debug, Clone, PartialEq)]
pub struct OperationOutcome {
    pub operation_id: String,
    pub context: TextContext,
    pub plan: ReplacementPlan,
    pub apply_result: ApplyReplacementResult,
    pub clipboard_guard: Option<ClipboardGuardReport>,
    pub metrics: OperationMetrics,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardGuardReport {
    pub before: ClipboardSnapshot,
    pub after_before_restore: Option<ClipboardSnapshot>,
    pub clipboard_changed: bool,
    pub restore_ok: bool,
    pub restore_attempts: usize,
    pub donor_marker_seen: bool,
    pub final_snapshot: Option<ClipboardSnapshot>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationError {
    Platform(PlatformError),
    Correction(CorrectionError),
    CorrectionWithContext(CorrectionError, TextContext),
    Transaction(TransactionError),
    ForegroundChanged,
}

#[derive(Debug, Default)]
pub struct NoClipboard;

impl ClipboardBackend for NoClipboard {
    fn capture(&self) -> Result<ClipboardSnapshot, PlatformError> {
        Err(PlatformError::Unsupported)
    }

    fn restore(&self, _snapshot: ClipboardSnapshot) -> Result<(), PlatformError> {
        Err(PlatformError::Unsupported)
    }
}

pub struct OperationRunner<'a, F, C, R, B = NoClipboard> {
    foreground: &'a F,
    context_provider: &'a C,
    replacer: &'a R,
    clipboard: Option<&'a B>,
    gate: OperationGate,
    next_operation_number: u64,
}

impl<'a, F, C, R> OperationRunner<'a, F, C, R, NoClipboard>
where
    F: ForegroundProvider,
    C: TextContextProvider,
    R: TextReplacer,
{
    pub fn new(foreground: &'a F, context_provider: &'a C, replacer: &'a R) -> Self {
        Self {
            foreground,
            context_provider,
            replacer,
            clipboard: None,
            gate: OperationGate::new(),
            next_operation_number: 0,
        }
    }
}

impl<'a, F, C, R, B> OperationRunner<'a, F, C, R, B>
where
    F: ForegroundProvider,
    C: TextContextProvider,
    R: TextReplacer,
    B: ClipboardBackend,
{
    pub fn new_with_clipboard(
        foreground: &'a F,
        context_provider: &'a C,
        replacer: &'a R,
        clipboard: &'a B,
    ) -> Self {
        Self {
            foreground,
            context_provider,
            replacer,
            clipboard: Some(clipboard),
            gate: OperationGate::new(),
            next_operation_number: 0,
        }
    }

    pub fn handle_hotkey(
        &mut self,
        mode: CorrectionMode,
    ) -> Result<OperationOutcome, OperationError> {
        self.handle_hotkey_with_pre_apply(mode, |_, _| {})
    }

    pub fn handle_hotkey_with_pre_apply<P>(
        &mut self,
        mode: CorrectionMode,
        pre_apply: P,
    ) -> Result<OperationOutcome, OperationError>
    where
        P: FnOnce(&TextContext, &ReplacementPlan),
    {
        self.next_operation_number += 1;
        let operation_id = format!("op-{}", self.next_operation_number);
        let mut transaction = Transaction::new(operation_id.clone(), mode);

        transaction
            .transition_to(OperationState::HotkeyReceived)
            .map_err(OperationError::Transaction)?;

        let foreground_before = self
            .foreground
            .foreground_control()
            .map_err(OperationError::Platform)?;
        let control_key = foreground_before.key();
        self.gate
            .try_acquire(control_key.clone())
            .map_err(OperationError::Transaction)?;

        let result = self.handle_acquired_operation(
            &mut transaction,
            mode,
            &foreground_before.key(),
            pre_apply,
        );
        self.gate.release(&control_key);
        result
    }

    fn handle_acquired_operation<P>(
        &self,
        transaction: &mut Transaction,
        mode: CorrectionMode,
        expected_control_key: &str,
        pre_apply: P,
    ) -> Result<OperationOutcome, OperationError>
    where
        P: FnOnce(&TextContext, &ReplacementPlan),
    {
        let context = self
            .context_provider
            .text_context()
            .map_err(OperationError::Platform)?;
        transaction
            .transition_to(OperationState::ContextCaptured)
            .map_err(OperationError::Transaction)?;

        let plan = build_replacement_plan(&context, mode)
            .map_err(|error| OperationError::CorrectionWithContext(error, context.clone()))?;
        transaction
            .transition_to(OperationState::PlanBuilt)
            .map_err(OperationError::Transaction)?;

        let foreground_before_apply = self
            .foreground
            .foreground_control()
            .map_err(OperationError::Platform)?;
        if foreground_before_apply.key() != expected_control_key {
            transaction.fail();
            return Err(OperationError::ForegroundChanged);
        }

        transaction
            .transition_to(OperationState::PreflightChecked)
            .map_err(OperationError::Transaction)?;

        let should_guard_clipboard = should_guard_clipboard(&context);
        let clipboard_before = should_guard_clipboard
            .then(|| {
                self.clipboard
                    .and_then(|clipboard| clipboard.capture().ok())
            })
            .flatten();
        pre_apply(&context, &plan);
        let foreground_after_pre_apply = self
            .foreground
            .foreground_control()
            .map_err(OperationError::Platform)?;
        if foreground_after_pre_apply.key() != expected_control_key {
            transaction.fail();
            return Err(OperationError::ForegroundChanged);
        }
        let apply_result = match self.replacer.apply_replacement(&context, &plan) {
            Ok(result) => result,
            Err(error) => {
                if should_guard_clipboard {
                    if let (Some(clipboard), Some(before)) = (self.clipboard, clipboard_before) {
                        let _ = guard_clipboard_from_snapshot(clipboard, before);
                    }
                }
                return Err(OperationError::Platform(error));
            }
        };
        transaction
            .transition_to(OperationState::ReplacementApplied)
            .map_err(OperationError::Transaction)?;
        let clipboard_guard = if should_guard_clipboard {
            self.clipboard.and_then(|clipboard| {
                clipboard_before.map(|before| guard_clipboard_from_snapshot(clipboard, before))
            })
        } else {
            None
        };
        transaction
            .transition_to(OperationState::Verified)
            .map_err(OperationError::Transaction)?;
        transaction
            .transition_to(OperationState::Completed)
            .map_err(OperationError::Transaction)?;

        Ok(OperationOutcome {
            operation_id: transaction.operation_id().to_owned(),
            context,
            plan,
            apply_result,
            clipboard_guard,
            metrics: transaction.metrics(),
        })
    }
}

fn should_guard_clipboard(context: &TextContext) -> bool {
    let Some(binding) = &context.capabilities.method_binding else {
        return true;
    };
    binding.replace_methods.iter().any(|method| {
        matches!(
            method,
            MethodId::TerminalClipboardShortcut
                | MethodId::XtermKeyboardSelection
                | MethodId::WebKeyboardSelection
                | MethodId::ClipboardSelection
                | MethodId::SendInput
        )
    })
}

pub fn guard_clipboard_from_snapshot<B: ClipboardBackend>(
    clipboard: &B,
    before: ClipboardSnapshot,
) -> ClipboardGuardReport {
    std::thread::sleep(Duration::from_millis(80));
    let after_before_restore = clipboard.capture().ok();
    let clipboard_changed = after_before_restore
        .as_ref()
        .map(|after| !clipboard_contents_equal(after, &before))
        .unwrap_or(true);
    let donor_marker_seen = after_before_restore
        .as_ref()
        .is_some_and(contains_hotkeyhandler_marker);

    if !clipboard_changed {
        return ClipboardGuardReport {
            final_snapshot: after_before_restore
                .clone()
                .or_else(|| Some(before.clone())),
            before,
            after_before_restore,
            clipboard_changed,
            restore_ok: true,
            restore_attempts: 0,
            donor_marker_seen,
            last_error: None,
        };
    }

    restore_clipboard_until_stable(clipboard, before, after_before_restore, donor_marker_seen)
}

fn restore_clipboard_until_stable<B: ClipboardBackend>(
    clipboard: &B,
    before: ClipboardSnapshot,
    initial_after: Option<ClipboardSnapshot>,
    initial_donor_marker_seen: bool,
) -> ClipboardGuardReport {
    let started = Instant::now();
    let timeout = Duration::from_millis(2_000);
    let stable_for = Duration::from_millis(250);
    let mut attempts = 0;
    let mut donor_marker_seen = initial_donor_marker_seen;
    let mut last_error = None;
    let mut restored_at = None;
    let mut final_snapshot = initial_after.clone();

    while started.elapsed() < timeout {
        match clipboard.capture() {
            Ok(snapshot) if clipboard_contents_equal(&snapshot, &before) => {
                final_snapshot = Some(snapshot);
                if restored_at
                    .map(|time: Instant| time.elapsed() >= stable_for)
                    .unwrap_or(false)
                {
                    return ClipboardGuardReport {
                        before,
                        after_before_restore: initial_after,
                        clipboard_changed: true,
                        restore_ok: true,
                        restore_attempts: attempts,
                        donor_marker_seen,
                        final_snapshot,
                        last_error,
                    };
                }
            }
            Ok(snapshot) => {
                donor_marker_seen |= contains_hotkeyhandler_marker(&snapshot);
                attempts += 1;
                match clipboard.restore(before.clone()) {
                    Ok(()) => {
                        restored_at = Some(Instant::now());
                        final_snapshot = Some(before.clone());
                    }
                    Err(error) => {
                        last_error = Some(format!("{error:?}"));
                        final_snapshot = Some(snapshot);
                    }
                }
            }
            Err(error) => {
                attempts += 1;
                last_error = Some(format!("capture: {error:?}"));
                if let Err(error) = clipboard.restore(before.clone()) {
                    last_error = Some(format!("restore: {error:?}"));
                } else {
                    restored_at = Some(Instant::now());
                    final_snapshot = Some(before.clone());
                }
            }
        }

        std::thread::sleep(Duration::from_millis(40));
    }

    let restore_ok = final_snapshot
        .as_ref()
        .is_some_and(|snapshot| clipboard_contents_equal(snapshot, &before));
    ClipboardGuardReport {
        before,
        after_before_restore: initial_after,
        clipboard_changed: true,
        restore_ok,
        restore_attempts: attempts,
        donor_marker_seen,
        final_snapshot,
        last_error,
    }
}

fn clipboard_contents_equal(left: &ClipboardSnapshot, right: &ClipboardSnapshot) -> bool {
    left.text == right.text && left.formats == right.formats
}

fn contains_hotkeyhandler_marker(snapshot: &ClipboardSnapshot) -> bool {
    snapshot
        .text
        .as_deref()
        .is_some_and(|text| text.contains("__HKH_"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};
    use stepler_core::{ReplacementPlan, TextContext};
    use stepler_platform::{ClipboardFormatSnapshot, ForegroundControl, PlatformError};

    struct FakeForeground {
        controls: Vec<ForegroundControl>,
    }

    impl ForegroundProvider for FakeForeground {
        fn foreground_control(&self) -> Result<ForegroundControl, PlatformError> {
            self.controls
                .first()
                .cloned()
                .ok_or(PlatformError::ForegroundUnavailable)
        }
    }

    struct ChangingForeground;

    impl ForegroundProvider for ChangingForeground {
        fn foreground_control(&self) -> Result<ForegroundControl, PlatformError> {
            use std::sync::atomic::{AtomicUsize, Ordering};
            static CALLS: AtomicUsize = AtomicUsize::new(0);
            let calls = CALLS.fetch_add(1, Ordering::SeqCst);
            Ok(ForegroundControl {
                app_id: String::from("test"),
                window_id: format!("window-{calls}"),
                control_id: String::from("control"),
            })
        }
    }

    struct FakeContextProvider;

    impl TextContextProvider for FakeContextProvider {
        fn text_context(&self) -> Result<TextContext, PlatformError> {
            Ok(TextContext::new("k.,jdm"))
        }
    }

    struct FakeWebKeyboardContextProvider;

    impl TextContextProvider for FakeWebKeyboardContextProvider {
        fn text_context(&self) -> Result<TextContext, PlatformError> {
            let mut context = TextContext::new("k.,jdm");
            context.capabilities.method_binding = Some(stepler_core::MethodBinding::new(
                MethodId::WebKeyboardSelection,
                vec![MethodId::WebKeyboardSelection],
            ));
            Ok(context)
        }
    }

    struct FakeReplacer;

    impl TextReplacer for FakeReplacer {
        fn apply_replacement(
            &self,
            _context: &TextContext,
            plan: &ReplacementPlan,
        ) -> Result<ApplyReplacementResult, PlatformError> {
            Ok(ApplyReplacementResult {
                applied: true,
                actual_before_text: Some(plan.expected_before_text.clone()),
                actual_after_text: Some(plan.replacement_text.clone()),
                method: String::from("fake"),
                retry_count: 0,
                timings: Vec::new(),
            })
        }
    }

    struct MutatingReplacer<'a> {
        clipboard: &'a FakeClipboard,
    }

    impl TextReplacer for MutatingReplacer<'_> {
        fn apply_replacement(
            &self,
            _context: &TextContext,
            plan: &ReplacementPlan,
        ) -> Result<ApplyReplacementResult, PlatformError> {
            self.clipboard.set_text("__HKH_LEFT_TEXT_MARKER_test");
            Ok(ApplyReplacementResult {
                applied: true,
                actual_before_text: Some(plan.expected_before_text.clone()),
                actual_after_text: Some(plan.replacement_text.clone()),
                method: String::from("fake_mutating"),
                retry_count: 0,
                timings: Vec::new(),
            })
        }
    }

    #[derive(Debug)]
    struct FakeClipboard {
        snapshot: RefCell<ClipboardSnapshot>,
    }

    impl FakeClipboard {
        fn new(text: &str) -> Self {
            Self {
                snapshot: RefCell::new(clipboard_snapshot(text, 1)),
            }
        }

        fn set_text(&self, text: &str) {
            let sequence_number = self.snapshot.borrow().sequence_number.unwrap_or_default() + 1;
            *self.snapshot.borrow_mut() = clipboard_snapshot(text, sequence_number);
        }
    }

    impl ClipboardBackend for FakeClipboard {
        fn capture(&self) -> Result<ClipboardSnapshot, PlatformError> {
            Ok(self.snapshot.borrow().clone())
        }

        fn restore(&self, snapshot: ClipboardSnapshot) -> Result<(), PlatformError> {
            *self.snapshot.borrow_mut() = snapshot;
            Ok(())
        }
    }

    #[derive(Debug)]
    struct SequenceAdvancingClipboard {
        snapshot: RefCell<ClipboardSnapshot>,
        restore_calls: Cell<usize>,
    }

    impl SequenceAdvancingClipboard {
        fn new(snapshot: ClipboardSnapshot) -> Self {
            Self {
                snapshot: RefCell::new(snapshot),
                restore_calls: Cell::new(0),
            }
        }
    }

    impl ClipboardBackend for SequenceAdvancingClipboard {
        fn capture(&self) -> Result<ClipboardSnapshot, PlatformError> {
            Ok(self.snapshot.borrow().clone())
        }

        fn restore(&self, mut snapshot: ClipboardSnapshot) -> Result<(), PlatformError> {
            self.restore_calls.set(self.restore_calls.get() + 1);
            snapshot.sequence_number =
                Some(self.snapshot.borrow().sequence_number.unwrap_or_default() + 1);
            *self.snapshot.borrow_mut() = snapshot;
            Ok(())
        }
    }

    fn clipboard_snapshot(text: &str, sequence_number: u32) -> ClipboardSnapshot {
        ClipboardSnapshot {
            text: Some(String::from(text)),
            sequence_number: Some(sequence_number),
            formats: vec![ClipboardFormatSnapshot {
                format: 13,
                bytes: text.as_bytes().to_vec(),
            }],
        }
    }

    #[test]
    fn clipboard_guard_accepts_restored_contents_with_a_new_sequence_number() {
        let mut before = clipboard_snapshot("original clipboard", 1);
        before.formats.push(ClipboardFormatSnapshot {
            format: 8,
            bytes: vec![1, 2, 3, 4],
        });
        let clipboard = SequenceAdvancingClipboard::new(clipboard_snapshot("temporary text", 2));
        let started = Instant::now();

        let report = guard_clipboard_from_snapshot(&clipboard, before.clone());

        assert!(
            started.elapsed() < Duration::from_secs(1),
            "restored clipboard content should not wait for the guard timeout"
        );
        assert_eq!(report.restore_attempts, 1);
        assert_eq!(clipboard.restore_calls.get(), 1);
        assert!(report.restore_ok);
        let restored = clipboard.capture().unwrap();
        assert_eq!(restored.text, before.text);
        assert_eq!(restored.formats, before.formats);
        assert_ne!(restored.sequence_number, before.sequence_number);
    }

    #[test]
    fn clipboard_guard_ignores_a_sequence_only_change() {
        let before = clipboard_snapshot("original clipboard", 1);
        let mut same_contents = before.clone();
        same_contents.sequence_number = Some(2);
        let clipboard = SequenceAdvancingClipboard::new(same_contents);

        let report = guard_clipboard_from_snapshot(&clipboard, before);

        assert!(!report.clipboard_changed);
        assert!(report.restore_ok);
        assert_eq!(report.restore_attempts, 0);
        assert_eq!(clipboard.restore_calls.get(), 0);
    }

    #[test]
    fn clipboard_guard_restores_changed_format_bytes() {
        let mut before = clipboard_snapshot("original clipboard", 1);
        before.formats.push(ClipboardFormatSnapshot {
            format: 8,
            bytes: vec![1, 2, 3, 4],
        });
        let mut changed = before.clone();
        changed.sequence_number = Some(2);
        changed.formats[1].bytes = vec![4, 3, 2, 1];
        let clipboard = SequenceAdvancingClipboard::new(changed);

        let report = guard_clipboard_from_snapshot(&clipboard, before.clone());

        assert!(report.clipboard_changed);
        assert!(report.restore_ok);
        assert_eq!(report.restore_attempts, 1);
        assert_eq!(clipboard.capture().unwrap().formats, before.formats);
    }

    fn control() -> ForegroundControl {
        ForegroundControl {
            app_id: String::from("test"),
            window_id: String::from("window"),
            control_id: String::from("control"),
        }
    }

    #[test]
    fn runner_applies_pause_operation() {
        let foreground = FakeForeground {
            controls: vec![control()],
        };
        let context_provider = FakeContextProvider;
        let replacer = FakeReplacer;
        let mut runner = OperationRunner::new(&foreground, &context_provider, &replacer);

        let outcome = runner.handle_hotkey(CorrectionMode::Pause).unwrap();

        assert_eq!(
            outcome.apply_result.actual_before_text,
            Some(String::from("k.,jdm"))
        );
        assert_eq!(outcome.plan.range.start, 0);
        assert_eq!(outcome.plan.expected_before_text, "k.,jdm");
        assert_eq!(outcome.context.text_snapshot, "k.,jdm");
        assert_eq!(
            outcome.apply_result.actual_after_text,
            Some(String::from("любовь"))
        );
        assert!(!outcome.metrics.timings.is_empty());
    }

    #[test]
    fn runner_restores_unexpected_clipboard_mutation_in_transaction_layer() {
        let foreground = FakeForeground {
            controls: vec![control()],
        };
        let context_provider = FakeContextProvider;
        let clipboard = FakeClipboard::new("original clipboard");
        let replacer = MutatingReplacer {
            clipboard: &clipboard,
        };
        let mut runner = OperationRunner::new_with_clipboard(
            &foreground,
            &context_provider,
            &replacer,
            &clipboard,
        );

        let outcome = runner.handle_hotkey(CorrectionMode::Pause).unwrap();
        let report = outcome.clipboard_guard.unwrap();

        assert!(report.clipboard_changed);
        assert!(report.restore_ok);
        assert!(report.donor_marker_seen);
        assert_eq!(
            clipboard.capture().unwrap().text.as_deref(),
            Some("original clipboard")
        );
    }

    #[test]
    fn runner_guards_web_keyboard_clipboard_mutation() {
        let foreground = FakeForeground {
            controls: vec![control()],
        };
        let context_provider = FakeWebKeyboardContextProvider;
        let clipboard = FakeClipboard::new("original clipboard");
        let replacer = MutatingReplacer {
            clipboard: &clipboard,
        };
        let mut runner = OperationRunner::new_with_clipboard(
            &foreground,
            &context_provider,
            &replacer,
            &clipboard,
        );

        let outcome = runner.handle_hotkey(CorrectionMode::Pause).unwrap();
        let report = outcome.clipboard_guard.unwrap();

        assert!(report.clipboard_changed);
        assert!(report.restore_ok);
        assert_eq!(
            clipboard.capture().unwrap().text.as_deref(),
            Some("original clipboard")
        );
    }

    #[test]
    fn runner_fails_if_foreground_changes_before_apply() {
        let foreground = ChangingForeground;
        let context_provider = FakeContextProvider;
        let replacer = FakeReplacer;
        let mut runner = OperationRunner::new(&foreground, &context_provider, &replacer);

        let err = runner.handle_hotkey(CorrectionMode::Pause).unwrap_err();

        assert_eq!(err, OperationError::ForegroundChanged);
    }

    #[test]
    fn runner_calls_pre_apply_after_preflight_and_before_replacement() {
        struct OrderedReplacer<'a> {
            order: &'a RefCell<Vec<&'static str>>,
        }

        impl TextReplacer for OrderedReplacer<'_> {
            fn apply_replacement(
                &self,
                _context: &TextContext,
                plan: &ReplacementPlan,
            ) -> Result<ApplyReplacementResult, PlatformError> {
                self.order.borrow_mut().push("apply");
                Ok(ApplyReplacementResult {
                    applied: true,
                    actual_before_text: Some(plan.expected_before_text.clone()),
                    actual_after_text: Some(plan.replacement_text.clone()),
                    method: String::from("ordered"),
                    retry_count: 0,
                    timings: Vec::new(),
                })
            }
        }

        let order = RefCell::new(Vec::new());
        let foreground = FakeForeground {
            controls: vec![control()],
        };
        let context_provider = FakeContextProvider;
        let replacer = OrderedReplacer { order: &order };
        let mut runner = OperationRunner::new(&foreground, &context_provider, &replacer);

        runner
            .handle_hotkey_with_pre_apply(CorrectionMode::Pause, |_, _| {
                order.borrow_mut().push("pre_apply");
            })
            .unwrap();

        assert_eq!(&*order.borrow(), &["pre_apply", "apply"]);
    }

    #[test]
    fn runner_does_not_apply_if_pre_apply_changes_foreground() {
        struct ForegroundSequence {
            calls: RefCell<usize>,
        }

        impl ForegroundProvider for ForegroundSequence {
            fn foreground_control(&self) -> Result<ForegroundControl, PlatformError> {
                let mut calls = self.calls.borrow_mut();
                *calls += 1;
                let mut value = control();
                if *calls >= 3 {
                    value.window_id = String::from("changed-window");
                }
                Ok(value)
            }
        }

        let foreground = ForegroundSequence {
            calls: RefCell::new(0),
        };
        let context_provider = FakeContextProvider;
        let replacer = FakeReplacer;
        let mut runner = OperationRunner::new(&foreground, &context_provider, &replacer);

        let error = runner
            .handle_hotkey_with_pre_apply(CorrectionMode::Pause, |_, _| {})
            .unwrap_err();

        assert_eq!(error, OperationError::ForegroundChanged);
    }
}
