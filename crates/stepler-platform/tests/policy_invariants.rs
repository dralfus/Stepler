use std::sync::{Mutex, MutexGuard, OnceLock};
use stepler_core::MethodId;

use stepler_platform::{
    adapter_contract, default_probe_policies, default_surface_policies, method_is_bridge_method,
    probe_policy_for, surface_allows_risky_method, surface_policy_for, SurfaceKind, ALL_METHOD_IDS,
    BRIDGE_METHOD_IDS,
};

const DIAGNOSTIC_UNKNOWN_PROBES_ENV: &str = "STEPLER_DIAGNOSTIC_UNKNOWN_PROBES";

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
fn forbidden_methods_are_not_preferred_for_context_or_replacement() {
    for policy in default_surface_policies() {
        let preferred_methods = all_preferred_methods(&policy);

        let overlap = policy
            .forbidden_methods
            .iter()
            .copied()
            .filter(|method| preferred_methods.contains(method))
            .collect::<Vec<_>>();
        assert!(
            overlap.is_empty(),
            "{:?}: forbidden methods are also preferred context/replacement methods: {:?}",
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

    let probe_policies = default_probe_policies()
        .into_iter()
        .map(|policy| policy.surface)
        .collect::<Vec<_>>();

    for policy in default_surface_policies() {
        assert!(
            probe_policies.contains(&policy.surface),
            "{:?}: surface policy has no matching probe policy",
            policy.surface
        );
    }
}

#[test]
fn policy_tables_do_not_have_duplicate_surfaces() {
    let probe_surfaces = default_probe_policies()
        .into_iter()
        .map(|policy| policy.surface)
        .collect::<Vec<_>>();
    assert_eq!(
        unique_surfaces(&probe_surfaces).len(),
        probe_surfaces.len(),
        "duplicate surfaces in probe policies: {:?}",
        duplicate_surfaces(&probe_surfaces)
    );

    let surface_surfaces = default_surface_policies()
        .into_iter()
        .map(|policy| policy.surface)
        .collect::<Vec<_>>();
    assert_eq!(
        unique_surfaces(&surface_surfaces).len(),
        surface_surfaces.len(),
        "duplicate surfaces in surface policies: {:?}",
        duplicate_surfaces(&surface_surfaces)
    );
}

#[test]
fn probe_methods_are_surface_context_preferences_or_documented_exceptions() {
    let _guard = env_guard();
    unsafe {
        std::env::remove_var(DIAGNOSTIC_UNKNOWN_PROBES_ENV);
    }

    for probe_policy in default_probe_policies() {
        let surface_policy = surface_policy_for(probe_policy.surface);
        let preferred_context_methods = surface_policy
            .pause_methods
            .context_methods
            .iter()
            .chain(surface_policy.scrolllock_methods.context_methods.iter())
            .copied()
            .collect::<Vec<_>>();

        let unexpected = probe_policy
            .probe_methods
            .iter()
            .copied()
            .filter(|method| {
                !preferred_context_methods.contains(method)
                    && !documented_probe_context_exception(probe_policy.surface, *method)
            })
            .collect::<Vec<_>>();

        assert!(
            unexpected.is_empty(),
            "{:?}: probe methods are not context preferences or documented exceptions: {:?}",
            probe_policy.surface,
            unexpected
        );
    }
}

#[test]
fn bridge_methods_require_explicit_surface_allowance() {
    let _guard = env_guard();
    unsafe {
        std::env::remove_var(DIAGNOSTIC_UNKNOWN_PROBES_ENV);
    }

    for method in BRIDGE_METHOD_IDS {
        assert!(
            method_is_bridge_method(*method),
            "{} should be marked as bridge method",
            method.as_str()
        );
    }

    for probe_policy in default_probe_policies() {
        for method in &probe_policy.probe_methods {
            if !method_is_bridge_method(*method) {
                continue;
            }

            assert!(
                surface_allows_bridge_method(probe_policy.surface, *method),
                "{:?}: bridge probe method {} lacks explicit surface allowance",
                probe_policy.surface,
                method.as_str()
            );
        }
    }

    for surface_policy in default_surface_policies() {
        let preferred_methods = all_preferred_methods(&surface_policy);
        for method in preferred_methods {
            if !method_is_bridge_method(method) {
                continue;
            }

            assert!(
                surface_allows_bridge_method(surface_policy.surface, method),
                "{:?}: bridge preferred method {} lacks explicit surface allowance",
                surface_policy.surface,
                method.as_str()
            );
        }
    }
}

#[test]
fn ordinary_unknown_surface_is_conservative() {
    let _guard = env_guard();
    unsafe {
        std::env::remove_var(DIAGNOSTIC_UNKNOWN_PROBES_ENV);
    }

    let probe_policy = probe_policy_for(SurfaceKind::Unknown);
    assert_eq!(probe_policy.probe_methods, conservative_unknown_methods());

    for forbidden in [
        MethodId::TerminalClipboardShortcut,
        MethodId::ClipboardSelection,
        MethodId::SendInput,
        MethodId::ConsoleBuffer,
        MethodId::PsReadLine,
        MethodId::WebKeyboardSelection,
        MethodId::Win32EditMessages,
        MethodId::SshTerminal,
        MethodId::WordCom,
        MethodId::XtermKeyboardSelection,
    ] {
        assert!(
            probe_policy.suppressed_methods.contains(&forbidden),
            "ordinary Unknown should suppress {}",
            forbidden.as_str()
        );
    }

    let surface_policy = surface_policy_for(SurfaceKind::Unknown);
    assert_eq!(
        surface_policy.pause_methods.context_methods,
        conservative_unknown_methods()
    );
    assert_eq!(
        surface_policy.scrolllock_methods.context_methods,
        conservative_unknown_methods()
    );
    assert!(!surface_policy.allow_risky_methods);
    assert!(surface_policy
        .forbidden_methods
        .contains(&MethodId::WebKeyboardSelection));
    assert!(surface_policy
        .forbidden_methods
        .contains(&MethodId::ClipboardSelection));
}

#[test]
fn diagnostic_unknown_surface_can_probe_all_methods_explicitly() {
    let _guard = env_guard();
    unsafe {
        std::env::set_var(DIAGNOSTIC_UNKNOWN_PROBES_ENV, "1");
    }

    let probe_policy = probe_policy_for(SurfaceKind::Unknown);

    unsafe {
        std::env::remove_var(DIAGNOSTIC_UNKNOWN_PROBES_ENV);
    }

    assert_eq!(probe_policy.probe_methods, ALL_METHOD_IDS);
    assert!(probe_policy.suppressed_methods.is_empty());
    assert!(!probe_policy.fast_probe);
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

fn all_preferred_methods(policy: &stepler_platform::SurfacePolicy) -> Vec<MethodId> {
    unique_methods(
        &policy
            .pause_methods
            .context_methods
            .iter()
            .chain(policy.pause_methods.replace_methods.iter())
            .chain(policy.scrolllock_methods.context_methods.iter())
            .chain(policy.scrolllock_methods.replace_methods.iter())
            .copied()
            .collect::<Vec<_>>(),
    )
}

fn unique_surfaces(surfaces: &[SurfaceKind]) -> Vec<SurfaceKind> {
    let mut unique = Vec::new();
    for surface in surfaces {
        if !unique.contains(surface) {
            unique.push(*surface);
        }
    }
    unique
}

fn duplicate_surfaces(surfaces: &[SurfaceKind]) -> Vec<SurfaceKind> {
    let mut duplicates = Vec::new();
    for surface in surfaces {
        if surfaces
            .iter()
            .filter(|candidate| *candidate == surface)
            .count()
            > 1
            && !duplicates.contains(surface)
        {
            duplicates.push(*surface);
        }
    }
    duplicates
}

fn documented_probe_context_exception(_surface: SurfaceKind, _method: MethodId) -> bool {
    false
}

fn surface_allows_bridge_method(surface: SurfaceKind, method: MethodId) -> bool {
    matches!(
        (surface, method),
        (
            SurfaceKind::WindowsTerminalCmd,
            MethodId::TerminalClipboardShortcut
        ) | (SurfaceKind::WindowsTerminalPowerShell, MethodId::PsReadLine)
            | (SurfaceKind::QwenTerminal, MethodId::XtermKeyboardSelection)
    )
}

fn conservative_unknown_methods() -> Vec<MethodId> {
    vec![
        MethodId::UiAutomationEditableText,
        MethodId::UiAutomationDocumentText,
        MethodId::UiAutomationText,
    ]
}

fn env_guard() -> MutexGuard<'static, ()> {
    static ENV_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();
    ENV_MUTEX
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("env mutex poisoned")
}
