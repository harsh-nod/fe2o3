//! Diagnostic mapping only, not a V22 loader or simulation admission.
use super::*;
#[test]
fn physical_lds_exchange_keeps_its_distinct_unsupported_profile_code() {
    assert_eq!(
        serde_json::to_value(unsupported_code(
            &UnsupportedFeatureV1::PhysicalLdsExchangeProfile
        ))
        .unwrap(),
        "physical_lds_exchange_profile"
    );
}

#[test]
fn global_copy_alias_and_pending_capture_refusals_keep_exact_codes() {
    for (error, expected) in [
        (
            SimulationPreflightErrorV1::PhysicalLdsExchangeDebugUnavailableV22,
            "preflight_physical_lds_exchange_debug_unavailable_v22",
        ),
        (
            SimulationPreflightErrorV1::PhysicalLdsExchangeAliasedArgumentsV22,
            "preflight_physical_lds_exchange_aliased_arguments_v22",
        ),
    ] {
        assert_eq!(
            serde_json::to_value(preflight_kind(&error)).unwrap(),
            expected
        );
    }
}
