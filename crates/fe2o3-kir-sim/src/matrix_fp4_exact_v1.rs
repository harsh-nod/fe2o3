//! Exact identity-scale FP4 subdomain; no arbitrary scaling or hardware authority.
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
            == Some(TensorLayoutContractV1::gfx950_scaled_mfma_fp4_e2m1_f32_m16n16k128_wave64())
}

/// Decode one nibble in half units. -0 is valid FP4, but outside this model.
pub(crate) fn half_units(nibble: u32) -> Option<i64> {
    if nibble >= 16 || nibble == 8 {
        return None;
    }
    let magnitude = [0_i64, 1, 2, 3, 4, 6, 8, 12][(nibble & 7) as usize];
    Some(if nibble & 8 == 0 {
        magnitude
    } else {
        -magnitude
    })
}

/// +0 or exactly quarter-integral normal F32, |C| <= 2^20, in quarter units.
/// The discarded mantissa bits must be zero: this is not a rounding operation.
pub(crate) fn accumulator_quarters(bits: u32) -> Option<i64> {
    if bits == 0 {
        return Some(0);
    }
    let magnitude = bits & 0x7fff_ffff;
    if magnitude == 0 || magnitude > 0x4980_0000 {
        return None;
    }
    let exponent = (magnitude >> 23) & 255;
    if !(125..=147).contains(&exponent) {
        return None;
    }
    let discarded = 148 - exponent;
    let significand = (1_u32 << 23) | (magnitude & 0x7f_ffff);
    if significand & ((1_u32 << discarded) - 1) != 0 {
        return None;
    }
    let value = i64::from(significand >> discarded);
    Some(if bits >> 31 == 0 { value } else { -value })
}

/// Exact bit construction for the proved result bound; cancellation yields +0.
/// |4*C + sum128(2*A * 2*B)| <= 4,212,736 < 2^23.
/// Thus every product and every possible partial sum is an exact F32 quarter.
pub(crate) fn quarters_to_bits(value: i64) -> u32 {
    if value == 0 {
        return 0;
    }
    let magnitude = value.unsigned_abs();
    assert!(magnitude <= 4_212_736, "proved FP4 exact result bound");
    let exponent = 63 - magnitude.leading_zeros();
    let fraction = ((magnitude - (1_u64 << exponent)) << (23 - exponent)) as u32;
    (u32::from(value < 0) << 31) | ((exponent + 125) << 23) | fraction
}

#[cfg(test)]
#[path = "matrix_fp4_exact_v1_tests.rs"]
mod tests;
