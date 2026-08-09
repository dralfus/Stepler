use super::*;

#[cfg(windows)]
#[derive(Debug, Default, Clone, Copy)]
pub(super) struct ConsoleBufferMethod;

#[cfg(windows)]
impl ConsoleBufferMethod {
    pub(super) fn probe(&self, target: &ForegroundTarget) -> Option<MethodProbe> {
        (target.app_class == "ConsoleWindowClass")
            .then(|| MethodProbe::safe(MethodId::ConsoleBuffer, "classic console buffer"))
    }

    pub(super) fn capture(
        &self,
        foreground: isize,
        focused: isize,
        app_class: &str,
        focused_class: &str,
    ) -> Result<TextContext, PlatformError> {
        let input = read_console_input_text(foreground)?;
        let text_len = input.len();
        Ok(TextContext {
            app_id: format!("{app_class}/{focused_class}"),
            window_id: hwnd_id(foreground),
            control_id: format!("terminal-console:{}", hwnd_id(focused)),
            text_snapshot: input,
            caret_range: TextRange::caret(text_len),
            selection_range: None,
            capabilities: Capabilities {
                can_replace_directly: false,
                can_read_selection: false,
                can_read_caret: false,
                method_binding: Some(MethodBinding::new(
                    MethodId::ConsoleBuffer,
                    vec![MethodId::ConsoleBuffer],
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
        let foreground = foreground_hwnd()?;
        let current_text = read_console_input_text(foreground)?;
        let actual_before = slice_by_range(&current_text, plan.range);
        if current_text != context.text_snapshot
            || actual_before != Some(plan.expected_before_text.as_str())
        {
            return Err(PlatformError::PreflightFailed);
        }

        let replacement = replace_range_text(&current_text, plan.range, &plan.replacement_text)
            .ok_or(PlatformError::PreflightFailed)?;
        clear_console_input_line(foreground)?;
        send_unicode_text(&replacement)?;
        std::thread::sleep(Duration::from_millis(60));

        Ok(ApplyReplacementResult {
            applied: true,
            actual_before_text: Some(current_text),
            actual_after_text: Some(replacement),
            method: MethodId::ConsoleBuffer.as_str().to_owned(),
            retry_count: 0,
            timings: Vec::new(),
        })
    }
}

#[cfg(windows)]
fn clear_console_input_line(hwnd: isize) -> Result<(), PlatformError> {
    for _ in 0..3 {
        send_key_virtual(VK_ESCAPE);
        std::thread::sleep(Duration::from_millis(45));
        match read_console_input_text(hwnd) {
            Err(PlatformError::ReplacementUnavailable) => return Ok(()),
            Ok(input) if input.trim().is_empty() => return Ok(()),
            Ok(_) => {}
            Err(error) => return Err(error),
        }
    }

    Err(PlatformError::PreflightFailed)
}
