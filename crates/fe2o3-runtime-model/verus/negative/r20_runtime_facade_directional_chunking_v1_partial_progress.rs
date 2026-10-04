use vstd::prelude::*;
verus! {
pub open spec fn mutated_retry_completed_v1(_completed: nat) -> nat { 0 }
pub proof fn mutated_retry_preserves_partial_progress_v1()
    ensures mutated_retry_completed_v1(4194272) == 4194272, {}
}
