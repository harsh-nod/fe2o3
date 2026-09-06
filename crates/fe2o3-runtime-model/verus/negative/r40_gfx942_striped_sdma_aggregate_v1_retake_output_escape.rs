// Expected-negative R40 mutation: post-retirement model-retake failure exposes the completed
// ticket vector instead of retaining it exclusively in terminal custody.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_completed_output_len_v1(request_count: nat) -> nat { request_count }
pub open spec fn mutated_terminal_retained_len_v1() -> nat { 0 }
pub proof fn mutated_retake_failure_keeps_terminal_only_v1(request_count: nat)
    requires request_count > 0,
    ensures
        mutated_completed_output_len_v1(request_count) == 0,
        mutated_terminal_retained_len_v1() == request_count,
{}
}
