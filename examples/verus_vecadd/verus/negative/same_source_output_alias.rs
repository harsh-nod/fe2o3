use vstd::prelude::*;

#[path = "../vecadd.rs"]
mod model;

verus! {

/// Expected failure: all source-evidence obligations except output/input
/// disjointness hold, but the output allocation aliases the first input.
pub fn mutated_same_source_accepts_output_input_alias(
    domain: model::ModelLaunchDomain1d,
    thread: model::ModelThreadInDomain1d,
    a: &[u32],
    b: &[u32],
    output: &mut [u32],
    Ghost(evidence): Ghost<model::VecAddSourceEvidence>,
) -> (result: Result<(), model::VecAddError>)
    requires
        thread.launch == domain,
        thread.linear < domain.length,
        a@.len() == domain.length,
        b@.len() == domain.length,
        old(output)@.len() == domain.length,
        a@[thread.linear as int] as nat + b@[thread.linear as int] as nat
            <= u32::MAX as nat,
        model::vecadd_source_evidence_is_well_formed(
            evidence,
            domain.length as nat,
            thread.linear as nat,
        ),
        evidence.output_allocation.id == evidence.a_allocation.id,
        evidence.output_allocation.id != evidence.b_allocation.id,
    ensures
        result.is_ok(),
{
    model::same_source_vecadd_thread( // rejects_output_input_alias
        domain,
        thread,
        a,
        b,
        output,
        Ghost(evidence),
    )
}

} // verus!
