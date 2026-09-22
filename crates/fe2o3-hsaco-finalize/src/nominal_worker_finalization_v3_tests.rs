use super::*;

#[test]
fn manifest_comparison_is_prepaid_before_equality_or_mismatch() {
    for (left, right) in [
        (b"manifest".as_slice(), b"manifest".as_slice()),
        (b"x", b"manifest"),
    ] {
        let mut calls = Vec::new();
        let error = check_export_manifest(left, right, &mut |n| {
            calls.push(n);
            Err("denied")
        })
        .unwrap_err();
        assert_eq!(calls, [9]);
        assert!(matches!(
            error,
            NominalWorkerFinalizationErrorV3::Finalization(NominalFinalizationErrorV3::Work(
                "denied"
            ))
        ));
    }
}
