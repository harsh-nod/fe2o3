use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub enum ResponseV1 { SuccessNonzero, Rejected, Quiescent }

pub struct WorkerStateV1 {
    pub attempted_requests: nat,
    pub accepted_backend_custodies: nat,
}

pub open spec fn mutated_observe_response_v1(
    state: WorkerStateV1,
    _response: ResponseV1,
) -> WorkerStateV1 {
    WorkerStateV1 {
        accepted_backend_custodies: state.accepted_backend_custodies + 1,
        ..state
    }
}

pub proof fn mutated_rejection_does_not_accept_backend_custody_v1(state: WorkerStateV1)
    ensures
        mutated_observe_response_v1(state, ResponseV1::Rejected)
            .accepted_backend_custodies == state.accepted_backend_custodies,
{
}

}
