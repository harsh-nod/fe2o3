use fe2o3_proof_contracts::{
    AffineInequalityV2, ConstrainedAffineBoundsCertificateErrorV2,
    ConstrainedAffineBoundsCertificateV2, ConstrainedAffineBoundsQueryV2,
    MAX_CONSTRAINED_AFFINE_MULTIPLIER_V2, check_constrained_affine_bounds_certificate_v2,
};

fn guarded_certificate(extent: u64) -> ConstrainedAffineBoundsCertificateV2 {
    // Domain permits x + y == 14, but the retained path constraint admits only
    // x + y <= 9. The upper proof therefore depends on the constraint row.
    let query = ConstrainedAffineBoundsQueryV2::new(
        vec![0, 0],
        vec![8, 8],
        vec![AffineInequalityV2::new(-9, vec![1, 1])],
        0,
        vec![1, 1],
        extent,
    );
    // Rows: constraint, x lower, x upper, y lower, y upper.
    ConstrainedAffineBoundsCertificateV2::new(
        query,
        vec![0, 0],
        vec![0, 1, 0, 1, 0],
        vec![1, 0, 0, 0, 0],
    )
}

#[test]
fn constrained_row_proves_a_bound_that_the_box_does_not() {
    let certificate = guarded_certificate(10);
    let checked = check_constrained_affine_bounds_certificate_v2(&certificate).unwrap();
    assert_eq!(checked.certificate(), &certificate);
    assert!(checked.establishes_nonnegative_strict_upper_bound());
    assert!(!checked.grants_compiler_refinement_authority());
    assert_eq!(certificate.query().evaluate(&[7, 2]).unwrap(), 9);
    assert_eq!(
        certificate.query().evaluate(&[7, 7]),
        Err(ConstrainedAffineBoundsCertificateErrorV2::PointViolatesConstraint { constraint: 0 })
    );
}

#[test]
fn rejects_extent_constraint_map_and_multiplier_mutations() {
    assert_eq!(
        check_constrained_affine_bounds_certificate_v2(&guarded_certificate(9)).unwrap_err(),
        ConstrainedAffineBoundsCertificateErrorV2::UpperConstantNotDominated
    );

    let wrong_constraint = ConstrainedAffineBoundsCertificateV2::new(
        ConstrainedAffineBoundsQueryV2::new(
            vec![0, 0],
            vec![8, 8],
            vec![AffineInequalityV2::new(-10, vec![1, 1])],
            0,
            vec![1, 1],
            10,
        ),
        vec![0, 0],
        vec![0, 1, 0, 1, 0],
        vec![1, 0, 0, 0, 0],
    );
    assert_eq!(
        check_constrained_affine_bounds_certificate_v2(&wrong_constraint).unwrap_err(),
        ConstrainedAffineBoundsCertificateErrorV2::UpperConstantNotDominated
    );

    let wrong_map = ConstrainedAffineBoundsCertificateV2::new(
        ConstrainedAffineBoundsQueryV2::new(
            vec![0, 0],
            vec![8, 8],
            vec![AffineInequalityV2::new(-9, vec![1, 1])],
            0,
            vec![2, 1],
            10,
        ),
        vec![0, 0],
        vec![0, 2, 0, 1, 0],
        vec![1, 0, 0, 0, 0],
    );
    assert_eq!(
        check_constrained_affine_bounds_certificate_v2(&wrong_map).unwrap_err(),
        ConstrainedAffineBoundsCertificateErrorV2::UpperCoefficientMismatch { dimension: 0 }
    );

    let excessive = ConstrainedAffineBoundsCertificateV2::new(
        guarded_certificate(10).query().clone(),
        vec![0, 0],
        vec![MAX_CONSTRAINED_AFFINE_MULTIPLIER_V2 + 1, 1, 0, 1, 0],
        vec![1, 0, 0, 0, 0],
    );
    assert_eq!(
        check_constrained_affine_bounds_certificate_v2(&excessive).unwrap_err(),
        ConstrainedAffineBoundsCertificateErrorV2::MultiplierLimitExceeded { row: 0 }
    );
}

#[test]
fn rejects_empty_malformed_and_overflowing_queries() {
    let empty = ConstrainedAffineBoundsCertificateV2::new(
        ConstrainedAffineBoundsQueryV2::new(vec![0], vec![0], vec![], 0, vec![1], 1),
        vec![0],
        vec![1, 0],
        vec![0, 1],
    );
    assert_eq!(
        check_constrained_affine_bounds_certificate_v2(&empty).unwrap_err(),
        ConstrainedAffineBoundsCertificateErrorV2::EmptyDomainDimension { dimension: 0 }
    );

    let malformed = ConstrainedAffineBoundsCertificateV2::new(
        ConstrainedAffineBoundsQueryV2::new(
            vec![0],
            vec![2],
            vec![AffineInequalityV2::new(0, vec![1, 2])],
            0,
            vec![1],
            2,
        ),
        vec![0],
        vec![0; 3],
        vec![0; 3],
    );
    assert_eq!(
        check_constrained_affine_bounds_certificate_v2(&malformed).unwrap_err(),
        ConstrainedAffineBoundsCertificateErrorV2::ConstraintRankMismatch { constraint: 0 }
    );

    let overflow = ConstrainedAffineBoundsCertificateV2::new(
        ConstrainedAffineBoundsQueryV2::new(
            vec![i128::MIN],
            vec![i128::MIN + 1],
            vec![],
            0,
            vec![1],
            1,
        ),
        vec![i128::MIN],
        vec![1, 0],
        vec![0, 1],
    );
    assert_eq!(
        check_constrained_affine_bounds_certificate_v2(&overflow).unwrap_err(),
        ConstrainedAffineBoundsCertificateErrorV2::ArithmeticOverflow
    );
}

#[test]
fn contradictory_constraints_cannot_create_a_vacuous_certificate() {
    let contradictory = ConstrainedAffineBoundsCertificateV2::new(
        ConstrainedAffineBoundsQueryV2::new(
            vec![0],
            vec![8],
            vec![
                AffineInequalityV2::new(-1, vec![1]),
                AffineInequalityV2::new(2, vec![-1]),
            ],
            i128::MAX,
            vec![0],
            1,
        ),
        vec![0],
        vec![0; 4],
        vec![0; 4],
    );
    assert_eq!(
        check_constrained_affine_bounds_certificate_v2(&contradictory).unwrap_err(),
        ConstrainedAffineBoundsCertificateErrorV2::PointViolatesConstraint { constraint: 1 }
    );
}

#[test]
fn substituted_domain_witness_is_rejected() {
    let original = guarded_certificate(10);
    let substituted = ConstrainedAffineBoundsCertificateV2::new(
        original.query().clone(),
        vec![7, 7],
        original.lower_multipliers().to_vec(),
        original.upper_multipliers().to_vec(),
    );
    assert_eq!(
        check_constrained_affine_bounds_certificate_v2(&substituted).unwrap_err(),
        ConstrainedAffineBoundsCertificateErrorV2::PointViolatesConstraint { constraint: 0 }
    );
}

#[test]
fn substituted_constraint_order_is_rejected() {
    let original = ConstrainedAffineBoundsCertificateV2::new(
        ConstrainedAffineBoundsQueryV2::new(
            vec![0, 0],
            vec![8, 8],
            vec![
                AffineInequalityV2::new(-9, vec![1, 1]),
                AffineInequalityV2::new(-14, vec![1, 1]),
            ],
            0,
            vec![1, 1],
            10,
        ),
        vec![0, 0],
        vec![0, 0, 1, 0, 1, 0],
        vec![1, 0, 0, 0, 0, 0],
    );
    check_constrained_affine_bounds_certificate_v2(&original).unwrap();
    let reordered = ConstrainedAffineBoundsCertificateV2::new(
        ConstrainedAffineBoundsQueryV2::new(
            original.query().lower().to_vec(),
            original.query().upper_exclusive().to_vec(),
            original
                .query()
                .constraints()
                .iter()
                .rev()
                .cloned()
                .collect(),
            original.query().constant(),
            original.query().coefficients().to_vec(),
            original.query().extent(),
        ),
        original.domain_witness().to_vec(),
        original.lower_multipliers().to_vec(),
        original.upper_multipliers().to_vec(),
    );
    assert_eq!(
        check_constrained_affine_bounds_certificate_v2(&reordered).unwrap_err(),
        ConstrainedAffineBoundsCertificateErrorV2::UpperConstantNotDominated
    );
}
