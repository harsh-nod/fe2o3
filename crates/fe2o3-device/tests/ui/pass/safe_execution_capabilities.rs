use fe2o3_device::{
    Bf16MfmaAMatrix, Bf16MfmaBMatrix, DeviceMatrix, DynamicLds,
    F32AccumulatorFragment, Gfx942Collectives, Invocation3D, SubgroupTile, Wave64,
    WaveLane, Workgroup, WorkgroupCollectiveScratch, WorkgroupLdsScope,
    gfx942_lds_bf16_tile_pair_m16x16_v1,
    gfx942_publish_lds_bf16_tile_pair_m16x16_v1, sync,
};

fn safe_execution_surface(lhs_bits: &[u16], rhs_bits: &[u16]) {
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

    let lhs_view = Bf16MfmaAMatrix::row_major(lhs_bits, 0, 16, 16, 16).unwrap();
    let rhs_view = Bf16MfmaBMatrix::row_major(rhs_bits, 0, 16, 16, 16).unwrap();
    let lhs_fragment = lhs_view.load_m16k16(&lane, 0, 0).unwrap();
    let rhs_fragment = rhs_view.load_k16n16(&lane, 0, 0).unwrap();
    let (mut lhs, mut rhs) = gfx942_lds_bf16_tile_pair_m16x16_v1();
    lhs.write_mfma_fragment(&lane, lhs_fragment);
    rhs.write_mfma_fragment(&lane, rhs_fragment);
    let (lhs, rhs) = gfx942_publish_lds_bf16_tile_pair_m16x16_v1(lhs, rhs);
    let lhs = lhs.read_mfma_fragment(&lane);
    let rhs = rhs.read_mfma_fragment(&lane);
    let matrix = DeviceMatrix::current();
    let accumulator = F32AccumulatorFragment::zero(&lane);
    let _ = matrix.multiply_accumulate(lhs, rhs, accumulator);
}

fn main() {
    let _ = safe_execution_surface as fn(&[u16], &[u16]);
}
