use vstd::prelude::*;
use vstd::std_specs::ops::*;

#[path = "../vecadd.rs"]
mod model;

verus! {

/// Expected failure: all non-bounds obligations are retained, but an
/// arbitrary launch witness cannot execute the real shared body.
pub fn mutated_real_kernel_without_thread_bounds(
    thread: model::ModelGpuThreadIndex,
    a: &[f32],
    b: &[f32],
    output: model::ModelGpuDisjointSlice,
    Ghost(evidence): Ghost<model::VecAddSourceEvidence>,
) -> (result: model::ModelGpuDisjointSlice)
    requires
        a@.len() == output.values@.len(),
        b@.len() == output.values@.len(),
        forall |index: int| 0 <= index < output.values@.len() ==>
            a@[index].add_req(b@[index]),
        model::real_vecadd_source_evidence_is_valid(
            evidence,
            output.values@.len(),
            thread.linear as nat,
        ),
{
    model::real_kernel_vecadd_body( // rejects_missing_real_kernel_thread_bound
        thread,
        a,
        b,
        output,
        Ghost(evidence),
    )
}

} // verus!
