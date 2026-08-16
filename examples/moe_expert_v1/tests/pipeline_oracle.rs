use fe2o3_device::Bf16;
use fe2o3_moe_expert_v1::{
    DROP_ROUTE_V1, MOE_COMBINED_OUTPUT_ELEMENTS_V1, MOE_EXPERT_CAPACITY_V1,
    MOE_EXPERT_INPUT_WIDTH_V1, MOE_EXPERT_OUTPUT_WIDTH_V1, MOE_EXPERT_TILE_ELEMENTS_V1,
    MOE_EXPERT_TILE_ROWS_V1, MOE_EXPERT_WEIGHT_ELEMENTS_V1, MOE_EXPERTS_V1,
    MOE_ROUTES_PER_TOKEN_V1, MOE_ROUTES_V1, MOE_TOKEN_ACTIVATION_ELEMENTS_V1,
    MoeExpertExpectedEvidenceV1, MoeExpertInputErrorV1, combined_output_index_v1,
    compact_output_index_v1, expert_route_id_v1, expert_tile_index_v1, expert_weight_index_v1,
    moe_expert_independent_oracle_v1, run_host_scheduled_moe_experts_v1, token_activation_index_v1,
};
use fe2o3_moe_top2_v1::{RoutingOutputsV1, moe_top2_oracle_v1};

const CANARY_U16: u16 = 0x55aa;

fn bf16(value: f32) -> u16 {
    Bf16::from_f32(value).to_bits()
}

fn route(logits: &[f32; 32]) -> RoutingOutputsV1 {
    let mut output = RoutingOutputsV1::filled(0xdead_beef);
    moe_top2_oracle_v1(logits, &mut output).unwrap();
    output
}

fn equal_logits() -> [f32; 32] {
    [1.0; 32]
}

fn balanced_logits() -> [f32; 32] {
    let mut logits = [-4.0_f32; 32];
    for token in 0..8 {
        let first = token % 4;
        let second = (token + 1) % 4;
        logits[token * 4 + first] = 4.0;
        logits[token * 4 + second] = 3.0;
    }
    logits
}

fn patterned_activations() -> [u16; MOE_TOKEN_ACTIVATION_ELEMENTS_V1] {
    core::array::from_fn(|index| bf16(((index % 13) as f32 - 6.0) * 0.25))
}

fn patterned_weights() -> [u16; MOE_EXPERT_WEIGHT_ELEMENTS_V1] {
    core::array::from_fn(|index| {
        let expert = index / MOE_EXPERT_TILE_ELEMENTS_V1;
        let within = index % MOE_EXPERT_TILE_ELEMENTS_V1;
        let depth = within / MOE_EXPERT_OUTPUT_WIDTH_V1;
        let output = within % MOE_EXPERT_OUTPUT_WIDTH_V1;
        let value = if depth == output {
            1.0 + expert as f32 * 0.25
        } else {
            ((depth + output + expert) % 5) as f32 * 0.03125
        };
        bf16(value)
    })
}

fn route_weights() -> [f32; MOE_ROUTES_V1] {
    core::array::from_fn(|route| if route % 2 == 0 { 0.75 } else { 0.25 })
}

fn assert_execution_matches_oracle(logits: &[f32; 32]) {
    let routing = route(logits);
    let activations = patterned_activations();
    let weights = patterned_weights();
    let route_weights = route_weights();
    let actual =
        run_host_scheduled_moe_experts_v1(logits, &activations, &weights, &route_weights, &routing)
            .unwrap();
    let expected =
        moe_expert_independent_oracle_v1(logits, &activations, &weights, &route_weights, &routing)
            .unwrap();

    assert_eq!(actual.expert_output_tiles, expected.expert_output_tiles);
    assert_eq!(actual.compact_output, expected.compact_output);
    assert_eq!(actual.combined_output, expected.combined_output);
    assert_eq!(actual.dispatches.len(), MOE_EXPERTS_V1);

    for expert in 0..MOE_EXPERTS_V1 {
        assert_eq!(actual.dispatches[expert].expert, expert);
        assert_eq!(
            actual.dispatches[expert].active_rows,
            routing.admitted_counts[expert] as usize
        );
        for row in actual.dispatches[expert].active_rows..MOE_EXPERT_TILE_ROWS_V1 {
            for output in 0..MOE_EXPERT_OUTPUT_WIDTH_V1 {
                let index = expert * MOE_EXPERT_TILE_ELEMENTS_V1
                    + row * MOE_EXPERT_OUTPUT_WIDTH_V1
                    + output;
                assert_eq!(actual.activation_tiles[index], 0);
                assert_eq!(actual.expert_output_tiles[index], 0.0);
            }
        }
    }
    for route_id in 0..MOE_ROUTES_V1 {
        let slot = routing.inverse[route_id];
        if slot == DROP_ROUTE_V1 {
            assert!(
                expected.route_output[route_id * 16..route_id * 16 + 16]
                    .iter()
                    .all(|value| *value == 0.0)
            );
        } else {
            assert_eq!(routing.permutation[slot as usize], route_id as u32);
            assert_eq!(
                &expected.route_output[route_id * 16..route_id * 16 + 16],
                &actual.compact_output[slot as usize * 16..slot as usize * 16 + 16],
            );
        }
    }
}

#[test]
fn capacity_drops_empty_experts_padding_inverse_weights_and_all_outputs_match() {
    let logits = equal_logits();
    let routing = route(&logits);
    assert_eq!(routing.admitted_counts, [4, 4, 0, 0]);
    assert!(routing.inverse.iter().any(|slot| *slot == DROP_ROUTE_V1));
    assert_execution_matches_oracle(&logits);
}

#[test]
fn balanced_pattern_fills_every_expert_and_matches_every_output() {
    let logits = balanced_logits();
    let routing = route(&logits);
    assert_eq!(routing.admitted_counts, [4; MOE_EXPERTS_V1]);
    assert!(routing.inverse.iter().all(|slot| *slot != DROP_ROUTE_V1));
    assert_execution_matches_oracle(&logits);
}

#[test]
fn inputs_and_guard_canaries_are_untouched() {
    let logits = balanced_logits();
    let routing = route(&logits);
    let activations = patterned_activations();
    let weights = patterned_weights();
    let route_weights = route_weights();
    let mut guarded_activations = vec![CANARY_U16];
    guarded_activations.extend_from_slice(&activations);
    guarded_activations.push(CANARY_U16);
    let mut guarded_weights = vec![CANARY_U16];
    guarded_weights.extend_from_slice(&weights);
    guarded_weights.push(CANARY_U16);
    let before_activations = guarded_activations.clone();
    let before_weights = guarded_weights.clone();

    let _ = run_host_scheduled_moe_experts_v1(
        &logits,
        &guarded_activations[1..=MOE_TOKEN_ACTIVATION_ELEMENTS_V1],
        &guarded_weights[1..=MOE_EXPERT_WEIGHT_ELEMENTS_V1],
        &route_weights,
        &routing,
    )
    .unwrap();
    let _ = moe_expert_independent_oracle_v1(
        &logits,
        &guarded_activations[1..=MOE_TOKEN_ACTIVATION_ELEMENTS_V1],
        &guarded_weights[1..=MOE_EXPERT_WEIGHT_ELEMENTS_V1],
        &route_weights,
        &routing,
    )
    .unwrap();
    assert_eq!(guarded_activations, before_activations);
    assert_eq!(guarded_weights, before_weights);
    assert_eq!(guarded_activations[0], CANARY_U16);
    assert_eq!(*guarded_activations.last().unwrap(), CANARY_U16);
    assert_eq!(guarded_weights[0], CANARY_U16);
    assert_eq!(*guarded_weights.last().unwrap(), CANARY_U16);
}

#[test]
fn malformed_lengths_weights_bf16_and_routing_fail_before_results_exist() {
    let logits = balanced_logits();
    let routing = route(&logits);
    let activations = patterned_activations();
    let weights = patterned_weights();
    let route_weights = route_weights();
    assert!(matches!(
        run_host_scheduled_moe_experts_v1(
            &logits,
            &activations[..activations.len() - 1],
            &weights,
            &route_weights,
            &routing,
        ),
        Err(MoeExpertInputErrorV1::WrongLength {
            input: "token_activations",
            ..
        })
    ));
    let mut bad_route_weights = route_weights;
    bad_route_weights[0] = 0.5;
    assert_eq!(
        run_host_scheduled_moe_experts_v1(
            &logits,
            &activations,
            &weights,
            &bad_route_weights,
            &routing,
        ),
        Err(MoeExpertInputErrorV1::RouteWeights { token: 0 })
    );
    let mut bad_activations = activations;
    bad_activations[7] = 0x7f80;
    assert_eq!(
        run_host_scheduled_moe_experts_v1(
            &logits,
            &bad_activations,
            &weights,
            &route_weights,
            &routing,
        ),
        Err(MoeExpertInputErrorV1::NonFiniteBf16 {
            input: "token_activations",
            index: 7,
            bits: 0x7f80,
        })
    );
    let mut bad_routing = routing;
    bad_routing.inverse[0] = 15;
    assert!(matches!(
        run_host_scheduled_moe_experts_v1(
            &logits,
            &activations,
            &weights,
            &route_weights,
            &bad_routing,
        ),
        Err(MoeExpertInputErrorV1::Routing(_))
    ));
}

#[test]
fn exact_index_maps_cover_only_the_fixed_extents() {
    assert_eq!(expert_route_id_v1(7, 1), Some(15));
    assert_eq!(expert_route_id_v1(8, 0), None);
    assert_eq!(token_activation_index_v1(7, 15), Some(127));
    assert_eq!(token_activation_index_v1(7, 16), None);
    assert_eq!(expert_weight_index_v1(3, 15, 15), Some(1023));
    assert_eq!(expert_weight_index_v1(4, 0, 0), None);
    assert_eq!(expert_tile_index_v1(3, 15, 15), Some(1023));
    assert_eq!(expert_tile_index_v1(3, 16, 0), None);
    assert_eq!(compact_output_index_v1(15, 15), Some(255));
    assert_eq!(compact_output_index_v1(16, 0), None);
    assert_eq!(combined_output_index_v1(7, 15), Some(127));
    assert_eq!(combined_output_index_v1(8, 0), None);
    assert_eq!(MOE_EXPERT_CAPACITY_V1, 4);
}

#[test]
fn expected_proof_values_are_copyable_and_inert() {
    let expected = MoeExpertExpectedEvidenceV1 {
        proof_source: [1; 32],
        kernel_source: [2; 32],
        transcript: [3; 32],
    };
    let copied = expected;
    assert_eq!(copied, expected);
    assert!(!expected.authenticates_anything());
    assert!(!expected.proves_source_to_machine_refinement());
    assert!(!expected.proves_generalized_race_freedom());
    assert!(!expected.proves_protected_gpu_execution());
    assert_eq!(MOE_ROUTES_PER_TOKEN_V1, 2);
    assert_eq!(MOE_EXPERT_INPUT_WIDTH_V1, 16);
    assert_eq!(MOE_COMBINED_OUTPUT_ELEMENTS_V1, 128);
}
