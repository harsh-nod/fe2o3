use fe2o3_device::{Index1D, KernelMarkerV1, WriteOnlyDisjointSlice};
use fe2o3_gfx950_advanced_systems::kernel::{
    __fe2o3_kernel_marker_gfx950_combine_expert_ranks_v1,
    __fe2o3_kernel_marker_gfx950_moe_expert_rank_fp4_fp8_v1,
    __fe2o3_kernel_marker_gfx950_moe_route_fp4_t16_e4_k2_v1,
    __fe2o3_kernel_marker_gfx950_muon_update_4x4_v1,
    __fe2o3_kernel_marker_gfx950_qwen_ngram_gather_v1,
    __fe2o3_kernel_marker_gfx950_speculative_transaction_v1,
    __fe2o3_kernel_marker_gfx950_stage_gradient_shard_v1,
};

type RoutePhysical = fn(&[u8], &[f32], &mut [u32], &mut [f32], &mut [u32], &mut [i32]);

type ExpertPhysical = fn(&[u8], &[u8], &[u32], &[f32], u32, u32, &mut [f32]);

type CombinePhysical = fn(&[f32], &[f32], WriteOnlyDisjointSlice<f32, Index1D>);

type SpeculativePhysical = fn(
    &[i32],
    &[i32],
    &[f32],
    &[f32],
    &[f32],
    &[f32],
    &mut [u32],
    &mut [u32],
    WriteOnlyDisjointSlice<f32, Index1D>,
);

type NgramPhysical = fn(&[i32], &[u64], &[i32], &[i32], &[i32], &mut [i32]);

type StagePhysical = fn(&[f32], &mut [f32]);

type MuonPhysical = fn(&[f32], &mut [f32], &mut [f32]);

#[test]
fn logical_context_and_global_wrappers_are_absent_from_every_physical_abi() {
    let _: RoutePhysical =
        <__fe2o3_kernel_marker_gfx950_moe_route_fp4_t16_e4_k2_v1 as KernelMarkerV1>::FUNCTION;
    let _: ExpertPhysical =
        <__fe2o3_kernel_marker_gfx950_moe_expert_rank_fp4_fp8_v1 as KernelMarkerV1>::FUNCTION;
    let _: CombinePhysical =
        <__fe2o3_kernel_marker_gfx950_combine_expert_ranks_v1 as KernelMarkerV1>::FUNCTION;
    let _: SpeculativePhysical =
        <__fe2o3_kernel_marker_gfx950_speculative_transaction_v1 as KernelMarkerV1>::FUNCTION;
    let _: NgramPhysical =
        <__fe2o3_kernel_marker_gfx950_qwen_ngram_gather_v1 as KernelMarkerV1>::FUNCTION;
    let _: StagePhysical =
        <__fe2o3_kernel_marker_gfx950_stage_gradient_shard_v1 as KernelMarkerV1>::FUNCTION;
    let _: MuonPhysical =
        <__fe2o3_kernel_marker_gfx950_muon_update_4x4_v1 as KernelMarkerV1>::FUNCTION;
}
