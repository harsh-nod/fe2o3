use vstd::prelude::*;

#[path = "../vecadd.rs"]
mod model;

verus! {

/// Mutation: an input is indexed before the shared body's `get_mut` guard.
/// The remaining call retains the production theorem's exact conditional
/// premises, isolating this one unguarded access.
pub fn mutated_real_kernel_bypasses_output_guard(
    thread: model::ModelGpuThreadIndex,
    a: &[model::ModelFloat],
    b: &[model::ModelFloat],
    output: model::ModelGpuDisjointSlice,
    Ghost(evidence): Ghost<model::VecAddSourceEvidence>,
) -> (result: model::ModelGpuDisjointSlice)
    requires
        a@.len() == output.values@.len(),
        b@.len() == output.values@.len(),
        thread.linear < output.values@.len() ==>
            model::real_vecadd_source_evidence_is_valid(
                evidence,
                output.values@.len(),
                thread.linear as nat,
            ),
{
    let idx = model::model_gpu_thread::index_1d(thread);
    let i = idx.get();
    let _bypassed_input = a[i]; // real_kernel_guard_bypass_input_index
    model::real_kernel_vecadd_body(thread, a, b, output, Ghost(evidence))
}

} // verus!
