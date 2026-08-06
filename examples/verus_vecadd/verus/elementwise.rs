use vstd::prelude::*;

include!("../src/elementwise_bodies.rs");

verus! {

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum ElementwiseError {
    DomainLengthMismatch,
    GatherIndexOutOfBounds,
}

pub fn identity_source(thread: usize) -> (source: usize)
    ensures
        source == thread,
{
    thread
}

/// Total mathematical affine semantics. This is integer arithmetic, not an
/// IEEE floating-point model.
pub open spec fn affine_math(value: i16, scale: i16, bias: i32) -> int {
    value as int * scale as int + bias as int
}

/// The widened executable adapter refines `affine_math` for every value in the
/// input types; no overflow premise is needed.
pub fn exact_affine(value: i16, scale: i16, bias: i32) -> (result: i64)
    ensures
        result as int == affine_math(value, scale, bias),
{
    assert(i16::MIN as int <= value as int <= i16::MAX as int);
    assert(i16::MIN as int <= scale as int <= i16::MAX as int);
    assert(i32::MIN as int <= bias as int <= i32::MAX as int);
    assert(i64::MIN as int
        <= value as int * scale as int + bias as int
        <= i64::MAX as int) by (nonlinear_arith);
    value as i64 * scale as i64 + bias as i64
}

pub fn selected_source(indices: &[usize], thread: usize) -> (source: usize)
    requires
        thread < indices@.len(),
    ensures
        source == indices@[thread as int],
{
    indices[thread]
}

/// Verifies one identity copy and its frame condition from the shared body.
pub fn verified_copy_thread(
    thread: usize,
    input: &[i64],
    output: &mut [i64],
) -> (result: Result<(), ElementwiseError>)
    requires
        input@.len() == old(output)@.len(),
        thread < old(output)@.len(),
    ensures
        result.is_ok(),
        final(output)@ == old(output)@.update(thread as int, input@[thread as int]),
{
    copy_kernel_body!(
        thread,
        identity_source,
        input,
        output,
        ElementwiseError::DomainLengthMismatch
    )
}

/// Verifies exact total integer affine semantics and the identity-write frame.
pub fn verified_affine_map_thread(
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
        final(output)@ == old(output)@.update(
            thread as int,
            affine_math(input@[thread as int], scale, bias) as i64,
        ),
{
    affine_map_kernel_body!(
        thread,
        exact_affine,
        input,
        output,
        scale,
        bias,
        ElementwiseError::DomainLengthMismatch
    )
}

/// Verifies the selected gather read is in bounds before the identity write.
pub fn verified_gather_thread(
    thread: usize,
    input: &[i64],
    indices: &[usize],
    output: &mut [i64],
) -> (result: Result<(), ElementwiseError>)
    requires
        indices@.len() == old(output)@.len(),
        thread < old(output)@.len(),
        indices@[thread as int] < input@.len(),
    ensures
        result.is_ok(),
        final(output)@ == old(output)@.update(
            thread as int,
            input@[indices@[thread as int] as int],
        ),
{
    gather_kernel_body!(
        thread,
        selected_source,
        input,
        indices,
        output,
        ElementwiseError::DomainLengthMismatch,
        ElementwiseError::GatherIndexOutOfBounds
    )
}

} // verus!
