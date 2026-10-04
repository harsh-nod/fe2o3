//! Diagnostic mapping only: the CLI does not admit physical-entry capture.
use super::*;

#[test]
fn physical_entry_v20_refusals_keep_exact_machine_readable_codes() {
    let kind =
        preflight_kind(&SimulationPreflightErrorV1::PhysicalEntrySymbolicDebugUnavailableV20);
    assert_eq!(
        kind,
        ErrorKind::PreflightPhysicalEntrySymbolicDebugUnavailableV20
    );
    assert_eq!(
        serde_json::to_value(kind).unwrap(),
        "preflight_physical_entry_symbolic_debug_unavailable_v20"
    );
    for (feature, expected) in [
        (UnsupportedFeatureV1::PhysicalEntry, "physical_entry"),
        (
            UnsupportedFeatureV1::PhysicalEntryProfile,
            "physical_entry_profile",
        ),
    ] {
        assert_eq!(
            serde_json::to_value(unsupported_code(&feature)).unwrap(),
            expected
        );
    }
}
