use fe2o3_gfx950_gpt_oss_decode::{OPENAI_GPT_OSS_COMMIT, PROFILE_BOUNDARY};

#[test]
fn production_source_preserves_the_fixed_layer_tile_contract() {
    let source = include_str!("../src/kernel.rs");
    assert!(source.contains("gfx950_gpt_oss_120b_decode_megakernel_v1"));
    assert!(source.contains("launch(required = [64, 1, 1]"));
    assert!(source.contains("control_flow(loop_bounds(2880, 64, 16))"));
    assert_eq!(source.matches("multiply_accumulate(\n").count(), 4);
    assert_eq!(source.matches("multiply_accumulate_fp4(").count(), 4);
    assert!(source.contains("while source < 64"));
    assert!(source.contains("local_expert0 = lane_index * 2"));
    assert!(source.contains("local_expert1 = local_expert0 + 1"));
    assert!(source.contains("let selected = (id0 as usize)"));
    assert!(!source.contains("unsafe"));
    assert_eq!(OPENAI_GPT_OSS_COMMIT.len(), 40);
    assert!(PROFILE_BOUNDARY.contains("batch=1"));
    assert!(PROFILE_BOUNDARY.contains("full 128-way top-4 router"));
    assert!(PROFILE_BOUNDARY.contains("selected top-1 MLP1"));
    assert!(PROFILE_BOUNDARY.contains("attention rows 8..16"));
    assert!(PROFILE_BOUNDARY.contains("expert rows 1..16"));
}
