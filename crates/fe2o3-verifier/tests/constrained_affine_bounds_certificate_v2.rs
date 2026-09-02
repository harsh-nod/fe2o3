use fe2o3_proof_contracts::{
    AffineInequalityV2, ConstrainedAffineBoundsCertificateErrorV2,
    ConstrainedAffineBoundsCertificateV2, ConstrainedAffineBoundsQueryV2,
};
use fe2o3_verifier::{
    CONSTRAINED_AFFINE_BOUNDS_PROOF_SOURCE_SHA256_V2,
    CONSTRAINED_AFFINE_BOUNDS_VERUS_CLOSURE_MANIFEST_SHA256_V2,
    CONSTRAINED_AFFINE_BOUNDS_VERUS_EXECUTABLE_SHA256_V2,
    verify_compiler_constrained_affine_bounds_certificate_v2,
};

fn certificate() -> ConstrainedAffineBoundsCertificateV2 {
    ConstrainedAffineBoundsCertificateV2::new(
        ConstrainedAffineBoundsQueryV2::new(
            vec![0],
            vec![16],
            vec![AffineInequalityV2::new(-7, vec![1])],
            0,
            vec![1],
            8,
        ),
        vec![0],
        vec![0, 1, 0],
        vec![1, 0, 0],
    )
}

#[test]
fn verifier_rechecks_the_universal_theorem_and_exact_proof_binding() {
    let certificate = certificate();
    let verified = verify_compiler_constrained_affine_bounds_certificate_v2(&certificate).unwrap();
    assert_eq!(verified.certificate(), &certificate);
    assert!(verified.establishes_nonempty_constrained_domain());
    assert!(verified.establishes_nonnegative_strict_upper_bound());
    assert!(!verified.grants_compiler_refinement_authority());
    assert!(!verified.grants_artifact_or_launch_authority());
    let binding = verified.proof_binding();
    assert_eq!(
        binding.proof_source_sha256(),
        CONSTRAINED_AFFINE_BOUNDS_PROOF_SOURCE_SHA256_V2
    );
    assert_eq!(
        binding.verus_executable_sha256(),
        CONSTRAINED_AFFINE_BOUNDS_VERUS_EXECUTABLE_SHA256_V2
    );
    assert_eq!(
        binding.verus_closure_manifest_sha256(),
        CONSTRAINED_AFFINE_BOUNDS_VERUS_CLOSURE_MANIFEST_SHA256_V2
    );
    assert!(binding.proof_source_sha256().iter().any(|byte| *byte != 0));
    assert!(
        binding
            .verus_executable_sha256()
            .iter()
            .any(|byte| *byte != 0)
    );
    assert!(
        binding
            .verus_closure_manifest_sha256()
            .iter()
            .any(|byte| *byte != 0)
    );
}

#[test]
fn verifier_rejects_extent_map_constraint_and_witness_mutations() {
    let original = certificate();
    let make = |query, witness| {
        ConstrainedAffineBoundsCertificateV2::new(
            query,
            witness,
            original.lower_multipliers().to_vec(),
            original.upper_multipliers().to_vec(),
        )
    };
    let q = original.query();
    let extent = make(
        ConstrainedAffineBoundsQueryV2::new(
            q.lower().to_vec(),
            q.upper_exclusive().to_vec(),
            q.constraints().to_vec(),
            q.constant(),
            q.coefficients().to_vec(),
            7,
        ),
        vec![0],
    );
    assert_eq!(
        verify_compiler_constrained_affine_bounds_certificate_v2(&extent)
            .unwrap_err()
            .source_kind(),
        ConstrainedAffineBoundsCertificateErrorV2::UpperConstantNotDominated
    );
    let map = make(
        ConstrainedAffineBoundsQueryV2::new(
            q.lower().to_vec(),
            q.upper_exclusive().to_vec(),
            q.constraints().to_vec(),
            q.constant(),
            vec![2],
            q.extent(),
        ),
        vec![0],
    );
    assert!(verify_compiler_constrained_affine_bounds_certificate_v2(&map).is_err());
    let constraint = make(
        ConstrainedAffineBoundsQueryV2::new(
            q.lower().to_vec(),
            q.upper_exclusive().to_vec(),
            vec![AffineInequalityV2::new(-8, vec![1])],
            q.constant(),
            q.coefficients().to_vec(),
            q.extent(),
        ),
        vec![0],
    );
    assert!(verify_compiler_constrained_affine_bounds_certificate_v2(&constraint).is_err());
    let witness = make(q.clone(), vec![15]);
    assert_eq!(
        verify_compiler_constrained_affine_bounds_certificate_v2(&witness)
            .unwrap_err()
            .source_kind(),
        ConstrainedAffineBoundsCertificateErrorV2::PointViolatesConstraint { constraint: 0 }
    );
}
