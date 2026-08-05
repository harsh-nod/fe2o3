use vstd::prelude::*;

#[path = "../vecadd.rs"]
mod model;

verus! {

/// Expected failure: this wrapper invokes the shared executable body but
/// claims that it stores one more than the vecadd result.
pub fn mutated_same_source_claims_wrong_sum(
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
        a@[thread.linear as int] as nat + b@[thread.linear as int] as nat + 1
            <= u32::MAX as nat,
        model::vecadd_source_evidence_is_valid(
            evidence,
            domain.length as nat,
            thread.linear as nat,
        ),
    ensures
        result.is_ok(),
        final(output)@ == old(output)@.update(
            thread.linear as int,
            (model::vecadd_value_u32(a@, b@, thread.linear as nat) as nat + 1) as u32,
        ),
{
    model::same_source_vecadd_thread(domain, thread, a, b, output, Ghost(evidence)) // mutated_same_source_claims_wrong_sum
}

} // verus!
