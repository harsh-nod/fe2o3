use vstd::prelude::*;
verus! {
pub open spec fn mutated_poll_quiescent_v1(_expected: nat, _observed: nat) -> bool { true }
pub proof fn mutated_foreign_submission_cannot_observe_quiescence_v1()
    ensures !mutated_poll_quiescent_v1(7, 8), {}
}
