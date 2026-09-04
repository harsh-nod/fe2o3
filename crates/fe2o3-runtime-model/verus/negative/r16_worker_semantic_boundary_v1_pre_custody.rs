use vstd::prelude::*;

verus! {

pub struct WorkerStateV1 {
    pub accepted_backend_custodies: nat,
}

pub open spec fn mutated_reject_before_custody_v1(state: WorkerStateV1) -> WorkerStateV1 {
    WorkerStateV1 {
        accepted_backend_custodies: state.accepted_backend_custodies + 1,
    }
}

pub proof fn mutated_invalid_request_remains_pre_custody_v1(state: WorkerStateV1)
    ensures mutated_reject_before_custody_v1(state).accepted_backend_custodies
        == state.accepted_backend_custodies,
{
}

}
