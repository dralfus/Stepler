use stepler_core::MethodId;
use stepler_platform::{
    adapter_contract, default_probe_policies, default_surface_policies,
    surface_allows_risky_method, ALL_METHOD_IDS,
};

#[test]
fn probe_policies_partition_all_known_methods() {
    let probe_policies = default_probe_policies();
    assert!(!probe_policies.is_empty(), "probe policies are empty");

    for policy in probe_policies {
        assert_eq!(
            unique_methods(&policy.probe_methods).len(),
            policy.probe_methods.len(),
            "{:?}: duplicate probe method",
            policy.surface
        );
        assert_eq!(
            unique_methods(&policy.suppressed_methods).len(),
            policy.suppressed_methods.len(),
            "{:?}: duplicate suppressed method",
            policy.surface
        );

        for method in policy
            .probe_methods
            .iter()
            .chain(policy.suppressed_methods.iter())
        {
            assert!(
                ALL_METHOD_IDS.contains(method),
                "{:?}: unknown method in probe policy: {}",
                policy.surface,
                method.as_str()
            );
        }

        let overlap = policy
            .probe_methods
            .iter()
            .copied()
            .filter(|method| policy.suppressed_methods.contains(method))
            .collect::<Vec<_>>();
        assert!(
            overlap.is_empty(),
            "{:?}: methods are both probed and suppressed: {:?}",
            policy.surface,
            overlap
        );

        let covered = unique_methods(
            &policy
                .probe_methods
                .iter()
                .chain(policy.suppressed_methods.iter())
                .copied()
                .collect::<Vec<_>>(),
        );
        assert_eq!(
            covered.len(),
            ALL_METHOD_IDS.len(),
            "{:?}: probe/suppressed coverage size",
            policy.surface
        );
        for method in ALL_METHOD_IDS {
            assert!(
                covered.contains(method),
                "{:?}: missing method {} from probe/suppressed coverage",
                policy.surface,
                method.as_str()
            );
        }
    }
}

#[test]
fn surface_policy_preferences_reference_known_methods() {
    for policy in default_surface_policies() {
        for (label, methods) in [
            ("pause context", &policy.pause_methods.context_methods),
            ("pause replace", &policy.pause_methods.replace_methods),
            (
                "scrolllock context",
                &policy.scrolllock_methods.context_methods,
            ),
            (
                "scrolllock replace",
                &policy.scrolllock_methods.replace_methods,
            ),
            ("forbidden", &policy.forbidden_methods),
        ] {
            assert!(
                !methods.is_empty() || label == "forbidden",
                "{:?}: {label} methods are empty",
                policy.surface
            );
            for method in methods {
                assert!(
                    ALL_METHOD_IDS.contains(method),
                    "{:?}: {label} contains unknown method {}",
                    policy.surface,
                    method.as_str()
                );
            }
        }
    }
}

#[test]
fn forbidden_methods_are_not_preferred_for_replacement() {
    for policy in default_surface_policies() {
        let preferred_replacements = policy
            .pause_methods
            .replace_methods
            .iter()
            .chain(policy.scrolllock_methods.replace_methods.iter())
            .copied()
            .collect::<Vec<_>>();

        let overlap = policy
            .forbidden_methods
            .iter()
            .copied()
            .filter(|method| preferred_replacements.contains(method))
            .collect::<Vec<_>>();
        assert!(
            overlap.is_empty(),
            "{:?}: forbidden methods are also preferred replacements: {:?}",
            policy.surface,
            overlap
        );
    }
}

#[test]
fn risky_preferred_methods_require_explicit_surface_allowance() {
    for policy in default_surface_policies() {
        let preferred_methods = unique_methods(
            &policy
                .pause_methods
                .context_methods
                .iter()
                .chain(policy.pause_methods.replace_methods.iter())
                .chain(policy.scrolllock_methods.context_methods.iter())
                .chain(policy.scrolllock_methods.replace_methods.iter())
                .copied()
                .collect::<Vec<_>>(),
        );

        for method in preferred_methods {
            if !adapter_contract(method).risky {
                continue;
            }

            if policy.allow_risky_methods {
                assert!(
                    surface_allows_risky_method(policy.surface, method),
                    "{:?}: risky method {} allowed by policy but not by explicit surface allowance",
                    policy.surface,
                    method.as_str()
                );
            } else {
                assert!(
                    !surface_allows_risky_method(policy.surface, method),
                    "{:?}: risky method {} has explicit surface allowance while policy blocks risky methods",
                    policy.surface,
                    method.as_str()
                );
            }
        }
    }
}

#[test]
fn probe_policies_have_matching_surface_policies() {
    let surface_policies = default_surface_policies()
        .into_iter()
        .map(|policy| policy.surface)
        .collect::<Vec<_>>();

    for policy in default_probe_policies() {
        assert!(
            surface_policies.contains(&policy.surface),
            "{:?}: probe policy has no matching surface policy",
            policy.surface
        );
    }
}

fn unique_methods(methods: &[MethodId]) -> Vec<MethodId> {
    let mut unique = Vec::new();
    for method in methods {
        if !unique.contains(method) {
            unique.push(*method);
        }
    }
    unique
}
