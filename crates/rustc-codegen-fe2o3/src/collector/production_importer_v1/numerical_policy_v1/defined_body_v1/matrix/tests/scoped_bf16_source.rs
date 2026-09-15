#![no_std]
use fe2o3_device::{
    Global, KernelContext, KernelError, KernelResult, ReadOnly, StrictIeee, SubgroupWidth64, kernel,
};

#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1]))]
pub fn scoped_bf16_lane_source(
    mut context: KernelContext<'_>,
    a: Global<'_, u16, ReadOnly>,
    b: Global<'_, u16, ReadOnly>,
    n: u32,
) -> KernelResult {
    let n = n as usize;
    let policy = context.numerical_policy::<StrictIeee>();
    context.with_workgroup(|workgroup| {
        let subgroup = workgroup.subgroup::<SubgroupWidth64>();
        subgroup.with_matrix(workgroup.epoch(), |matrix, lane| {
            let bound = matrix.with_numerical_policy(&policy);
            let Ok(a) = bound.bf16_a_global_row_major(&a, 0, n, n, n) else {
                return Err(KernelError::InvalidArgument);
            };
            let Ok(b) = bound.bf16_b_global_row_major(&b, 0, n, n, n) else {
                return Err(KernelError::InvalidArgument);
            };
            let _a0 = a.load_m16k16(lane, 0, 0);
            let _b0 = b.load_k16n16(lane, 0, 0);
            let _a1 = a.load_m16k16(lane, 0, 16);
            let _b1 = b.load_k16n16(lane, 16, 0);
            Ok(())
        })
    })
}
