use vstd::prelude::*;

#[path = "../vecadd.rs"]
mod model;

verus! {

/// Expected failure: the real shared body requires exclusive output ownership,
/// but this mutation aliases the output allocation with the first input.
pub fn mutated_real_kernel_accepts_output_input_alias(
    thread: model::ModelGpuThreadIndex,
    a: &[model::ModelFloat],
    b: &[model::ModelFloat],
    output: model::ModelGpuDisjointSlice,
    Ghost(evidence): Ghost<model::VecAddSourceEvidence>,
) -> (result: model::ModelGpuDisjointSlice)
    requires
        thread.linear < output.values@.len(),
        a@.len() == output.values@.len(),
        b@.len() == output.values@.len(),
        model::vecadd_source_evidence_is_well_formed(
            evidence,
            output.values@.len(),
            thread.linear as nat,
        ),
        evidence.element_size == 4,
        evidence.a_allocation.address_space_size <= usize::MAX as nat,
        evidence.b_allocation.address_space_size <= usize::MAX as nat,
        evidence.output_allocation.address_space_size <= usize::MAX as nat,
        evidence.output_allocation.id == evidence.a_allocation.id,
        evidence.output_allocation.id != evidence.b_allocation.id,
{
    model::real_kernel_vecadd_body( // rejects_real_output_input_alias
        thread,
        a,
        b,
        output,
        Ghost(evidence),
    )
}

} // verus!
