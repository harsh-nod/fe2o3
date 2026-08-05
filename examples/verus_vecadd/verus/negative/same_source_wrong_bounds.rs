use vstd::prelude::*;

#[path = "../vecadd.rs"]
mod model;

verus! {

/// Expected failure: the shared body requires a branded in-bounds thread, but
/// this mutation tries to invoke it for an arbitrary runtime and ghost index.
pub fn mutated_same_source_without_thread_bounds(
    domain: model::ModelLaunchDomain1d,
    thread: model::ModelThreadInDomain1d,
    a: &[u32],
    b: &[u32],
    output: &mut [u32],
    Ghost(evidence): Ghost<model::VecAddSourceEvidence>,
) -> (result: Result<(), model::VecAddError>)
    requires
        thread.launch == domain,
        a@.len() == domain.length,
        b@.len() == domain.length,
        old(output)@.len() == domain.length,
    ensures
        result.is_ok(),
{
    model::same_source_vecadd_thread(domain, thread, a, b, output, Ghost(evidence)) // mutated_same_source_without_thread_bounds
}

} // verus!
