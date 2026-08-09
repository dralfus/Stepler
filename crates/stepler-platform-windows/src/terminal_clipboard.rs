use super::*;

#[cfg(windows)]
#[derive(Debug, Default, Clone, Copy)]
pub(super) struct TerminalClipboardShortcutMethod;

#[cfg(windows)]
impl TerminalClipboardShortcutMethod {
    pub(super) fn probe(&self, target: &ForegroundTarget) -> Option<MethodProbe> {
        is_supported_terminal_class(&target.app_class, &target.focused_class).then(|| {
            MethodProbe::risky(
                MethodId::TerminalClipboardShortcut,
                "terminal clipboard shortcut fallback",
            )
        })
    }

    pub(super) fn capture(
        &self,
        foreground: isize,
        focused: isize,
        app_class: &str,
        focused_class: &str,
    ) -> Result<TextContext, PlatformError> {
        let captured = read_terminal_left_text()?;
        let left_text = captured.text;
        let text_len = left_text.len();
        Ok(TextContext {
            app_id: format!("{app_class}/{focused_class}"),
            window_id: hwnd_id(foreground),
            control_id: format!(
                "terminal:{}:{}",
                hwnd_id(focused),
                captured.selection_kind.id()
            ),
            text_snapshot: left_text,
            caret_range: TextRange::caret(text_len),
            selection_range: None,
            capabilities: Capabilities {
                can_replace_directly: false,
                can_read_selection: false,
                can_read_caret: false,
                method_binding: Some(MethodBinding::new(
                    MethodId::TerminalClipboardShortcut,
                    vec![MethodId::TerminalClipboardShortcut],
                )),
            },
            telemetry: Default::default(),
        })
    }

    pub(super) fn apply(
        &self,
        context: &TextContext,
        plan: &ReplacementPlan,
    ) -> Result<ApplyReplacementResult, PlatformError> {
        let actual_before = slice_by_range(&context.text_snapshot, plan.range);
        if actual_before != Some(plan.expected_before_text.as_str()) {
            return Err(PlatformError::PreflightFailed);
        }

        let replacement =
            replace_range_text(&context.text_snapshot, plan.range, &plan.replacement_text)
                .ok_or(PlatformError::PreflightFailed)?;
        match TerminalSelectionKind::from_control_id(&context.control_id) {
            TerminalSelectionKind::LeftOfCaret => send_key_chord(&[VK_LSHIFT], VK_HOME),
            TerminalSelectionKind::PreviousWord => {
                send_key_chord(&[VK_CONTROL, VK_LSHIFT], VK_LEFT);
            }
        }
        std::thread::sleep(Duration::from_millis(40));
        restore_clipboard(clipboard_snapshot_from_text(&replacement))?;
        send_terminal_shortcut_with_english_layout(&[VK_CONTROL, VK_SHIFT], VK_V);
        std::thread::sleep(Duration::from_millis(60));

        Ok(ApplyReplacementResult {
            applied: true,
            actual_before_text: Some(context.text_snapshot.clone()),
            actual_after_text: Some(replacement),
            method: MethodId::TerminalClipboardShortcut.as_str().to_owned(),
            retry_count: 0,
            timings: Vec::new(),
        })
    }
}
