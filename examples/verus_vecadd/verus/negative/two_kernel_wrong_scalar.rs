use vstd::prelude::*;

#[path = "../two_kernel.rs"]
mod model;

include!("../../src/two_kernel_bodies.rs");

verus! {

/// Mutation: alpha multiplies by the scalar immediately after the requested
/// scalar instead of the scalar argument itself.
pub fn mutated_alpha_arithmetic(scale: i16, value: i16) -> (result: i64)
    ensures
        result as int == (scale as int + 1) * value as int,
{
    assert(i16::MIN as int <= scale as int <= i16::MAX as int);
    assert(i16::MIN as int <= value as int <= i16::MAX as int);
    assert(i64::MIN as int
        <= (scale as int + 1) * value as int
        <= i64::MAX as int) by (nonlinear_arith);
    (scale as i64 + 1) * value as i64
}

pub fn mutated_alpha_uses_wrong_scalar_result(
    thread: usize,
    scale: i16,
    input: &[i16],
    mut output: model::ModelDisjointSlice,
) -> (result: model::ModelDisjointSlice)
    requires
        input@.len() == output.values@.len(),
        thread < output.values@.len(),
    ensures
        result.values@ == output.values@.update(
            thread as int,
            model::alpha_math(scale, input@[thread as int]) as i64,
        ), // mutated_alpha_uses_wrong_scalar_result
{
    alpha_kernel_body!(thread, mutated_alpha_arithmetic, scale, input, output);
    output
}

} // verus!
