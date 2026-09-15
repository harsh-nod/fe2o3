// Registered source for the separate Global FP4/FP8 constructor import tests.
#![no_std]
use fe2o3_device::{
    Global, KernelContext, KernelError, KernelResult, ReadOnly, StrictIeee, SubgroupWidth64, kernel,
};

#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1]))]
pub fn policy_global_matrix_constructors(
    mut context: KernelContext<'_>,
    bits: Global<'_, u8, ReadOnly>,
    offset: usize,
    rows: usize,
    columns: usize,
    stride: usize,
) -> KernelResult {
    let policy = context.numerical_policy::<StrictIeee>();
    context.with_workgroup(|workgroup| {
        let subgroup = workgroup.subgroup::<SubgroupWidth64>();
        subgroup.with_matrix(workgroup.epoch(), |matrix, _lane| {
            let bound = matrix.with_numerical_policy(&policy);
            let narrowed = bound.gfx950();
            let a4 = narrowed.fp4_a_global_row_major(&bits, offset, rows, columns, stride);
            let b4 = narrowed.fp4_b_global_row_major(&bits, offset, rows, columns, stride);
            let a8 = narrowed.fp8_a_global_row_major(&bits, offset, rows, columns, stride);
            let b8 = narrowed.fp8_b_global_row_major(&bits, offset, rows, columns, stride);
            let Ok(_a4) = a4 else {
                return Err(KernelError::InvalidArgument);
            };
            let Ok(_b4) = b4 else {
                return Err(KernelError::InvalidArgument);
            };
            let Ok(_a8) = a8 else {
                return Err(KernelError::InvalidArgument);
            };
            let Ok(_b8) = b8 else {
                return Err(KernelError::InvalidArgument);
            };
            Ok(())
        })
    })
}
