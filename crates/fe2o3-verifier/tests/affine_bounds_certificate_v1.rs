use fe2o3_proof_contracts::{
    AffineBoundsCertificateErrorV1, AffineBoundsCertificateV1, AffineBoundsQueryV1,
};
use fe2o3_verifier::verify_compiler_affine_bounds_certificate_v1;

fn certificate(extent: u64) -> AffineBoundsCertificateV1 {
    AffineBoundsCertificateV1::new(
        AffineBoundsQueryV1::new(vec![0, 0], vec![8, 4], 1, vec![4, 1], extent),
        vec![0, 0],
        vec![7, 3],
        1,
        32,
    )
}

#[test]
fn verifier_rechecks_and_retains_the_exact_local_theorem() {
    let certificate = certificate(33);
    let verified = verify_compiler_affine_bounds_certificate_v1(&certificate).unwrap();
    assert_eq!(verified.certificate(), &certificate);
    assert!(verified.establishes_nonnegative_strict_upper_bound());
    assert!(!verified.grants_compiler_refinement_authority());
    assert!(!verified.grants_artifact_or_launch_authority());
}

#[test]
fn verifier_rejects_a_tightened_extent_even_with_unchanged_extrema() {
    let error = verify_compiler_affine_bounds_certificate_v1(&certificate(32)).unwrap_err();
    assert_eq!(
        error.source_kind(),
        AffineBoundsCertificateErrorV1::BoundNotEstablished
    );
}
