use crate::{
    classify_surface, default_surface_policies, default_surface_policy,
    surface_allows_risky_method, ForegroundTarget, MethodProbe, ProbeSafety, SurfaceClassification,
    SurfacePolicy,
};
use stepler_core::{CorrectionMode, MethodId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolveDecision {
    pub context_method: MethodId,
    pub replacement_method: MethodId,
    pub safety: ProbeSafety,
    pub reason: String,
    pub surface: SurfaceClassification,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolveTraceEntry {
    pub method: MethodId,
    pub mode: CorrectionMode,
    pub safety: ProbeSafety,
    pub confidence: u8,
    pub preference_rank: usize,
    pub replacement_method: Option<MethodId>,
    pub outcome: ResolveTraceOutcome,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolveTraceOutcome {
    Accepted,
    ForbiddenByPolicy,
    RiskyMethodBlocked,
    UnsupportedProbe,
    ReplacementForbiddenByPolicy,
    SkippedAfterAccepted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveError {
    NoSupportedMethod,
    ForbiddenByPolicy(MethodId),
    RiskyMethodBlocked(MethodId),
}

#[derive(Debug, Clone)]
pub struct MethodResolver {
    policies: Vec<SurfacePolicy>,
}

impl MethodResolver {
    pub fn new(policies: Vec<SurfacePolicy>) -> Self {
        Self { policies }
    }

    pub fn resolve(
        &self,
        target: &ForegroundTarget,
        probes: &[MethodProbe],
    ) -> Result<ResolveDecision, ResolveError> {
        self.resolve_for_mode(target, probes, CorrectionMode::Pause)
    }

    pub fn resolve_for_mode(
        &self,
        target: &ForegroundTarget,
        probes: &[MethodProbe],
        mode: CorrectionMode,
    ) -> Result<ResolveDecision, ResolveError> {
        let classification = classify_surface(target);
        let policy = self.policy_for(&classification);
        let preferences = policy.preferences_for(mode);
        let mut candidates = probes
            .iter()
            .filter(|probe| probe.safety != ProbeSafety::Unsupported)
            .collect::<Vec<_>>();
        candidates.sort_by_key(|probe| {
            (
                method_preference_rank(probe.method_id, &preferences.context_methods),
                std::cmp::Reverse(probe.confidence),
            )
        });

        for probe in candidates {
            if policy.forbidden_methods.contains(&probe.method_id) {
                continue;
            }
            if probe.safety == ProbeSafety::Risky
                && (!policy.allow_risky_methods
                    || !surface_allows_risky_method(classification.kind, probe.method_id))
            {
                continue;
            }
            let replacement_method = policy
                .preferences_for(mode)
                .replace_methods
                .iter()
                .copied()
                .find(|method| *method == probe.method_id)
                .unwrap_or(probe.method_id);
            if policy.forbidden_methods.contains(&replacement_method) {
                continue;
            }

            return Ok(ResolveDecision {
                context_method: probe.method_id,
                replacement_method,
                safety: probe.safety,
                reason: format!(
                    "{} via surface {:?} confidence={} evidence={}",
                    probe.reason,
                    classification.kind,
                    classification.confidence,
                    classification.evidence.join("; ")
                ),
                surface: classification,
            });
        }

        if probes.iter().any(|probe| {
            probe.safety != ProbeSafety::Unsupported
                && policy.forbidden_methods.contains(&probe.method_id)
        }) {
            return Err(ResolveError::ForbiddenByPolicy(
                probes
                    .iter()
                    .find(|probe| policy.forbidden_methods.contains(&probe.method_id))
                    .map(|probe| probe.method_id)
                    .unwrap(),
            ));
        }
        if probes.iter().any(|probe| {
            probe.safety == ProbeSafety::Risky
                && (!policy.allow_risky_methods
                    || !surface_allows_risky_method(classification.kind, probe.method_id))
                && !policy.forbidden_methods.contains(&probe.method_id)
        }) {
            return Err(ResolveError::RiskyMethodBlocked(
                probes
                    .iter()
                    .find(|probe| probe.safety == ProbeSafety::Risky)
                    .map(|probe| probe.method_id)
                    .unwrap(),
            ));
        }

        Err(ResolveError::NoSupportedMethod)
    }

    pub fn trace_for_mode(
        &self,
        target: &ForegroundTarget,
        probes: &[MethodProbe],
        mode: CorrectionMode,
    ) -> Vec<ResolveTraceEntry> {
        let classification = classify_surface(target);
        let policy = self.policy_for(&classification);
        let preferences = policy.preferences_for(mode);
        let mut probes = probes.iter().collect::<Vec<_>>();
        probes.sort_by_key(|probe| {
            (
                method_preference_rank(probe.method_id, &preferences.context_methods),
                std::cmp::Reverse(probe.confidence),
            )
        });

        let mut accepted = false;
        probes
            .into_iter()
            .map(|probe| {
                let preference_rank =
                    method_preference_rank(probe.method_id, &preferences.context_methods);
                let replacement_method = preferences
                    .replace_methods
                    .iter()
                    .copied()
                    .find(|method| *method == probe.method_id)
                    .unwrap_or(probe.method_id);

                let (outcome, replacement_method, reason) =
                    if probe.safety == ProbeSafety::Unsupported {
                        (
                            ResolveTraceOutcome::UnsupportedProbe,
                            None,
                            probe.reason.clone(),
                        )
                    } else if policy.forbidden_methods.contains(&probe.method_id) {
                        (
                            ResolveTraceOutcome::ForbiddenByPolicy,
                            None,
                            format!(
                                "{} forbidden by {:?}",
                                probe.method_id.as_str(),
                                policy.surface
                            ),
                        )
                    } else if probe.safety == ProbeSafety::Risky
                        && (!policy.allow_risky_methods
                            || !surface_allows_risky_method(classification.kind, probe.method_id))
                    {
                        (
                            ResolveTraceOutcome::RiskyMethodBlocked,
                            None,
                            format!(
                                "{} risky blocked by {:?}",
                                probe.method_id.as_str(),
                                policy.surface
                            ),
                        )
                    } else if policy.forbidden_methods.contains(&replacement_method) {
                        (
                            ResolveTraceOutcome::ReplacementForbiddenByPolicy,
                            Some(replacement_method),
                            format!(
                                "replacement {} forbidden by {:?}",
                                replacement_method.as_str(),
                                policy.surface
                            ),
                        )
                    } else if accepted {
                        (
                            ResolveTraceOutcome::SkippedAfterAccepted,
                            Some(replacement_method),
                            String::from("viable candidate skipped after accepted candidate"),
                        )
                    } else {
                        accepted = true;
                        (
                            ResolveTraceOutcome::Accepted,
                            Some(replacement_method),
                            format!(
                                "{} accepted via surface {:?} confidence={} evidence={}",
                                probe.reason,
                                classification.kind,
                                classification.confidence,
                                classification.evidence.join("; ")
                            ),
                        )
                    };

                ResolveTraceEntry {
                    method: probe.method_id,
                    mode,
                    safety: probe.safety,
                    confidence: probe.confidence,
                    preference_rank,
                    replacement_method,
                    outcome,
                    reason,
                }
            })
            .collect()
    }

    fn policy_for(&self, classification: &SurfaceClassification) -> SurfacePolicy {
        self.policies
            .iter()
            .find(|policy| policy.matches(classification))
            .cloned()
            .unwrap_or_else(default_surface_policy)
    }
}

impl Default for MethodResolver {
    fn default() -> Self {
        Self::new(default_surface_policies())
    }
}

fn method_preference_rank(method: MethodId, preferred_methods: &[MethodId]) -> usize {
    preferred_methods
        .iter()
        .position(|preferred| *preferred == method)
        .unwrap_or(usize::MAX)
}
