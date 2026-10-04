use super::*;

#[test]
fn fixed6_census_identity_is_compiled_and_does_not_relax_overflow_admission() {
    assert_eq!(
        serde_json::to_value(CensusMode::FixedCheckedOutput {
            policy: DIAGNOSTIC_POLICY
        })
        .unwrap(),
        serde_json::json!({"kind": "fixed-checked-output", "policy": 6})
    );
    let error = run_production_fixed_checked_output_policy6_extraction_driver_v1(
        &["rustc".into()],
        Path::new("unused-fixed6-invalid-argv.ll"),
    )
    .unwrap_err();
    assert!(error.contains("requires exactly one canonical"));
}
