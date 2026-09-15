#![no_std]
use fe2o3_device::{
    Gfx950F32AccumulatorFragment, Gfx950Fp4E2M1, Gfx950Fp8E4M3, Global, KernelContext, KernelError,
    KernelResult, ReadOnly, StrictIeee, SubgroupWidth64, kernel,
};

#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1]))]
pub fn matrix_terminal_abis(
    mut context: KernelContext<'_>,
    bits: Global<'_, u8, ReadOnly>,
    rows: usize,
    columns: usize,
    stride: usize,
) -> KernelResult {
    let policy = context.numerical_policy::<StrictIeee>();
    context.with_workgroup(|workgroup| {
        let subgroup = workgroup.subgroup::<SubgroupWidth64>();
        subgroup.with_matrix(workgroup.epoch(), |matrix, lane| {
            let bound = matrix.with_numerical_policy(&policy);
            let narrowed = bound.gfx950();
            let Ok(a4) = narrowed.fp4_a_global_row_major(&bits, 0, rows, columns, stride) else {
                return Err(KernelError::InvalidArgument);
            };
            let Ok(b4) = narrowed.fp4_b_global_row_major(&bits, 0, rows, columns, stride) else {
                return Err(KernelError::InvalidArgument);
            };
            let Ok(a8) = narrowed.fp8_a_global_row_major(&bits, 0, rows, columns, stride) else {
                return Err(KernelError::InvalidArgument);
            };
            let Ok(b8) = narrowed.fp8_b_global_row_major(&bits, 0, rows, columns, stride) else {
                return Err(KernelError::InvalidArgument);
            };
            let acc4 = Gfx950F32AccumulatorFragment::<Gfx950Fp4E2M1, _>::zero(lane);
            let acc4 = narrowed.multiply_accumulate_fp4(
                a4.load_m16k128(lane, 0, 0),
                b4.load_k128n16(lane, 0, 0),
                acc4,
            );
            let values4 = acc4.into_values();
            let mixed = Gfx950F32AccumulatorFragment::<Gfx950Fp4E2M1, _>::zero(lane);
            let mixed = narrowed.multiply_accumulate_fp4_fp8(
                a4.load_m16k128(lane, 0, 0),
                b8.load_k128n16(lane, 0, 0),
                mixed,
            );
            let values_mixed = mixed.into_values();
            let acc8 = Gfx950F32AccumulatorFragment::<Gfx950Fp8E4M3, _>::zero(lane);
            let acc8 = narrowed.multiply_accumulate_fp8(
                a8.load_m16k128(lane, 0, 0),
                b8.load_k128n16(lane, 0, 0),
                acc8,
            );
            let values8 = acc8.into_values();
            if values4[0] > 0.0 || values_mixed[0] > 0.0 || values8[0] > 0.0 {
                Err(KernelError::InvalidArgument)
            } else {
                Ok(())
            }
        })
    })
}
