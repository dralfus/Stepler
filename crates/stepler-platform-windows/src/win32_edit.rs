use super::*;

#[cfg(windows)]
#[derive(Debug, Default, Clone, Copy)]
pub(super) struct Win32EditMessagesMethod;

#[cfg(windows)]
impl Win32EditMessagesMethod {
    pub(super) fn probe(&self, target: &ForegroundTarget) -> Option<MethodProbe> {
        is_supported_edit_class(&target.focused_class)
            .then(|| MethodProbe::safe(MethodId::Win32EditMessages, "focused Win32 edit control"))
    }

    pub(super) fn capture(
        &self,
        foreground: isize,
        focused: isize,
        app_class: String,
        focused_class: String,
    ) -> Result<TextContext, PlatformError> {
        let text = window_text(focused)?;
        let (selection_start, selection_end) =
            edit_selection(focused).unwrap_or((text.len(), text.len()));
        let selection_start =
            edit_offset_to_byte_offset(&text, selection_start).unwrap_or(text.len());
        let selection_end = edit_offset_to_byte_offset(&text, selection_end).unwrap_or(text.len());
        let selection_range = if selection_start != selection_end {
            Some(TextRange::new(selection_start, selection_end))
        } else {
            None
        };

        Ok(TextContext {
            app_id: win32_edit_app_id(&app_class, &focused_class),
            window_id: hwnd_id(foreground),
            control_id: hwnd_id(focused),
            text_snapshot: text,
            caret_range: TextRange::caret(selection_end),
            selection_range,
            capabilities: Capabilities {
                can_replace_directly: true,
                can_read_selection: true,
                can_read_caret: true,
                method_binding: Some(MethodBinding::new(
                    MethodId::Win32EditMessages,
                    vec![MethodId::Win32EditMessages],
                )),
            },
        })
    }

    pub(super) fn apply(
        &self,
        context: &TextContext,
        plan: &ReplacementPlan,
    ) -> Result<ApplyReplacementResult, PlatformError> {
        let hwnd =
            parse_hwnd_id(&context.control_id).ok_or(PlatformError::ReplacementUnavailable)?;
        let focused_class = window_class_name(hwnd).unwrap_or_else(|| String::from("unknown"));
        if !is_supported_edit_class(&focused_class) {
            return Err(PlatformError::ReplacementUnavailable);
        }

        let current_text = window_text(hwnd)?;
        let actual_before = slice_by_range(&current_text, plan.range)
            .ok_or(PlatformError::PreflightFailed)?
            .to_owned();

        if actual_before != plan.expected_before_text {
            return Err(PlatformError::PreflightFailed);
        }

        set_edit_selection(hwnd, plan.range.start, plan.range.end)?;
        replace_edit_selection(hwnd, &plan.replacement_text)?;
        let target_caret = win32_adjusted_caret_after_replacement(context, plan);
        set_edit_selection(hwnd, target_caret, target_caret)?;

        let actual_after = window_text(hwnd).ok();
        Ok(ApplyReplacementResult {
            applied: true,
            actual_before_text: Some(actual_before),
            actual_after_text: actual_after,
            method: MethodId::Win32EditMessages.as_str().to_owned(),
        })
    }
}

fn win32_edit_app_id(app_class: &str, focused_class: &str) -> String {
    if app_class.eq_ignore_ascii_case("rctrl_renwnd32")
        && focused_class.to_ascii_lowercase().starts_with("richedit")
    {
        format!("{app_class}/{focused_class}")
    } else {
        app_class.to_owned()
    }
}

pub(super) fn win32_adjusted_caret_after_replacement(
    context: &TextContext,
    plan: &ReplacementPlan,
) -> usize {
    let caret = context.caret_range.end;
    if caret <= plan.range.start {
        return caret;
    }

    if caret <= plan.range.end {
        return plan.range.start + plan.replacement_text.len();
    }

    let removed_len = plan.range.end - plan.range.start;
    caret + plan.replacement_text.len() - removed_len
}
