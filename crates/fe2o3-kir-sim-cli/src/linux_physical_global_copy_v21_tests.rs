//! Diagnostic mapping only, not a V21 loader or simulation admission.
use super::*;
#[test]
fn physical_global_copy_keeps_its_distinct_unsupported_profile_code() {
    assert_eq!(
        serde_json::to_value(unsupported_code(
            &UnsupportedFeatureV1::PhysicalGlobalCopyProfile
        ))
        .unwrap(),
        "physical_global_copy_profile"
    );
}

#[test]
fn global_copy_alias_and_pending_capture_refusals_keep_exact_codes() {
    for (error, expected) in [
        (
            SimulationPreflightErrorV1::PhysicalGlobalCopyPendingDebugUnavailableV21,
            "preflight_physical_global_copy_pending_debug_unavailable_v21",
        ),
        (
            SimulationPreflightErrorV1::PhysicalGlobalCopyAliasedArgumentsV21,
            "preflight_physical_global_copy_aliased_arguments_v21",
        ),
    ] {
        assert_eq!(
            serde_json::to_value(preflight_kind(&error)).unwrap(),
            expected
        );
    }
}
