use fe2o3_proof_contracts::{
    AffineBoundsCertificateErrorV1, AffineBoundsCertificateV1, AffineBoundsQueryV1,
    check_affine_bounds_certificate_v1,
};

fn query(extent: u64) -> AffineBoundsQueryV1 {
    AffineBoundsQueryV1::new(vec![-2, 1], vec![4, 5], 20, vec![3, -2], extent)
}

fn certificate(extent: u64) -> AffineBoundsCertificateV1 {
    AffineBoundsCertificateV1::new(query(extent), vec![-2, 4], vec![3, 1], 6, 27)
}

#[test]
fn checked_endpoints_bound_every_point_in_the_formal_box_semantics() {
    let certificate = certificate(28);
    let checked = check_affine_bounds_certificate_v1(&certificate).unwrap();
    assert!(checked.establishes_nonnegative_strict_upper_bound());
    assert!(!checked.grants_compiler_refinement_authority());
    assert!(!certificate.authenticates_producer());
    assert!(!certificate.grants_lowering_or_launch_authority());

    for x in -2..4 {
        for y in 1..5 {
            let value = certificate.query().evaluate(&[x, y]).unwrap();
            assert!((0..28).contains(&value));
        }
    }
}

#[test]
fn endpoint_value_extent_and_shape_mutations_fail_closed() {
    let cases = [
        (
            AffineBoundsCertificateV1::new(query(28), vec![-1, 4], vec![3, 1], 6, 27),
            AffineBoundsCertificateErrorV1::MinimumEndpointMismatch { dimension: 0 },
        ),
        (
            AffineBoundsCertificateV1::new(query(28), vec![-2, 4], vec![2, 1], 6, 27),
            AffineBoundsCertificateErrorV1::MaximumEndpointMismatch { dimension: 0 },
        ),
        (
            AffineBoundsCertificateV1::new(query(28), vec![-2, 4], vec![3, 1], 7, 27),
            AffineBoundsCertificateErrorV1::MinimumMismatch,
        ),
        (
            AffineBoundsCertificateV1::new(query(28), vec![-2, 4], vec![3, 1], 6, 26),
            AffineBoundsCertificateErrorV1::MaximumMismatch,
        ),
        (
            certificate(27),
            AffineBoundsCertificateErrorV1::BoundNotEstablished,
        ),
        (
            AffineBoundsCertificateV1::new(query(28), vec![-2], vec![3], 6, 27),
            AffineBoundsCertificateErrorV1::EndpointRankMismatch,
        ),
    ];
    for (mutated, expected) in cases {
        assert_eq!(
            check_affine_bounds_certificate_v1(&mutated).unwrap_err(),
            expected
        );
    }
}

#[test]
fn invalid_domains_ranks_and_arithmetic_never_become_proofs() {
    let zero_extent = AffineBoundsCertificateV1::new(
        AffineBoundsQueryV1::new(vec![], vec![], 0, vec![], 0),
        vec![],
        vec![],
        0,
        0,
    );
    assert_eq!(
        check_affine_bounds_certificate_v1(&zero_extent).unwrap_err(),
        AffineBoundsCertificateErrorV1::ZeroExtent
    );

    let empty = AffineBoundsCertificateV1::new(
        AffineBoundsQueryV1::new(vec![4], vec![4], 0, vec![1], 8),
        vec![4],
        vec![4],
        4,
        4,
    );
    assert_eq!(
        check_affine_bounds_certificate_v1(&empty).unwrap_err(),
        AffineBoundsCertificateErrorV1::EmptyDomainDimension { dimension: 0 }
    );

    let wrong_rank = AffineBoundsCertificateV1::new(
        AffineBoundsQueryV1::new(vec![0], vec![1, 2], 0, vec![1], 8),
        vec![0],
        vec![0],
        0,
        0,
    );
    assert_eq!(
        check_affine_bounds_certificate_v1(&wrong_rank).unwrap_err(),
        AffineBoundsCertificateErrorV1::RankMismatch
    );

    let excessive_rank = AffineBoundsCertificateV1::new(
        AffineBoundsQueryV1::new(vec![0; 17], vec![1; 17], 0, vec![0; 17], 1),
        vec![0; 17],
        vec![0; 17],
        0,
        0,
    );
    assert_eq!(
        check_affine_bounds_certificate_v1(&excessive_rank).unwrap_err(),
        AffineBoundsCertificateErrorV1::RankLimitExceeded {
            actual: 17,
            limit: 16,
        }
    );

    let overflow = AffineBoundsCertificateV1::new(
        AffineBoundsQueryV1::new(vec![1], vec![2], i128::MAX, vec![1], u64::MAX),
        vec![1],
        vec![1],
        i128::MAX,
        i128::MAX,
    );
    assert_eq!(
        check_affine_bounds_certificate_v1(&overflow).unwrap_err(),
        AffineBoundsCertificateErrorV1::ArithmeticOverflow
    );
}
