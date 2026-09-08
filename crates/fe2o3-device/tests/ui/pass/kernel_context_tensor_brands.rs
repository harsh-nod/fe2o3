use fe2o3_device::{
    Bf16MfmaAFragment, Bf16MfmaAMatrix, Bf16MfmaBFragment, Bf16MfmaBMatrix, CurrentTarget,
    F32AccumulatorFragment, Global, KernelCapabilityBrand, KernelContext, MatrixCapability,
    ReadOnly, RegisteredLaunch, StrictIeee, Wave64, WaveLane,
};

enum TensorKernel {}

type Brand<'kernel> = KernelCapabilityBrand<'kernel, TensorKernel, CurrentTarget, RegisteredLaunch>;

fn branded_tensor_surface<'kernel>(
    context: KernelContext<'kernel, TensorKernel>,
    bf16_bits: &[u16],
    fp4_bits: &Global<'kernel, u8, ReadOnly, Brand<'kernel>>,
) {
    let lane: WaveLane<Wave64, Brand<'kernel>> = context.lane();
    let matrix: MatrixCapability<Brand<'kernel>> = context.matrix();

    let lhs_view: Bf16MfmaAMatrix<'_, Brand<'kernel>> =
        matrix.bf16_a_row_major(bf16_bits, 0, 16, 16, 16).unwrap();
    let rhs_view: Bf16MfmaBMatrix<'_, Brand<'kernel>> =
        matrix.bf16_b_row_major(bf16_bits, 0, 16, 16, 16).unwrap();
    let lhs: Bf16MfmaAFragment<'_, Brand<'kernel>> = lhs_view.load_m16k16(&lane, 0, 0);
    let rhs: Bf16MfmaBFragment<'_, Brand<'kernel>> = rhs_view.load_k16n16(&lane, 0, 0);
    let accumulator = matrix.bf16_zero_accumulator(&lane);
    let _: F32AccumulatorFragment<'_, _, _, _, Brand<'kernel>> =
        matrix.multiply_accumulate(lhs, rhs, accumulator);

    let policy = context.numerical_policy::<StrictIeee>();
    let policy_matrix = matrix.with_numerical_policy(&policy);
    let gfx950 = policy_matrix.gfx950();
    let gfx950_lhs_view = gfx950
        .fp4_a_global_row_major(fp4_bits, 0, 16, 128, 128)
        .unwrap();
    let gfx950_rhs_view = gfx950
        .fp4_b_global_row_major(fp4_bits, 0, 128, 16, 16)
        .unwrap();
    let gfx950_lhs = gfx950_lhs_view.load_m16k128(&lane, 0, 0);
    let gfx950_rhs = gfx950_rhs_view.load_k128n16(&lane, 0, 0);
    let gfx950_accumulator = gfx950.fp4_zero_accumulator(&lane);
    let _ = gfx950.multiply_accumulate_fp4(gfx950_lhs, gfx950_rhs, gfx950_accumulator);
}

fn main() {
    let _ = branded_tensor_surface;
}
