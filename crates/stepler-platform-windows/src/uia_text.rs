use super::*;

#[cfg(windows)]
#[derive(Debug, Default, Clone, Copy)]
pub(super) struct UiAutomationTextMethod;

#[cfg(windows)]
impl UiAutomationTextMethod {
    pub(super) fn probe(&self, target: &ForegroundTarget) -> Option<MethodProbe> {
        if is_supported_edit_class(&target.focused_class)
            || is_supported_terminal_class(&target.app_class, &target.focused_class)
            || is_word_target(target)
            || target.app_class.eq_ignore_ascii_case("Progman")
            || target.app_class.eq_ignore_ascii_case("WorkerW")
            || target.focused_class.eq_ignore_ascii_case("SysListView32")
        {
            return None;
        }

        Some(MethodProbe::safe(
            MethodId::UiAutomationText,
            "focused UI Automation text/value candidate",
        ))
    }

    pub(super) fn capture(
        &self,
        foreground: isize,
        focused: isize,
        app_class: &str,
        focused_class: &str,
    ) -> Result<TextContext, PlatformError> {
        capture_uia_text_context(foreground, focused, app_class, focused_class, false)
    }

    pub(super) fn apply(
        &self,
        context: &TextContext,
        plan: &ReplacementPlan,
    ) -> Result<ApplyReplacementResult, PlatformError> {
        apply_uia_text_replacement(context, plan, MethodId::UiAutomationText)
    }
}

#[cfg(windows)]
#[derive(Debug, Default, Clone, Copy)]
pub(super) struct UiAutomationEditableTextMethod;

#[cfg(windows)]
impl UiAutomationEditableTextMethod {
    pub(super) fn probe(&self, target: &ForegroundTarget) -> Option<MethodProbe> {
        if is_supported_edit_class(&target.focused_class)
            || is_supported_terminal_class(&target.app_class, &target.focused_class)
            || is_word_target(target)
            || target.app_class.eq_ignore_ascii_case("Progman")
            || target.app_class.eq_ignore_ascii_case("WorkerW")
            || target.focused_class.eq_ignore_ascii_case("SysListView32")
        {
            return None;
        }

        Some(MethodProbe::safe(
            MethodId::UiAutomationEditableText,
            "focused UI Automation editable text candidate",
        ))
    }

    pub(super) fn capture(
        &self,
        foreground: isize,
        focused: isize,
        app_class: &str,
        focused_class: &str,
    ) -> Result<TextContext, PlatformError> {
        let mut context =
            capture_uia_text_context(foreground, focused, app_class, focused_class, true)?;
        context.capabilities.method_binding = Some(MethodBinding::new(
            MethodId::UiAutomationEditableText,
            vec![MethodId::UiAutomationEditableText],
        ));
        Ok(context)
    }

    pub(super) fn apply(
        &self,
        context: &TextContext,
        plan: &ReplacementPlan,
    ) -> Result<ApplyReplacementResult, PlatformError> {
        apply_uia_text_replacement(context, plan, MethodId::UiAutomationEditableText)
    }
}

#[cfg(windows)]
#[derive(Debug, Default, Clone, Copy)]
pub(super) struct UiAutomationDocumentTextMethod;

#[cfg(windows)]
impl UiAutomationDocumentTextMethod {
    pub(super) fn probe(&self, target: &ForegroundTarget) -> Option<MethodProbe> {
        if is_supported_edit_class(&target.focused_class)
            || is_supported_terminal_class(&target.app_class, &target.focused_class)
            || is_word_target(target)
            || target.app_class.eq_ignore_ascii_case("Progman")
            || target.app_class.eq_ignore_ascii_case("WorkerW")
            || target.focused_class.eq_ignore_ascii_case("SysListView32")
        {
            return None;
        }

        Some(MethodProbe::safe(
            MethodId::UiAutomationDocumentText,
            "focused UI Automation document text selection candidate",
        ))
    }

    pub(super) fn capture_with_options(
        &self,
        foreground: isize,
        focused: isize,
        app_class: &str,
        focused_class: &str,
        allow_caret_fallback: bool,
    ) -> Result<TextContext, PlatformError> {
        let env = [
            ("STEPLER_UIA_FOREGROUND_HWND", foreground.to_string()),
            (
                "STEPLER_UIA_DOCUMENT_ALLOW_CARET_FALLBACK",
                if allow_caret_fallback { "1" } else { "0" }.to_owned(),
            ),
        ];
        let output = run_powershell_script(UIA_DOCUMENT_CAPTURE_SCRIPT, &env)?;
        let fields = parse_key_value_lines(&output);
        if fields.get("ok").map(String::as_str) != Some("1") {
            return Err(PlatformError::ReplacementUnavailableReason(
                fields
                    .get("error")
                    .cloned()
                    .unwrap_or_else(|| String::from("uia_document_capture_failed")),
            ));
        }
        let text = fields
            .get("text_b64")
            .and_then(|value| decode_utf16le_base64(value).ok())
            .ok_or_else(|| {
                PlatformError::ReplacementUnavailableReason(String::from(
                    "missing_or_invalid_selection_text",
                ))
            })?;
        if text.trim().is_empty() {
            return Err(PlatformError::ReplacementUnavailableReason(String::from(
                "empty_selection_text",
            )));
        }
        let runtime_id = fields
            .get("runtime_id")
            .cloned()
            .unwrap_or_else(|| String::from("unknown"));
        let is_caret = fields.get("kind").map(String::as_str) == Some("caret");
        let control_prefix = if is_caret { "uia-doc-caret" } else { "uia-doc" };
        let selection_range = if is_caret {
            None
        } else {
            Some(TextRange::new(0, text.len()))
        };
        Ok(TextContext {
            app_id: format!("{app_class}/{focused_class}"),
            window_id: hwnd_id(foreground),
            control_id: format!("{control_prefix}:{}:{}", runtime_id, hwnd_id(focused)),
            text_snapshot: text.clone(),
            caret_range: TextRange::caret(text.len()),
            selection_range,
            capabilities: Capabilities {
                can_replace_directly: true,
                can_read_selection: !is_caret,
                can_read_caret: is_caret,
                method_binding: Some(MethodBinding::new(
                    MethodId::UiAutomationDocumentText,
                    vec![MethodId::UiAutomationDocumentText],
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
        if context.control_id.starts_with("uia-doc-caret:") {
            return self.apply_caret_range(context, plan);
        }

        if plan.range != TextRange::new(0, context.text_snapshot.len())
            || plan.expected_before_text != context.text_snapshot
        {
            return Err(PlatformError::PreflightFailed);
        }
        if env_flag_enabled("STEPLER_UIA_DOCUMENT_STRICT_APPLY", false) {
            return self.apply_strict(context, plan);
        }

        let expected_foreground =
            parse_hwnd_id(&context.window_id).ok_or(PlatformError::PreflightFailed)?;
        if foreground_hwnd()? != expected_foreground {
            return Err(PlatformError::PreflightFailed);
        }

        send_unicode_text(&plan.replacement_text)?;
        std::thread::sleep(Duration::from_millis(5));

        Ok(ApplyReplacementResult {
            applied: true,
            actual_before_text: Some(context.text_snapshot.clone()),
            actual_after_text: Some(plan.replacement_text.clone()),
            method: MethodId::UiAutomationDocumentText.as_str().to_owned(),
            retry_count: 0,
        })
    }

    fn apply_caret_range(
        &self,
        context: &TextContext,
        plan: &ReplacementPlan,
    ) -> Result<ApplyReplacementResult, PlatformError> {
        let actual_before = slice_by_range(&context.text_snapshot, plan.range)
            .ok_or(PlatformError::PreflightFailed)?
            .to_owned();
        if actual_before != plan.expected_before_text {
            return Err(PlatformError::PreflightFailed);
        }
        let runtime_id = parse_uia_document_runtime_id(&context.control_id)
            .ok_or(PlatformError::ReplacementUnavailable)?;
        let left_len_utf16 = context.text_snapshot.encode_utf16().count();
        let start_utf16 = byte_offset_to_utf16(&context.text_snapshot, plan.range.start);
        let end_utf16 = byte_offset_to_utf16(&context.text_snapshot, plan.range.end);
        let select_output = run_powershell_script(
            UIA_DOCUMENT_SELECT_CARET_RANGE_SCRIPT,
            &[
                (
                    "STEPLER_UIA_FOREGROUND_HWND",
                    parse_hwnd_id(&context.window_id)
                        .map(|hwnd| hwnd.to_string())
                        .unwrap_or_default(),
                ),
                ("STEPLER_UIA_RUNTIME_ID", runtime_id),
                (
                    "STEPLER_UIA_EXPECTED_B64",
                    encode_utf16le_base64(&plan.expected_before_text),
                ),
                (
                    "STEPLER_UIA_START_DELTA_UTF16",
                    (start_utf16 as isize - left_len_utf16 as isize).to_string(),
                ),
                (
                    "STEPLER_UIA_END_DELTA_UTF16",
                    (end_utf16 as isize - left_len_utf16 as isize).to_string(),
                ),
            ],
        )?;
        let select_fields = parse_key_value_lines(&select_output);
        if select_fields.get("ok").map(String::as_str) != Some("1") {
            return Err(PlatformError::PreflightFailed);
        }

        let expected_foreground =
            parse_hwnd_id(&context.window_id).ok_or(PlatformError::PreflightFailed)?;
        if foreground_hwnd()? != expected_foreground {
            return Err(PlatformError::PreflightFailed);
        }

        send_unicode_text(&plan.replacement_text)?;
        std::thread::sleep(Duration::from_millis(20));

        Ok(ApplyReplacementResult {
            applied: true,
            actual_before_text: Some(actual_before),
            actual_after_text: Some(plan.replacement_text.clone()),
            method: MethodId::UiAutomationDocumentText.as_str().to_owned(),
            retry_count: 0,
        })
    }

    fn apply_strict(
        &self,
        context: &TextContext,
        plan: &ReplacementPlan,
    ) -> Result<ApplyReplacementResult, PlatformError> {
        let runtime_id = parse_uia_document_runtime_id(&context.control_id)
            .ok_or(PlatformError::ReplacementUnavailable)?;
        let select_output = run_powershell_script(
            UIA_DOCUMENT_SELECT_SCRIPT,
            &[
                (
                    "STEPLER_UIA_FOREGROUND_HWND",
                    parse_hwnd_id(&context.window_id)
                        .map(|hwnd| hwnd.to_string())
                        .unwrap_or_default(),
                ),
                ("STEPLER_UIA_RUNTIME_ID", runtime_id.clone()),
                (
                    "STEPLER_UIA_EXPECTED_B64",
                    encode_utf16le_base64(&plan.expected_before_text),
                ),
            ],
        )?;
        let select_fields = parse_key_value_lines(&select_output);
        if select_fields.get("ok").map(String::as_str) != Some("1") {
            return Err(PlatformError::PreflightFailed);
        }

        send_unicode_text(&plan.replacement_text)?;
        std::thread::sleep(Duration::from_millis(80));

        let verify_output = run_powershell_script(
            UIA_DOCUMENT_VERIFY_SCRIPT,
            &[
                (
                    "STEPLER_UIA_FOREGROUND_HWND",
                    parse_hwnd_id(&context.window_id)
                        .map(|hwnd| hwnd.to_string())
                        .unwrap_or_default(),
                ),
                ("STEPLER_UIA_RUNTIME_ID", runtime_id),
                (
                    "STEPLER_UIA_REPLACEMENT_B64",
                    encode_utf16le_base64(&plan.replacement_text),
                ),
            ],
        )?;
        let verify_fields = parse_key_value_lines(&verify_output);
        if verify_fields.get("ok").map(String::as_str) != Some("1") {
            return Err(PlatformError::PreflightFailed);
        }
        let actual_after = verify_fields
            .get("actual_b64")
            .and_then(|value| decode_utf16le_base64(value).ok());

        Ok(ApplyReplacementResult {
            applied: true,
            actual_before_text: Some(context.text_snapshot.clone()),
            actual_after_text: actual_after,
            method: MethodId::UiAutomationDocumentText.as_str().to_owned(),
            retry_count: 0,
        })
    }
}

#[cfg(windows)]
fn capture_uia_text_context(
    foreground: isize,
    focused: isize,
    app_class: &str,
    focused_class: &str,
    strict_editable: bool,
) -> Result<TextContext, PlatformError> {
    let strict_value = if strict_editable { "1" } else { "0" };
    let output = run_powershell_script(
        UIA_CAPTURE_SCRIPT,
        &[
            ("STEPLER_UIA_FOREGROUND_HWND", foreground.to_string()),
            ("STEPLER_UIA_STRICT_EDITABLE", strict_value.to_owned()),
        ],
    )?;
    let fields = parse_key_value_lines(&output);
    if fields.get("ok").map(String::as_str) != Some("1") {
        return Err(PlatformError::ReplacementUnavailableReason(
            fields
                .get("error")
                .cloned()
                .unwrap_or_else(|| String::from("uia_capture_failed")),
        ));
    }
    if fields.get("can_set_value").map(String::as_str) != Some("1") {
        return Err(PlatformError::ReplacementUnavailableReason(String::from(
            "no_writable_value",
        )));
    }

    let text = fields
        .get("text_b64")
        .and_then(|value| decode_utf16le_base64(value).ok())
        .ok_or_else(|| {
            PlatformError::ReplacementUnavailableReason(String::from("missing_or_invalid_text"))
        })?;
    if text.is_empty() {
        return Err(PlatformError::ReplacementUnavailableReason(String::from(
            "empty_text",
        )));
    }

    let selection_start_utf16 = fields
        .get("selection_start")
        .and_then(|value| value.parse::<usize>().ok());
    let selection_end_utf16 = fields
        .get("selection_end")
        .and_then(|value| value.parse::<usize>().ok());
    let caret_utf16 = fields
        .get("caret")
        .and_then(|value| value.parse::<usize>().ok());

    let selection_range =
        selection_start_utf16
            .zip(selection_end_utf16)
            .and_then(|(start, end)| {
                if start == end {
                    return None;
                }
                Some(TextRange::new(
                    edit_offset_to_byte_offset(&text, start)?,
                    edit_offset_to_byte_offset(&text, end)?,
                ))
            });
    let caret = caret_utf16
        .and_then(|caret| edit_offset_to_byte_offset(&text, caret))
        .or_else(|| selection_range.map(|range| range.end))
        .unwrap_or_else(|| text.len());
    let selection_range = selection_range.or_else(|| {
        let start = selection_start_utf16?;
        let end = selection_end_utf16?;
        if start != end {
            return None;
        }
        let caret = edit_offset_to_byte_offset(&text, start)?;
        (caret != text.len()).then_some(TextRange::caret(caret))
    });
    let user_selection_range = selection_range.and_then(|range| {
        if range.start == range.end {
            None
        } else {
            Some(range)
        }
    });
    let caret = user_selection_range.map(|range| range.end).unwrap_or(caret);
    let runtime_id = fields
        .get("runtime_id")
        .cloned()
        .unwrap_or_else(|| String::from("unknown"));
    let method = if strict_editable {
        MethodId::UiAutomationEditableText
    } else {
        MethodId::UiAutomationText
    };

    Ok(TextContext {
        app_id: format!("{app_class}/{focused_class}"),
        window_id: hwnd_id(foreground),
        control_id: format!("uia:{}:{}", runtime_id, hwnd_id(focused)),
        text_snapshot: text,
        caret_range: TextRange::caret(caret),
        selection_range: user_selection_range,
        capabilities: Capabilities {
            can_replace_directly: true,
            can_read_selection: user_selection_range.is_some(),
            can_read_caret: true,
            method_binding: Some(MethodBinding::new(method, vec![method])),
        },
        telemetry: Default::default(),
    })
}

#[cfg(windows)]
fn apply_uia_text_replacement(
    context: &TextContext,
    plan: &ReplacementPlan,
    method: MethodId,
) -> Result<ApplyReplacementResult, PlatformError> {
    let actual_before = slice_by_range(&context.text_snapshot, plan.range)
        .ok_or(PlatformError::PreflightFailed)?
        .to_owned();
    if actual_before != plan.expected_before_text {
        return Err(PlatformError::PreflightFailed);
    }

    let replacement =
        replace_range_text(&context.text_snapshot, plan.range, &plan.replacement_text)
            .ok_or(PlatformError::PreflightFailed)?;
    let runtime_id =
        parse_uia_runtime_id(&context.control_id).ok_or(PlatformError::ReplacementUnavailable)?;
    let caret_after_utf16 = byte_offset_to_utf16(&context.text_snapshot, plan.range.start)
        + plan.replacement_text.encode_utf16().count();
    let env = [
        (
            "STEPLER_UIA_FOREGROUND_HWND",
            parse_hwnd_id(&context.window_id)
                .map(|hwnd| hwnd.to_string())
                .unwrap_or_default(),
        ),
        ("STEPLER_UIA_RUNTIME_ID", runtime_id),
        (
            "STEPLER_UIA_EXPECTED_B64",
            encode_utf16le_base64(&context.text_snapshot),
        ),
        (
            "STEPLER_UIA_REPLACEMENT_B64",
            encode_utf16le_base64(&replacement),
        ),
        ("STEPLER_UIA_CARET_UTF16", caret_after_utf16.to_string()),
    ];
    let output = run_powershell_script(UIA_APPLY_SCRIPT, &env)?;
    let fields = parse_key_value_lines(&output);
    if fields.get("ok").map(String::as_str) != Some("1") {
        return Err(PlatformError::PreflightFailed);
    }
    let actual_after = fields
        .get("after_b64")
        .and_then(|value| decode_utf16le_base64(value).ok());

    Ok(ApplyReplacementResult {
        applied: true,
        actual_before_text: Some(actual_before),
        actual_after_text: actual_after,
        method: method.as_str().to_owned(),
        retry_count: 0,
    })
}
