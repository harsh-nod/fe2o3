use fe2o3_gfx950_advanced_systems::GFX950_ADVANCED_SYSTEMS_SOURCE_BLOCKER;

#[test]
fn rust_source_is_primary_but_lowering_claims_fail_closed() {
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
    assert_eq!(source.matches("#[kernel(").count(), 7);
    assert!(GFX950_ADVANCED_SYSTEMS_SOURCE_BLOCKER.contains("rustc-codegen-fe2o3"));
}
