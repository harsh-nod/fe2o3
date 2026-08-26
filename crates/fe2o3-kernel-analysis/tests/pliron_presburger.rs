use fe2o3_kernel_analysis::{
    MAX_PRESBURGER_WORK_UNITS_V1, PlironPresburgerAnalysisV1, PresburgerAffineExprV1,
    PresburgerBoxV1, PresburgerCollisionDecisionV1, PresburgerConstraintV1,
    PresburgerCoverageDecisionV1, PresburgerEquivalenceDecisionV1, PresburgerFailureV1,
    PresburgerFiniteImageV1, PresburgerMapExprV1, PresburgerMapV1, PresburgerRangeDecisionV1,
    PresburgerSetDecisionV1, PresburgerSetV1,
};

fn affine(constant: i128, coefficients: &[i128]) -> PresburgerAffineExprV1 {
    PresburgerAffineExprV1::new(constant, coefficients.to_vec()).unwrap()
}

fn map(extents: &[u64], outputs: Vec<PresburgerMapExprV1>) -> PresburgerMapV1 {
    PresburgerMapV1::new(
        PresburgerSetV1::box_only(PresburgerBoxV1::zero_based(extents).unwrap()),
        outputs,
    )
    .unwrap()
}

#[test]
fn signed_affine_equalities_return_an_exact_witness() {
    let set = PresburgerSetV1::new(
        PresburgerBoxV1::new(vec![-4, 0], vec![5, 8]).unwrap(),
        vec![PresburgerConstraintV1::EqualZero(affine(-1, &[1, -1]))],
    )
    .unwrap();

    let PresburgerSetDecisionV1::Witness(witness) = set.find_witness() else {
        panic!("expected a witness for x - y - 1 = 0");
    };
    assert_eq!(witness.point()[0] - witness.point()[1], 1);
}

#[test]
fn contradictory_affine_inequalities_prove_the_set_empty() {
    let set = PresburgerSetV1::new(
        PresburgerBoxV1::zero_based(&[16]).unwrap(),
        vec![
            // x <= 3
            PresburgerConstraintV1::LessEqualZero(affine(-3, &[1])),
            // x >= 8, encoded as 8 - x <= 0
            PresburgerConstraintV1::LessEqualZero(affine(8, &[-1])),
        ],
    )
    .unwrap();

    assert_eq!(set.find_witness(), PresburgerSetDecisionV1::Empty);
}

#[test]
fn congruence_constraints_model_lane_and_bank_partitions() {
    let set = PresburgerSetV1::new(
        PresburgerBoxV1::zero_based(&[16]).unwrap(),
        vec![PresburgerConstraintV1::CongruentZero {
            expression: affine(-3, &[1]),
            modulus: 4,
        }],
    )
    .unwrap();

    let PresburgerSetDecisionV1::Witness(witness) = set.find_witness() else {
        panic!("expected x congruent to 3 modulo 4");
    };
    assert_eq!(witness.point()[0], 3);
}

#[test]
fn affine_bounds_query_returns_the_failing_invocation() {
    let access = map(&[8], vec![PresburgerMapExprV1::Affine(affine(1, &[2]))]);
    assert_eq!(
        access.find_out_of_bounds(&[12]),
        PresburgerRangeDecisionV1::Counterexample {
            domain: vec![6],
            range: vec![13],
        }
    );
}

#[test]
fn guarded_edge_domain_proves_an_affine_access_safe() {
    let guarded = PresburgerSetV1::new(
        PresburgerBoxV1::zero_based(&[8]).unwrap(),
        vec![PresburgerConstraintV1::LessEqualZero(affine(-5, &[1]))],
    )
    .unwrap();
    let access =
        PresburgerMapV1::new(guarded, vec![PresburgerMapExprV1::Affine(affine(1, &[2]))]).unwrap();
    assert_eq!(
        access.find_out_of_bounds(&[12]),
        PresburgerRangeDecisionV1::Proved
    );
}

#[test]
fn modulo_ownership_reports_a_duplicate_owner() {
    let owners = map(
        &[8],
        vec![PresburgerMapExprV1::Remainder {
            dividend: affine(0, &[1]),
            modulus: 4,
        }],
    );
    assert_eq!(
        owners.find_collision(),
        PresburgerCollisionDecisionV1::Counterexample {
            first: vec![0],
            second: vec![4],
            range: vec![0],
        }
    );
}

#[test]
fn affine_stride_is_an_injective_ownership_map() {
    let owners = map(&[256], vec![PresburgerMapExprV1::Affine(affine(7, &[4]))]);
    assert_eq!(
        owners.find_collision(),
        PresburgerCollisionDecisionV1::Proved
    );
}

#[test]
fn cross_effect_query_finds_a_race_between_distinct_invocations() {
    let writes = map(&[8], vec![PresburgerMapExprV1::Affine(affine(0, &[2]))]);
    let reads = map(&[8], vec![PresburgerMapExprV1::Affine(affine(2, &[2]))]);
    assert_eq!(
        writes.find_cross_collision(&reads, true),
        PresburgerCollisionDecisionV1::Counterexample {
            first: vec![1],
            second: vec![0],
            range: vec![2],
        }
    );
}

#[test]
fn disjoint_even_and_odd_effects_prove_race_freedom() {
    let even = map(&[8], vec![PresburgerMapExprV1::Affine(affine(0, &[2]))]);
    let odd = map(&[8], vec![PresburgerMapExprV1::Affine(affine(1, &[2]))]);
    assert_eq!(
        even.find_cross_collision(&odd, true),
        PresburgerCollisionDecisionV1::Proved
    );
}

#[test]
fn floor_divided_grid_reports_the_uncovered_tail() {
    let owners = map(&[3], vec![PresburgerMapExprV1::Affine(affine(0, &[1]))]);
    assert_eq!(
        owners.find_uncovered(&[4]),
        PresburgerCoverageDecisionV1::Hole { point: vec![3] }
    );
}

#[test]
fn multidimensional_tile_map_proves_total_coverage() {
    let owners = map(
        &[2, 3],
        vec![
            PresburgerMapExprV1::Affine(affine(0, &[1, 0])),
            PresburgerMapExprV1::Affine(affine(0, &[0, 1])),
        ],
    );
    assert_eq!(
        owners.find_uncovered(&[2, 3]),
        PresburgerCoverageDecisionV1::Proved
    );
}

#[test]
fn traced_finite_image_uses_the_same_box_coverage_query() {
    let image = PresburgerFiniteImageV1::new(2, [vec![0, 0], vec![0, 1], vec![1, 0]]).unwrap();
    assert_eq!(
        image.find_uncovered(&[2, 2]),
        PresburgerCoverageDecisionV1::Hole { point: vec![1, 1] }
    );
}

#[test]
fn malformed_relations_are_rejected_before_search() {
    assert_eq!(
        PresburgerBoxV1::new(vec![0], vec![1, 2]),
        Err(PresburgerFailureV1::InvalidModel {
            detail: "box bound vectors have different ranks",
        })
    );
    assert!(matches!(
        PresburgerSetV1::new(
            PresburgerBoxV1::zero_based(&[4]).unwrap(),
            vec![PresburgerConstraintV1::CongruentZero {
                expression: affine(0, &[1]),
                modulus: 0,
            }],
        ),
        Err(PresburgerFailureV1::InvalidModel { .. })
    ));
}

#[test]
fn layout_coordinate_comparison_proposes_a_concrete_mismatch() {
    let row_major = map(
        &[2, 3],
        vec![
            PresburgerMapExprV1::Affine(affine(0, &[1, 0])),
            PresburgerMapExprV1::Affine(affine(0, &[0, 1])),
        ],
    );
    let transposed = map(
        &[2, 3],
        vec![
            PresburgerMapExprV1::Affine(affine(0, &[0, 1])),
            PresburgerMapExprV1::Affine(affine(0, &[1, 0])),
        ],
    );
    assert_eq!(
        row_major.find_mismatch(&transposed),
        PresburgerEquivalenceDecisionV1::Counterexample {
            domain: vec![0, 1],
            first: vec![0, 1],
            second: vec![1, 0],
        }
    );
}

#[test]
fn equivalent_layout_formulas_are_proved_pointwise() {
    let first = map(&[64], vec![PresburgerMapExprV1::Affine(affine(3, &[4]))]);
    let second = map(
        &[64],
        vec![PresburgerMapExprV1::Affine(
            affine(1, &[2]).checked_add(&affine(2, &[2])).unwrap(),
        )],
    );
    assert_eq!(
        first.find_mismatch(&second),
        PresburgerEquivalenceDecisionV1::Proved
    );
}

#[test]
fn invalid_loop_domain_is_empty_and_valid_domain_has_a_phase() {
    let invalid = PresburgerSetV1::new(
        PresburgerBoxV1::zero_based(&[8]).unwrap(),
        vec![
            PresburgerConstraintV1::LessEqualZero(affine(-2, &[1])),
            PresburgerConstraintV1::LessEqualZero(affine(5, &[-1])),
        ],
    )
    .unwrap();
    assert_eq!(invalid.find_witness(), PresburgerSetDecisionV1::Empty);

    let valid = PresburgerSetV1::new(
        PresburgerBoxV1::zero_based(&[8]).unwrap(),
        vec![
            PresburgerConstraintV1::LessEqualZero(affine(-5, &[1])),
            PresburgerConstraintV1::LessEqualZero(affine(2, &[-1])),
        ],
    )
    .unwrap();
    assert!(matches!(
        valid.find_witness(),
        PresburgerSetDecisionV1::Witness(_)
    ));
}

#[test]
fn arithmetic_overflow_is_never_treated_as_a_proof() {
    let expression = affine(i128::MAX, &[1]);
    assert_eq!(
        expression.evaluate(&[1]),
        Err(PresburgerFailureV1::ArithmeticOverflow)
    );
}

#[test]
fn resource_exhaustion_is_explicitly_incomplete() {
    let huge = PresburgerSetV1::new(
        PresburgerBoxV1::zero_based(&[MAX_PRESBURGER_WORK_UNITS_V1 as u64]).unwrap(),
        vec![PresburgerConstraintV1::CongruentZero {
            expression: affine(1, &[0]),
            modulus: 2,
        }],
    )
    .unwrap();
    assert!(matches!(
        huge.find_witness(),
        PresburgerSetDecisionV1::Incomplete(PresburgerFailureV1::ResourceLimit { .. })
    ));
}

#[test]
fn dynamic_launch_without_a_finite_bound_is_unsupported() {
    let analysis = PlironPresburgerAnalysisV1::for_launch_extents(vec![0]);
    assert_eq!(
        analysis.map_for_facts(&[]),
        Err(PresburgerFailureV1::Unsupported {
            detail: "a dynamic launch extent has no finite compiler bound",
        })
    );
}
