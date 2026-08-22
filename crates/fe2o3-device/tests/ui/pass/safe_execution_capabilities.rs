use fe2o3_device::{
    Bf16MfmaFragment, DeviceMatrix, DynamicLds, F32AccumulatorFragment, Gfx942Collectives,
    Invocation3D, SubgroupTile, Wave64, WaveLane, Workgroup, WorkgroupCollectiveScratch,
    WorkgroupLdsScope, gfx942_lds_bf16_tile_pair_m16x16_v1,
    gfx942_publish_lds_bf16_tile_pair_m16x16_v1, sync,
};

fn safe_execution_surface() {
    let lane = WaveLane::<Wave64>::current();
    let wave = SubgroupTile::<64>::from_wave64_snapshot(&lane);
    let invocation = Invocation3D::current();
    let group = Workgroup::from_invocation_snapshot(&invocation).unwrap();
    let collectives = Gfx942Collectives::current();

    let _ = wave.reduce_sum(&collectives, 7_u32);
    sync::syncthreads();
    group.synchronize();

    let mut scope = WorkgroupLdsScope::current();
    let lds = DynamicLds::<i32>::exact_current::<64>(&mut scope);
    let mut scratch = WorkgroupCollectiveScratch::from_dynamic_lds(&group, lds).unwrap();
    let _ = group.reduce_sum(&collectives, &mut scratch, 7_i32);

    let (mut lhs, mut rhs) = gfx942_lds_bf16_tile_pair_m16x16_v1();
    let _: () = lhs.write_mfma_fragment(&lane, Bf16MfmaFragment::ZERO);
    let _: () = rhs.write_mfma_fragment(&lane, Bf16MfmaFragment::ZERO);
    let (lhs, rhs) = gfx942_publish_lds_bf16_tile_pair_m16x16_v1(lhs, rhs);
    let lhs = lhs.read_mfma_fragment(&lane);
    let rhs = rhs.read_mfma_fragment(&lane);
    let matrix = DeviceMatrix::current();
    let _ = matrix.multiply_accumulate(lhs, rhs, F32AccumulatorFragment::ZERO);
}

fn main() {
    let _ = safe_execution_surface as fn();
}
