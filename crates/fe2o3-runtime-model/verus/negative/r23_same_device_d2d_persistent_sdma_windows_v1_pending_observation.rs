use vstd::prelude::*;
verus! {
pub open spec fn mutated_pending_dirty_bytes_v1(prior: nat) -> nat { prior + 1 }
pub proof fn mutated_d2d_pending_is_observation_only_v1()
    ensures mutated_pending_dirty_bytes_v1(0) == 0, {}
}
