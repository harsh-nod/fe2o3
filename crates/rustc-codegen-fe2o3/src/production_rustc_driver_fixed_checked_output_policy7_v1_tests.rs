use super::*;

#[test]
fn fixed7_census_identity_is_compiled_and_overflow_admission_is_unchanged() {
    assert_eq!(
        serde_json::to_value(CensusMode::FixedCheckedOutput {
            policy: DIAGNOSTIC_POLICY
        })
        .unwrap(),
        serde_json::json!({"kind": "fixed-checked-output", "policy": 7})
    );
    for args in [
        vec!["rustc".into()],
        vec!["rustc".into(), "-Coverflow-checks=off".into()],
        vec![
            "rustc".into(),
            "-Coverflow-checks=on".into(),
            "-Coverflow-checks=on".into(),
        ],
    ] {
        let error = run_production_fixed_checked_output_policy7_extraction_driver_v1(
            &args,
            Path::new("unused-fixed7-invalid-argv.ll"),
        )
        .unwrap_err();
        assert!(error.contains("requires exactly one canonical"));
    }
}
