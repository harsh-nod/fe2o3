//! Closed numerical subdomain, not general BF16 or hardware authority.
use fe2o3_kernel_ir::{
    Convergence, MatrixMultiplyProfile, MatrixOperation, MatrixOperationKind, SynchronizationScope,
    TensorLayoutContractV1,
};

/// Positional operand only; no user values are retained in a domain refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum MatrixInputRoleV1 {
    A = 0,
    B = 1,
    Accumulator = 2,
}

pub(crate) fn supported(matrix: &MatrixOperation) -> bool {
    if !matches!(&matrix.kind, MatrixOperationKind::MultiplyAccumulate { profile, .. }
        if *profile == MatrixMultiplyProfile::bf16_f32_m16n16k16_wave64())
        || matrix.active_lanes != 64
        || matrix.convergence != Convergence::uniform(SynchronizationScope::Subgroup)
    {
        return false;
    }
    let Some(layout) = matrix.tensor_layout else {
        return false;
    };
    for a_xor in [false, true] {
        for b_xor in [false, true] {
            for zero_tail in [false, true] {
                let mut exact = TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64();
                if a_xor {
                    exact = exact.with_a_lds_xor4();
                }
                if b_xor {
                    exact = exact.with_b_lds_xor4();
                }
                if zero_tail {
                    exact = exact.with_zero_filled_predicate_inputs();
                }
                if layout == exact {
                    return true;
                }
            }
        }
    }
    false
}

/// +0 or a signed integral normal F32 within the supplied positive bound.
/// Bit-only classification; no rounding, host float, or silent coercion.
fn integral_f32(bits: u32, maximum_bits: u32) -> bool {
    if bits == 0 {
        return true;
    }
    let magnitude = bits & 0x7fff_ffff;
    if magnitude == 0 || magnitude > maximum_bits {
        return false;
    }
    let exponent = (magnitude >> 23) & 255;
    if !(127..=150).contains(&exponent) {
        return false;
    }
    let discarded = 150 - exponent;
    let fraction_mask = (1_u32 << discarded) - 1;
    magnitude & fraction_mask == 0
}

pub(crate) fn bf16_input(bits: u16) -> bool {
    integral_f32(u32::from(bits) << 16, 0x4180_0000) // |x| <= 16
}
pub(crate) fn f32_accumulator(bits: u32) -> bool {
    integral_f32(bits, 0x4980_0000) // |x| <= 2^20
}

#[cfg(test)]
#[path = "matrix_bf16_exact_v1_tests.rs"]
mod tests;
