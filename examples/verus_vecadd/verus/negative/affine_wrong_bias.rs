use vstd::prelude::*;

include!("../../src/elementwise_bodies.rs");

verus! {

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum ElementwiseError {
    DomainLengthMismatch,
}

pub open spec fn affine_math(value: i16, scale: i16, bias: i32) -> int {
    value as int * scale as int + bias as int
}

/// Exact mutation: the adapter adds one to the requested affine bias.
pub fn mutated_affine(value: i16, scale: i16, bias: i32) -> (result: i64)
    ensures
        result as int == affine_math(value, scale, bias) + 1,
{
    assert(i16::MIN as int <= value as int <= i16::MAX as int);
    assert(i16::MIN as int <= scale as int <= i16::MAX as int);
    assert(i32::MIN as int <= bias as int <= i32::MAX as int);
    assert(i64::MIN as int
        <= value as int * scale as int + bias as int + 1
        <= i64::MAX as int) by (nonlinear_arith);
    value as i64 * scale as i64 + bias as i64 + 1
}

pub fn mutated_affine_claims_requested_bias(
    thread: usize,
    input: &[i16],
    output: &mut [i64],
    scale: i16,
    bias: i32,
) -> (result: Result<(), ElementwiseError>)
    requires
        input@.len() == old(output)@.len(),
        thread < old(output)@.len(),
    ensures
        result.is_ok(),
        final(output)@ == old(output)@.update(thread as int, affine_math(input@[thread as int], scale, bias) as i64), // mutated_affine_claims_requested_bias
{
    affine_map_kernel_body!(
        thread,
        mutated_affine,
        input,
        output,
        scale,
        bias,
        ElementwiseError::DomainLengthMismatch
    )
}

} // verus!
