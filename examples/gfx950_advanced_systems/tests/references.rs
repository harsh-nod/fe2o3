use fe2o3_gfx950_advanced_systems::{
    ALL_EXPERTS, CANDIDATES, DRAFT_STEPS, EXPERTS, GRADIENT_SHARDS, HIDDEN, MUON_ELEMENTS, NGRAM,
    OUTPUT, QUERIES, STATE_WIDTH, SYSTEM_BATCHES, TABLE_SIZE, TOKENS, TOP_K,
    reference::{
        batched_moe_rank_reference, batched_moe_routing_reference, batched_muon_reference,
        decode_fp8, moe_rank_reference, moe_routing_reference, muon_reference, ngram_reference,
        speculative_reference,
    },
};

#[test]
fn fp8_decoder_rejects_both_ocp_nan_encodings() {
    assert!(decode_fp8(0x7f).is_nan());
    assert!(decode_fp8(0xff).is_nan());
}

#[test]
fn moe_reference_routes_and_combines_nonuniform_low_precision_inputs() {
    let activations = (0..TOKENS * HIDDEN)
        .map(|index| [0x0, 0x1, 0x2, 0x9, 0xa][(index * 7 + index / HIDDEN) % 5])
        .collect::<Vec<_>>();
    let router_weights = (0..EXPERTS * HIDDEN)
        .map(|index| 0.015625 * ((index * 5 + index / HIDDEN * 3) % 11) as f32 - 0.078125)
        .collect::<Vec<_>>();
    let expert_weights = (0..ALL_EXPERTS * HIDDEN * OUTPUT)
        .map(|index| [0x00, 0x30, 0x38, 0xb0, 0xb8][(index * 3 + index / OUTPUT) % 5])
        .collect::<Vec<_>>();

    let routing = moe_routing_reference(&activations, &router_weights);
    assert_eq!(
        routing.expert_counts.iter().sum::<u32>(),
        (TOKENS * TOP_K) as u32
    );
    assert!(
        routing
            .top_experts
            .iter()
            .all(|expert| *expert < EXPERTS as u32)
    );
    for weights in routing.top_weights.chunks_exact(TOP_K) {
        assert!((weights.iter().sum::<f32>() - 1.0).abs() <= 1.0e-6);
    }

    let rank0 = moe_rank_reference(&activations, &expert_weights, &routing, 0, true);
    let rank1 = moe_rank_reference(&activations, &expert_weights, &routing, 2, false);
    let combined = rank0
        .iter()
        .zip(&rank1)
        .map(|(left, right)| left + right)
        .collect::<Vec<_>>();
    assert!(combined.iter().all(|value| value.is_finite()));
    assert!(combined.windows(2).any(|pair| pair[0] != pair[1]));
}

#[test]
fn speculative_commit_and_rollback_are_transactional() {
    let target = [11, 12, 13, 14];
    let mut draft = vec![0; CANDIDATES * DRAFT_STEPS];
    let mut scores = vec![0.9; CANDIDATES * DRAFT_STEPS];
    for candidate in 0..CANDIDATES {
        draft[candidate * DRAFT_STEPS..(candidate + 1) * DRAFT_STEPS].copy_from_slice(&target);
    }
    draft[DRAFT_STEPS + 2] = 99;
    scores[2 * DRAFT_STEPS + 1] = 0.0;
    let thresholds = [0.25, 0.35, 0.45, 0.55];
    let base = (0..STATE_WIDTH)
        .map(|i| i as f32 * 0.125)
        .collect::<Vec<_>>();
    let deltas = vec![0.01; CANDIDATES * DRAFT_STEPS * STATE_WIDTH];
    let result = speculative_reference(&draft, &target, &scores, &thresholds, &base, &deltas);
    assert_eq!(result.committed.iter().sum::<u32>(), 6);
    for candidate in [1, 2] {
        assert_eq!(
            &result.state[candidate * STATE_WIDTH..(candidate + 1) * STATE_WIDTH],
            base
        );
    }
}

#[test]
fn ngram_reference_rejects_collision_and_uses_priority_then_slot() {
    let mut queries = vec![0; QUERIES * NGRAM];
    for query in 0..QUERIES {
        queries[query * NGRAM..(query + 1) * NGRAM].copy_from_slice(&[
            100 + query as i32,
            200,
            300,
        ]);
    }
    let hash = queries[..NGRAM]
        .iter()
        .fold(1_469_598_103_934_665_603_u64, |h, v| {
            (h ^ (*v as u32 as u64)).wrapping_mul(1_099_511_628_211)
        });
    let mut hashes = vec![0; TABLE_SIZE];
    let mut grams = vec![-1; TABLE_SIZE * NGRAM];
    let mut values = vec![-1; TABLE_SIZE];
    let mut priorities = vec![-1; TABLE_SIZE];
    for (slot, value, priority) in [(1, 11, 2), (3, 33, 4)] {
        hashes[slot] = hash;
        grams[slot * NGRAM..(slot + 1) * NGRAM].copy_from_slice(&queries[..NGRAM]);
        values[slot] = value;
        priorities[slot] = priority;
    }
    hashes[2] = hash;
    grams[2 * NGRAM..3 * NGRAM].copy_from_slice(&[9, 9, 9]);
    priorities[2] = 99;
    assert_eq!(
        ngram_reference(&queries, &hashes, &grams, &values, &priorities)[0],
        33
    );
}

#[test]
fn muon_reference_is_finite_and_deterministic() {
    let shards = (0..GRADIENT_SHARDS * MUON_ELEMENTS)
        .map(|index| 0.025 * ((index * 5 % 11) as f32 - 5.0))
        .collect::<Vec<_>>();
    let first = muon_reference(&shards);
    let second = muon_reference(&shards);
    assert_eq!(first, second);
    assert!(first.norm.is_finite() && first.norm > 0.0);
    assert!(first.update.iter().all(|value| value.is_finite()));
}

#[test]
fn fixed_dimensions_remain_bound_to_the_hardware_fixture() {
    assert_eq!((TOKENS, HIDDEN), (16, 128));
}

#[test]
fn batched_references_keep_all_sixteen_wave_instances_disjoint() {
    let activations = (0..SYSTEM_BATCHES * TOKENS * HIDDEN)
        .map(|index| {
            let batch = index / (TOKENS * HIDDEN);
            [0x0, 0x1, 0x2, 0x9, 0xa][(index * 7 + batch * 3) % 5]
        })
        .collect::<Vec<_>>();
    let router_weights = (0..SYSTEM_BATCHES * EXPERTS * HIDDEN)
        .map(|index| {
            let batch = index / (EXPERTS * HIDDEN);
            0.015625 * ((index * 5 + batch * 7) % 11) as f32 - 0.078125
        })
        .collect::<Vec<_>>();
    let expert_weights = (0..SYSTEM_BATCHES * ALL_EXPERTS * HIDDEN * OUTPUT)
        .map(|index| {
            let batch = index / (ALL_EXPERTS * HIDDEN * OUTPUT);
            [0x00, 0x30, 0x38, 0xb0, 0xb8][(index * 3 + batch) % 5]
        })
        .collect::<Vec<_>>();

    let routing = batched_moe_routing_reference(&activations, &router_weights);
    assert_eq!(routing.top_experts.len(), SYSTEM_BATCHES * TOKENS * TOP_K);
    assert_eq!(routing.expert_counts.len(), SYSTEM_BATCHES * EXPERTS);
    for batch in 0..SYSTEM_BATCHES {
        let activation_base = batch * TOKENS * HIDDEN;
        let router_base = batch * EXPERTS * HIDDEN;
        let route_base = batch * TOKENS * TOP_K;
        let expected = moe_routing_reference(
            &activations[activation_base..activation_base + TOKENS * HIDDEN],
            &router_weights[router_base..router_base + EXPERTS * HIDDEN],
        );
        assert_eq!(
            &routing.top_experts[route_base..route_base + TOKENS * TOP_K],
            expected.top_experts
        );
    }

    let ranks = batched_moe_rank_reference(&activations, &expert_weights, &routing, 0, true);
    assert_eq!(ranks.len(), SYSTEM_BATCHES * TOKENS * OUTPUT);

    let shards = (0..SYSTEM_BATCHES * GRADIENT_SHARDS * MUON_ELEMENTS)
        .map(|index| {
            let batch = index / (GRADIENT_SHARDS * MUON_ELEMENTS);
            0.025 * (((index * 5 + batch * 3) % 11) as f32 - 5.0)
        })
        .collect::<Vec<_>>();
    let muon = batched_muon_reference(&shards);
    assert_eq!(muon.update.len(), SYSTEM_BATCHES * MUON_ELEMENTS);
    assert_eq!(muon.norms.len(), SYSTEM_BATCHES);
}
