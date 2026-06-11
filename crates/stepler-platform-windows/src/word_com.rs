use super::*;

#[cfg(windows)]
#[derive(Debug, Default, Clone, Copy)]
pub(super) struct WordComMethod;

#[cfg(windows)]
impl WordComMethod {
    pub(super) fn probe(&self, target: &ForegroundTarget) -> Option<MethodProbe> {
        (is_word_target(target) || is_outlook_target(target))
            .then(|| MethodProbe::safe(MethodId::WordCom, "Word COM object model"))
    }

    pub(super) fn capture(
        &self,
        foreground: isize,
        focused: isize,
        app_class: &str,
        focused_class: &str,
    ) -> Result<TextContext, PlatformError> {
        let process_name = window_process_name(foreground);
        let is_outlook =
            is_outlook_class_or_process(app_class, focused_class, process_name.as_deref());
        append_hotkey_signal_log(&format!(
            "word_com_capture_start app_class={app_class} focused_class={focused_class} outlook={is_outlook}"
        ));
        let output = run_powershell_script(
            if is_outlook {
                OUTLOOK_WORD_CAPTURE_SCRIPT
            } else {
                WORD_CAPTURE_SCRIPT
            },
            &[],
        )?;
        append_hotkey_signal_log("word_com_capture_script_done");
        let fields = parse_key_value_lines(&output);
        if fields.get("ok").map(String::as_str) != Some("1") {
            return Err(PlatformError::ReplacementUnavailableReason(
                fields
                    .get("error")
                    .cloned()
                    .unwrap_or_else(|| String::from("uia_capture_failed")),
            ));
        }
        let text = fields
            .get("text_b64")
            .and_then(|value| decode_utf16le_base64(value).ok())
            .ok_or(PlatformError::ReplacementUnavailable)?;
        let base = fields
            .get("base")
            .and_then(|value| value.parse::<usize>().ok())
            .ok_or(PlatformError::ReplacementUnavailable)?;
        let selection_range = (fields.get("kind").map(String::as_str) == Some("selection"))
            .then(|| TextRange::new(0, text.len()));
        if text.trim().is_empty() {
            return Err(PlatformError::ReplacementUnavailable);
        }
        append_hotkey_signal_log(&format!(
            "word_com_capture_done text_len={} base={} kind={}",
            text.len(),
            base,
            fields.get("kind").map(String::as_str).unwrap_or("")
        ));

        Ok(TextContext {
            app_id: format!("{app_class}/{focused_class}"),
            window_id: hwnd_id(foreground),
            control_id: format!(
                "{}:{}:{}",
                if is_outlook {
                    "outlook-word-com"
                } else {
                    "word-com"
                },
                base,
                hwnd_id(focused)
            ),
            caret_range: TextRange::caret(text.len()),
            selection_range,
            text_snapshot: text,
            capabilities: Capabilities {
                can_replace_directly: true,
                can_read_selection: true,
                can_read_caret: true,
                method_binding: Some(MethodBinding::new(
                    MethodId::WordCom,
                    vec![MethodId::WordCom],
                )),
            },
        })
    }

    pub(super) fn apply(
        &self,
        context: &TextContext,
        plan: &ReplacementPlan,
    ) -> Result<ApplyReplacementResult, PlatformError> {
        let base = parse_word_com_base(&context.control_id)
            .ok_or(PlatformError::ReplacementUnavailable)?;
        let is_outlook = context.control_id.starts_with("outlook-word-com:");
        let actual_before = slice_by_range(&context.text_snapshot, plan.range)
            .ok_or(PlatformError::PreflightFailed)?
            .to_owned();
        if actual_before != plan.expected_before_text {
            return Err(PlatformError::PreflightFailed);
        }

        let abs_start = base + byte_offset_to_utf16(&context.text_snapshot, plan.range.start);
        let abs_end = base + byte_offset_to_utf16(&context.text_snapshot, plan.range.end);
        let original_caret =
            base + byte_offset_to_utf16(&context.text_snapshot, context.caret_range.end);
        let replacement_delta = plan.replacement_text.encode_utf16().count() as isize
            - plan.expected_before_text.encode_utf16().count() as isize;
        let target_caret = if context.caret_range.end >= plan.range.end {
            original_caret.saturating_add_signed(replacement_delta)
        } else {
            original_caret
        };
        let env = [
            ("STEPLER_WORD_START", abs_start.to_string()),
            ("STEPLER_WORD_END", abs_end.to_string()),
            ("STEPLER_WORD_CARET", target_caret.to_string()),
            (
                "STEPLER_WORD_EXPECTED_B64",
                encode_utf16le_base64(&plan.expected_before_text),
            ),
            (
                "STEPLER_WORD_REPLACEMENT_B64",
                encode_utf16le_base64(&plan.replacement_text),
            ),
        ];
        append_hotkey_signal_log(&format!(
            "word_com_apply_start start={abs_start} end={abs_end} caret={target_caret} expected_len={} replacement_len={}",
            plan.expected_before_text.len(),
            plan.replacement_text.len()
        ));
        let output = run_powershell_script(
            if is_outlook {
                OUTLOOK_WORD_APPLY_SCRIPT
            } else {
                WORD_APPLY_SCRIPT
            },
            &env,
        )?;
        append_hotkey_signal_log("word_com_apply_script_done");
        let fields = parse_key_value_lines(&output);
        if fields.get("ok").map(String::as_str) != Some("1") {
            return Err(PlatformError::PreflightFailed);
        }
        let actual_after = fields
            .get("after_b64")
            .and_then(|value| decode_utf16le_base64(value).ok());
        append_hotkey_signal_log("word_com_apply_done");

        Ok(ApplyReplacementResult {
            applied: true,
            actual_before_text: Some(actual_before),
            actual_after_text: actual_after,
            method: MethodId::WordCom.as_str().to_owned(),
        })
    }
}
