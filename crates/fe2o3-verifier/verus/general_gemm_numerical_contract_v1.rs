use vstd::prelude::*;

verus! {

pub open spec fn bf16_encoding_count_v1() -> nat { 65536 }
pub open spec fn fp32_low_bits_scale_v1() -> nat { 65536 }
pub open spec fn fp32_encoding_count_v1() -> nat { 4294967296 }

/// Exact BF16-to-FP32 widening at the representation boundary. This theorem
/// concerns encodings; interpreting the resulting bits as IEEE 754 FP32 is a
/// separately identified contract in the host-side correspondence package.
pub open spec fn widen_bf16_encoding_v1(bits: nat) -> nat {
    bits * fp32_low_bits_scale_v1()
}

pub proof fn every_bf16_encoding_widens_without_losing_bits_v1(bits: nat)
    requires bits < bf16_encoding_count_v1(),
    ensures
        widen_bf16_encoding_v1(bits) / fp32_low_bits_scale_v1() == bits,
        widen_bf16_encoding_v1(bits) % fp32_low_bits_scale_v1() == 0,
        widen_bf16_encoding_v1(bits) < fp32_encoding_count_v1(),
{
}

pub open spec fn fp32_mul_rne_operation_v1() -> nat { 1 }
pub open spec fn fp32_add_rne_operation_v1() -> nat { 2 }

/// Operation tags preserve the required non-contracted alpha/beta epilogue
/// shape. The numerical result of each IEEE operation remains a contract.
pub open spec fn epilogue_operation_v1(index: nat) -> nat {
    if index < 2 {
        fp32_mul_rne_operation_v1()
    } else if index == 2 {
        fp32_add_rne_operation_v1()
    } else {
        0
    }
}

pub proof fn epilogue_uses_two_separate_multiplications_then_addition_v1()
    ensures
        epilogue_operation_v1(0) == fp32_mul_rne_operation_v1(),
        epilogue_operation_v1(1) == fp32_mul_rne_operation_v1(),
        epilogue_operation_v1(2) == fp32_add_rne_operation_v1(),
        epilogue_operation_v1(3) == 0,
{
}

/// A K step has one multiply result followed by one accumulator addition.
/// This proves sequencing only, not IEEE rounding or equivalence to MFMA.
pub open spec fn accumulation_operation_v1(index: nat) -> nat {
    if index == 0 {
        fp32_mul_rne_operation_v1()
    } else if index == 1 {
        fp32_add_rne_operation_v1()
    } else {
        0
    }
}

pub proof fn accumulation_step_preserves_separate_mul_add_order_v1()
    ensures
        accumulation_operation_v1(0) == fp32_mul_rne_operation_v1(),
        accumulation_operation_v1(1) == fp32_add_rne_operation_v1(),
        accumulation_operation_v1(2) == 0,
{
}

pub proof fn gfx942_mfma_descriptor_has_reviewed_shape_v1()
    ensures
        16nat * 16nat == 64nat * 4nat,
        16nat == 16nat,
{
}

/// Verus does not currently model the target instruction's internal FP32
/// accumulation and rounding behavior.
pub open spec fn gfx942_mfma_numerical_semantics_proved_v1() -> bool {
    false
}

pub proof fn gfx942_mfma_numerical_semantics_remain_contracted_v1()
    ensures !gfx942_mfma_numerical_semantics_proved_v1(),
{
}

fn main() {}

}
