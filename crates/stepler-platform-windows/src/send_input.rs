use super::*;

#[cfg(windows)]
#[derive(Debug, Default, Clone, Copy)]
pub(super) struct SendInputMethod;

#[cfg(windows)]
impl SendInputMethod {
    pub(super) fn probe(&self, target: &ForegroundTarget) -> Option<MethodProbe> {
        if target.app_class.eq_ignore_ascii_case("Progman")
            || target.app_class.eq_ignore_ascii_case("WorkerW")
            || target.focused_class.eq_ignore_ascii_case("SysListView32")
        {
            return None;
        }

        let mut probe = MethodProbe::risky(MethodId::SendInput, "generic SendInput text fallback");
        probe.requires_clipboard = false;
        probe.can_preflight = false;
        probe.can_verify = false;
        Some(probe)
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

        send_unicode_text(&plan.replacement_text)?;
        std::thread::sleep(Duration::from_millis(40));

        Ok(ApplyReplacementResult {
            applied: true,
            actual_before_text: Some(context.text_snapshot.clone()),
            actual_after_text: Some(plan.replacement_text.clone()),
            method: MethodId::SendInput.as_str().to_owned(),
            retry_count: 0,
            timings: Vec::new(),
        })
    }
}
