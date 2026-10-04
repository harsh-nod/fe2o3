//! Argument/profile controls only; no fabricated source export acceptance.
use super::*;
#[test]
fn arguments_are_closed_absolute_and_exact_hash_bound() {
    assert!(
        arguments(vec![
            "loop".into(),
            "/tmp/input.fe2sim".into(),
            "a".repeat(64)
        ])
        .is_ok()
    );
    for args in [
        vec!["unknown".into(), "/tmp/input".into(), "a".repeat(64)],
        vec!["loop".into(), "relative".into(), "a".repeat(64)],
        vec!["workgroup".into(), "/tmp/input\n".into(), "a".repeat(64)],
        vec!["workgroup".into(), "/tmp/input".into(), "A".repeat(64)],
        vec!["loop".into(), "/tmp/input".into()],
    ] {
        assert!(arguments(args).is_err());
    }
}
#[test]
fn source_profile_matches_public_cli_envelope_without_implicit_reuse() {
    let disabled = options(false).unwrap();
    assert!(disabled.allocation_reuse().is_none());
    let enabled = options(true).unwrap();
    assert_eq!(
        enabled
            .allocation_reuse()
            .unwrap()
            .max_cached_payload_bytes(),
        8192
    );
    let fe2o3_kir_debugger::RuntimeAllocationCaptureModeV1::Enabled(limits) = enabled.allocations()
    else {
        panic!()
    };
    assert_eq!(
        (
            limits.max_records(),
            limits.max_transitions(),
            limits.max_metadata_bytes(),
            limits.max_validation_work()
        ),
        (65536, 8192, 8 * 1024 * 1024, 1_000_000)
    );
}
#[test]
fn source_modes_refuse_non_bundle_bytes_before_any_execution() {
    assert!(report("loop", vec![0; 8]).is_err());
    assert!(report("workgroup", vec![0; 8]).is_err());
    assert!(report("unknown", vec![0; 8]).is_err());
}
