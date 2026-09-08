use fe2o3_device::{
    Blocked, CurrentTarget, DisjointWrite, Gfx950Fp4E2M1, Global, GlobalGfx950Fp4MfmaAMatrix,
    Index1D, InitialEpoch, KernelCapabilityBrand, PolicyDeviceMath, PolicyGfx950Matrix, ReadOnly,
    RegisteredLaunch, StrictIeee, Subgroup, SubgroupBrand, SubgroupLane, SubgroupWidth64,
    ThreadIndex, WorkgroupCapability, WorkgroupEpoch,
};

enum KernelA {}

type Root<'kernel> = KernelCapabilityBrand<'kernel, KernelA, CurrentTarget, RegisteredLaunch>;
type MatrixBrand<'workgroup> =
    SubgroupBrand<'workgroup, SubgroupWidth64, Root<'workgroup>, InitialEpoch>;

fn global_matrix_and_policy<'kernel>(
    gfx950: &PolicyGfx950Matrix<'kernel, MatrixBrand<'kernel>, Root<'kernel>, StrictIeee>,
    math: &PolicyDeviceMath<'kernel, Root<'kernel>, StrictIeee>,
    a_bits: &Global<'kernel, u8, ReadOnly, Root<'kernel>>,
    b_bits: &Global<'kernel, u8, ReadOnly, Root<'kernel>>,
    lane: &SubgroupLane<SubgroupWidth64, MatrixBrand<'kernel>>,
) {
    let a = gfx950
        .fp4_a_global_row_major(a_bits, 0, 16, 128, 128)
        .unwrap();
    let b = gfx950
        .fp4_b_global_row_major(b_bits, 0, 128, 16, 16)
        .unwrap();
    let lhs = a.load_m16k128(lane, 0, 0);
    let rhs = b.load_k128n16(lane, 0, 0);
    let accumulator = gfx950.fp4_zero_accumulator(lane);
    let _ = gfx950.multiply_accumulate_fp4(lhs, rhs, accumulator);
    let _ = math.exp_f32(1.0);
}

fn blocked_output<'kernel>(
    index: ThreadIndex<Index1D, Root<'kernel>>,
    output: &mut Global<'kernel, f32, DisjointWrite<Blocked<Index1D, 64, 4>>, Root<'kernel>>,
) {
    if let Some(block) = index.checked_block::<64, 4>() {
        let _ = output.store_block(&block, 0, 1.0);
    }
}

fn transpose_epoch<'operation, 'workgroup>(
    subgroup: &'operation Subgroup<'workgroup, SubgroupWidth64, Root<'workgroup>, InitialEpoch>,
    epoch: &'operation WorkgroupEpoch<'workgroup, Root<'workgroup>, InitialEpoch>,
    workgroup: WorkgroupCapability<'workgroup, Root<'workgroup>, InitialEpoch>,
    matrix: &GlobalGfx950Fp4MfmaAMatrix<'_, '_, MatrixBrand<'workgroup>, Root<'workgroup>>,
) {
    let wave16 = subgroup.gfx950_wave16(epoch);
    let tile = wave16.transpose_tile::<Gfx950Fp4E2M1>();
    let staged = tile.stage_k_transposed(matrix, 0, 0);
    let (next_workgroup, published) = staged.publish(workgroup);
    let next_subgroup = next_workgroup.subgroup::<SubgroupWidth64>();
    let _ = published.read_mfma_fragment(next_subgroup.lane());
}

fn main() {}
