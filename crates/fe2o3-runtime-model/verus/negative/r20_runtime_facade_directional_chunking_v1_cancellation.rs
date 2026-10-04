use vstd::prelude::*;
verus! {
pub open spec fn mutated_cancel_v1(completed: nat) -> nat { 0 }
pub proof fn mutated_partial_progress_cannot_cancel_v1()
    ensures mutated_cancel_v1(4096) == 4096, {}
}
