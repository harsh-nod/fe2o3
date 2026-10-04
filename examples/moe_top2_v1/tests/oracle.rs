use fe2o3_moe_top2_v1::{
    DROP_ROUTE_V1, MOE_EXPERTS_V1, MOE_ROUTES_V1, MoeOracleErrorV1, RoutingOutputsV1,
    deterministic_vectors_v1, moe_top2_oracle_v1,
};

fn route(logits: &[f32]) -> RoutingOutputsV1 {
    let mut output = RoutingOutputsV1::filled(0xabab_abab);
    moe_top2_oracle_v1(logits, &mut output).unwrap();
    output
}

#[test]
fn deterministic_corpus_is_complete_and_reproducible() {
    let vectors = deterministic_vectors_v1();
    assert_eq!(vectors.len(), 6);
    assert_eq!(vectors[0].name, "nominal-balanced");
    assert!(
        vectors
            .iter()
            .all(|vector| vector.logits.iter().all(|x| x.is_finite()))
    );

    for vector in vectors {
        assert_eq!(
            route(&vector.logits),
            route(&vector.logits),
            "{}",
            vector.name
        );
    }
}

#[test]
fn nominal_vector_fills_every_slot_without_drops() {
    let vector = deterministic_vectors_v1()[0];
    let output = route(&vector.logits);
    assert_eq!(output.requested_counts, [4, 4, 4, 4]);
    assert_eq!(output.admitted_counts, [4, 4, 4, 4]);
    assert_eq!(output.expert_offsets, [0, 4, 8, 12, 16]);
    assert!(output.route_slots.iter().all(|slot| *slot != DROP_ROUTE_V1));
    assert!(
        output
            .permutation
            .iter()
            .all(|route| *route != DROP_ROUTE_V1)
    );
}

#[test]
fn equal_logits_use_total_lower_expert_tie_break() {
    let vector = deterministic_vectors_v1()[1];
    let output = route(&vector.logits);
    for selected in output.top2_experts.chunks_exact(2) {
        assert_eq!(selected, [0, 1]);
    }
    assert_eq!(output.requested_counts, [8, 8, 0, 0]);
    assert_eq!(output.admitted_counts, [4, 4, 0, 0]);
    assert_eq!(output.expert_offsets, [0, 4, 8, 8, 8]);
    assert!(
        output.permutation[8..]
            .iter()
            .all(|route| *route == DROP_ROUTE_V1)
    );
}

#[test]
fn overflow_keeps_each_experts_stable_route_prefix() {
    let vector = deterministic_vectors_v1()[3];
    let output = route(&vector.logits);
    assert_eq!(output.requested_counts[0], 8);

    let accepted_expert_zero: Vec<u32> = output.permutation
        [output.expert_offsets[0] as usize..output.expert_offsets[1] as usize]
        .to_vec();
    assert_eq!(accepted_expert_zero, [0, 2, 4, 6]);
    for route in [8_usize, 10, 12, 14] {
        assert_eq!(output.route_slots[route], DROP_ROUTE_V1);
        assert_eq!(output.inverse[route], DROP_ROUTE_V1);
    }
}

#[test]
fn empty_experts_have_empty_scan_segments() {
    let vector = deterministic_vectors_v1()[4];
    let output = route(&vector.logits);
    assert_eq!(output.requested_counts, [8, 8, 0, 0]);
    assert_eq!(output.expert_offsets[2], output.expert_offsets[3]);
    assert_eq!(output.expert_offsets[3], output.expert_offsets[4]);
}

#[test]
fn accepted_permutation_and_inverse_round_trip() {
    for vector in deterministic_vectors_v1() {
        let output = route(&vector.logits);
        let accepted = output.expert_offsets[MOE_EXPERTS_V1] as usize;
        for slot in 0..accepted {
            let route_id = output.permutation[slot] as usize;
            assert!(route_id < MOE_ROUTES_V1, "{}", vector.name);
            assert_eq!(output.route_slots[route_id], slot as u32);
            assert_eq!(output.inverse[route_id], slot as u32);
        }
        assert!(
            output.permutation[accepted..]
                .iter()
                .all(|route_id| *route_id == DROP_ROUTE_V1)
        );
    }
}

#[test]
fn wrong_length_preserves_all_outputs() {
    let before = RoutingOutputsV1::filled(0x1234_5678);
    let mut output = before.clone();
    let error = moe_top2_oracle_v1(&[0.0; 31], &mut output).unwrap_err();
    assert_eq!(
        error,
        MoeOracleErrorV1::WrongLogitLength {
            expected: 32,
            actual: 31,
        }
    );
    assert_eq!(output, before);
}

#[test]
fn non_finite_input_preserves_all_outputs() {
    for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut logits = [0.0_f32; 32];
        logits[11] = invalid;
        let before = RoutingOutputsV1::filled(0x7654_3210);
        let mut output = before.clone();
        assert_eq!(
            moe_top2_oracle_v1(&logits, &mut output),
            Err(MoeOracleErrorV1::NonFiniteLogit {
                token: 2,
                expert: 3,
            })
        );
        assert_eq!(output, before);
    }
}
