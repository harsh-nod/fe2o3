use fe2o3_moe_top2_v1::{
    DROP_ROUTE_V1, MODEL_ASSURANCE_V1, MOE_EXPERTS_V1, MOE_ROUTES_V1, ModelViolationV1,
    RoutingOutputsV1, SOURCE_MODEL_REFINEMENT_PROVED_V1, deterministic_vectors_v1,
    moe_top2_oracle_v1, validate_routing_model_v1,
};

fn routed(logits: &[f32]) -> RoutingOutputsV1 {
    let mut output = RoutingOutputsV1::filled(0xcccc_cccc);
    moe_top2_oracle_v1(logits, &mut output).unwrap();
    output
}

fn nominal() -> ([f32; 32], RoutingOutputsV1) {
    let logits = deterministic_vectors_v1()[0].logits;
    let output = routed(&logits);
    (logits, output)
}

#[test]
fn assurance_is_explicitly_not_a_verus_refinement_claim() {
    const { assert!(!SOURCE_MODEL_REFINEMENT_PROVED_V1) };
    assert!(MODEL_ASSURANCE_V1.contains("executable bounded invariant model"));
    assert!(MODEL_ASSURANCE_V1.contains("not a machine-checked Verus/source refinement proof"));
}

#[test]
fn complete_deterministic_corpus_satisfies_the_model() {
    for vector in deterministic_vectors_v1() {
        let output = routed(&vector.logits);
        assert_eq!(
            validate_routing_model_v1(&vector.logits, &output),
            Ok(()),
            "{}",
            vector.name
        );
    }
}

#[test]
fn exhaustive_two_row_ternary_logits_satisfy_all_obligations() {
    const VALUES: [f32; 3] = [-1.0, 0.0, 1.0];
    for mut encoding in 0_usize..3_usize.pow(8) {
        let mut rows = [[0.0_f32; 4]; 2];
        for row in &mut rows {
            for value in row {
                *value = VALUES[encoding % VALUES.len()];
                encoding /= VALUES.len();
            }
        }
        let mut logits = [0.0_f32; 32];
        for token in 0..8 {
            logits[token * 4..token * 4 + 4].copy_from_slice(&rows[token % 2]);
        }
        let output = routed(&logits);
        assert_eq!(validate_routing_model_v1(&logits, &output), Ok(()));
    }
}

#[test]
fn wrong_shape_and_non_finite_inputs_are_rejected() {
    let (mut logits, output) = nominal();
    assert_eq!(
        validate_routing_model_v1(&logits[..31], &output),
        Err(ModelViolationV1::WrongLogitLength)
    );
    logits[3] = f32::NAN;
    assert_eq!(
        validate_routing_model_v1(&logits, &output),
        Err(ModelViolationV1::NonFiniteLogit)
    );
}

#[test]
fn top2_range_distinctness_and_tie_break_mutations_are_rejected() {
    let (logits, mut output) = nominal();
    output.top2_experts[0] = MOE_EXPERTS_V1 as u32;
    assert_eq!(
        validate_routing_model_v1(&logits, &output),
        Err(ModelViolationV1::Top2Range { route: 0 })
    );

    let (logits, mut output) = nominal();
    output.top2_experts[1] = output.top2_experts[0];
    assert_eq!(
        validate_routing_model_v1(&logits, &output),
        Err(ModelViolationV1::Top2Distinctness { token: 0 })
    );

    let logits = deterministic_vectors_v1()[1].logits;
    let mut output = routed(&logits);
    output.top2_experts.swap(0, 1);
    assert_eq!(
        validate_routing_model_v1(&logits, &output),
        Err(ModelViolationV1::Top2Order { token: 0 })
    );
}

#[test]
fn count_capacity_and_scan_mutations_are_rejected() {
    let (logits, mut output) = nominal();
    output.requested_counts[0] -= 1;
    assert_eq!(
        validate_routing_model_v1(&logits, &output),
        Err(ModelViolationV1::RequestedCounts)
    );

    let (logits, mut output) = nominal();
    output.admitted_counts[0] = 5;
    assert_eq!(
        validate_routing_model_v1(&logits, &output),
        Err(ModelViolationV1::ExpertCapacity { expert: 0 })
    );

    let (logits, mut output) = nominal();
    output.expert_offsets[2] += 1;
    assert_eq!(
        validate_routing_model_v1(&logits, &output),
        Err(ModelViolationV1::ExclusiveScan)
    );
}

#[test]
fn duplicate_missing_and_out_of_range_slots_are_rejected() {
    let (logits, mut output) = nominal();
    output.route_slots[2] = output.route_slots[0];
    assert_eq!(
        validate_routing_model_v1(&logits, &output),
        Err(ModelViolationV1::DuplicateSlot {
            slot: output.route_slots[0] as usize,
        })
    );

    let (logits, mut output) = nominal();
    output.permutation[0] = DROP_ROUTE_V1;
    assert_eq!(
        validate_routing_model_v1(&logits, &output),
        Err(ModelViolationV1::Permutation { slot: 0 })
    );

    let (logits, mut output) = nominal();
    output.route_slots[0] = MOE_ROUTES_V1 as u32;
    assert_eq!(
        validate_routing_model_v1(&logits, &output),
        Err(ModelViolationV1::SlotRange { route: 0 })
    );
}

#[test]
fn bad_inverse_and_overflow_policy_drift_are_rejected() {
    let (logits, mut output) = nominal();
    output.inverse[0] = output.inverse[1];
    assert_eq!(
        validate_routing_model_v1(&logits, &output),
        Err(ModelViolationV1::Inverse { route: 0 })
    );

    let logits = deterministic_vectors_v1()[1].logits;
    let mut output = routed(&logits);
    let dropped = 8;
    assert_eq!(output.route_slots[dropped], DROP_ROUTE_V1);
    output.route_slots[dropped] = 0;
    output.inverse[dropped] = 0;
    assert_eq!(
        validate_routing_model_v1(&logits, &output),
        Err(ModelViolationV1::StableSlot { route: dropped })
    );
}

#[test]
fn unused_permutation_tail_must_remain_sentinel() {
    let logits = deterministic_vectors_v1()[1].logits;
    let mut output = routed(&logits);
    let accepted = output.expert_offsets[MOE_EXPERTS_V1] as usize;
    assert!(accepted < MOE_ROUTES_V1);
    output.permutation[accepted] = 0;
    assert_eq!(
        validate_routing_model_v1(&logits, &output),
        Err(ModelViolationV1::PermutationTail { slot: accepted })
    );
}
