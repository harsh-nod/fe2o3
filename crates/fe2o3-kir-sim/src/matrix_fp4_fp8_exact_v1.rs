//! Exact mixed FP4 E2M1 x OCP E4M3 identity-scale subdomain.
//! The existing layout fixes A to FP4 and B to FP8; no reverse/scaled variant.
pub(crate) use crate::matrix_fp4_exact_v1::half_units;
pub(crate) use crate::matrix_fp8_exact_v1::{
    accumulator_sixteenths, quarter_units, sixteenths_to_bits,
};
use fe2o3_kernel_ir::{
    Convergence, MatrixMultiplyProfile, MatrixOperation, MatrixOperationKind, SynchronizationScope,
    TensorLayoutContractV1,
};

pub(crate) fn supported(matrix: &MatrixOperation) -> bool {
    matches!(&matrix.kind, MatrixOperationKind::ScaledMultiplyAccumulate { profile, .. }
        if *profile == MatrixMultiplyProfile::fp4_e2m1_f32_m16n16k128_wave64())
        && matrix.active_lanes == 64
        && matrix.convergence == Convergence::uniform(SynchronizationScope::Subgroup)
        && matrix.tensor_layout
            == Some(
                TensorLayoutContractV1::gfx950_scaled_mfma_fp4_e2m1_fp8_e4m3_f32_m16n16k128_wave64(
                ),
            )
}

// Q = 16*C + sum128(2 * (2*A) * (4*B)).
// |Q| <= 4,194,304 + 128*2*12*64 = 4,390,912 < 2^23.
// Every product and partial sum is exactly representable as an F32 sixteenth.
// Reuse the existing bit encoders, including their conservative -0 refusal.

#[cfg(test)]
#[path = "matrix_fp4_fp8_exact_v1_tests.rs"]
mod tests;
