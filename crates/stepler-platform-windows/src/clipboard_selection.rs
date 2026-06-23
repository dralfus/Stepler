use super::*;

#[cfg(windows)]
#[derive(Debug, Default, Clone, Copy)]
pub(super) struct ClipboardSelectionMethod;

#[cfg(windows)]
impl ClipboardSelectionMethod {
    pub(super) fn probe(&self, target: &ForegroundTarget) -> Option<MethodProbe> {
        if target.app_class.eq_ignore_ascii_case("Progman")
            || target.app_class.eq_ignore_ascii_case("WorkerW")
            || target.focused_class.eq_ignore_ascii_case("SysListView32")
        {
            return None;
        }

        Some(MethodProbe::risky(
            MethodId::ClipboardSelection,
            "generic selected text clipboard fallback",
        ))
    }

    pub(super) fn capture(
        &self,
        foreground: isize,
        focused: isize,
        app_class: &str,
        focused_class: &str,
    ) -> Result<TextContext, PlatformError> {
        let snapshot = capture_clipboard()?;
        let sequence_before = snapshot.sequence_number;
        send_key_chord(&[VK_CONTROL], VK_C);
        let copied = wait_for_clipboard_selection_text(
            snapshot.text.as_deref(),
            sequence_before,
            Duration::from_millis(700),
        )
        .filter(|text| !text.trim().is_empty())
        .filter(|text| !looks_like_hotkeyhandler_marker(text))
        .ok_or(PlatformError::ReplacementUnavailable);
        let _ = restore_clipboard(snapshot);
        let text = copied?;
        let text_len = text.len();

        Ok(TextContext {
            app_id: format!("{app_class}/{focused_class}"),
            window_id: hwnd_id(foreground),
            control_id: format!("clipboard-selection:{}", hwnd_id(focused)),
            text_snapshot: text,
            caret_range: TextRange::caret(text_len),
            selection_range: Some(TextRange::new(0, text_len)),
            capabilities: Capabilities {
                can_replace_directly: false,
                can_read_selection: true,
                can_read_caret: false,
                method_binding: Some(MethodBinding::new(
                    MethodId::ClipboardSelection,
                    vec![MethodId::ClipboardSelection, MethodId::SendInput],
                )),
            },
        })
    }

    pub(super) fn apply(
        &self,
        context: &TextContext,
        plan: &ReplacementPlan,
    ) -> Result<ApplyReplacementResult, PlatformError> {
        if plan.range != TextRange::new(0, context.text_snapshot.len())
            || plan.expected_before_text != context.text_snapshot
        {
            return Err(PlatformError::PreflightFailed);
        }

        restore_clipboard(clipboard_snapshot_from_text(&plan.replacement_text))?;
        send_key_chord(&[VK_CONTROL], VK_V);
        std::thread::sleep(Duration::from_millis(60));

        Ok(ApplyReplacementResult {
            applied: true,
            actual_before_text: Some(context.text_snapshot.clone()),
            actual_after_text: Some(plan.replacement_text.clone()),
            method: MethodId::ClipboardSelection.as_str().to_owned(),
        })
    }
}
