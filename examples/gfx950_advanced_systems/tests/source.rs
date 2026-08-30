use fe2o3_gfx950_advanced_systems::{
    DISPATCH_CAPACITY, GFX950_ADVANCED_SYSTEMS_RUST_SOURCE_PRESENT_V1,
    GFX950_ADVANCED_SYSTEMS_SOURCE_BLOCKER, GFX950_ADVANCED_SYSTEMS_SOURCE_LOWERING_SUPPORTED,
    MUON_ELEMENTS, OUTPUT, STATE_WIDTH, TABLE_SIZE, TOKENS, TOP_K,
};

#[test]
fn rust_source_is_primary_and_uses_production_lowering() {
    let source = include_str!("../src/kernel.rs");
    for symbol in [
        "gfx950_moe_route_fp4_t16_e4_k2_v1",
        "gfx950_moe_expert_rank_fp4_fp8_v1",
        "gfx950_combine_expert_ranks_v1",
        "gfx950_speculative_transaction_v1",
        "gfx950_qwen_ngram_gather_v1",
        "gfx950_stage_gradient_shard_v1",
        "gfx950_muon_update_4x4_v1",
    ] {
        assert!(source.contains(symbol));
    }
    assert_eq!(source.matches("pub fn gfx950_").count(), 7);
    assert!(GFX950_ADVANCED_SYSTEMS_RUST_SOURCE_PRESENT_V1);
    assert!(GFX950_ADVANCED_SYSTEMS_SOURCE_LOWERING_SUPPORTED);
    assert!(GFX950_ADVANCED_SYSTEMS_SOURCE_BLOCKER.contains("formal compiler refinement"));
    assert!(GFX950_ADVANCED_SYSTEMS_SOURCE_BLOCKER.contains("protected publication"));
    let manifest = include_str!("../Cargo.toml");
    for feature in [
        "kernel-moe-route",
        "kernel-moe-expert-rank",
        "kernel-combine-expert-ranks",
        "kernel-speculative-transaction",
        "kernel-qwen-ngram-gather",
        "kernel-stage-gradient-shard",
        "kernel-muon-update",
    ] {
        assert!(manifest.contains(feature));
        assert!(source.contains(feature));
    }
    for feature in [
        "ablation-expert-serial",
        "ablation-combine-transposed",
        "ablation-speculative-recompute-prefix",
        "ablation-ngram-reverse-probe",
        "ablation-stage-tile4",
        "ablation-muon-broadcast16",
    ] {
        assert!(manifest.contains(feature));
        assert!(source.contains(feature));
    }
    assert!(source.contains("let accepted = accepted_prefix!(candidate);"));
    let crate_root = include_str!("../src/lib.rs");
    assert!(manifest.contains("ablation-route-owner-only"));
    assert!(crate_root.contains("ablation-route-owner-only is rejected"));
    assert!(crate_root.contains("ablation-route-unpacked is retained only"));
    assert_eq!(source.matches("namespace = \"").count(), 13);
    assert_eq!(source.matches("max_grid = [1, 1, 1]").count(), 11);
    assert_eq!(source.matches("launch(required = [256, 1, 1]").count(), 3);
    assert_eq!(source.matches("launch(required = [64, 1, 1]").count(), 10);

    assert_eq!(OUTPUT, 16);
    assert!(OUTPUT.is_power_of_two());
    assert_eq!(TOP_K, 2);
    assert_eq!(STATE_WIDTH, 8);
    assert!(STATE_WIDTH.is_power_of_two());
    assert_eq!(TABLE_SIZE, 16);
    assert!(TABLE_SIZE.is_power_of_two());
    assert_eq!(MUON_ELEMENTS, 16);
    assert!(MUON_ELEMENTS.is_power_of_two());

    assert_eq!(TOKENS * TOP_K, 32);
    assert_eq!(DISPATCH_CAPACITY, TOKENS * TOP_K);
    assert!(TOKENS * TOP_K <= i32::MAX as usize);
    let compact = source.split_whitespace().collect::<String>();
    assert!(compact.contains("(recordasi32).wrapping_sub(dispatched).wrapping_mul(choose)"));
    assert!(source.contains("count.wrapping_add((selected == count_expert) as u32)"));
    assert!(source.contains("seen.wrapping_add(dispatch_matches)"));
    assert!(compact.contains(".wrapping_sub(precedes12).wrapping_sub(precedes13)"));
    assert!(
        compact.contains(
            "lane.wrapping_sub((dispatch_expertasusize).wrapping_mul(DISPATCH_CAPACITY))"
        )
    );
    assert!(source.contains(".wrapping_shr(((bits & 7) as u32).wrapping_mul(4))"));
    assert!(compact.contains("packed_routes.wrapping_shr(2_usize.wrapping_mul(record)asu32)"));
    assert!(source.contains("record += 1;"));
    assert!(source.contains("depth += 1;"));
}
