use fe2o3_device::{
    Bf16MfmaAMatrix, Bf16MfmaBMatrix, DeviceMatrix, DynamicLds,
    Bf16MfmaAFragment, Bf16MfmaBFragment, F32AccumulatorFragment, Gfx942Collectives,
    Invocation3D, SubgroupTile, Wave64, WaveLane, Workgroup, WorkgroupCollectiveScratch,
    WorkgroupLdsScope, WorkgroupPipeline, sync,
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
    let lhs_fragment = lhs_view.load_m16k16(&lane, 0, 0);
    let rhs_fragment = rhs_view.load_k16n16(&lane, 0, 0);
    let mut pipeline_scope = WorkgroupLdsScope::current();
    let mut lhs_pipeline =
        WorkgroupPipeline::<Bf16MfmaAFragment<'_>, 2, 64, 1>::current(&mut pipeline_scope);
    let mut rhs_pipeline =
        WorkgroupPipeline::<Bf16MfmaBFragment<'_>, 2, 64, 1>::current(&mut pipeline_scope);
    let lane_index = lane.get() as usize;
    lhs_pipeline.stage(0);
    lhs_pipeline.write(0, lane_index, lhs_fragment);
    lhs_pipeline.commit(0);
    rhs_pipeline.stage(0);
    rhs_pipeline.write(0, lane_index, rhs_fragment);
    rhs_pipeline.commit(0);
    lhs_pipeline.wait(0);
    lhs_pipeline.consume(0);
    let lhs_fragment = lhs_pipeline.read(0, lane_index);
    lhs_pipeline.release(0);
    rhs_pipeline.wait(0);
    rhs_pipeline.consume(0);
    let rhs_fragment = rhs_pipeline.read(0, lane_index);
    rhs_pipeline.release(0);
    let matrix = DeviceMatrix::current();
    let accumulator = F32AccumulatorFragment::zero(&lane);
    let _ = matrix.multiply_accumulate(lhs_fragment, rhs_fragment, accumulator);
}

fn main() {
    let _ = safe_execution_surface as fn(&[u16], &[u16]);
}
