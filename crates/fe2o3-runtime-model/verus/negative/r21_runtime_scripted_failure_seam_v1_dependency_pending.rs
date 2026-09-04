use vstd::prelude::*;
verus! {
pub open spec fn mutated_dependency_pending_epoch_v1(epoch: nat) -> nat { epoch + 1 }
pub proof fn mutated_dependency_pending_is_observation_only_v1()
    ensures mutated_dependency_pending_epoch_v1(7) == 7, {}
}
