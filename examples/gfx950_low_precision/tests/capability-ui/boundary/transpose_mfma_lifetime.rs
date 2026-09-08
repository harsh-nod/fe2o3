#![forbid(unsafe_code)]

use fe2o3_device::{
    CurrentTarget, Gfx950Fp8E4M3, Global, InitialEpoch, KernelCapabilityBrand,
    NumericalPolicyCapability, ReadOnly, RegisteredLaunch, StrictIeee, SubgroupWidth64,
    WorkgroupCapability,
};

enum Kernel {}

type Root<'kernel> = KernelCapabilityBrand<'kernel, Kernel, CurrentTarget, RegisteredLaunch>;

fn transpose_then_mfma<'kernel, 'workgroup>(
    workgroup: WorkgroupCapability<'workgroup, Root<'kernel>, InitialEpoch>,
    policy: &NumericalPolicyCapability<Root<'kernel>, StrictIeee>,
    query: &Global<'kernel, u8, ReadOnly, Root<'kernel>>,
    key: &Global<'kernel, u8, ReadOnly, Root<'kernel>>,
) {
    let subgroup = workgroup.subgroup::<SubgroupWidth64>();
    let wave16 = subgroup.gfx950_wave16(workgroup.epoch());
    let transpose = wave16.transpose_tile::<Gfx950Fp8E4M3>();
    let staged = subgroup.with_matrix(workgroup.epoch(), |matrix, _lane| {
        let policy_matrix = matrix.with_numerical_policy(policy);
        let matrix = policy_matrix.gfx950();
        let key = matrix.fp8_a_global_row_major(key, 0, 16, 128, 128).unwrap();
        transpose.stage_k_transposed(&key, 0, 0)
    });
    let (workgroup, published) = staged.publish(workgroup);
    let subgroup = workgroup.subgroup::<SubgroupWidth64>();
    subgroup.with_matrix(workgroup.epoch(), |matrix, lane| {
        let policy_matrix = matrix.with_numerical_policy(policy);
        let matrix = policy_matrix.gfx950();
        let query = matrix
            .fp8_a_global_row_major(query, 0, 16, 128, 128)
            .unwrap()
            .load_m16k128(lane, 0, 0);
        let key = published.read_mfma_fragment(lane);
        let accumulator = matrix.fp8_zero_accumulator(lane);
        let _ = matrix.multiply_accumulate_fp8(query, key, accumulator);
    });
}
