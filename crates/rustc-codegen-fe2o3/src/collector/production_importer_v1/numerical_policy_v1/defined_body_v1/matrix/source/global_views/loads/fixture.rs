#![no_std]
use fe2o3_device::{
    Global, KernelContext, KernelError, KernelResult, ReadOnly, StrictIeee, SubgroupWidth64, kernel,
};

#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1]))]
pub fn policy_global_matrix_loads(
    mut context: KernelContext<'_>,
    bits: Global<'_, u8, ReadOnly>,
    offset: usize,
    rows: usize,
    columns: usize,
    stride: usize,
    first_base: usize,
    second_base: usize,
) -> KernelResult {
    let policy = context.numerical_policy::<StrictIeee>();
    context.with_workgroup(|workgroup| {
        let subgroup = workgroup.subgroup::<SubgroupWidth64>();
        subgroup.with_matrix(workgroup.epoch(), |matrix, lane| {
            let bound = matrix.with_numerical_policy(&policy);
            let narrowed = bound.gfx950();
            let Ok(a4) = narrowed.fp4_a_global_row_major(&bits, offset, rows, columns, stride)
            else {
                return Err(KernelError::InvalidArgument);
            };
            let Ok(b4) = narrowed.fp4_b_global_row_major(&bits, offset, rows, columns, stride)
            else {
                return Err(KernelError::InvalidArgument);
            };
            let Ok(a8) = narrowed.fp8_a_global_row_major(&bits, offset, rows, columns, stride)
            else {
                return Err(KernelError::InvalidArgument);
            };
            let Ok(b8) = narrowed.fp8_b_global_row_major(&bits, offset, rows, columns, stride)
            else {
                return Err(KernelError::InvalidArgument);
            };
            let _a4 = a4.load_m16k128(lane, first_base, second_base);
            let _b4 = b4.load_k128n16(lane, first_base, second_base);
            let _a8 = a8.load_m16k128(lane, first_base, second_base);
            let _b8 = b8.load_k128n16(lane, first_base, second_base);
            Ok(())
        })
    })
}
