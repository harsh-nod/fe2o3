use vstd::prelude::*;

include!("../../src/elementwise_bodies.rs");

verus! {

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum ElementwiseError {
    DomainLengthMismatch,
}

/// Exact mutation: every copy reads source element zero instead of its
/// identity-owned source element.
pub fn mutated_copy_source(_thread: usize) -> (source: usize)
    ensures
        source == 0,
{
    0
}

pub fn mutated_copy_claims_identity_source(
    thread: usize,
    input: &[i64],
    output: &mut [i64],
) -> (result: Result<(), ElementwiseError>)
    requires
        input@.len() == old(output)@.len(),
        0 < thread < old(output)@.len(),
        input@[0] != input@[thread as int],
    ensures
        result.is_ok(),
        final(output)@ == old(output)@.update(thread as int, input@[thread as int]), // mutated_copy_claims_identity_source
{
    copy_kernel_body!(
        thread,
        mutated_copy_source,
        input,
        output,
        ElementwiseError::DomainLengthMismatch
    )
}

} // verus!
