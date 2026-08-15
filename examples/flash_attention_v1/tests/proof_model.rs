use fe2o3_flash_attention_v1::{
    MODEL_ASSURANCE_V1, OnlineInvariantViolationV1, OnlineStateV1, OwnershipViolationV1,
    SOURCE_MODEL_REFINEMENT_PROVED_V1, access_coordinate_v1, deterministic_vectors_v1,
    exact_ownership_map_v1, online_trace_v1, validate_online_state_v1,
    validate_output_ownership_v1,
};

fn scores_for_row(q: &[f32], k: &[f32], query_row: usize) -> Vec<f64> {
    (0..=query_row)
        .map(|key_row| {
            (0..16)
                .map(|feature| {
                    f64::from(q[query_row * 16 + feature]) * f64::from(k[key_row * 16 + feature])
                })
                .sum::<f64>()
                * 0.25
        })
        .collect()
}

#[test]
fn assurance_status_does_not_claim_a_refinement_proof() {
    assert!(!std::hint::black_box(SOURCE_MODEL_REFINEMENT_PROVED_V1));
    assert!(MODEL_ASSURANCE_V1.contains("not a machine-checked refinement proof"));
}

#[test]
fn every_corpus_prefix_satisfies_online_max_sum_and_numerator_invariants() {
    for vector in deterministic_vectors_v1() {
        for query_row in 0..8 {
            let scores = scores_for_row(&vector.q, &vector.k, query_row);
            for output_column in 0..16 {
                let values: Vec<_> = (0..=query_row)
                    .map(|key_row| f64::from(vector.v[key_row * 16 + output_column]))
                    .collect();
                let trace = online_trace_v1(&scores, &values).unwrap();
                assert_eq!(trace.len(), query_row + 1);
                for prefix in 1..=trace.len() {
                    validate_online_state_v1(
                        &scores[..prefix],
                        &values[..prefix],
                        trace[prefix - 1],
                    )
                    .unwrap_or_else(|error| {
                        panic!(
                            "{} row {query_row} column {output_column} prefix {prefix}: {error:?}",
                            vector.name
                        )
                    });
                }
            }
        }
    }
}

#[test]
fn hostile_online_state_mutations_are_detected() {
    let scores = [-3.0, 2.0, 1.0, 5.0];
    let values = [7.0, -2.0, 4.0, 9.0];
    let state = *online_trace_v1(&scores, &values).unwrap().last().unwrap();
    assert_eq!(validate_online_state_v1(&scores, &values, state), Ok(()));

    let mut wrong = state;
    wrong.consumed_keys -= 1;
    assert_eq!(
        validate_online_state_v1(&scores, &values, wrong),
        Err(OnlineInvariantViolationV1::PrefixLength)
    );

    let mut wrong = state;
    wrong.maximum = 4.0;
    assert_eq!(
        validate_online_state_v1(&scores, &values, wrong),
        Err(OnlineInvariantViolationV1::Maximum)
    );

    let mut wrong = state;
    wrong.denominator += 0.25;
    assert_eq!(
        validate_online_state_v1(&scores, &values, wrong),
        Err(OnlineInvariantViolationV1::Denominator)
    );

    let mut wrong = state;
    wrong.numerator -= 0.25;
    assert_eq!(
        validate_online_state_v1(&scores, &values, wrong),
        Err(OnlineInvariantViolationV1::Numerator)
    );
}

#[test]
fn access_model_proves_bounded_causal_reads_and_exact_writes_by_exhaustion() {
    for lane in 0..64 {
        for key_row in 0..8 {
            for feature in 0..16 {
                for output_slot in 0..2 {
                    let access = access_coordinate_v1(lane, key_row, feature, output_slot);
                    let query_row = (2 * lane) / 16;
                    if key_row <= query_row {
                        let access = access.expect("causal access is modeled");
                        assert_eq!(access.query_row, query_row);
                        assert!(access.q_index < 128);
                        assert!(access.k_index < 128);
                        assert!(access.v_index < 128);
                        assert!(access.output_index < 128);
                        assert_eq!(access.output_index, 2 * lane + output_slot);
                    } else {
                        assert_eq!(access, None);
                    }
                }
            }
        }
    }

    assert_eq!(access_coordinate_v1(64, 0, 0, 0), None);
    assert_eq!(access_coordinate_v1(0, 0, 16, 0), None);
    assert_eq!(access_coordinate_v1(0, 0, 0, 2), None);
    assert_eq!(access_coordinate_v1(0, 8, 0, 0), None);
}

#[test]
fn ownership_model_is_total_and_hostile_maps_fail_closed() {
    let exact = exact_ownership_map_v1();
    assert_eq!(validate_output_ownership_v1(&exact), Ok(()));

    assert_eq!(
        validate_output_ownership_v1(&exact[..63]),
        Err(OwnershipViolationV1::WrongLaneCount)
    );

    let mut wrong = exact;
    wrong[63][1] = 128;
    assert_eq!(
        validate_output_ownership_v1(&wrong),
        Err(OwnershipViolationV1::OutOfBounds {
            lane: 63,
            index: 128,
        })
    );

    let mut wrong = exact;
    wrong[17][0] = wrong[16][1];
    assert_eq!(
        validate_output_ownership_v1(&wrong),
        Err(OwnershipViolationV1::DuplicateWriter {
            index: wrong[16][1],
        })
    );
}

#[test]
fn validator_rejects_non_profile_model_inputs() {
    assert!(online_trace_v1(&[], &[]).is_err());
    assert!(online_trace_v1(&[0.0], &[]).is_err());
    assert!(online_trace_v1(&[0.0; 9], &[0.0; 9]).is_err());
    assert!(online_trace_v1(&[f64::NAN], &[0.0]).is_err());
    assert!(online_trace_v1(&[0.0], &[f64::INFINITY]).is_err());

    let structurally_wrong = OnlineStateV1 {
        consumed_keys: 0,
        maximum: 0.0,
        denominator: 1.0,
        numerator: 0.0,
    };
    assert_eq!(
        validate_online_state_v1(&[0.0], &[0.0], structurally_wrong),
        Err(OnlineInvariantViolationV1::PrefixLength)
    );
}
