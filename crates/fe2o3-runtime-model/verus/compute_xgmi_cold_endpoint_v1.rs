//! Shared cold predicate only; queue-state observations and native effects are not refined.
use vstd::prelude::*;
include!("../../fe2o3-kfd/src/queue_live/compute_xgmi_cold_body.rs");

verus! {

struct ComputeXgmiColdEndpointFactsV1 {
    completion_releasable: bool,
    submission_pristine: bool,
    dispatch_attached: bool,
    unpublished_clear: bool,
    detached_data_count: usize,
    detached_generation_present: bool,
    detached_identity_count: usize,
    detached_insertion_present: bool,
    next_persistent_generation: u64,
}

spec fn initial_facts(facts: ComputeXgmiColdEndpointFactsV1) -> bool {
    facts.completion_releasable
        && facts.submission_pristine
        && !facts.dispatch_attached
        && facts.unpublished_clear
        && facts.detached_data_count == 0
        && !facts.detached_generation_present
        && facts.detached_identity_count == 0
        && !facts.detached_insertion_present
        && facts.next_persistent_generation == 1
}

fn compute_xgmi_cold_endpoint_is_quiescent_v1(facts: ComputeXgmiColdEndpointFactsV1)
    -> (result: bool)
    ensures result == initial_facts(facts),
{
    compute_xgmi_cold_endpoint_body_v1!(facts)
}

// This is the production combination of an unchanged established predicate and
// the shared cold predicate. The established predicate's semantics are a premise.
fn extend_established_quiescence_v1(
    established: bool, primary_session: bool, facts: ComputeXgmiColdEndpointFactsV1,
)
    -> (result: bool)
    ensures
        established ==> result,
        (!primary_session || !initial_facts(facts)) ==> result == established,
        result && !established ==> primary_session && initial_facts(facts),
        result == (established || (primary_session && initial_facts(facts))),
{
    let cold = primary_session && compute_xgmi_cold_endpoint_is_quiescent_v1(facts);
    established || cold
}

}
