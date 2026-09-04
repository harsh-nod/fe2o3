use vstd::prelude::*;
verus! {
pub open spec fn mutated_unauthenticated_dirty_v1(window_bytes: nat) -> nat { window_bytes }
pub proof fn mutated_d2d_unauthenticated_completion_may_certify_dirty_v1()
    ensures mutated_unauthenticated_dirty_v1(4096) == 0, {}
}
