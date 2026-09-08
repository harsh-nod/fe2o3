#![forbid(unsafe_code)]

use fe2o3_device::{
    CurrentTarget, DisjointWrite, Global, Index1D, KernelCapabilityBrand, KernelContext, ReadOnly,
    RegisteredLaunch, StrictIeee, SubgroupWidth64, kernel,
};

#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn logical_context_has_no_physical_kernarg(
    context: KernelContext<'_>,
    input: Global<'_, u8, ReadOnly>,
    mut output: Global<'_, u8, DisjointWrite<Index1D>>,
) {
    let index = context.invocation().index_1d();
    let value = input.load(index.get()).unwrap_or(0);
    let _ = output.store(index.into_disjoint(), value);
}

fn branded_gfx950_path<'kernel, Kernel>(
    mut context: KernelContext<'kernel, Kernel>,
    lhs: Global<
        'kernel,
        u8,
        ReadOnly,
        KernelCapabilityBrand<'kernel, Kernel, CurrentTarget, RegisteredLaunch>,
    >,
    rhs: Global<
        'kernel,
        u8,
        ReadOnly,
        KernelCapabilityBrand<'kernel, Kernel, CurrentTarget, RegisteredLaunch>,
    >,
) {
    let policy = context.numerical_policy::<StrictIeee>();
    context.with_workgroup(|workgroup| {
        let subgroup = workgroup.subgroup::<SubgroupWidth64>();
        subgroup.with_matrix(workgroup.epoch(), |matrix, lane| {
            let policy_matrix = matrix.with_numerical_policy(&policy);
            let matrix = policy_matrix.gfx950();
            let lhs = matrix
                .fp4_a_global_row_major(&lhs, 0, 16, 128, 128)
                .unwrap()
                .load_m16k128(lane, 0, 0);
            let rhs = matrix
                .fp4_b_global_row_major(&rhs, 0, 128, 16, 16)
                .unwrap()
                .load_k128n16(lane, 0, 0);
            let accumulator = matrix.fp4_zero_accumulator(lane);
            let _ = matrix
                .multiply_accumulate_fp4(lhs, rhs, accumulator)
                .into_values();
        });
    });
}
