//! Exact OCP E4M3 identity-scale subdomain, not arbitrary scaling or hardware authority.
use fe2o3_kernel_ir::{
    Convergence, MatrixMultiplyProfile, MatrixOperation, MatrixOperationKind, SynchronizationScope,
    TensorLayoutContractV1,
};

pub(crate) fn supported(matrix: &MatrixOperation) -> bool {
    matches!(&matrix.kind, MatrixOperationKind::ScaledMultiplyAccumulate { profile, .. }
        if *profile == MatrixMultiplyProfile::fp8_e4m3_f32_m16n16k128_wave64())
        && matrix.active_lanes == 64
        && matrix.convergence == Convergence::uniform(SynchronizationScope::Subgroup)
        && matrix.tensor_layout
            == Some(TensorLayoutContractV1::gfx950_scaled_mfma_fp8_e4m3_f32_m16n16k128_wave64())
}

/// Decode an OCP E4M3 byte exactly in quarter units, |x| <= 16.
/// Valid FP8 -0, smaller fractions and larger finite values are outside this
/// model. In particular this is not FNUZ (which uses a different exponent bias).
pub(crate) fn quarter_units(byte: u32) -> Option<i64> {
    if byte == 0 {
        return Some(0);
    }
    if byte > 255 || byte == 0x80 || (byte & 0x7f) > 0x58 {
        return None;
    }
    let exponent = (byte >> 3) & 15;
    if !(5..=11).contains(&exponent) {
        return None;
    }
    let significand = 8 | (byte & 7);
    let value = if exponent < 8 {
        let discarded = 8 - exponent;
        if significand & ((1_u32 << discarded) - 1) != 0 {
            return None;
        }
        significand >> discarded
    } else {
        significand << (exponent - 8)
    };
    let value = i64::from(value);
    Some(if byte & 0x80 == 0 { value } else { -value })
}

/// +0 or exactly sixteenth-integral normal F32, |C| <= 2^18.
/// Reject rather than round an off-grid bit pattern.
pub(crate) fn accumulator_sixteenths(bits: u32) -> Option<i64> {
    if bits == 0 {
        return Some(0);
    }
    let magnitude = bits & 0x7fff_ffff;
    if magnitude == 0 || magnitude > 0x4880_0000 {
        return None;
    }
    let exponent = (magnitude >> 23) & 255;
    if !(123..=145).contains(&exponent) {
        return None;
    }
    let discarded = 146 - exponent;
    let significand = (1_u32 << 23) | (magnitude & 0x7f_ffff);
    if significand & ((1_u32 << discarded) - 1) != 0 {
        return None;
    }
    let value = i64::from(significand >> discarded);
    Some(if bits >> 31 == 0 { value } else { -value })
}

/// Exact bit construction; cancellation yields +0.
/// |16*C + sum128(4*A * 4*B)| <= 4,718,592 < 2^23.
/// Every product and every possible partial sum is thus an exact F32 sixteenth.
pub(crate) fn sixteenths_to_bits(value: i64) -> u32 {
    if value == 0 {
        return 0;
    }
    let magnitude = value.unsigned_abs();
    assert!(magnitude <= 4_718_592, "proved FP8 exact result bound");
    let exponent = 63 - magnitude.leading_zeros();
    let fraction = ((magnitude - (1_u64 << exponent)) << (23 - exponent)) as u32;
    (u32::from(value < 0) << 31) | ((exponent + 123) << 23) | fraction
}

#[cfg(test)]
#[path = "matrix_fp8_exact_v1_tests.rs"]
mod tests;
