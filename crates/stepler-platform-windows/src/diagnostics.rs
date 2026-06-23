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
    pub surface: WindowsSurfaceDiagnostics,
    pub uia_focus: Option<WindowsUiaFocusDiagnostics>,
    pub probe_plan_methods: Vec<String>,
    pub runtime_probe_methods: Vec<String>,
    pub probe_plan_suppressed_methods: Vec<String>,
    pub probe_plan_fast: bool,
    pub probes: Vec<WindowsMethodProbeDiagnostics>,
    pub pause_trace: Vec<WindowsResolveTraceDiagnostics>,
    pub scrolllock_trace: Vec<WindowsResolveTraceDiagnostics>,
    pub selected_context_method: Option<String>,
    pub selected_replacement_method: Option<String>,
    pub selected_pause_context_method: Option<String>,
    pub selected_pause_replacement_method: Option<String>,
    pub selected_scrolllock_context_method: Option<String>,
    pub selected_scrolllock_replacement_method: Option<String>,
    pub context_method: Option<String>,
    pub context_error: Option<String>,
    pub context_skipped: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsResolveTraceDiagnostics {
    pub method: String,
    pub mode: String,
    pub safety: String,
    pub confidence: u8,
    pub preference_rank: usize,
    pub replacement_method: Option<String>,
    pub outcome: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsSurfaceDiagnostics {
    pub kind: String,
    pub confidence: u8,
    pub evidence: Vec<String>,
    pub web_keyboard_profile: String,
    pub allow_risky_methods: bool,
    pub pause_context_methods: Vec<String>,
    pub pause_replace_methods: Vec<String>,
    pub scrolllock_context_methods: Vec<String>,
    pub scrolllock_replace_methods: Vec<String>,
    pub forbidden_methods: Vec<String>,
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
    let target = foreground_target_from_handles(foreground, focused);
    let app_class = target.app_class.clone();
    let focused_class = target.focused_class.clone();
    let probe_plan = probe_plan_for(&target);
    let probe_plan_methods = probe_plan
        .probe_methods
        .iter()
        .map(|method| method.as_str().to_owned())
        .collect::<Vec<_>>();
    let runtime_probe_methods = windows_runtime_probe_methods(&target)
        .into_iter()
        .map(|method| method.as_str().to_owned())
        .collect::<Vec<_>>();
    let probe_plan_suppressed_methods = probe_plan
        .suppressed_methods
        .iter()
        .map(|method| method.as_str().to_owned())
        .collect::<Vec<_>>();
    let probe_plan_fast = probe_plan.fast_probe;
    let probes = windows_method_probes(&target);
    let classification = classify_surface(&target);
    let policy = surface_policy_for(classification.kind);
    let resolver = MethodResolver::default();
    let pause_decision = resolver
        .resolve_for_mode(&target, &probes, stepler_core::CorrectionMode::Pause)
        .ok();
    let scrolllock_decision = resolver
        .resolve_for_mode(&target, &probes, stepler_core::CorrectionMode::ScrollLock)
        .ok();
    let pause_trace =
        resolver.trace_for_mode(&target, &probes, stepler_core::CorrectionMode::Pause);
    let scrolllock_trace =
        resolver.trace_for_mode(&target, &probes, stepler_core::CorrectionMode::ScrollLock);
    let decision = pause_decision.clone();
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
        surface: WindowsSurfaceDiagnostics {
            kind: format!("{:?}", classification.kind),
            confidence: classification.confidence,
            evidence: classification.evidence,
            web_keyboard_profile: format!(
                "{:?}",
                web_keyboard_profile_for_surface(classification.kind)
            ),
            allow_risky_methods: policy.allow_risky_methods,
            pause_context_methods: policy
                .pause_methods
                .context_methods
                .iter()
                .map(|method| method.as_str().to_owned())
                .collect(),
            pause_replace_methods: policy
                .pause_methods
                .replace_methods
                .iter()
                .map(|method| method.as_str().to_owned())
                .collect(),
            scrolllock_context_methods: policy
                .scrolllock_methods
                .context_methods
                .iter()
                .map(|method| method.as_str().to_owned())
                .collect(),
            scrolllock_replace_methods: policy
                .scrolllock_methods
                .replace_methods
                .iter()
                .map(|method| method.as_str().to_owned())
                .collect(),
            forbidden_methods: policy
                .forbidden_methods
                .iter()
                .map(|method| method.as_str().to_owned())
                .collect(),
        },
        uia_focus: uia_focus_diagnostics().ok(),
        probe_plan_methods,
        runtime_probe_methods,
        probe_plan_suppressed_methods,
        probe_plan_fast,
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
        pause_trace: pause_trace
            .into_iter()
            .map(resolve_trace_diagnostics)
            .collect(),
        scrolllock_trace: scrolllock_trace
            .into_iter()
            .map(resolve_trace_diagnostics)
            .collect(),
        selected_context_method: decision
            .as_ref()
            .map(|decision| decision.context_method.as_str().to_owned()),
        selected_replacement_method: decision
            .as_ref()
            .map(|decision| decision.replacement_method.as_str().to_owned()),
        selected_pause_context_method: pause_decision
            .as_ref()
            .map(|decision| decision.context_method.as_str().to_owned()),
        selected_pause_replacement_method: pause_decision
            .as_ref()
            .map(|decision| decision.replacement_method.as_str().to_owned()),
        selected_scrolllock_context_method: scrolllock_decision
            .as_ref()
            .map(|decision| decision.context_method.as_str().to_owned()),
        selected_scrolllock_replacement_method: scrolllock_decision
            .as_ref()
            .map(|decision| decision.replacement_method.as_str().to_owned()),
        context_method,
        context_error,
        context_skipped,
    })
}

#[cfg(windows)]
pub(super) fn hotkey_failure_trace_summary_impl(
    mode: stepler_core::CorrectionMode,
    final_error: &str,
) -> Result<String, PlatformError> {
    let foreground = foreground_hwnd()?;
    let focused = focused_window(foreground).unwrap_or(foreground);
    let target = foreground_target_from_handles(foreground, focused);
    Ok(hotkey_failure_trace_summary_for_target(
        &target,
        mode,
        final_error,
    ))
}

#[cfg(not(windows))]
pub(super) fn hotkey_failure_trace_summary_impl(
    _mode: stepler_core::CorrectionMode,
    _final_error: &str,
) -> Result<String, PlatformError> {
    Err(PlatformError::Unsupported)
}

#[cfg(windows)]
pub(super) fn hotkey_failure_trace_summary_for_target(
    target: &ForegroundTarget,
    mode: stepler_core::CorrectionMode,
    final_error: &str,
) -> String {
    let classification = classify_surface(target);
    let probe_plan = probe_plan_for(target);
    let runtime_methods = windows_runtime_probe_methods(target);
    let probes = windows_method_probes(target);
    let probed_methods = probes
        .iter()
        .map(|probe| probe.method_id)
        .collect::<Vec<_>>();
    let probe_none = runtime_methods
        .iter()
        .copied()
        .filter(|method| !probed_methods.contains(method))
        .collect::<Vec<_>>();
    let trace = MethodResolver::default().trace_for_mode(target, &probes, mode);
    let accepted = trace
        .iter()
        .find(|entry| entry.outcome == stepler_platform::ResolveTraceOutcome::Accepted);
    let policy_skipped = trace
        .iter()
        .filter(|entry| {
            matches!(
                entry.outcome,
                stepler_platform::ResolveTraceOutcome::ForbiddenByPolicy
                    | stepler_platform::ResolveTraceOutcome::RiskyMethodBlocked
                    | stepler_platform::ResolveTraceOutcome::ReplacementForbiddenByPolicy
            )
        })
        .map(|entry| trace_method_label(entry.method, Some(entry.outcome)))
        .collect::<Vec<_>>();

    let selected = accepted
        .map(|entry| {
            let replacement = entry
                .replacement_method
                .map(|method| method.as_str())
                .unwrap_or("none");
            format!("{}->{replacement}", entry.method.as_str())
        })
        .unwrap_or_else(|| String::from("none"));

    format!(
        "surface={:?}; confidence={}; mode={:?}; selected={}; probe_plan=[{}]; runtime=[{}]; probes=[{}]; probe_none=[{}]; suppressed=[{}]; policy_skipped=[{}]; final=operation_failed:{}",
        classification.kind,
        classification.confidence,
        mode,
        selected,
        method_list(&probe_plan.probe_methods),
        method_list(&runtime_methods),
        method_list(&probed_methods),
        method_list(&probe_none),
        method_list(&probe_plan.suppressed_methods),
        policy_skipped.join("|"),
        final_error
    )
}

#[cfg(windows)]
fn trace_method_label(
    method: MethodId,
    outcome: Option<stepler_platform::ResolveTraceOutcome>,
) -> String {
    match outcome {
        Some(outcome) => format!("{}:{outcome:?}", method.as_str()),
        None => method.as_str().to_owned(),
    }
}

#[cfg(windows)]
fn method_list(methods: &[MethodId]) -> String {
    methods
        .iter()
        .map(|method| method.as_str())
        .collect::<Vec<_>>()
        .join(",")
}

#[cfg(windows)]
fn foreground_target_from_handles(foreground: isize, focused: isize) -> ForegroundTarget {
    let app_class = window_class_name(foreground).unwrap_or_else(|| String::from("unknown"));
    let focused_class = window_class_name(focused).unwrap_or_else(|| String::from("unknown"));
    let mut title = window_title(foreground).unwrap_or_default();
    if let Some(marker_title) = active_terminal_app_marker_title() {
        if !title.contains(marker_title) {
            title = format!("{title} {marker_title}");
        }
    }
    ForegroundTarget {
        app_class,
        focused_class,
        title,
        process_name: window_process_name(foreground),
        window_id: hwnd_id(foreground),
        control_id: hwnd_id(focused),
    }
}

fn resolve_trace_diagnostics(
    entry: stepler_platform::ResolveTraceEntry,
) -> WindowsResolveTraceDiagnostics {
    WindowsResolveTraceDiagnostics {
        method: entry.method.as_str().to_owned(),
        mode: format!("{:?}", entry.mode),
        safety: format!("{:?}", entry.safety),
        confidence: entry.confidence,
        preference_rank: entry.preference_rank,
        replacement_method: entry
            .replacement_method
            .map(|method| method.as_str().to_owned()),
        outcome: format!("{:?}", entry.outcome),
        reason: entry.reason,
    }
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
