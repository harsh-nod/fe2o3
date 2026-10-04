use vstd::prelude::*;

include!("../../src/elementwise_bodies.rs");

verus! {

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum ElementwiseError {
    DomainLengthMismatch,
    GatherIndexOutOfBounds,
}

/// Exact mutation: every gather reads source element zero instead of the index
/// selected by the index buffer.
pub fn mutated_gather_source(_indices: &[usize], _thread: usize) -> (source: usize)
    ensures
        source == 0,
{
    0
}

pub fn mutated_gather_claims_selected_index(
    thread: usize,
    input: &[i64],
    indices: &[usize],
    output: &mut [i64],
) -> (result: Result<(), ElementwiseError>)
    requires
        indices@.len() == old(output)@.len(),
        thread < old(output)@.len(),
        0 < indices@[thread as int] < input@.len(),
        input@[0] != input@[indices@[thread as int] as int],
    ensures
        result.is_ok(),
        final(output)@ == old(output)@.update(thread as int, input@[indices@[thread as int] as int]), // mutated_gather_claims_selected_index
{
    gather_kernel_body!(
        thread,
        mutated_gather_source,
        input,
        indices,
        output,
        ElementwiseError::DomainLengthMismatch,
        ElementwiseError::GatherIndexOutOfBounds
    )
}

} // verus!
