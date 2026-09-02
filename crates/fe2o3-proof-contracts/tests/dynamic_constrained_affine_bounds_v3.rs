use fe2o3_proof_contracts::{
    AffineInequalityV2, ConstrainedAffineBoundsCertificateV2, ConstrainedAffineBoundsQueryV2,
    DYNAMIC_AFFINE_COMPONENT_CEILING_V3, DynamicConstrainedAffineBoundsCertificateErrorV3,
    DynamicConstrainedAffineBoundsCertificateV3,
    check_dynamic_constrained_affine_bounds_certificate_v3,
};

fn certificate() -> DynamicConstrainedAffineBoundsCertificateV3 {
    let maximum = i128::from(u64::MAX);
    let constraints = vec![
        // gid < input_a.len(), gid < input_b.len(), gid < output.len().
        AffineInequalityV2::new(1, vec![1, -1, 0, 0]),
        AffineInequalityV2::new(1, vec![1, 0, -1, 0]),
        AffineInequalityV2::new(1, vec![1, 0, 0, -1]),
    ];
    let index_query = ConstrainedAffineBoundsQueryV2::new(
        vec![0, 0, 0, 0],
        vec![16, maximum + 1, maximum + 1, maximum + 1],
        constraints.clone(),
        0,
        vec![1, 0, 0, 0],
        DYNAMIC_AFFINE_COMPONENT_CEILING_V3,
    );
    let slack_query = ConstrainedAffineBoundsQueryV2::new(
        vec![0, 0, 0, 0],
        vec![16, maximum + 1, maximum + 1, maximum + 1],
        constraints,
        -1,
        vec![-1, 1, 0, 0],
        DYNAMIC_AFFINE_COMPONENT_CEILING_V3,
    );
    DynamicConstrainedAffineBoundsCertificateV3::new(
        0,
        vec![0, 1, 0, 0],
        ConstrainedAffineBoundsCertificateV2::new(
            index_query,
            vec![0, 1, 1, 1],
            vec![0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0],
            vec![0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0],
        ),
        ConstrainedAffineBoundsCertificateV2::new(
            slack_query,
            vec![0, 1, 1, 1],
            vec![1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            vec![0, 0, 0, 1, 0, 0, 1, 0, 0, 0, 0],
        ),
    )
}

#[test]
fn accepts_three_guard_per_allocation_runtime_extent_relation() {
    let certificate = certificate();
    let checked = check_dynamic_constrained_affine_bounds_certificate_v3(&certificate).unwrap();
    assert!(checked.establishes_nonempty_domain_and_dynamic_bound());
    assert_eq!(checked.certificate(), &certificate);
}

#[test]
fn rejects_extent_slack_and_domain_substitutions() {
    let original = certificate();
    let wrong_extent = DynamicConstrainedAffineBoundsCertificateV3::new(
        1,
        original.extent_coefficients().to_vec(),
        original.index_certificate().clone(),
        original.slack_certificate().clone(),
    );
    assert_eq!(
        check_dynamic_constrained_affine_bounds_certificate_v3(&wrong_extent).unwrap_err(),
        DynamicConstrainedAffineBoundsCertificateErrorV3::SlackConstantMismatch
    );

    let slack = original.slack_certificate();
    let different_domain = ConstrainedAffineBoundsCertificateV2::new(
        ConstrainedAffineBoundsQueryV2::new(
            vec![0, 0, 0, 0],
            vec![
                15,
                i128::from(u64::MAX) + 1,
                i128::from(u64::MAX) + 1,
                i128::from(u64::MAX) + 1,
            ],
            slack.query().constraints().to_vec(),
            slack.query().constant(),
            slack.query().coefficients().to_vec(),
            slack.query().extent(),
        ),
        slack.domain_witness().to_vec(),
        slack.lower_multipliers().to_vec(),
        slack.upper_multipliers().to_vec(),
    );
    let substituted = DynamicConstrainedAffineBoundsCertificateV3::new(
        original.extent_constant(),
        original.extent_coefficients().to_vec(),
        original.index_certificate().clone(),
        different_domain,
    );
    assert_eq!(
        check_dynamic_constrained_affine_bounds_certificate_v3(&substituted).unwrap_err(),
        DynamicConstrainedAffineBoundsCertificateErrorV3::DomainMismatch
    );
}

#[test]
fn rejects_tampered_witness_and_relation_overflow() {
    let original = certificate();
    let slack = original.slack_certificate();
    let bad_witness = ConstrainedAffineBoundsCertificateV2::new(
        slack.query().clone(),
        vec![8, 1, 1, 1],
        slack.lower_multipliers().to_vec(),
        slack.upper_multipliers().to_vec(),
    );
    let substituted = DynamicConstrainedAffineBoundsCertificateV3::new(
        original.extent_constant(),
        original.extent_coefficients().to_vec(),
        original.index_certificate().clone(),
        bad_witness,
    );
    assert!(matches!(
        check_dynamic_constrained_affine_bounds_certificate_v3(&substituted),
        Err(DynamicConstrainedAffineBoundsCertificateErrorV3::SlackCertificate(_))
    ));

    let overflow = DynamicConstrainedAffineBoundsCertificateV3::new(
        i128::MIN,
        original.extent_coefficients().to_vec(),
        original.index_certificate().clone(),
        original.slack_certificate().clone(),
    );
    assert_eq!(
        check_dynamic_constrained_affine_bounds_certificate_v3(&overflow).unwrap_err(),
        DynamicConstrainedAffineBoundsCertificateErrorV3::ArithmeticOverflow
    );
}

#[test]
fn rejects_contradictory_guard_domain() {
    let original = certificate();
    let rebuild = |component: &ConstrainedAffineBoundsCertificateV2| {
        let mut constraints = component.query().constraints().to_vec();
        // input_a.len() <= gid contradicts the retained gid < input_a.len().
        constraints.push(AffineInequalityV2::new(0, vec![-1, 1, 0, 0]));
        let mut lower = component.lower_multipliers().to_vec();
        lower.insert(3, 0);
        let mut upper = component.upper_multipliers().to_vec();
        upper.insert(3, 0);
        ConstrainedAffineBoundsCertificateV2::new(
            ConstrainedAffineBoundsQueryV2::new(
                component.query().lower().to_vec(),
                component.query().upper_exclusive().to_vec(),
                constraints,
                component.query().constant(),
                component.query().coefficients().to_vec(),
                component.query().extent(),
            ),
            component.domain_witness().to_vec(),
            lower,
            upper,
        )
    };
    let contradictory = DynamicConstrainedAffineBoundsCertificateV3::new(
        original.extent_constant(),
        original.extent_coefficients().to_vec(),
        rebuild(original.index_certificate()),
        rebuild(original.slack_certificate()),
    );
    assert!(matches!(
        check_dynamic_constrained_affine_bounds_certificate_v3(&contradictory),
        Err(DynamicConstrainedAffineBoundsCertificateErrorV3::IndexCertificate(_))
    ));
}

#[test]
fn rejects_duplicate_guard_rows_even_when_components_still_verify() {
    let original = certificate();
    let rebuild = |component: &ConstrainedAffineBoundsCertificateV2| {
        let mut constraints = component.query().constraints().to_vec();
        constraints.push(constraints[0].clone());
        let mut lower = component.lower_multipliers().to_vec();
        lower.insert(3, 0);
        let mut upper = component.upper_multipliers().to_vec();
        upper.insert(3, 0);
        ConstrainedAffineBoundsCertificateV2::new(
            ConstrainedAffineBoundsQueryV2::new(
                component.query().lower().to_vec(),
                component.query().upper_exclusive().to_vec(),
                constraints,
                component.query().constant(),
                component.query().coefficients().to_vec(),
                component.query().extent(),
            ),
            component.domain_witness().to_vec(),
            lower,
            upper,
        )
    };
    let duplicate = DynamicConstrainedAffineBoundsCertificateV3::new(
        original.extent_constant(),
        original.extent_coefficients().to_vec(),
        rebuild(original.index_certificate()),
        rebuild(original.slack_certificate()),
    );
    assert_eq!(
        check_dynamic_constrained_affine_bounds_certificate_v3(&duplicate).unwrap_err(),
        DynamicConstrainedAffineBoundsCertificateErrorV3::DuplicateConstraint { second: 3 }
    );
}
