use vstd::prelude::*;

#[path = "../two_kernel.rs"]
mod model;

verus! {

/// Mutation: reads alpha's input before the output ownership/bounds guard.
pub fn mutated_alpha_bypasses_output_guard(
    thread: usize,
    scale: i16,
    input: &[i16],
    output: model::ModelDisjointSlice,
    Ghost(evidence): Ghost<model::AlphaEvidence>,
) -> (result: model::ModelDisjointSlice)
    requires
        input@.len() == output.values@.len(),
        thread < output.values@.len() ==>
            model::alpha_evidence_is_valid(
                evidence,
                output.values@.len(),
                thread as nat,
            ),
{
    let _unguarded = input[thread]; // mutated_alpha_bypasses_output_guard
    model::verified_alpha_thread(thread, scale, input, output, Ghost(evidence))
}

} // verus!
