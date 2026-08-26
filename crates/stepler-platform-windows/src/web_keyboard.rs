use super::*;

#[cfg(windows)]
#[derive(Debug, Default, Clone, Copy)]
pub(super) struct WebKeyboardSelectionMethod;

#[cfg(windows)]
#[derive(Debug, Default, Clone, Copy)]
pub(super) struct XtermKeyboardSelectionMethod;

#[cfg(windows)]
impl XtermKeyboardSelectionMethod {
    pub(super) fn probe(&self, target: &ForegroundTarget) -> Option<MethodProbe> {
        let enabled_for_browser =
            env_flag_enabled("STEPLER_ENABLE_XTERM_KEYBOARD_SELECTION", false)
                && is_browser_like_target(target)
                && focused_is_xterm_textarea();
        let enabled_for_terminal = is_xterm_terminal_target(target)
            || has_terminal_app_marker(target)
                && is_supported_terminal_class(&target.app_class, &target.focused_class);
        if !enabled_for_browser && !enabled_for_terminal {
            return None;
        }

        let mut probe = MethodProbe::safe(
            MethodId::XtermKeyboardSelection,
            "xterm textarea keyboard selection with terminal copy/paste shortcuts",
        );
        probe.requires_clipboard = true;
        Some(probe)
    }

    pub(super) fn capture(
        &self,
        foreground: isize,
        focused: isize,
        app_class: &str,
        focused_class: &str,
    ) -> Result<TextContext, PlatformError> {
        let mut timings = vec![TelemetryTiming {
            phase: String::from("retry"),
            elapsed_ms: 0,
        }];
        append_hotkey_signal_log(&format!(
            "xterm_capture start app={app_class:?} focused={focused_class:?} marker={}",
            has_active_terminal_app_marker()
        ));
        if foreground_hwnd()? != foreground {
            append_hotkey_signal_log("xterm_capture fail=foreground_changed");
            return Err(PlatformError::PreflightFailed);
        }
        if has_active_terminal_app_marker()
            && is_supported_terminal_class(app_class, focused_class)
            && !focused_is_xterm_textarea()
        {
            append_hotkey_signal_log("xterm_capture fail=terminal_app_no_safe_text_capture");
            return Err(PlatformError::ReplacementUnavailableReason(String::from(
                "terminal_app_no_safe_text_capture",
            )));
        }

        let capture_started = Instant::now();
        let snapshot = capture_clipboard_text_only()?;
        add_telemetry_timing(
            &mut timings,
            "capture",
            capture_started.elapsed().as_millis(),
        );
        let verify_started = Instant::now();
        let selected = copy_selected_text_checked_with_chord(
            &snapshot,
            &[VK_CONTROL, VK_SHIFT],
            VK_C,
            Duration::from_millis(260),
        )
        .filter(|text| !text.trim().is_empty())
        .filter(|text| !looks_like_hotkeyhandler_marker(text));
        add_telemetry_timing(&mut timings, "verify", verify_started.elapsed().as_millis());
        let restore_started = Instant::now();
        let _ = restore_clipboard_text_only(&snapshot);
        add_telemetry_timing(
            &mut timings,
            "clipboard_restore",
            restore_started.elapsed().as_millis(),
        );
        if let Some(text) = selected {
            let text_len = text.len();
            append_hotkey_signal_log(&format!(
                "xterm_capture branch=selected len={}",
                text.encode_utf16().count()
            ));
            return Ok(TextContext {
                app_id: format!("{app_class}/{focused_class}"),
                window_id: hwnd_id(foreground),
                control_id: format!("xterm-selection:{}", hwnd_id(focused)),
                text_snapshot: text,
                caret_range: TextRange::caret(text_len),
                selection_range: Some(TextRange::new(0, text_len)),
                capabilities: Capabilities {
                    can_replace_directly: false,
                    can_read_selection: true,
                    can_read_caret: true,
                    method_binding: Some(MethodBinding::new(
                        MethodId::XtermKeyboardSelection,
                        vec![MethodId::XtermKeyboardSelection],
                    )),
                },
                telemetry: ContextTelemetry {
                    timings,
                    ..Default::default()
                },
            });
        }

        let capture_started = Instant::now();
        let snapshot = capture_clipboard_text_only()?;
        add_telemetry_timing(
            &mut timings,
            "capture",
            capture_started.elapsed().as_millis(),
        );
        send_key_chord(&[VK_LSHIFT], VK_HOME);
        std::thread::sleep(Duration::from_millis(40));
        let verify_started = Instant::now();
        let copied = copy_selected_text_checked_with_chord(
            &snapshot,
            &[VK_CONTROL, VK_SHIFT],
            VK_C,
            Duration::from_millis(360),
        )
        .filter(|text| !text.trim().is_empty())
        .filter(|text| !looks_like_hotkeyhandler_marker(text));
        add_telemetry_timing(&mut timings, "verify", verify_started.elapsed().as_millis());
        send_key(VK_RIGHT);
        let restore_started = Instant::now();
        let _ = restore_clipboard_text_only(&snapshot);
        add_telemetry_timing(
            &mut timings,
            "clipboard_restore",
            restore_started.elapsed().as_millis(),
        );

        if let Some(text) = copied {
            let text_len = text.len();
            append_hotkey_signal_log(&format!(
                "xterm_capture branch=line_left len={}",
                text.encode_utf16().count()
            ));
            return Ok(TextContext {
                app_id: format!("{app_class}/{focused_class}"),
                window_id: hwnd_id(foreground),
                control_id: format!("xterm-line-selection:{}", hwnd_id(focused)),
                text_snapshot: text,
                caret_range: TextRange::caret(text_len),
                selection_range: None,
                capabilities: Capabilities {
                    can_replace_directly: false,
                    can_read_selection: false,
                    can_read_caret: true,
                    method_binding: Some(MethodBinding::new(
                        MethodId::XtermKeyboardSelection,
                        vec![MethodId::XtermKeyboardSelection],
                    )),
                },
                telemetry: ContextTelemetry {
                    timings,
                    ..Default::default()
                },
            });
        }

        append_hotkey_signal_log("xterm_capture fail=empty_after_selected_and_line_left");
        Err(PlatformError::ReplacementUnavailableReason(String::from(
            "xterm_keyboard_capture_empty",
        )))
    }

    pub(super) fn apply(
        &self,
        context: &TextContext,
        plan: &ReplacementPlan,
    ) -> Result<ApplyReplacementResult, PlatformError> {
        let mut timings = vec![TelemetryTiming {
            phase: String::from("retry"),
            elapsed_ms: 0,
        }];
        let actual_before = slice_by_range(&context.text_snapshot, plan.range)
            .ok_or(PlatformError::PreflightFailed)?
            .to_owned();
        if actual_before != plan.expected_before_text {
            return Err(PlatformError::PreflightFailed);
        }

        let replace_entire_context =
            context.selection_range.is_none() && plan.range.end != context.text_snapshot.len();
        let replacement_text = if replace_entire_context {
            replace_range_text(&context.text_snapshot, plan.range, &plan.replacement_text)
                .ok_or(PlatformError::PreflightFailed)?
        } else {
            plan.replacement_text.clone()
        };
        let expected_selection = if replace_entire_context {
            context.text_snapshot.as_str()
        } else {
            actual_before.as_str()
        };

        let capture_started = Instant::now();
        let snapshot = capture_clipboard_text_only()?;
        add_telemetry_timing(
            &mut timings,
            "capture",
            capture_started.elapsed().as_millis(),
        );
        if context.selection_range.is_none() {
            if context.control_id.starts_with("xterm-line-selection:") && replace_entire_context {
                send_key_chord(&[VK_LSHIFT], VK_HOME);
            } else {
                select_left_utf16_units(expected_selection.encode_utf16().count())?;
            }
            std::thread::sleep(Duration::from_millis(35));
            let verify_started = Instant::now();
            let selected = copy_selected_text_checked_with_chord(
                &snapshot,
                &[VK_CONTROL, VK_SHIFT],
                VK_C,
                Duration::from_millis(360),
            );
            add_telemetry_timing(&mut timings, "verify", verify_started.elapsed().as_millis());
            if selected.as_deref() != Some(expected_selection) {
                restore_web_line_left_context_caret();
                let restore_started = Instant::now();
                let _ = restore_clipboard_text_only(&snapshot);
                add_telemetry_timing(
                    &mut timings,
                    "clipboard_restore",
                    restore_started.elapsed().as_millis(),
                );
                return Err(PlatformError::ReplacementUnavailableReason(format!(
                    "xterm_keyboard_preflight expected={} actual={}",
                    preview_for_error(expected_selection, 40),
                    preview_for_error(selected.as_deref().unwrap_or("<none>"), 40)
                )));
            }
        }

        let apply_started = Instant::now();
        restore_clipboard(clipboard_snapshot_from_text(&replacement_text))?;
        send_key_chord_virtual(&[VK_CONTROL, VK_SHIFT], VK_V);
        std::thread::sleep(Duration::from_millis(80));
        add_telemetry_timing(&mut timings, "apply", apply_started.elapsed().as_millis());
        let restore_started = Instant::now();
        let _ = restore_clipboard_text_only(&snapshot);
        add_telemetry_timing(
            &mut timings,
            "clipboard_restore",
            restore_started.elapsed().as_millis(),
        );

        Ok(ApplyReplacementResult {
            applied: true,
            actual_before_text: Some(actual_before),
            actual_after_text: Some(replacement_text),
            method: MethodId::XtermKeyboardSelection.as_str().to_owned(),
            retry_count: 0,
            timings,
        })
    }
}

#[cfg(windows)]
fn add_telemetry_timing(timings: &mut Vec<TelemetryTiming>, phase: &str, elapsed_ms: u128) {
    if let Some(existing) = timings.iter_mut().find(|timing| timing.phase == phase) {
        existing.elapsed_ms += elapsed_ms;
    } else {
        timings.push(TelemetryTiming {
            phase: phase.to_owned(),
            elapsed_ms,
        });
    }
}

#[cfg(all(test, windows))]
mod telemetry_tests {
    use super::*;

    #[test]
    fn xterm_phase_timings_stay_separate_and_aggregate_per_phase() {
        let mut timings = Vec::new();
        for (phase, elapsed_ms) in [
            ("capture", 4),
            ("apply", 5),
            ("verify", 6),
            ("retry", 0),
            ("clipboard_restore", 2),
            ("capture", 3),
        ] {
            add_telemetry_timing(&mut timings, phase, elapsed_ms);
        }

        assert_eq!(
            timings,
            vec![
                TelemetryTiming {
                    phase: String::from("capture"),
                    elapsed_ms: 7,
                },
                TelemetryTiming {
                    phase: String::from("apply"),
                    elapsed_ms: 5,
                },
                TelemetryTiming {
                    phase: String::from("verify"),
                    elapsed_ms: 6,
                },
                TelemetryTiming {
                    phase: String::from("retry"),
                    elapsed_ms: 0,
                },
                TelemetryTiming {
                    phase: String::from("clipboard_restore"),
                    elapsed_ms: 2,
                },
            ]
        );
    }
}

#[cfg(windows)]
impl WebKeyboardSelectionMethod {
    pub(super) fn probe(&self, target: &ForegroundTarget) -> Option<MethodProbe> {
        if !is_web_keyboard_technical_target(target) {
            return None;
        }

        let mut probe = MethodProbe::safe(
            MethodId::WebKeyboardSelection,
            "editor keyboard selection with clipboard preflight",
        );
        probe.requires_clipboard = true;
        Some(probe)
    }

    pub(super) fn capture(
        &self,
        foreground: isize,
        focused: isize,
        app_class: &str,
        focused_class: &str,
    ) -> Result<TextContext, PlatformError> {
        let expected_foreground = foreground;
        if foreground_hwnd()? != expected_foreground {
            return Err(PlatformError::PreflightFailed);
        }
        if foreground_is_codex_embedded_terminal(foreground) {
            return Err(PlatformError::ReplacementUnavailableReason(String::from(
                "embedded_terminal_xterm_unsupported",
            )));
        }
        let foreground_title = window_title(foreground).unwrap_or_default();
        let profile = web_keyboard_profile_for_surface(foreground_surface_kind(
            foreground,
            app_class,
            focused_class,
        ));
        let effective_profile =
            web_keyboard_effective_profile_for_title(profile, &foreground_title);
        let timing = web_keyboard_timing_profile(effective_profile);
        let fast_profile = web_keyboard_profile_is_fast(effective_profile);
        let rocket_fast = web_keyboard_profile_is_rocket(effective_profile);
        let allow_captured_left = web_keyboard_allows_captured_left_for_title(&foreground_title);

        for attempt in 0..2 {
            let snapshot = capture_web_keyboard_clipboard(fast_profile, timing.clipboard_timeout)?;
            let scrolllock_mode = active_correction_mode_is_scrolllock();

            let selected = copy_web_keyboard_selected_text(
                &snapshot,
                timing.selected_timeout,
                fast_profile,
                timing.clipboard_timeout,
            )
            .filter(|text| is_plausible_web_selected_text(text))
            .filter(|text| !looks_like_hotkeyhandler_marker(text));
            if let Some(text) = selected {
                append_hotkey_signal_log(&format!(
                    "web_keyboard_capture branch=selected len={}",
                    text.len()
                ));
                let _ = restore_clipboard_text_only(&snapshot);
                return Ok(web_keyboard_context(
                    app_class,
                    focused_class,
                    foreground,
                    focused,
                    "web-keyboard-selection-selected",
                    text,
                    true,
                ));
            }

            if scrolllock_mode {
                let prefer_line_context = web_keyboard_prefers_line_context_for_scrolllock(
                    foreground_surface_kind(foreground, app_class, focused_class),
                );
                if !prefer_line_context && allow_captured_left {
                    select_web_left_context();
                    let copied_raw = copy_web_keyboard_selected_text(
                        &snapshot,
                        timing.short_context_timeout,
                        fast_profile,
                        timing.clipboard_timeout,
                    );
                    let copied = copied_raw
                        .filter(|text| is_plausible_web_left_context_text(text))
                        .filter(|text| !looks_like_hotkeyhandler_marker(text))
                        .filter(|text| {
                            web_keyboard_allows_captured_left_text_for_surface(
                                app_class,
                                focused_class,
                                text,
                            )
                        });
                    send_key(VK_RIGHT);
                    let _ = restore_web_keyboard_clipboard(
                        &snapshot,
                        fast_profile,
                        timing.clipboard_timeout,
                    );

                    if let Some(text) = copied {
                        append_hotkey_signal_log(&format!(
                            "web_keyboard_capture branch=scrolllock_left len={}",
                            text.len()
                        ));
                        let context = web_keyboard_context(
                            app_class,
                            focused_class,
                            foreground,
                            focused,
                            "web-keyboard-captured-left-selection",
                            text,
                            false,
                        );
                        if stepler_core::build_replacement_plan(
                            &context,
                            stepler_core::CorrectionMode::ScrollLock,
                        )
                        .is_ok()
                        {
                            return Ok(context);
                        }
                        append_hotkey_signal_log(
                            "web_keyboard_capture branch=scrolllock_left skipped=no_plan",
                        );
                    }
                } else if !prefer_line_context {
                    append_hotkey_signal_log(
                        "web_keyboard_capture branch=scrolllock_left skipped=title_policy",
                    );
                } else {
                    append_hotkey_signal_log(
                        "web_keyboard_capture branch=scrolllock_left skipped=surface_line_contract",
                    );
                }

                if !prefer_line_context && web_ctrl_a_fallback_enabled() {
                    let snapshot =
                        capture_web_keyboard_clipboard(fast_profile, timing.clipboard_timeout)?;
                    select_web_all_context();
                    let copied_raw = copy_web_keyboard_selected_text(
                        &snapshot,
                        timing.short_context_timeout,
                        fast_profile,
                        timing.clipboard_timeout,
                    );
                    let copied = copied_raw
                        .filter(|text| is_plausible_web_field_text(text))
                        .filter(|text| !looks_like_hotkeyhandler_marker(text));
                    send_key(VK_RIGHT);
                    let _ = restore_web_keyboard_clipboard(
                        &snapshot,
                        fast_profile,
                        timing.clipboard_timeout,
                    );

                    if let Some(text) = copied {
                        append_hotkey_signal_log(&format!(
                            "web_keyboard_capture branch=scrolllock_field_all len={}",
                            text.len()
                        ));
                        return Ok(web_keyboard_context(
                            app_class,
                            focused_class,
                            foreground,
                            focused,
                            "web-keyboard-field-selection",
                            text,
                            true,
                        ));
                    }
                }

                let snapshot =
                    capture_web_keyboard_clipboard(fast_profile, timing.clipboard_timeout)?;
                select_web_line_left_context();
                let copied_raw = copy_web_keyboard_selected_text(
                    &snapshot,
                    timing.short_context_timeout,
                    fast_profile,
                    timing.clipboard_timeout,
                );
                let copied = copied_raw
                    .filter(|text| is_plausible_web_left_context_text(text))
                    .filter(|text| !looks_like_hotkeyhandler_marker(text));
                send_key(VK_RIGHT);
                let _ = restore_web_keyboard_clipboard(
                    &snapshot,
                    fast_profile,
                    timing.clipboard_timeout,
                );

                if let Some(text) = copied {
                    append_hotkey_signal_log(&format!(
                        "web_keyboard_capture branch=scrolllock_line_left len={}",
                        text.len()
                    ));
                    return Ok(web_keyboard_context(
                        app_class,
                        focused_class,
                        foreground,
                        focused,
                        web_keyboard_control_prefix(
                            "web-keyboard-line-selection",
                            effective_profile,
                        ),
                        text,
                        false,
                    ));
                }

                release_modifier_keys();
                std::thread::sleep(Duration::from_millis(50));
                continue;
            }

            select_web_line_left_context();
            let copied_raw = copy_web_keyboard_selected_text(
                &snapshot,
                timing.line_context_timeout,
                fast_profile,
                timing.clipboard_timeout,
            );
            let copied = copied_raw
                .filter(|text| is_plausible_web_left_context_text(text))
                .filter(|text| !looks_like_hotkeyhandler_marker(text));
            if copied.is_none() || !rocket_fast {
                restore_web_line_left_context_caret();
            }
            let _ =
                restore_web_keyboard_clipboard(&snapshot, fast_profile, timing.clipboard_timeout);

            if let Some(text) = copied {
                append_hotkey_signal_log(&format!(
                    "web_keyboard_capture branch=line_left len={}",
                    text.len()
                ));
                return Ok(web_keyboard_context(
                    app_class,
                    focused_class,
                    foreground,
                    focused,
                    if rocket_fast {
                        "web-keyboard-rocket-active-line-selection"
                    } else {
                        web_keyboard_control_prefix(
                            "web-keyboard-line-selection",
                            effective_profile,
                        )
                    },
                    text,
                    false,
                ));
            }

            if attempt == 0 {
                release_modifier_keys();
                std::thread::sleep(timing.retry_pause);
                if !allow_captured_left {
                    append_hotkey_signal_log(
                        "web_keyboard_capture branch=left_retry skipped=title_policy",
                    );
                    continue;
                }
                let snapshot =
                    capture_web_keyboard_clipboard(fast_profile, timing.clipboard_timeout)?;
                select_web_left_context();
                let copied_raw = copy_web_keyboard_selected_text(
                    &snapshot,
                    timing.line_context_timeout,
                    fast_profile,
                    timing.clipboard_timeout,
                );
                let copied = copied_raw
                    .filter(|text| is_plausible_web_left_context_text(text))
                    .filter(|text| !looks_like_hotkeyhandler_marker(text))
                    .filter(|text| {
                        web_keyboard_allows_captured_left_text_for_surface(
                            app_class,
                            focused_class,
                            text,
                        )
                    });
                send_key(VK_RIGHT);
                let _ = restore_web_keyboard_clipboard(
                    &snapshot,
                    fast_profile,
                    timing.clipboard_timeout,
                );

                if let Some(text) = copied {
                    append_hotkey_signal_log(&format!(
                        "web_keyboard_capture branch=left_retry len={}",
                        text.len()
                    ));
                    return Ok(web_keyboard_context(
                        app_class,
                        focused_class,
                        foreground,
                        focused,
                        "web-keyboard-captured-left-selection",
                        text,
                        false,
                    ));
                }
            }

            if web_ctrl_a_fallback_enabled() {
                let snapshot =
                    capture_web_keyboard_clipboard(fast_profile, timing.clipboard_timeout)?;
                select_web_all_context();
                let copied_raw = copy_web_keyboard_selected_text(
                    &snapshot,
                    Duration::from_millis(320),
                    fast_profile,
                    timing.clipboard_timeout,
                );
                let copied = copied_raw
                    .filter(|text| is_plausible_web_field_text(text))
                    .filter(|text| !looks_like_hotkeyhandler_marker(text));
                send_key(VK_RIGHT);
                let _ = restore_web_keyboard_clipboard(
                    &snapshot,
                    fast_profile,
                    timing.clipboard_timeout,
                );

                if let Some(text) = copied {
                    append_hotkey_signal_log(&format!(
                        "web_keyboard_capture branch=field_all len={}",
                        text.len()
                    ));
                    return Ok(web_keyboard_context(
                        app_class,
                        focused_class,
                        foreground,
                        focused,
                        "web-keyboard-field-selection",
                        text,
                        true,
                    ));
                }
            }

            if attempt == 0 {
                release_modifier_keys();
                std::thread::sleep(timing.attempt_pause);
            }
        }

        Err(PlatformError::ReplacementUnavailableReason(String::from(
            "web_keyboard_capture_empty_after_left_context_retry",
        )))
    }

    pub(super) fn apply(
        &self,
        context: &TextContext,
        plan: &ReplacementPlan,
    ) -> Result<ApplyReplacementResult, PlatformError> {
        let actual_before = slice_by_range(&context.text_snapshot, plan.range)
            .ok_or_else(|| {
                PlatformError::ReplacementUnavailableReason(String::from(
                    "web_keyboard_preflight invalid_range",
                ))
            })?
            .to_owned();
        if actual_before != plan.expected_before_text {
            return Err(PlatformError::ReplacementUnavailableReason(format!(
                "web_keyboard_preflight plan_expected={} actual_range={}",
                preview_for_error(&plan.expected_before_text, 40),
                preview_for_error(&actual_before, 40)
            )));
        }
        let expected_foreground = parse_hwnd_id(&context.window_id).ok_or_else(|| {
            PlatformError::ReplacementUnavailableReason(String::from(
                "web_keyboard_preflight invalid_hwnd",
            ))
        })?;
        if foreground_hwnd()? != expected_foreground {
            return Err(PlatformError::ReplacementUnavailableReason(String::from(
                "web_keyboard_preflight foreground_changed",
            )));
        }

        if web_keyboard_rocket_active_line_context(&context.control_id) {
            let replacement_text =
                replace_range_text(&context.text_snapshot, plan.range, &plan.replacement_text)
                    .ok_or(PlatformError::PreflightFailed)?;
            let snapshot = capture_clipboard_text_only()?;
            restore_clipboard(clipboard_snapshot_from_text(&replacement_text))?;
            send_key_chord_virtual(&[VK_CONTROL], VK_V);
            std::thread::sleep(Duration::from_millis(30));
            let _ = restore_clipboard_text_only(&snapshot);
            append_hotkey_signal_log(&format!(
                "web_keyboard_rocket_active_line_paste expected_len={} replacement_len={}",
                context.text_snapshot.len(),
                replacement_text.len()
            ));
            return Ok(ApplyReplacementResult {
                applied: true,
                actual_before_text: Some(actual_before),
                actual_after_text: Some(replacement_text),
                method: MethodId::WebKeyboardSelection.as_str().to_owned(),
                retry_count: 0,
                timings: Vec::new(),
            });
        }

        if context.selection_range.is_some() {
            preflight_web_keyboard_selected_context(&actual_before)?;
            send_unicode_text(&plan.replacement_text)?;
            return Ok(ApplyReplacementResult {
                applied: true,
                actual_before_text: Some(actual_before),
                actual_after_text: Some(plan.replacement_text.clone()),
                method: MethodId::WebKeyboardSelection.as_str().to_owned(),
                retry_count: 0,
                timings: Vec::new(),
            });
        }

        if web_keyboard_captured_left_context(&context.control_id) {
            let snapshot = capture_clipboard_text_only()?;
            let mut replacement = None;
            let mut replacement_retry_count = 0;
            for attempt in 0..2 {
                select_web_left_context();
                std::thread::sleep(if attempt == 0 {
                    Duration::from_millis(50)
                } else {
                    Duration::from_millis(80)
                });
                let selected = copy_selected_text_checked(&snapshot, Duration::from_millis(650));
                let Some(selected_text) = selected else {
                    if attempt == 0 {
                        append_hotkey_signal_log(
                            "web_keyboard_captured_left_preflight_retry reason=copy_empty",
                        );
                        send_key(VK_RIGHT);
                        release_modifier_keys();
                        std::thread::sleep(Duration::from_millis(80));
                        continue;
                    }
                    send_key(VK_RIGHT);
                    let _ = restore_clipboard_text_only(&snapshot);
                    return Err(PlatformError::ReplacementUnavailableReason(format!(
                        "web_keyboard_captured_left_preflight expected={} actual=<none>",
                        preview_for_error(&context.text_snapshot, 40)
                    )));
                };
                if !web_keyboard_allows_captured_left_apply_selection(context, &selected_text) {
                    send_key(VK_RIGHT);
                    let _ = restore_clipboard_text_only(&snapshot);
                    return Err(PlatformError::ReplacementUnavailableReason(format!(
                        "web_keyboard_captured_left_preflight multiline_selection expected={} actual={}",
                        preview_for_error(&context.text_snapshot, 40),
                        preview_for_error(&selected_text, 40)
                    )));
                }

                match web_keyboard_captured_left_replacement_text(context, plan, &selected_text) {
                    Ok(result) => {
                        replacement_retry_count = attempt;
                        replacement = Some(result);
                        break;
                    }
                    Err(_)
                        if attempt == 0
                            && web_keyboard_captured_left_should_retry_selection(
                                context,
                                &selected_text,
                            ) =>
                    {
                        append_hotkey_signal_log(&format!(
                            "web_keyboard_captured_left_preflight_retry reason=short_suffix expected={} actual={}",
                            preview_for_error(&context.text_snapshot, 40),
                            preview_for_error(&selected_text, 40)
                        ));
                        send_key(VK_RIGHT);
                        release_modifier_keys();
                        std::thread::sleep(Duration::from_millis(80));
                    }
                    Err(error) => {
                        send_key(VK_RIGHT);
                        let _ = restore_clipboard_text_only(&snapshot);
                        return Err(error);
                    }
                }
            }

            let Some((replacement_text, actual_before_text)) = replacement else {
                send_key(VK_RIGHT);
                let _ = restore_clipboard_text_only(&snapshot);
                return Err(PlatformError::ReplacementUnavailable);
            };

            let _ = restore_clipboard_text_only(&snapshot);
            std::thread::sleep(Duration::from_millis(20));
            send_unicode_text(&replacement_text)?;
            append_hotkey_signal_log(&format!(
                "web_keyboard_captured_left_sendinput expected_len={} replacement_len={}",
                context.text_snapshot.len(),
                replacement_text.len()
            ));
            return Ok(ApplyReplacementResult {
                applied: true,
                actual_before_text: Some(actual_before_text),
                actual_after_text: Some(replacement_text),
                method: MethodId::WebKeyboardSelection.as_str().to_owned(),
                retry_count: replacement_retry_count as u32,
                timings: Vec::new(),
            });
        }

        if is_web_keyboard_line_context(&context.control_id) && is_sticky_notes_context(context) {
            let snapshot = capture_clipboard_text_only()?;
            select_web_line_left_context();
            std::thread::sleep(Duration::from_millis(50));
            let selected = copy_selected_text_checked(&snapshot, Duration::from_millis(650));
            let Some(selected_text) = selected else {
                send_key(VK_RIGHT);
                let _ = restore_clipboard_text_only(&snapshot);
                return Err(PlatformError::ReplacementUnavailableReason(format!(
                    "web_keyboard_sticky_line_preflight expected={} actual=<none>",
                    preview_for_error(&context.text_snapshot, 40)
                )));
            };

            let (replacement_text, actual_before_text) =
                match web_keyboard_sticky_line_replacement_text(context, plan, &selected_text) {
                    Ok(replacement) => replacement,
                    Err(error) => {
                        send_key(VK_RIGHT);
                        let _ = restore_clipboard_text_only(&snapshot);
                        return Err(error);
                    }
                };

            let _ = restore_clipboard_text_only(&snapshot);
            std::thread::sleep(Duration::from_millis(20));
            send_unicode_text(&replacement_text)?;
            append_hotkey_signal_log(&format!(
                "web_keyboard_sticky_line_sendinput selected_len={} replacement_len={}",
                selected_text.len(),
                replacement_text.len()
            ));
            return Ok(ApplyReplacementResult {
                applied: true,
                actual_before_text: Some(actual_before_text),
                actual_after_text: Some(replacement_text),
                method: MethodId::WebKeyboardSelection.as_str().to_owned(),
                retry_count: 0,
                timings: Vec::new(),
            });
        }

        let replace_entire_context = plan.range.end != context.text_snapshot.len();
        let replacement_text = if replace_entire_context {
            let mut rebuilt = String::with_capacity(
                context.text_snapshot.len() - actual_before.len() + plan.replacement_text.len(),
            );
            rebuilt.push_str(&context.text_snapshot[..plan.range.start]);
            rebuilt.push_str(&plan.replacement_text);
            rebuilt.push_str(&context.text_snapshot[plan.range.end..]);
            rebuilt
        } else {
            plan.replacement_text.clone()
        };
        let expected_selection = if replace_entire_context {
            context.text_snapshot.as_str()
        } else {
            actual_before.as_str()
        };

        let foreground_title = window_title(expected_foreground).unwrap_or_default();
        let allow_fast_line_apply =
            web_keyboard_allows_fast_line_apply_for_title(&foreground_title)
                || !is_web_keyboard_line_context(&context.control_id);
        let allow_relaxed_line_preflight =
            web_keyboard_allows_relaxed_line_preflight_for_title(&foreground_title);

        if web_keyboard_uses_precise_range_apply(
            &foreground_title,
            &context.control_id,
            replace_entire_context,
        ) {
            return apply_web_keyboard_precise_range(context, plan, &actual_before);
        }

        if web_keyboard_fast_context(&context.control_id) && allow_fast_line_apply {
            select_left_utf16_units(expected_selection.encode_utf16().count())?;
            std::thread::sleep(Duration::from_millis(10));
            if web_keyboard_rocket_fast_context(&context.control_id) {
                let text_to_send = replacement_text.clone();
                let snapshot = capture_clipboard_text_only()?;
                restore_clipboard(clipboard_snapshot_from_text(&text_to_send))?;
                send_key_chord_virtual(&[VK_CONTROL], VK_V);
                std::thread::sleep(Duration::from_millis(30));
                let _ = restore_clipboard_text_only(&snapshot);
                append_hotkey_signal_log(&format!(
                    "web_keyboard_rocket_fast_paste expected_len={} replacement_len={}",
                    expected_selection.len(),
                    text_to_send.len()
                ));
            } else {
                let selected_prefix_to_preserve = preflight_fast_web_selection(expected_selection)?;
                let text_to_send = if selected_prefix_to_preserve.is_empty() {
                    replacement_text.clone()
                } else {
                    format!("{selected_prefix_to_preserve}{replacement_text}")
                };
                send_unicode_text(&text_to_send)?;
                append_hotkey_signal_log(&format!(
                    "web_keyboard_fast_apply expected_len={} replacement_len={}",
                    expected_selection.len(),
                    text_to_send.len()
                ));
                return Ok(ApplyReplacementResult {
                    applied: true,
                    actual_before_text: Some(actual_before),
                    actual_after_text: Some(text_to_send),
                    method: MethodId::WebKeyboardSelection.as_str().to_owned(),
                    retry_count: 0,
                    timings: Vec::new(),
                });
            }
            append_hotkey_signal_log(&format!(
                "web_keyboard_fast_apply expected_len={} replacement_len={}",
                expected_selection.len(),
                replacement_text.len()
            ));
            return Ok(ApplyReplacementResult {
                applied: true,
                actual_before_text: Some(actual_before),
                actual_after_text: Some(replacement_text),
                method: MethodId::WebKeyboardSelection.as_str().to_owned(),
                retry_count: 0,
                timings: Vec::new(),
            });
        }
        if web_keyboard_fast_context(&context.control_id) {
            append_hotkey_signal_log("web_keyboard_fast_apply skipped=line_title_policy");
        }

        let snapshot = capture_clipboard_text_only()?;
        let use_precise_left_selection = is_web_keyboard_line_context(&context.control_id)
            || context.control_id.starts_with("web-keyboard-selection:");
        let mut selected = if use_precise_left_selection {
            select_left_utf16_units(expected_selection.encode_utf16().count())?;
            std::thread::sleep(Duration::from_millis(50));
            copy_selected_text_checked(&snapshot, Duration::from_millis(650))
        } else {
            select_left_utf16_units(expected_selection.encode_utf16().count())?;
            std::thread::sleep(Duration::from_millis(35));
            copy_selected_text_checked(&snapshot, Duration::from_millis(450))
        };
        if selected.is_none() && use_precise_left_selection {
            release_modifier_keys();
            std::thread::sleep(Duration::from_millis(80));
            select_left_utf16_units(expected_selection.encode_utf16().count())?;
            std::thread::sleep(Duration::from_millis(50));
            selected = copy_selected_text_checked(&snapshot, Duration::from_millis(650));
        }
        if selected.is_none() && use_precise_left_selection && allow_relaxed_line_preflight {
            append_hotkey_signal_log(&format!(
                "web_keyboard_preflight_relaxed expected={} reason=copy_empty_after_precise_selection",
                preview_for_error(expected_selection, 40)
            ));
            selected = Some(expected_selection.to_owned());
        } else if selected.is_none() && use_precise_left_selection {
            append_hotkey_signal_log(&format!(
                "web_keyboard_preflight_relaxed skipped=title_policy expected={}",
                preview_for_error(expected_selection, 40)
            ));
        }
        if !replace_entire_context && selected.as_deref() != Some(expected_selection) {
            selected = extend_web_selection_to_expected_prefix(
                selected,
                &actual_before,
                &snapshot,
                Duration::from_millis(450),
            );
        }
        let selected_prefix_to_preserve =
            if !replace_entire_context && selected.as_deref() != Some(expected_selection) {
                selected
                    .as_deref()
                    .and_then(|selected| shifted_web_selection_prefix(selected, expected_selection))
                    .unwrap_or_default()
                    .to_owned()
            } else {
                String::new()
            };
        if selected.as_deref() != Some(expected_selection) {
            if selected_prefix_to_preserve.is_empty() {
                send_key(VK_RIGHT);
                let _ = restore_clipboard_text_only(&snapshot);
                return Err(PlatformError::ReplacementUnavailableReason(format!(
                    "web_keyboard_preflight expected={} actual={}",
                    preview_for_error(&actual_before, 40),
                    preview_for_error(selected.as_deref().unwrap_or("<none>"), 40)
                )));
            }
        }

        let _ = restore_clipboard_text_only(&snapshot);
        std::thread::sleep(Duration::from_millis(20));
        let text_to_send = if selected_prefix_to_preserve.is_empty() {
            replacement_text.clone()
        } else {
            format!("{selected_prefix_to_preserve}{replacement_text}")
        };
        send_unicode_text(&text_to_send)?;

        Ok(ApplyReplacementResult {
            applied: true,
            actual_before_text: Some(actual_before),
            actual_after_text: Some(text_to_send),
            method: MethodId::WebKeyboardSelection.as_str().to_owned(),
            retry_count: 0,
            timings: Vec::new(),
        })
    }
}

#[cfg(windows)]
fn preflight_web_keyboard_selected_context(expected_selection: &str) -> Result<(), PlatformError> {
    let snapshot = capture_clipboard_text_only()?;
    let selected = copy_selected_text_checked(&snapshot, Duration::from_millis(260));
    if selected.as_deref() == Some(expected_selection) {
        let _ = restore_clipboard_text_only(&snapshot);
        return Ok(());
    }

    if selected.is_none() {
        select_left_utf16_units(expected_selection.encode_utf16().count())?;
        std::thread::sleep(Duration::from_millis(25));
        let selected_after_recovery =
            copy_selected_text_checked(&snapshot, Duration::from_millis(360));
        let _ = restore_clipboard_text_only(&snapshot);
        if selected_after_recovery.as_deref() == Some(expected_selection) {
            append_hotkey_signal_log(&format!(
                "web_keyboard_selected_preflight_recovered expected_len={}",
                expected_selection.len()
            ));
            return Ok(());
        }

        send_key(VK_RIGHT);
        return Err(PlatformError::ReplacementUnavailableReason(format!(
            "web_keyboard_selected_preflight expected={} actual={}",
            preview_for_error(expected_selection, 40),
            preview_for_error(selected_after_recovery.as_deref().unwrap_or("<none>"), 40)
        )));
    }

    let _ = restore_clipboard_text_only(&snapshot);
    Err(PlatformError::ReplacementUnavailableReason(format!(
        "web_keyboard_selected_preflight expected={} actual={}",
        preview_for_error(expected_selection, 40),
        preview_for_error(selected.as_deref().unwrap_or("<none>"), 40)
    )))
}

#[cfg(windows)]
fn apply_web_keyboard_precise_range(
    context: &TextContext,
    plan: &ReplacementPlan,
    actual_before: &str,
) -> Result<ApplyReplacementResult, PlatformError> {
    let suffix = slice_by_range(
        &context.text_snapshot,
        TextRange::new(plan.range.end, context.text_snapshot.len()),
    )
    .ok_or(PlatformError::PreflightFailed)?;
    move_left_utf16_units(suffix.encode_utf16().count())?;
    select_left_utf16_units(actual_before.encode_utf16().count())?;
    std::thread::sleep(Duration::from_millis(50));

    let snapshot = capture_clipboard_text_only()?;
    let selected = copy_selected_text_checked(&snapshot, Duration::from_millis(650));
    if selected
        .as_deref()
        .is_some_and(|selected| selected != actual_before)
    {
        restore_precise_range_caret(&suffix);
        let _ = restore_clipboard_text_only(&snapshot);
        return Err(PlatformError::ReplacementUnavailableReason(format!(
            "web_keyboard_precise_range_preflight expected={} actual={}",
            preview_for_error(actual_before, 40),
            preview_for_error(selected.as_deref().unwrap_or("<none>"), 40)
        )));
    }
    if selected.is_none() {
        append_hotkey_signal_log(&format!(
            "web_keyboard_precise_range_preflight_unverified expected={}",
            preview_for_error(actual_before, 40)
        ));
    }

    let _ = restore_clipboard_text_only(&snapshot);
    std::thread::sleep(Duration::from_millis(20));
    send_unicode_text(&plan.replacement_text)?;
    std::thread::sleep(Duration::from_millis(30));
    move_right_utf16_units(suffix.encode_utf16().count())?;
    append_hotkey_signal_log(&format!(
        "web_keyboard_precise_range_sendinput suffix_len={} expected_len={} replacement_len={} verified={}",
        suffix.len(),
        actual_before.len(),
        plan.replacement_text.len(),
        selected.is_some()
    ));

    Ok(ApplyReplacementResult {
        applied: true,
        actual_before_text: Some(actual_before.to_owned()),
        actual_after_text: Some(plan.replacement_text.clone()),
        method: MethodId::WebKeyboardSelection.as_str().to_owned(),
        retry_count: 0,
        timings: Vec::new(),
    })
}

#[cfg(windows)]
fn restore_precise_range_caret(suffix: &str) {
    send_key(VK_RIGHT);
    release_modifier_keys();
    let _ = move_right_utf16_units(suffix.encode_utf16().count());
}

#[cfg(windows)]
fn move_left_utf16_units(count: usize) -> Result<(), PlatformError> {
    if count == 0 {
        return Ok(());
    }
    if count > 512 {
        return Err(PlatformError::ReplacementUnavailableReason(format!(
            "web_keyboard_move_left_too_long count={count}"
        )));
    }

    let mut events = Vec::with_capacity(count * 2);
    for _ in 0..count {
        events.push(KeyboardInputEvent::new(
            VK_LEFT,
            false,
            KeyboardInputMode::ScanCode,
        ));
        events.push(KeyboardInputEvent::new(
            VK_LEFT,
            true,
            KeyboardInputMode::ScanCode,
        ));
    }
    if !send_keyboard_input(&events) {
        return Err(PlatformError::ReplacementUnavailableReason(String::from(
            "web_keyboard_move_left_send_input_failed",
        )));
    }
    std::thread::sleep(Duration::from_millis(20));
    Ok(())
}

#[cfg(windows)]
fn move_right_utf16_units(count: usize) -> Result<(), PlatformError> {
    if count == 0 {
        return Ok(());
    }
    if count > 512 {
        return Err(PlatformError::ReplacementUnavailableReason(format!(
            "web_keyboard_move_right_too_long count={count}"
        )));
    }

    let mut events = Vec::with_capacity(count * 2);
    for _ in 0..count {
        events.push(KeyboardInputEvent::new(
            VK_RIGHT,
            false,
            KeyboardInputMode::ScanCode,
        ));
        events.push(KeyboardInputEvent::new(
            VK_RIGHT,
            true,
            KeyboardInputMode::ScanCode,
        ));
    }
    if !send_keyboard_input(&events) {
        return Err(PlatformError::ReplacementUnavailableReason(String::from(
            "web_keyboard_move_right_send_input_failed",
        )));
    }
    std::thread::sleep(Duration::from_millis(20));
    Ok(())
}

#[cfg(windows)]
fn capture_web_keyboard_clipboard(
    fast_profile: bool,
    clipboard_timeout: Duration,
) -> Result<ClipboardSnapshot, PlatformError> {
    if fast_profile {
        capture_clipboard_text_only_with_timeout(clipboard_timeout)
    } else {
        capture_clipboard_text_only()
    }
}

#[cfg(windows)]
fn restore_web_keyboard_clipboard(
    snapshot: &ClipboardSnapshot,
    fast_profile: bool,
    clipboard_timeout: Duration,
) -> Result<(), PlatformError> {
    if fast_profile {
        restore_clipboard_text_only_with_timeout(snapshot, clipboard_timeout)
    } else {
        restore_clipboard_text_only(snapshot)
    }
}

#[cfg(windows)]
fn restore_web_line_left_context_caret() {
    send_key_virtual(VK_RIGHT);
    release_modifier_keys();
}

#[cfg(windows)]
fn preflight_fast_web_selection(expected_selection: &str) -> Result<String, PlatformError> {
    let clipboard_timeout = Duration::from_millis(180);
    let snapshot = capture_clipboard_text_only_with_timeout(clipboard_timeout)?;
    let mut selected = copy_selected_text_checked_with_chord_and_clipboard_timeout(
        &snapshot,
        &[VK_CONTROL],
        VK_INSERT,
        Duration::from_millis(220),
        clipboard_timeout,
    );

    if let Some(prefix) =
        accepted_fast_web_selection_prefix(selected.as_deref(), expected_selection)
    {
        let _ = restore_clipboard_text_only_with_timeout(&snapshot, clipboard_timeout);
        return Ok(prefix);
    }

    if selected.as_deref() != Some(expected_selection) {
        append_hotkey_signal_log(&format!(
            "web_keyboard_fast_preflight_retry expected={} actual={}",
            preview_for_error(expected_selection, 40),
            preview_for_error(selected.as_deref().unwrap_or("<none>"), 40)
        ));
        send_key(VK_RIGHT);
        std::thread::sleep(Duration::from_millis(35));
        select_left_utf16_units(expected_selection.encode_utf16().count())?;
        std::thread::sleep(Duration::from_millis(25));
        selected = copy_selected_text_checked_with_chord_and_clipboard_timeout(
            &snapshot,
            &[VK_CONTROL],
            VK_INSERT,
            Duration::from_millis(260),
            clipboard_timeout,
        );
    }

    if accepted_fast_web_selection_prefix(selected.as_deref(), expected_selection).is_none()
        && selected
            .as_deref()
            .is_some_and(|text| !text.is_empty() && expected_selection.ends_with(text))
    {
        append_hotkey_signal_log(&format!(
            "web_keyboard_fast_preflight_extend expected={} actual={}",
            preview_for_error(expected_selection, 40),
            preview_for_error(selected.as_deref().unwrap_or("<none>"), 40)
        ));
        selected = extend_web_selection_to_expected_prefix(
            selected,
            expected_selection,
            &snapshot,
            Duration::from_millis(260),
        );
    }

    let _ = restore_clipboard_text_only_with_timeout(&snapshot, clipboard_timeout);
    if let Some(prefix) =
        accepted_fast_web_selection_prefix(selected.as_deref(), expected_selection)
    {
        return Ok(prefix);
    }

    send_key(VK_RIGHT);
    Err(PlatformError::ReplacementUnavailableReason(format!(
        "web_keyboard_fast_preflight expected={} actual={}",
        preview_for_error(expected_selection, 40),
        preview_for_error(selected.as_deref().unwrap_or("<none>"), 40)
    )))
}

#[cfg(windows)]
pub(super) fn accepted_fast_web_selection_prefix(
    selected: Option<&str>,
    expected_selection: &str,
) -> Option<String> {
    let selected = selected?;
    if selected == expected_selection {
        return Some(String::new());
    }

    shifted_web_selection_prefix(selected, expected_selection).map(str::to_owned)
}

#[cfg(windows)]
fn copy_web_keyboard_selected_text(
    snapshot: &ClipboardSnapshot,
    timeout: Duration,
    fast_profile: bool,
    clipboard_timeout: Duration,
) -> Option<String> {
    if fast_profile {
        copy_selected_text_checked_with_chord_and_clipboard_timeout(
            snapshot,
            &[VK_CONTROL],
            VK_INSERT,
            timeout,
            clipboard_timeout,
        )
    } else {
        copy_selected_text_checked_with_chord(snapshot, &[VK_CONTROL], VK_INSERT, timeout)
    }
}

#[cfg(windows)]
pub(super) fn web_keyboard_context(
    app_class: &str,
    focused_class: &str,
    foreground: isize,
    focused: isize,
    control_prefix: &str,
    text: String,
    has_selection: bool,
) -> TextContext {
    let text = normalize_web_keyboard_context_text(control_prefix, text);
    let text_len = text.len();
    TextContext {
        app_id: format!("{app_class}/{focused_class}"),
        window_id: hwnd_id(foreground),
        control_id: format!("{control_prefix}:{}", hwnd_id(focused)),
        text_snapshot: text,
        caret_range: TextRange::caret(text_len),
        selection_range: has_selection.then_some(TextRange::new(0, text_len)),
        capabilities: Capabilities {
            can_replace_directly: false,
            can_read_selection: has_selection,
            can_read_caret: true,
            method_binding: Some(MethodBinding::new(
                MethodId::WebKeyboardSelection,
                vec![MethodId::WebKeyboardSelection],
            )),
        },
        telemetry: Default::default(),
    }
}

#[cfg(windows)]
fn normalize_web_keyboard_context_text(control_prefix: &str, text: String) -> String {
    if control_prefix != "web-keyboard-captured-left-selection" {
        return text;
    }

    let (core, _) = split_trailing_line_breaks(&text);
    core.to_owned()
}

#[cfg(windows)]
pub(super) fn web_keyboard_fast_context(control_id: &str) -> bool {
    control_id.starts_with("web-keyboard-fast-selection:")
        || control_id.starts_with("web-keyboard-fast-line-selection:")
        || web_keyboard_rocket_fast_context(control_id)
}

#[cfg(windows)]
pub(super) fn web_keyboard_rocket_fast_context(control_id: &str) -> bool {
    control_id.starts_with("web-keyboard-rocket-fast-selection:")
        || control_id.starts_with("web-keyboard-rocket-fast-line-selection:")
}

#[cfg(windows)]
pub(super) fn web_keyboard_rocket_active_line_context(control_id: &str) -> bool {
    control_id.starts_with("web-keyboard-rocket-active-line-selection:")
}

#[cfg(windows)]
pub(super) fn web_keyboard_captured_left_context(control_id: &str) -> bool {
    control_id.starts_with("web-keyboard-captured-left-selection:")
}

#[cfg(windows)]
pub(super) fn web_keyboard_allows_captured_left_for_title(title: &str) -> bool {
    !web_keyboard_is_confluence_like_title(title)
}

#[cfg(windows)]
pub(super) fn web_keyboard_allows_captured_left_text_for_surface(
    app_class: &str,
    focused_class: &str,
    text: &str,
) -> bool {
    !web_keyboard_text_has_line_break(text)
        || (app_class == "ApplicationFrameWindow"
            && focused_class == "Windows.UI.Input.InputSite.WindowClass")
}

#[cfg(windows)]
pub(super) fn web_keyboard_allows_captured_left_apply_selection(
    context: &TextContext,
    selected_text: &str,
) -> bool {
    !web_keyboard_text_has_line_break(selected_text)
        || is_sticky_notes_context(context)
        || web_keyboard_wrapped_list_tail_selection(context, selected_text).is_some()
}

#[cfg(windows)]
pub(super) fn web_keyboard_allows_fast_line_apply_for_title(title: &str) -> bool {
    !web_keyboard_is_confluence_like_title(title)
}

#[cfg(windows)]
pub(super) fn web_keyboard_allows_relaxed_line_preflight_for_title(title: &str) -> bool {
    !web_keyboard_is_confluence_like_title(title) && !web_keyboard_is_jira_like_title(title)
}

#[cfg(windows)]
pub(super) fn web_keyboard_effective_profile_for_title(
    profile: WebKeyboardProfile,
    title: &str,
) -> WebKeyboardProfile {
    if profile == WebKeyboardProfile::Fast && web_keyboard_is_jira_like_title(title) {
        WebKeyboardProfile::Standard
    } else {
        profile
    }
}

#[cfg(windows)]
fn web_keyboard_is_jira_like_title(title: &str) -> bool {
    title.to_ascii_lowercase().contains("jira")
}

#[cfg(windows)]
pub(super) fn web_keyboard_uses_precise_range_apply(
    title: &str,
    control_id: &str,
    replace_entire_context: bool,
) -> bool {
    replace_entire_context
        && is_web_keyboard_line_context(control_id)
        && web_keyboard_is_confluence_like_title(title)
}

#[cfg(windows)]
fn web_keyboard_is_confluence_like_title(title: &str) -> bool {
    let normalized = title.to_ascii_lowercase();
    normalized.contains("confluence") || normalized.contains("gs-labs wiki")
}

#[cfg(windows)]
fn web_keyboard_text_has_line_break(text: &str) -> bool {
    text.contains('\r') || text.contains('\n')
}

#[cfg(windows)]
pub(super) fn web_keyboard_captured_left_replacement_text(
    context: &TextContext,
    plan: &ReplacementPlan,
    selected_text: &str,
) -> Result<(String, String), PlatformError> {
    if let Some(wrapped_tail) = web_keyboard_wrapped_list_tail_selection(context, selected_text) {
        let tail_context = TextContext {
            app_id: context.app_id.clone(),
            window_id: context.window_id.clone(),
            control_id: context.control_id.clone(),
            text_snapshot: wrapped_tail.to_owned(),
            caret_range: TextRange::caret(wrapped_tail.len()),
            selection_range: None,
            capabilities: context.capabilities.clone(),
            telemetry: context.telemetry.clone(),
        };
        let tail_plan =
            stepler_core::build_replacement_plan(&tail_context, correction_mode_from_plan(plan))
                .map_err(|error| {
                    PlatformError::ReplacementUnavailableReason(format!(
                        "web_keyboard_captured_left_replan_failed error={error:?} selected={}",
                        preview_for_error(wrapped_tail, 40)
                    ))
                })?;
        let replacement_text =
            replace_range_text(wrapped_tail, tail_plan.range, &tail_plan.replacement_text)
                .ok_or(PlatformError::PreflightFailed)?;

        return Ok((replacement_text, tail_plan.expected_before_text));
    }

    let (selected_core, selected_suffix) = split_trailing_line_breaks(selected_text);
    let (context_core, _) = split_trailing_line_breaks(&context.text_snapshot);

    if selected_core == context_core {
        let core_plan = if context_core == context.text_snapshot {
            plan.clone()
        } else {
            let core_context = TextContext {
                app_id: context.app_id.clone(),
                window_id: context.window_id.clone(),
                control_id: context.control_id.clone(),
                text_snapshot: context_core.to_owned(),
                caret_range: TextRange::caret(context_core.len()),
                selection_range: None,
                capabilities: context.capabilities.clone(),
                telemetry: context.telemetry.clone(),
            };
            stepler_core::build_replacement_plan(&core_context, correction_mode_from_plan(plan))
                .map_err(|error| {
                    PlatformError::ReplacementUnavailableReason(format!(
                        "web_keyboard_captured_left_replan_failed error={error:?} selected={}",
                        preview_for_error(context_core, 40)
                    ))
                })?
        };
        let mut replacement_text =
            replace_range_text(context_core, core_plan.range, &core_plan.replacement_text)
                .ok_or(PlatformError::PreflightFailed)?;
        replacement_text.push_str(selected_suffix);
        return Ok((replacement_text, core_plan.expected_before_text));
    }

    if context_core.is_empty() || !selected_core.ends_with(context_core) {
        return Err(PlatformError::ReplacementUnavailableReason(format!(
            "web_keyboard_captured_left_preflight expected={} actual={}",
            preview_for_error(context_core, 40),
            preview_for_error(selected_text, 40)
        )));
    }

    let prefix_len = selected_core.len() - context_core.len();
    let selected_prefix = &selected_core[..prefix_len];

    if correction_mode_from_plan(plan) == CorrectionMode::Pause {
        if !selected_prefix.chars().all(char::is_whitespace) {
            return Err(PlatformError::ReplacementUnavailableReason(format!(
                "web_keyboard_captured_left_preflight non_whitespace_prefix expected={} actual={}",
                preview_for_error(context_core, 40),
                preview_for_error(selected_text, 40)
            )));
        }
        let replacement_suffix =
            replace_range_text(context_core, plan.range, &plan.replacement_text)
                .ok_or(PlatformError::PreflightFailed)?;
        let replacement_text = format!("{selected_prefix}{replacement_suffix}{selected_suffix}");

        return Ok((replacement_text, plan.expected_before_text.clone()));
    }

    if selected_prefix.contains('\r') || selected_prefix.contains('\n') {
        return Err(PlatformError::ReplacementUnavailableReason(format!(
            "web_keyboard_captured_left_preflight multiline_prefix expected={} actual={}",
            preview_for_error(context_core, 40),
            preview_for_error(selected_text, 40)
        )));
    }

    let selected_context = TextContext {
        app_id: context.app_id.clone(),
        window_id: context.window_id.clone(),
        control_id: context.control_id.clone(),
        text_snapshot: selected_core.to_owned(),
        caret_range: TextRange::caret(selected_core.len()),
        selection_range: None,
        capabilities: context.capabilities.clone(),
        telemetry: context.telemetry.clone(),
    };
    let selected_plan =
        stepler_core::build_replacement_plan(&selected_context, CorrectionMode::ScrollLock)
            .map_err(|error| {
                PlatformError::ReplacementUnavailableReason(format!(
                    "web_keyboard_captured_left_replan_failed error={error:?} selected={}",
                    preview_for_error(selected_core, 40)
                ))
            })?;
    let replacement_text = replace_range_text(
        selected_core,
        selected_plan.range,
        &selected_plan.replacement_text,
    )
    .ok_or(PlatformError::PreflightFailed)?;
    let replacement_text = format!("{replacement_text}{selected_suffix}");

    Ok((replacement_text, selected_plan.expected_before_text))
}

#[cfg(windows)]
fn web_keyboard_wrapped_list_tail_selection<'a>(
    context: &TextContext,
    selected_text: &'a str,
) -> Option<&'a str> {
    if is_sticky_notes_context(context) || !web_keyboard_text_has_line_break(selected_text) {
        return None;
    }

    let (context_core, _) = split_trailing_line_breaks(&context.text_snapshot);
    if context_core.is_empty() || web_keyboard_text_has_line_break(context_core) {
        return None;
    }

    let tail = selected_text
        .rsplit(['\r', '\n'])
        .find(|line| !line.trim().is_empty())?
        .trim_start();
    if tail.is_empty() || !context_core.ends_with(tail) {
        return None;
    }

    let prefix = &context_core[..context_core.len() - tail.len()];
    if web_keyboard_is_wrapped_list_marker_prefix(prefix) {
        Some(tail)
    } else {
        None
    }
}

#[cfg(windows)]
fn web_keyboard_is_wrapped_list_marker_prefix(prefix: &str) -> bool {
    let prefix = prefix.trim_start();
    let mut chars = prefix.chars().peekable();

    if chars.next_if(|ch| matches!(ch, '-' | '*' | '•')).is_some() {
        return chars.next().is_some_and(char::is_whitespace) && chars.all(char::is_whitespace);
    }

    let mut saw_digit = false;
    while chars.next_if(|ch| ch.is_ascii_digit()).is_some() {
        saw_digit = true;
    }

    saw_digit
        && chars.next_if(|ch| matches!(ch, '.' | ')')).is_some()
        && chars.next().is_some_and(char::is_whitespace)
        && chars.all(char::is_whitespace)
}

#[cfg(windows)]
pub(super) fn web_keyboard_captured_left_should_retry_selection(
    context: &TextContext,
    selected_text: &str,
) -> bool {
    let (selected_core, _) = split_trailing_line_breaks(selected_text);
    let (context_core, _) = split_trailing_line_breaks(&context.text_snapshot);

    !selected_core.is_empty()
        && selected_core != context_core
        && context_core.ends_with(selected_core)
}

#[cfg(windows)]
fn is_sticky_notes_context(context: &TextContext) -> bool {
    context
        .app_id
        .starts_with("ApplicationFrameWindow/Windows.UI.Input.InputSite.WindowClass")
}

#[cfg(windows)]
pub(super) fn web_keyboard_prefers_line_context_for_scrolllock(surface_kind: SurfaceKind) -> bool {
    surface_kind == SurfaceKind::StickyNotes
}

#[cfg(windows)]
pub(super) fn web_keyboard_sticky_line_replacement_text(
    context: &TextContext,
    plan: &ReplacementPlan,
    selected_text: &str,
) -> Result<(String, String), PlatformError> {
    let (selected_core, selected_suffix) = split_trailing_line_breaks(selected_text);
    let (context_core, _) = split_trailing_line_breaks(&context.text_snapshot);

    if context_core.is_empty() || !selected_core.ends_with(context_core) {
        return Err(PlatformError::ReplacementUnavailableReason(format!(
            "web_keyboard_sticky_line_preflight expected={} actual={}",
            preview_for_error(context_core, 40),
            preview_for_error(selected_text, 40)
        )));
    }

    let selected_context = TextContext {
        app_id: context.app_id.clone(),
        window_id: context.window_id.clone(),
        control_id: context.control_id.clone(),
        text_snapshot: selected_core.to_owned(),
        caret_range: TextRange::caret(selected_core.len()),
        selection_range: None,
        capabilities: context.capabilities.clone(),
        telemetry: context.telemetry.clone(),
    };
    let selected_plan =
        stepler_core::build_replacement_plan(&selected_context, correction_mode_from_plan(plan))
            .map_err(|error| {
                PlatformError::ReplacementUnavailableReason(format!(
                    "web_keyboard_sticky_line_replan_failed error={error:?} selected={}",
                    preview_for_error(selected_core, 40)
                ))
            })?;
    let replacement_text = replace_range_text(
        selected_core,
        selected_plan.range,
        &selected_plan.replacement_text,
    )
    .ok_or(PlatformError::PreflightFailed)?;
    let replacement_text = format!("{replacement_text}{selected_suffix}");

    Ok((replacement_text, selected_plan.expected_before_text))
}

#[cfg(windows)]
fn split_trailing_line_breaks(text: &str) -> (&str, &str) {
    let trimmed_len = text.trim_end_matches(['\r', '\n']).len();
    text.split_at(trimmed_len)
}

#[cfg(windows)]
fn correction_mode_from_plan(plan: &ReplacementPlan) -> CorrectionMode {
    if plan.reason.starts_with("scrolllock_") {
        CorrectionMode::ScrollLock
    } else {
        CorrectionMode::Pause
    }
}

#[cfg(windows)]
pub(super) fn shifted_web_selection_prefix<'a>(
    selected: &'a str,
    expected: &str,
) -> Option<&'a str> {
    let prefix = selected.strip_suffix(expected)?;
    if !is_safe_shifted_web_selection_prefix(prefix) {
        return None;
    }

    Some(prefix)
}

#[cfg(windows)]
fn is_safe_shifted_web_selection_prefix(prefix: &str) -> bool {
    if prefix.is_empty() {
        return false;
    }
    if prefix.contains('\r') || prefix.contains('\n') {
        return false;
    }
    if prefix.chars().all(char::is_whitespace) {
        return prefix.chars().count() <= 4;
    }

    let trimmed = prefix.trim_start_matches([' ', '\t']);
    let indent_len = prefix.len() - trimmed.len();
    if prefix[..indent_len].chars().count() > 4 {
        return false;
    }

    if let Some((digits, rest)) = trimmed.split_once('.') {
        if !digits.is_empty()
            && digits.len() <= 3
            && digits.chars().all(|ch| ch.is_ascii_digit())
            && !rest.is_empty()
            && rest.chars().all(char::is_whitespace)
            && rest.chars().count() <= 2
        {
            return true;
        }
    }

    prefix.chars().count() <= 128
}

#[cfg(windows)]
pub(super) fn is_web_keyboard_line_context(control_id: &str) -> bool {
    control_id.starts_with("web-keyboard-line-selection:")
        || control_id.starts_with("web-keyboard-fast-line-selection:")
        || control_id.starts_with("web-keyboard-rocket-fast-line-selection:")
}

#[cfg(windows)]
fn web_ctrl_a_fallback_enabled() -> bool {
    env_flag_enabled("STEPLER_ENABLE_WEB_CTRL_A_FALLBACK", false)
}

#[cfg(windows)]
fn is_plausible_web_selected_text(text: &str) -> bool {
    let trimmed = text.trim();
    !trimmed.is_empty() && text.len() <= 4096 && !looks_like_browser_document_dump(text)
}

#[cfg(windows)]
fn is_plausible_web_left_context_text(text: &str) -> bool {
    let trimmed = text.trim();
    !trimmed.is_empty()
        && text.len() <= 1024
        && text.lines().count() <= 4
        && !looks_like_browser_document_dump(text)
}

#[cfg(windows)]
pub(super) fn looks_like_browser_document_dump(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("skip to main content")
        || lower.contains("main content")
        || lower.contains("unread messages")
}
