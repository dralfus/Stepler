use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsFocusDiagnostics {
    pub foreground_hwnd: String,
    pub foreground_class: String,
    pub foreground_title: String,
    pub focused_hwnd: String,
    pub focused_class: String,
    pub focused_title: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsMethodDiagnostics {
    pub foreground: WindowsFocusDiagnostics,
    pub uia_focus: Option<WindowsUiaFocusDiagnostics>,
    pub probes: Vec<WindowsMethodProbeDiagnostics>,
    pub selected_context_method: Option<String>,
    pub selected_replacement_method: Option<String>,
    pub context_method: Option<String>,
    pub context_error: Option<String>,
    pub context_skipped: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsUiaFocusDiagnostics {
    pub name: String,
    pub control_type: String,
    pub automation_id: String,
    pub class_name: String,
    pub framework_id: String,
    pub has_keyboard_focus: bool,
    pub is_keyboard_focusable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsMethodProbeDiagnostics {
    pub method: String,
    pub safety: String,
    pub requires_clipboard: bool,
    pub requires_focus_stability: bool,
    pub can_preflight: bool,
    pub can_verify: bool,
    pub reason: String,
}

#[cfg(windows)]
pub(super) fn focus_diagnostics_impl() -> Result<WindowsFocusDiagnostics, PlatformError> {
    let foreground = foreground_hwnd()?;
    let focused = focused_window(foreground).unwrap_or(foreground);
    Ok(WindowsFocusDiagnostics {
        foreground_hwnd: hwnd_id(foreground),
        foreground_class: window_class_name(foreground).unwrap_or_else(|| String::from("unknown")),
        foreground_title: window_title(foreground).unwrap_or_default(),
        focused_hwnd: hwnd_id(focused),
        focused_class: window_class_name(focused).unwrap_or_else(|| String::from("unknown")),
        focused_title: window_title(focused).unwrap_or_default(),
    })
}

#[cfg(windows)]
pub(super) fn method_diagnostics_impl() -> Result<WindowsMethodDiagnostics, PlatformError> {
    let foreground = foreground_hwnd()?;
    let focused = focused_window(foreground).unwrap_or(foreground);
    let app_class = window_class_name(foreground).unwrap_or_else(|| String::from("unknown"));
    let focused_class = window_class_name(focused).unwrap_or_else(|| String::from("unknown"));
    let mut title = window_title(foreground).unwrap_or_default();
    if let Some(marker_title) = active_terminal_app_marker_title() {
        if !title.contains(marker_title) {
            title = format!("{title} {marker_title}");
        }
    }
    let target = ForegroundTarget {
        app_class: app_class.clone(),
        focused_class: focused_class.clone(),
        title,
        process_name: window_process_name(foreground),
        window_id: hwnd_id(foreground),
        control_id: hwnd_id(focused),
    };
    let probes = windows_method_probes(&target);
    let decision = MethodResolver::default().resolve(&target, &probes).ok();
    let run_context = std::env::var("STEPLER_DIAGNOSE_CONTEXT")
        .map(|value| value == "1")
        .unwrap_or(false);
    let (context_method, context_error, context_skipped) = if run_context {
        let context = text_context();
        match context {
            Ok(context) => (
                context
                    .capabilities
                    .method_binding
                    .as_ref()
                    .map(|binding| binding.context_method.as_str().to_owned()),
                None,
                false,
            ),
            Err(error) => (None, Some(format!("{error:?}")), false),
        }
    } else {
        (None, None, true)
    };

    Ok(WindowsMethodDiagnostics {
        foreground: WindowsFocusDiagnostics {
            foreground_hwnd: hwnd_id(foreground),
            foreground_class: app_class,
            foreground_title: target.title,
            focused_hwnd: hwnd_id(focused),
            focused_class,
            focused_title: window_title(focused).unwrap_or_default(),
        },
        uia_focus: uia_focus_diagnostics().ok(),
        probes: probes
            .into_iter()
            .map(|probe| WindowsMethodProbeDiagnostics {
                method: probe.method_id.as_str().to_owned(),
                safety: format!("{:?}", probe.safety),
                requires_clipboard: probe.requires_clipboard,
                requires_focus_stability: probe.requires_focus_stability,
                can_preflight: probe.can_preflight,
                can_verify: probe.can_verify,
                reason: probe.reason,
            })
            .collect(),
        selected_context_method: decision
            .as_ref()
            .map(|decision| decision.context_method.as_str().to_owned()),
        selected_replacement_method: decision
            .as_ref()
            .map(|decision| decision.replacement_method.as_str().to_owned()),
        context_method,
        context_error,
        context_skipped,
    })
}

#[cfg(not(windows))]
pub(super) fn focus_diagnostics_impl() -> Result<WindowsFocusDiagnostics, PlatformError> {
    Err(PlatformError::Unsupported)
}

#[cfg(not(windows))]
pub(super) fn method_diagnostics_impl() -> Result<WindowsMethodDiagnostics, PlatformError> {
    Err(PlatformError::Unsupported)
}

#[cfg(windows)]
pub(super) fn uia_focus_diagnostics() -> Result<WindowsUiaFocusDiagnostics, PlatformError> {
    let output = run_powershell_script(UIA_FOCUS_DIAGNOSTICS_SCRIPT, &[])?;
    let fields = parse_key_value_lines(&output);
    if fields.get("ok").map(String::as_str) != Some("1") {
        return Err(PlatformError::ReplacementUnavailable);
    }

    Ok(WindowsUiaFocusDiagnostics {
        name: fields.get("name").cloned().unwrap_or_default(),
        control_type: fields.get("control_type").cloned().unwrap_or_default(),
        automation_id: fields.get("automation_id").cloned().unwrap_or_default(),
        class_name: fields.get("class_name").cloned().unwrap_or_default(),
        framework_id: fields.get("framework_id").cloned().unwrap_or_default(),
        has_keyboard_focus: fields
            .get("has_keyboard_focus")
            .map(String::as_str)
            .is_some_and(|value| value == "1"),
        is_keyboard_focusable: fields
            .get("is_keyboard_focusable")
            .map(String::as_str)
            .is_some_and(|value| value == "1"),
    })
}
